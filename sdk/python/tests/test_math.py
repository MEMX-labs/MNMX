"""
Tests for MNMX math utilities.
"""

from __future__ import annotations

import math

import pytest

from mnmx.math_utils import (
    bps_to_decimal,
    calculate_price_impact,
    calculate_slippage,
    clamp,
    concentrated_liquidity_swap,
    constant_product_input,
    constant_product_output,
    ewma,
    geometric_mean,
    isqrt,
    logistic,
    optimal_split,
    sqrt_price_to_price,
    weighted_average,
)
from mnmx.types import PoolState


POOL_ADDR = "A" * 44
TOKEN_A = "SoLMint111111111111111111111111111111111111"
TOKEN_B = "USDCMint11111111111111111111111111111111111"


def _pool(ra: int, rb: int, fee: int = 30) -> PoolState:
    return PoolState(
        address=POOL_ADDR,
        token_a_mint=TOKEN_A,
        token_b_mint=TOKEN_B,
        reserve_a=ra,
        reserve_b=rb,
        fee_bps=fee,
    )


class TestConstantProductOutput:
    def test_basic_output(self) -> None:
        out = constant_product_output(10_000, 1_000_000, 500_000, 30)
        assert out > 0
        assert out < 10_000  # can't get more than input at 2:1 ratio

    def test_zero_amount(self) -> None:
        assert constant_product_output(0, 1_000_000, 500_000, 30) == 0

    def test_zero_reserves(self) -> None:
        assert constant_product_output(10_000, 0, 500_000, 30) == 0
        assert constant_product_output(10_000, 1_000_000, 0, 30) == 0

    def test_no_fee(self) -> None:
        with_fee = constant_product_output(10_000, 1_000_000, 500_000, 30)
        no_fee = constant_product_output(10_000, 1_000_000, 500_000, 0)
        assert no_fee > with_fee

    def test_higher_fee_less_output(self) -> None:
        low = constant_product_output(10_000, 1_000_000, 500_000, 10)
        high = constant_product_output(10_000, 1_000_000, 500_000, 100)
        assert low > high

    def test_preserves_k_invariant(self) -> None:
        ra, rb = 1_000_000, 500_000
        amount = 10_000
        out = constant_product_output(amount, ra, rb, 0)
        # after swap: new_ra * new_rb >= old k (fees go to LPs)
        new_k = (ra + amount) * (rb - out)
        assert new_k >= ra * rb


class TestConstantProductInputInverse:
    def test_inverse_relationship(self) -> None:
        ra, rb, fee = 1_000_000, 500_000, 30
        # get output for 10_000 input
        out = constant_product_output(10_000, ra, rb, fee)
        # calculate input needed for that output
        inp = constant_product_input(out, ra, rb, fee)
        # should be close to 10_000 (ceiling division adds 1)
        assert abs(inp - 10_000) <= 2

    def test_zero_amount(self) -> None:
        assert constant_product_input(0, 1_000_000, 500_000, 30) == 0

    def test_amount_exceeding_reserves(self) -> None:
        assert constant_product_input(600_000, 1_000_000, 500_000, 30) == 0

    def test_input_always_positive(self) -> None:
        inp = constant_product_input(1_000, 1_000_000, 500_000, 30)
        assert inp > 0


class TestPriceImpact:
    def test_zero_amount_zero_impact(self) -> None:
        assert calculate_price_impact(0, 1_000_000, 500_000) == 0.0

    def test_small_trade_small_impact(self) -> None:
        impact = calculate_price_impact(100, 1_000_000, 500_000)
        assert impact < 0.001

    def test_large_trade_large_impact(self) -> None:
        impact = calculate_price_impact(500_000, 1_000_000, 500_000)
        assert impact > 0.1

    def test_impact_bounded_zero_to_one(self) -> None:
        for amount in [1, 100, 10_000, 500_000, 999_999]:
            impact = calculate_price_impact(amount, 1_000_000, 500_000)
            assert 0.0 <= impact <= 1.0

    def test_impact_increases_monotonically(self) -> None:
        prev = 0.0
        for amount in [100, 1_000, 10_000, 100_000]:
            impact = calculate_price_impact(amount, 1_000_000, 500_000)
            assert impact >= prev
            prev = impact


class TestSlippage:
    def test_slippage_bounded(self) -> None:
        for amount in [1, 100, 10_000, 100_000]:
            slip = calculate_slippage(amount, 1_000_000, 500_000, 30)
            assert 0.0 <= slip <= 1.0

    def test_zero_amount_zero_slippage(self) -> None:
        assert calculate_slippage(0, 1_000_000, 500_000, 30) == 0.0

    def test_slippage_increases_with_fee(self) -> None:
        low = calculate_slippage(10_000, 1_000_000, 500_000, 10)
        high = calculate_slippage(10_000, 1_000_000, 500_000, 100)
        assert high > low


class TestOptimalSplit:
    def test_single_pool_gets_all(self) -> None:
        pools = [_pool(1_000_000, 500_000)]
        result = optimal_split(10_000, pools)
        assert result == [10_000]

    def test_multiple_pools_sum_to_total(self) -> None:
        pools = [_pool(1_000_000, 500_000), _pool(2_000_000, 1_000_000)]
        result = optimal_split(50_000, pools)
        assert sum(result) == 50_000

    def test_bigger_pool_gets_more(self) -> None:
        small = _pool(100_000, 50_000)
        big = _pool(10_000_000, 5_000_000)
        big_pool = PoolState(
            address="B" * 44,
            token_a_mint=TOKEN_A,
            token_b_mint=TOKEN_B,
            reserve_a=10_000_000,
            reserve_b=5_000_000,
            fee_bps=30,
        )
        result = optimal_split(100_000, [small, big_pool])
        assert result[1] > result[0]

    def test_empty_pools(self) -> None:
        assert optimal_split(10_000, []) == []


class TestIsqrt:
    def test_perfect_squares(self) -> None:
        assert isqrt(0) == 0
        assert isqrt(1) == 1
        assert isqrt(4) == 2
        assert isqrt(9) == 3
        assert isqrt(16) == 4
        assert isqrt(10000) == 100

    def test_non_perfect_squares(self) -> None:
        assert isqrt(2) == 1
        assert isqrt(3) == 1
        assert isqrt(5) == 2
        assert isqrt(8) == 2

    def test_large_number(self) -> None:
        n = 10**18
        root = isqrt(n)
        assert root * root <= n
        assert (root + 1) * (root + 1) > n

    def test_negative_raises(self) -> None:
        with pytest.raises(ValueError):
            isqrt(-1)


class TestEwma:
    def test_single_value(self) -> None:
        assert ewma([5.0], 0.5) == [5.0]

    def test_smoothing_effect(self) -> None:
        values = [0.0, 10.0, 0.0, 10.0, 0.0]
        smoothed = ewma(values, 0.3)
        # smoothed values should have less variance
        raw_range = max(values) - min(values)
        smoothed_range = max(smoothed) - min(smoothed)
        assert smoothed_range < raw_range

    def test_alpha_one_gives_original(self) -> None:
        values = [1.0, 2.0, 3.0, 4.0]
        result = ewma(values, 1.0)
        assert result == values

    def test_empty_input(self) -> None:
        assert ewma([], 0.5) == []

    def test_invalid_alpha_raises(self) -> None:
        with pytest.raises(ValueError):
            ewma([1.0], 0.0)
        with pytest.raises(ValueError):
            ewma([1.0], -0.1)


class TestHelpers:
    def test_bps_to_decimal(self) -> None:
        assert bps_to_decimal(100) == 0.01
        assert bps_to_decimal(30) == 0.003
        assert bps_to_decimal(10000) == 1.0

    def test_logistic(self) -> None:
        assert abs(logistic(0.0) - 0.5) < 1e-10
        assert logistic(100.0) > 0.99
        assert logistic(-100.0) < 0.01

    def test_weighted_average(self) -> None:
        assert weighted_average([(10.0, 1.0), (20.0, 1.0)]) == 15.0
        assert weighted_average([(10.0, 3.0), (20.0, 1.0)]) == 12.5
        assert weighted_average([]) == 0.0

    def test_geometric_mean(self) -> None:
        result = geometric_mean([2.0, 8.0])
        assert abs(result - 4.0) < 0.001

    def test_clamp(self) -> None:
        assert clamp(5.0, 0.0, 10.0) == 5.0
        assert clamp(-1.0, 0.0, 10.0) == 0.0
        assert clamp(15.0, 0.0, 10.0) == 10.0
