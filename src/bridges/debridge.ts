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

