// ─────────────────────────────────────────────────────────────
// MNMX Core Types
// Cross-chain routing protocol type definitions
// ─────────────────────────────────────────────────────────────

/**
 * Supported blockchain networks.
 */
export type Chain =
  | 'ethereum'
  | 'solana'
  | 'arbitrum'
  | 'base'
  | 'polygon'
  | 'bnb'
  | 'optimism'
  | 'avalanche';

/**
 * All supported chains as a readonly array for runtime checks.
 */
export const ALL_CHAINS: readonly Chain[] = [
  'ethereum',
  'solana',
  'arbitrum',
  'base',
  'polygon',
  'bnb',
  'optimism',
  'avalanche',
] as const;

/**
 * Routing strategy selection.
 * - minimax: game-tree search for best guaranteed minimum outcome
 * - cheapest: minimize total fees
 * - fastest: minimize total estimated time
 * - safest: maximize reliability scores
 */
export type Strategy = 'minimax' | 'cheapest' | 'fastest' | 'safest';

/**
 * Bridge transaction status.
 */
export type BridgeStatus = 'pending' | 'confirming' | 'completed' | 'failed';

/**
 * Log severity levels.
 */
export enum LogLevel {
  Debug = 0,
  Info = 1,
  Warn = 2,
  Error = 3,
  Silent = 4,
}

// ─────────────────────────────────────────────────────────────
// Token & Chain Interfaces
// ─────────────────────────────────────────────────────────────

/**
 * Represents a token on a specific chain.
 */
