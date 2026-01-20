use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Which side of the game tree we are evaluating.
/// Agent = the autonomous on-chain agent (maximizing player).
/// Adversary = MEV bots / extractors (minimizing player).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Player {
    Agent,
    Adversary,
}

impl Player {
    pub fn opponent(&self) -> Player {
        match self {
            Player::Agent => Player::Adversary,
            Player::Adversary => Player::Agent,
        }
    }
}

/// The kind of on-chain action that can be taken.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ActionKind {
    Swap,
    Transfer,
    Stake,
    Unstake,
    Liquidate,
    AddLiquidity,
    RemoveLiquidity,
}

impl ActionKind {
    /// Returns a deterministic ordering index used by move ordering heuristics.
    pub fn priority_index(&self) -> u32 {
        match self {
            ActionKind::Liquidate => 0,
            ActionKind::Swap => 1,
            ActionKind::RemoveLiquidity => 2,
            ActionKind::AddLiquidity => 3,
            ActionKind::Transfer => 4,
            ActionKind::Unstake => 5,
            ActionKind::Stake => 6,
        }
    }

    /// Returns a human-readable label for logging.
    pub fn label(&self) -> &'static str {
        match self {
            ActionKind::Swap => "swap",
            ActionKind::Transfer => "transfer",
            ActionKind::Stake => "stake",
            ActionKind::Unstake => "unstake",
            ActionKind::Liquidate => "liquidate",
            ActionKind::AddLiquidity => "add_liq",
            ActionKind::RemoveLiquidity => "rem_liq",
        }
    }
}

/// A concrete on-chain action the agent can execute.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExecutionAction {
    pub kind: ActionKind,
    pub token_mint: String,
    pub amount: u64,
    pub destination: String,
    pub slippage_bps: u16,
    pub pool_address: String,
    pub priority_fee: u64,
}

impl ExecutionAction {
    pub fn new(
        kind: ActionKind,
        token_mint: &str,
        amount: u64,
        destination: &str,
        slippage_bps: u16,
        pool_address: &str,
        priority_fee: u64,
    ) -> Self {
        Self {
            kind,
            token_mint: token_mint.to_string(),
            amount,
            destination: destination.to_string(),
            slippage_bps,
            pool_address: pool_address.to_string(),
            priority_fee,
        }
    }

    /// Produce a compact key for hashing / history tables.
    pub fn action_key(&self) -> String {
        format!(
            "{}:{}:{}:{}",
            self.kind.label(),
            self.token_mint,
            self.amount,
            self.pool_address
        )
    }

    /// Estimated total cost in lamports (priority fee + base fee estimate).
    pub fn estimated_total_fee(&self) -> u64 {
        self.priority_fee.saturating_add(5000)
    }
}

/// A single node in the game tree.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameNode {
    pub action: Option<ExecutionAction>,
    pub state_hash: String,
    pub children: Vec<GameNode>,
    pub score: f64,
    pub depth: u32,
    pub is_terminal: bool,
    pub player: Player,
}

impl GameNode {
    pub fn new_root(state_hash: String) -> Self {
        Self {
            action: None,
            state_hash,
            children: Vec::new(),
            score: 0.0,
            depth: 0,
            is_terminal: false,
            player: Player::Agent,
        }
    }

    pub fn new_child(
        action: ExecutionAction,
        state_hash: String,
        depth: u32,
        player: Player,
    ) -> Self {
        Self {
            action: Some(action),
            state_hash,
            children: Vec::new(),
            score: 0.0,
            depth,
            is_terminal: false,
            player,
        }
    }

    /// Total number of nodes in this subtree, including self.
    pub fn subtree_size(&self) -> usize {
        1 + self.children.iter().map(|c| c.subtree_size()).sum::<usize>()
    }
