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
