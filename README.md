# MNMX Core

[![CI](https://img.shields.io/github/actions/workflow/status/mnmx-protocol/mnmx-core/ci.yml?branch=main&style=flat-square&label=build&color=1a1a2e)](https://github.com/mnmx-protocol/mnmx-core/actions)
[![License](https://img.shields.io/badge/license-MIT-1a1a2e?style=flat-square)](./LICENSE)
[![TypeScript](https://img.shields.io/badge/TypeScript-strict-1a1a2e?style=flat-square)](https://www.typescriptlang.org/)
[![Solana](https://img.shields.io/badge/Solana-mainnet--beta-1a1a2e?style=flat-square)](https://solana.com/)
[![npm version](https://img.shields.io/npm/v/@mnmx/core?style=flat-square&color=1a1a2e)](https://www.npmjs.com/package/@mnmx/core)

---

**Minimax execution engine for autonomous on-chain agents.**

MNMX applies adversarial game-tree search to on-chain execution. The core insight is that DeFi transactions operate in an adversarial environment -- MEV bots watch the mempool and act against you. This is structurally identical to a two-player zero-sum game: your agent is the maximizer, MEV bots are the minimizers.

The same minimax algorithm that powers chess engines can determine the optimal sequence of on-chain actions when the opponent is a rational, profit-seeking adversary. MNMX implements this with alpha-beta pruning, iterative deepening, transposition tables, and move ordering heuristics -- the same toolkit that took game engines from brute force to superhuman play.

The result: execution plans that are provably optimal against rational adversaries, not just heuristically "good enough."

## Architecture

```mermaid
graph TD
    A[OnChainState] --> B[GameTreeBuilder]
    B --> C[MinimaxEngine]
    C --> D[ExecutionPlan]
    D --> E[PlanExecutor]

    C --> F[PositionEvaluator]
    C --> G[MoveOrderer]
    C --> H[TranspositionTable]
    C --> I[MevDetector]

    B --> J[StateReader]
    E --> J

    subgraph Engine Core
        C
        F
        G
        H
    end
