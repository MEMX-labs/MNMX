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
