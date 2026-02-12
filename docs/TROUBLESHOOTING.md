# Troubleshooting

## No Route Found

**Symptom:** `NoRouteFoundError` when calling `findRoute()` or `find_route()`.

**Common causes:**
- The source and destination chains have no bridge in common
- Bridge liquidity is too low for the transfer amount
- All bridges are currently offline or degraded
- `maxHops` is set too low for the chain pair

**Solutions:**
1. Check supported chain pairs with `getSupportedChains()`
2. Increase `maxHops` to allow indirect routing
3. Reduce transfer amount
4. Check bridge health with `getHealth()`

## Search Timeout

**Symptom:** `SearchTimeoutError` during route discovery.

**Common causes:**
- Transfer involves many possible paths (high branching factor)
- Timeout is set too low
- Bridge APIs are slow to respond

**Solutions:**
1. Increase `timeout` in router config
2. Reduce `maxHops` to limit search space
3. Exclude slow bridges with `excludeBridges`

## High Slippage Warning

**Symptom:** `guaranteedMinimum` is significantly lower than `expectedOutput`.

**Common causes:**
- Low liquidity on the selected bridge
- Large transfer amount relative to pool depth
- High market volatility

**Solutions:**
1. Use `safest` strategy for large transfers
2. Split into multiple smaller transfers
3. Increase `slippageTolerance` if the gap is acceptable
