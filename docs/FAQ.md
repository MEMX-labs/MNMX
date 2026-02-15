# FAQ

## What is minimax optimization?

Minimax is a decision strategy from game theory that finds the move
which maximizes the minimum possible outcome. In routing terms: it
finds the route with the best worst-case result.

## How is this different from other bridge aggregators?

Most aggregators optimize for expected value (best average case).
MNMX optimizes for guaranteed minimum (best worst case). This
produces more predictable outcomes, especially for large transfers.

## Which chains are supported?

Ethereum, Solana, Arbitrum, Base, Polygon, BNB Chain, Optimism,
and Avalanche.

## Which bridges does MNMX use?

Wormhole, deBridge, LayerZero, and Allbridge. Custom bridges can
be added by implementing the BridgeAdapter interface.

## Can I add my own bridge?

Yes. Implement the `BridgeAdapter` interface and register it with
`router.registerBridge()`. See the Bridge Adapters documentation.

## What are the strategy options?

- **minimax** (default): Best guaranteed minimum outcome
- **cheapest**: Lowest total fees
- **fastest**: Shortest transfer time
- **safest**: Highest bridge reliability
