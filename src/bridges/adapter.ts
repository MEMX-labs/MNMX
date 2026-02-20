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
