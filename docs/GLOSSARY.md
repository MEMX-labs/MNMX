# Glossary

## Alpha-Beta Pruning
An optimization technique for minimax search that eliminates branches
that cannot influence the final decision. Reduces search space by 90%+
without affecting the optimal result.

## Adversarial Model
A mathematical model of worst-case market conditions used by the minimax
engine to estimate the guaranteed minimum outcome of a route.

## Bridge Adapter
A modular integration layer that connects the MNMX routing engine to a
specific cross-chain bridge protocol (e.g., Wormhole, deBridge).

## Guaranteed Minimum
The worst-case output of a route under the adversarial model. This is
the value that minimax optimization maximizes.

## MEV (Maximal Extractable Value)
Value that can be extracted from users by reordering, inserting, or
censoring transactions. MNMX models MEV as an adversarial cost.

## Minimax
A decision rule that minimizes the possible loss for a worst-case
scenario. Originally from game theory, applied here to route selection.

## Route Hop
A single segment of a cross-chain route, typically involving one bridge
transfer between two chains.

## Scoring Weights
Five configurable parameters (fees, slippage, speed, reliability, MEV
exposure) that control how routes are evaluated and compared.

## Slippage
The difference between the expected output and the actual output of a
token transfer, caused by price movement during execution.

## Strategy Profile
A preset configuration of scoring weights optimized for a specific use
case: minimax (default), cheapest, fastest, or safest.

## Transposition Table
A cache that stores previously evaluated positions to avoid redundant
computation during minimax search.
