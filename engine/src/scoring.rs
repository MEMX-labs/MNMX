use crate::math;
use crate::types::{Route, RouteHop, ScoringWeights, Strategy};

/// Scores routes using weighted multi-objective evaluation.
pub struct RouteScorer {
    weights: ScoringWeights,
}

/// Component scores for a route or hop.
#[derive(Debug, Clone)]
pub struct ScoreBreakdown {
    pub fee_score: f64,
    pub slippage_score: f64,
    pub speed_score: f64,
    pub reliability_score: f64,
    pub mev_score: f64,
    pub composite: f64,
}

impl RouteScorer {
    pub fn new(weights: ScoringWeights) -> Self {
        Self { weights }
    }

    pub fn with_strategy(strategy: Strategy) -> Self {
        Self {
            weights: get_strategy_weights(strategy),
        }
    }

    /// Score a complete route, returning a value in [0, 1] where 1 is best.
    pub fn score_route(&self, route: &Route) -> f64 {
        if route.hops.is_empty() {
            return 0.0;
        }

        let fee_score = self.normalize_fee(route.total_fees, route.expected_output + route.total_fees);
        let slippage_score = self.compute_route_slippage_score(route);
        let speed_score = self.normalize_speed(route.estimated_time);
        let reliability_score = self.compute_route_reliability(route);
        let mev_score = self.normalize_mev(route);

        let composite = self.weights.fees * fee_score
            + self.weights.slippage * slippage_score
            + self.weights.speed * speed_score
            + self.weights.reliability * reliability_score
            + self.weights.mev_exposure * mev_score;

        math::clamp_f64(composite, 0.0, 1.0)
    }

    /// Score a complete route and return the breakdown.
    pub fn score_route_detailed(&self, route: &Route) -> ScoreBreakdown {
        if route.hops.is_empty() {
            return ScoreBreakdown {
                fee_score: 0.0,
                slippage_score: 0.0,
                speed_score: 0.0,
                reliability_score: 0.0,
                mev_score: 0.0,
                composite: 0.0,
            };
        }

        let fee_score = self.normalize_fee(route.total_fees, route.expected_output + route.total_fees);
        let slippage_score = self.compute_route_slippage_score(route);
        let speed_score = self.normalize_speed(route.estimated_time);
        let reliability_score = self.compute_route_reliability(route);
        let mev_score = self.normalize_mev(route);

        let composite = self.weights.fees * fee_score
            + self.weights.slippage * slippage_score
            + self.weights.speed * speed_score
            + self.weights.reliability * reliability_score
            + self.weights.mev_exposure * mev_score;

        ScoreBreakdown {
            fee_score,
            slippage_score,
            speed_score,
            reliability_score,
            mev_score,
            composite: math::clamp_f64(composite, 0.0, 1.0),
        }
    }

    /// Score a single hop.
    pub fn score_hop(&self, hop: &RouteHop) -> f64 {
        let fee_score = self.normalize_fee(hop.fee, hop.input_amount);
        let slippage_score = self.normalize_slippage(hop.slippage());
        let speed_score = self.normalize_speed(hop.estimated_time);
        // For a single hop, reliability is estimated from the bridge and chains
        let reliability_score = self.estimate_hop_reliability(hop);
        let mev_score = self.estimate_hop_mev(hop);

        let composite = self.weights.fees * fee_score
            + self.weights.slippage * slippage_score
            + self.weights.speed * speed_score
            + self.weights.reliability * reliability_score
            + self.weights.mev_exposure * mev_score;

        math::clamp_f64(composite, 0.0, 1.0)
    }

    /// Normalize fee to a score in [0, 1] where lower fees are better (higher score).
    /// fee_ratio = fee / total_value, mapped through an inverse curve.
    pub fn normalize_fee(&self, fee: f64, total_value: f64) -> f64 {
        if total_value <= 0.0 {
            return 0.0;
        }
        let fee_ratio = fee / total_value;
        // Map fee ratio to score: 0% fee = 1.0, 1% = 0.8, 5% = 0.2, 10%+ = ~0
        // Using exponential decay: score = e^(-k * fee_ratio)
        let k = 30.0;
        let score = (-k * fee_ratio).exp();
        math::clamp_f64(score, 0.0, 1.0)
    }

    /// Normalize slippage to [0, 1] where lower slippage is better.
    pub fn normalize_slippage(&self, slippage: f64) -> f64 {
        if slippage <= 0.0 {
            return 1.0;
        }
        // 0.1% slippage = ~0.97, 1% = ~0.74, 5% = ~0.22
        let k = 30.0;
        let score = (-k * slippage).exp();
        math::clamp_f64(score, 0.0, 1.0)
    }

    /// Normalize time (in seconds) to [0, 1] where faster is better.
    pub fn normalize_speed(&self, time_seconds: u64) -> f64 {
        if time_seconds == 0 {
            return 1.0;
        }
        // Mapping: 30s = ~0.95, 120s = ~0.8, 600s = ~0.4, 1800s = ~0.1
        // Using: score = 1 / (1 + t/300)
        let t = time_seconds as f64;
        let score = 1.0 / (1.0 + t / 300.0);
        math::clamp_f64(score, 0.0, 1.0)
    }

    /// Normalize bridge reliability to [0, 1].
    pub fn normalize_reliability(&self, success_rate: f64, congestion_factor: f64) -> f64 {
        let raw = success_rate * congestion_factor;
        math::clamp_f64(raw, 0.0, 1.0)
    }

    /// Normalize MEV exposure to [0, 1] where lower exposure is better.
    pub fn normalize_mev(&self, route: &Route) -> f64 {
        // MEV risk increases with:
        // - Number of hops (more transactions = more surface)
        // - Value of route (larger amounts attract more MEV)
        // - Chains involved (Ethereum mainnet has highest MEV)
        let hop_penalty = match route.hops.len() {
            0 => return 1.0,
            1 => 0.05,
            2 => 0.12,
            3 => 0.20,
            _ => 0.30,
        };

        let value_penalty = if route.expected_output > 100_000.0 {
            0.15
        } else if route.expected_output > 10_000.0 {
            0.08
        } else if route.expected_output > 1_000.0 {
            0.03
        } else {
            0.01
        };

        let eth_mainnet_hops = route
            .hops
            .iter()
            .filter(|h| {
                h.from_chain == crate::types::Chain::Ethereum
                    || h.to_chain == crate::types::Chain::Ethereum
            })
            .count();
        let chain_penalty = eth_mainnet_hops as f64 * 0.05;

        let total_penalty = hop_penalty + value_penalty + chain_penalty;
        let score = 1.0 - total_penalty;
        math::clamp_f64(score, 0.0, 1.0)
    }

    /// Compute aggregate slippage score for a route.
    fn compute_route_slippage_score(&self, route: &Route) -> f64 {
        if route.hops.is_empty() {
            return 1.0;
        }
        // Compound slippage across hops
        let mut cumulative_retention = 1.0;
        for hop in &route.hops {
            let hop_slippage = hop.slippage();
            cumulative_retention *= 1.0 - hop_slippage;
        }
        let total_slippage = 1.0 - cumulative_retention;
        self.normalize_slippage(total_slippage)
    }

    /// Compute aggregate reliability for a route.
    fn compute_route_reliability(&self, route: &Route) -> f64 {
        if route.hops.is_empty() {
            return 1.0;
        }
        // Route reliability is the product of hop reliabilities
        let mut reliability = 1.0;
        for hop in &route.hops {
            let hop_rel = self.estimate_hop_reliability(hop);
            reliability *= hop_rel;
        }
        reliability
    }

    /// Estimate reliability for a single hop based on bridge and chains.
    fn estimate_hop_reliability(&self, hop: &RouteHop) -> f64 {
        // Base reliability from bridge name heuristic
        let base = match hop.bridge.as_str() {
            "Wormhole" => 0.97,
            "LayerZero" => 0.98,
            "deBridge" => 0.96,
            "Allbridge" => 0.94,
            _ => 0.90,
        };

        // Chain-specific adjustment
        let chain_factor = match (hop.from_chain, hop.to_chain) {
            (crate::types::Chain::Ethereum, _) | (_, crate::types::Chain::Ethereum) => 0.99,
            (crate::types::Chain::Solana, _) | (_, crate::types::Chain::Solana) => 0.95,
            _ => 0.98,
        };

        base * chain_factor
    }

    /// Estimate MEV exposure for a single hop.
    fn estimate_hop_mev(&self, hop: &RouteHop) -> f64 {
        let value_factor = if hop.input_amount > 50_000.0 {
            0.7
        } else if hop.input_amount > 10_000.0 {
            0.85
        } else {
            0.95
        };

        let chain_factor = match hop.from_chain {
            crate::types::Chain::Ethereum => 0.85,
            crate::types::Chain::BnbChain => 0.90,
            _ => 0.95,
        };

        value_factor * chain_factor
    }
}

/// Compare two routes by minimax score. Returns Ordering.
pub fn compare_routes(a: &Route, b: &Route) -> std::cmp::Ordering {
    b.minimax_score
        .partial_cmp(&a.minimax_score)
        .unwrap_or(std::cmp::Ordering::Equal)
}

/// Get scoring weights for a given strategy.
pub fn get_strategy_weights(strategy: Strategy) -> ScoringWeights {
    match strategy {
        Strategy::Minimax => ScoringWeights {
            fees: 0.25,
            slippage: 0.25,
            speed: 0.15,
            reliability: 0.20,
            mev_exposure: 0.15,
        },
        Strategy::Cheapest => ScoringWeights {
            fees: 0.50,
            slippage: 0.20,
            speed: 0.05,
            reliability: 0.15,
            mev_exposure: 0.10,
        },
        Strategy::Fastest => ScoringWeights {
            fees: 0.10,
            slippage: 0.15,
            speed: 0.50,
            reliability: 0.15,
            mev_exposure: 0.10,
        },
        Strategy::Safest => ScoringWeights {
            fees: 0.10,
            slippage: 0.15,
            speed: 0.05,
            reliability: 0.45,
            mev_exposure: 0.25,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Chain, Token};

    fn make_route(hops: usize, fee_per_hop: f64, amount: f64) -> Route {
        let mut route = Route::new();
        let mut current_amount = amount;
        for i in 0..hops {
            let chains = [Chain::Ethereum, Chain::Arbitrum, Chain::Base, Chain::Polygon];
            let from_chain = chains[i % chains.len()];
            let to_chain = chains[(i + 1) % chains.len()];
            let fee = current_amount * fee_per_hop;
            let output = current_amount - fee;
            route.hops.push(RouteHop {
                from_chain,
                to_chain,
                from_token: Token::new("USDC", from_chain, 6, "0xaaa"),
                to_token: Token::new("USDC", to_chain, 6, "0xbbb"),
                bridge: "LayerZero".to_string(),
                input_amount: current_amount,
                output_amount: output,
                fee,
                estimated_time: 120,
            });
            route.total_fees += fee;
            current_amount = output;
        }
        route.expected_output = current_amount;
        route.estimated_time = 120 * hops as u64;
        route
    }

    #[test]
    fn test_fee_normalization() {
        let scorer = RouteScorer::with_strategy(Strategy::Minimax);
        // 0% fee -> score near 1.0
        assert!(scorer.normalize_fee(0.0, 1000.0) > 0.99);
        // 1% fee -> score ~0.74
        let s = scorer.normalize_fee(10.0, 1000.0);
        assert!(s > 0.5 && s < 0.9);
        // 10% fee -> low score
        assert!(scorer.normalize_fee(100.0, 1000.0) < 0.1);
    }

    #[test]
    fn test_speed_normalization() {
        let scorer = RouteScorer::with_strategy(Strategy::Minimax);
        assert!(scorer.normalize_speed(0) > 0.99);
        assert!(scorer.normalize_speed(30) > 0.85);
        assert!(scorer.normalize_speed(600) < 0.5);
    }

    #[test]
    fn test_route_scoring() {
        let scorer = RouteScorer::with_strategy(Strategy::Minimax);
        let route1 = make_route(1, 0.003, 10000.0);
        let route2 = make_route(3, 0.005, 10000.0);
        let s1 = scorer.score_route(&route1);
        let s2 = scorer.score_route(&route2);
        // Single hop with lower fees should score better
        assert!(s1 > s2, "single hop should beat 3-hop: {} vs {}", s1, s2);
    }

    #[test]
    fn test_strategy_weights_valid() {
        for strategy in &[Strategy::Minimax, Strategy::Cheapest, Strategy::Fastest, Strategy::Safest] {
            let w = get_strategy_weights(*strategy);
            assert!(w.is_valid(), "weights for {:?} don't sum to 1", strategy);
        }
    }

    #[test]
    fn test_compare_routes() {
        let mut a = Route::new();
        a.minimax_score = 0.8;
        let mut b = Route::new();
        b.minimax_score = 0.6;
        assert_eq!(compare_routes(&a, &b), std::cmp::Ordering::Less);
    }
}
