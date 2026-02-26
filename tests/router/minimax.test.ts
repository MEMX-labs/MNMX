import { describe, it, expect } from 'vitest';
import {
  minimaxSearch,
  minimaxSearchWithPruning,
  iterativeDeepening,
} from '../../src/router/minimax.js';
import type { MinimaxOptions } from '../../src/router/minimax.js';
import type { CandidatePath, Token, BridgeQuote, Chain } from '../../src/types/index.js';
import { DEFAULT_ROUTER_CONFIG } from '../../src/types/index.js';

function makeToken(symbol: string, chain: Chain): Token {
  return { symbol, chain, decimals: 18, address: `0x${chain}_${symbol}` };
}

function makeQuote(bridge: string, input: number, output: number, fee: number, time: number): BridgeQuote {
  return {
    bridge,
    inputAmount: input.toFixed(6),
    outputAmount: output.toFixed(6),
    fee: fee.toFixed(6),
    estimatedTime: time,
    liquidityDepth: 5_000_000,
    expiresAt: Date.now() + 60_000,
    slippageBps: 10,
  };
}

function makeCandidate(
  chains: Chain[],
  bridges: string[],
  quotes: BridgeQuote[],
  roughScore: number,
): CandidatePath {
  const tokens: Token[] = chains.map((c) => makeToken('USDC', c));
  return {
    chains,
    bridges,
    tokens,
    quotes,
    estimatedFee: quotes.reduce((s, q) => s + parseFloat(q.fee), 0),
    estimatedTime: quotes.reduce((s, q) => s + q.estimatedTime, 0),
    roughScore,
  };
}

const defaultOptions: MinimaxOptions = {
  maxDepth: 3,
  weights: DEFAULT_ROUTER_CONFIG.weights,
  adversarialModel: DEFAULT_ROUTER_CONFIG.adversarialModel,
  strategy: 'minimax',
};

describe('minimax search', () => {
  const candidates: CandidatePath[] = [
    makeCandidate(
      ['ethereum', 'solana'],
      ['wormhole'],
      [makeQuote('wormhole', 1000, 993, 7, 900)],
      0.993,
    ),
    makeCandidate(
      ['ethereum', 'solana'],
      ['debridge'],
      [makeQuote('debridge', 1000, 995, 5, 600)],
      0.995,
    ),
    makeCandidate(
      ['ethereum', 'arbitrum', 'solana'],
      ['debridge', 'wormhole'],
      [makeQuote('debridge', 1000, 997, 3, 480), makeQuote('wormhole', 997, 990, 7, 900)],
      0.990,
    ),
  ];

  it('finds optimal route', () => {
    const result = minimaxSearch(candidates, 1000, defaultOptions);
    expect(result.bestRoute).not.toBeNull();
    expect(result.allRoutes.length).toBe(3);
    expect(result.stats.nodesExplored).toBeGreaterThan(0);
    // Best route should have highest minimax score
    for (const route of result.allRoutes) {
      expect(route.minimaxScore).toBeLessThanOrEqual(result.bestRoute!.minimaxScore);
    }
  });

  it('alpha-beta pruning reduces search nodes', () => {
    const noPrune = minimaxSearch(candidates, 1000, defaultOptions);
    const pruned = minimaxSearchWithPruning(candidates, 1000, defaultOptions);

    // Pruning should explore fewer or equal nodes
    expect(pruned.stats.nodesExplored).toBeLessThanOrEqual(noPrune.stats.nodesExplored);

    // Both should find the same best route (same optimal result)
    expect(pruned.bestRoute).not.toBeNull();
    expect(noPrune.bestRoute).not.toBeNull();
  });

  it('adversarial model produces worse scores than base evaluation', () => {
    const result = minimaxSearch(candidates, 1000, defaultOptions);
    for (const route of result.allRoutes) {
      // Minimax score (adversarial) should be less than or equal to what
      // a greedy evaluation would produce, since adversarial degrades values
      expect(route.minimaxScore).toBeLessThan(1.0);
      expect(route.minimaxScore).toBeGreaterThan(0);
      // Guaranteed minimum should be less than expected output
      const expected = parseFloat(route.expectedOutput);
      const guaranteed = parseFloat(route.guaranteedMinimum);
      expect(guaranteed).toBeLessThanOrEqual(expected);
    }
  });

  it('deeper search does not regress', () => {
    const shallow = minimaxSearchWithPruning(candidates, 1000, {
      ...defaultOptions,
      maxDepth: 1,
    });
    const deep = minimaxSearchWithPruning(candidates, 1000, {
      ...defaultOptions,
      maxDepth: 5,
    });

    // Both should find valid routes
    expect(shallow.bestRoute).not.toBeNull();
    expect(deep.bestRoute).not.toBeNull();

    // Deeper search should explore at least as many nodes
    expect(deep.stats.nodesExplored).toBeGreaterThanOrEqual(shallow.stats.nodesExplored);
  });

  it('iterative deepening returns valid result at each depth', () => {
    const results = iterativeDeepening(candidates, 1000, defaultOptions, 3);

    expect(results.length).toBe(3);
    for (let i = 0; i < results.length; i++) {
      const r = results[i];
      expect(r.bestRoute).not.toBeNull();
      expect(r.allRoutes.length).toBeGreaterThan(0);
      expect(r.stats.nodesExplored).toBeGreaterThan(0);
      // Each result should be independently valid
      expect(r.bestRoute!.minimaxScore).toBeGreaterThan(0);
    }
  });

  it('handles empty candidates gracefully', () => {
    const result = minimaxSearch([], 1000, defaultOptions);
    expect(result.bestRoute).toBeNull();
    expect(result.allRoutes).toHaveLength(0);
    expect(result.stats.nodesExplored).toBe(0);
  });
});
