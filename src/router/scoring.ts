// ─────────────────────────────────────────────────────────────
// Route Scoring Engine
// Multi-dimensional scoring for route comparison
// ─────────────────────────────────────────────────────────────

import type {
  Route,
  RouteHop,
  ScoringWeights,
  Strategy,
  CandidatePath,
  AdversarialModel,
} from '../types/index.js';
import { STRATEGY_WEIGHTS } from '../types/index.js';

/**
 * Maximum reference values for normalization.
 */
const MAX_FEE_RATIO = 0.10;
const MAX_SLIPPAGE_BPS = 200;
const MAX_TIME_SECONDS = 1800;
const MIN_RELIABILITY = 0.80;
const MAX_MEV_RATIO = 0.05;

/**
 * Normalize a fee as a fraction of input amount.
 * Lower fees produce higher scores (closer to 1.0).
 */
export function normalizeFee(fee: number, inputAmount: number): number {
  if (inputAmount <= 0) return 0;
  const feeRatio = fee / inputAmount;
  return Math.max(0, Math.min(1, 1 - feeRatio / MAX_FEE_RATIO));
}

/**
 * Normalize speed: map estimated time in seconds to a 0-1 score.
 * Uses MAX_TIME_SECONDS as the worst acceptable time.
 */
export function normalizeSpeed(estimatedTimeSeconds: number): number {
  if (estimatedTimeSeconds <= 0) return 1;
  if (estimatedTimeSeconds >= MAX_TIME_SECONDS) return 0;
  return 1 - estimatedTimeSeconds / MAX_TIME_SECONDS;
}

/**
 * Normalize reliability from a 0-1 success rate.
 * For multi-hop routes, multiply per-hop reliabilities.
 */
export function normalizeReliability(perHopRates: number[]): number {
  if (perHopRates.length === 0) return 0;
  let compound = 1;
  for (const rate of perHopRates) {
    compound *= Math.max(0, Math.min(1, rate));
  }
  return compound;
}

/**
 * Normalize slippage from basis points to a 0-1 score.
 * 0 bps = 1.0, MAX_SLIPPAGE_BPS+ bps = 0.0.
 */
export function normalizeSlippage(slippageBps: number): number {
  if (slippageBps <= 0) return 1;
  if (slippageBps >= MAX_SLIPPAGE_BPS) return 0;
  return 1 - slippageBps / MAX_SLIPPAGE_BPS;
}

/**
 * Normalize MEV exposure as a fraction of input.
 * Lower MEV exposure = higher score.
 */
export function normalizeMevExposure(mevAmount: number, inputAmount: number): number {
  if (inputAmount <= 0) return 1;
  const ratio = mevAmount / inputAmount;
  return Math.max(0, Math.min(1, 1 - ratio / MAX_MEV_RATIO));
}

/**
 * Compute a composite score for a route given individual scores and weights.
 */
export function computeScore(
  feeScore: number,
  slippageScore: number,
  speedScore: number,
  reliabilityScore: number,
  mevScore: number,
  weights: ScoringWeights,
): number {
  return (
    feeScore * weights.fees +
    slippageScore * weights.slippage +
    speedScore * weights.speed +
    reliabilityScore * weights.reliability +
    mevScore * weights.mevExposure
  );
}

/**
 * Get weights for a given strategy.
 */
export function getWeightsForStrategy(strategy: Strategy): ScoringWeights {
  return STRATEGY_WEIGHTS[strategy];
}

/**
 * Verify that weights sum to 1.0 (within tolerance).
 */
export function weightsAreValid(weights: ScoringWeights): boolean {
  const sum =
    weights.fees +
    weights.slippage +
    weights.speed +
    weights.reliability +
    weights.mevExposure;
  return Math.abs(sum - 1.0) < 1e-6;
}

/**
 * Normalize weights so they sum to 1.0.
 */
export function normalizeWeights(weights: ScoringWeights): ScoringWeights {
  const sum =
    weights.fees +
    weights.slippage +
    weights.speed +
    weights.reliability +
    weights.mevExposure;
  if (sum === 0) return getWeightsForStrategy('minimax');
  return {
    fees: weights.fees / sum,
    slippage: weights.slippage / sum,
    speed: weights.speed / sum,
    reliability: weights.reliability / sum,
    mevExposure: weights.mevExposure / sum,
  };
}

/**
 * Compare two routes by their minimax scores (descending).
 * Returns negative if a should come first, positive if b should come first.
 */
export function compareRoutes(a: Route, b: Route): number {
  return b.minimaxScore - a.minimaxScore;
}

/**
 * Estimate MEV exposure for a single hop.
 * MEV risk depends on chain mempool visibility, amount, and time in-flight.
 */
export function estimateHopMevExposure(hop: RouteHop): number {
  const amount = parseFloat(hop.inputAmount);
  const timeFraction = hop.estimatedTime / 3600;
  const chainFactor = getChainMevFactor(hop.fromChain);
  return amount * timeFraction * chainFactor * 0.001;
}

/**
 * MEV risk factor by chain. Higher = more MEV risk.
 */
function getChainMevFactor(chain: string): number {
  const factors: Record<string, number> = {
    ethereum: 1.0,
    arbitrum: 0.4,
    base: 0.3,
    optimism: 0.35,
    polygon: 0.5,
    bnb: 0.45,
    avalanche: 0.3,
    solana: 0.6,
  };
  return factors[chain] ?? 0.5;
}

/**
 * Estimate reliability from liquidity depth relative to transfer amount.
 */
export function estimateReliabilityFromLiquidity(
  liquidityDepth: number,
  amount: number,
): number {
  if (amount <= 0) return 0.95;
  const ratio = liquidityDepth / Math.max(amount, 1);
  const base = 0.80;
  const ceiling = 0.995;
  return Math.min(ceiling, base + (ceiling - base) * (1 - 1 / (1 + ratio * 0.5)));
}

/**
 * Score a complete Route object using all dimensions.
 */
export function scoreRoute(route: Route, weights: ScoringWeights): number {
  if (route.path.length === 0) return 0;

  const totalInput = parseFloat(route.path[0].inputAmount);
  const totalFees = parseFloat(route.totalFees);

  const feeScore = normalizeFee(totalFees, totalInput);

  const totalSlippage = route.path.reduce((s, h) => s + h.slippageBps, 0);
  const slippageScore = normalizeSlippage(totalSlippage);

  const speedScore = normalizeSpeed(route.estimatedTime);

  const perHopReliability = route.path.map((h) =>
    estimateReliabilityFromLiquidity(h.liquidityDepth, parseFloat(h.inputAmount))
  );
  const reliabilityScore = normalizeReliability(perHopReliability);

  let totalMev = 0;
  for (const hop of route.path) {
    totalMev += estimateHopMevExposure(hop);
  }
  const mevScore = normalizeMevExposure(totalMev, totalInput);

  return computeScore(feeScore, slippageScore, speedScore, reliabilityScore, mevScore, weights);
}

/**
 * Score and rank a list of candidate paths.
 */
export function rankCandidates(
  candidates: CandidatePath[],
  inputAmount: number,
  weights: ScoringWeights,
): CandidatePath[] {
  const scored = candidates.map((c) => {
    const feeScore = normalizeFee(c.estimatedFee, inputAmount);
    const speedScore = normalizeSpeed(c.estimatedTime);
    const avgSlippage =
      c.quotes.length > 0
        ? c.quotes.reduce((s, q) => s + q.slippageBps, 0) / c.quotes.length
        : 0;
    const slippageScore = normalizeSlippage(avgSlippage);

    // Reliability decreases with more hops
    const reliabilityScore = Math.max(0, 1 - c.chains.length * 0.02);

    // MEV estimate based on total time and chain exposure
    const totalTime = c.estimatedTime;
    const mevFraction = (totalTime / 3600) * 0.001;
    const mevAmount = inputAmount * mevFraction;
    const mevScore = normalizeMevExposure(mevAmount, inputAmount);

    const score = computeScore(
      feeScore,
      slippageScore,
      speedScore,
      reliabilityScore,
      mevScore,
      weights,
    );

    return { ...c, roughScore: score };
  });

  scored.sort((a, b) => b.roughScore - a.roughScore);
  return scored;
}

/**
 * Detailed score breakdown for a route.
 */
export interface ScoreBreakdown {
  overall: number;
  fees: number;
  slippage: number;
  speed: number;
  reliability: number;
  mevExposure: number;
  weights: ScoringWeights;
}

/**
 * Get a detailed score breakdown for a route.
 */
export function getScoreBreakdown(
  route: Route,
  weights: ScoringWeights,
): ScoreBreakdown {
  if (route.path.length === 0) {
    return {
      overall: 0,
      fees: 0,
      slippage: 0,
      speed: 0,
      reliability: 0,
      mevExposure: 0,
      weights,
    };
  }

  const totalInput = parseFloat(route.path[0].inputAmount);
  const totalFees = parseFloat(route.totalFees);

  const fees = normalizeFee(totalFees, totalInput);
  const totalSlippage = route.path.reduce((s, h) => s + h.slippageBps, 0);
  const slippage = normalizeSlippage(totalSlippage);
  const speed = normalizeSpeed(route.estimatedTime);

  const perHopReliability = route.path.map((h) =>
    estimateReliabilityFromLiquidity(h.liquidityDepth, parseFloat(h.inputAmount))
  );
  const reliability = normalizeReliability(perHopReliability);

  let totalMev = 0;
  for (const hop of route.path) {
    totalMev += estimateHopMevExposure(hop);
  }
  const mevExposure = normalizeMevExposure(totalMev, totalInput);

  const overall = computeScore(fees, slippage, speed, reliability, mevExposure, weights);

  return { overall, fees, slippage, speed, reliability, mevExposure, weights };
}
