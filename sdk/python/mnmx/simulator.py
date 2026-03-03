"""Route simulation engine for the MNMX SDK.

Simulates routes under varying market conditions including Monte Carlo
analysis and adversarial stress testing.
"""

from __future__ import annotations

import random
from dataclasses import dataclass, field
from typing import Sequence

from mnmx.exceptions import SimulationError
from mnmx.math_utils import (
    clamp,
    compute_mean,
    compute_median,
    compute_percentile,
    compute_std_dev,
)
from mnmx.types import (
    AdversarialModel,
    MonteCarloResult,
    Route,
    RouteHop,
    SimulationResult,
)


@dataclass
class SimulationConditions:
    """Market conditions applied during simulation."""

    slippage_multiplier: float = 1.0
    gas_multiplier: float = 1.0
    bridge_delay_multiplier: float = 1.0
    mev_extraction: float = 0.0
    price_movement: float = 0.0
    liquidity_factor: float = 1.0  # 1.0 = normal, <1 = reduced

    def describe(self) -> str:
        parts: list[str] = []
        if self.slippage_multiplier != 1.0:
            parts.append(f"slippage x{self.slippage_multiplier:.2f}")
        if self.gas_multiplier != 1.0:
            parts.append(f"gas x{self.gas_multiplier:.2f}")
        if self.bridge_delay_multiplier != 1.0:
            parts.append(f"delay x{self.bridge_delay_multiplier:.2f}")
        if self.mev_extraction > 0:
            parts.append(f"mev {self.mev_extraction:.4f}")
        if self.price_movement != 0:
            parts.append(f"price {self.price_movement:+.4f}")
        if self.liquidity_factor != 1.0:
            parts.append(f"liq x{self.liquidity_factor:.2f}")
        return ", ".join(parts) if parts else "baseline"


# Pre-defined stress scenarios
STRESS_SCENARIOS: list[SimulationConditions] = [
    # Normal market
    SimulationConditions(),
    # High gas
    SimulationConditions(gas_multiplier=3.0),
    # Flash crash
    SimulationConditions(price_movement=0.05, slippage_multiplier=3.0, liquidity_factor=0.3),
    # MEV attack
    SimulationConditions(mev_extraction=0.015, slippage_multiplier=2.0),
    # Bridge congestion
    SimulationConditions(bridge_delay_multiplier=5.0, gas_multiplier=2.0),
    # Low liquidity
    SimulationConditions(liquidity_factor=0.1, slippage_multiplier=4.0),
    # Moderate adversarial
    SimulationConditions(
        slippage_multiplier=1.5,
        gas_multiplier=1.5,
        bridge_delay_multiplier=2.0,
        mev_extraction=0.003,
        price_movement=0.01,
    ),
    # Extreme adversarial
    SimulationConditions(
        slippage_multiplier=4.0,
        gas_multiplier=4.0,
        bridge_delay_multiplier=6.0,
        mev_extraction=0.02,
        price_movement=0.08,
        liquidity_factor=0.15,
    ),
]


class RouteSimulator:
    """Simulates routes under various market conditions."""

