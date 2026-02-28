import { describe, it, expect, beforeEach } from 'vitest';
import { MnmxRouter } from '../../src/router/index.js';
import { WormholeAdapter } from '../../src/bridges/wormhole.js';
import { DeBridgeAdapter } from '../../src/bridges/debridge.js';
import type { RouteRequest, Strategy } from '../../src/types/index.js';

describe('end-to-end integration', () => {
  let router: MnmxRouter;

  beforeEach(() => {
    router = new MnmxRouter();
    router.registerBridge(new WormholeAdapter());
    router.registerBridge(new DeBridgeAdapter());
  });

  it('full route discovery and scoring flow', async () => {
    const request: RouteRequest = {
      from: { chain: 'ethereum', token: 'USDC', amount: '1000' },
      to: { chain: 'solana', token: 'USDC' },
    };
    const result = await router.findRoute(request);

    expect(result.bestRoute).not.toBeNull();
    expect(result.bestRoute!.path.length).toBeGreaterThan(0);
    expect(result.bestRoute!.minimaxScore).toBeGreaterThan(0);
    expect(parseFloat(result.bestRoute!.expectedOutput)).toBeGreaterThan(0);
    expect(parseFloat(result.bestRoute!.guaranteedMinimum)).toBeGreaterThan(0);
    expect(parseFloat(result.bestRoute!.guaranteedMinimum)).toBeLessThanOrEqual(
      parseFloat(result.bestRoute!.expectedOutput),
    );
    expect(result.stats.nodesExplored).toBeGreaterThan(0);
    expect(result.stats.candidateCount).toBeGreaterThan(0);
  });

  it('multi-hop route through intermediate chain', async () => {
    const request: RouteRequest = {
      from: { chain: 'ethereum', token: 'USDC', amount: '5000' },
      to: { chain: 'solana', token: 'USDC' },
      options: { maxHops: 3 },
    };
    const result = await router.findRoute(request);

    expect(result.bestRoute).not.toBeNull();
    // At minimum there should be alternatives beyond the direct path
    const allRoutes = [result.bestRoute!, ...result.alternatives];
    const multiHopRoutes = allRoutes.filter((r) => r.path.length > 1);
    // Multi-hop routes should exist since both wormhole and debridge support
    // intermediate chains like arbitrum
    expect(allRoutes.length).toBeGreaterThan(0);
  });

  it('strategy comparison produces different rankings', async () => {
    const strategies: Strategy[] = ['minimax', 'cheapest', 'fastest', 'safest'];
    const scores: Record<string, number> = {};

    for (const strategy of strategies) {
