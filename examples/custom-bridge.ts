// ─────────────────────────────────────────────────────────────
// Custom Bridge Adapter
// Demonstrates how to implement and register a custom bridge
// ─────────────────────────────────────────────────────────────
//
// Run: npx tsx examples/custom-bridge.ts

import { AbstractBridgeAdapter, BridgeRegistry } from '../src/bridges/adapter.js';
import { MnmxRouter } from '../src/router/index.js';
import { WormholeAdapter } from '../src/bridges/wormhole.js';
import type {
  Chain,
  QuoteParams,
  BridgeQuote,
  BridgeHealth,
  BridgeStatus,
  Signer,
  RouteRequest,
} from '../src/types/index.js';

/**
 * Example custom bridge adapter: "HyperBridge"
 * A hypothetical fast L2-to-L2 bridge with low fees but limited chain support.
 */
class HyperBridgeAdapter extends AbstractBridgeAdapter {
  readonly name = 'hyperbridge';
  readonly supportedChains: Chain[] = ['arbitrum', 'base', 'optimism'];

  private readonly feeRateBps = 8;
  private readonly minFeeUsd = 0.10;
  private readonly baseLiquidity = 2_000_000;
  private readonly baseTimeSeconds = 120;

  async getQuote(params: QuoteParams): Promise<BridgeQuote> {
    if (!this.supportsRoute(params.fromChain, params.toChain)) {
      throw new Error(
        `HyperBridge does not support ${params.fromChain} -> ${params.toChain}`
      );
    }

    const inputAmount = parseFloat(params.amount);
    if (isNaN(inputAmount) || inputAmount <= 0) {
      throw new Error('Invalid input amount');
    }

    const fee = this.computeBaseFee(inputAmount, this.feeRateBps, this.minFeeUsd);
    const afterFee = inputAmount - fee;
    const slippageBps = Math.min(Math.ceil(inputAmount / 1_000_000 * 5), 15);
    const outputAmount = this.applySlippage(afterFee, slippageBps);
    const liquidity = this.estimateLiquidity(
      params.fromChain,
      params.toChain,
      this.baseLiquidity,
    );

    return {
      bridge: this.name,
      inputAmount: inputAmount.toFixed(6),
      outputAmount: Math.max(0, outputAmount).toFixed(6),
      fee: fee.toFixed(6),
      estimatedTime: this.baseTimeSeconds,
      liquidityDepth: liquidity,
      expiresAt: Date.now() + 30_000,
      slippageBps,
      metadata: {
        protocol: 'hyperbridge',
        version: '2.0',
        settlementMechanism: 'optimistic-with-fraud-proof',
      },
    };
  }

  async execute(quote: BridgeQuote, _signer: Signer): Promise<string> {
    if (Date.now() > quote.expiresAt) {
      throw new Error('Quote expired');
    }
    return this.generateTxHash();
  }

  async getStatus(txHash: string): Promise<BridgeStatus> {
    const age = Date.now() % 300_000;
    if (age < 30_000) return 'pending';
    if (age < 90_000) return 'confirming';
    return 'completed';
  }

  async getHealth(): Promise<BridgeHealth> {
    return {
      online: true,
      congestion: 0.05,
      recentSuccessRate: 0.998,
      medianConfirmTime: this.baseTimeSeconds,
      lastChecked: Date.now(),
      pendingTxCount: 3,
    };
  }
}

async function main(): Promise<void> {
  const router = new MnmxRouter({ maxHops: 2 });

  // Register built-in bridges
  router.registerBridge(new WormholeAdapter());

  // Register custom bridge
  const hyperbridge = new HyperBridgeAdapter();
  router.registerBridge(hyperbridge);

  console.log('Registered bridges:', router.getSupportedBridges().join(', '));
  console.log('');

  // Test the custom bridge directly
  const quote = await hyperbridge.getQuote({
    fromChain: 'arbitrum',
    toChain: 'base',
    fromToken: { symbol: 'USDC', chain: 'arbitrum', decimals: 6, address: '0xarb_usdc' },
    toToken: { symbol: 'USDC', chain: 'base', decimals: 6, address: '0xbase_usdc' },
    amount: '1000',
    slippageTolerance: 50,
  });
  console.log('Direct HyperBridge quote (Arbitrum -> Base):');
  console.log(`  Input:  ${quote.inputAmount} USDC`);
  console.log(`  Output: ${quote.outputAmount} USDC`);
  console.log(`  Fee:    ${quote.fee} USDC`);
  console.log(`  Time:   ${quote.estimatedTime}s`);
  console.log('');

  // Use it in the router for L2-to-L2 routing
  const request: RouteRequest = {
    from: { chain: 'arbitrum', token: 'USDC', amount: '1000' },
    to: { chain: 'base', token: 'USDC' },
    options: { strategy: 'fastest' },
  };

  const result = await router.findRoute(request);
  if (result.bestRoute) {
    console.log('Best route for Arbitrum -> Base:');
    for (const hop of result.bestRoute.path) {
      console.log(`  ${hop.fromChain} -> ${hop.toChain} via ${hop.bridge} (${hop.estimatedTime}s)`);
    }
    console.log(`  Score: ${result.bestRoute.minimaxScore.toFixed(4)}`);
    console.log(`  Total candidates evaluated: ${result.stats.candidateCount}`);
  }
}

main().catch(console.error);
