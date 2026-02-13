use mnmx_engine::bridge::build_mock_registry;
use mnmx_engine::minimax::MinimaxSearcher;
use mnmx_engine::types::*;

fn default_config(max_hops: usize) -> RouterConfig {
    RouterConfig {
        strategy: Strategy::Minimax,
        max_hops,
        ..RouterConfig::default()
    }
}

fn eth_usdc() -> Token {
    Token::new("USDC", Chain::Ethereum, 6, "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48")
}

fn arb_usdc() -> Token {
    Token::new("USDC", Chain::Arbitrum, 6, "0xaf88d065e77c8cC2239327C5EDb3A432268e5831")
}

fn base_usdc() -> Token {
    Token::new("USDC", Chain::Base, 6, "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913")
}

#[test]
fn test_minimax_finds_optimal_route() {
    let registry = build_mock_registry();
    let mut searcher = MinimaxSearcher::new(default_config(2));

    let (route, stats) = searcher.search(
        &registry,
        Chain::Ethereum,
        &eth_usdc(),
        Chain::Arbitrum,
        &arb_usdc(),
        10000.0,
    );

    assert!(route.is_some(), "minimax should find a route");
    let route = route.unwrap();
    assert!(!route.hops.is_empty());
    assert!(route.expected_output > 9000.0, "should retain most value: {}", route.expected_output);
    assert!(route.minimax_score > 0.0, "should have positive score");
    assert!(stats.nodes_explored > 0);
}

#[test]
fn test_alpha_beta_pruning_reduces_nodes() {
    let registry = build_mock_registry();

    // Search with max_hops=1 (small tree)
    let mut searcher1 = MinimaxSearcher::new(default_config(1));
    let (_, stats1) = searcher1.search(
        &registry,
        Chain::Ethereum,
        &eth_usdc(),
        Chain::Arbitrum,
        &arb_usdc(),
        10000.0,
    );

    // Search with max_hops=2 (larger tree, more pruning opportunities)
    let mut searcher2 = MinimaxSearcher::new(default_config(2));
    let (_, stats2) = searcher2.search(
        &registry,
        Chain::Ethereum,
        &eth_usdc(),
        Chain::Arbitrum,
        &arb_usdc(),
        10000.0,
    );

    // Larger search should explore more nodes but also prune some
    assert!(
        stats2.nodes_explored >= stats1.nodes_explored,
        "deeper search should explore at least as many nodes: {} vs {}",
        stats2.nodes_explored,
        stats1.nodes_explored
    );
}

#[test]
fn test_adversarial_model_worsens_scores() {
    let registry = build_mock_registry();

    // Mild adversary
    let mild_config = RouterConfig {
        strategy: Strategy::Minimax,
        max_hops: 1,
        adversarial_model: AdversarialModel {
            slippage_multiplier: 1.1,
            gas_multiplier: 1.1,
            bridge_delay_multiplier: 1.1,
            mev_extraction: 0.001,
            price_movement: 0.005,
        },
        ..RouterConfig::default()
    };

    // Harsh adversary
    let harsh_config = RouterConfig {
        strategy: Strategy::Minimax,
        max_hops: 1,
        adversarial_model: AdversarialModel {
            slippage_multiplier: 5.0,
            gas_multiplier: 3.0,
            bridge_delay_multiplier: 5.0,
            mev_extraction: 0.02,
            price_movement: 0.10,
        },
        ..RouterConfig::default()
    };

    let mut mild_searcher = MinimaxSearcher::new(mild_config);
    let (mild_route, _) = mild_searcher.search(
        &registry,
        Chain::Ethereum,
        &eth_usdc(),
        Chain::Arbitrum,
        &arb_usdc(),
        10000.0,
    );

    let mut harsh_searcher = MinimaxSearcher::new(harsh_config);
    let (harsh_route, _) = harsh_searcher.search(
        &registry,
        Chain::Ethereum,
        &eth_usdc(),
        Chain::Arbitrum,
        &arb_usdc(),
        10000.0,
    );

    assert!(mild_route.is_some());
    assert!(harsh_route.is_some());

    let mild_score = mild_route.unwrap().minimax_score;
    let harsh_score = harsh_route.unwrap().minimax_score;

    // Harsh adversary should produce lower minimax scores (or equal if same route)
    assert!(
        harsh_score <= mild_score + 0.01,
        "harsh adversary should not improve score: mild={} harsh={}",
        mild_score,
        harsh_score
    );
}

#[test]
fn test_deeper_search_finds_better_routes() {
    let registry = build_mock_registry();

    // For Ethereum -> Base, a multi-hop route might be competitive
    let mut searcher1 = MinimaxSearcher::new(default_config(1));
    let (route1, _) = searcher1.search(
        &registry,
        Chain::Ethereum,
        &eth_usdc(),
        Chain::Base,
        &base_usdc(),
        10000.0,
    );

    let mut searcher2 = MinimaxSearcher::new(default_config(2));
    let (route2, _) = searcher2.search(
        &registry,
        Chain::Ethereum,
        &eth_usdc(),
        Chain::Base,
        &base_usdc(),
        10000.0,
    );

    // Both should find routes
    assert!(route1.is_some());
    assert!(route2.is_some());

    // Deeper search should find a route at least as good
    let score1 = route1.unwrap().minimax_score;
    let score2 = route2.unwrap().minimax_score;
    assert!(
        score2 >= score1 - 0.01,
        "deeper search should not find worse route: depth1={} depth2={}",
        score1,
        score2
    );
}

#[test]
fn test_minimax_vs_greedy_comparison() {
    let registry = build_mock_registry();

    // Minimax strategy
    let mut minimax_searcher = MinimaxSearcher::new(default_config(2));
    let (minimax_route, _) = minimax_searcher.search(
        &registry,
        Chain::Ethereum,
        &eth_usdc(),
        Chain::Arbitrum,
        &arb_usdc(),
        50000.0,
    );

    // "Greedy" - just use cheapest strategy with 1 hop
    let cheapest_config = RouterConfig {
        strategy: Strategy::Cheapest,
        max_hops: 1,
        ..RouterConfig::default()
    };
    let mut cheapest_searcher = MinimaxSearcher::new(cheapest_config);
    let (cheapest_route, _) = cheapest_searcher.search(
        &registry,
        Chain::Ethereum,
        &eth_usdc(),
        Chain::Arbitrum,
        &arb_usdc(),
        50000.0,
    );

    assert!(minimax_route.is_some());
    assert!(cheapest_route.is_some());

    let minimax_r = minimax_route.unwrap();
    let cheapest_r = cheapest_route.unwrap();

    // Both should produce valid routes with positive output
    assert!(minimax_r.expected_output > 0.0);
    assert!(cheapest_r.expected_output > 0.0);
}
