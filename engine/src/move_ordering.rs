use std::collections::HashMap;

use crate::types::*;

/// Maximum search depth for killer move storage.
const MAX_DEPTH: usize = 64;

/// Number of killer move slots per depth.
const KILLER_SLOTS: usize = 2;

/// Orders moves to improve alpha-beta pruning efficiency.
///
/// Uses three heuristics:
/// 1. **MVV-LVA** (Most Valuable Victim - Least Valuable Attacker):
///    Prioritizes actions that interact with high-value targets.
/// 2. **Killer moves**: Remembers moves that caused beta cutoffs at the
///    same depth in sibling nodes.
/// 3. **History heuristic**: Tracks cumulative success of each move
///    across the entire search.
#[derive(Debug, Clone)]
pub struct MoveOrderer {
    /// Killer move slots: [depth][slot] -> action key.
    killer_moves: Vec<[Option<String>; KILLER_SLOTS]>,
    /// History table: action_key -> cumulative depth^2 bonus.
    history_table: HashMap<String, f64>,
    /// Counter for total ordering calls (diagnostic).
    ordering_calls: u64,
}

impl MoveOrderer {
    pub fn new() -> Self {
        Self {
            killer_moves: vec![[None, None]; MAX_DEPTH],
            history_table: HashMap::new(),
            ordering_calls: 0,
        }
    }

    /// Sort `actions` in-place so the most promising moves come first.
    ///
    /// The scoring combines MVV-LVA, killer bonuses, and history scores,
    /// then sorts descending.
    pub fn order_moves(
        &self,
        actions: &mut [ExecutionAction],
        state: &OnChainState,
        depth: u32,
    ) {
        if actions.len() <= 1 {
            return;
        }

        // Compute a score for each action and sort by it descending.
        let mut scored: Vec<(usize, f64)> = actions
            .iter()
            .enumerate()
            .map(|(i, action)| {
                let mvv = self.mvv_lva_score(action, state);
                let killer = self.killer_bonus(action, depth);
                let history = self.history_score(action);
                (i, mvv + killer + history)
            })
            .collect();

        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Reorder actions according to scored order.
        let ordered: Vec<ExecutionAction> = scored
            .iter()
            .map(|(idx, _)| actions[*idx].clone())
            .collect();

        for (i, action) in ordered.into_iter().enumerate() {
            actions[i] = action;
        }
    }

    /// Record a move as a killer at the given depth.
    ///
    /// Uses a two-slot replacement scheme: if the move is not already in
    /// slot 0, shift slot 0 to slot 1 and insert the new move in slot 0.
    pub fn update_killer(&mut self, depth: u32, action: &ExecutionAction) {
        let d = depth as usize;
        if d >= MAX_DEPTH {
            return;
        }

        let key = action.action_key();
        let slots = &mut self.killer_moves[d];

        // Don't add if already in slot 0
        if slots[0].as_ref() == Some(&key) {
            return;
        }

        // Shift slot 0 -> slot 1, insert new in slot 0
        slots[1] = slots[0].take();
        slots[0] = Some(key);
    }

    /// Record a successful move in the history table.
    ///
    /// Bonus = depth^2, so deeper cutoffs are weighted more heavily.
    pub fn update_history(&mut self, action: &ExecutionAction, depth: u32) {
        let key = action.action_key();
        let bonus = (depth as f64) * (depth as f64);
        let entry = self.history_table.entry(key).or_insert(0.0);
        *entry += bonus;

        // Age / cap the history to prevent overflow domination
        if *entry > 100_000.0 {
            // Scale all entries down
            for val in self.history_table.values_mut() {
                *val *= 0.5;
            }
        }
    }

    /// MVV-LVA heuristic: score an action based on the "value" of the
    /// target and the "cost" of the attacker.
    ///
    /// For on-chain actions:
    /// - High-value: Liquidations (capturing collateral), large swaps
    /// - Low-cost: Actions with low priority fees
    ///
    /// Returns a score in roughly [0, 100].
    pub fn mvv_lva_score(&self, action: &ExecutionAction, state: &OnChainState) -> f64 {
        let victim_value = self.estimate_victim_value(action, state);
        let attacker_cost = self.estimate_attacker_cost(action);

        // MVV-LVA: maximize victim value, minimize attacker cost
        victim_value - attacker_cost
    }

    /// Estimate the "value" of the target / opportunity.
    fn estimate_victim_value(&self, action: &ExecutionAction, state: &OnChainState) -> f64 {
        // Base value from action kind
        let kind_value = match action.kind {
            ActionKind::Liquidate => 80.0,
            ActionKind::Swap => 50.0,
            ActionKind::RemoveLiquidity => 40.0,
            ActionKind::AddLiquidity => 30.0,
            ActionKind::Transfer => 20.0,
            ActionKind::Unstake => 15.0,
            ActionKind::Stake => 10.0,
        };

        // Amount relative to our balance
        let balance = state
            .token_balances
            .get(&action.token_mint)
            .copied()
            .unwrap_or(1) as f64;
        let amount_ratio = (action.amount as f64 / balance).min(1.0);

        // Pool size factor: bigger pools are safer
        let pool_factor = state
            .pool_states
            .iter()
            .find(|p| p.address == action.pool_address)
            .map(|p| {
                let tvl = p.tvl() as f64;
                if tvl > 0.0 {
                    (action.amount as f64 / tvl).min(1.0)
                } else {
                    0.5
                }
            })
            .unwrap_or(0.5);

        kind_value * (0.5 + amount_ratio * 0.3 + (1.0 - pool_factor) * 0.2)
    }

    /// Estimate the "cost" of executing this action.
    fn estimate_attacker_cost(&self, action: &ExecutionAction) -> f64 {
        // Normalize priority fee: 5000 lamports baseline = 1.0
        let fee_cost = action.priority_fee as f64 / 5000.0;
        // Slippage tolerance as cost
        let slip_cost = action.slippage_bps as f64 / 100.0;

        fee_cost + slip_cost
    }

    /// Return a bonus if the action matches a killer move at this depth.
    pub fn killer_bonus(&self, action: &ExecutionAction, depth: u32) -> f64 {
        let d = depth as usize;
        if d >= MAX_DEPTH {
            return 0.0;
        }

        let key = action.action_key();
        let slots = &self.killer_moves[d];

        if slots[0].as_ref() == Some(&key) {
            50.0 // Primary killer
        } else if slots[1].as_ref() == Some(&key) {
            25.0 // Secondary killer
        } else {
            0.0
        }
    }

    /// Return the history heuristic score for this action.
    pub fn history_score(&self, action: &ExecutionAction) -> f64 {
        let key = action.action_key();
        self.history_table.get(&key).copied().unwrap_or(0.0)
    }

    /// Reset all heuristic state. Called between searches if desired.
    pub fn reset(&mut self) {
        self.killer_moves = vec![[None, None]; MAX_DEPTH];
        self.history_table.clear();
        self.ordering_calls = 0;
    }

    /// Number of times order_moves has been called.
    pub fn total_ordering_calls(&self) -> u64 {
        self.ordering_calls
    }

    /// Number of distinct entries in the history table.
    pub fn history_table_size(&self) -> usize {
        self.history_table.len()
    }

    /// Increment the ordering call counter. Called internally or by the engine.
    pub fn record_ordering_call(&mut self) {
        self.ordering_calls += 1;
    }
}

impl Default for MoveOrderer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn test_state() -> OnChainState {
        let mut balances = HashMap::new();
        balances.insert("SOL".to_string(), 1_000_000);
        balances.insert("USDC".to_string(), 5_000_000);
        OnChainState {
            token_balances: balances,
            pool_states: vec![PoolState::new(
                "pool1",
                10_000_000,
                50_000_000,
                30,
                "SOL",
                "USDC",
            )],
            pending_transactions: Vec::new(),
            slot: 100,
            block_time: 1700000000,
        }
    }
