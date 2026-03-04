"""Shared pytest fixtures for the MNMX test suite."""

from __future__ import annotations

import pytest

from mnmx.router import MnmxRouter
from mnmx.types import (
    Chain,
    Route,
    RouteHop,
    RouteRequest,
    RouterConfig,
    ScoringWeights,
)


@pytest.fixture
def router() -> MnmxRouter:
    """A default MnmxRouter with sensible test config."""
    return MnmxRouter(
        strategy="minimax",
        config=RouterConfig(
            strategy="minimax",
            max_hops=2,
            slippage_tolerance=0.005,
            timeout_ms=5000,
        ),
    )


@pytest.fixture
def sample_route() -> Route:
    """A two-hop sample route for simulator tests."""
    hop1 = RouteHop(
        from_chain=Chain.ETHEREUM,
        to_chain=Chain.ARBITRUM,
        from_token="USDC",
        to_token="USDC",
        bridge="wormhole",
        input_amount=1000.0,
        output_amount=996.0,
        fee=2.50,
        estimated_time=180,
    )
    hop2 = RouteHop(
        from_chain=Chain.ARBITRUM,
        to_chain=Chain.POLYGON,
        from_token="USDC",
        to_token="USDC",
        bridge="layerzero",
        input_amount=996.0,
        output_amount=993.0,
        fee=1.80,
        estimated_time=90,
    )
    return Route(
        path=[hop1, hop2],
        expected_output=993.0,
        guaranteed_minimum=980.0,
        total_fees=4.30,
        estimated_time=270,
        minimax_score=0.85,
        strategy="minimax",
    )


@pytest.fixture
def sample_request() -> RouteRequest:
    """A sample route request."""
    return RouteRequest(
        from_chain=Chain.ETHEREUM,
        from_token="USDC",
        amount=1000.0,
        to_chain=Chain.POLYGON,
        to_token="USDC",
        strategy="minimax",
        max_hops=2,
        slippage_tolerance=0.005,
    )
