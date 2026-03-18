# Configuration Reference

## Router Configuration

```typescript
const config = {
  maxHops: 5,               // Maximum hops per route
  timeoutMs: 30_000,        // Search timeout
  slippageTolerance: 0.5,   // Default slippage %
  minBridgeLiquidity: 10_000, // Min liquidity in USD
  maxConcurrentQuotes: 10,  // Parallel quote limit
  minRouteScore: 0.1,       // Score threshold
  maxMevRate: 0.01,         // Max acceptable MEV
  quoteExpiryMs: 30_000,    // Quote validity
};
```

## Strategy Options

| Strategy | Optimizes For | Use Case |
|----------|--------------|----------|
| `minimax` | Worst-case output | Default, risk-averse |
| `fastest` | Execution time | Time-sensitive transfers |
| `cheapest` | Total fees | Large amounts |
| `safest` | Reliability + worst-case | High-value transfers |

## Circuit Breaker

Automatically disables a bridge adapter after 5 consecutive failures.
Resets after 60 seconds.

```typescript
{
  circuitBreakerThreshold: 5,
  circuitBreakerResetMs: 60_000,
}
```

## Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `MNMX_LOG_LEVEL` | Logging verbosity | `info` |
| `MNMX_TIMEOUT_MS` | Global timeout | `30000` |
| `MNMX_MAX_HOPS` | Maximum route hops | `5` |
