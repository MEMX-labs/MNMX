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

    let threat = detector.analyze_jit_risk(&action, &pool);
    assert!(
        threat.is_none(),
        "Small swap should not trigger JIT detection"
    );
}

#[test]
fn test_backrun_detection() {
    let detector = MevDetector::new();
    let action = test_swap(500_000);

    let pending = vec![PendingTx::new(
        "sig_back",
        "ARBbot_backrun",
        "other_pool",
        200_000,
        100,
        30_000,
    )];

    let threat = detector.analyze_backrun_risk(&action, &pending);
    assert!(threat.is_some(), "Should detect backrun risk with known bot");
    assert_eq!(threat.unwrap().kind, MevKind::Backrun);
}

#[test]
fn test_no_threats_when_clean() {
    let detector = MevDetector::new();
    let state = make_state_with_pool();
    let action = test_swap(1_000); // Small swap, no pending txs

    let threats = detector.detect_threats(&action, &state);
    // Small trade with no pending txs should have minimal or no threats
    // (may still detect JIT if amount/fee_income is large enough)
    for threat in &threats {
        assert!(threat.probability < 0.5, "Clean state should have low-probability threats");
    }
}

#[test]
fn test_no_threats_for_non_pool_action() {
    let detector = MevDetector::new();
    let state = make_state_with_pool();
    let transfer = ExecutionAction::new(
        ActionKind::Transfer,
        "SOL",
        1_000_000,
        "wallet2",
        0,
        "",
        5000,
    );

    let threats = detector.detect_threats(&transfer, &state);
    assert!(
        threats.is_empty(),
        "Transfer should not generate MEV threats"
    );
}

#[test]
fn test_probability_calculation() {
    // Low signals => low probability
    let low_p = MevDetector::calculate_probability(&[0.05, 0.05, 0.05]);
    assert!(low_p < 0.2, "Low signals should give low probability: {}", low_p);

    // High signals => high probability
    let high_p = MevDetector::calculate_probability(&[0.9, 0.85, 0.95]);
    assert!(high_p > 0.8, "High signals should give high probability: {}", high_p);

    // Empty signals => 0
    assert_eq!(MevDetector::calculate_probability(&[]), 0.0);

    // Single 50% signal
    let mid = MevDetector::calculate_probability(&[0.5]);
    assert!(
        (mid - 0.5).abs() < 0.05,
        "Single 0.5 signal should give ~0.5: {}",
        mid
    );
}

#[test]
fn test_cost_estimation() {
    let pool = test_pool();

    // Sandwich cost should increase with amount
    let cost_small = MevDetector::estimate_sandwich_cost(100_000, &pool);
    let cost_large = MevDetector::estimate_sandwich_cost(1_000_000, &pool);
    assert!(
        cost_large > cost_small,
        "Larger trade should have higher sandwich cost: large={} small={}",
        cost_large,
        cost_small
    );
}

#[test]
fn test_frontrun_cost_estimation() {
    let action = test_swap(200_000);
    let competing = PendingTx::new("sig", "bot", "pool1", 100_000, 100, 50_000);
    let cost = MevDetector::estimate_frontrun_cost(&action, &competing);
    assert!(cost > 0, "Frontrun cost should be positive");
}

#[test]
fn test_known_mev_bot_detection() {
    assert!(MevDetector::is_known_mev_bot("MEVextractor"));
    assert!(MevDetector::is_known_mev_bot("ARBbot123"));
    assert!(MevDetector::is_known_mev_bot("jito_relayer"));
    assert!(MevDetector::is_known_mev_bot("my_sandwich_bot"));
    assert!(MevDetector::is_known_mev_bot("BLXR_relay"));
    assert!(!MevDetector::is_known_mev_bot("RegularUserWallet"));
    assert!(!MevDetector::is_known_mev_bot("11111111111111111111111111111111"));
}

#[test]
fn test_detect_multiple_threats() {
    let detector = MevDetector::new();
    let mut state = make_state_with_pool();

    // Add multiple pending txs from bots
    state.pending_transactions.push(PendingTx::new(
        "sig1", "MEVbot1", "pool1", 500_000, 100, 100_000,
    ));
    state.pending_transactions.push(PendingTx::new(
        "sig2", "ARBbot2", "pool1", 300_000, 100, 80_000,
    ));

    let action = test_swap(1_000_000); // Large swap
    let threats = detector.detect_threats(&action, &state);

    // Should detect multiple threat types
    assert!(
        threats.len() >= 2,
        "Should detect multiple threats, got {}",
        threats.len()
    );

    let kinds: Vec<MevKind> = threats.iter().map(|t| t.kind).collect();
    assert!(
        kinds.contains(&MevKind::Sandwich),
        "Should detect sandwich"
    );
}

#[test]
fn test_threat_expected_value() {
    let threat = MevThreat::new(MevKind::Sandwich, 0.5, 10_000, "bot", "pool");
    let ev = threat.expected_value();
    assert!(
        (ev - 5_000.0).abs() < 0.01,
        "Expected value should be probability * cost: {}",
        ev
    );
}
