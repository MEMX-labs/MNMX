"""Route scoring engine for the MNMX SDK.

Scores routes across multiple dimensions (fees, slippage, speed,
reliability, MEV exposure) using configurable weights.
"""

from __future__ import annotations

from mnmx.math_utils import clamp, normalize_to_range, safe_divide, weighted_average
from mnmx.types import Route, RouteHop, ScoringWeights, Strategy


# Upper bounds used for normalization.  Values above these receive a score of 0.
_MAX_FEE_RATIO = 0.10  # 10% of amount
_MAX_SLIPPAGE = 0.05  # 5%
_MAX_TIME_SECONDS = 1800  # 30 minutes
_MIN_RELIABILITY = 0.80  # below this -> score 0
_MAX_MEV_RATIO = 0.03  # 3%


STRATEGY_WEIGHTS: dict[str, ScoringWeights] = {
    "minimax": ScoringWeights(
        fees=0.20,
        slippage=0.30,
        speed=0.15,
        reliability=0.25,
        mev_exposure=0.10,
    ),
    "maximin": ScoringWeights(
        fees=0.15,
        slippage=0.35,
        speed=0.10,
        reliability=0.30,
        mev_exposure=0.10,
    ),
    "balanced": ScoringWeights(
        fees=0.20,
        slippage=0.20,
        speed=0.20,
        reliability=0.20,
        mev_exposure=0.20,
    ),
    "aggressive": ScoringWeights(
        fees=0.35,
        slippage=0.10,
        speed=0.30,
        reliability=0.15,
        mev_exposure=0.10,
    ),
    "conservative": ScoringWeights(
        fees=0.10,
        slippage=0.30,
        speed=0.05,
        reliability=0.40,
        mev_exposure=0.15,
    ),
}


def get_strategy_weights(strategy: Strategy) -> ScoringWeights:
    """Return the canonical scoring weights for a strategy name."""
    if strategy in STRATEGY_WEIGHTS:
        return STRATEGY_WEIGHTS[strategy]
    return STRATEGY_WEIGHTS["balanced"]


class RouteScorer:
    """Scores routes and individual hops using weighted multi-dimensional analysis."""

    def __init__(self, default_weights: ScoringWeights | None = None) -> None:
        self._default_weights = default_weights or ScoringWeights()

    # ---- public API --------------------------------------------------------

    def score_route(self, route: Route, weights: ScoringWeights | None = None) -> float:
        """Compute a composite score in [0, 1] for *route*.  Higher is better."""
        w = (weights or self._default_weights).normalized()
        if not route.path:
            return 0.0

        initial_amount = route.path[0].input_amount
        if initial_amount == 0:
            return 0.0

        fee_score = self.normalize_fee(route.total_fees, initial_amount)
        slip_score = self.normalize_slippage(
            safe_divide(initial_amount - route.expected_output - route.total_fees, initial_amount)
        )
        speed_score = self.normalize_speed(route.estimated_time)
        # reliability: average per-hop implied reliability (from fee ratio heuristic)
        hop_reliabilities = [
            clamp(1.0 - hop.fee_percentage / 100.0 * 2.0, 0.0, 1.0) for hop in route.path
        ]
        avg_reliability = sum(hop_reliabilities) / len(hop_reliabilities)
        rel_score = self.normalize_reliability(avg_reliability)
        mev_score = self.normalize_mev(route.total_fees * 0.1, initial_amount)  # heuristic

        return weighted_average(
            [fee_score, slip_score, speed_score, rel_score, mev_score],
            [w.fees, w.slippage, w.speed, w.reliability, w.mev_exposure],
        )

    def score_hop(self, hop: RouteHop, weights: ScoringWeights | None = None) -> float:
        """Score a single hop in [0, 1]."""
        w = (weights or self._default_weights).normalized()
        if hop.input_amount == 0:
            return 0.0

        fee_score = self.normalize_fee(hop.fee, hop.input_amount)
        slip_score = self.normalize_slippage(
            safe_divide(hop.input_amount - hop.output_amount - hop.fee, hop.input_amount)
        )
        speed_score = self.normalize_speed(hop.estimated_time)
        rel_score = self.normalize_reliability(clamp(1.0 - hop.fee_percentage / 100.0 * 2.0, 0.0, 1.0))
        mev_score = self.normalize_mev(hop.fee * 0.1, hop.input_amount)

        return weighted_average(
            [fee_score, slip_score, speed_score, rel_score, mev_score],
            [w.fees, w.slippage, w.speed, w.reliability, w.mev_exposure],
        )

    # ---- normalization helpers (all return 0-1, higher = better) -----------

    @staticmethod
    def normalize_fee(fee: float, amount: float) -> float:
        """Score fees: 1.0 = zero fees, 0.0 = fees >= _MAX_FEE_RATIO of amount."""
        ratio = safe_divide(fee, amount)
        return 1.0 - clamp(ratio / _MAX_FEE_RATIO, 0.0, 1.0)

    @staticmethod
    def normalize_slippage(slippage: float) -> float:
        """Score slippage: 1.0 = zero slippage, 0.0 = slippage >= _MAX_SLIPPAGE."""
        return 1.0 - clamp(slippage / _MAX_SLIPPAGE, 0.0, 1.0)

    @staticmethod
    def normalize_speed(time_seconds: int | float) -> float:
        """Score speed: 1.0 = instant, 0.0 = >= _MAX_TIME_SECONDS."""
        return 1.0 - clamp(float(time_seconds) / _MAX_TIME_SECONDS, 0.0, 1.0)

    @staticmethod
    def normalize_reliability(success_rate: float) -> float:
        """Score reliability: 1.0 = 100% success, 0.0 = <= _MIN_RELIABILITY."""
        return normalize_to_range(success_rate, _MIN_RELIABILITY, 1.0)

    @staticmethod
    def normalize_mev(exposure: float, amount: float) -> float:
        """Score MEV exposure: 1.0 = no exposure, 0.0 = exposure >= _MAX_MEV_RATIO."""
        ratio = safe_divide(exposure, amount)
        return 1.0 - clamp(ratio / _MAX_MEV_RATIO, 0.0, 1.0)
