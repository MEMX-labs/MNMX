// ─────────────────────────────────────────────────────────────
// Wormhole Bridge Adapter
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
 * Wormhole chain ID mapping for cross-chain messaging.
 */
const WORMHOLE_CHAIN_IDS: Partial<Record<Chain, number>> = {
  ethereum: 2,
  solana: 1,
  arbitrum: 23,
  base: 30,
  polygon: 5,
  optimism: 24,
  avalanche: 6,
};

/**
 * Wormhole-specific fee tiers by chain pair type.
 */
const WORMHOLE_FEE_TIERS: Record<string, number> = {
  'evm-evm': 15,       // 15 bps for EVM-to-EVM
  'evm-solana': 25,     // 25 bps for EVM-to-Solana
  'solana-evm': 25,     // 25 bps for Solana-to-EVM
};

/**
 * Estimated bridge times in seconds by chain pair.
 */
const WORMHOLE_TIMES: Record<string, number> = {
  'evm-evm': 900,
  'evm-solana': 600,
  'solana-evm': 900,
};

/**
 * Wormhole bridge adapter.
 * Implements cross-chain token transfers via Wormhole's guardian network.
 */
export class WormholeAdapter extends AbstractBridgeAdapter {
  readonly name = 'wormhole';
  readonly supportedChains: Chain[] = [
    'ethereum', 'solana', 'arbitrum', 'base', 'polygon', 'optimism', 'avalanche',
  ];

  private guardianFeeUsd = 0.50;
  private relayerBaseFeeUsd = 2.00;

  /**
   * Get the Wormhole chain ID for a chain.
   */
  private getWormholeChainId(chain: Chain): number {
    const id = WORMHOLE_CHAIN_IDS[chain];
    if (id === undefined) throw new Error(`Wormhole does not support chain: ${chain}`);
    return id;
  }

  /**
   * Determine the chain type (evm or solana).
   */
  private getChainType(chain: Chain): 'evm' | 'solana' {
    return chain === 'solana' ? 'solana' : 'evm';
  }

  /**
   * Calculate the fee tier for a given chain pair.
   */
  private getFeeTierBps(fromChain: Chain, toChain: Chain): number {
    const fromType = this.getChainType(fromChain);
    const toType = this.getChainType(toChain);
    const key = `${fromType}-${toType}`;
    return WORMHOLE_FEE_TIERS[key] ?? 20;
  }

  /**
   * Calculate the estimated bridge time.
   */
