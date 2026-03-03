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

    def __init__(self, adversarial_model: AdversarialModel | None = None) -> None:
        self._adversarial = adversarial_model or AdversarialModel()

    def simulate(
        self,
        route: Route,
        conditions: SimulationConditions | None = None,
    ) -> SimulationResult:
        """Simulate a route under specific market conditions.

        If no conditions are given, the simulator uses the configured
        adversarial model defaults.
        """
        if not route.path:
            raise SimulationError("Route has no hops")

        cond = conditions or SimulationConditions(
            slippage_multiplier=self._adversarial.slippage_multiplier,
            gas_multiplier=self._adversarial.gas_multiplier,
            bridge_delay_multiplier=self._adversarial.bridge_delay_multiplier,
            mev_extraction=self._adversarial.mev_extraction,
            price_movement=self._adversarial.price_movement,
        )

        return self._compute_output(route, cond)

    def monte_carlo(
        self,
        route: Route,
        iterations: int = 10000,
        seed: int | None = None,
    ) -> MonteCarloResult:
        """Run Monte Carlo simulation over random market conditions.

        Each iteration samples conditions from distributions centered
        on the adversarial model defaults.
        """
        if not route.path:
            raise SimulationError("Route has no hops")
        if iterations < 1:
            raise SimulationError("iterations must be >= 1")

        rng = random.Random(seed)
        outputs: list[float] = []

        for _ in range(iterations):
            cond = self._random_conditions(rng)
            result = self._compute_output(route, cond)
            outputs.append(result.output)

        return MonteCarloResult(
            mean_output=compute_mean(outputs),
            median_output=compute_median(outputs),
            std_dev=compute_std_dev(outputs),
            percentile_5=compute_percentile(outputs, 5.0),
            percentile_95=compute_percentile(outputs, 95.0),
            min_output=min(outputs),
            max_output=max(outputs),
            iterations=iterations,
        )

    def stress_test(
        self,
        route: Route,
        scenarios: list[SimulationConditions] | None = None,
    ) -> list[SimulationResult]:
        """Run the route through a series of stress scenarios."""
        if not route.path:
            raise SimulationError("Route has no hops")
        chosen = scenarios if scenarios is not None else STRESS_SCENARIOS
        results: list[SimulationResult] = []
        for scenario in chosen:
            results.append(self._compute_output(route, scenario))
        return results

    # ---- internals ---------------------------------------------------------

    def _compute_output(
        self,
        route: Route,
        conditions: SimulationConditions,
    ) -> SimulationResult:
        """Walk through every hop, applying conditions at each step."""
        current_amount = route.path[0].input_amount
        total_fees = 0.0
        total_time = 0
        total_mev = 0.0

        for hop in route.path:
            # scale the amount proportionally if it differs from the hop's recorded input
            ratio = current_amount / hop.input_amount if hop.input_amount > 0 else 1.0

            # fee scaled by gas multiplier
            fee = hop.fee * ratio * conditions.gas_multiplier
