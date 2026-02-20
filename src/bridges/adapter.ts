// ─────────────────────────────────────────────────────────────
// Bridge Adapter Interface & Registry
// ─────────────────────────────────────────────────────────────

import type {
  Chain,
  BridgeQuote,
  BridgeHealth,
  BridgeStatus,
  QuoteParams,
  Signer,
} from '../types/index.js';

/**
 * Interface that all bridge adapters must implement.
 */
export interface BridgeAdapter {
  /** Unique bridge name */
  readonly name: string;

  /** Chains this bridge supports */
  readonly supportedChains: Chain[];

  /** Whether this bridge supports the given chain pair */
  supportsRoute(fromChain: Chain, toChain: Chain): boolean;

  /** Get a quote for bridging tokens */
  getQuote(params: QuoteParams): Promise<BridgeQuote>;

  /** Execute a bridge transfer */
  execute(quote: BridgeQuote, signer: Signer): Promise<string>;

  /** Check the status of a bridge transfer by tx hash */
  getStatus(txHash: string): Promise<BridgeStatus>;

  /** Get the current health of this bridge */
  getHealth(): Promise<BridgeHealth>;
}

/**
 * Abstract base class for bridge adapters with shared logic.
 */
export abstract class AbstractBridgeAdapter implements BridgeAdapter {
  abstract readonly name: string;
  abstract readonly supportedChains: Chain[];

  supportsRoute(fromChain: Chain, toChain: Chain): boolean {
    return (
      fromChain !== toChain &&
      this.supportedChains.includes(fromChain) &&
      this.supportedChains.includes(toChain)
    );
  }

  abstract getQuote(params: QuoteParams): Promise<BridgeQuote>;
  abstract execute(quote: BridgeQuote, signer: Signer): Promise<string>;

  async getStatus(_txHash: string): Promise<BridgeStatus> {
    // Simulate status progression
    const roll = Math.random();
    if (roll < 0.7) return 'completed';
    if (roll < 0.9) return 'confirming';
    if (roll < 0.97) return 'pending';
    return 'failed';
  }

  async getHealth(): Promise<BridgeHealth> {
    return {
      online: true,
      congestion: Math.random() * 0.3,
      recentSuccessRate: 0.95 + Math.random() * 0.05,
      medianConfirmTime: 60 + Math.floor(Math.random() * 240),
      lastChecked: Date.now(),
      pendingTxCount: Math.floor(Math.random() * 50),
    };
  }

  /**
   * Compute a base fee as a fraction of input amount.
   */
  protected computeBaseFee(
    inputAmount: number,
    feeRateBps: number,
    minFee: number
  ): number {
    const proportionalFee = inputAmount * (feeRateBps / 10000);
    return Math.max(proportionalFee, minFee);
  }

  /**
   * Apply slippage to get output amount.
   */
  protected applySlippage(
    amount: number,
    slippageBps: number
  ): number {
    return amount * (1 - slippageBps / 10000);
  }

  /**
   * Generate a simulated transaction hash.
   */
  protected generateTxHash(): string {
    const chars = '0123456789abcdef';
    let hash = '0x';
    for (let i = 0; i < 64; i++) {
      hash += chars[Math.floor(Math.random() * 16)];
    }
    return hash;
  }

  /**
   * Estimate liquidity depth for a token pair.
   */
  protected estimateLiquidity(
    fromChain: Chain,
    toChain: Chain,
    baseLiquidity: number
  ): number {
    const majorChains: Chain[] = ['ethereum', 'arbitrum', 'polygon'];
    const fromMultiplier = majorChains.includes(fromChain) ? 1.5 : 1.0;
    const toMultiplier = majorChains.includes(toChain) ? 1.5 : 1.0;
    const jitter = 0.8 + Math.random() * 0.4;
    return baseLiquidity * fromMultiplier * toMultiplier * jitter;
  }
}

/**
 * Registry for managing bridge adapters.
 */
export class BridgeRegistry {
  private adapters: Map<string, BridgeAdapter> = new Map();

  register(adapter: BridgeAdapter): void {
    this.adapters.set(adapter.name, adapter);
  }

  get(name: string): BridgeAdapter | undefined {
    return this.adapters.get(name);
  }

  getForPair(fromChain: Chain, toChain: Chain): BridgeAdapter[] {
    const result: BridgeAdapter[] = [];
    for (const adapter of this.adapters.values()) {
      if (adapter.supportsRoute(fromChain, toChain)) {
        result.push(adapter);
      }
    }
    return result;
  }

  getAll(): BridgeAdapter[] {
    return Array.from(this.adapters.values());
  }

  remove(name: string): boolean {
    return this.adapters.delete(name);
  }

  getNames(): string[] {
    return Array.from(this.adapters.keys());
  }

  has(name: string): boolean {
    return this.adapters.has(name);
  }
}
