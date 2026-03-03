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
                    continue
                b1s = self._registry.get_for_pair(src, mid1)
                b2s = self._registry.get_for_pair(mid1, mid2)
                b3s = self._registry.get_for_pair(mid2, dst)
                for b1 in b1s:
                    for b2 in b2s:
                        for b3 in b3s:
                            results.append(
                                ([src, mid1, mid2, dst], [b1.name, b2.name, b3.name])
                            )
                            # limit combinatorial explosion
                            if len(results) > 500:
                                return results

        return results

    def _evaluate_path(
        self,
        request: RouteRequest,
        chain_sequence: list[Chain],
        bridge_names: list[str],
    ) -> Route | None:
        """Run minimax evaluation on a single candidate path."""
        self._stats.nodes_explored += 1

        hops: list[RouteHop] = []
        current_amount = request.amount
        total_fee = 0.0
        total_time = 0
        from_token = request.from_token

        for i, bridge_name in enumerate(bridge_names):
            src_chain = chain_sequence[i]
            dst_chain = chain_sequence[i + 1]
            to_token = request.to_token if i == len(bridge_names) - 1 else from_token

            try:
                bridge = self._registry.get(bridge_name)
                quote = bridge.get_quote(src_chain, dst_chain, from_token, to_token, current_amount)
            except Exception:
                self._stats.nodes_pruned += 1
                return None

            # adversarial adjustment (MIN player moves)
            adv = self._config.adversarial_model
            adversarial_fee = quote.fee * adv.gas_multiplier
            adversarial_slippage = quote.slippage * adv.slippage_multiplier
            mev_loss = current_amount * adv.mev_extraction
            price_impact = current_amount * adv.price_movement

            worst_case_output = (
                current_amount
                - adversarial_fee
                - (current_amount * adversarial_slippage)
                - mev_loss
                - price_impact
            )
            worst_case_output = max(worst_case_output, 0.0)

            hop = RouteHop(
                from_chain=src_chain,
                to_chain=dst_chain,
                from_token=from_token,
                to_token=to_token,
                bridge=bridge_name,
                input_amount=current_amount,
                output_amount=quote.output_amount,
                fee=quote.fee,
                estimated_time=int(quote.estimated_time * adv.bridge_delay_multiplier),
            )
            hops.append(hop)

            total_fee += quote.fee
            total_time += hop.estimated_time
            current_amount = quote.output_amount
            from_token = to_token

        if not hops:
            return None

        self._stats.max_depth_reached = max(self._stats.max_depth_reached, len(hops))

        # guaranteed minimum via minimax: apply adversarial model to expected output
        expected_output = current_amount
        guaranteed_minimum = self._compute_guaranteed_minimum(hops, request.amount)

        route = Route(
            path=hops,
            expected_output=expected_output,
            guaranteed_minimum=guaranteed_minimum,
            total_fees=total_fee,
            estimated_time=total_time,
            minimax_score=0.0,  # scored later
            strategy=request.strategy,
        )
        return route

    def _compute_guaranteed_minimum(self, hops: list[RouteHop], initial_amount: float) -> float:
        """Compute worst-case guaranteed output using adversarial model."""
        adv = self._config.adversarial_model
        amount = initial_amount

        for hop in hops:
            fee_worst = hop.fee * adv.gas_multiplier
            slippage_worst = amount * (hop.fee / hop.input_amount) * adv.slippage_multiplier
            mev = amount * adv.mev_extraction
            price = amount * adv.price_movement
            amount = amount - fee_worst - slippage_worst - mev - price
            amount = max(amount, 0.0)

        return amount

    def _run_minimax(
        self,
        node: _SearchNode,
        target_chain: Chain,
        target_token: str,
        alpha: float,
        beta: float,
        is_maximizing: bool,
        request: RouteRequest,
    ) -> float:
        """Recursive minimax with alpha-beta pruning.

        MAX player picks the best bridge.
        MIN player applies worst-case adversarial conditions.
        """
        self._stats.nodes_explored += 1

        # terminal: reached destination
        if node.chain == target_chain and node.depth > 0:
            return node.amount

        # depth limit
        if node.depth >= request.max_hops:
            self._stats.nodes_pruned += 1
            return 0.0 if node.chain != target_chain else node.amount

        bridges = self._registry.get_for_pair(node.chain, target_chain)
        if not bridges and node.depth < request.max_hops - 1:
            # try intermediate hops
            all_adapters = self._registry.get_all()
            reachable: list[tuple[BridgeAdapter, Chain]] = []
            for adapter in all_adapters:
                for chain in adapter.supported_chains:
                    if chain != node.chain and adapter.supports_pair(node.chain, chain):
                        reachable.append((adapter, chain))
            if not reachable:
                return 0.0
            bridges_with_targets = reachable
        else:
            bridges_with_targets = [(b, target_chain) for b in bridges]

        if is_maximizing:
            best = float("-inf")
            for bridge, next_chain in bridges_with_targets:
                try:
                    quote = bridge.get_quote(
                        node.chain, next_chain, node.token, target_token, node.amount
                    )
                except Exception:
                    self._stats.nodes_pruned += 1
                    continue

                child = _SearchNode(
                    chain=next_chain,
                    token=target_token,
                    amount=quote.output_amount,
                    depth=node.depth + 1,
                )
                val = self._run_minimax(child, target_chain, target_token, alpha, beta, False, request)
                best = max(best, val)
                alpha = max(alpha, val)
                if beta <= alpha:
                    self._stats.nodes_pruned += 1
                    break
            return best if best != float("-inf") else 0.0
        else:
            # MIN player: apply adversarial conditions
            adv = self._config.adversarial_model
            worst_amount = node.amount * (1.0 - adv.mev_extraction - adv.price_movement)
            worst_amount = max(worst_amount, 0.0)
            child = _SearchNode(
                chain=node.chain,
                token=node.token,
                amount=worst_amount,
                depth=node.depth,
            )
            return self._run_minimax(child, target_chain, target_token, alpha, beta, True, request)

    def _collect_quotes(
        self,
        from_chain: Chain,
        to_chain: Chain,
        from_token: str,
        to_token: str,
        amount: float,
    ) -> list[BridgeQuote]:
        """Collect quotes from all bridges supporting the given pair."""
        bridges = self._registry.get_for_pair(from_chain, to_chain)
        quotes: list[BridgeQuote] = []
        for bridge in bridges:
            try:
                q = bridge.get_quote(from_chain, to_chain, from_token, to_token, amount)
                quotes.append(q)
            except Exception:
                continue
        return quotes

    def _apply_strategy_weights(self, routes: list[Route], strategy: Strategy) -> list[Route]:
        """Re-score and sort routes according to strategy weights."""
        weights = get_strategy_weights(strategy)
        for route in routes:
            route.minimax_score = self._scorer.score_route(route, weights)
            route.strategy = strategy
        routes.sort(key=lambda r: r.minimax_score, reverse=True)
        return routes
