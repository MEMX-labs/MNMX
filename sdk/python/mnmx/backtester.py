"""
Backtesting framework for MNMX trading strategies.

Replays historical on-chain states through a strategy, recording trades,
computing PnL, and producing risk-adjusted performance metrics.
"""

from __future__ import annotations

import math
from abc import ABC, abstractmethod
from dataclasses import dataclass, field
from typing import Any

from mnmx.math_utils import constant_product_output, calculate_slippage
from mnmx.simulator import Simulator
from mnmx.types import (
    ActionKind,
    BacktestConfig,
    BacktestResult,
    ExecutionAction,
    MevKind,
    MevThreat,
    OnChainState,
    SimulationConfig,
    SimulationResult,
    TradeRecord,
)


# ---------------------------------------------------------------------------
# Strategy protocol
# ---------------------------------------------------------------------------

class Strategy(ABC):
    """Abstract base class for backtestable trading strategies."""

    @abstractmethod
    def decide(self, state: OnChainState) -> ExecutionAction | None:
        """
        Examine current on-chain state and decide on an action.

        Return None to skip this slot (no trade).
        """
        ...

    def on_trade_result(self, record: TradeRecord) -> None:
        """Optional callback after each trade is recorded."""


# ---------------------------------------------------------------------------
# Built-in strategies
# ---------------------------------------------------------------------------

class SimpleSwapStrategy(Strategy):
    """
    Naive strategy: swap a fixed amount on the first available pool
    whenever the price impact is below a threshold.
    """

    def __init__(
        self,
        token_in: str,
        token_out: str,
        amount: int,
        max_impact_bps: int = 100,
    ) -> None:
        self.token_in = token_in
        self.token_out = token_out
        self.amount = amount
        self.max_impact_bps = max_impact_bps

    def decide(self, state: OnChainState) -> ExecutionAction | None:
        balance = state.balances.get(self.token_in, 0)
        if balance < self.amount:
            return None

        for pool in state.pools:
            tokens = {pool.token_a_mint, pool.token_b_mint}
            if self.token_in in tokens and self.token_out in tokens:
                if pool.token_a_mint == self.token_in:
                    reserve_in, reserve_out = pool.reserve_a, pool.reserve_b
                else:
                    reserve_in, reserve_out = pool.reserve_b, pool.reserve_a

                if reserve_in == 0 or reserve_out == 0:
                    continue

                from mnmx.math_utils import calculate_price_impact
                impact = calculate_price_impact(self.amount, reserve_in, reserve_out)
                if impact * 10_000 > self.max_impact_bps:
                    continue
