import { describe, it, expect } from 'vitest';
import {
  normalizeFee,
  normalizeSpeed,
  normalizeSlippage,
  normalizeReliability,
  computeScore,
  getWeightsForStrategy,
  weightsAreValid,
  compareRoutes,
} from '../../src/router/scoring.js';
import { STRATEGY_WEIGHTS } from '../../src/types/index.js';
import type { Route, Strategy, ScoringWeights } from '../../src/types/index.js';

describe('scoring', () => {
  describe('weight validation', () => {
    it('all strategy weights sum to 1.0', () => {
      const strategies: Strategy[] = ['minimax', 'cheapest', 'fastest', 'safest'];
      for (const strategy of strategies) {
        const weights = getWeightsForStrategy(strategy);
        expect(weightsAreValid(weights)).toBe(true);
        const sum = weights.fees + weights.slippage + weights.speed + weights.reliability + weights.mevExposure;
        expect(Math.abs(sum - 1.0)).toBeLessThan(1e-6);
      }
    });

    it('STRATEGY_WEIGHTS presets match getWeightsForStrategy', () => {
      const strategies: Strategy[] = ['minimax', 'cheapest', 'fastest', 'safest'];
      for (const s of strategies) {
        expect(getWeightsForStrategy(s)).toEqual(STRATEGY_WEIGHTS[s]);
      }
    });

    it('detects invalid weights', () => {
      const bad: ScoringWeights = { fees: 0.5, slippage: 0.5, speed: 0.5, reliability: 0.5, mevExposure: 0.5 };
      expect(weightsAreValid(bad)).toBe(false);
    });
  });

  describe('fee normalization', () => {
    it('normalizes zero fee to 1.0', () => {
      expect(normalizeFee(0, 1000)).toBe(1);
    });

    it('normalizes high fee to 0.0', () => {
      // 10% fee ratio = MAX_FEE_RATIO => score 0
      expect(normalizeFee(100, 1000)).toBe(0);
    });

    it('normalizes proportionally', () => {
      // 5% fee = 0.05/0.10 = 0.5 ratio => score = 1 - 0.5 = 0.5
      const score = normalizeFee(50, 1000);
      expect(score).toBeCloseTo(0.5, 2);
    });

    it('handles zero input amount', () => {
      expect(normalizeFee(10, 0)).toBe(0);
    });

    it('clamps negative scores to 0', () => {
      // Fee > 10% of input should clamp to 0
      expect(normalizeFee(200, 1000)).toBe(0);
    });
  });

  describe('speed normalization', () => {
    it('maps instant transfer to 1.0', () => {
      expect(normalizeSpeed(0)).toBe(1);
    });

    it('maps max time transfer to 0.0', () => {
      expect(normalizeSpeed(1800)).toBe(0);
    });

    it('maps intermediate values proportionally', () => {
      const score = normalizeSpeed(900);
      expect(score).toBeCloseTo(0.5, 2);
    });

    it('clamps values beyond max time', () => {
      expect(normalizeSpeed(3600)).toBe(0);
    });
  });

  describe('route comparison ordering', () => {
    it('orders routes by minimax score descending', () => {
      const routeA = { minimaxScore: 0.85 } as Route;
      const routeB = { minimaxScore: 0.72 } as Route;
      expect(compareRoutes(routeA, routeB)).toBeLessThan(0);
      expect(compareRoutes(routeB, routeA)).toBeGreaterThan(0);
      expect(compareRoutes(routeA, routeA)).toBe(0);
    });
  });

  describe('strategy weight presets', () => {
    it('cheapest strategy weights fees most heavily', () => {
      const w = STRATEGY_WEIGHTS.cheapest;
      expect(w.fees).toBeGreaterThan(w.slippage);
      expect(w.fees).toBeGreaterThan(w.speed);
      expect(w.fees).toBeGreaterThan(w.reliability);
      expect(w.fees).toBeGreaterThan(w.mevExposure);
    });

    it('fastest strategy weights speed most heavily', () => {
      const w = STRATEGY_WEIGHTS.fastest;
      expect(w.speed).toBeGreaterThan(w.fees);
      expect(w.speed).toBeGreaterThan(w.slippage);
      expect(w.speed).toBeGreaterThan(w.reliability);
      expect(w.speed).toBeGreaterThan(w.mevExposure);
    });

    it('safest strategy weights reliability most heavily', () => {
      const w = STRATEGY_WEIGHTS.safest;
      expect(w.reliability).toBeGreaterThan(w.fees);
      expect(w.reliability).toBeGreaterThan(w.slippage);
      expect(w.reliability).toBeGreaterThan(w.speed);
      expect(w.reliability).toBeGreaterThan(w.mevExposure);
    });
  });
});
