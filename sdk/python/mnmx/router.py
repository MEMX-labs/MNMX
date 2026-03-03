"""Core MNMX router: minimax-based cross-chain path discovery."""

from __future__ import annotations

import time
from dataclasses import dataclass, field
from itertools import permutations
from typing import Any

from mnmx.bridges import BridgeAdapter, BridgeRegistry, create_default_registry
from mnmx.exceptions import (
    InvalidConfigError,
    NoRouteFoundError,
    RouteTimeoutError,
)
from mnmx.scoring import RouteScorer, get_strategy_weights
from mnmx.types import (
    AdversarialModel,
    BridgeQuote,
    Chain,
    Route,
    RouteHop,
    RouteRequest,
    RouterConfig,
    ScoringWeights,
    SearchStats,
    Strategy,
    VALID_STRATEGIES,
)


@dataclass
class _SearchNode:
    """Internal node in the minimax game tree."""

    chain: Chain
    token: str
    amount: float
    depth: int
    hops: list[RouteHop] = field(default_factory=list)
    total_fee: float = 0.0
    total_time: int = 0


class MnmxRouter:
    """Cross-chain router using minimax search with alpha-beta pruning.

    The router models cross-chain routing as a two-player game:
    - MAX player: the user, choosing the best bridge at each hop
    - MIN player: the adversarial market (slippage, MEV, delays)

    The minimax search finds the route whose *worst-case* outcome
    is maximised (the guaranteed minimum).
    """

    def __init__(
        self,
        strategy: Strategy = "minimax",
        config: RouterConfig | None = None,
        registry: BridgeRegistry | None = None,
        **kwargs: Any,
    ) -> None:
        if config is not None:
            self._config = config
        else:
            weights = kwargs.get("weights")
            adversarial = kwargs.get("adversarial_model")
            self._config = RouterConfig(
                strategy=strategy,
                slippage_tolerance=kwargs.get("slippage_tolerance", 0.005),
                timeout_ms=kwargs.get("timeout_ms", 5000),
                max_hops=kwargs.get("max_hops", 3),
                weights=weights if isinstance(weights, ScoringWeights) else ScoringWeights(),
                adversarial_model=adversarial if isinstance(adversarial, AdversarialModel) else AdversarialModel(),
            )
        self._registry = registry or create_default_registry()
        self._scorer = RouteScorer(self._config.weights)
        self._stats = SearchStats(0, 0, 0, 0.0)

    # ---- public API --------------------------------------------------------

    @property
    def config(self) -> RouterConfig:
        return self._config

    @property
    def last_search_stats(self) -> SearchStats:
        return self._stats

    def find_route(
        self,
        from_chain: str | Chain,
        from_token: str,
        amount: float,
        to_chain: str | Chain,
        to_token: str,
        **kwargs: Any,
    ) -> Route:
        """Find the single best route using the configured strategy."""
        routes = self.find_all_routes(from_chain, from_token, amount, to_chain, to_token, **kwargs)
        if not routes:
            src = from_chain if isinstance(from_chain, str) else from_chain.value
            dst = to_chain if isinstance(to_chain, str) else to_chain.value
            raise NoRouteFoundError(src, dst, from_token, to_token)
        return routes[0]

    def find_all_routes(
        self,
        from_chain: str | Chain,
        from_token: str,
        amount: float,
        to_chain: str | Chain,
        to_token: str,
        **kwargs: Any,
    ) -> list[Route]:
        """Find all viable routes, sorted best-first by minimax score."""
        request = self._build_request(from_chain, from_token, amount, to_chain, to_token, **kwargs)
        self._validate_request(request)

        start_ms = time.monotonic() * 1000

        # discover candidate paths (sequences of (chain, bridge) stops)
        candidate_paths = self._discover_paths(request)

        # run minimax on each path to get scored routes
        routes: list[Route] = []
        for path_chains, path_bridges in candidate_paths:
            elapsed = time.monotonic() * 1000 - start_ms
            if elapsed > self._config.timeout_ms:
                break
            route = self._evaluate_path(request, path_chains, path_bridges)
            if route is not None:
                routes.append(route)

        self._stats.search_time_ms = time.monotonic() * 1000 - start_ms

        # sort by minimax score descending
        strategy = kwargs.get("strategy", self._config.strategy)
        weights = get_strategy_weights(strategy)
        for r in routes:
            r.minimax_score = self._scorer.score_route(r, weights)
            r.strategy = strategy

        routes.sort(key=lambda r: r.minimax_score, reverse=True)
        return routes

    def get_supported_chains(self) -> list[str]:
        return Chain.all_names()

    def get_supported_bridges(self) -> list[str]:
        return self._registry.names()

    # ---- internal ----------------------------------------------------------

    def _build_request(
        self,
        from_chain: str | Chain,
        from_token: str,
        amount: float,
        to_chain: str | Chain,
        to_token: str,
        **kwargs: Any,
    ) -> RouteRequest:
        src = Chain.from_str(from_chain) if isinstance(from_chain, str) else from_chain
        dst = Chain.from_str(to_chain) if isinstance(to_chain, str) else to_chain
        return RouteRequest(
            from_chain=src,
            from_token=from_token,
            amount=amount,
            to_chain=dst,
            to_token=to_token,
            strategy=kwargs.get("strategy", self._config.strategy),
            max_hops=kwargs.get("max_hops", self._config.max_hops),
            slippage_tolerance=kwargs.get("slippage_tolerance", self._config.slippage_tolerance),
        )

    def _validate_request(self, request: RouteRequest) -> None:
        if request.from_chain == request.to_chain and request.from_token == request.to_token:
            raise InvalidConfigError("route", "Source and destination are identical")
        if request.amount <= 0:
            raise InvalidConfigError("amount", "Amount must be positive")

    def _discover_paths(
        self, request: RouteRequest
    ) -> list[tuple[list[Chain], list[str]]]:
        """Enumerate candidate paths as (chain sequence, bridge sequence)."""
        results: list[tuple[list[Chain], list[str]]] = []

        src = request.from_chain
        dst = request.to_chain

        # 1-hop direct routes
        direct_bridges = self._registry.get_for_pair(src, dst)
        for bridge in direct_bridges:
            results.append(([src, dst], [bridge.name]))

        if request.max_hops < 2:
            return results

        # 2-hop routes via intermediate chains
        all_chains = list(Chain)
        intermediate_chains = [c for c in all_chains if c != src and c != dst]
        for mid in intermediate_chains:
            bridges_leg1 = self._registry.get_for_pair(src, mid)
            bridges_leg2 = self._registry.get_for_pair(mid, dst)
            for b1 in bridges_leg1:
                for b2 in bridges_leg2:
                    results.append(([src, mid, dst], [b1.name, b2.name]))

        if request.max_hops < 3:
            return results

        # 3-hop routes via two intermediates
        for mid1 in intermediate_chains:
            for mid2 in intermediate_chains:
                if mid1 == mid2:
