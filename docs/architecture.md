# Architecture

## System Overview

MNMX is a cross-chain routing protocol that treats token bridging as a game-tree
search problem. Instead of greedily picking the cheapest or fastest bridge, MNMX
uses minimax search to find the route with the best **guaranteed minimum
outcome** under adversarial conditions (slippage spikes, MEV extraction, gas
surges, bridge delays).

The system is split across three languages, each chosen for its strengths:

| Layer             | Language   | Purpose                                    |
|-------------------|------------|--------------------------------------------|
| Routing engine    | Rust       | Performance-critical path search and math  |
| TypeScript SDK    | TypeScript | Developer-facing API, bridge adapters      |
| Python SDK        | Python     | Research, simulation, batch analysis, CLI  |

## Multi-Language Architecture

### Rust Engine (`engine/`)

The Rust crate `mnmx-engine` contains:

- **types** -- `Chain`, `Token`, `Route`, `RouteHop`, `BridgeQuote`,
  `ScoringWeights`, `AdversarialModel`, `SearchStats`.
- **math** -- Normalization, weighted averages, variance, softmax, sigmoid,
  basis-point conversions.
- **stats** -- `SearchStatsCollector` for tracking nodes explored, pruning
  efficiency, and depth histograms during search.
- **router** -- `MnmxRouter` orchestrating path discovery, state collection,
  and minimax search.
- **minimax** -- `MinimaxSearcher` implementing alpha-beta search with
  transposition tables.
- **path_discovery** -- `PathDiscovery` for BFS/DFS chain graph traversal.
- **bridge** -- `BridgeAdapter` trait and `BridgeRegistry`.
- **scoring** -- `RouteScorer` with five-dimensional scoring.
- **pruning** -- Alpha-beta state and transposition tables.
- **state** -- `StateCollector` aggregating chain, bridge, and market state.
- **risk** -- `RiskAssessor` for route-level risk classification.

### TypeScript SDK (`src/`)

The `@mnmx/core` npm package mirrors the Rust engine's concepts:

- **types/** -- All interfaces and constants (`Chain`, `Token`, `Route`,
  `ScoringWeights`, `AdversarialModel`, etc.).
- **router/** -- `MnmxRouter` class, path discovery, minimax search, scoring.
- **bridges/** -- `BridgeAdapter` interface, `AbstractBridgeAdapter` base class,
  `BridgeRegistry`, and concrete adapters (Wormhole, deBridge, LayerZero,
  Allbridge).
- **chains/** -- Per-chain configuration and token registries.
- **utils/** -- Logger, math utilities, hashing.

### Python SDK (`sdk/python/`)

The `mnmx-sdk` Python package provides:

- **MnmxRouter** -- Route discovery and scoring.
- **RouteSimulator** -- Single-route simulation with adversarial modeling.
- **BatchAnalyzer** -- Batch route analysis across token pairs.
- **math_utils** -- Statistical helpers (percentiles, variance, normalization).
- **exceptions** -- Typed error hierarchy.
- **CLI** -- `mnmx` command-line tool via Click.

## Data Flow

A route request flows through five stages:

```
1. Path Discovery     Find all chain-level paths (DFS with cycle detection)
         |
2. State Collection   Fetch quotes from each bridge for each hop
         |
3. Minimax Search     Game-tree search with alpha-beta pruning
         |
4. Route Evaluation   Score routes on 5 dimensions, rank by strategy
         |
5. Execution          Sign and submit transactions hop by hop
```

### Stage 1: Path Discovery

Given a source chain and destination chain, discover all chain-level paths
up to `maxHops`. Uses DFS on a graph where nodes are chains and edges are
bridge-supported pairs. Dominated paths (those that are strict supersets
of shorter paths) are pruned.

### Stage 2: State Collection

For each chain-level path, enumerate all bridge combinations (cartesian product
of available bridges per hop). For each combination, fetch quotes sequentially
(the output of hop N becomes the input of hop N+1). Discard combinations where
any bridge fails to quote.

### Stage 3: Minimax Search

Treat each candidate path as a game tree node. The **maximizer** (the user)
picks the path with the best guaranteed outcome. The **minimizer** (the
adversarial model) applies worst-case multipliers:

- Slippage x1.5
- Gas costs x1.3
- Bridge delays x1.4
- MEV extraction 0.5%
- Adverse price movement 1%
- Per-hop failure probability 2%

Alpha-beta pruning skips branches that cannot improve on the current best.

### Stage 4: Route Evaluation

Each surviving route is scored on five dimensions:

1. **Fees** -- Normalized fee ratio (lower is better).
2. **Slippage** -- Normalized basis points (lower is better).
3. **Speed** -- Normalized time (faster is better).
4. **Reliability** -- Compound per-hop success rate.
5. **MEV exposure** -- Estimated extractable value (lower is better).

Weights depend on the selected strategy (`minimax`, `cheapest`, `fastest`,
`safest`). Routes are sorted by weighted composite score.

### Stage 5: Execution

The caller receives the best route and alternatives. To execute, the caller
provides a `Signer` and the router processes hops sequentially, calling
`bridge.execute()` for each hop and monitoring status via `bridge.getStatus()`.

## Bridge Adapter Layer

All bridge integrations implement the `BridgeAdapter` interface:

```typescript
interface BridgeAdapter {
  readonly name: string;
  readonly supportedChains: Chain[];
  supportsRoute(from: Chain, to: Chain): boolean;
  getQuote(params: QuoteParams): Promise<BridgeQuote>;
  execute(quote: BridgeQuote, signer: Signer): Promise<string>;
  getStatus(txHash: string): Promise<BridgeStatus>;
  getHealth(): Promise<BridgeHealth>;
}
```

The `AbstractBridgeAdapter` base class provides shared logic for fee
computation, slippage application, liquidity estimation, and transaction
hash generation.

## Design Decisions

### Why minimax instead of expected value?

Expected-value optimization averages over outcomes. In cross-chain routing,
tail risks (bridge failures, MEV extraction, gas spikes) are correlated and
can compound across hops. Minimax guarantees the best **worst-case** outcome,
making it suitable for users who need certainty about minimum received amounts.

### Why multi-language?

- Rust for the engine: zero-cost abstractions, no GC pauses, critical for
  latency-sensitive search.
- TypeScript for the SDK: most DeFi developers work in TypeScript. Bridge
  adapter implementations benefit from async/await and the npm ecosystem.
- Python for research: Monte Carlo simulations, data analysis, and rapid
  prototyping are natural in Python.

### Why alpha-beta pruning?

Without pruning, minimax explores O(b^d) nodes where b is the branching
factor and d is the search depth. Alpha-beta reduces this to O(b^(d/2))
in the best case, making deeper searches feasible within timeout budgets.

### Why five scoring dimensions?

A single "cost" metric conflates distinct risks. A route can be cheap but
slow, or fast but exposed to MEV. Five dimensions let users express
preferences via strategy weights while the engine optimizes the composite.
