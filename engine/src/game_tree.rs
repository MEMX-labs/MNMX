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
