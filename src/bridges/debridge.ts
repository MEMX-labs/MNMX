// ─────────────────────────────────────────────────────────────
// deBridge (DLN) Bridge Adapter
// ─────────────────────────────────────────────────────────────

import type {
  Chain,
  BridgeQuote,
  BridgeHealth,
  BridgeStatus,
  QuoteParams,
  Signer,
} from '../types/index.js';
import { AbstractBridgeAdapter } from './adapter.js';

/**
 * deBridge chain identifiers used in the DLN protocol.
 */
const DEBRIDGE_CHAIN_IDS: Partial<Record<Chain, number>> = {
  ethereum: 1,
  solana: 7565164,
  arbitrum: 42161,
  bnb: 56,
  polygon: 137,
};

/**
 * DLN taker margin configuration by chain.
 * Takers compete to fill orders; margin varies by chain activity.
 */
const DLN_TAKER_MARGINS: Partial<Record<Chain, number>> = {
  ethereum: 8,    // 8 bps
  solana: 5,      // 5 bps
  arbitrum: 6,    // 6 bps
  bnb: 7,         // 7 bps
  polygon: 6,     // 6 bps
};

/**
 * deBridge adapter implementing the DLN (DeBridge Liquidity Network) protocol.
 * DLN uses an intent-based model where market makers (takers) compete to fill
 * cross-chain orders, resulting in competitive rates.
 */
export class DeBridgeAdapter extends AbstractBridgeAdapter {
  readonly name = 'debridge';
  readonly supportedChains: Chain[] = [
    'ethereum', 'solana', 'arbitrum', 'bnb', 'polygon',
  ];

  /** Protocol fee in basis points */
  private protocolFeeBps = 4;
  /** Fixed infrastructure fee in USD */
  private infrastructureFeeUsd = 1.0;

  /**
   * Get deBridge chain ID.
   */
  private getDeBridgeChainId(chain: Chain): number {
    const id = DEBRIDGE_CHAIN_IDS[chain];
    if (id === undefined) throw new Error(`deBridge does not support chain: ${chain}`);
    return id;
  }

  /**
   * Compute the taker margin for a destination chain.
   * Takers take a margin to cover their execution costs.
   */
  private getTakerMarginBps(toChain: Chain): number {
    return DLN_TAKER_MARGINS[toChain] ?? 8;
  }

  /**
   * Estimate the DLN order fill time.
   * DLN orders are typically filled very quickly by competing takers.
   */
  private estimateFillTime(fromChain: Chain, toChain: Chain): number {
    // DLN is fast because takers pre-fund the destination
    let baseTime = 30; // seconds for order placement

    // Source chain finality affects how quickly takers can verify
    if (fromChain === 'ethereum') baseTime += 180;
    else if (fromChain === 'solana') baseTime += 15;
    else baseTime += 60; // L2s

    // Destination chain execution time
    if (toChain === 'ethereum') baseTime += 60;
    else if (toChain === 'solana') baseTime += 10;
    else baseTime += 20;

    // Add some variance for taker competition time
    baseTime += Math.floor(Math.random() * 30);
    return baseTime;
  }

  /**
   * Compute the total DLN fee structure.
   */
  private computeDlnFees(
    fromChain: Chain,
    toChain: Chain,
    inputAmount: number
  ): { fee: number; slippageBps: number; takerMarginBps: number } {
    // Protocol fee
    const protocolFee = inputAmount * (this.protocolFeeBps / 10000);

    // Taker margin (market-maker spread)
    const takerMarginBps = this.getTakerMarginBps(toChain);
    const takerFee = inputAmount * (takerMarginBps / 10000);

    // Infrastructure fee (covers relayer costs)
    const infraFee = this.infrastructureFeeUsd;

    // Gas subsidy fee for destination chain execution
    let gasSubsidy = 0;
    if (toChain === 'ethereum') gasSubsidy = 3.0;
    else if (toChain === 'bnb') gasSubsidy = 0.3;
    else gasSubsidy = 0.2;

    const totalFee = protocolFee + takerFee + infraFee + gasSubsidy;

    // DLN has minimal slippage because takers guarantee the output
    const slippageBps = Math.min(
      Math.floor((inputAmount / 10000000) * 5) + 1,
      20
    );

    return { fee: totalFee, slippageBps, takerMarginBps };
  }

  async getQuote(params: QuoteParams): Promise<BridgeQuote> {
    if (!this.supportsRoute(params.fromChain, params.toChain)) {
      throw new Error(
        `deBridge does not support ${params.fromChain} -> ${params.toChain}`
      );
    }

    const inputAmount = parseFloat(params.amount);
    if (isNaN(inputAmount) || inputAmount <= 0) {
      throw new Error('Invalid input amount');
    }

    const { fee, slippageBps, takerMarginBps } = this.computeDlnFees(
      params.fromChain,
      params.toChain,
      inputAmount
    );

    const afterFee = inputAmount - fee;
    const outputAmount = this.applySlippage(afterFee, slippageBps);
    const estimatedTime = this.estimateFillTime(params.fromChain, params.toChain);
    const liquidityDepth = this.estimateLiquidity(
      params.fromChain,
      params.toChain,
      8000000
    );

    return {
      bridge: this.name,
      inputAmount: inputAmount.toFixed(6),
      outputAmount: Math.max(0, outputAmount).toFixed(6),
      fee: fee.toFixed(6),
      estimatedTime,
      liquidityDepth,
      expiresAt: Date.now() + 30000, // DLN quotes are shorter-lived
      slippageBps,
      metadata: {
        deBridgeFromChainId: this.getDeBridgeChainId(params.fromChain),
        deBridgeToChainId: this.getDeBridgeChainId(params.toChain),
        protocolFeeBps: this.protocolFeeBps,
        takerMarginBps,
        orderType: 'DLN_TRADE',
      },
    };
  }

  async execute(quote: BridgeQuote, _signer: Signer): Promise<string> {
    if (Date.now() > quote.expiresAt) {
      throw new Error('deBridge quote has expired');
    }
    // In production, this would:
    // 1. Create a DLN order via the DlnSource contract
    // 2. Takers monitor and compete to fill the order
    // 3. Taker calls DlnDestination.fulfillOrder on the destination chain
    // 4. Once source chain finalizes, taker claims their funds
    return this.generateTxHash();
  }

  async getStatus(txHash: string): Promise<BridgeStatus> {
    // Simulate DLN order status
    // In production: check via deBridge API
    const hashNum = parseInt(txHash.slice(2, 10), 16);
    const elapsed = Date.now() % 5000;
    if (elapsed < 1000) return 'pending';
    if (elapsed < 2000) return 'confirming';
    if (hashNum % 100 < 98) return 'completed';
    return 'failed';
  }

  async getHealth(): Promise<BridgeHealth> {
    // DLN has very high reliability due to taker competition
    return {
      online: true,
      congestion: Math.random() * 0.1,
      recentSuccessRate: 0.98 + Math.random() * 0.02,
      medianConfirmTime: 120 + Math.floor(Math.random() * 60),
      lastChecked: Date.now(),
      pendingTxCount: Math.floor(Math.random() * 20),
    };
  }
}
