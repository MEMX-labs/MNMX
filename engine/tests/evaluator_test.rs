use std::collections::HashMap;

use mnmx_engine::*;

fn make_state() -> OnChainState {
    let mut balances = HashMap::new();
    balances.insert("SOL".to_string(), 1_000_000);
    balances.insert("USDC".to_string(), 5_000_000);

    OnChainState {
        token_balances: balances,
        pool_states: vec![PoolState::new(
            "pool1",
            10_000_000,
            50_000_000,
            30,
            "SOL",
            "USDC",
        )],
        pending_transactions: Vec::new(),
        slot: 100,
        block_time: 1700000000,
    }
}

fn make_swap_action(amount: u64, priority_fee: u64) -> ExecutionAction {
    ExecutionAction::new(
        ActionKind::Swap,
        "SOL",
        amount,
        "USDC",
        50,
        "pool1",
        priority_fee,
    )
}

#[test]
fn test_gas_cost_scoring() {
    let state = make_state();

    // Low fee action
    let low_fee = make_swap_action(100_000, 1000);
    let gas_low = PositionEvaluator::evaluate_gas_cost(&low_fee, &state);

    // High fee action
    let high_fee = make_swap_action(100_000, 50_000);
    let gas_high = PositionEvaluator::evaluate_gas_cost(&high_fee, &state);

    // Both should be negative (gas is a cost)
    assert!(gas_low < 0.0, "gas_low={}", gas_low);
    assert!(gas_high < 0.0, "gas_high={}", gas_high);

    // Higher fee should have worse (more negative) score
    assert!(
        gas_high < gas_low,
        "Higher fee should produce lower score: high={} low={}",
        gas_high,
        gas_low
    );
}

#[test]
fn test_slippage_calculation() {
    let pool = PoolState::new("pool1", 10_000_000, 50_000_000, 30, "SOL", "USDC");

    // Small swap: low slippage
    let small_swap = make_swap_action(10_000, 5000);
    let slip_small = PositionEvaluator::evaluate_slippage(&small_swap, &pool);

    // Large swap: high slippage
    let large_swap = make_swap_action(5_000_000, 5000);
    let slip_large = PositionEvaluator::evaluate_slippage(&large_swap, &pool);

    // Both should be negative (slippage is bad)
    assert!(slip_small < 0.0);
    assert!(slip_large < 0.0);

    // Larger swap should have worse slippage
    assert!(
        slip_large < slip_small,
        "Larger swap should have worse slippage: large={} small={}",
        slip_large,
        slip_small
    );
}

#[test]
fn test_mev_exposure_scoring() {
    let action = make_swap_action(100_000, 5000);

    // No pending transactions: no MEV exposure
    let mev_clean = PositionEvaluator::evaluate_mev_exposure(&action, &[]);
    assert_eq!(mev_clean, 0.0);

    // Pending transactions on the same pool
    let pending = vec![
        PendingTx::new("sig1", "bot1", "pool1", 50_000, 100, 20_000),
        PendingTx::new("sig2", "bot2", "pool1", 80_000, 100, 30_000),
    ];
    let mev_risky = PositionEvaluator::evaluate_mev_exposure(&action, &pending);

    // Should be negative (MEV exposure is bad)
    assert!(
        mev_risky < 0.0,
        "MEV exposure should be negative: {}",
        mev_risky
    );

    // More pending txs on different pool: less exposure
    let pending_other = vec![PendingTx::new(
        "sig3",
        "bot3",
        "other_pool",
        50_000,
        100,
        20_000,
    )];
    let mev_other = PositionEvaluator::evaluate_mev_exposure(&action, &pending_other);
    assert!(
        mev_other > mev_risky,
        "Different pool should have less exposure: other={} risky={}",
        mev_other,
        mev_risky
    );
}

#[test]
fn test_profit_evaluation() {
    let state = make_state();

    // Swap should have some profit signal
    let swap = make_swap_action(100_000, 5000);
    let profit = PositionEvaluator::evaluate_profit(&swap, &state);
    assert!(
        profit.is_finite(),
        "Profit should be finite: {}",
        profit
    );

    // Liquidation should have positive profit signal
    let liquidate = ExecutionAction::new(
        ActionKind::Liquidate,
        "SOL",
        500_000,
        "vault",
        0,
        "pool1",
        10000,
    );
    let liq_profit = PositionEvaluator::evaluate_profit(&liquidate, &state);
    assert!(
        liq_profit > 0.0,
        "Liquidation should be profitable: {}",
        liq_profit
    );
}

#[test]
fn test_weighted_combination() {
    let evaluator = PositionEvaluator::new(EvalWeights::default());
    let state = make_state();
    let action = make_swap_action(100_000, 5000);
    let result = evaluator.evaluate(&state, &action);

    // Score should be the weighted combination of components
    let weights = EvalWeights::default();
    let expected = weights.combine(&result.breakdown);
    assert!(
        (result.score - expected).abs() < 0.001,
        "Score {} should match weighted combination {}",
        result.score,
        expected
    );
}

#[test]
fn test_confidence_scoring() {
    // State with good info should have higher confidence
    let good_state = make_state();
    let good_conf = PositionEvaluator::calculate_confidence(&good_state);

    // State with no info
    let empty_state = OnChainState::new(0, 0);
    let empty_conf = PositionEvaluator::calculate_confidence(&empty_state);

    assert!(
        good_conf > empty_conf,
        "Good state should have higher confidence: good={} empty={}",
        good_conf,
        empty_conf
    );
    assert!(good_conf >= 0.0 && good_conf <= 1.0);
    assert!(empty_conf >= 0.0 && empty_conf <= 1.0);
}

#[test]
fn test_no_pool_found() {
    let evaluator = PositionEvaluator::new(EvalWeights::default());
    // Action references a pool that doesn't exist in the state
    let action = ExecutionAction::new(
        ActionKind::Swap,
        "SOL",
        100_000,
        "USDC",
        50,
        "nonexistent_pool",
        5000,
    );
    let state = make_state();
    let result = evaluator.evaluate(&state, &action);

    // Should still produce a valid result
    assert!(result.score.is_finite());
    assert!(result.confidence > 0.0);
}

#[test]
fn test_evaluate_transfer_zero_slippage() {
    let pool = PoolState::new("pool1", 10_000_000, 50_000_000, 30, "SOL", "USDC");
    let transfer = ExecutionAction::new(
        ActionKind::Transfer,
        "SOL",
        100_000,
        "wallet2",
        0,
        "pool1",
        5000,
    );
    let slip = PositionEvaluator::evaluate_slippage(&transfer, &pool);
    assert_eq!(slip, 0.0, "Transfer should have zero slippage");
}

#[test]
fn test_evaluate_add_liquidity() {
    let evaluator = PositionEvaluator::new(EvalWeights::default());
    let state = make_state();
    let action = ExecutionAction::new(
        ActionKind::AddLiquidity,
        "SOL",
        250_000,
        "pool1",
        100,
        "pool1",
        5000,
    );
    let result = evaluator.evaluate(&state, &action);
    assert!(result.score.is_finite());
}

#[test]
fn test_static_evaluation() {
    let evaluator = PositionEvaluator::new(EvalWeights::default());

    // Small state
    let mut small_state = OnChainState::new(50, 1700000000);
    small_state.token_balances.insert("SOL".to_string(), 100_000);
    let small_score = evaluator.evaluate_static(&small_state);
    assert!(small_score.is_finite());

    // Richer state should have higher static score
    let mut rich_state = OnChainState::new(50, 1700000000);
    rich_state.token_balances.insert("SOL".to_string(), 50_000_000);
    let rich_score = evaluator.evaluate_static(&rich_state);
    assert!(
        rich_score > small_score,
        "Richer state should score higher: rich={} small={}",
        rich_score,
        small_score
    );
}

#[test]
fn test_price_impact_wrapper() {
    let impact = PositionEvaluator::calculate_price_impact(100_000, 10_000_000, 50_000_000);
    assert!(impact > 0.0 && impact < 1.0);
}
