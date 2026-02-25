use mnmx_engine::*;

fn test_pool() -> PoolState {
    PoolState::new("pool1", 10_000_000, 50_000_000, 30, "SOL", "USDC")
}

fn test_swap(amount: u64) -> ExecutionAction {
    ExecutionAction::new(ActionKind::Swap, "SOL", amount, "USDC", 50, "pool1", 5000)
}

fn make_state_with_pool() -> OnChainState {
    let mut state = OnChainState::new(100, 1700000000);
    state.pool_states.push(test_pool());
    state
}

#[test]
fn test_sandwich_detection() {
    let detector = MevDetector::new();
    let pool = test_pool();
    let action = test_swap(500_000); // 5% of reserve_a

    let pending = vec![PendingTx::new(
        "sig_mev",
        "MEVbot1",
        "pool1",
        300_000,
        100,
        50_000,
    )];

    let threat = detector.analyze_sandwich_risk(&action, &pending, &[pool]);
    assert!(threat.is_some(), "Should detect sandwich risk");

    let t = threat.unwrap();
    assert_eq!(t.kind, MevKind::Sandwich);
    assert!(t.probability > 0.0 && t.probability <= 1.0);
    assert!(t.estimated_cost > 0);
    assert_eq!(t.affected_pool, "pool1");
}

#[test]
fn test_sandwich_not_detected_small_trade() {
    let detector = MevDetector::new();
    let pool = test_pool();
    let action = test_swap(50); // Tiny trade

    let threat = detector.analyze_sandwich_risk(&action, &[], &[pool]);
    assert!(
        threat.is_none(),
        "Tiny trade should not trigger sandwich detection"
    );
}

#[test]
fn test_frontrun_detection() {
    let detector = MevDetector::new();
    let action = test_swap(200_000);

    // Pending tx with higher fee on the same pool
    let pending = vec![PendingTx::new(
        "sig_front",
        "ARBbot1",
        "pool1",
        100_000,
        100,
        100_000, // Much higher fee
    )];

    let threat = detector.analyze_frontrun_risk(&action, &pending);
    assert!(threat.is_some(), "Should detect frontrun risk");

    let t = threat.unwrap();
    assert_eq!(t.kind, MevKind::Frontrun);
    assert!(t.probability > 0.0);
}

#[test]
fn test_frontrun_not_detected_no_higher_fee() {
    let detector = MevDetector::new();
    let action = ExecutionAction::new(
        ActionKind::Swap,
        "SOL",
        200_000,
        "USDC",
        50,
        "pool1",
        100_000, // Very high fee
    );

    // Pending txs with lower fees
    let pending = vec![PendingTx::new("sig1", "user1", "pool1", 50_000, 100, 1_000)];

    let threat = detector.analyze_frontrun_risk(&action, &pending);
    assert!(
        threat.is_none(),
        "Should not detect frontrun when our fee is highest"
    );
}

#[test]
fn test_jit_detection() {
    let detector = MevDetector::new();
    let pool = test_pool();
    let action = test_swap(5_000_000); // Large swap attracting JIT

    let threat = detector.analyze_jit_risk(&action, &pool);
    assert!(threat.is_some(), "Should detect JIT risk for large swap");

    let t = threat.unwrap();
    assert_eq!(t.kind, MevKind::JitLiquidity);
    assert!(t.probability > 0.0);
    assert!(t.estimated_cost > 0);
}

#[test]
fn test_jit_not_detected_small_swap() {
    let detector = MevDetector::new();
    let pool = test_pool();
    let action = test_swap(100); // Very small swap
