use sha2::{Digest, Sha256};

use crate::evaluator::PositionEvaluator;
use crate::math;
use crate::types::*;

/// Builds and expands game trees for the minimax search.
///
/// The game tree alternates between Agent moves (maximizing) and Adversary
/// responses (minimizing). Adversary moves model MEV bot behaviour such as
/// sandwich attacks, front-running, back-running, and JIT liquidity provision.
#[derive(Debug, Clone)]
pub struct GameTreeBuilder {
    evaluator: PositionEvaluator,
}

impl GameTreeBuilder {
    pub fn new(evaluator: PositionEvaluator) -> Self {
        Self { evaluator }
    }

    /// Build a complete game tree up to `max_depth` plies.
    ///
    /// The root node represents the current state. At even depths the Agent
    /// moves; at odd depths the Adversary responds.
    pub fn build_tree(
        &self,
        state: &OnChainState,
        actions: &[ExecutionAction],
        threats: &[MevThreat],
        max_depth: u32,
    ) -> GameNode {
        let root_hash = Self::hash_state(state);
        let mut root = GameNode::new_root(root_hash);

        if max_depth == 0 || actions.is_empty() {
            root.is_terminal = true;
            root.score = self.evaluator.evaluate_static(state);
            return root;
        }

        self.build_recursive(&mut root, state, actions, threats, max_depth, 0);
        root
    }

    /// Recursively build the tree by alternating Agent and Adversary plies.
    fn build_recursive(
        &self,
        node: &mut GameNode,
        state: &OnChainState,
        available_actions: &[ExecutionAction],
        threats: &[MevThreat],
        max_depth: u32,
        current_depth: u32,
    ) {
        if current_depth >= max_depth {
            node.is_terminal = true;
            node.score = self.evaluator.evaluate_static(state);
            return;
        }

        match node.player {
            Player::Agent => {
                // Generate agent moves
                let moves = if available_actions.is_empty() {
                    Self::generate_agent_moves(state)
                } else {
                    available_actions.to_vec()
                };

                if moves.is_empty() {
                    node.is_terminal = true;
                    node.score = self.evaluator.evaluate_static(state);
                    return;
                }

                for action in &moves {
                    let new_state = Self::simulate_action(state, action);
                    let child_hash = Self::hash_state(&new_state);
                    let mut child = GameNode::new_child(
                        action.clone(),
                        child_hash,
                        current_depth + 1,
                        Player::Adversary,
                    );

                    // Adversary responds to the agent's action
                    let adversary_threats =
                        Self::generate_adversary_moves(&new_state, action);
                    let combined_threats: Vec<MevThreat> = threats
                        .iter()
                        .chain(adversary_threats.iter())
                        .cloned()
                        .collect();

                    // Convert threats to adversary actions for the next ply
                    let threat_actions: Vec<ExecutionAction> = combined_threats
                        .iter()
                        .map(|t| Self::threat_to_action(t))
                        .collect();

                    self.build_recursive(
                        &mut child,
                        &new_state,
                        &threat_actions,
                        &combined_threats,
                        max_depth,
                        current_depth + 1,
                    );

                    node.children.push(child);
                }
            }
            Player::Adversary => {
                // Adversary ply: model each threat as a possible response
                if available_actions.is_empty() && threats.is_empty() {
                    // No adversary response: "pass" node
                    node.is_terminal = true;
                    node.score = self.evaluator.evaluate_static(state);
                    return;
                }

                // Create one child per threat / adversary action
                let adversary_actions = if !available_actions.is_empty() {
                    available_actions.to_vec()
                } else {
                    threats
                        .iter()
                        .map(|t| Self::threat_to_action(t))
                        .collect()
                };

                if adversary_actions.is_empty() {
                    node.is_terminal = true;
                    node.score = self.evaluator.evaluate_static(state);
                    return;
                }

                for adv_action in &adversary_actions {
                    let new_state = Self::simulate_action(state, adv_action);
                    let child_hash = Self::hash_state(&new_state);
                    let mut child = GameNode::new_child(
                        adv_action.clone(),
                        child_hash,
                        current_depth + 1,
                        Player::Agent,
                    );

                    // Agent gets to respond again with the original action set
                    let agent_moves = Self::generate_agent_moves(&new_state);
                    self.build_recursive(
                        &mut child,
                        &new_state,
                        &agent_moves,
                        threats,
                        max_depth,
                        current_depth + 1,
                    );

                    node.children.push(child);
                }

                // Also add a "no MEV" child where the adversary does nothing
                let pass_hash = format!("{}_pass", node.state_hash);
                let mut pass_child = GameNode {
                    action: None,
                    state_hash: pass_hash,
                    children: Vec::new(),
                    score: 0.0,
                    depth: current_depth + 1,
                    is_terminal: false,
                    player: Player::Agent,
                };
                let agent_moves = Self::generate_agent_moves(state);
                self.build_recursive(
                    &mut pass_child,
                    state,
                    &agent_moves,
                    threats,
                    max_depth,
                    current_depth + 1,
                );
                node.children.push(pass_child);
            }
        }
    }

    /// Expand a single node by generating its children.
    pub fn expand_node(&self, node: &mut GameNode, state: &OnChainState) {
        if node.is_terminal || !node.children.is_empty() {
            return;
        }

        match node.player {
            Player::Agent => {
                let moves = Self::generate_agent_moves(state);
                for action in moves {
                    let new_state = Self::simulate_action(state, &action);
                    let hash = Self::hash_state(&new_state);
                    let child =
                        GameNode::new_child(action, hash, node.depth + 1, Player::Adversary);
                    node.children.push(child);
                }
            }
            Player::Adversary => {
                // Generate adversary responses based on the node's action
                if let Some(ref action) = node.action {
                    let threats = Self::generate_adversary_moves(state, action);
                    for threat in threats {
                        let adv_action = Self::threat_to_action(&threat);
                        let new_state = Self::simulate_action(state, &adv_action);
                        let hash = Self::hash_state(&new_state);
                        let child = GameNode::new_child(
                            adv_action,
                            hash,
                            node.depth + 1,
                            Player::Agent,
                        );
                        node.children.push(child);
                    }
                }
            }
        }

        if node.children.is_empty() {
            node.is_terminal = true;
            node.score = self.evaluator.evaluate_static(state);
        }
    }

    /// Generate candidate moves for the Agent based on the current state.
    ///
    /// Inspects token balances and available pools to create feasible actions.
    pub fn generate_agent_moves(state: &OnChainState) -> Vec<ExecutionAction> {
        let mut moves = Vec::new();

        for pool in &state.pool_states {
            // Try swapping token A -> B
            if let Some(&balance_a) = state.token_balances.get(&pool.token_a_mint) {
                if balance_a > 0 && pool.reserve_a > 0 && pool.reserve_b > 0 {
                    // Swap a fraction: 10%, 25%, 50%
                    for &frac in &[10u64, 25, 50] {
                        let amount = balance_a.saturating_mul(frac) / 100;
                        if amount > 0 {
                            moves.push(ExecutionAction::new(
                                ActionKind::Swap,
                                &pool.token_a_mint,
                                amount,
                                &pool.token_b_mint,
                                50, // 0.5% default slippage tolerance
                                &pool.address,
                                5000,
                            ));
                        }
                    }
                }
            }

            // Try swapping token B -> A
            if let Some(&balance_b) = state.token_balances.get(&pool.token_b_mint) {
                if balance_b > 0 && pool.reserve_a > 0 && pool.reserve_b > 0 {
                    for &frac in &[10u64, 25, 50] {
                        let amount = balance_b.saturating_mul(frac) / 100;
                        if amount > 0 {
                            moves.push(ExecutionAction::new(
                                ActionKind::Swap,
                                &pool.token_b_mint,
                                amount,
                                &pool.token_a_mint,
                                50,
                                &pool.address,
                                5000,
                            ));
                        }
                    }
                }
            }

            // Try adding liquidity
            if let Some(&bal) = state.token_balances.get(&pool.token_a_mint) {
                if bal > 10_000 {
                    moves.push(ExecutionAction::new(
                        ActionKind::AddLiquidity,
                        &pool.token_a_mint,
                        bal / 4,
                        &pool.address,
                        100,
                        &pool.address,
                        5000,
                    ));
                }
            }
        }

        moves
    }

    /// Generate adversary (MEV) moves in response to an agent action.
    pub fn generate_adversary_moves(
        state: &OnChainState,
        agent_action: &ExecutionAction,
    ) -> Vec<MevThreat> {
        let mut threats = Vec::new();

        // Only pool-interactive actions attract MEV
        match agent_action.kind {
            ActionKind::Swap | ActionKind::AddLiquidity | ActionKind::RemoveLiquidity => {}
            _ => return threats,
        }

        let pool = match state
            .pool_states
            .iter()
            .find(|p| p.address == agent_action.pool_address)
        {
            Some(p) => p,
            None => return threats,
        };

        // Sandwich attack: profitable if agent's trade is large relative to pool
        let impact = math::calculate_price_impact(
            agent_action.amount,
            pool.reserve_a,
            pool.reserve_b,
        );
        if impact > 0.001 {
            // > 0.1% price impact -> sandwich opportunity
            let sandwich_cost = Self::estimate_sandwich_cost(agent_action.amount, pool);
            threats.push(MevThreat::new(
                MevKind::Sandwich,
                math::clamp_f64(impact * 10.0, 0.0, 0.95),
                sandwich_cost,
                "mev_bot_sandwich",
                &pool.address,
            ));
        }

        // Frontrun: if there are pending txs with lower fees
        let can_frontrun = state
            .pending_transactions
            .iter()
            .any(|tx| tx.to == pool.address && tx.fee < agent_action.priority_fee * 2);
        if can_frontrun && agent_action.amount > pool.reserve_a / 100 {
            let frontrun_cost = agent_action.amount / 200; // ~0.5% cost
            threats.push(MevThreat::new(
                MevKind::Frontrun,
                0.3,
                frontrun_cost,
                "mev_bot_frontrun",
                &pool.address,
            ));
        }

        // JIT liquidity: large swaps attract JIT providers
        if agent_action.kind == ActionKind::Swap && agent_action.amount > pool.reserve_a / 20
        {
            threats.push(MevThreat::new(
                MevKind::JitLiquidity,
                0.4,
                agent_action.amount / 500,
                "jit_provider",
                &pool.address,
            ));
        }

        // Backrun: someone profiting from the price impact we create
        if impact > 0.005 {
            threats.push(MevThreat::new(
                MevKind::Backrun,
                math::clamp_f64(impact * 5.0, 0.0, 0.8),
                agent_action.amount / 300,
                "mev_bot_backrun",
                &pool.address,
            ));
        }

        threats
    }

    /// Simulate applying an action to a state, returning the new state.
    pub fn simulate_action(
        state: &OnChainState,
        action: &ExecutionAction,
    ) -> OnChainState {
        let mut new_state = state.clone();
        new_state.slot += 1;

        match action.kind {
            ActionKind::Swap => {
                Self::simulate_swap(&mut new_state, action);
            }
            ActionKind::Transfer => {
                // Deduct from source balance
                if let Some(bal) = new_state.token_balances.get_mut(&action.token_mint) {
                    *bal = bal.saturating_sub(action.amount);
                }
                // Add to destination (tracked if we know it)
                let dest_bal = new_state
                    .token_balances
                    .entry(action.destination.clone())
                    .or_insert(0);
                *dest_bal = dest_bal.saturating_add(action.amount);
            }
            ActionKind::Stake | ActionKind::Unstake => {
                // Staking locks tokens; unstaking releases them
                if let Some(bal) = new_state.token_balances.get_mut(&action.token_mint) {
                    if action.kind == ActionKind::Stake {
                        *bal = bal.saturating_sub(action.amount);
                    } else {
                        *bal = bal.saturating_add(action.amount);
                    }
                }
            }
            ActionKind::Liquidate => {
                // Gain the liquidation bonus
                let bonus = action.amount / 20; // 5% liquidation bonus
                let bal = new_state
                    .token_balances
                    .entry(action.token_mint.clone())
                    .or_insert(0);
                *bal = bal.saturating_add(bonus);
            }
            ActionKind::AddLiquidity => {
                Self::simulate_add_liquidity(&mut new_state, action);
            }
            ActionKind::RemoveLiquidity => {
                Self::simulate_remove_liquidity(&mut new_state, action);
            }
        }

        new_state
    }

    /// Simulate a swap on the state.
    fn simulate_swap(state: &mut OnChainState, action: &ExecutionAction) {
        let pool_idx = state
            .pool_states
            .iter()
            .position(|p| p.address == action.pool_address);

        if let Some(idx) = pool_idx {
            let pool = &state.pool_states[idx];
            let is_a_to_b = action.token_mint == pool.token_a_mint;

            let (reserve_in, reserve_out) = if is_a_to_b {
                (pool.reserve_a, pool.reserve_b)
            } else {
                (pool.reserve_b, pool.reserve_a)
            };

            let output = math::constant_product_swap(
                action.amount,
                reserve_in,
                reserve_out,
                pool.fee_rate_bps,
            );

            // Update pool reserves
            let pool = &mut state.pool_states[idx];
            if is_a_to_b {
                pool.reserve_a = pool.reserve_a.saturating_add(action.amount);
                pool.reserve_b = pool.reserve_b.saturating_sub(output);
            } else {
                pool.reserve_b = pool.reserve_b.saturating_add(action.amount);
                pool.reserve_a = pool.reserve_a.saturating_sub(output);
            }

            // Update token balances
            if let Some(bal) = state.token_balances.get_mut(&action.token_mint) {
                *bal = bal.saturating_sub(action.amount);
            }

            let out_mint = if is_a_to_b {
                &state.pool_states[idx].token_b_mint
            } else {
                &state.pool_states[idx].token_a_mint
            }
            .clone();

            let out_bal = state.token_balances.entry(out_mint).or_insert(0);
            *out_bal = out_bal.saturating_add(output);
        }
    }

    /// Simulate adding liquidity.
    fn simulate_add_liquidity(state: &mut OnChainState, action: &ExecutionAction) {
        if let Some(pool) = state
            .pool_states
            .iter_mut()
            .find(|p| p.address == action.pool_address)
        {
            let is_token_a = action.token_mint == pool.token_a_mint;
            if is_token_a {
                pool.reserve_a = pool.reserve_a.saturating_add(action.amount);
                // Proportional token B deposit
                let proportional_b = if pool.reserve_a > 0 {
                    (action.amount as u128 * pool.reserve_b as u128
                        / pool.reserve_a as u128) as u64
                } else {
                    action.amount
                };
                pool.reserve_b = pool.reserve_b.saturating_add(proportional_b);
            } else {
                pool.reserve_b = pool.reserve_b.saturating_add(action.amount);
            }
            pool.liquidity = (pool.reserve_a as u128).saturating_mul(pool.reserve_b as u128);
        }

        if let Some(bal) = state.token_balances.get_mut(&action.token_mint) {
            *bal = bal.saturating_sub(action.amount);
        }
    }

    /// Simulate removing liquidity.
    fn simulate_remove_liquidity(state: &mut OnChainState, action: &ExecutionAction) {
        if let Some(pool) = state
            .pool_states
            .iter_mut()
            .find(|p| p.address == action.pool_address)
        {
            let share = if pool.reserve_a > 0 {
                math::clamp_f64(action.amount as f64 / pool.reserve_a as f64, 0.0, 1.0)
            } else {
                0.0
            };
            let removed_a = (pool.reserve_a as f64 * share) as u64;
            let removed_b = (pool.reserve_b as f64 * share) as u64;
            pool.reserve_a = pool.reserve_a.saturating_sub(removed_a);
            pool.reserve_b = pool.reserve_b.saturating_sub(removed_b);
            pool.liquidity = (pool.reserve_a as u128).saturating_mul(pool.reserve_b as u128);

            let mint_a = pool.token_a_mint.clone();
            let mint_b = pool.token_b_mint.clone();

            let bal_a = state.token_balances.entry(mint_a).or_insert(0);
            *bal_a = bal_a.saturating_add(removed_a);

            let bal_b = state.token_balances.entry(mint_b).or_insert(0);
            *bal_b = bal_b.saturating_add(removed_b);
        }
    }

    /// Simulate an MEV response modifying the state.
    pub fn simulate_mev_response(
        state: &OnChainState,
        threat: &MevThreat,
    ) -> OnChainState {
        let action = Self::threat_to_action(threat);
        Self::simulate_action(state, &action)
    }

    /// Convert an MevThreat into an ExecutionAction for simulation.
    fn threat_to_action(threat: &MevThreat) -> ExecutionAction {
        let kind = match threat.kind {
            MevKind::Sandwich | MevKind::Frontrun | MevKind::Backrun => ActionKind::Swap,
            MevKind::JitLiquidity => ActionKind::AddLiquidity,
        };
