// ─────────────────────────────────────────────────────────────
// Basic Route Discovery
// Find and display a route from USDC on Ethereum to USDC on Solana
// ─────────────────────────────────────────────────────────────
//
// Run: npx tsx examples/basic-route.ts

import { MnmxRouter } from '../src/router/index.js';
import { WormholeAdapter } from '../src/bridges/wormhole.js';
import { DeBridgeAdapter } from '../src/bridges/debridge.js';
import type { RouteRequest } from '../src/types/index.js';

async function main(): Promise<void> {
  const router = new MnmxRouter({ strategy: 'minimax', maxHops: 3 });
  router.registerBridge(new WormholeAdapter());
  router.registerBridge(new DeBridgeAdapter());

  const request: RouteRequest = {
    from: { chain: 'ethereum', token: 'USDC', amount: '1000' },
    to: { chain: 'solana', token: 'USDC' },
  };

  console.log('Finding route: 1000 USDC Ethereum -> Solana\n');

  const result = await router.findRoute(request);

  if (!result.bestRoute) {
    console.log('No route found.');
    return;
  }

  const route = result.bestRoute;
  console.log(`Best route (${route.strategy} strategy):`);
  console.log(`  Route ID:           ${route.routeId}`);
  console.log(`  Hops:               ${route.path.length}`);
  console.log(`  Expected output:    ${route.expectedOutput} USDC`);
  console.log(`  Guaranteed minimum: ${route.guaranteedMinimum} USDC`);
  console.log(`  Total fees:         ${route.totalFees} USDC`);
  console.log(`  Estimated time:     ${route.estimatedTime}s`);
  console.log(`  Minimax score:      ${route.minimaxScore.toFixed(4)}`);

  console.log('\n  Path:');
  for (const hop of route.path) {
    console.log(
      `    ${hop.fromChain} -> ${hop.toChain} via ${hop.bridge}` +
      ` | ${hop.inputAmount} -> ${hop.outputAmount} | fee: ${hop.fee} | ${hop.estimatedTime}s`
    );
  }

  console.log('\n  Search stats:');
  console.log(`    Nodes explored:  ${result.stats.nodesExplored}`);
  console.log(`    Nodes pruned:    ${result.stats.nodesPruned}`);
  console.log(`    Candidates:      ${result.stats.candidateCount}`);
  console.log(`    Search time:     ${result.stats.searchTimeMs}ms`);

  if (result.alternatives.length > 0) {
    console.log(`\n  ${result.alternatives.length} alternative route(s) found.`);
  }
}

main().catch(console.error);
