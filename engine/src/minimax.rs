use crate::bridge::BridgeRegistry;
use crate::math;
use crate::pruning::{
    compute_state_hash, MoveKey, PruningState, TranspositionEntry, TranspositionFlag,
    TranspositionTable,
};
use crate::risk::RiskAssessor;
use crate::scoring::RouteScorer;
use crate::stats::SearchStatsCollector;
use crate::types::{Chain, Route, RouteHop, RouterConfig, SearchStats, Token};

/// The minimax searcher that finds optimal routes through the game tree.
pub struct MinimaxSearcher {
    scorer: RouteScorer,
    risk_assessor: RiskAssessor,
    transposition_table: TranspositionTable,
    config: RouterConfig,
}

/// Internal node in the search tree.
#[derive(Debug, Clone)]
struct SearchNode {
    current_chain: Chain,
    current_token: Token,
    remaining_amount: f64,
    hops_taken: Vec<RouteHop>,
    bridges_used: Vec<String>,
    total_fees: f64,
    total_time: u64,
    depth: u32,
}

impl MinimaxSearcher {
    pub fn new(config: RouterConfig) -> Self {
        let scorer = RouteScorer::new(config.weights.clone());
        let risk_assessor = RiskAssessor::new(config.adversarial_model.clone());
        Self {
            scorer,
            risk_assessor,
            transposition_table: TranspositionTable::new(10_000),
            config,
        }
    }

    /// Main entry point: run iterative-deepening minimax search.
    /// Returns the best route found and search statistics.
    pub fn search(
        &mut self,
        registry: &BridgeRegistry,
        from_chain: Chain,
        from_token: &Token,
        to_chain: Chain,
        to_token: &Token,
        amount: f64,
    ) -> (Option<Route>, SearchStats) {
        let mut stats = SearchStatsCollector::new();
        let mut best_route: Option<Route> = None;
        let mut best_score = f64::NEG_INFINITY;

        // Iterative deepening from depth 1 to max_hops
        for max_depth in 1..=self.config.max_hops as u32 {
            self.transposition_table.clear();

            let root = SearchNode {
                current_chain: from_chain,
                current_token: from_token.clone(),
                remaining_amount: amount,
                hops_taken: Vec::new(),
                bridges_used: Vec::new(),
                total_fees: 0.0,
                total_time: 0,
                depth: 0,
            };

            let mut pruning = PruningState::new(max_depth as usize + 1);

            let (score, route) = self.minimax(
                registry,
                &root,
                to_chain,
                to_token,
                max_depth,
                true, // maximizing (player wants best outcome)
                &mut pruning,
                &mut stats,
            );

            if let Some(ref r) = route {
                if score > best_score {
                    best_score = score;
                    best_route = Some(r.clone());
                }
            }

            stats.record_depth(max_depth);
