/**
 * Tests for the MNMX Position Evaluator
 */

import { describe, it, expect } from 'vitest';
import { PositionEvaluator } from '../../src/engine/evaluator.js';
import type {
  ExecutionAction,
  OnChainState,
  PoolState,
  SearchConfig,
} from '../../src/types/index.js';
import { DEFAULT_SEARCH_CONFIG } from '../../src/types/index.js';

// ── Fixtures ────────────────────────────────────────────────────────

function makePool(overrides: Partial<PoolState> = {}): PoolState {
  return {
    address: 'Pool111111111111111111111111111111111111111',
    tokenMintA: 'MintA11111111111111111111111111111111111111',
    tokenMintB: 'MintB11111111111111111111111111111111111111',
    reserveA: 1_000_000_000n,
    reserveB: 1_000_000_000n,
    feeBps: 30,
    ...overrides,
  };
}

function makeState(pool: PoolState): OnChainState {
  const poolStates = new Map<string, PoolState>();
  poolStates.set(pool.address, pool);

  const tokenBalances = new Map<string, bigint>();
  tokenBalances.set(pool.tokenMintA, 500_000_000n);
  tokenBalances.set(pool.tokenMintB, 500_000_000n);

  return {
    tokenBalances,
    poolStates,
    pendingTransactions: [],
    slot: 200,
    timestamp: Date.now(),
  };
}

function makeSwap(pool: PoolState, amount: bigint, slippageBps = 50): ExecutionAction {
  return {
    kind: 'swap',
    tokenMintIn: pool.tokenMintA,
    tokenMintOut: pool.tokenMintB,
    amount,
    slippageBps,
    pool: pool.address,
    priority: 1,
    label: 'test swap',
  };
}

// ── Tests ───────────────────────────────────────────────────────────

describe('PositionEvaluator', () => {
  const evaluator = new PositionEvaluator(DEFAULT_SEARCH_CONFIG);

  it('should return a score between 0 and 1', () => {
    const pool = makePool();
    const state = makeState(pool);
    const action = makeSwap(pool, 10_000_000n);

    const result = evaluator.evaluate(state, action);

    expect(result.score).toBeGreaterThanOrEqual(0);
    expect(result.score).toBeLessThanOrEqual(1);
  });

  it('should give lower slippage score for larger trades', () => {
    const pool = makePool();
    const state = makeState(pool);

    const small = evaluator.evaluate(state, makeSwap(pool, 1_000_000n));
    const large = evaluator.evaluate(state, makeSwap(pool, 500_000_000n));

    expect(small.breakdown.slippageImpact).toBeGreaterThan(
      large.breakdown.slippageImpact,
    );
  });

  it('should give higher MEV exposure for shallow pools', () => {
    const deepPool = makePool({ reserveA: 10_000_000_000n, reserveB: 10_000_000_000n });
    const shallowPool = makePool({
      address: 'Pool222222222222222222222222222222222222222',
      reserveA: 100_000_000n,
      reserveB: 100_000_000n,
    });

    const deepState = makeState(deepPool);
    const shallowState = makeState(shallowPool);

    const amount = 50_000_000n;

    const deepResult = evaluator.evaluate(deepState, makeSwap(deepPool, amount));
    const shallowResult = evaluator.evaluate(shallowState, makeSwap(shallowPool, amount));

    // Deep pool should have lower MEV exposure (higher score = less exposure)
    expect(deepResult.breakdown.mevExposure).toBeGreaterThan(
      shallowResult.breakdown.mevExposure,
    );
  });

  it('should reflect evaluation weights in the final score', () => {
    const pool = makePool();
    const state = makeState(pool);
    const action = makeSwap(pool, 10_000_000n);

    const profitHeavy = new PositionEvaluator({
      ...DEFAULT_SEARCH_CONFIG,
      evaluationWeights: {
        gasCost: 0,
        slippageImpact: 0,
        mevExposure: 0,
        profitPotential: 1,
      },
    });

    const gasHeavy = new PositionEvaluator({
      ...DEFAULT_SEARCH_CONFIG,
      evaluationWeights: {
        gasCost: 1,
        slippageImpact: 0,
        mevExposure: 0,
        profitPotential: 0,
      },
    });

    const profitResult = profitHeavy.evaluate(state, action);
    const gasResult = gasHeavy.evaluate(state, action);

    // Results should differ because the weights prioritise different dimensions
    expect(Math.abs(profitResult.score - gasResult.score)).toBeGreaterThan(0.001);
  });

  it('should give lower confidence when balance is insufficient', () => {
    const pool = makePool();
    const state = makeState(pool);

    // Balance of MintA is 500M, so a 10M swap is fine
    const normalSwap = makeSwap(pool, 10_000_000n);
    const normalResult = evaluator.evaluate(state, normalSwap);

    // But a 2B swap exceeds balance
    const oversizedSwap = makeSwap(pool, 2_000_000_000n);
    const oversizedResult = evaluator.evaluate(state, oversizedSwap);

    expect(normalResult.confidence).toBeGreaterThan(oversizedResult.confidence);
  });

  it('should give lower confidence for congested mempool', () => {
    const pool = makePool();
    const quietState = makeState(pool);

    const busyState = makeState(pool);
    busyState.pendingTransactions = Array.from({ length: 30 }, (_, i) => ({
      signature: `sig${i}`,
      fromAddress: `addr${i}`,
      toAddress: `addr${i + 1}`,
      programId: '11111111111111111111111111111111',
      data: new Uint8Array(0),
      lamports: 0n,
      slot: 200,
    }));

    const action = makeSwap(pool, 10_000_000n);

    const quietResult = evaluator.evaluate(quietState, action);
    const busyResult = evaluator.evaluate(busyState, action);

    expect(quietResult.confidence).toBeGreaterThan(busyResult.confidence);
  });

  it('should score transfers with low gas cost', () => {
    const pool = makePool();
    const state = makeState(pool);

    const transfer: ExecutionAction = {
      kind: 'transfer',
      tokenMintIn: pool.tokenMintA,
      tokenMintOut: pool.tokenMintA,
      amount: 1_000_000n,
      slippageBps: 0,
      pool: pool.address,
      priority: 1,
      label: 'transfer',
    };

    const result = evaluator.evaluate(state, transfer);
    // Transfers have low CU overhead, so gas score should be high
    expect(result.breakdown.gasCost).toBeGreaterThan(0.5);
  });
});
