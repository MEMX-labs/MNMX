import { describe, it, expect, beforeEach } from 'vitest';
import { discoverChainPaths, filterDominatedPaths, buildCandidatePaths } from '../../src/router/path-discovery.js';
import { BridgeRegistry } from '../../src/bridges/adapter.js';
import { WormholeAdapter } from '../../src/bridges/wormhole.js';
import { DeBridgeAdapter } from '../../src/bridges/debridge.js';
import type { Chain, Token } from '../../src/types/index.js';

function makeToken(symbol: string, chain: Chain): Token {
  return { symbol, chain, decimals: 18, address: `0x${chain}_${symbol}` };
}

describe('path-discovery', () => {
  let registry: BridgeRegistry;

  beforeEach(() => {
    registry = new BridgeRegistry();
    registry.register(new WormholeAdapter());
    registry.register(new DeBridgeAdapter());
  });

  const defaultOpts = {
    maxHops: 3,
    excludeBridges: [] as string[],
    excludeChains: [] as Chain[],
    minLiquidity: 0,
  };

  describe('discoverChainPaths', () => {
    it('discovers direct paths between supported chain pairs', () => {
      const paths = discoverChainPaths('ethereum', 'solana', registry, defaultOpts);
      const directPaths = paths.filter((p) => p.length === 2);
      expect(directPaths.length).toBeGreaterThan(0);
      for (const p of directPaths) {
        expect(p[0]).toBe('ethereum');
        expect(p[p.length - 1]).toBe('solana');
      }
    });

    it('discovers multi-hop paths', () => {
      const paths = discoverChainPaths('ethereum', 'solana', registry, defaultOpts);
      const multiHopPaths = paths.filter((p) => p.length > 2);
      expect(multiHopPaths.length).toBeGreaterThan(0);
      for (const p of multiHopPaths) {
        expect(p[0]).toBe('ethereum');
        expect(p[p.length - 1]).toBe('solana');
        expect(p.length).toBeGreaterThan(2);
        // Verify no duplicate chains in path
        const unique = new Set(p);
        expect(unique.size).toBe(p.length);
      }
    });

    it('respects max hops constraint', () => {
      const paths1 = discoverChainPaths('ethereum', 'solana', registry, {
        ...defaultOpts,
        maxHops: 1,
      });
      for (const p of paths1) {
        expect(p.length).toBeLessThanOrEqual(2); // 1 hop = 2 chains
      }

      const paths2 = discoverChainPaths('ethereum', 'solana', registry, {
        ...defaultOpts,
        maxHops: 2,
      });
      for (const p of paths2) {
        expect(p.length).toBeLessThanOrEqual(3); // 2 hops = 3 chains
      }
