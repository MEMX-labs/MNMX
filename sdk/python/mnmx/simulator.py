"""
Local simulator for MNMX actions.

Runs constant-product AMM simulations, MEV attack modelling, and
Monte Carlo analysis entirely offline — no engine connection required.
"""

from __future__ import annotations

import copy
import math
import random
from dataclasses import dataclass, field
from typing import Any

from mnmx.exceptions import InsufficientLiquidityError, SimulationError
from mnmx.math_utils import (
    bps_to_decimal,
    calculate_price_impact,
    calculate_slippage,
    constant_product_output,
)
from mnmx.types import (
    ActionKind,
    ExecutionAction,
    MevKind,
    MevThreat,
    OnChainState,
    PendingTx,
    PoolState,
    SimulationConfig,
    SimulationResult,
)


@dataclass
class MonteCarloResult:
    """Aggregated results from a Monte Carlo simulation run."""

    mean_output: float = 0.0
    std_output: float = 0.0
    percentile_5: float = 0.0
    percentile_25: float = 0.0
    percentile_50: float = 0.0
    percentile_75: float = 0.0
    percentile_95: float = 0.0
    worst_case: float = 0.0
    best_case: float = 0.0
    mev_attack_probability: float = 0.0
    avg_mev_loss: float = 0.0
    iterations: int = 0
    raw_outputs: list[float] = field(default_factory=list)


class Simulator:
    """
    Offline action simulator using constant-product AMM math.

    Supports swap simulation, MEV attack modelling, and Monte Carlo
    analysis with configurable parameters.
    """

    def __init__(self, config: SimulationConfig | None = None) -> None:
        self.config = config or SimulationConfig()
        self._rng = random.Random(42)

    def seed(self, seed: int) -> None:
        """Reseed the random number generator for reproducible results."""
        self._rng = random.Random(seed)

    # -- primary API --------------------------------------------------------

    def simulate_action(
        self, state: OnChainState, action: ExecutionAction
    ) -> SimulationResult:
        """Simulate any supported action type against the given state."""
        if action.kind == ActionKind.SWAP:
            return self.simulate_swap(state, action)
        if action.kind == ActionKind.ADD_LIQUIDITY:
            return self._simulate_add_liquidity(state, action)
        if action.kind == ActionKind.REMOVE_LIQUIDITY:
            return self._simulate_remove_liquidity(state, action)
        if action.kind == ActionKind.NO_OP:
            return SimulationResult(
                success=True,
                amount_out=0,
                gas_cost_lamports=0,
            )
        return SimulationResult(
            success=False,
            error=f"Unsupported action kind: {action.kind.value}",
        )

    def simulate_swap(
        self, state: OnChainState, action: ExecutionAction
    ) -> SimulationResult:
        """Simulate a token swap on a constant-product pool."""
        pool = state.get_pool(action.pool_address)
        if pool is None:
            return SimulationResult(
                success=False,
                error=f"Pool {action.pool_address} not found in state",
            )

        reserve_in, reserve_out = self._resolve_reserves(pool, action.token_in)
        if reserve_in == 0 or reserve_out == 0:
            raise InsufficientLiquidityError(
                pool_address=action.pool_address,
                available=0,
                requested=action.amount_in,
            )

        amount_out = constant_product_output(
            action.amount_in, reserve_in, reserve_out, pool.fee_bps
        )
