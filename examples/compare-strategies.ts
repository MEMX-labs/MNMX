// ─────────────────────────────────────────────────────────────
// Strategy Comparison
// Compare minimax, cheapest, fastest, and safest strategies
// for the same cross-chain transfer
// ─────────────────────────────────────────────────────────────
//
// Run: npx tsx examples/compare-strategies.ts

import { MnmxRouter } from '../src/router/index.js';
import { WormholeAdapter } from '../src/bridges/wormhole.js';
import { DeBridgeAdapter } from '../src/bridges/debridge.js';
import type { RouteRequest, Strategy, Route } from '../src/types/index.js';

async function main(): Promise<void> {
  const router = new MnmxRouter({ maxHops: 3 });
  router.registerBridge(new WormholeAdapter());
  router.registerBridge(new DeBridgeAdapter());

  const strategies: Strategy[] = ['minimax', 'cheapest', 'fastest', 'safest'];
  const amount = '5000';

  console.log(`Comparing strategies for ${amount} USDC: Ethereum -> Arbitrum\n`);
  console.log(
    'Strategy'.padEnd(12) +
    'Output'.padEnd(14) +
    'Guaranteed'.padEnd(14) +
    'Fees'.padEnd(12) +
    'Time(s)'.padEnd(10) +
    'Score'.padEnd(10) +
    'Hops'
  );
  console.log('-'.repeat(72));

  const results: Array<{ strategy: Strategy; route: Route }> = [];

  for (const strategy of strategies) {
    const request: RouteRequest = {
      from: { chain: 'ethereum', token: 'USDC', amount },
      to: { chain: 'arbitrum', token: 'USDC' },
      options: { strategy },
    };

    const result = await router.findRoute(request);
    if (!result.bestRoute) {
      console.log(`${strategy.padEnd(12)} No route found`);
      continue;
    }

    const r = result.bestRoute;
    results.push({ strategy, route: r });

    console.log(
      strategy.padEnd(12) +
      parseFloat(r.expectedOutput).toFixed(2).padEnd(14) +
      parseFloat(r.guaranteedMinimum).toFixed(2).padEnd(14) +
      parseFloat(r.totalFees).toFixed(2).padEnd(12) +
      r.estimatedTime.toString().padEnd(10) +
      r.minimaxScore.toFixed(4).padEnd(10) +
      r.path.length.toString()
    );
  }

  console.log('\n--- Analysis ---\n');

  if (results.length >= 2) {
    const sorted = [...results].sort((a, b) => b.route.minimaxScore - a.route.minimaxScore);
    console.log(`Highest minimax score: ${sorted[0].strategy} (${sorted[0].route.minimaxScore.toFixed(4)})`);

    const cheapest = [...results].sort(
      (a, b) => parseFloat(a.route.totalFees) - parseFloat(b.route.totalFees),
    );
    console.log(`Lowest fees:           ${cheapest[0].strategy} (${parseFloat(cheapest[0].route.totalFees).toFixed(2)} USDC)`);

    const fastest = [...results].sort((a, b) => a.route.estimatedTime - b.route.estimatedTime);
    console.log(`Fastest time:          ${fastest[0].strategy} (${fastest[0].route.estimatedTime}s)`);

    const safest = [...results].sort(
      (a, b) => parseFloat(b.route.guaranteedMinimum) - parseFloat(a.route.guaranteedMinimum),
    );
    console.log(`Highest guarantee:     ${safest[0].strategy} (${parseFloat(safest[0].route.guaranteedMinimum).toFixed(2)} USDC)`);
  }
}

main().catch(console.error);
