// ─────────────────────────────────────────────────────────────
// Minimax Search Engine
// Game-tree search for optimal cross-chain routing
// ─────────────────────────────────────────────────────────────

import type {
  CandidatePath,
  Route,
  RouteHop,
  Strategy,
  ScoringWeights,
  AdversarialModel,
  SearchStats,
  BridgeQuote,
  Chain,
  Token,
} from '../types/index.js';
import {
  DEFAULT_ROUTER_CONFIG,
} from '../types/index.js';
import {
  normalizeFee,
  normalizeSpeed,
  normalizeSlippage,
  normalizeMevExposure,
  computeScore,
} from './scoring.js';

// ─────────────────────────────────────────────────────────────
// Types
// ─────────────────────────────────────────────────────────────

export interface MinimaxOptions {
  maxDepth: number;
  weights: ScoringWeights;
  adversarialModel: AdversarialModel;
  strategy: Strategy;
  /** Timeout in ms; search stops early if exceeded */
  timeoutMs?: number;
}

export interface MinimaxResult {
  bestRoute: Route | null;
  allRoutes: Route[];
  stats: SearchStats;
}

/**
 * A node in the minimax search tree.
 * Each node represents a state after applying some set of adversarial
 * conditions to a candidate path.
 */
export interface SearchNode {
  /** The candidate being evaluated */
  candidate: CandidatePath;
  /** Current adversarial scenario index */
  scenarioIndex: number;
  /** Depth in the search tree */
  depth: number;
  /** Score at this node */
  score: number;
  /** Whether this is a maximizing node (router) or minimizing (adversary) */
  isMaximizing: boolean;
  /** Child nodes */
  children: SearchNode[];
}

/**
 * An adversarial scenario: a specific configuration of worst-case parameters.
 */
interface AdversarialScenario {
  label: string;
  slippageMultiplier: number;
  gasMultiplier: number;
  bridgeDelayMultiplier: number;
  mevExtraction: number;
  priceMovement: number;
  failureProbability: number;
}

// ─────────────────────────────────────────────────────────────
// Adversarial Scenarios
// ─────────────────────────────────────────────────────────────

/**
 * Generate a set of adversarial scenarios from the base model.
 * Each scenario represents a different "move" the adversary can make.
 */
function generateScenarios(model: AdversarialModel): AdversarialScenario[] {
  return [
    // Base adversarial scenario
    {
      label: 'base',
      slippageMultiplier: model.slippageMultiplier,
      gasMultiplier: model.gasMultiplier,
      bridgeDelayMultiplier: model.bridgeDelayMultiplier,
      mevExtraction: model.mevExtraction,
      priceMovement: model.priceMovement,
      failureProbability: model.failureProbability,
    },
    // High slippage scenario
    {
      label: 'high-slippage',
      slippageMultiplier: model.slippageMultiplier * 2.0,
      gasMultiplier: model.gasMultiplier,
      bridgeDelayMultiplier: model.bridgeDelayMultiplier,
      mevExtraction: model.mevExtraction,
      priceMovement: model.priceMovement * 1.5,
      failureProbability: model.failureProbability,
    },
    // High gas scenario
    {
      label: 'high-gas',
      slippageMultiplier: model.slippageMultiplier,
      gasMultiplier: model.gasMultiplier * 2.5,
      bridgeDelayMultiplier: model.bridgeDelayMultiplier,
      mevExtraction: model.mevExtraction,
      priceMovement: model.priceMovement,
      failureProbability: model.failureProbability,
    },
    // High delay scenario (bridge congestion)
    {
      label: 'congestion',
      slippageMultiplier: model.slippageMultiplier * 1.2,
      gasMultiplier: model.gasMultiplier * 1.5,
      bridgeDelayMultiplier: model.bridgeDelayMultiplier * 3.0,
      mevExtraction: model.mevExtraction * 1.5,
      priceMovement: model.priceMovement * 2.0,
      failureProbability: model.failureProbability * 2,
    },
    // MEV attack scenario
    {
      label: 'mev-attack',
      slippageMultiplier: model.slippageMultiplier * 1.5,
      gasMultiplier: model.gasMultiplier * 1.2,
      bridgeDelayMultiplier: model.bridgeDelayMultiplier,
      mevExtraction: model.mevExtraction * 5.0,
      priceMovement: model.priceMovement * 3.0,
      failureProbability: model.failureProbability,
    },
  ];
}

// ─────────────────────────────────────────────────────────────
// Core Minimax Functions
// ─────────────────────────────────────────────────────────────

/**
 * Evaluate a candidate path under normal conditions.
 */
function evaluateCandidate(
  candidate: CandidatePath,
  inputAmount: number,
  weights: ScoringWeights,
): number {
  const feeScore = normalizeFee(candidate.estimatedFee, inputAmount);
  const speedScore = normalizeSpeed(candidate.estimatedTime);
  const avgSlippage =
    candidate.quotes.length > 0
      ? candidate.quotes.reduce((s, q) => s + q.slippageBps, 0) / candidate.quotes.length
      : 0;
  const slippageScore = normalizeSlippage(avgSlippage);

  // Reliability degrades with more hops
  const reliabilityScore = Math.max(0, 1 - candidate.chains.length * 0.02);

  // MEV estimate
  const timeFraction = candidate.estimatedTime / 3600;
  const mevAmount = inputAmount * timeFraction * 0.001;
  const mevScore = normalizeMevExposure(mevAmount, inputAmount);

  return computeScore(feeScore, slippageScore, speedScore, reliabilityScore, mevScore, weights);
}

/**
 * Evaluate a candidate path under an adversarial scenario.
 * The adversary degrades fees, slippage, delay, and applies MEV extraction.
 */
function evaluateAdversarial(
  candidate: CandidatePath,
  inputAmount: number,
  weights: ScoringWeights,
  scenario: AdversarialScenario,
): number {
  const adjustedFee = candidate.estimatedFee * scenario.gasMultiplier;
  const feeScore = normalizeFee(adjustedFee, inputAmount);

  const adjustedTime = candidate.estimatedTime * scenario.bridgeDelayMultiplier;
  const speedScore = normalizeSpeed(adjustedTime);

  const avgSlippage =
    candidate.quotes.length > 0
      ? candidate.quotes.reduce((s, q) => s + q.slippageBps, 0) / candidate.quotes.length
      : 0;
  const adjustedSlippage = avgSlippage * scenario.slippageMultiplier;
  const slippageScore = normalizeSlippage(adjustedSlippage);

  // Reliability decreases with more hops and higher failure probability
  const hopCount = candidate.chains.length - 1;
  const perHopSuccess = 1 - scenario.failureProbability;
  const compoundReliability = Math.pow(perHopSuccess, hopCount);
  const reliabilityScore = Math.max(0, compoundReliability * (1 - hopCount * 0.01));

  // MEV under attack
  const mevAmount = inputAmount * scenario.mevExtraction;
  const mevScore = normalizeMevExposure(mevAmount, inputAmount);

  return computeScore(feeScore, slippageScore, speedScore, reliabilityScore, mevScore, weights);
}

/**
 * Apply adversarial degradation to expected output.
 */
function applyAdversarialToOutput(
  expectedOutput: number,
  scenario: AdversarialScenario,
