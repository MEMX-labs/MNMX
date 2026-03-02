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

