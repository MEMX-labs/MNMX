/**
 * MNMX Time Manager
 *
 * Allocates search time across iterative deepening depths using a model
 * inspired by chess engine time controls. Adapts allocation based on the
 * "game phase" of the on-chain environment:
 *
 *  - Opening:  many candidate actions, broad search, conservative time use.
 *  - Midgame:  moderate branching, balanced allocation.
 *  - Endgame:  few critical actions, deep search, aggressive time use.
 *
 * Also provides extension logic for unstable positions (where the best
 * move changes between iterations) and single-reply situations (where
 * only one reasonable action exists and search can terminate early).
 */

import type { SearchConfig } from '../types/index.js';

// ── Types ────────────────────────────────────────────────────────────

export type GamePhase = 'opening' | 'midgame' | 'endgame';

export interface TimeAllocation {
  /** Total time budget for this search invocation (ms). */
  readonly totalBudgetMs: number;
  /** Hard deadline -- search must stop by this time (ms since epoch). */
  readonly hardDeadlineMs: number;
  /** Soft target -- aim to finish each iteration within this (ms). */
  readonly softTargetMs: number;
  /** Maximum time allowed for a single depth iteration (ms). */
  readonly maxIterationMs: number;
  /** Time already consumed when this allocation was created (ms). */
  readonly elapsedAtCreation: number;
  /** Whether an extension has been applied. */
  readonly extended: boolean;
  /** Reason for the most recent extension, if any. */
  readonly extensionReason: string | null;
  /** Timestamp of allocation creation (ms since epoch). */
  readonly createdAt: number;
}

export interface DepthTiming {
  readonly depth: number;
  readonly startMs: number;
  readonly endMs: number;
  readonly durationMs: number;
  readonly nodesSearched: number;
}

// ── Phase Coefficients ───────────────────────────────────────────────

/**
 * Fraction of total time to use as the soft target.
 * Opening uses less (saving time for deeper iterations), endgame uses more.
 */
const SOFT_TARGET_FRACTION: Record<GamePhase, number> = {
  opening: 0.35,
  midgame: 0.50,
  endgame: 0.70,
};

/**
 * Maximum fraction of total time any single iteration may consume.
 */
const MAX_ITERATION_FRACTION: Record<GamePhase, number> = {
  opening: 0.25,
  midgame: 0.40,
  endgame: 0.55,
};

/**
 * Extension multiplier applied to the soft target when extending.
 */
const EXTENSION_MULTIPLIER: Record<string, number> = {
  instability: 1.5,
  single_reply: 0.3,
};

// ── Time Manager ─────────────────────────────────────────────────────

export class TimeManager {
  private readonly depthTimings: DepthTiming[] = [];
  private emergencyStopped = false;

  /**
   * Create a time allocation for a search invocation.
   */
  allocate(config: SearchConfig, gamePhase: GamePhase): TimeAllocation {
    const now = performance.now();
    const totalBudget = config.timeLimitMs;

    const softTarget = totalBudget * SOFT_TARGET_FRACTION[gamePhase];
    const maxIteration = totalBudget * MAX_ITERATION_FRACTION[gamePhase];

    return {
      totalBudgetMs: totalBudget,
      hardDeadlineMs: now + totalBudget,
      softTargetMs: softTarget,
      maxIterationMs: maxIteration,
      elapsedAtCreation: 0,
      extended: false,
      extensionReason: null,
      createdAt: now,
    };
  }

  /**
   * Determine whether search should stop at the current point.
   *
   * Returns true if:
   *  1. The hard deadline has been reached, or
   *  2. The elapsed time exceeds the soft target and we are between
   *     iterations (not mid-search), or
   *  3. The predicted time for the next depth exceeds the remaining budget.
   *  4. An emergency stop has been triggered.
   */
  shouldStop(elapsed: number, allocation: TimeAllocation): boolean {
    if (this.emergencyStopped) return true;

    // Hard deadline
    if (elapsed >= allocation.totalBudgetMs) return true;

    // Soft target exceeded
    if (elapsed >= allocation.softTargetMs) {
      // Predict next iteration duration using exponential growth model
      const predicted = this.predictNextIterationMs();
      const remaining = allocation.totalBudgetMs - elapsed;

      // Stop if we predict the next iteration will not finish in time
      if (predicted > remaining * 0.9) return true;
    }

    return false;
  }

  /**
   * Extend the time allocation for a specific reason.
   *
   * - `instability`: The best move changed between iterations, indicating
   *   the position is not yet stable. Grant 50% more time.
   * - `single_reply`: Only one reasonable move exists. Cut the remaining
   *   budget to 30% since deep search is unnecessary.
   */
  extend(
    allocation: TimeAllocation,
    reason: 'instability' | 'single_reply',
  ): TimeAllocation {
    const multiplier = EXTENSION_MULTIPLIER[reason];
    const now = performance.now();
    const elapsed = now - allocation.createdAt;
    const remaining = allocation.totalBudgetMs - elapsed;

    const newRemaining = remaining * multiplier;
    const newTotal = elapsed + newRemaining;

    return {
      totalBudgetMs: newTotal,
      hardDeadlineMs: allocation.createdAt + newTotal,
      softTargetMs: allocation.softTargetMs * multiplier,
      maxIterationMs: allocation.maxIterationMs * multiplier,
      elapsedAtCreation: elapsed,
      extended: true,
      extensionReason: reason,
      createdAt: allocation.createdAt,
    };
  }

  /**
   * Record the timing for a completed depth iteration.
   */
  recordDepth(
    depth: number,
    startMs: number,
    endMs: number,
    nodesSearched: number,
  ): void {
    this.depthTimings.push({
      depth,
      startMs,
      endMs,
      durationMs: endMs - startMs,
      nodesSearched,
    });
  }

  /**
   * Get the recorded timing for a specific depth, or null if not recorded.
   */
  getDepthTiming(depth: number): DepthTiming | null {
    return this.depthTimings.find((t) => t.depth === depth) ?? null;
  }

  /**
   * Get all recorded depth timings.
   */
  getAllTimings(): ReadonlyArray<DepthTiming> {
    return this.depthTimings;
  }

  /**
   * Trigger an emergency stop. Once called, `shouldStop` always returns true.
   * This is used when external conditions change (e.g., a new block arrives
   * and the search state is stale).
   */
  emergencyStop(): void {
    this.emergencyStopped = true;
  }

  /**
   * Check whether an emergency stop has been triggered.
   */
  isEmergencyStopped(): boolean {
    return this.emergencyStopped;
  }

  /**
   * Reset all state for a new search session.
   */
  reset(): void {
    this.depthTimings.length = 0;
    this.emergencyStopped = false;
  }

  // ── Private ────────────────────────────────────────────────────────

  /**
   * Predict how long the next depth iteration will take based on
   * the observed growth pattern.
   *
   * Uses the effective branching factor (EBF) derived from the ratio
   * of successive iteration durations. If fewer than two data points
   * exist, returns a conservative estimate.
   */
  private predictNextIterationMs(): number {
    const n = this.depthTimings.length;

    if (n === 0) return Infinity;
    if (n === 1) return this.depthTimings[0]!.durationMs * 4;

    // Compute average branching factor from duration ratios
    let totalRatio = 0;
    let ratioCount = 0;

    for (let i = 1; i < n; i++) {
      const prev = this.depthTimings[i - 1]!.durationMs;
      const curr = this.depthTimings[i]!.durationMs;

      if (prev > 0) {
        totalRatio += curr / prev;
        ratioCount++;
      }
    }

    const avgBranchingFactor =
      ratioCount > 0 ? totalRatio / ratioCount : 4;

    // Predict next duration as last duration * branching factor
    const lastDuration = this.depthTimings[n - 1]!.durationMs;
    return lastDuration * avgBranchingFactor;
  }
}
