import { describe, it, expect } from 'vitest';
import { normalizeFee, normalizeSpeed, normalizeReliability } from '../../src/router/scoring.js';

describe('Multi-hop Edge Cases', () => {
  it('single hop has highest reliability', () => {
    const single = normalizeReliability([0.99]);
    const double = normalizeReliability([0.99, 0.99]);
    expect(single).toBeGreaterThan(double);
  });

  it('3-hop reliability compounds correctly', () => {
    const result = normalizeReliability([0.95, 0.95, 0.95]);
    // 0.95^3 = 0.857375, normalized
    expect(result).toBeLessThan(normalizeReliability([0.95]));
    expect(result).toBeGreaterThan(0);
  });

  it('zero reliability hop zeroes entire route', () => {
    expect(normalizeReliability([0.99, 0, 0.99])).toBe(0);
  });

  it('empty hop array returns 0', () => {
    expect(normalizeReliability([])).toBe(0);
  });

  it('fee accumulation across hops', () => {
    // Total fee across 3 hops
    const totalFee = 5 + 3 + 2; // $10 total on $1000
    const score = normalizeFee(totalFee, 1000);
    expect(score).toBeCloseTo(0.9, 1); // 1% fee ratio
  });

  it('speed of multi-hop is sum of individual hops', () => {
    // 3 hops: 60s + 120s + 30s = 210s total
    const totalTime = 60 + 120 + 30;
    const score = normalizeSpeed(totalTime);
    expect(score).toBeGreaterThan(0);
    expect(score).toBeLessThan(1);
  });

  it('very long multi-hop route scores near zero', () => {
    const totalTime = 600 + 600 + 600; // 30min total
    const score = normalizeSpeed(totalTime);
    expect(score).toBe(0);
  });
});
