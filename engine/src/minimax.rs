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
            let adversarial_hop = self.apply_adversarial_model(hop);
            let child = self.apply_move(node, &adversarial_hop);

            // After adversary, player moves again (maximizing)
            let (score, route) = self.minimax(
                registry,
                &child,
                target_chain,
                target_token,
                max_depth,
                true,
                pruning,
                stats,
            );

            if score < best_score {
                best_score = score;
                best_route = route;
            }

            pruning.update_bounds(score, false);
            if pruning.should_prune(best_score, false) {
                stats.record_pruned();
                pruning.record_killer_move(
                    node.depth as usize,
                    MoveKey::from_hop(&adversarial_hop),
                );
                break;
            }
        }

        let flag = if best_score <= pruning.alpha {
            TranspositionFlag::UpperBound
        } else if best_score >= pruning.beta {
            TranspositionFlag::LowerBound
        } else {
            TranspositionFlag::Exact
        };
        self.store_in_tt(state_hash, max_depth - node.depth, best_score, flag);

        (best_score, best_route)
    }

    /// Evaluate a complete or partial route at a leaf node.
    pub fn evaluate_route(&self, route: &Route) -> f64 {
        let base_score = self.scorer.score_route(route);

        // Apply risk assessment as a modifier
        let risk_assessment = self.risk_assessor.assess_route_risk(route);
        let risk_factor = match risk_assessment.risk_level {
            crate::types::RiskLevel::Low => 1.0,
            crate::types::RiskLevel::Medium => 0.85,
            crate::types::RiskLevel::High => 0.6,
            crate::types::RiskLevel::Critical => 0.3,
        };

        // Value retention bonus: routes that preserve more value score higher
        let input = route.hops.first().map(|h| h.input_amount).unwrap_or(1.0);
        let retention = if input > 0.0 {
            route.expected_output / input
        } else {
            0.0
        };
        let retention_score = math::clamp_f64(retention, 0.0, 1.0);

        // Combine base score, risk factor, and retention
        let composite = base_score * 0.5 + retention_score * 0.3 + risk_factor * 0.2;
        math::clamp_f64(composite, 0.0, 1.0)
    }

    /// Generate possible next hops from the current search node.
    fn generate_moves(
        &self,
        registry: &BridgeRegistry,
        node: &SearchNode,
        target_chain: Chain,
        target_token: &Token,
    ) -> Vec<RouteHop> {
        let mut moves = Vec::new();

        // Try all chains reachable from the current chain
        for &dest_chain in Chain::all() {
            if dest_chain == node.current_chain {
                continue;
            }
            // Avoid revisiting chains (no loops)
            if node
                .hops_taken
                .iter()
                .any(|h| h.from_chain == dest_chain)
            {
                continue;
            }

            let bridges = registry.get_bridges_for_pair(node.current_chain, dest_chain);
            for bridge in bridges {
                let health = bridge.get_health();
                if !health.online {
                    continue;
                }

                // Determine output token
                let to_token = if dest_chain == target_chain {
                    target_token.clone()
                } else {
                    // Intermediate hop uses USDC or same token
                    if node.current_token.is_stablecoin() {
                        Token::new(
                            &node.current_token.symbol,
                            dest_chain,
                            node.current_token.decimals,
                            &format!("0x{}_{}", node.current_token.symbol, dest_chain.chain_id()),
                        )
                    } else {
                        Token::new(
                            "USDC",
                            dest_chain,
                            6,
                            &format!("0xUSDC_{}", dest_chain.chain_id()),
                        )
                    }
                };

                if let Some(quote) =
                    bridge.get_quote(&node.current_token, &to_token, node.remaining_amount)
                {
                    moves.push(RouteHop {
                        from_chain: node.current_chain,
                        to_chain: dest_chain,
                        from_token: node.current_token.clone(),
                        to_token: to_token,
                        bridge: bridge.name().to_string(),
                        input_amount: node.remaining_amount,
                        output_amount: quote.output_amount,
                        fee: quote.fee,
                        estimated_time: quote.estimated_time,
                    });
                }
            }
        }

        moves
    }

    /// Apply the adversarial model to a hop, computing worst-case values.
    pub fn apply_adversarial_model(&self, hop: &RouteHop) -> RouteHop {
        let model = &self.config.adversarial_model;

        let observed_slippage = hop.slippage();
        let worst_slippage = math::clamp_f64(
            observed_slippage * model.slippage_multiplier,
            0.0,
            0.5,
        );

        let worst_fee = hop.fee * model.gas_multiplier;
        let worst_output = hop.input_amount * (1.0 - worst_slippage) - worst_fee;
        let worst_output = if worst_output < 0.0 { 0.0 } else { worst_output };

        let mev_loss = hop.input_amount * model.mev_extraction;
        let final_output = worst_output - mev_loss;
        let final_output = if final_output < 0.0 { 0.0 } else { final_output };

        let worst_time = (hop.estimated_time as f64 * model.bridge_delay_multiplier) as u64;

        RouteHop {
            from_chain: hop.from_chain,
            to_chain: hop.to_chain,
            from_token: hop.from_token.clone(),
            to_token: hop.to_token.clone(),
            bridge: hop.bridge.clone(),
            input_amount: hop.input_amount,
            output_amount: final_output,
            fee: worst_fee,
            estimated_time: worst_time,
        }
    }

    // ------- Internal helpers -------

    fn apply_move(&self, node: &SearchNode, hop: &RouteHop) -> SearchNode {
        let mut new_hops = node.hops_taken.clone();
        new_hops.push(hop.clone());
        let mut new_bridges = node.bridges_used.clone();
        new_bridges.push(hop.bridge.clone());

        SearchNode {
            current_chain: hop.to_chain,
            current_token: hop.to_token.clone(),
            remaining_amount: hop.output_amount,
            hops_taken: new_hops,
            bridges_used: new_bridges,
            total_fees: node.total_fees + hop.fee,
            total_time: node.total_time + hop.estimated_time,
            depth: node.depth + 1,
        }
    }

    fn build_route_from_node(&self, node: &SearchNode) -> Route {
        Route {
            hops: node.hops_taken.clone(),
            expected_output: node.remaining_amount,
            guaranteed_minimum: node.remaining_amount * 0.95, // conservative estimate
            total_fees: node.total_fees,
            estimated_time: node.total_time,
            minimax_score: 0.0, // will be set by caller
        }
    }

    fn store_in_tt(&mut self, hash: u64, depth: u32, score: f64, flag: TranspositionFlag) {
        self.transposition_table.insert(TranspositionEntry {
            hash,
            depth,
            score,
            flag,
            best_move: None,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bridge::build_mock_registry;
    use crate::types::Strategy;

    fn default_config() -> RouterConfig {
        RouterConfig {
            strategy: Strategy::Minimax,
            max_hops: 2,
            ..RouterConfig::default()
        }
    }

    #[test]
    fn test_minimax_finds_route() {
        let registry = build_mock_registry();
        let mut searcher = MinimaxSearcher::new(default_config());
        let from = Token::new("USDC", Chain::Ethereum, 6, "0xaaa");
        let to = Token::new("USDC", Chain::Arbitrum, 6, "0xbbb");

        let (route, stats) = searcher.search(
            &registry,
            Chain::Ethereum,
            &from,
            Chain::Arbitrum,
            &to,
            10000.0,
        );

        assert!(route.is_some(), "should find at least one route");
        let r = route.unwrap();
        assert!(!r.hops.is_empty());
        assert!(r.expected_output > 0.0);
        assert!(stats.nodes_explored > 0);
    }

    #[test]
    fn test_adversarial_model_worsens_output() {
        let config = default_config();
        let searcher = MinimaxSearcher::new(config);

        let hop = RouteHop {
            from_chain: Chain::Ethereum,
            to_chain: Chain::Arbitrum,
            from_token: Token::new("USDC", Chain::Ethereum, 6, "0xa"),
            to_token: Token::new("USDC", Chain::Arbitrum, 6, "0xb"),
            bridge: "Wormhole".to_string(),
            input_amount: 10000.0,
            output_amount: 9970.0,
            fee: 30.0,
            estimated_time: 180,
        };

        let adversarial = searcher.apply_adversarial_model(&hop);
        assert!(
            adversarial.output_amount < hop.output_amount,
            "adversarial output {} should be less than normal {}",
            adversarial.output_amount,
            hop.output_amount
        );
        assert!(adversarial.fee >= hop.fee);
        assert!(adversarial.estimated_time >= hop.estimated_time);
    }

    #[test]
    fn test_generate_moves_no_loops() {
