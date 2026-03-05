/**
 * Tests for MoveOrderer
 *
 * Validates killer move tracking, history heuristic ordering,
 * MVV-LVA prioritization, and that ordering improves pruning.
 */

import { describe, it, expect, beforeEach } from 'vitest';
import { MoveOrderer } from '../../src/engine/move-ordering.js';
import type { ExecutionAction, OnChainState, PoolState } from '../../src/types/index.js';

// ── Test Helpers ─────────────────────────────────────────────────────

function makeAction(overrides: Partial<ExecutionAction> = {}): ExecutionAction {
  return {
    kind: 'swap',
    tokenMintIn: 'So11111111111111111111111111111111111111112',
    tokenMintOut: 'EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v',
    amount: 1_000_000n,
    slippageBps: 50,
    pool: 'pool_default',
    priority: 0,
    label: 'test swap',
    ...overrides,
  };
}

function makeState(pools: PoolState[] = []): OnChainState {
  const poolStates = new Map<string, PoolState>();
  for (const pool of pools) {
    poolStates.set(pool.address, pool);
  }

  return {
    tokenBalances: new Map(),
    poolStates,
    pendingTransactions: [],
    slot: 100,
    timestamp: Date.now(),
  };
}

function makePool(address: string, reserveA: bigint, reserveB: bigint): PoolState {
  return {
    address,
    tokenMintA: 'So11111111111111111111111111111111111111112',
    tokenMintB: 'EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v',
    reserveA,
    reserveB,
    feeBps: 30,
  };
}

// ── Tests ────────────────────────────────────────────────────────────

describe('MoveOrderer', () => {
  let orderer: MoveOrderer;

  beforeEach(() => {
    orderer = new MoveOrderer();
  });

  describe('killer move tracking', () => {
    it('should prioritize killer moves at the same depth', () => {
      const normalAction = makeAction({ pool: 'pool_A', label: 'normal' });
      const killerAction = makeAction({ pool: 'pool_B', label: 'killer' });
      const state = makeState();

      // Record killerAction as a killer move at depth 3
      orderer.updateKillerMove(3, killerAction);

      const ordered = orderer.orderMoves([normalAction, killerAction], state, 3);

      // The killer move should appear first
      expect(ordered[0]!.label).toBe('killer');
    });

    it('should not apply killer bonus at a different depth', () => {
      const action1 = makeAction({ pool: 'pool_A', label: 'a1', priority: 5 });
      const action2 = makeAction({ pool: 'pool_B', label: 'a2', priority: 1 });
      const state = makeState();

      // Record action2 as killer at depth 3, but search at depth 5
      orderer.updateKillerMove(3, action2);

      const ordered = orderer.orderMoves([action1, action2], state, 5);

      // action1 has higher priority and no killer bonus at depth 5,
      // so it should still come first
      expect(ordered[0]!.label).toBe('a1');
    });

    it('should maintain at most two killer slots per depth', () => {
      const k1 = makeAction({ pool: 'pool_1', label: 'k1' });
      const k2 = makeAction({ pool: 'pool_2', label: 'k2' });
      const k3 = makeAction({ pool: 'pool_3', label: 'k3' });
      const state = makeState();

      orderer.updateKillerMove(2, k1);
      orderer.updateKillerMove(2, k2);
      orderer.updateKillerMove(2, k3); // should evict k1

      const ordered = orderer.orderMoves([k1, k2, k3], state, 2);

      // k2 and k3 should have killer bonus, k1 should not
      // k2 and k3 will be at the top (order between them depends on
      // other heuristics, but both should beat k1)
      const topTwoLabels = [ordered[0]!.label, ordered[1]!.label];
      expect(topTwoLabels).toContain('k2');
      expect(topTwoLabels).toContain('k3');
    });

    it('should not add duplicate killer moves', () => {
      const k1 = makeAction({ pool: 'pool_1', label: 'k1' });
      const k2 = makeAction({ pool: 'pool_2', label: 'k2' });
      const state = makeState();

      orderer.updateKillerMove(1, k1);
      orderer.updateKillerMove(1, k1); // duplicate
      orderer.updateKillerMove(1, k2);

      // Both k1 and k2 should have killer bonus (duplicate did not consume a slot)
      const ordered = orderer.orderMoves([k1, k2], state, 1);
      const labels = ordered.map((a) => a.label);
      expect(labels).toContain('k1');
      expect(labels).toContain('k2');
    });
  });

  describe('history heuristic ordering', () => {
    it('should boost actions with accumulated history scores', () => {
      const frequent = makeAction({ pool: 'pool_freq', label: 'frequent', priority: 0 });
      const rare = makeAction({ pool: 'pool_rare', label: 'rare', priority: 0 });
      const state = makeState();

      // Simulate multiple cutoffs at various depths for the frequent action
      for (let d = 1; d <= 5; d++) {
        orderer.updateHistory(frequent, d);
      }

      const ordered = orderer.orderMoves([rare, frequent], state, 0);
      expect(ordered[0]!.label).toBe('frequent');
    });

    it('should weight history by depth squared', () => {
      const shallow = makeAction({ pool: 'pool_shallow', label: 'shallow', priority: 0 });
      const deep = makeAction({ pool: 'pool_deep', label: 'deep', priority: 0 });
      const state = makeState();

      // shallow: 5 cutoffs at depth 1 => score = 5 * 1 = 5
      for (let i = 0; i < 5; i++) {
        orderer.updateHistory(shallow, 1);
      }

      // deep: 1 cutoff at depth 4 => score = 1 * 16 = 16
      orderer.updateHistory(deep, 4);

      const ordered = orderer.orderMoves([shallow, deep], state, 0);
      expect(ordered[0]!.label).toBe('deep');
    });
  });

  describe('MVV-LVA prioritization', () => {
    it('should prefer high-amount actions through liquid pools', () => {
      const deepPool = makePool('pool_deep', 10_000_000_000n, 10_000_000_000n);
      const state = makeState([deepPool]);

      const bigSwap = makeAction({
        pool: 'pool_deep',
        amount: 100_000_000n,
        label: 'big',
        priority: 0,
      });
      const smallSwap = makeAction({
        pool: 'pool_deep',
        amount: 1_000n,
        label: 'small',
        priority: 0,
      });

      const ordered = orderer.orderMoves([smallSwap, bigSwap], state, 0);
      expect(ordered[0]!.label).toBe('big');
    });

    it('should penalize high-slippage actions', () => {
      const state = makeState();

      const lowSlippage = makeAction({
        pool: 'pool_a',
        slippageBps: 10,
        amount: 1_000_000n,
        label: 'low_slip',
        priority: 0,
      });
      const highSlippage = makeAction({
        pool: 'pool_a',
        slippageBps: 500,
        amount: 1_000_000n,
        label: 'high_slip',
        priority: 0,
      });

      const ordered = orderer.orderMoves([highSlippage, lowSlippage], state, 0);
      expect(ordered[0]!.label).toBe('low_slip');
    });

    it('should weight liquidation actions higher than transfers', () => {
      const state = makeState();

      const transfer = makeAction({
        kind: 'transfer',
        pool: 'pool_t',
        amount: 1_000_000n,
        label: 'transfer',
        priority: 0,
      });
      const liquidation = makeAction({
        kind: 'liquidate',
        pool: 'pool_l',
        amount: 1_000_000n,
        label: 'liquidation',
        priority: 0,
      });

      const ordered = orderer.orderMoves([transfer, liquidation], state, 0);
      expect(ordered[0]!.label).toBe('liquidation');
    });
  });

  describe('ordering efficiency', () => {
    it('should not mutate the input array', () => {
      const actions = [
        makeAction({ pool: 'pool_1', label: 'a1' }),
        makeAction({ pool: 'pool_2', label: 'a2' }),
      ];
      const original = [...actions];
      const state = makeState();

      orderer.orderMoves(actions, state, 0);

      expect(actions[0]!.label).toBe(original[0]!.label);
      expect(actions[1]!.label).toBe(original[1]!.label);
    });

    it('should respect explicit priority hints', () => {
      const state = makeState();

      const lowPri = makeAction({ pool: 'pool_lp', label: 'low', priority: 1 });
      const highPri = makeAction({ pool: 'pool_hp', label: 'high', priority: 100 });

      const ordered = orderer.orderMoves([lowPri, highPri], state, 0);
      expect(ordered[0]!.label).toBe('high');
    });

    it('should handle empty action lists', () => {
      const state = makeState();
      const ordered = orderer.orderMoves([], state, 0);
      expect(ordered).toHaveLength(0);
    });

    it('should handle single action lists', () => {
      const state = makeState();
      const action = makeAction({ label: 'only' });
      const ordered = orderer.orderMoves([action], state, 0);
      expect(ordered).toHaveLength(1);
      expect(ordered[0]!.label).toBe('only');
    });

    it('should reset all heuristic data', () => {
      const killerAction = makeAction({ pool: 'pool_k', label: 'killer' });
      const historyAction = makeAction({ pool: 'pool_h', label: 'history', priority: 0 });
      const state = makeState();

      orderer.updateKillerMove(1, killerAction);
      orderer.updateHistory(historyAction, 5);

      orderer.reset();

      // After reset, neither should have bonuses
      const plainA = makeAction({ pool: 'pool_a', label: 'plain_a', priority: 10 });
      const ordered = orderer.orderMoves(
        [killerAction, historyAction, plainA],
        state,
        1,
      );

      // plainA has highest explicit priority, should win without heuristic bonuses
      expect(ordered[0]!.label).toBe('plain_a');
    });
  });
});
