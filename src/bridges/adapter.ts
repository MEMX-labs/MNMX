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
import { createHash } from 'crypto';

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

  /** Cached health data with TTL */
  private healthCache: { data: BridgeHealth; expiresAt: number } | null = null;
  private static readonly HEALTH_CACHE_TTL = 30_000;

  supportsRoute(fromChain: Chain, toChain: Chain): boolean {
    return (
      fromChain !== toChain &&
      this.supportedChains.includes(fromChain) &&
      this.supportedChains.includes(toChain)
    );
  }

  abstract getQuote(params: QuoteParams): Promise<BridgeQuote>;
  abstract execute(quote: BridgeQuote, signer: Signer): Promise<string>;

  /**
   * Query the bridge API for transaction status.
   * Subclasses should override with protocol-specific API calls.
   */
  abstract getStatus(txHash: string): Promise<BridgeStatus>;

  /**
   * Fetch bridge health metrics. Uses a short-lived cache to avoid
   * overwhelming bridge APIs with repeated health checks during
   * route scoring.
   */
  async getHealth(): Promise<BridgeHealth> {
    if (this.healthCache && Date.now() < this.healthCache.expiresAt) {
      return this.healthCache.data;
    }
    const health = await this.fetchHealth();
    this.healthCache = {
      data: health,
      expiresAt: Date.now() + AbstractBridgeAdapter.HEALTH_CACHE_TTL,
    };
    return health;
  }

  /** Fetch fresh health data from bridge API. Override in subclasses. */
  protected abstract fetchHealth(): Promise<BridgeHealth>;

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
   * Derive a deterministic transaction hash from quote data and timestamp.
   * Used as a placeholder until the actual on-chain tx hash is available.
   */
  protected deriveTxHash(quote: BridgeQuote): string {
    const input = `${quote.bridge}:${quote.inputAmount}:${quote.outputAmount}:${Date.now()}`;
    const hash = createHash('sha256').update(input).digest('hex');
    return '0x' + hash;
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
    return baseLiquidity * fromMultiplier * toMultiplier;
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
