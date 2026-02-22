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

                min_out = constant_product_output(
                    self.amount, reserve_in, reserve_out, pool.fee_bps
                )
                # allow 1% slippage from simulated output
                min_out = int(min_out * 0.99)

                return ExecutionAction(
                    kind=ActionKind.SWAP,
                    pool_address=pool.address,
                    token_in=self.token_in,
                    token_out=self.token_out,
                    amount_in=self.amount,
                    min_amount_out=min_out,
                )
        return None


class MevAwareStrategy(Strategy):
    """
    Strategy that avoids trading when MEV risk is elevated.

    Monitors pending transactions and skips trades when sandwich or
    frontrun threats are detected in the mempool.
    """

    def __init__(
        self,
        token_in: str,
        token_out: str,
        amount: int,
        max_impact_bps: int = 100,
        max_mev_risk: float = 0.3,
    ) -> None:
        self.token_in = token_in
        self.token_out = token_out
        self.amount = amount
        self.max_impact_bps = max_impact_bps
        self.max_mev_risk = max_mev_risk
        self._consecutive_skips = 0
        self._max_consecutive_skips = 5

    def decide(self, state: OnChainState) -> ExecutionAction | None:
        balance = state.balances.get(self.token_in, 0)
        if balance < self.amount:
            return None

        for pool in state.pools:
            tokens = {pool.token_a_mint, pool.token_b_mint}
            if self.token_in not in tokens or self.token_out not in tokens:
                continue

            if pool.token_a_mint == self.token_in:
                reserve_in, reserve_out = pool.reserve_a, pool.reserve_b
            else:
                reserve_in, reserve_out = pool.reserve_b, pool.reserve_a

            if reserve_in == 0 or reserve_out == 0:
                continue

            # check MEV risk from pending txs
            competing = sum(
                1 for tx in state.pending_txs
                if tx.action.pool_address == pool.address
            )
            size_ratio = self.amount / reserve_in if reserve_in > 0 else 1.0
            mev_risk = min(1.0, size_ratio * 5.0 + competing * 0.1)

            # allow override after too many skips
            if mev_risk > self.max_mev_risk and self._consecutive_skips < self._max_consecutive_skips:
                self._consecutive_skips += 1
                continue

            from mnmx.math_utils import calculate_price_impact
            impact = calculate_price_impact(self.amount, reserve_in, reserve_out)
            if impact * 10_000 > self.max_impact_bps:
                continue

            min_out = constant_product_output(
                self.amount, reserve_in, reserve_out, pool.fee_bps
            )
            min_out = int(min_out * 0.99)

            self._consecutive_skips = 0
            return ExecutionAction(
                kind=ActionKind.SWAP,
                pool_address=pool.address,
                token_in=self.token_in,
                token_out=self.token_out,
                amount_in=self.amount,
                min_amount_out=min_out,
                priority_fee_lamports=10_000 if mev_risk > 0.2 else 5000,
            )

        return None


# ---------------------------------------------------------------------------
# Backtest metrics
# ---------------------------------------------------------------------------

@dataclass
class BacktestMetrics:
    """Summary performance metrics from a backtest."""
    total_pnl: int = 0
    win_rate: float = 0.0
    sharpe_ratio: float = 0.0
    max_drawdown: float = 0.0
    avg_slippage_bps: float = 0.0
    mev_losses: int = 0
    total_gas: int = 0
    num_trades: int = 0
    profit_factor: float = 0.0


# ---------------------------------------------------------------------------
# Backtester
# ---------------------------------------------------------------------------

class Backtester:
    """
    Replay historical states through a strategy, record outcomes, and
    produce aggregate metrics.
    """

    def __init__(self, config: BacktestConfig | None = None) -> None:
        self.config = config or BacktestConfig()
        self._simulator = Simulator(
            SimulationConfig(
                fee_bps=self.config.fee_bps,
                include_mev_simulation=self.config.include_mev,
            )
        )
        self._trade_records: list[TradeRecord] = []
        self._equity_curve: list[float] = []

    def run(
        self,
        historical_states: list[OnChainState],
        strategy: Strategy,
    ) -> BacktestResult:
        """
        Execute a full backtest across a series of historical states.

        Each state represents a single slot / time step. The strategy is
        queried at each step and any resulting action is simulated.
        """
        self._trade_records = []
        self._equity_curve = []

        # initialise portfolio value
        balances = dict(self.config.initial_balance)
        cumulative_value = float(sum(balances.values()))
        self._equity_curve.append(cumulative_value)

        for state in historical_states:
            # inject current balances into state for strategy visibility
            state_with_balances = state.model_copy(
                update={"balances": dict(balances)}
            )

            action = self.evaluate_strategy(state_with_balances, strategy)
            if action is None:
                self._equity_curve.append(cumulative_value)
                continue

            result = self._simulator.simulate_action(state_with_balances, action)

            # calculate MEV loss by comparing to clean output
            mev_loss = 0
            if self.config.include_mev and result.mev_risk > 0.3:
                mev_loss = int(result.amount_out * result.mev_risk * 0.02)

            actual_output = max(0, result.amount_out - mev_loss)
            pnl = actual_output - action.amount_in  # simplified PnL

            record = TradeRecord(
                slot=state.slot,
                action=action,
                amount_out=actual_output,
                pnl_lamports=pnl,
                gas_cost=result.gas_cost_lamports,
                mev_loss=mev_loss,
                slippage_bps=result.slippage_bps,
            )

            self.record_result(record, strategy)

            # update balances
            if result.success:
                balances[action.token_in] = max(
                    0, balances.get(action.token_in, 0) - action.amount_in
                )
                balances[action.token_out] = (
                    balances.get(action.token_out, 0) + actual_output
                )

            cumulative_value = float(sum(balances.values()))
            self._equity_curve.append(cumulative_value)

        metrics = self.calculate_metrics()

        start_slot = historical_states[0].slot if historical_states else 0
        end_slot = historical_states[-1].slot if historical_states else 0

        return BacktestResult(
            trades=self._trade_records,
            total_pnl=metrics.total_pnl,
            win_rate=metrics.win_rate,
            sharpe_ratio=metrics.sharpe_ratio,
            max_drawdown=metrics.max_drawdown,
            avg_slippage_bps=metrics.avg_slippage_bps,
            total_mev_losses=metrics.mev_losses,
            total_gas_costs=metrics.total_gas,
            equity_curve=self._equity_curve,
            num_trades=metrics.num_trades,
            start_slot=start_slot,
            end_slot=end_slot,
            duration_slots=max(0, end_slot - start_slot),
        )
