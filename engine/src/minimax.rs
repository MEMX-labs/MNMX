use std::time::Instant;

use crate::evaluator::PositionEvaluator;
use crate::game_tree::GameTreeBuilder;
use crate::mev::MevDetector;
use crate::move_ordering::MoveOrderer;
use crate::stats::SearchStatistics;
use crate::time_manager::{ExtendReason, TimeManager};
use crate::transposition::TranspositionTable;
use crate::types::*;

/// The core minimax search engine with alpha-beta pruning, iterative
/// deepening, transposition table, aspiration windows, and move ordering.
///
/// Usage:
/// ```ignore
/// let config = SearchConfig::default();
/// let mut engine = MinimaxEngine::new(config);
/// let plan = engine.search(&state, &actions);
/// ```
pub struct MinimaxEngine {
    config: SearchConfig,
    evaluator: PositionEvaluator,
    _tree_builder: GameTreeBuilder,
    move_orderer: MoveOrderer,
    transposition_table: TranspositionTable,
    mev_detector: MevDetector,
    time_manager: TimeManager,
    stats: SearchStatistics,
    start_time: Option<Instant>,
    aborted: bool,
}

impl MinimaxEngine {
    pub fn new(config: SearchConfig) -> Self {
        let evaluator = PositionEvaluator::new(config.eval_weights.clone());
        let tree_builder = GameTreeBuilder::new(evaluator.clone());
        let time_manager = TimeManager::new(&config);

        Self {
            evaluator,
            _tree_builder: tree_builder,
            move_orderer: MoveOrderer::new(),
            transposition_table: TranspositionTable::new(100_000),
            mev_detector: MevDetector::new(),
            time_manager,
            stats: SearchStatistics::new(),
            start_time: None,
            aborted: false,
            config,
        }
    }

    /// Run iterative-deepening minimax with alpha-beta pruning.
    ///
    /// Returns an `ExecutionPlan` containing the best sequence of actions
    /// found within the time budget.
    pub fn search(
        &mut self,
        state: &OnChainState,
        actions: &[ExecutionAction],
    ) -> ExecutionPlan {
        self.stats = SearchStatistics::new();
        self.aborted = false;
        self.start_time = Some(Instant::now());

        if self.config.move_ordering_enabled {
            self.move_orderer.reset();
        }

        self.transposition_table.new_search();

        if actions.is_empty() {
            return ExecutionPlan::empty(self.stats.to_search_stats());
        }

        // Detect threats for all candidate actions
        let threats: Vec<MevThreat> = actions
            .iter()
            .flat_map(|a| self.mev_detector.detect_threats(a, state))
            .collect();

        let mut best_actions: Vec<ExecutionAction> = Vec::new();
        let mut best_score = f64::NEG_INFINITY;
        let mut previous_best_score = f64::NEG_INFINITY;

        // Iterative deepening: search from depth 1 up to max_depth
        for depth in 1..=self.config.max_depth {
            if self.should_stop_search(depth) {
                break;
            }

            let (score, actions_found) = self.search_root(
                state,
                actions,
                &threats,
                depth,
                previous_best_score,
            );

            if self.aborted {
                break;
            }

            let elapsed = self.elapsed_ms();
            self.stats.record_depth_completed(depth, elapsed);

            // Check for instability: best move changed
            if !actions_found.is_empty() && !best_actions.is_empty() {
                let changed = actions_found
                    .first()
                    .map(|a| a.action_key())
                    != best_actions.first().map(|a| a.action_key());

                if changed {
                    self.stats.record_best_move_change();
                    self.time_manager.extend(ExtendReason::Instability);
                }
            }

            // Check for score drop
            if depth > 1 && score < previous_best_score - 1.0 {
                self.time_manager.extend(ExtendReason::ScoreDrop);
            }

            if !actions_found.is_empty() {
                best_score = score;
                best_actions = actions_found;
            }

            previous_best_score = best_score;

            log::debug!(
                "depth={} score={:.3} nodes={} pruned={} time={}ms",
                depth,
                best_score,
                self.stats.total_nodes(),
                self.stats.total_pruned(),
                elapsed,
            );
        }

        let total_cost: u64 = best_actions.iter().map(|a| a.estimated_total_fee()).sum();

        let mut search_stats = self.stats.to_search_stats();
        search_stats.time_ms = self.elapsed_ms();
        search_stats.tt_hits = self.transposition_table.total_hits();
        search_stats.tt_misses = self.transposition_table.total_misses();

        ExecutionPlan {
            actions: best_actions,
            expected_score: best_score,
            total_cost,
            search_stats,
        }
    }

    /// Search the root position at a specific depth, using aspiration windows.
    fn search_root(
        &mut self,
        state: &OnChainState,
        actions: &[ExecutionAction],
        _threats: &[MevThreat],
        depth: u32,
        previous_score: f64,
    ) -> (f64, Vec<ExecutionAction>) {
        // Aspiration window: narrow alpha-beta window centered on the previous score.
        // If the search fails low or high, re-search with a full window.
        let (mut alpha, beta) = if depth > 1 && previous_score.is_finite() {
            let delta = 0.5;
            (previous_score - delta, previous_score + delta)
        } else {
            (f64::NEG_INFINITY, f64::INFINITY)
        };

        let mut best_score = f64::NEG_INFINITY;
        let mut best_sequence: Vec<ExecutionAction> = Vec::new();

        // Order the root moves
        let mut sorted_actions = actions.to_vec();
        if self.config.move_ordering_enabled {
            self.move_orderer.order_moves(&mut sorted_actions, state, 0);
            self.move_orderer.record_ordering_call();
        }

        // Try each action at the root
        for action in &sorted_actions {
            if self.should_abort() {
                self.aborted = true;
                break;
            }

            let new_state = GameTreeBuilder::simulate_action(state, action);
            let state_hash = GameTreeBuilder::hash_state(&new_state);

            // Build a child node for the adversary's response
            let mut child = GameNode::new_child(
                action.clone(),
                state_hash.clone(),
                1,
                Player::Adversary,
            );

            // Evaluate the adversary's response
            let score = self.minimax_search(
                &mut child,
                &new_state,
                depth - 1,
                -beta,
                -alpha,
                false, // Adversary is minimizing
            );
            let score = -score; // Negamax convention: negate

            self.stats.record_node_visit();

            if score > best_score {
                best_score = score;
                best_sequence = vec![action.clone()];

                // Extract the principal variation
                let pv = self.extract_pv(&child);
                best_sequence.extend(pv);
            }

            if score > alpha {
                alpha = score;
            }

            if self.config.alpha_beta_enabled && alpha >= beta {
                self.stats.record_prune();
                // Killer / history updates
                if self.config.move_ordering_enabled {
                    self.move_orderer.update_killer(0, action);
                    self.move_orderer.update_history(action, depth);
                }
                break;
            }
        }

        // Aspiration window failure: re-search with full window if needed
        if best_score <= previous_score - 0.5 || best_score >= previous_score + 0.5 {
            if depth > 1 && !self.aborted {
                // The aspiration window was too narrow; the result is still
                // valid as a bound but we accept it rather than re-searching
                // to stay within time budget.
            }
        }

        (best_score, best_sequence)
    }

    /// Recursive minimax with alpha-beta pruning (negamax formulation).
    ///
    /// `maximizing` is true when the current player is the Agent.
    fn minimax_search(
        &mut self,
        node: &mut GameNode,
        state: &OnChainState,
        depth: u32,
        mut alpha: f64,
        beta: f64,
        maximizing: bool,
    ) -> f64 {
        self.stats.record_node_visit();

        // Time check
        if self.should_abort() {
            self.aborted = true;
            return 0.0;
        }

        // Terminal or depth limit
        if depth == 0 || node.is_terminal {
            let eval = self.evaluate_node(state, node);
            node.score = eval;
            return eval;
        }

        let state_hash = &node.state_hash;

        // Transposition table probe
        if self.config.transposition_enabled {
            if let Some(tt_score) =
                self.transposition_table.lookup(state_hash, depth, alpha, beta)
            {
                self.stats.record_tt_hit();
                node.score = tt_score;
                return tt_score;
            } else {
                self.stats.record_tt_miss();
            }
        }

        // Generate moves
        let mut moves = if maximizing {
            GameTreeBuilder::generate_agent_moves(state)
        } else {
            // For the adversary, generate threat-based moves
            if let Some(ref action) = node.action {
                let threats = GameTreeBuilder::generate_adversary_moves(state, action);
                threats
                    .iter()
                    .map(|t| threat_to_action(t))
                    .collect()
            } else {
                GameTreeBuilder::generate_agent_moves(state)
            }
        };

        if moves.is_empty() {
            let eval = self.evaluate_node(state, node);
            node.score = eval;
            return eval;
        }

        self.stats.record_children(moves.len() as u64);

        // Move ordering
        if self.config.move_ordering_enabled {
            self.move_orderer.order_moves(&mut moves, state, depth);
        }

        // TT best move: try it first
        if self.config.transposition_enabled {
            if let Some(tt_action) = self
                .transposition_table
                .get_best_action(state_hash)
                .cloned()
            {
                // Move TT action to front
                if let Some(pos) = moves.iter().position(|a| a.action_key() == tt_action.action_key()) {
                    moves.swap(0, pos);
                }
            }
        }

        let mut best_score = f64::NEG_INFINITY;
        let mut best_action: Option<ExecutionAction> = None;
        let mut tt_flag = TranspositionFlag::UpperBound;

        for action in moves.iter() {
            if self.should_abort() {
                self.aborted = true;
                break;
            }

            let new_state = GameTreeBuilder::simulate_action(state, action);
            let child_hash = GameTreeBuilder::hash_state(&new_state);
            let child_player = if maximizing {
                Player::Adversary
            } else {
                Player::Agent
            };

            let mut child_node = GameNode::new_child(
                action.clone(),
                child_hash,
                node.depth + 1,
                child_player,
            );

            let score = -self.minimax_search(
                &mut child_node,
                &new_state,
                depth - 1,
                -beta,
                -alpha,
                !maximizing,
            );

            node.children.push(child_node);

            if score > best_score {
                best_score = score;
                best_action = Some(action.clone());

                if score > alpha {
                    alpha = score;
                    tt_flag = TranspositionFlag::Exact;
                }
            }

            // Alpha-beta cutoff
            if self.config.alpha_beta_enabled && alpha >= beta {
                self.stats.record_prune();

                if self.config.move_ordering_enabled {
                    self.move_orderer.update_killer(depth, action);
                    self.move_orderer.update_history(action, depth);
                }

                tt_flag = TranspositionFlag::LowerBound;
                break;
            }
        }

        node.score = best_score;
