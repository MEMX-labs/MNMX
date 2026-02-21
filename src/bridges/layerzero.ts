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
