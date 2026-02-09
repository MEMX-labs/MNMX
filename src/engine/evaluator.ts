/**
 * MNMX Position Evaluator
 *
 * Assigns a scalar score to a (state, action) pair by analysing four
 * orthogonal dimensions of execution quality:
 *
 *  - Gas cost         (compute units + priority fee overhead)
 *  - Slippage impact  (market-impact loss relative to midpoint price)
 *  - MEV exposure     (probability-weighted cost of adversarial extraction)
 *  - Profit potential  (net expected value after all costs)
 *
 * Each dimension produces a normalised sub-score in [0, 1] which is
 * combined via a configurable weight vector into the final evaluation.
 */

import type {
  EvaluationResult,
  EvaluationWeights,
  ExecutionAction,
  OnChainState,
  PoolState,
  SearchConfig,
} from '../types/index.js';
import {
  calculateSlippage,
  constantProductSwap,
  estimatePriceImpact,
} from '../utils/math.js';

// ── Constants ───────────────────────────────────────────────────────

/** Baseline compute-unit cost for a simple Solana instruction. */
const BASE_COMPUTE_UNITS = 200_000;

/** Extra CU overhead per action kind (rough estimates). */
const CU_OVERHEADS: Record<string, number> = {
  swap: 150_000,
  transfer: 50_000,
  stake: 120_000,
  unstake: 120_000,
  liquidate: 300_000,
  provide_liquidity: 200_000,
  remove_liquidity: 180_000,
  borrow: 250_000,
  repay: 220_000,
};

/** Maximum compute budget we consider "normal". */
const MAX_ACCEPTABLE_CU = 1_400_000;

/** Lamports per compute unit at typical priority. */
const MICRO_LAMPORTS_PER_CU = 1_000;

/** Slots before state data is considered stale. */
const STALENESS_THRESHOLD_SLOTS = 5;

// ── Evaluator ───────────────────────────────────────────────────────

export class PositionEvaluator {
  private readonly weights: EvaluationWeights;

  constructor(config: SearchConfig) {
    this.weights = config.evaluationWeights;
  }

  /**
   * Evaluate a proposed action given the current on-chain state.
   * Returns a composite score, a per-dimension breakdown, and a
   * confidence metric.
   */
  evaluate(state: OnChainState, action: ExecutionAction): EvaluationResult {
    const gasScore = this.evaluateGasCost(action);
    const slippageScore = this.evaluateSlippage(state, action);
    const mevScore = this.evaluateMevExposure(state, action);
    const profitScore = this.evaluateProfit(state, action);

    // Weighted linear combination – each sub-score is in [0, 1]
    const composite =
      gasScore * this.weights.gasCost +
      slippageScore * this.weights.slippageImpact +
      mevScore * this.weights.mevExposure +
      profitScore * this.weights.profitPotential;

    const confidence = this.computeConfidence(state, action);

    return {
      score: composite * confidence,
      breakdown: {
        gasCost: gasScore,
        slippageImpact: slippageScore,
        mevExposure: mevScore,
        profitPotential: profitScore,
      },
      confidence,
    };
  }

  // ── Gas Cost ────────────────────────────────────────────────────

  /**
   * Score inversely proportional to estimated compute-unit usage.
   * 0 = maximum CU budget exhausted, 1 = minimal CU cost.
   */
  private evaluateGasCost(action: ExecutionAction): number {
    const overhead = CU_OVERHEADS[action.kind] ?? 100_000;
    const estimatedCU = BASE_COMPUTE_UNITS + overhead;
    const ratio = estimatedCU / MAX_ACCEPTABLE_CU;
    return Math.max(0, 1 - ratio);
  }

  // ── Slippage ────────────────────────────────────────────────────

  /**
   * Evaluate the slippage cost of executing `action` against the
   * relevant pool.  Returns 1 for zero slippage, 0 for catastrophic.
   */
  private evaluateSlippage(
    state: OnChainState,
    action: ExecutionAction,
  ): number {
    const pool = state.poolStates.get(action.pool);
    if (!pool) {
      // No pool data – pessimistic assumption
      return 0.3;
    }

    const { reserveA, reserveB, feeBps } = pool;

    // Determine direction
    const [reserveIn, reserveOut] = this.orderReserves(pool, action);

    const slippageBps = calculateSlippage(action.amount, reserveIn, reserveOut, feeBps);
    const slippagePercent = Number(slippageBps) / 100;

    // Map slippage to [0, 1] via a logistic curve – 1% slippage ≈ 0.5 score
    const k = 3; // steepness
    return 1 / (1 + Math.exp(k * (slippagePercent - 1)));
  }

  // ── MEV Exposure ────────────────────────────────────────────────

  /**
   * Estimate the probability and expected cost of MEV attacks for
   * this action.  Larger swaps on shallow pools are more exposed.
   */
  private evaluateMevExposure(
    state: OnChainState,
    action: ExecutionAction,
  ): number {
    // Only swaps and liquidity actions have meaningful MEV risk
    if (!['swap', 'provide_liquidity', 'remove_liquidity', 'liquidate'].includes(action.kind)) {
      return 0.95;
    }

    const pool = state.poolStates.get(action.pool);
    if (!pool) return 0.4;

    const priceImpact = estimatePriceImpact(
      action.amount,
      this.orderReserves(pool, action)[0],
    );

    // Sandwich probability rises with price impact
    const sandwichProb = Math.min(priceImpact * 5, 0.9);

    // Pending-tx density increases frontrun risk
    const pendingCount = state.pendingTransactions.length;
    const congestionFactor = Math.min(pendingCount / 50, 1);

    // Slippage tolerance gives MEV bots room to operate
    const toleranceFactor = Math.min(action.slippageBps / 300, 1);

    const totalRisk = Math.min(
      sandwichProb * 0.5 + congestionFactor * 0.25 + toleranceFactor * 0.25,
      1,
    );

    return 1 - totalRisk;
  }

  // ── Profit ──────────────────────────────────────────────────────

  /**
   * Estimate the net expected value of executing this action.
   * Positive profit → score approaching 1; break-even → 0.5; loss → 0.
   */
  private evaluateProfit(
    state: OnChainState,
    action: ExecutionAction,
  ): number {
    const pool = state.poolStates.get(action.pool);
    if (!pool) return 0.5; // no data, neutral assumption

    const [reserveIn, reserveOut] = this.orderReserves(pool, action);

    const outputAmount = constantProductSwap(
      action.amount,
      reserveIn,
      reserveOut,
      pool.feeBps,
    );

    // Gas cost in token-equivalent terms (very rough)
    const overhead = CU_OVERHEADS[action.kind] ?? 100_000;
    const gasCostLamports = BigInt(BASE_COMPUTE_UNITS + overhead) * BigInt(MICRO_LAMPORTS_PER_CU);
    // Assume 1 token unit ≈ 1_000_000 lamports for normalisation
    const gasCostTokens = gasCostLamports / 1_000_000n;

    // Ideal output at marginal price (no impact)
    const idealOutput = (action.amount * reserveOut) / reserveIn;

    // Net value = output - gas, relative to ideal
    if (idealOutput === 0n) return 0.5;

    const netValue = outputAmount - gasCostTokens;
    const ratio = Number(netValue) / Number(idealOutput);

    // Logistic map: ratio of 1 → ~0.73, ratio > 1 → approaching 1
    return 1 / (1 + Math.exp(-4 * (ratio - 0.5)));
  }

  // ── Confidence ──────────────────────────────────────────────────

  /**
   * Confidence in [0, 1] based on data freshness, pool availability,
   * and balance sufficiency.
   */
  private computeConfidence(
    state: OnChainState,
    action: ExecutionAction,
  ): number {
    let confidence = 1.0;

    // Penalise stale state
    const now = Date.now();
    const ageMs = now - state.timestamp;
    if (ageMs > 10_000) {
      confidence *= Math.max(0.5, 1 - ageMs / 60_000);
    }

    // Pool data available?
    if (!state.poolStates.has(action.pool)) {
      confidence *= 0.6;
    }

    // Sufficient balance for the action?
    const balance = state.tokenBalances.get(action.tokenMintIn) ?? 0n;
    if (balance < action.amount) {
      confidence *= 0.3;
    }

    // Pending transaction congestion lowers confidence
    if (state.pendingTransactions.length > 20) {
      confidence *= 0.85;
    }

    return Math.max(0.1, confidence);
  }

  // ── Helpers ─────────────────────────────────────────────────────

  /**
   * Determine which pool reserve corresponds to the input token and
   * which to the output.
   */
  private orderReserves(
    pool: PoolState,
    action: ExecutionAction,
  ): [bigint, bigint] {
    if (action.tokenMintIn === pool.tokenMintA) {
      return [pool.reserveA, pool.reserveB];
    }
    return [pool.reserveB, pool.reserveA];
  }
}
