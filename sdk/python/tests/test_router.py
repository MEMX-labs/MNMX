"""Tests for the MNMX router."""

from __future__ import annotations

import pytest

from mnmx.exceptions import InvalidConfigError, NoRouteFoundError
from mnmx.router import MnmxRouter
from mnmx.types import Chain, RouterConfig, ScoringWeights


class TestFindRouteBasic:
    def test_find_route_returns_route(self, router: MnmxRouter) -> None:
        route = router.find_route("ethereum", "USDC", 1000.0, "polygon", "USDC")
        assert route is not None
        assert route.expected_output > 0
        assert route.guaranteed_minimum > 0
        assert route.total_fees > 0
        assert route.hop_count >= 1

    def test_find_route_output_less_than_input(self, router: MnmxRouter) -> None:
        route = router.find_route("ethereum", "USDC", 1000.0, "arbitrum", "USDC")
        assert route.expected_output < 1000.0

    def test_find_route_guaranteed_minimum_less_than_expected(self, router: MnmxRouter) -> None:
        route = router.find_route("ethereum", "USDC", 1000.0, "polygon", "USDC")
        assert route.guaranteed_minimum <= route.expected_output

    def test_find_route_has_strategy(self, router: MnmxRouter) -> None:
        route = router.find_route("ethereum", "USDC", 500.0, "bsc", "USDC")
        assert route.strategy == "minimax"

    def test_find_route_chains_visited(self, router: MnmxRouter) -> None:
        route = router.find_route("ethereum", "USDC", 1000.0, "polygon", "USDC")
        chains = route.chains_visited
        assert Chain.ETHEREUM in chains
        assert Chain.POLYGON in chains


class TestFindAllRoutes:
    def test_find_all_routes_returns_list(self, router: MnmxRouter) -> None:
        routes = router.find_all_routes("ethereum", "USDC", 1000.0, "polygon", "USDC")
        assert isinstance(routes, list)
        assert len(routes) >= 1

    def test_routes_sorted_by_score(self, router: MnmxRouter) -> None:
        routes = router.find_all_routes("ethereum", "USDC", 1000.0, "polygon", "USDC")
        if len(routes) >= 2:
            for i in range(len(routes) - 1):
                assert routes[i].minimax_score >= routes[i + 1].minimax_score

    def test_all_routes_have_positive_output(self, router: MnmxRouter) -> None:
        routes = router.find_all_routes("ethereum", "USDC", 1000.0, "arbitrum", "USDC")
        for route in routes:
            assert route.expected_output > 0


class TestStrategySelection:
    def test_minimax_strategy(self) -> None:
        router = MnmxRouter(strategy="minimax")
        route = router.find_route("ethereum", "USDC", 1000.0, "polygon", "USDC", strategy="minimax")
        assert route.strategy == "minimax"

    def test_aggressive_strategy(self) -> None:
        router = MnmxRouter(strategy="aggressive")
        route = router.find_route("ethereum", "USDC", 1000.0, "polygon", "USDC", strategy="aggressive")
        assert route.strategy == "aggressive"

    def test_conservative_strategy(self) -> None:
        router = MnmxRouter(strategy="conservative")
        route = router.find_route("ethereum", "USDC", 1000.0, "polygon", "USDC", strategy="conservative")
        assert route.strategy == "conservative"

    def test_balanced_strategy(self) -> None:
        router = MnmxRouter(strategy="balanced")
        route = router.find_route("ethereum", "USDC", 1000.0, "polygon", "USDC", strategy="balanced")
        assert route.strategy == "balanced"

    def test_different_strategies_may_score_differently(self) -> None:
        router = MnmxRouter(strategy="minimax", config=RouterConfig(max_hops=1))
        r1 = router.find_route("ethereum", "USDC", 1000.0, "polygon", "USDC", strategy="minimax")
        r2 = router.find_route("ethereum", "USDC", 1000.0, "polygon", "USDC", strategy="aggressive")
        # Scores may differ because different weight distributions
        assert isinstance(r1.minimax_score, float)
        assert isinstance(r2.minimax_score, float)


class TestMaxHopsConstraint:
    def test_single_hop_route(self) -> None:
        router = MnmxRouter(config=RouterConfig(max_hops=1))
        routes = router.find_all_routes("ethereum", "USDC", 1000.0, "polygon", "USDC")
        for route in routes:
            assert route.hop_count <= 1

    def test_two_hop_limit(self) -> None:
        router = MnmxRouter(config=RouterConfig(max_hops=2))
        routes = router.find_all_routes("ethereum", "USDC", 1000.0, "polygon", "USDC")
        for route in routes:
            assert route.hop_count <= 2


class TestUnsupportedChain:
    def test_unknown_chain_raises(self, router: MnmxRouter) -> None:
        with pytest.raises(ValueError, match="Unknown chain"):
            router.find_route("mars", "USDC", 1000.0, "polygon", "USDC")

    def test_identical_source_dest_raises(self, router: MnmxRouter) -> None:
        with pytest.raises(InvalidConfigError):
            router.find_route("ethereum", "USDC", 1000.0, "ethereum", "USDC")


class TestCustomWeights:
    def test_custom_weights_accepted(self) -> None:
        weights = ScoringWeights(fees=0.5, slippage=0.1, speed=0.1, reliability=0.2, mev_exposure=0.1)
        router = MnmxRouter(config=RouterConfig(weights=weights, max_hops=1))
        route = router.find_route("ethereum", "USDC", 1000.0, "polygon", "USDC")
        assert route is not None
        assert route.minimax_score > 0

    def test_fee_heavy_weights_vs_speed_heavy(self) -> None:
        fee_w = ScoringWeights(fees=0.8, slippage=0.05, speed=0.05, reliability=0.05, mev_exposure=0.05)
        speed_w = ScoringWeights(fees=0.05, slippage=0.05, speed=0.8, reliability=0.05, mev_exposure=0.05)
        r1 = MnmxRouter(config=RouterConfig(weights=fee_w, max_hops=1))
        r2 = MnmxRouter(config=RouterConfig(weights=speed_w, max_hops=1))
        route1 = r1.find_route("ethereum", "USDC", 1000.0, "polygon", "USDC")
        route2 = r2.find_route("ethereum", "USDC", 1000.0, "polygon", "USDC")
        # both should produce valid routes
        assert route1.expected_output > 0
        assert route2.expected_output > 0


class TestSupportedChainsBridges:
    def test_get_supported_chains(self, router: MnmxRouter) -> None:
        chains = router.get_supported_chains()
        assert "ethereum" in chains
        assert "polygon" in chains
        assert len(chains) >= 5

    def test_get_supported_bridges(self, router: MnmxRouter) -> None:
        bridges = router.get_supported_bridges()
        assert "wormhole" in bridges
        assert "layerzero" in bridges
        assert len(bridges) >= 3
