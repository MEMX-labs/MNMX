"""
Command-line interface for the MNMX SDK.

Provides subcommands for search, simulation, backtesting, pool analysis,
and threat detection with optional JSON output.
"""

from __future__ import annotations

import argparse
import asyncio
import json
import sys
from pathlib import Path
from typing import Any

from rich.console import Console
from rich.table import Table
from rich.panel import Panel
from rich.text import Text

from mnmx.types import (
    ActionKind,
    BacktestConfig,
    ExecutionAction,
    OnChainState,
    SearchConfig,
    SimulationConfig,
)


console = Console()


def main() -> None:
    """Entry point for the mnmx CLI."""
    parser = argparse.ArgumentParser(
        prog="mnmx",
        description="MNMX SDK — Minimax execution engine CLI",
    )
    parser.add_argument(
        "--json", action="store_true", help="Output results as JSON"
    )
    parser.add_argument(
        "--endpoint",
        default="http://localhost:8080",
        help="MNMX engine endpoint URL",
    )
    parser.add_argument("--api-key", default=None, help="API key for authentication")

    subparsers = parser.add_subparsers(dest="command", required=True)

    # -- search -------------------------------------------------------------
    p_search = subparsers.add_parser("search", help="Run minimax search")
    p_search.add_argument("--wallet", required=True, help="Wallet address")
    p_search.add_argument(
        "--action",
        required=True,
        choices=["swap", "add_liquidity", "remove_liquidity"],
    )
    p_search.add_argument("--amount", required=True, type=int, help="Amount in lamports")
    p_search.add_argument("--pool", required=True, help="Pool address")
    p_search.add_argument("--token-in", default="SOL", help="Input token mint")
    p_search.add_argument("--token-out", default="USDC", help="Output token mint")
    p_search.add_argument("--depth", type=int, default=6, help="Search depth")

    # -- simulate -----------------------------------------------------------
    p_sim = subparsers.add_parser("simulate", help="Simulate an action locally")
    p_sim.add_argument("--state", required=True, help="Path to state JSON file")
    p_sim.add_argument("--action", required=True, help="Path to action JSON file")

    # -- backtest -----------------------------------------------------------
    p_bt = subparsers.add_parser("backtest", help="Run a backtest")
    p_bt.add_argument("--data", required=True, help="Path to historical states JSON")
    p_bt.add_argument(
        "--strategy",
        default="simple",
        choices=["simple", "mev-aware"],
        help="Strategy to backtest",
    )
    p_bt.add_argument("--amount", type=int, default=1_000_000, help="Trade amount")
    p_bt.add_argument("--token-in", default="SOL")
    p_bt.add_argument("--token-out", default="USDC")

    # -- analyze-pool -------------------------------------------------------
    p_pool = subparsers.add_parser("analyze-pool", help="Analyze a liquidity pool")
    p_pool.add_argument("--pool", required=True, help="Pool address")
    p_pool.add_argument("--state", default=None, help="Path to state JSON (for local analysis)")

    # -- threats ------------------------------------------------------------
    p_threat = subparsers.add_parser("threats", help="Detect MEV threats")
    p_threat.add_argument("--action", required=True, help="Path to action JSON file")
    p_threat.add_argument("--state", required=True, help="Path to state JSON file")

    args = parser.parse_args()

    try:
        if args.command == "search":
            asyncio.run(_cmd_search(args))
        elif args.command == "simulate":
            _cmd_simulate(args)
        elif args.command == "backtest":
            _cmd_backtest(args)
        elif args.command == "analyze-pool":
            _cmd_analyze_pool(args)
        elif args.command == "threats":
            asyncio.run(_cmd_threats(args))
    except FileNotFoundError as exc:
        console.print(f"[red]File not found:[/red] {exc}")
        sys.exit(1)
    except KeyboardInterrupt:
        console.print("\n[yellow]Interrupted[/yellow]")
        sys.exit(130)
    except Exception as exc:
        console.print(f"[red]Error:[/red] {exc}")
        sys.exit(1)


# ---------------------------------------------------------------------------
# Subcommand implementations
# ---------------------------------------------------------------------------

async def _cmd_search(args: argparse.Namespace) -> None:
    from mnmx.client import MnmxClient

    action = ExecutionAction(
        kind=ActionKind(args.action),
        pool_address=args.pool,
        token_in=args.token_in,
        token_out=args.token_out,
        amount_in=args.amount,
    )
    state = OnChainState(
        slot=0,
        wallet_address=args.wallet,
    )
    config = SearchConfig(max_depth=args.depth)

    async with MnmxClient(args.endpoint, api_key=args.api_key) as client:
        plan = await client.search(state, [action], config)

    if args.json:
        print(plan.model_dump_json(indent=2))
        return

    table = Table(title="Search Results")
    table.add_column("Metric", style="cyan")
    table.add_column("Value", style="green")
    table.add_row("Expected value", f"{plan.expected_value:.6f}")
    table.add_row("Worst-case value", f"{plan.worst_case_value:.6f}")
    table.add_row("Search depth", str(plan.search_depth))
    table.add_row("Nodes explored", f"{plan.nodes_explored:,}")
    table.add_row("Time (ms)", f"{plan.time_ms:.1f}")
    table.add_row("Actions", str(len(plan.actions)))
    table.add_row("Threats mitigated", str(len(plan.threats_mitigated)))
    console.print(table)


def _cmd_simulate(args: argparse.Namespace) -> None:
    from mnmx.simulator import Simulator

    state_data = json.loads(Path(args.state).read_text())
    action_data = json.loads(Path(args.action).read_text())

    state = OnChainState.model_validate(state_data)
    action = ExecutionAction.model_validate(action_data)

    sim = Simulator(SimulationConfig())
    result = sim.simulate_action(state, action)

    if args.json:
        print(result.model_dump_json(indent=2))
        return

    panel_lines = [
        f"Success:        {result.success}",
        f"Amount out:     {result.amount_out:,}",
        f"Price impact:   {result.price_impact_bps} bps",
        f"Slippage:       {result.slippage_bps} bps",
        f"Gas cost:       {result.gas_cost_lamports:,} lamports",
        f"MEV risk:       {result.mev_risk:.2%}",
        f"Effective price: {result.effective_price:.8f}",
    ]
    if result.warnings:
        panel_lines.append("")
        panel_lines.append("Warnings:")
        for w in result.warnings:
            panel_lines.append(f"  - {w}")
    if result.error:
        panel_lines.append(f"\nError: {result.error}")

    console.print(Panel("\n".join(panel_lines), title="Simulation Result"))


def _cmd_backtest(args: argparse.Namespace) -> None:
    from mnmx.backtester import Backtester, SimpleSwapStrategy, MevAwareStrategy

    data = json.loads(Path(args.data).read_text())
    states = [OnChainState.model_validate(s) for s in data]

    if args.strategy == "mev-aware":
        strategy = MevAwareStrategy(
            token_in=args.token_in,
            token_out=args.token_out,
            amount=args.amount,
        )
    else:
        strategy = SimpleSwapStrategy(
            token_in=args.token_in,
            token_out=args.token_out,
            amount=args.amount,
        )

    config = BacktestConfig(
        initial_balance={args.token_in: args.amount * 100},
    )
    bt = Backtester(config)
    result = bt.run(states, strategy)

    if args.json:
        print(result.model_dump_json(indent=2))
        return

    report = bt.generate_report(result)
    console.print(report)


def _cmd_analyze_pool(args: argparse.Namespace) -> None:
    from mnmx.pool_analyzer import PoolAnalyzer

    if args.state:
        state_data = json.loads(Path(args.state).read_text())
        state = OnChainState.model_validate(state_data)
        pool = state.get_pool(args.pool)
        if pool is None:
            console.print(f"[red]Pool {args.pool} not found in state[/red]")
            sys.exit(1)

        analyzer = PoolAnalyzer()
        analysis = analyzer.analyze_pool_local(pool, state.token_prices_usd)

        if args.json:
            result = {
                "address": analysis.pool.address,
                "tvl_usd": analysis.tvl_usd,
                "price_a_in_b": analysis.price_a_in_b,
                "price_b_in_a": analysis.price_b_in_a,
                "fee_apr": analysis.fee_apr_estimate,
                "imbalance": analysis.imbalance_ratio,
                "depth": [
                    {
                        "impact_bps": d.impact_bps,
                        "max_buy": d.max_buy_amount,
                        "max_sell": d.max_sell_amount,
                    }
                    for d in analysis.depth_levels
                ],
            }
            print(json.dumps(result, indent=2))
            return

        table = Table(title=f"Pool Analysis: {args.pool[:16]}...")
        table.add_column("Metric", style="cyan")
        table.add_column("Value", style="green")
        table.add_row("TVL (USD)", f"${analysis.tvl_usd:,.2f}")
        table.add_row("Price A/B", f"{analysis.price_a_in_b:.8f}")
        table.add_row("Price B/A", f"{analysis.price_b_in_a:.8f}")
        table.add_row("Fee APR", f"{analysis.fee_apr_estimate:.2%}")
        table.add_row("Imbalance", f"{analysis.imbalance_ratio:.4f}")
        table.add_row("Reserve A", f"{analysis.pool.reserve_a:,}")
        table.add_row("Reserve B", f"{analysis.pool.reserve_b:,}")
        console.print(table)

        if analysis.depth_levels:
            depth_table = Table(title="Liquidity Depth")
            depth_table.add_column("Impact (bps)")
            depth_table.add_column("Max Buy")
            depth_table.add_column("Max Sell")
            for d in analysis.depth_levels:
                depth_table.add_row(
                    str(d.impact_bps),
                    f"{d.max_buy_amount:,}",
                    f"{d.max_sell_amount:,}",
                )
            console.print(depth_table)
    else:
        console.print("[yellow]Remote pool analysis requires --state file for local mode[/yellow]")


async def _cmd_threats(args: argparse.Namespace) -> None:
    from mnmx.client import MnmxClient

    action_data = json.loads(Path(args.action).read_text())
    state_data = json.loads(Path(args.state).read_text())

    action = ExecutionAction.model_validate(action_data)
    state = OnChainState.model_validate(state_data)

    async with MnmxClient(args.endpoint, api_key=args.api_key) as client:
        threats = await client.detect_threats(action, state)

    if args.json:
        print(json.dumps([t.model_dump() for t in threats], indent=2))
        return

    if not threats:
        console.print("[green]No MEV threats detected[/green]")
        return

    table = Table(title="MEV Threats Detected")
    table.add_column("Type", style="red")
    table.add_column("Confidence", style="yellow")
    table.add_column("Est. Loss", style="red")
    table.add_column("Description")

    for t in threats:
        table.add_row(
            t.kind.value,
            f"{t.confidence:.1%}",
            f"{t.estimated_victim_loss_lamports:,}",
            t.description or "—",
        )
    console.print(table)


if __name__ == "__main__":
    main()
