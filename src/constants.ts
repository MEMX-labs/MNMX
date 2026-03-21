/** Maximum number of hops allowed in a single route. */
export const MAX_HOPS = 5;

/** Default route search timeout in milliseconds. */
export const DEFAULT_TIMEOUT_MS = 30_000;

/** Default slippage tolerance in percentage. */
export const DEFAULT_SLIPPAGE_TOLERANCE = 0.5;

/** Minimum bridge liquidity threshold in USD. */
export const MIN_BRIDGE_LIQUIDITY_USD = 10_000;

/** Maximum concurrent bridge quote requests. */
export const MAX_CONCURRENT_QUOTES = 10;

/** Score threshold below which routes are discarded. */
export const MIN_ROUTE_SCORE = 0.1;

/** Maximum acceptable MEV extraction rate. */
export const MAX_MEV_RATE = 0.01;

/** Bridge quote expiry time in milliseconds. */
export const QUOTE_EXPIRY_MS = 30_000;

/** Number of consecutive failures before circuit breaker trips. */
export const CIRCUIT_BREAKER_THRESHOLD = 5;

/** Time in ms before circuit breaker resets after tripping. */
export const CIRCUIT_BREAKER_RESET_MS = 60_000;

/** Maximum quote age in ms before considered stale. */
export const QUOTE_STALENESS_MS = 15_000;

/** Maximum adapter response time in ms before timeout. */
export const ADAPTER_TIMEOUT_MS = 10_000;
