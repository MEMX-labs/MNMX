<p align="center">
  <img src="assets/banner.png" alt="MNMX Banner" width="100%" />
</p>

# MNMX

[![CI](https://img.shields.io/github/actions/workflow/status/MEMX-labs/MNMX/ci.yml?branch=main&style=flat-square&label=build&color=1a1a2e)](https://github.com/MEMX-labs/MNMX/actions)
[![License](https://img.shields.io/badge/license-MIT-1a1a2e?style=flat-square)](./LICENSE)
[![Rust](https://img.shields.io/badge/Rust-engine-1a1a2e?style=flat-square)](https://www.rust-lang.org/)
[![TypeScript](https://img.shields.io/badge/TypeScript-SDK-1a1a2e?style=flat-square)](https://www.typescriptlang.org/)
[![Python](https://img.shields.io/badge/Python-SDK-1a1a2e?style=flat-square)](https://www.python.org/)
[![Website](https://img.shields.io/badge/website-mnmx.app-1a1a2e?style=flat-square)](https://mnmx.app)
[![Twitter](https://img.shields.io/badge/twitter-@mnmxapp-1a1a2e?style=flat-square)](https://x.com/mnmxapp)
[![Docs](https://img.shields.io/badge/docs-mnmx.app%2Fdocs-1a1a2e?style=flat-square)](https://mnmx.app/docs)

---

**Minimax-optimal cross-chain routing. Evaluate every path, guarantee the best worst-case outcome.**

Every cross-chain transfer is a game against market conditions. Slippage spikes. Gas surges. Bridge congestion. MEV extraction. Most aggregators optimize for the best case — MNMX optimizes for the **worst case** and guarantees you the best floor.

MNMX applies the same class of algorithms that defeated world champions in chess to cross-chain route optimization. The engine enumerates every possible path across bridges, models adversarial market conditions at each hop, and uses **minimax search with alpha-beta pruning** to find the route that maximizes your guaranteed minimum outcome.

The key insight: bridge routing is structurally isomorphic to game tree search. Your moves are route choices. The opponent's moves are worst-case market conditions. The minimax algorithm finds the path that remains optimal even when everything goes wrong.

## Why Minimax

Every other aggregator uses **expected value** optimization — pick the route with the highest average outcome. This works when conditions are stable. It fails catastrophically when they're not.

| | Expected Value | Minimax |
|---|---|---|
| Optimizes for | Average case | Worst case |
| When conditions are good | Similar results | Similar results |
| When conditions are bad | Catastrophic loss | **Best guaranteed floor** |
| Large transfers | High variance | **Low variance** |
| Bridge congestion | Unpredictable | **Predictable** |

Consider a $100K transfer with two routes:
- **Route A**: Expected output $99,500. Worst case: $96,200.
- **Route B**: Expected output $99,200. Worst case: $98,800.

Expected-value optimization picks Route A. Minimax picks Route B. When the bridge gets congested and slippage doubles, Route A loses $3,800. Route B loses $1,200. The difference is **$2,600 in guaranteed savings**.

## Architecture

```mermaid
graph TD
    A[RouteRequest] --> B[PathDiscovery]
    B --> C[StateCollector]
    C --> D[MinimaxEngine]
    D --> E[RouteScorer]
    E --> F[RouteExecutor]

    D --> G[AlphaBetaPruning]
    D --> H[TranspositionTable]
    D --> I[AdversarialModel]

    B --> J[BridgeAdapters]
    F --> J

    subgraph Engine Core
