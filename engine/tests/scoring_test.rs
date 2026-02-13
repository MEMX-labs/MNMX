use mnmx_engine::scoring::{compare_routes, get_strategy_weights, RouteScorer};
use mnmx_engine::types::*;

fn make_route_with_params(
    hops: usize,
    fee_rate: f64,
    amount: f64,
    time_per_hop: u64,
) -> Route {
    let chains = [Chain::Ethereum, Chain::Arbitrum, Chain::Base, Chain::Polygon];
    let mut route = Route::new();
    let mut current = amount;

    for i in 0..hops {
        let from = chains[i % chains.len()];
        let to = chains[(i + 1) % chains.len()];
        let fee = current * fee_rate;
        let output = current - fee;
        route.hops.push(RouteHop {
            from_chain: from,
            to_chain: to,
            from_token: Token::new("USDC", from, 6, "0xa"),
            to_token: Token::new("USDC", to, 6, "0xb"),
            bridge: "LayerZero".to_string(),
            input_amount: current,
            output_amount: output,
            fee,
            estimated_time: time_per_hop,
        });
        route.total_fees += fee;
        current = output;
    }

    route.expected_output = current;
    route.estimated_time = time_per_hop * hops as u64;
    route
}

#[test]
fn test_scoring_weights_sum_to_one() {
    let default_weights = ScoringWeights::default();
    assert!(
        default_weights.is_valid(),
        "default weights should sum to 1.0, got {}",
        default_weights.sum()
    );
}

#[test]
fn test_minimax_strategy_weights() {
    let w = get_strategy_weights(Strategy::Minimax);
    assert!(w.is_valid());
    // Minimax should be balanced
    assert!(w.fees > 0.1);
    assert!(w.slippage > 0.1);
    assert!(w.speed > 0.05);
    assert!(w.reliability > 0.1);
    assert!(w.mev_exposure > 0.05);
}

#[test]
fn test_cheapest_strategy_weights() {
    let w = get_strategy_weights(Strategy::Cheapest);
    assert!(w.is_valid());
    // Cheapest should heavily weight fees
    assert!(w.fees > w.slippage);
    assert!(w.fees > w.speed);
    assert!(w.fees > w.reliability);
    assert!(w.fees > w.mev_exposure);
}

#[test]
fn test_fastest_strategy_weights() {
