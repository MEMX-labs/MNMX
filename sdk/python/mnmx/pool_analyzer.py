"""
Pool analysis utilities for AMM liquidity pools.

Provides TVL calculation, depth analysis, swap estimation, and
multi-pool arbitrage detection.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any

from mnmx.client import MnmxClient
from mnmx.math_utils import (
    constant_product_output,
    constant_product_input,
    calculate_price_impact,
    bps_to_decimal,
)
from mnmx.types import PoolState


# ---------------------------------------------------------------------------
# Data classes
# ---------------------------------------------------------------------------

@dataclass
class SwapEstimate:
    """Estimated output and costs for a potential swap."""
    amount_out: int = 0
    price_impact_bps: int = 0
    effective_price: float = 0.0
    fee_amount: int = 0
    minimum_received: int = 0  # after max slippage


@dataclass
class LiquidityDepth:
    """How much can be traded at various impact levels."""
    impact_bps: int = 0
    max_buy_amount: int = 0
    max_sell_amount: int = 0
    buy_depth_usd: float = 0.0
    sell_depth_usd: float = 0.0


@dataclass
class ArbitrageRoute:
    """A circular route through pools that may yield profit."""
    pools: list[PoolState] = field(default_factory=list)
    tokens: list[str] = field(default_factory=list)
    expected_profit_bps: int = 0
    optimal_amount: int = 0
    estimated_profit: int = 0


@dataclass
class PoolAnalysis:
    """Complete analysis of a single liquidity pool."""
    pool: PoolState
    tvl_usd: float = 0.0
    price_a_in_b: float = 0.0
    price_b_in_a: float = 0.0
    depth_levels: list[LiquidityDepth] = field(default_factory=list)
    fee_apr_estimate: float = 0.0
    volume_24h_estimate: float = 0.0
    imbalance_ratio: float = 0.0


# ---------------------------------------------------------------------------
# PoolAnalyzer
# ---------------------------------------------------------------------------

class PoolAnalyzer:
    """
    Analyzes AMM pools for liquidity depth, arbitrage opportunities,
    and swap estimation.
    """

    def __init__(self, client: MnmxClient | None = None) -> None:
        self.client = client

    async def analyze_pool(self, pool_address: str) -> PoolAnalysis:
        """
        Fetch a pool's state from the engine and run a full analysis.

        Requires a connected MnmxClient.
        """
        if self.client is None:
            raise RuntimeError("PoolAnalyzer requires an MnmxClient for remote analysis")

        pool = await self.client.get_pool_state(pool_address)
        prices: dict[str, float] = {}
        # best-effort price fetch
        try:
            balances_a = await self.client.get_token_balances(pool.token_a_mint)
            balances_b = await self.client.get_token_balances(pool.token_b_mint)
        except Exception:
            pass

        return self.analyze_pool_local(pool, prices)

    def analyze_pool_local(
        self,
        pool: PoolState,
        prices: dict[str, float] | None = None,
    ) -> PoolAnalysis:
        """Run analysis on a local PoolState without an API call."""
        prices = prices or {}
        tvl = self.calculate_tvl(pool, prices)

        depth_levels = [
            self.calculate_depth(pool, bps, prices)
            for bps in [10, 50, 100, 200, 500]
        ]
