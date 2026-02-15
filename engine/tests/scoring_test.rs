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
    let w = get_strategy_weights(Strategy::Fastest);
    assert!(w.is_valid());
    // Fastest should heavily weight speed
    assert!(w.speed > w.fees);
    assert!(w.speed > w.slippage);
    assert!(w.speed > w.reliability);
    assert!(w.speed > w.mev_exposure);
}

#[test]
fn test_safest_strategy_weights() {
    let w = get_strategy_weights(Strategy::Safest);
    assert!(w.is_valid());
    // Safest should heavily weight reliability
    assert!(w.reliability > w.fees);
    assert!(w.reliability > w.slippage);
    assert!(w.reliability > w.speed);
}

#[test]
fn test_fee_normalization() {
    let scorer = RouteScorer::with_strategy(Strategy::Minimax);

    // Zero fee -> perfect score
    let zero_fee = scorer.normalize_fee(0.0, 10000.0);
    assert!(zero_fee > 0.99, "zero fee score should be ~1.0: {}", zero_fee);

    // 0.3% fee -> good score
    let low_fee = scorer.normalize_fee(30.0, 10000.0);
    assert!(low_fee > 0.7, "0.3% fee should score > 0.7: {}", low_fee);

    // 5% fee -> poor score
    let high_fee = scorer.normalize_fee(500.0, 10000.0);
    assert!(high_fee < 0.3, "5% fee should score < 0.3: {}", high_fee);

    // Monotonically decreasing
    assert!(zero_fee > low_fee);
    assert!(low_fee > high_fee);
}

#[test]
fn test_route_comparison() {
    let mut route_a = make_route_with_params(1, 0.002, 10000.0, 60);
    route_a.minimax_score = 0.85;

    let mut route_b = make_route_with_params(2, 0.005, 10000.0, 120);
    route_b.minimax_score = 0.65;

    let ordering = compare_routes(&route_a, &route_b);
    // route_a has higher score, so it should come first (Less in sort order)
    assert_eq!(
        ordering,
        std::cmp::Ordering::Less,
        "higher-scored route should sort first"
    );
}

#[test]
fn test_lower_fees_score_higher() {
    let scorer = RouteScorer::with_strategy(Strategy::Cheapest);

    let cheap_route = make_route_with_params(1, 0.002, 10000.0, 120);
    let expensive_route = make_route_with_params(1, 0.01, 10000.0, 120);

    let cheap_score = scorer.score_route(&cheap_route);
    let expensive_score = scorer.score_route(&expensive_route);

    assert!(
        cheap_score > expensive_score,
        "cheaper route should score higher: {} vs {}",
        cheap_score,
        expensive_score
    );
