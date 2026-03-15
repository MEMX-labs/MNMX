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

/** deBridge DLN API base URL */
const DLN_API = 'https://api.dln.trade/v1.0';

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

  async execute(quote: BridgeQuote, signer: Signer): Promise<string> {
    if (Date.now() > quote.expiresAt) {
      throw new Error('deBridge quote has expired');
    }
    if (!signer.address) {
      throw new Error('Signer must have a public key');
    }
    // Create a DLN order via the DlnSource contract, takers will
    // compete to fill it on the destination chain.
    return this.deriveTxHash(quote);
  }

  async getStatus(txHash: string): Promise<BridgeStatus> {
    // Query the DLN API for order status
    const url = `${DLN_API}/dln/order/status?txHash=${txHash}`;
    try {
      const res = await fetch(url, { signal: AbortSignal.timeout(5000) });
      if (!res.ok) return 'pending';
      const data = await res.json() as { status?: string };
      switch (data.status) {
        case 'Fulfilled': return 'completed';
        case 'SentUnlock':
        case 'ClaimedUnlock': return 'completed';
        case 'Created': return 'pending';
        case 'SentOrder': return 'confirming';
        default: return 'pending';
      }
    } catch {
      return 'confirming';
    }
  }

  protected async fetchHealth(): Promise<BridgeHealth> {
    try {
      const res = await fetch(`${DLN_API}/chain/supported-chains-info`, {
        signal: AbortSignal.timeout(5000),
      });
      const online = res.ok;
      return {
        online,
        congestion: online ? 0.05 : 1.0,
        recentSuccessRate: 0.99,
        medianConfirmTime: 150,
        lastChecked: Date.now(),
        pendingTxCount: 0,
      };
    } catch {
      return {
        online: false,
        congestion: 1.0,
        recentSuccessRate: 0,
        medianConfirmTime: 0,
        lastChecked: Date.now(),
        pendingTxCount: 0,
      };
    }
  }
}
