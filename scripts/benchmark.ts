// ─────────────────────────────────────────────────────────────
// MNMX Benchmark
// Time path discovery and minimax search with sample data
// ─────────────────────────────────────────────────────────────
//
// Run: npx tsx scripts/benchmark.ts

import { BridgeRegistry } from '../src/bridges/adapter.js';
import { WormholeAdapter } from '../src/bridges/wormhole.js';
import { DeBridgeAdapter } from '../src/bridges/debridge.js';
import {
  discoverChainPaths,
  filterDominatedPaths,
  buildCandidatePaths,
} from '../src/router/path-discovery.js';
import {
  minimaxSearch,
  minimaxSearchWithPruning,
  iterativeDeepening,
} from '../src/router/minimax.js';
import { DEFAULT_ROUTER_CONFIG } from '../src/types/index.js';
import type { Chain, Token } from '../src/types/index.js';

function makeToken(symbol: string, chain: Chain): Token {
  return { symbol, chain, decimals: 6, address: `0x${chain}_${symbol}` };
}

function formatMs(ms: number): string {
  return ms < 1 ? `${(ms * 1000).toFixed(0)}us` : `${ms.toFixed(2)}ms`;
}

async function benchmark(): Promise<void> {
  const registry = new BridgeRegistry();
  registry.register(new WormholeAdapter());
  registry.register(new DeBridgeAdapter());

  const pairs: Array<[Chain, Chain]> = [
    ['ethereum', 'solana'],
    ['ethereum', 'arbitrum'],
    ['polygon', 'solana'],
    ['arbitrum', 'bnb'],
  ];

  const maxHops = [1, 2, 3];
  const amounts = ['100', '1000', '10000', '100000'];

  console.log('MNMX Benchmark');
  console.log('='.repeat(72));
  console.log('');

  // Benchmark: Path Discovery
  console.log('--- Path Discovery ---');
  console.log(
    'From'.padEnd(12) + 'To'.padEnd(12) +
    'Hops'.padEnd(6) + 'Paths'.padEnd(8) +
    'Filtered'.padEnd(10) + 'Time'.padEnd(12)
  );

  for (const [from, to] of pairs) {
    for (const hops of maxHops) {
      const opts = { maxHops: hops, excludeBridges: [], excludeChains: [] as Chain[], minLiquidity: 0 };
      const start = performance.now();
      const paths = discoverChainPaths(from, to, registry, opts);
      const filtered = filterDominatedPaths(paths);
      const elapsed = performance.now() - start;

      console.log(
        from.padEnd(12) + to.padEnd(12) +
        hops.toString().padEnd(6) +
        paths.length.toString().padEnd(8) +
        filtered.length.toString().padEnd(10) +
        formatMs(elapsed).padEnd(12)
      );
    }
  }

  console.log('');

  // Benchmark: Full Pipeline (Path Discovery + Quoting + Minimax)
  console.log('--- Full Pipeline ---');
  console.log(
    'Route'.padEnd(24) + 'Amount'.padEnd(10) +
    'Candidates'.padEnd(12) + 'Nodes'.padEnd(8) +
    'Pruned'.padEnd(8) + 'Score'.padEnd(10) +
    'Time'.padEnd(12)
  );

  for (const [from, to] of pairs) {
    for (const amount of amounts) {
      const opts = { maxHops: 2, excludeBridges: [], excludeChains: [] as Chain[], minLiquidity: 0 };

      const pipelineStart = performance.now();

      const paths = discoverChainPaths(from, to, registry, opts);
      const filtered = filterDominatedPaths(paths);

      const fromToken = makeToken('USDC', from);
      const toToken = makeToken('USDC', to);

      const candidates = await buildCandidatePaths(
        filtered, fromToken, toToken, amount, registry, opts,
      );

      if (candidates.length === 0) continue;

      const result = minimaxSearchWithPruning(candidates, parseFloat(amount), {
        maxDepth: 2,
        weights: DEFAULT_ROUTER_CONFIG.weights,
        adversarialModel: DEFAULT_ROUTER_CONFIG.adversarialModel,
        strategy: 'minimax',
      });

      const pipelineElapsed = performance.now() - pipelineStart;

      const routeLabel = `${from}->${to}`;
      const scoreStr = result.bestRoute
        ? result.bestRoute.minimaxScore.toFixed(4)
        : 'N/A';

      console.log(
        routeLabel.padEnd(24) +
        amount.padEnd(10) +
        result.stats.candidateCount.toString().padEnd(12) +
        result.stats.nodesExplored.toString().padEnd(8) +
        result.stats.nodesPruned.toString().padEnd(8) +
        scoreStr.padEnd(10) +
        formatMs(pipelineElapsed).padEnd(12)
      );
    }
  }

  console.log('');

  // Benchmark: Pruning vs No Pruning
  console.log('--- Pruning Comparison ---');
  const from = 'ethereum' as Chain;
  const to = 'solana' as Chain;
  const opts = { maxHops: 3, excludeBridges: [], excludeChains: [] as Chain[], minLiquidity: 0 };
  const paths = discoverChainPaths(from, to, registry, opts);
  const filtered = filterDominatedPaths(paths);
  const fromToken = makeToken('USDC', from);
  const toToken = makeToken('USDC', to);
  const candidates = await buildCandidatePaths(
    filtered, fromToken, toToken, '10000', registry, opts,
  );

  if (candidates.length > 0) {
    const searchOpts = {
      maxDepth: 3,
      weights: DEFAULT_ROUTER_CONFIG.weights,
      adversarialModel: DEFAULT_ROUTER_CONFIG.adversarialModel,
      strategy: 'minimax' as const,
    };

    const noPruneStart = performance.now();
    const noPruneResult = minimaxSearch(candidates, 10000, searchOpts);
    const noPruneTime = performance.now() - noPruneStart;

    const pruneStart = performance.now();
    const pruneResult = minimaxSearchWithPruning(candidates, 10000, searchOpts);
    const pruneTime = performance.now() - pruneStart;

    console.log(`Candidates: ${candidates.length}`);
    console.log(`Without pruning: ${noPruneResult.stats.nodesExplored} nodes, ${formatMs(noPruneTime)}`);
    console.log(`With pruning:    ${pruneResult.stats.nodesExplored} nodes, ${pruneResult.stats.nodesPruned} pruned, ${formatMs(pruneTime)}`);
    console.log(`Node reduction:  ${(100 * (1 - pruneResult.stats.nodesExplored / noPruneResult.stats.nodesExplored)).toFixed(1)}%`);
  }
}

benchmark().catch(console.error);
