"""
Tests for the MNMX PoolAnalyzer.
"""

from __future__ import annotations

import pytest

from mnmx.pool_analyzer import (
    ArbitrageRoute,
    LiquidityDepth,
    PoolAnalyzer,
    PoolAnalysis,
    SwapEstimate,
)
from mnmx.types import PoolState


POOL_A_ADDR = "A" * 44
POOL_B_ADDR = "B" * 44
TOKEN_SOL = "SoLMint111111111111111111111111111111111111"
TOKEN_USDC = "USDCMint11111111111111111111111111111111111"
TOKEN_USDT = "USDTMint11111111111111111111111111111111111"


def _pool(
    addr: str,
    ta: str,
    tb: str,
    ra: int,
    rb: int,
    fee: int = 30,
) -> PoolState:
    return PoolState(
        address=addr,
        token_a_mint=ta,
        token_b_mint=tb,
        reserve_a=ra,
        reserve_b=rb,
        fee_bps=fee,
    )


class TestTvlCalculation:
    def test_tvl_with_both_prices(self) -> None:
        pool = _pool(POOL_A_ADDR, TOKEN_SOL, TOKEN_USDC, 1_000, 100_000)
        analyzer = PoolAnalyzer()
        tvl = analyzer.calculate_tvl(pool, {TOKEN_SOL: 100.0, TOKEN_USDC: 1.0})
        assert tvl == 1_000 * 100.0 + 100_000 * 1.0  # 200_000

    def test_tvl_with_one_price_inferred(self) -> None:
        pool = _pool(POOL_A_ADDR, TOKEN_SOL, TOKEN_USDC, 1_000, 100_000)
        analyzer = PoolAnalyzer()
        tvl = analyzer.calculate_tvl(pool, {TOKEN_SOL: 100.0})
        # reserve_a * price_a = 100_000 for one side
        # inferred price_b = (1000 * 100) / 100_000 = 1.0
        # tvl = 100_000 + 100_000 = 200_000
        assert tvl == 200_000.0

    def test_tvl_with_no_prices(self) -> None:
        pool = _pool(POOL_A_ADDR, TOKEN_SOL, TOKEN_USDC, 1_000, 100_000)
        analyzer = PoolAnalyzer()
        tvl = analyzer.calculate_tvl(pool, {})
        assert tvl == 101_000.0  # fallback: raw sum

    def test_tvl_zero_reserves(self) -> None:
        pool = _pool(POOL_A_ADDR, TOKEN_SOL, TOKEN_USDC, 0, 0)
        analyzer = PoolAnalyzer()
        tvl = analyzer.calculate_tvl(pool, {TOKEN_SOL: 100.0})
        assert tvl == 0.0


class TestDepthAtVariousImpacts:
    def test_higher_impact_allows_more_trade(self) -> None:
        pool = _pool(POOL_A_ADDR, TOKEN_SOL, TOKEN_USDC, 1_000_000, 500_000)
        analyzer = PoolAnalyzer()

        depth_10 = analyzer.calculate_depth(pool, 10)
        depth_100 = analyzer.calculate_depth(pool, 100)
        depth_500 = analyzer.calculate_depth(pool, 500)

        assert depth_100.max_buy_amount >= depth_10.max_buy_amount
        assert depth_500.max_buy_amount >= depth_100.max_buy_amount

    def test_zero_impact_gives_zero_depth(self) -> None:
        pool = _pool(POOL_A_ADDR, TOKEN_SOL, TOKEN_USDC, 1_000_000, 500_000)
        analyzer = PoolAnalyzer()
        depth = analyzer.calculate_depth(pool, 0)
        assert depth.max_buy_amount == 0

    def test_depth_with_prices(self) -> None:
        pool = _pool(POOL_A_ADDR, TOKEN_SOL, TOKEN_USDC, 1_000_000, 500_000)
        analyzer = PoolAnalyzer()
        depth = analyzer.calculate_depth(pool, 100, {TOKEN_SOL: 100.0, TOKEN_USDC: 1.0})
        assert depth.buy_depth_usd > 0


class TestSwapEstimate:
    def test_basic_estimate(self) -> None:
        pool = _pool(POOL_A_ADDR, TOKEN_SOL, TOKEN_USDC, 1_000_000, 500_000)
        analyzer = PoolAnalyzer()
        est = analyzer.estimate_swap_output(pool, 10_000, TOKEN_SOL)

        assert est.amount_out > 0
        assert est.amount_out < 10_000  # 2:1 ratio, with fees
        assert est.price_impact_bps >= 0
        assert est.fee_amount > 0
        assert est.minimum_received <= est.amount_out

    def test_reverse_direction(self) -> None:
        pool = _pool(POOL_A_ADDR, TOKEN_SOL, TOKEN_USDC, 1_000_000, 500_000)
        analyzer = PoolAnalyzer()
        est = analyzer.estimate_swap_output(pool, 5_000, TOKEN_USDC)
        assert est.amount_out > 0

    def test_large_trade_high_impact(self) -> None:
        pool = _pool(POOL_A_ADDR, TOKEN_SOL, TOKEN_USDC, 100_000, 50_000)
        analyzer = PoolAnalyzer()
        est = analyzer.estimate_swap_output(pool, 50_000, TOKEN_SOL)
        assert est.price_impact_bps > 100


class TestArbitrageDetection:
    def test_finds_arbitrage_between_unbalanced_pools(self) -> None:
        # pool1: 1 SOL = 100 USDC (fair price)
        pool1 = _pool(POOL_A_ADDR, TOKEN_SOL, TOKEN_USDC, 10_000, 1_000_000)
        # pool2: 1 SOL = 50 USDC (cheap SOL)
        pool2 = _pool(POOL_B_ADDR, TOKEN_SOL, TOKEN_USDC, 20_000, 1_000_000)

        analyzer = PoolAnalyzer()
        routes = analyzer.find_arbitrage([pool1, pool2])

        # there should be at least one profitable route
        assert len(routes) >= 1
        best = routes[0]
        assert best.estimated_profit > 0

    def test_no_arbitrage_balanced_pools(self) -> None:
        # identical pools => no arb
        pool1 = _pool(POOL_A_ADDR, TOKEN_SOL, TOKEN_USDC, 1_000_000, 500_000)
        pool2 = _pool(POOL_B_ADDR, TOKEN_SOL, TOKEN_USDC, 1_000_000, 500_000)

        analyzer = PoolAnalyzer()
        routes = analyzer.find_arbitrage([pool1, pool2])

        # even if detected, profit should be zero or negative after fees
        for route in routes:
            assert route.estimated_profit <= 0

    def test_empty_pools_list(self) -> None:
        analyzer = PoolAnalyzer()
        routes = analyzer.find_arbitrage([])
        assert routes == []

    def test_single_pool_no_arb(self) -> None:
        pool = _pool(POOL_A_ADDR, TOKEN_SOL, TOKEN_USDC, 1_000_000, 500_000)
        analyzer = PoolAnalyzer()
        routes = analyzer.find_arbitrage([pool])
        assert routes == []


class TestOptimalArbitrageAmount:
    def test_optimal_amount_positive(self) -> None:
        pool1 = _pool(POOL_A_ADDR, TOKEN_SOL, TOKEN_USDC, 10_000, 1_000_000)
        pool2 = _pool(POOL_B_ADDR, TOKEN_SOL, TOKEN_USDC, 20_000, 1_000_000)

        analyzer = PoolAnalyzer()
        routes = analyzer.find_arbitrage([pool1, pool2])

        if routes:
            best = routes[0]
            assert best.optimal_amount > 0
            # verify the optimal amount actually gives the best profit
            profit_at_optimal = analyzer.calculate_route_profit(best, best.optimal_amount)
            profit_at_half = analyzer.calculate_route_profit(best, best.optimal_amount // 2)
            assert profit_at_optimal >= profit_at_half

    def test_route_profit_calculation(self) -> None:
        pool1 = _pool(POOL_A_ADDR, TOKEN_SOL, TOKEN_USDC, 10_000, 1_000_000)
        pool2 = _pool(POOL_B_ADDR, TOKEN_SOL, TOKEN_USDC, 20_000, 1_000_000)
        route = ArbitrageRoute(
            pools=[pool1, pool2],
            tokens=[TOKEN_SOL, TOKEN_USDC, TOKEN_SOL],
        )
        analyzer = PoolAnalyzer()
        profit = analyzer.calculate_route_profit(route, 100)
        # should be calculable (positive or negative)
        assert isinstance(profit, int)


class TestPoolAnalysisLocal:
    def test_full_analysis(self) -> None:
        pool = _pool(POOL_A_ADDR, TOKEN_SOL, TOKEN_USDC, 1_000_000, 500_000)
        analyzer = PoolAnalyzer()
        analysis = analyzer.analyze_pool_local(pool, {TOKEN_SOL: 100.0, TOKEN_USDC: 1.0})

        assert isinstance(analysis, PoolAnalysis)
        assert analysis.tvl_usd > 0
        assert analysis.price_a_in_b > 0
        assert analysis.price_b_in_a > 0
        assert len(analysis.depth_levels) == 5
        assert analysis.fee_apr_estimate > 0
        assert 0.0 <= analysis.imbalance_ratio <= 1.0

    def test_imbalance_zero_for_equal_reserves(self) -> None:
        pool = _pool(POOL_A_ADDR, TOKEN_SOL, TOKEN_USDC, 1_000_000, 1_000_000)
        analyzer = PoolAnalyzer()
        analysis = analyzer.analyze_pool_local(pool)
        assert analysis.imbalance_ratio == 0.0
