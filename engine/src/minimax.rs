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
