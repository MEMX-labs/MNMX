import { describe, it, expect } from 'vitest';
import { normalizeFee, normalizeSpeed, computeScore } from '../../src/router/scoring.js';
import { STRATEGY_WEIGHTS } from '../../src/types/index.js';

describe('Zero Amount Edge Cases', () => {
  it('normalizeFee handles zero amount without NaN', () => {
    const result = normalizeFee(0, 0);
    expect(result).toBe(0);
    expect(Number.isNaN(result)).toBe(false);
  });

  it('normalizeFee handles negative fee', () => {
    const result = normalizeFee(-5, 1000);
    expect(result).toBeGreaterThanOrEqual(0);
    expect(result).toBeLessThanOrEqual(1);
  });

  it('normalizeSpeed handles extremely small time', () => {
    const result = normalizeSpeed(0.001);
    expect(result).toBeCloseTo(1, 2);
  });

  it('computeScore returns finite number for zero dimensions', () => {
    const score = computeScore(
      { feeScore: 0, slippageScore: 0, speedScore: 0, reliabilityScore: 0, mevScore: 0 },
      STRATEGY_WEIGHTS.minimax
    );
    expect(Number.isFinite(score)).toBe(true);
    expect(score).toBe(0);
  });

  it('computeScore returns 1.0 for perfect scores', () => {
    const score = computeScore(
      { feeScore: 1, slippageScore: 1, speedScore: 1, reliabilityScore: 1, mevScore: 1 },
      STRATEGY_WEIGHTS.minimax
    );
    expect(score).toBeCloseTo(1, 2);
  });
});
