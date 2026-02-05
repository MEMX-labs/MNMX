use crate::math;
use crate::types::{AdversarialModel, Chain, RiskLevel, Route, RouteHop};

/// Assesses risk for routes using the adversarial model.
pub struct RiskAssessor {
    model: AdversarialModel,
}

/// Detailed risk assessment for a route.
#[derive(Debug, Clone)]
pub struct RouteRiskAssessment {
    pub risk_level: RiskLevel,
    pub worst_case_output: f64,
    pub worst_case_fees: f64,
    pub worst_case_time: u64,
    pub mev_loss_estimate: f64,
    pub price_impact_estimate: f64,
    pub confidence: f64,
}

/// Risk assessment for a single hop.
#[derive(Debug, Clone)]
pub struct HopRiskAssessment {
    pub worst_case_output: f64,
    pub worst_case_fee: f64,
    pub worst_case_time: u64,
    pub worst_case_slippage: f64,
    pub mev_risk: f64,
}

impl RiskAssessor {
    pub fn new(model: AdversarialModel) -> Self {
        Self { model }
    }

    /// Assess worst-case risk for an entire route.
    pub fn assess_route_risk(&self, route: &Route) -> RouteRiskAssessment {
        if route.hops.is_empty() {
            return RouteRiskAssessment {
                risk_level: RiskLevel::Low,
                worst_case_output: 0.0,
                worst_case_fees: 0.0,
                worst_case_time: 0,
                mev_loss_estimate: 0.0,
                price_impact_estimate: 0.0,
                confidence: 1.0,
            };
        }

        let mut worst_case_remaining = route.hops.first().map(|h| h.input_amount).unwrap_or(0.0);
        let mut total_worst_fees = 0.0;
        let mut total_worst_time = 0u64;
        for hop in &route.hops {
            let hop_assessment = self.assess_hop_risk(hop);
            let loss_ratio = if hop.input_amount > 0.0 {
                hop_assessment.worst_case_output / hop.input_amount
            } else {
                1.0
            };
            worst_case_remaining *= loss_ratio;
            total_worst_fees += hop_assessment.worst_case_fee;
            total_worst_time += hop_assessment.worst_case_time;
        }

        let total_mev_loss = self.estimate_mev_loss(route);
        let price_impact = self.compute_price_impact(route);
        worst_case_remaining -= total_mev_loss;
        worst_case_remaining *= 1.0 - price_impact;

        if worst_case_remaining < 0.0 {
            worst_case_remaining = 0.0;
        }

        let input_amount = route.hops.first().map(|h| h.input_amount).unwrap_or(1.0);
        let loss_fraction = 1.0 - (worst_case_remaining / input_amount);
        let risk_level = self.classify_risk(loss_fraction, route.hops.len());

        // Confidence decreases with more hops and more aggressive adversarial model
        let confidence = self.compute_confidence(route);

        RouteRiskAssessment {
            risk_level,
            worst_case_output: worst_case_remaining,
            worst_case_fees: total_worst_fees,
            worst_case_time: total_worst_time,
            mev_loss_estimate: total_mev_loss,
            price_impact_estimate: price_impact,
            confidence,
        }
    }

    /// Assess worst-case risk for a single hop.
    pub fn assess_hop_risk(&self, hop: &RouteHop) -> HopRiskAssessment {
        let worst_slippage = self.compute_worst_case_slippage(hop.slippage());
        let worst_fee = hop.fee * self.model.gas_multiplier;
        let worst_time = self.compute_worst_case_delay(hop.estimated_time);

        let worst_output = hop.input_amount * (1.0 - worst_slippage) - worst_fee;
        let worst_output = if worst_output < 0.0 { 0.0 } else { worst_output };

        let mev_risk = self.compute_hop_mev_risk(hop);

        HopRiskAssessment {
            worst_case_output: worst_output,
            worst_case_fee: worst_fee,
            worst_case_time: worst_time,
            worst_case_slippage: worst_slippage,
            mev_risk,
        }
    }

    /// Compute worst-case slippage by multiplying observed slippage.
    pub fn compute_worst_case_slippage(&self, observed_slippage: f64) -> f64 {
        let worst = observed_slippage * self.model.slippage_multiplier;
        // Cap at 50% - beyond that the trade should not execute
        math::clamp_f64(worst, 0.0, 0.5)
    }

    /// Compute worst-case gas cost.
    pub fn compute_worst_case_gas(&self, current_gas: f64) -> f64 {
        current_gas * self.model.gas_multiplier
    }

    /// Compute worst-case bridge delay in seconds.
    pub fn compute_worst_case_delay(&self, median_delay: u64) -> u64 {
        (median_delay as f64 * self.model.bridge_delay_multiplier) as u64
    }

    /// Estimate total MEV extraction for a route.
    pub fn estimate_mev_loss(&self, route: &Route) -> f64 {
        if route.hops.is_empty() {
            return 0.0;
        }

        let total_value = route.hops.first().map(|h| h.input_amount).unwrap_or(0.0);
        let base_mev = total_value * self.model.mev_extraction;

        // MEV increases with number of hops
        let hop_multiplier = match route.hops.len() {
            1 => 1.0,
            2 => 1.8,
            3 => 2.5,
            _ => 3.0,
        };

        // MEV is higher on certain chains
        let chain_factor: f64 = route
            .hops
            .iter()
            .map(|hop| match hop.from_chain {
                Chain::Ethereum => 1.5,
                Chain::BnbChain => 1.2,
                Chain::Polygon => 1.1,
                Chain::Arbitrum => 0.8,
                Chain::Base => 0.7,
                Chain::Optimism => 0.7,
                Chain::Solana => 0.6,
                Chain::Avalanche => 0.9,
            })
            .sum::<f64>()
            / route.hops.len() as f64;

        base_mev * hop_multiplier * chain_factor
    }

    /// Estimate adverse price movement during route execution.
    pub fn compute_price_impact(&self, route: &Route) -> f64 {
        if route.hops.is_empty() {
            return 0.0;
        }

        // Price can move against us during execution time
        let total_time = route.estimated_time as f64;
        // Assume price volatility scales with sqrt(time) (random walk)
        let time_factor = (total_time / 60.0).sqrt();

        // Base price movement from adversarial model
        let base_movement = self.model.price_movement;

        // Stablecoin routes have minimal price impact
        let token_factor = if route
            .hops
            .iter()
            .all(|h| h.from_token.is_stablecoin() && h.to_token.is_stablecoin())
        {
            0.05
        } else {
            1.0
        };

        let impact = base_movement * time_factor * token_factor;
        math::clamp_f64(impact, 0.0, 0.5)
    }

    /// Classify route risk based on potential loss and complexity.
    pub fn get_risk_level(&self, route: &Route) -> RiskLevel {
        let assessment = self.assess_route_risk(route);
        assessment.risk_level
    }

    // ------- Internal helpers -------

    fn classify_risk(&self, loss_fraction: f64, hop_count: usize) -> RiskLevel {
        let hop_risk_add = match hop_count {
            1 => 0.0,
            2 => 0.02,
            3 => 0.05,
            _ => 0.10,
        };
        let total_risk = loss_fraction + hop_risk_add;

        if total_risk < 0.02 {
            RiskLevel::Low
        } else if total_risk < 0.05 {
            RiskLevel::Medium
        } else if total_risk < 0.15 {
            RiskLevel::High
        } else {
            RiskLevel::Critical
        }
    }

    fn compute_confidence(&self, route: &Route) -> f64 {
        // Confidence decreases with hops and aggressive adversarial assumptions
        let hop_factor = 1.0 / (1.0 + 0.15 * route.hops.len() as f64);
        let model_factor = 1.0
            / (1.0
                + (self.model.slippage_multiplier - 1.0).abs()
                + (self.model.gas_multiplier - 1.0).abs());
        math::clamp_f64(hop_factor * model_factor, 0.0, 1.0)
    }

    fn compute_hop_mev_risk(&self, hop: &RouteHop) -> f64 {
        let chain_risk = match hop.from_chain {
            Chain::Ethereum => 0.008,
            Chain::BnbChain => 0.005,
            Chain::Polygon => 0.004,
            Chain::Arbitrum => 0.002,
            _ => 0.001,
        };

        let value_factor = if hop.input_amount > 100_000.0 {
            2.0
        } else if hop.input_amount > 10_000.0 {
            1.5
        } else {
            1.0
        };

        chain_risk * value_factor
    }
}

impl Default for RiskAssessor {
    fn default() -> Self {
        Self::new(AdversarialModel::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Token;

    fn make_test_route() -> Route {
        let mut route = Route::new();
        route.hops.push(RouteHop {
            from_chain: Chain::Ethereum,
            to_chain: Chain::Arbitrum,
            from_token: Token::new("USDC", Chain::Ethereum, 6, "0xaaa"),
            to_token: Token::new("USDC", Chain::Arbitrum, 6, "0xbbb"),
            bridge: "Wormhole".to_string(),
            input_amount: 10000.0,
            output_amount: 9950.0,
            fee: 30.0,
            estimated_time: 180,
        });
        route.expected_output = 9950.0;
        route.total_fees = 30.0;
        route.estimated_time = 180;
        route
    }

    #[test]
    fn test_risk_assessment() {
        let assessor = RiskAssessor::default();
        let route = make_test_route();
        let assessment = assessor.assess_route_risk(&route);
        assert!(assessment.worst_case_output > 0.0);
        assert!(assessment.worst_case_output < route.expected_output);
    }

    #[test]
    fn test_worst_case_slippage() {
        let assessor = RiskAssessor::default();
        let worst = assessor.compute_worst_case_slippage(0.01);
        // Default multiplier is 2.0, so 0.01 * 2 = 0.02
        assert!((worst - 0.02).abs() < 1e-9);
    }

    #[test]
    fn test_worst_case_delay() {
        let assessor = RiskAssessor::default();
        let worst = assessor.compute_worst_case_delay(120);
        // Default multiplier is 2.0, so 120 * 2 = 240
        assert_eq!(worst, 240);
    }

    #[test]
    fn test_mev_loss_increases_with_hops() {
        let assessor = RiskAssessor::default();
        let route1 = make_test_route();

        let mut route2 = make_test_route();
        route2.hops.push(RouteHop {
            from_chain: Chain::Arbitrum,
            to_chain: Chain::Base,
            from_token: Token::new("USDC", Chain::Arbitrum, 6, "0xbbb"),
            to_token: Token::new("USDC", Chain::Base, 6, "0xccc"),
            bridge: "LayerZero".to_string(),
            input_amount: 9950.0,
            output_amount: 9920.0,
            fee: 20.0,
            estimated_time: 60,
        });

        let mev1 = assessor.estimate_mev_loss(&route1);
        let mev2 = assessor.estimate_mev_loss(&route2);
        assert!(mev2 > mev1);
    }
}
