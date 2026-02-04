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
