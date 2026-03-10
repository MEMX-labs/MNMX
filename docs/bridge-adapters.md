# Bridge Adapters

## Supported Bridges

| Bridge      | Mechanism              | Chains                                           | Typical Fee | Typical Time |
|-------------|------------------------|--------------------------------------------------|-------------|--------------|
| Wormhole    | Guardian network (19 validators, 13 required) | ETH, SOL, ARB, BASE, POLY, OPT, AVAX | 15-25 bps   | 600-960s     |
| deBridge    | Intent-based (DLN taker network)              | ETH, SOL, ARB, BNB, POLY              | 12-20 bps   | 120-300s     |
| LayerZero   | Ultra-light node relayers                      | ETH, ARB, BASE, POLY, BNB, OPT, AVAX  | 10-20 bps   | 300-600s     |
| Allbridge   | Liquidity pool messaging                       | ETH, SOL, ARB, POLY, BNB, AVAX        | 20-35 bps   | 180-480s     |

### Wormhole

Wormhole uses a network of 19 guardian validators. A transfer requires 13/19
signatures on a Verifiable Action Approval (VAA). This provides strong security
guarantees but adds latency for guardian attestation.

Fee structure:
- Protocol fee: 15-25 bps depending on chain pair type (EVM-EVM, EVM-Solana).
- Relayer fee: $2-7 base fee + 0.01% of amount. Higher for L1 destinations.
- Guardian fee: $0.50 flat.

### deBridge (DLN)

deBridge uses the DeBridge Liquidity Network (DLN), an intent-based model where
market makers (takers) compete to fill cross-chain orders. This results in fast
fill times since takers pre-fund the destination.

Fee structure:
- Protocol fee: 4 bps.
- Taker margin: 5-8 bps depending on destination chain.
- Infrastructure fee: $1.00 flat.
- Gas subsidy: $0.20-3.00 depending on destination.

### LayerZero

LayerZero uses ultra-light nodes with configurable security via independent
oracles and relayers. Supports omnichain fungible tokens (OFT).

### Allbridge

Allbridge uses liquidity pools with cross-chain messaging. Each chain has
a pool, and transfers are rebalanced across pools.

## BridgeAdapter Interface

All bridge integrations implement the `BridgeAdapter` interface:

```typescript
interface BridgeAdapter {
  readonly name: string;
  readonly supportedChains: Chain[];

  supportsRoute(fromChain: Chain, toChain: Chain): boolean;
  getQuote(params: QuoteParams): Promise<BridgeQuote>;
  execute(quote: BridgeQuote, signer: Signer): Promise<string>;
  getStatus(txHash: string): Promise<BridgeStatus>;
  getHealth(): Promise<BridgeHealth>;
}
```

The `AbstractBridgeAdapter` base class provides:

- `supportsRoute()` -- Default implementation checking both chains are in
  `supportedChains` and are different.
- `computeBaseFee()` -- Proportional fee with minimum floor.
- `applySlippage()` -- Apply basis-point slippage to an amount.
- `estimateLiquidity()` -- Estimate depth based on chain pair and jitter.
- `generateTxHash()` -- Create a simulated 0x-prefixed hex hash.
- `getStatus()` -- Default probabilistic status simulation.
- `getHealth()` -- Default health with randomized metrics.

## How to Add a New Bridge

1. **Create a file** in `src/bridges/` (e.g., `src/bridges/mybridge.ts`).

2. **Extend `AbstractBridgeAdapter`**:

```typescript
import { AbstractBridgeAdapter } from './adapter.js';
import type { Chain, QuoteParams, BridgeQuote, Signer } from '../types/index.js';

export class MyBridgeAdapter extends AbstractBridgeAdapter {
  readonly name = 'mybridge';
  readonly supportedChains: Chain[] = ['ethereum', 'arbitrum', 'base'];

  async getQuote(params: QuoteParams): Promise<BridgeQuote> {
    if (!this.supportsRoute(params.fromChain, params.toChain)) {
      throw new Error(`MyBridge does not support this route`);
    }
    const inputAmount = parseFloat(params.amount);
    const fee = this.computeBaseFee(inputAmount, 15, 0.25);
    const output = this.applySlippage(inputAmount - fee, 5);
    return {
      bridge: this.name,
      inputAmount: inputAmount.toFixed(6),
      outputAmount: output.toFixed(6),
      fee: fee.toFixed(6),
      estimatedTime: 300,
      liquidityDepth: this.estimateLiquidity(params.fromChain, params.toChain, 5000000),
      expiresAt: Date.now() + 60000,
      slippageBps: 5,
    };
  }

  async execute(quote: BridgeQuote, signer: Signer): Promise<string> {
    if (Date.now() > quote.expiresAt) throw new Error('Quote expired');
    return this.generateTxHash();
  }
}
```

3. **Export from `src/bridges/index.ts`**:

```typescript
export { MyBridgeAdapter } from './mybridge.js';
```

4. **Register with the router**:

```typescript
router.registerBridge(new MyBridgeAdapter());
```

5. **Add tests** in `tests/bridges/mybridge.test.ts`.

## Health Monitoring

The `getHealth()` method returns:

```typescript
interface BridgeHealth {
  online: boolean;           // Is the bridge operational?
  congestion: number;        // 0.0 (none) to 1.0 (full)
  recentSuccessRate: number; // 0.0 to 1.0
  medianConfirmTime: number; // seconds
  lastChecked: number;       // timestamp ms
  pendingTxCount: number;    // number of pending transactions
}
```

Health data feeds into the reliability dimension of route scoring. A bridge
with `recentSuccessRate < 0.90` will be penalized. A bridge with
`online: false` is excluded from route discovery entirely.

## Bridge Selection Logic

During path discovery, the engine selects bridges using these criteria:

1. **Supported route**: The bridge must support the specific chain pair.
2. **Not excluded**: The bridge must not be in `excludeBridges`.
3. **Liquidity**: The bridge must have sufficient liquidity (above `minLiquidity`).
4. **Health**: Unhealthy bridges (offline or very high congestion) are deprioritized.

When multiple bridges support the same hop, all are included as candidates.
The minimax search evaluates each combination and selects the globally optimal
set of bridges across all hops.
