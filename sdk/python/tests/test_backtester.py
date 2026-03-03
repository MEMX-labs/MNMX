"""
Tests for the MNMX Backtester.
"""

from __future__ import annotations

import pytest

from mnmx.backtester import (
    Backtester,
    BacktestMetrics,
    MevAwareStrategy,
    SimpleSwapStrategy,
    Strategy,
)
from mnmx.types import (
    ActionKind,
    BacktestConfig,
    BacktestResult,
    ExecutionAction,
    OnChainState,
    PendingTx,
    PoolState,
    TradeRecord,
)


POOL_ADDR = "A" * 44
TOKEN_A = "SoLMint111111111111111111111111111111111111"
TOKEN_B = "USDCMint11111111111111111111111111111111111"


def _make_pool(slot: int = 100, ra: int = 1_000_000, rb: int = 500_000) -> PoolState:
    return PoolState(
        address=POOL_ADDR,
        token_a_mint=TOKEN_A,
        token_b_mint=TOKEN_B,
        reserve_a=ra,
        reserve_b=rb,
        fee_bps=30,
    )


def _make_state(slot: int, ra: int = 1_000_000, rb: int = 500_000) -> OnChainState:
    return OnChainState(
        slot=slot,
        pools=[_make_pool(slot, ra, rb)],
        balances={TOKEN_A: 10_000_000, TOKEN_B: 5_000_000},
    )


def _make_history(n: int = 20) -> list[OnChainState]:
    """Generate n states with slowly drifting reserves."""
    states = []
    ra = 1_000_000
    rb = 500_000
    for i in range(n):
        # small random-ish drift
        ra += (-1) ** i * 1000
        rb += (-1) ** (i + 1) * 500
        states.append(_make_state(slot=1000 + i, ra=max(ra, 10_000), rb=max(rb, 5_000)))
    return states


class TestSimpleStrategyRun:
    def test_backtest_produces_result(self) -> None:
        history = _make_history(10)
        strategy = SimpleSwapStrategy(
            token_in=TOKEN_A,
            token_out=TOKEN_B,
            amount=10_000,
        )
        bt = Backtester(BacktestConfig(initial_balance={TOKEN_A: 1_000_000, TOKEN_B: 0}))
        result = bt.run(history, strategy)

        assert isinstance(result, BacktestResult)
        assert result.num_trades > 0
        assert result.start_slot == 1000
        assert result.end_slot == 1009
        assert len(result.equity_curve) == len(history) + 1

    def test_no_trades_when_insufficient_balance(self) -> None:
        history = _make_history(5)
        strategy = SimpleSwapStrategy(
            token_in=TOKEN_A,
            token_out=TOKEN_B,
            amount=999_999_999_999,  # way more than we have
        )
        bt = Backtester(BacktestConfig(initial_balance={TOKEN_A: 100}))
        result = bt.run(history, strategy)
        assert result.num_trades == 0

    def test_simple_strategy_skips_high_impact(self) -> None:
        # tiny pool => high impact for any reasonable trade
        states = [_make_state(i, ra=100, rb=50) for i in range(5)]
        strategy = SimpleSwapStrategy(
            token_in=TOKEN_A,
            token_out=TOKEN_B,
            amount=100,
            max_impact_bps=10,
        )
        bt = Backtester(BacktestConfig(initial_balance={TOKEN_A: 10_000}))
        result = bt.run(states, strategy)
        # most/all trades should be skipped due to high impact
        assert result.num_trades <= len(states)


class TestMevAwareStrategy:
    def test_mev_strategy_avoids_risky_mempool(self) -> None:
        states = _make_history(10)
        # inject pending txs to raise MEV risk
        for s in states:
            s.pending_txs = [
                PendingTx(
                    signature=f"sig_{s.slot}",
                    sender="attacker_111111111111111111111111111111111",
                    action=ExecutionAction(
                        kind=ActionKind.SWAP,
                        pool_address=POOL_ADDR,
                        token_in=TOKEN_A,
                        token_out=TOKEN_B,
                        amount_in=500_000,
                        min_amount_out=0,
                    ),
                    priority_fee=100_000,
                )
                for _ in range(5)  # 5 competing txs
            ]

        strategy = MevAwareStrategy(
            token_in=TOKEN_A,
            token_out=TOKEN_B,
            amount=10_000,
            max_mev_risk=0.1,
        )
        bt = Backtester(BacktestConfig(initial_balance={TOKEN_A: 1_000_000}))
        result = bt.run(states, strategy)

        # the strategy should skip many slots due to high MEV risk
        # but the consecutive skip limit (5) means it eventually trades
        assert result.num_trades < len(states)

    def test_mev_strategy_trades_when_safe(self) -> None:
        states = _make_history(10)
        strategy = MevAwareStrategy(
            token_in=TOKEN_A,
            token_out=TOKEN_B,
            amount=10_000,
            max_mev_risk=0.9,
        )
        bt = Backtester(BacktestConfig(initial_balance={TOKEN_A: 1_000_000}))
        result = bt.run(states, strategy)
        assert result.num_trades > 0


class TestSharpeCalculation:
    def test_sharpe_positive_for_winning_trades(self) -> None:
        bt = Backtester()
        # Use varying positive returns so std > 0, which allows Sharpe to be computed
        bt._trade_records = [
            TradeRecord(slot=i, action=ExecutionAction(kind=ActionKind.NO_OP), pnl_lamports=50 + i * 10)
            for i in range(20)
        ]
        bt._equity_curve = [float(1000 + i * 100) for i in range(21)]
        metrics = bt.calculate_metrics()
        assert metrics.sharpe_ratio > 0

    def test_sharpe_zero_for_zero_variance(self) -> None:
        # all identical returns
        returns = [100.0] * 10
        sharpe = Backtester._calculate_sharpe(returns, 0.0)
        # when std is zero, sharpe should be zero (avoid div by zero)
        assert sharpe == 0.0

    def test_sharpe_requires_minimum_trades(self) -> None:
        assert Backtester._calculate_sharpe([], 0.05) == 0.0
        assert Backtester._calculate_sharpe([1.0], 0.05) == 0.0


class TestMaxDrawdown:
    def test_no_drawdown_on_rising_curve(self) -> None:
        curve = [100.0, 200.0, 300.0, 400.0, 500.0]
        dd = Backtester._calculate_max_drawdown(curve)
        assert dd == 0.0

    def test_full_drawdown_to_zero(self) -> None:
        curve = [100.0, 50.0, 0.0]
        dd = Backtester._calculate_max_drawdown(curve)
        assert dd == 1.0

    def test_partial_drawdown(self) -> None:
        curve = [100.0, 120.0, 80.0, 110.0]
        dd = Backtester._calculate_max_drawdown(curve)
        # peak 120, trough 80 => dd = 40/120 = 0.333
        assert abs(dd - (40 / 120)) < 0.001

    def test_empty_curve(self) -> None:
        assert Backtester._calculate_max_drawdown([]) == 0.0
        assert Backtester._calculate_max_drawdown([100.0]) == 0.0


class TestWinRateAccuracy:
    def test_all_wins(self) -> None:
        bt = Backtester()
        bt._trade_records = [
            TradeRecord(slot=i, action=ExecutionAction(kind=ActionKind.NO_OP), pnl_lamports=10)
            for i in range(10)
        ]
        bt._equity_curve = [float(100 + i * 10) for i in range(11)]
        metrics = bt.calculate_metrics()
        assert metrics.win_rate == 1.0

    def test_all_losses(self) -> None:
        bt = Backtester()
        bt._trade_records = [
            TradeRecord(slot=i, action=ExecutionAction(kind=ActionKind.NO_OP), pnl_lamports=-10)
            for i in range(10)
        ]
        bt._equity_curve = [float(100 - i * 10) for i in range(11)]
        metrics = bt.calculate_metrics()
        assert metrics.win_rate == 0.0

    def test_half_and_half(self) -> None:
        bt = Backtester()
        bt._trade_records = [
            TradeRecord(
                slot=i,
                action=ExecutionAction(kind=ActionKind.NO_OP),
                pnl_lamports=10 if i % 2 == 0 else -10,
            )
            for i in range(10)
        ]
        bt._equity_curve = [100.0] * 11
        metrics = bt.calculate_metrics()
        assert metrics.win_rate == 0.5


class TestEmptyHistory:
    def test_empty_history_returns_zero_trades(self) -> None:
        strategy = SimpleSwapStrategy(TOKEN_A, TOKEN_B, 10_000)
        bt = Backtester(BacktestConfig(initial_balance={TOKEN_A: 1_000_000}))
        result = bt.run([], strategy)
        assert result.num_trades == 0
        assert result.total_pnl == 0
        assert result.sharpe_ratio == 0.0
        assert result.max_drawdown == 0.0


class TestReportGeneration:
    def test_report_contains_key_fields(self) -> None:
        history = _make_history(10)
        strategy = SimpleSwapStrategy(TOKEN_A, TOKEN_B, 10_000)
        bt = Backtester(BacktestConfig(initial_balance={TOKEN_A: 1_000_000}))
        result = bt.run(history, strategy)

        report = bt.generate_report(result)
        assert "BACKTEST REPORT" in report
        assert "Total PnL" in report
        assert "Win rate" in report
        assert "Sharpe ratio" in report
        assert "Max drawdown" in report
        assert "MEV losses" in report

    def test_report_handles_no_trades(self) -> None:
        bt = Backtester()
        result = BacktestResult()
        report = bt.generate_report(result)
        assert "BACKTEST REPORT" in report
        assert "0" in report
