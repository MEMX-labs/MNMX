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

/**
 * A complete route from source to destination.
 */
export interface Route {
  /** Ordered list of hops */
  path: RouteHop[];
  /** Expected output amount (human-readable string) */
  expectedOutput: string;
  /** Guaranteed minimum output after adversarial modeling */
  guaranteedMinimum: string;
  /** Total fees across all hops (in input token equivalent) */
  totalFees: string;
  /** Total estimated time in seconds */
  estimatedTime: number;
  /** Minimax score (higher is better) */
  minimaxScore: number;
  /** Strategy used to find this route */
  strategy: Strategy;
  /** Unique identifier for this route */
  routeId: string;
  /** Timestamp when this route was computed */
  computedAt: number;
  /** Expiry timestamp (quotes go stale) */
  expiresAt: number;
}

/**
 * Describes the source and destination for a route request.
 */
export interface RouteEndpoint {
  /** The blockchain network */
  chain: Chain;
  /** Token symbol or address */
  token: string;
  /** Amount (only required for source) */
  amount?: string;
}

/**
 * A request to find a route.
 */
export interface RouteRequest {
  /** Source chain, token, and amount */
  from: RouteEndpoint & { amount: string };
  /** Destination chain and token */
  to: RouteEndpoint;
  /** Optional routing configuration overrides */
  options?: RouteOptions;
}

/**
 * Optional overrides for route finding.
 */
export interface RouteOptions {
  /** Routing strategy */
  strategy?: Strategy;
  /** Maximum number of hops */
  maxHops?: number;
  /** Slippage tolerance in basis points */
  slippageTolerance?: number;
  /** Timeout for route search in milliseconds */
  timeout?: number;
  /** Bridges to exclude */
  excludeBridges?: string[];
  /** Chains to exclude from intermediate hops */
  excludeChains?: Chain[];
  /** Minimum liquidity depth required */
  minLiquidity?: number;
  /** Custom scoring weights */
  weights?: Partial<ScoringWeights>;
  /** Custom adversarial model */
  adversarialModel?: Partial<AdversarialModel>;
}

// ─────────────────────────────────────────────────────────────
// Scoring & Adversarial Model
// ─────────────────────────────────────────────────────────────

/**
 * Weights for scoring different route properties.
 * All values should sum to 1.0 for normalized scoring.
 */
export interface ScoringWeights {
  /** Weight for fee minimization (0-1) */
  fees: number;
  /** Weight for slippage minimization (0-1) */
  slippage: number;
  /** Weight for speed (0-1) */
  speed: number;
  /** Weight for reliability (0-1) */
  reliability: number;
  /** Weight for MEV exposure minimization (0-1) */
  mevExposure: number;
}

/**
 * Adversarial model parameters for minimax worst-case analysis.
 * Values > 1.0 make the model more pessimistic.
 */
export interface AdversarialModel {
  /** Multiplier applied to expected slippage */
  slippageMultiplier: number;
  /** Multiplier applied to gas costs */
  gasMultiplier: number;
  /** Multiplier applied to bridge delay estimates */
  bridgeDelayMultiplier: number;
  /** Fraction of value extractable by MEV (0-1) */
  mevExtraction: number;
  /** Expected adverse price movement during execution (fraction) */
  priceMovement: number;
  /** Probability of bridge failure per hop (0-1) */
  failureProbability: number;
}

// ─────────────────────────────────────────────────────────────
// Router Configuration
// ─────────────────────────────────────────────────────────────

/**
 * Full router configuration.
 */
export interface RouterConfig {
  /** Default routing strategy */
  strategy: Strategy;
  /** Default slippage tolerance in basis points */
  slippageTolerance: number;
  /** Route search timeout in milliseconds */
  timeout: number;
  /** Maximum hops allowed */
  maxHops: number;
  /** List of bridge names to use (empty = all registered) */
  bridges: string[];
  /** List of bridge names to exclude */
  excludeBridges: string[];
  /** Scoring weights */
  weights: ScoringWeights;
  /** Adversarial model parameters */
  adversarialModel: AdversarialModel;
  /** Supported chains configuration */
  chains: Partial<Record<Chain, Partial<ChainConfig>>>;
  /** Minimum liquidity depth */
  minLiquidity: number;
  /** Log level */
  logLevel: LogLevel;
  /** Quote validity duration in milliseconds */
  quoteValidityMs: number;
}

// ─────────────────────────────────────────────────────────────
// Bridge Interfaces
// ─────────────────────────────────────────────────────────────

/**
 * Parameters for requesting a bridge quote.
 */
export interface QuoteParams {
  /** Source chain */
  fromChain: Chain;
  /** Destination chain */
  toChain: Chain;
  /** Token being sent */
  fromToken: Token;
  /** Token being received */
  toToken: Token;
  /** Amount to bridge (human-readable) */
  amount: string;
  /** Slippage tolerance in basis points */
  slippageTolerance: number;
  /** Sender address */
  senderAddress?: string;
  /** Recipient address */
  recipientAddress?: string;
}

/**
 * A quote from a bridge protocol.
 */
export interface BridgeQuote {
  /** Bridge name */
  bridge: string;
  /** Input amount (human-readable) */
  inputAmount: string;
  /** Output amount after fees and slippage (human-readable) */
  outputAmount: string;
  /** Fee amount in input token (human-readable) */
  fee: string;
