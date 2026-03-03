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
                str(i),
                " > ".join(r.bridges_used),
                str(r.hop_count),
                f"{r.expected_output:.4f}",
                f"{r.guaranteed_minimum:.4f}",
                f"{r.total_fees:.4f}",
                str(r.estimated_time),
                f"{r.minimax_score:.4f}",
            )

        console.print(table)

        stats = router.last_search_stats
        console.print(
            f"\n[dim]Explored {stats.nodes_explored} nodes, "
            f"pruned {stats.nodes_pruned}, "
            f"max depth {stats.max_depth_reached}, "
            f"in {stats.search_time_ms:.1f}ms[/dim]"
        )

    except Exception as exc:
        console.print(f"[red]Error: {exc}[/red]")
        sys.exit(1)


@main.command()
@click.argument("from_chain")
@click.argument("from_token")
@click.argument("amount", type=float)
@click.argument("to_chain")
@click.argument("to_token")
@click.option("--max-hops", "-m", default=2, type=int)
@click.option("--slippage", default=0.005, type=float)
def compare(
    from_chain: str,
    from_token: str,
    amount: float,
    to_chain: str,
    to_token: str,
    max_hops: int,
    slippage: float,
) -> None:
    """Compare all strategies for a given pair.

    Example: mnmx compare ethereum USDC 1000 arbitrum USDC
    """
    try:
        router = _make_router("minimax", max_hops, slippage)
        analyzer = BatchAnalyzer(router)
        analysis = analyzer.analyze_pair(from_chain, from_token, amount, to_chain, to_token)

        table = Table(title=f"Strategy Comparison: {from_token}@{from_chain} -> {to_token}@{to_chain}")
        table.add_column("Strategy")
        table.add_column("Output", justify="right")
        table.add_column("Min Output", justify="right")
        table.add_column("Fees", justify="right")
        table.add_column("Hops", justify="right")
        table.add_column("Score", justify="right")
        table.add_column("Best?", justify="center")

        best = analysis.best_strategy
        for strat, r in analysis.routes_by_strategy.items():
            if r is not None:
                marker = "[green]***[/green]" if strat == best else ""
                table.add_row(
                    strat,
                    f"{r.expected_output:.4f}",
                    f"{r.guaranteed_minimum:.4f}",
                    f"{r.total_fees:.4f}",
                    str(r.hop_count),
                    f"{r.minimax_score:.4f}",
                    marker,
                )
            else:
                table.add_row(strat, "N/A", "N/A", "N/A", "N/A", "N/A", "")

        console.print(table)
        console.print(f"\n[bold]Best strategy: {best}[/bold]  Score spread: {analysis.score_spread:.4f}")

    except Exception as exc:
        console.print(f"[red]Error: {exc}[/red]")
        sys.exit(1)


@main.command()
@click.argument("from_chain")
@click.argument("from_token")
@click.argument("amount", type=float)
@click.argument("to_chain")
@click.argument("to_token")
@click.option("--strategy", "-s", default="minimax", type=click.Choice(VALID_STRATEGIES))
@click.option("--iterations", "-n", default=5000, type=int, help="Monte Carlo iterations")
@click.option("--seed", default=None, type=int, help="Random seed for reproducibility")
@click.option("--max-hops", "-m", default=2, type=int)
def simulate(
    from_chain: str,
    from_token: str,
    amount: float,
    to_chain: str,
    to_token: str,
    strategy: str,
    iterations: int,
    seed: Optional[int],
    max_hops: int,
