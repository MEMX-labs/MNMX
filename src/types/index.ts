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
export interface Token {
  /** Ticker symbol, e.g. "USDC" */
  symbol: string;
  /** The chain this token lives on */
  chain: Chain;
  /** Number of decimals the token uses */
  decimals: number;
  /** Contract address (or mint address on Solana) */
  address: string;
  /** Optional human-readable name */
  name?: string;
  /** Optional logo URI */
  logoUri?: string;
  /** Whether this is the chain's native/gas token */
  isNative?: boolean;
}

/**
 * Configuration for a blockchain network.
 */
export interface ChainConfig {
  /** RPC endpoint URL */
  rpc: string;
  /** Block explorer base URL */
  blockExplorer: string;
  /** Native currency symbol */
  nativeCurrency: string;
  /** Numeric chain ID (EVM) or 0 for non-EVM */
  chainId: number;
  /** Average block time in seconds */
  avgBlockTime: number;
  /** Estimated finality time in seconds */
  finalityTime: number;
  /** Whether the chain is EVM-compatible */
  isEvm: boolean;
  /** Common tokens on this chain */
  tokens?: Token[];
}

// ─────────────────────────────────────────────────────────────
// Route Interfaces
// ─────────────────────────────────────────────────────────────

/**
 * A single hop in a cross-chain route.
 */
export interface RouteHop {
  /** Source chain */
  fromChain: Chain;
  /** Destination chain */
  toChain: Chain;
  /** Token being sent */
  fromToken: Token;
  /** Token being received */
  toToken: Token;
  /** Bridge used for this hop */
  bridge: string;
  /** Amount of fromToken going in (human-readable string) */
  inputAmount: string;
  /** Amount of toToken coming out (human-readable string) */
  outputAmount: string;
  /** Fee amount in fromToken (human-readable string) */
  fee: string;
  /** Estimated time for this hop in seconds */
  estimatedTime: number;
  /** Slippage in basis points for this hop */
  slippageBps: number;
  /** Liquidity depth at the time of quoting */
  liquidityDepth: number;
}
