/**
 * Tests for the MNMX Game Tree Builder
 */

import { describe, it, expect } from 'vitest';
import { GameTreeBuilder } from '../../src/engine/game-tree.js';
import type {
  ExecutionAction,
  MevThreat,
  OnChainState,
  PoolState,
  SearchConfig,
} from '../../src/types/index.js';

// ── Fixtures ────────────────────────────────────────────────────────

const TEST_POOL: PoolState = {
  address: 'PoolTestAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA',
  tokenMintA: 'MintTestAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA',
  tokenMintB: 'MintTestBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB',
  reserveA: 2_000_000_000n,
  reserveB: 2_000_000_000n,
  feeBps: 25,
};

function makeState(): OnChainState {
  const poolStates = new Map<string, PoolState>();
  poolStates.set(TEST_POOL.address, { ...TEST_POOL });
  const tokenBalances = new Map<string, bigint>();
  tokenBalances.set(TEST_POOL.tokenMintA, 1_000_000_000n);
  tokenBalances.set(TEST_POOL.tokenMintB, 500_000_000n);

  return {
    tokenBalances,
    poolStates,
    pendingTransactions: [],
    slot: 50,
    timestamp: Date.now(),
  };
}

function makeConfig(depth: number = 3): SearchConfig {
  return {
    maxDepth: depth,
    alphaBetaPruning: true,
    timeLimitMs: 5_000,
    evaluationWeights: {
      gasCost: 0.15,
      slippageImpact: 0.25,
      mevExposure: 0.35,
      profitPotential: 0.25,
    },
    maxTranspositionEntries: 10_000,
  };
}

function makeSwap(amount: bigint): ExecutionAction {
  return {
    kind: 'swap',
    tokenMintIn: TEST_POOL.tokenMintA,
    tokenMintOut: TEST_POOL.tokenMintB,
    amount,
    slippageBps: 50,
    pool: TEST_POOL.address,
    priority: 1,
    label: `swap ${amount}`,
  };
}

function makeThreat(): MevThreat {
  return {
    kind: 'sandwich',
    probability: 0.4,
    estimatedCost: 50_000n,
    sourceAddress: 'BotAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA',
    relatedPool: TEST_POOL.address,
    description: 'test sandwich',
  };
}

// ── Tests ───────────────────────────────────────────────────────────

describe('GameTreeBuilder', () => {
  it('should build a tree with the correct root properties', () => {
    const builder = new GameTreeBuilder(makeConfig(2));
    const state = makeState();
    const root = builder.buildTree(state, [makeSwap(10_000_000n)], []);

    expect(root.depth).toBe(0);
    expect(root.player).toBe('agent');
    expect(root.action).toBeNull();
    expect(root.stateHash).toBeTruthy();
  });

  it('should create children for each agent action', () => {
    const builder = new GameTreeBuilder(makeConfig(1));
    const state = makeState();
    const actions = [makeSwap(5_000_000n), makeSwap(10_000_000n)];
    const root = builder.buildTree(state, actions, []);

    expect(root.children.length).toBe(actions.length);
    root.children.forEach((child) => {
      expect(child.player).toBe('adversary');
      expect(child.depth).toBe(1);
    });
  });

  it('should respect maxDepth configuration', () => {
    const maxDepth = 2;
    const builder = new GameTreeBuilder(makeConfig(maxDepth));
    const state = makeState();
    const root = builder.buildTree(state, [makeSwap(10_000_000n)], [makeThreat()]);

    // Walk the tree and verify no node exceeds maxDepth
    const maxFound = findMaxDepth(root);
    expect(maxFound).toBeLessThanOrEqual(maxDepth);
  });

  it('should generate adversary moves for swap actions', () => {
    const builder = new GameTreeBuilder(makeConfig(2));
    const state = makeState();
    const action = makeSwap(100_000_000n); // large enough to trigger threats

    const threats = builder.generateAdversaryMoves(state, action);
    expect(threats.length).toBeGreaterThan(0);
    expect(threats.some((t) => t.kind === 'sandwich')).toBe(true);
  });

  it('should not generate adversary moves for transfer actions', () => {
    const builder = new GameTreeBuilder(makeConfig(2));
    const state = makeState();
    const transfer: ExecutionAction = {
      kind: 'transfer',
      tokenMintIn: TEST_POOL.tokenMintA,
      tokenMintOut: TEST_POOL.tokenMintA,
      amount: 1_000_000n,
      slippageBps: 0,
      pool: TEST_POOL.address,
      priority: 1,
      label: 'transfer',
    };

    const threats = builder.generateAdversaryMoves(state, transfer);
    expect(threats).toHaveLength(0);
  });

  it('should simulate a swap correctly', () => {
    const builder = new GameTreeBuilder(makeConfig(2));
    const state = makeState();
    const action = makeSwap(10_000_000n);

    const nextState = builder.simulateAction(state, action);

    // Input token balance should decrease
    const originalBalance = state.tokenBalances.get(action.tokenMintIn)!;
    const newBalance = nextState.tokenBalances.get(action.tokenMintIn)!;
    expect(newBalance).toBeLessThan(originalBalance);

    // Output token balance should increase
    const originalOut = state.tokenBalances.get(action.tokenMintOut)!;
    const newOut = nextState.tokenBalances.get(action.tokenMintOut)!;
    expect(newOut).toBeGreaterThan(originalOut);

    // Pool reserves should shift
    const originalPool = state.poolStates.get(action.pool)!;
    const newPool = nextState.poolStates.get(action.pool)!;
    expect(newPool.reserveA).toBeGreaterThan(originalPool.reserveA);
    expect(newPool.reserveB).toBeLessThan(originalPool.reserveB);
  });

  it('should not mutate the original state during simulation', () => {
    const builder = new GameTreeBuilder(makeConfig(2));
    const state = makeState();
    const originalBalanceA = state.tokenBalances.get(TEST_POOL.tokenMintA);
    const originalReserveA = state.poolStates.get(TEST_POOL.address)!.reserveA;

    builder.simulateAction(state, makeSwap(50_000_000n));

    expect(state.tokenBalances.get(TEST_POOL.tokenMintA)).toBe(originalBalanceA);
    expect(state.poolStates.get(TEST_POOL.address)!.reserveA).toBe(originalReserveA);
  });

  it('should simulate MEV response by shifting reserves', () => {
    const builder = new GameTreeBuilder(makeConfig(2));
    const state = makeState();
    const threat = makeThreat();

    const nextState = builder.simulateMevResponse(state, threat);
    const originalPool = state.poolStates.get(TEST_POOL.address)!;
    const newPool = nextState.poolStates.get(TEST_POOL.address)!;

    // Sandwich should shift reserves
    expect(newPool.reserveA).not.toBe(originalPool.reserveA);
  });

  it('should produce unique state hashes for different states', () => {
    const builder = new GameTreeBuilder(makeConfig(2));
    const state1 = makeState();
    const state2 = makeState();
    state2.slot = 999;

    const hash1 = builder.hashState(state1);
    const hash2 = builder.hashState(state2);

    expect(hash1).not.toBe(hash2);
  });

  it('should produce consistent hashes for identical states', () => {
    const builder = new GameTreeBuilder(makeConfig(2));
    const state = makeState();

    const hash1 = builder.hashState(state);
    const hash2 = builder.hashState(state);

    expect(hash1).toBe(hash2);
  });
});

// ── Helpers ─────────────────────────────────────────────────────────

function findMaxDepth(node: { depth: number; children: any[] }): number {
  if (node.children.length === 0) return node.depth;
  return Math.max(node.depth, ...node.children.map(findMaxDepth));
}
