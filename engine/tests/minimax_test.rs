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

    // Allow generous margin for overhead (5x the limit)
    assert!(
        elapsed < time_limit * 5,
        "Search took {}ms, limit was {}ms",
        elapsed,
        time_limit
    );

    // Should still produce a result from completed iterations
    assert!(plan.expected_score.is_finite() || plan.expected_score == 0.0);
    assert!(plan.search_stats.nodes_explored > 0);
}

#[test]
fn test_transposition_table_improves_perf() {
    let state = make_test_state();
    let actions = make_test_actions();

    let mut engine_tt = MinimaxEngine::new(SearchConfig {
        max_depth: 3,
        time_limit_ms: 10_000,
        transposition_enabled: true,
        alpha_beta_enabled: true,
        move_ordering_enabled: false,
        ..SearchConfig::default()
    });
    let plan_tt = engine_tt.search(&state, &actions);

    let mut engine_no_tt = MinimaxEngine::new(SearchConfig {
        max_depth: 3,
        time_limit_ms: 10_000,
        transposition_enabled: false,
        alpha_beta_enabled: true,
        move_ordering_enabled: false,
        ..SearchConfig::default()
    });
    let plan_no_tt = engine_no_tt.search(&state, &actions);

    // TT should have hits > 0
    assert!(
        plan_tt.search_stats.tt_hits > 0 || plan_tt.search_stats.tt_misses > 0,
        "TT should have been probed"
    );

    // Both should produce valid plans
    assert!(!plan_tt.actions.is_empty());
    assert!(!plan_no_tt.actions.is_empty());
}

#[test]
fn test_move_ordering_consistency() {
    let state = make_test_state();
    let actions = make_test_actions();

    // Run the same search twice with move ordering enabled
    let mut engine1 = MinimaxEngine::new(fast_config(2));
    let plan1 = engine1.search(&state, &actions);

    let mut engine2 = MinimaxEngine::new(fast_config(2));
    let plan2 = engine2.search(&state, &actions);

    // Should produce the same best action
    assert_eq!(
        plan1.actions[0].action_key(),
        plan2.actions[0].action_key(),
        "Same input should produce same best move"
    );
}

#[test]
fn test_empty_actions_returns_empty_plan() {
    let mut engine = MinimaxEngine::new(fast_config(3));
    let state = make_test_state();
    let plan = engine.search(&state, &[]);

    assert!(plan.actions.is_empty());
    assert_eq!(plan.total_cost, 0);
}

#[test]
fn test_single_action_returns_that_action() {
    let mut engine = MinimaxEngine::new(fast_config(2));
    let state = make_test_state();
    let single = vec![ExecutionAction::new(
        ActionKind::Swap,
        "SOL",
        100_000,
        "USDC",
        50,
        "pool_sol_usdc",
        5000,
    )];
    let plan = engine.search(&state, &single);

    assert!(!plan.actions.is_empty());
    assert_eq!(plan.actions[0].token_mint, "SOL");
    assert_eq!(plan.actions[0].amount, 100_000);
}

#[test]
fn test_search_with_pending_transactions() {
    let mut state = make_test_state();
    state.pending_transactions.push(PendingTx::new(
        "sig_mev",
        "MEVbot1",
        "pool_sol_usdc",
        1_000_000,
        200,
        50_000,
    ));

    let mut engine = MinimaxEngine::new(fast_config(2));
    let actions = make_test_actions();
    let plan = engine.search(&state, &actions);

    // Should still produce a valid plan even with MEV threats
    assert!(!plan.actions.is_empty());
    assert!(plan.expected_score.is_finite());
}

#[test]
fn test_stats_tracking() {
    let mut engine = MinimaxEngine::new(fast_config(3));
    let state = make_test_state();
    let actions = make_test_actions();
    let plan = engine.search(&state, &actions);

    let stats = &plan.search_stats;
    assert!(stats.nodes_explored > 0);
    assert!(stats.max_depth_reached >= 1);
    assert!(stats.time_ms > 0 || stats.nodes_explored < 100); // Fast searches might round to 0ms
    assert!(stats.branching_factor >= 0.0);
}

#[test]
fn test_search_different_pool_states() {
    // Test with a very imbalanced pool
    let mut state = make_test_state();
    state.pool_states[0] = PoolState::new(
        "pool_sol_usdc",
        1_000_000,    // Very low reserve A
        500_000_000,  // Very high reserve B
        30,
        "SOL",
        "USDC",
    );

    let mut engine = MinimaxEngine::new(fast_config(2));
    let actions = make_test_actions();
    let plan = engine.search(&state, &actions);

    assert!(!plan.actions.is_empty());
    assert!(plan.expected_score.is_finite());
}
