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
