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
