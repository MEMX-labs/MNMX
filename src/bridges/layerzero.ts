// ─────────────────────────────────────────────────────────────
// LayerZero Bridge Adapter
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
 * LayerZero V2 endpoint IDs.
 */
const LZ_ENDPOINT_IDS: Partial<Record<Chain, number>> = {
  ethereum: 30101,
  arbitrum: 30110,
  base: 30184,
  polygon: 30109,
  bnb: 30102,
  optimism: 30111,
  avalanche: 30106,
};

/**
 * LayerZero DVN (Decentralized Verifier Network) configuration.
 * Different security levels have different costs and speeds.
 */
interface DvnConfig {
  /** Number of DVNs required for verification */
  requiredDvns: number;
  /** Optional DVNs that can participate */
  optionalDvns: number;
  /** Optional DVN threshold */
  optionalThreshold: number;
  /** Base verification fee in USD */
  verificationFeeUsd: number;
}

const DEFAULT_DVN_CONFIG: DvnConfig = {
  requiredDvns: 2,
  optionalDvns: 3,
  optionalThreshold: 1,
  verificationFeeUsd: 0.75,
};

/**
 * LayerZero adapter implementing OFT (Omnichain Fungible Token) transfers.
 * Uses the LayerZero V2 messaging protocol with configurable DVN security.
 */
export class LayerZeroAdapter extends AbstractBridgeAdapter {
  readonly name = 'layerzero';
  readonly supportedChains: Chain[] = [
    'ethereum', 'arbitrum', 'base', 'polygon', 'bnb', 'optimism', 'avalanche',
  ];

  private dvnConfig: DvnConfig = DEFAULT_DVN_CONFIG;

  /**
   * Get the LayerZero endpoint ID for a chain.
   */
  private getEndpointId(chain: Chain): number {
    const id = LZ_ENDPOINT_IDS[chain];
    if (id === undefined) throw new Error(`LayerZero does not support chain: ${chain}`);
    return id;
  }

  /**
   * Compute LayerZero messaging fee.
   * LayerZero charges a messaging fee that covers DVN verification + executor gas.
   */
  private computeMessagingFee(
    fromChain: Chain,
    toChain: Chain,
    inputAmount: number
  ): number {
    // DVN verification fee
    const dvnFee = this.dvnConfig.verificationFeeUsd * this.dvnConfig.requiredDvns;

    // Executor fee (gas on destination chain)
    let executorFee = 0.5;
    if (toChain === 'ethereum') executorFee = 8.0;
    else if (toChain === 'bnb') executorFee = 0.3;
    else if (toChain === 'polygon') executorFee = 0.1;
    else executorFee = 0.4; // L2s

    // Treasury fee (protocol revenue)
    const treasuryFee = inputAmount * 0.0003; // 3 bps

    return dvnFee + executorFee + treasuryFee;
  }

  /**
   * Compute the OFT transfer fee (separate from messaging fee).
   */
  private computeOftFee(inputAmount: number): number {
    // OFT typically has minimal fees, mainly gas costs
    // Some OFT implementations charge a small fee
    return inputAmount * 0.0005; // 5 bps
  }

  /**
   * Estimate verification and delivery time.
   */
  private estimateDeliveryTime(fromChain: Chain, toChain: Chain): number {
    // Source chain finality wait
    let sourceFinality = 60;
    if (fromChain === 'ethereum') sourceFinality = 900;
    else if (fromChain === 'polygon') sourceFinality = 120;
    else sourceFinality = 60; // L2s are faster

    // DVN verification time (DVNs verify the message)
    const dvnVerification = 30 + this.dvnConfig.requiredDvns * 15;

    // Destination chain execution
    let destExecution = 30;
    if (toChain === 'ethereum') destExecution = 60;

    // Random jitter
    const jitter = Math.floor(Math.random() * 60);

    return sourceFinality + dvnVerification + destExecution + jitter;
  }

  async getQuote(params: QuoteParams): Promise<BridgeQuote> {
    if (!this.supportsRoute(params.fromChain, params.toChain)) {
      throw new Error(
        `LayerZero does not support ${params.fromChain} -> ${params.toChain}`
      );
    }

    const inputAmount = parseFloat(params.amount);
    if (isNaN(inputAmount) || inputAmount <= 0) {
      throw new Error('Invalid input amount');
    }

    const messagingFee = this.computeMessagingFee(
      params.fromChain,
      params.toChain,
      inputAmount
    );
    const oftFee = this.computeOftFee(inputAmount);
    const totalFee = messagingFee + oftFee;

    // LayerZero OFT has very low slippage since it's mint/burn
    const slippageBps = Math.min(
      Math.floor((inputAmount / 20000000) * 5) + 1,
      15
    );

    const afterFee = inputAmount - totalFee;
    const outputAmount = this.applySlippage(afterFee, slippageBps);
    const estimatedTime = this.estimateDeliveryTime(params.fromChain, params.toChain);
    const liquidityDepth = this.estimateLiquidity(
      params.fromChain,
      params.toChain,
      15000000
    );

    return {
      bridge: this.name,
      inputAmount: inputAmount.toFixed(6),
      outputAmount: Math.max(0, outputAmount).toFixed(6),
      fee: totalFee.toFixed(6),
      estimatedTime,
      liquidityDepth,
      expiresAt: Date.now() + 45000,
      slippageBps,
      metadata: {
        srcEndpointId: this.getEndpointId(params.fromChain),
        dstEndpointId: this.getEndpointId(params.toChain),
        requiredDvns: this.dvnConfig.requiredDvns,
        messagingFee: messagingFee.toFixed(6),
        oftFee: oftFee.toFixed(6),
        messageType: 'OFT_SEND',
      },
    };
  }

  async execute(quote: BridgeQuote, _signer: Signer): Promise<string> {
    if (Date.now() > quote.expiresAt) {
      throw new Error('LayerZero quote has expired');
    }
    // In production, this would:
    // 1. Encode the OFT send parameters (dstEid, to, amountLD, minAmountLD)
    // 2. Call quoteSend() to get the messaging fee
    // 3. Call send() on the OFT contract with the messaging fee as msg.value
    // 4. LayerZero DVNs verify the message
    // 5. Executor delivers and mints on destination
    return this.generateTxHash();
  }

  async getStatus(txHash: string): Promise<BridgeStatus> {
    // In production: check via LayerZero Scan API
    const hashNum = parseInt(txHash.slice(2, 10), 16);
    const progress = (Date.now() % 8000) / 8000;
    if (progress < 0.15) return 'pending';
    if (progress < 0.4) return 'confirming';
    if (hashNum % 100 < 98) return 'completed';
    return 'failed';
  }

  async getHealth(): Promise<BridgeHealth> {
    return {
      online: true,
      congestion: Math.random() * 0.12,
      recentSuccessRate: 0.98 + Math.random() * 0.02,
      medianConfirmTime: 300 + Math.floor(Math.random() * 180),
      lastChecked: Date.now(),
      pendingTxCount: Math.floor(Math.random() * 40),
    };
  }
}
