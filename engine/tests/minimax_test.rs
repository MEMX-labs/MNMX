use std::collections::HashMap;

use mnmx_engine::*;

fn make_test_state() -> OnChainState {
    let mut balances = HashMap::new();
    balances.insert("SOL".to_string(), 2_000_000);
    balances.insert("USDC".to_string(), 10_000_000);
    balances.insert("RAY".to_string(), 500_000);

    let pool_sol_usdc = PoolState::new(
        "pool_sol_usdc",
        50_000_000,
        250_000_000,
        30,
        "SOL",
        "USDC",
    );
    let pool_ray_usdc = PoolState::new(
        "pool_ray_usdc",
        20_000_000,
        40_000_000,
        25,
        "RAY",
        "USDC",
    );

    OnChainState {
        token_balances: balances,
        pool_states: vec![pool_sol_usdc, pool_ray_usdc],
        pending_transactions: Vec::new(),
        slot: 200,
        block_time: 1700000000,
    }
}

fn make_test_actions() -> Vec<ExecutionAction> {
    vec![
        ExecutionAction::new(
            ActionKind::Swap,
            "SOL",
            500_000,
            "USDC",
            50,
            "pool_sol_usdc",
            5000,
        ),
        ExecutionAction::new(
            ActionKind::Swap,
            "SOL",
            200_000,
            "USDC",
            30,
            "pool_sol_usdc",
            3000,
        ),
        ExecutionAction::new(
            ActionKind::Swap,
            "RAY",
            100_000,
            "USDC",
            50,
            "pool_ray_usdc",
            5000,
        ),
        ExecutionAction::new(
            ActionKind::Swap,
            "USDC",
            1_000_000,
            "SOL",
            100,
            "pool_sol_usdc",
            8000,
        ),
    ]
}

fn fast_config(max_depth: u32) -> SearchConfig {
    SearchConfig {
        max_depth,
        alpha_beta_enabled: true,
        time_limit_ms: 10_000,
        eval_weights: EvalWeights::default(),
        transposition_enabled: true,
        move_ordering_enabled: true,
    }
}

#[test]
fn test_basic_search_finds_action() {
    let mut engine = MinimaxEngine::new(fast_config(3));
    let state = make_test_state();
    let actions = make_test_actions();
    let plan = engine.search(&state, &actions);

    assert!(
        !plan.actions.is_empty(),
        "Search should find at least one action"
    );
    assert!(
        plan.expected_score.is_finite(),
        "Score should be finite, got {}",
        plan.expected_score
    );
    assert!(
        plan.search_stats.nodes_explored > 0,
        "Should explore at least one node"
    );
}

#[test]
fn test_alpha_beta_matches_full_search() {
    let state = make_test_state();
    let actions = make_test_actions();

    // Full minimax (no pruning)
    let mut engine_full = MinimaxEngine::new(SearchConfig {
        max_depth: 2,
        alpha_beta_enabled: false,
        time_limit_ms: 10_000,
        transposition_enabled: false,
        move_ordering_enabled: false,
        ..SearchConfig::default()
    });
    let plan_full = engine_full.search(&state, &actions);

    // Alpha-beta pruned
    let mut engine_ab = MinimaxEngine::new(SearchConfig {
        max_depth: 2,
        alpha_beta_enabled: true,
        time_limit_ms: 10_000,
        transposition_enabled: false,
        move_ordering_enabled: false,
        ..SearchConfig::default()
    });
    let plan_ab = engine_ab.search(&state, &actions);

    // Both should find actions
    assert!(
        !plan_full.actions.is_empty() && !plan_ab.actions.is_empty(),
        "Both should find actions"
    );

    // Alpha-beta should explore fewer or equal nodes than full search
    assert!(
        plan_ab.search_stats.nodes_explored <= plan_full.search_stats.nodes_explored,
        "AB explored {} vs full {}",
        plan_ab.search_stats.nodes_explored,
        plan_full.search_stats.nodes_explored,
    );

    // Scores should be close (alpha-beta preserves minimax value)
    let score_diff = (plan_ab.expected_score - plan_full.expected_score).abs();
    assert!(
        score_diff < 1.0,
        "AB score={} full score={} diff={}",
        plan_ab.expected_score,
        plan_full.expected_score,
        score_diff,
    );
}

#[test]
fn test_deeper_search_improves_score() {
    let state = make_test_state();
    let actions = make_test_actions();

    let mut engine_d1 = MinimaxEngine::new(SearchConfig {
        max_depth: 1,
        time_limit_ms: 10_000,
        ..fast_config(1)
    });
    let plan_d1 = engine_d1.search(&state, &actions);

    let mut engine_d3 = MinimaxEngine::new(SearchConfig {
        max_depth: 3,
        time_limit_ms: 10_000,
        ..fast_config(3)
    });
    let plan_d3 = engine_d3.search(&state, &actions);

    // Deeper search should explore more nodes
    assert!(
        plan_d3.search_stats.nodes_explored >= plan_d1.search_stats.nodes_explored,
        "Deeper search should explore more nodes: d3={} d1={}",
        plan_d3.search_stats.nodes_explored,
        plan_d1.search_stats.nodes_explored,
    );

    // Both should produce valid plans
    assert!(!plan_d1.actions.is_empty());
    assert!(!plan_d3.actions.is_empty());
}

#[test]
fn test_time_limit_respected() {
    let time_limit = 500;
    let mut engine = MinimaxEngine::new(SearchConfig {
        max_depth: 6, // Moderate depth — should be stopped by time after a few iterations
        time_limit_ms: time_limit,
        alpha_beta_enabled: true,
        transposition_enabled: true,
        move_ordering_enabled: true,
        ..SearchConfig::default()
    });
    let state = make_test_state();
    let actions = make_test_actions();

    let start = std::time::Instant::now();
    let plan = engine.search(&state, &actions);
    let elapsed = start.elapsed().as_millis() as u64;
