"""
Tests for the MNMX Simulator.
"""

from __future__ import annotations

import pytest

from mnmx.exceptions import InsufficientLiquidityError
from mnmx.math_utils import constant_product_output
from mnmx.simulator import Simulator, MonteCarloResult
from mnmx.types import (
    ActionKind,
    ExecutionAction,
    MevKind,
    MevThreat,
    OnChainState,
    PoolState,
    SimulationConfig,
    SimulationResult,
)


POOL_ADDR = "A" * 44
TOKEN_A = "SoLMint111111111111111111111111111111111111"
TOKEN_B = "USDCMint11111111111111111111111111111111111"


def _make_pool(reserve_a: int = 1_000_000, reserve_b: int = 500_000, fee: int = 30) -> PoolState:
    return PoolState(
        address=POOL_ADDR,
        token_a_mint=TOKEN_A,
        token_b_mint=TOKEN_B,
        reserve_a=reserve_a,
        reserve_b=reserve_b,
        fee_bps=fee,
    )


def _make_state(pool: PoolState | None = None) -> OnChainState:
    pool = pool or _make_pool()
    return OnChainState(
        slot=100,
        pools=[pool],
        balances={TOKEN_A: 10_000_000, TOKEN_B: 5_000_000},
    )


def _make_swap(amount: int = 10_000, min_out: int = 0) -> ExecutionAction:
    return ExecutionAction(
        kind=ActionKind.SWAP,
        pool_address=POOL_ADDR,
        token_in=TOKEN_A,
        token_out=TOKEN_B,
        amount_in=amount,
        min_amount_out=min_out,
    )


class TestConstantProductSwap:
    def test_basic_swap_produces_output(self) -> None:
        sim = Simulator()
        state = _make_state()
        action = _make_swap(10_000)
        result = sim.simulate_swap(state, action)

        assert result.success is True
        assert result.amount_out > 0
        assert result.amount_out < 10_000  # must be less due to fees/impact

    def test_swap_output_matches_math(self) -> None:
        pool = _make_pool()
        expected = constant_product_output(10_000, pool.reserve_a, pool.reserve_b, pool.fee_bps)

        sim = Simulator()
        result = sim.simulate_swap(_make_state(pool), _make_swap(10_000))

        assert result.amount_out == expected

    def test_larger_swap_has_more_impact(self) -> None:
        sim = Simulator()
        state = _make_state()

        small = sim.simulate_swap(state, _make_swap(1_000))
        large = sim.simulate_swap(state, _make_swap(100_000))

        assert large.price_impact_bps > small.price_impact_bps

    def test_tiny_amount_may_produce_zero_output(self) -> None:
        sim = Simulator()
        result = sim.simulate_action(
            _make_state(),
            ExecutionAction(
                kind=ActionKind.SWAP,
                pool_address=POOL_ADDR,
                token_in=TOKEN_A,
                token_out=TOKEN_B,
                amount_in=1,  # too small for integer math to yield output
                min_amount_out=0,
            ),
        )
        # amount_in=1 with fee produces 0 output via integer division
        assert result.success is False
        assert "zero output" in (result.error or "").lower()

    def test_swap_below_min_out_fails(self) -> None:
        sim = Simulator()
        result = sim.simulate_swap(
            _make_state(), _make_swap(10_000, min_out=999_999_999)
        )
        assert result.success is False
        assert "below minimum" in (result.error or "").lower()


class TestSlippageCalculation:
    def test_slippage_increases_with_size(self) -> None:
        sim = Simulator()
        state = _make_state()

        r1 = sim.simulate_swap(state, _make_swap(1_000))
        r2 = sim.simulate_swap(state, _make_swap(100_000))

        assert r2.slippage_bps > r1.slippage_bps

    def test_slippage_is_non_negative(self) -> None:
        sim = Simulator()
        result = sim.simulate_swap(_make_state(), _make_swap(5_000))
        assert result.slippage_bps >= 0

