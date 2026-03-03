"""CLI interface for the MNMX SDK."""

from __future__ import annotations

import json
import sys
from typing import Optional

import click
from rich.console import Console
from rich.table import Table

from mnmx.batch_analyzer import BatchAnalyzer
from mnmx.bridges import create_default_registry
from mnmx.router import MnmxRouter
from mnmx.simulator import RouteSimulator
from mnmx.types import VALID_STRATEGIES, Chain, RouterConfig, ScoringWeights

console = Console()


def _make_router(strategy: str, max_hops: int, slippage: float) -> MnmxRouter:
    return MnmxRouter(
        strategy=strategy,  # type: ignore[arg-type]
        config=RouterConfig(
            strategy=strategy,  # type: ignore[arg-type]
            max_hops=max_hops,
            slippage_tolerance=slippage,
        ),
    )


@click.group()
@click.version_option(version="0.1.0", prog_name="mnmx")
def main() -> None:
    """MNMX - Cross-chain routing via minimax search."""
    pass


@main.command()
@click.argument("from_chain")
@click.argument("from_token")
@click.argument("amount", type=float)
@click.argument("to_chain")
@click.argument("to_token")
@click.option("--strategy", "-s", default="minimax", type=click.Choice(VALID_STRATEGIES))
@click.option("--max-hops", "-m", default=2, type=int, help="Maximum hops (1-5)")
@click.option("--slippage", default=0.005, type=float, help="Slippage tolerance (decimal)")
@click.option("--all-routes", "-a", is_flag=True, help="Show all routes, not just the best")
def route(
    from_chain: str,
    from_token: str,
    amount: float,
    to_chain: str,
    to_token: str,
    strategy: str,
    max_hops: int,
    slippage: float,
    all_routes: bool,
) -> None:
    """Find an optimal cross-chain route.

    Example: mnmx route ethereum USDC 1000 polygon USDC
    """
    try:
        router = _make_router(strategy, max_hops, slippage)

        if all_routes:
            routes = router.find_all_routes(from_chain, from_token, amount, to_chain, to_token, strategy=strategy)
        else:
            best = router.find_route(from_chain, from_token, amount, to_chain, to_token, strategy=strategy)
            routes = [best]

        if not routes:
            console.print("[red]No routes found.[/red]")
            sys.exit(1)

        table = Table(title=f"Routes: {from_token}@{from_chain} -> {to_token}@{to_chain}")
        table.add_column("#", style="dim")
        table.add_column("Bridges")
        table.add_column("Hops", justify="right")
        table.add_column("Output", justify="right")
        table.add_column("Min Output", justify="right")
        table.add_column("Fees", justify="right")
        table.add_column("Time (s)", justify="right")
        table.add_column("Score", justify="right")

        for i, r in enumerate(routes[:20], 1):
            table.add_row(
