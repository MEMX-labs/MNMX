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
        }

        // Finalize the best route's minimax score
        if let Some(ref mut route) = best_route {
            route.minimax_score = best_score;
        }

        stats.finalize();
        (best_route, stats.to_search_stats())
    }

    /// Recursive minimax with alpha-beta pruning.
    /// When `is_maximizing` is true, the player is choosing (picks best route).
    /// When false, the adversary is choosing (picks worst-case scenario).
    fn minimax(
        &mut self,
        registry: &BridgeRegistry,
        node: &SearchNode,
        target_chain: Chain,
        target_token: &Token,
        max_depth: u32,
        is_maximizing: bool,
        pruning: &mut PruningState,
        stats: &mut SearchStatsCollector,
    ) -> (f64, Option<Route>) {
        stats.record_node(node.depth);

        // Check transposition table
        let state_hash = compute_state_hash(
            node.current_chain,
            node.remaining_amount,
            node.depth,
            &node.bridges_used,
        );
        if let Some(cached_score) =
            self.transposition_table
                .lookup(state_hash, max_depth - node.depth, pruning.alpha, pruning.beta)
        {
            let route = self.build_route_from_node(node);
            return (cached_score, Some(route));
        }

        // Terminal condition: reached target chain
        if node.current_chain == target_chain && !node.hops_taken.is_empty() {
            let route = self.build_route_from_node(node);
            let score = self.evaluate_route(&route);
            self.store_in_tt(state_hash, max_depth - node.depth, score, TranspositionFlag::Exact);
            return (score, Some(route));
        }

        // Depth limit reached
        if node.depth >= max_depth {
            if node.hops_taken.is_empty() {
                return (f64::NEG_INFINITY, None);
            }
            let route = self.build_route_from_node(node);
            let score = self.evaluate_route(&route);
            // Penalize routes that didn't reach destination
            let penalty = if node.current_chain != target_chain {
                0.3
            } else {
                1.0
            };
            let penalized = score * penalty;
            return (penalized, Some(route));
        }

        // Generate possible next moves
        let moves = self.generate_moves(registry, node, target_chain, target_token);
        if moves.is_empty() {
            if node.hops_taken.is_empty() {
                return (f64::NEG_INFINITY, None);
            }
            let route = self.build_route_from_node(node);
            let score = self.evaluate_route(&route);
            return (score, Some(route));
        }

        // Order moves for better pruning
        let ordering = pruning.get_move_ordering(&moves, node.depth as usize);

        if is_maximizing {
            self.maximize(
                registry,
                node,
                target_chain,
                target_token,
                max_depth,
                pruning,
                stats,
                &moves,
                &ordering,
                state_hash,
            )
        } else {
            self.minimize(
                registry,
                node,
                target_chain,
                target_token,
                max_depth,
                pruning,
                stats,
                &moves,
                &ordering,
                state_hash,
            )
        }
    }

    /// Maximizing node: player picks the move with the highest score.
    fn maximize(
        &mut self,
        registry: &BridgeRegistry,
        node: &SearchNode,
        target_chain: Chain,
        target_token: &Token,
        max_depth: u32,
        pruning: &mut PruningState,
        stats: &mut SearchStatsCollector,
        moves: &[RouteHop],
        ordering: &[usize],
        state_hash: u64,
    ) -> (f64, Option<Route>) {
        let mut best_score = f64::NEG_INFINITY;
        let mut best_route: Option<Route> = None;

        for &idx in ordering {
            let hop = &moves[idx];

            let child = self.apply_move(node, hop);
            // After player moves, adversary responds (minimizing)
            let (score, route) = self.minimax(
                registry,
                &child,
                target_chain,
                target_token,
                max_depth,
                false,
                pruning,
                stats,
            );

            if score > best_score {
                best_score = score;
                best_route = route;
            }

            pruning.update_bounds(score, true);
            if pruning.should_prune(best_score, true) {
                stats.record_pruned();
                pruning.record_killer_move(
                    node.depth as usize,
                    MoveKey::from_hop(hop),
                );
                pruning.record_history(MoveKey::from_hop(hop), node.depth);
                break;
            }
        }

        let flag = if best_score >= pruning.beta {
            TranspositionFlag::LowerBound
        } else if best_score <= pruning.alpha {
            TranspositionFlag::UpperBound
        } else {
            TranspositionFlag::Exact
        };
        self.store_in_tt(state_hash, max_depth - node.depth, best_score, flag);

        (best_score, best_route)
    }

    /// Minimizing node: adversary picks the worst-case scenario for the player.
    fn minimize(
        &mut self,
        registry: &BridgeRegistry,
        node: &SearchNode,
        target_chain: Chain,
        target_token: &Token,
        max_depth: u32,
        pruning: &mut PruningState,
        stats: &mut SearchStatsCollector,
        moves: &[RouteHop],
        ordering: &[usize],
        state_hash: u64,
    ) -> (f64, Option<Route>) {
        let mut best_score = f64::INFINITY;
        let mut best_route: Option<Route> = None;

        // The adversary applies worst-case perturbations to each move
        for &idx in ordering {
            let hop = &moves[idx];
