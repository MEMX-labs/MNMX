# Alpha-Beta Pruning in MNMX

## Overview

MNMX uses alpha-beta pruning to efficiently search the game tree of possible
cross-chain routes. This reduces the number of adversarial scenarios that need
to be evaluated without affecting the optimality of the result.

## How It Works

In standard minimax search, every leaf node must be evaluated. Alpha-beta
pruning maintains two bounds:

- **Alpha**: the best score the maximizer (route selector) can guarantee
- **Beta**: the best score the minimizer (adversarial model) can guarantee

When `alpha >= beta`, the remaining branches cannot affect the final decision
and are pruned.

## Pruning in Route Evaluation

For each candidate route, the engine simulates 5 adversarial scenarios:

1. **Fee spike** — bridge fees increase by 2-5x
2. **Slippage surge** — slippage exceeds estimate by 3x
3. **Delay** — execution time doubles
4. **Liquidity drain** — available liquidity drops 80%
5. **MEV extraction** — sandwich attack on the transaction

If a route's worst-case score under scenario 1-2 already falls below the
current best route's worst-case, scenarios 3-5 are skipped (pruned).

## Performance

On a typical search with 8+ candidate routes and 5 scenarios each:

- Without pruning: 40+ evaluations
- With pruning: 15-25 evaluations (40-60% reduction)

The pruning ratio improves as more routes are evaluated, since alpha tightens
with each better route found.
