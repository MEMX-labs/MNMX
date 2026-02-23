// ─────────────────────────────────────────────────────────────
// MNMX Router
// Main entry point for cross-chain route discovery and execution
// ─────────────────────────────────────────────────────────────

import type {
  Route,
  RouteRequest,
  RouterConfig,
  Strategy,
  SearchStats,
  Token,
  Chain,
  ExecOpts,
  ExecutionResult,
  BridgeStatus,
  ProgressEvent,
  ScoringWeights,
  CandidatePath,
} from '../types/index.js';
import {
  DEFAULT_ROUTER_CONFIG,
  STRATEGY_WEIGHTS,
  ALL_CHAINS,
} from '../types/index.js';
import { BridgeRegistry } from '../bridges/adapter.js';
import type { BridgeAdapter } from '../bridges/adapter.js';
import { findToken } from '../chains/index.js';
import {
  discoverChainPaths,
  filterDominatedPaths,
  buildCandidatePaths,
  PathDiscovery,
} from './path-discovery.js';
import {
  getWeightsForStrategy,
  rankCandidates,
  scoreRoute,
  getScoreBreakdown,
} from './scoring.js';
import {
  minimaxSearchWithPruning,
  minimaxSearch,
  iterativeDeepening,
  MinimaxEngine,
} from './minimax.js';
import type { MinimaxResult, MinimaxOptions } from './minimax.js';
import { createLogger } from '../utils/logger.js';
import { generateRequestId } from '../utils/hash.js';

const logger = createLogger('router');

// ─────────────────────────────────────────────────────────────
// Types
// ─────────────────────────────────────────────────────────────

export interface RouteResult {
  bestRoute: Route | null;
  alternatives: Route[];
  stats: SearchStats;
  requestId: string;
}

const EMPTY_STATS: SearchStats = {
  nodesExplored: 0,
  nodesPruned: 0,
  maxDepthReached: 0,
  searchTimeMs: 0,
  candidateCount: 0,
  quotesFetched: 0,
};

// ─────────────────────────────────────────────────────────────
// Router
// ─────────────────────────────────────────────────────────────

/**
 * MnmxRouter is the main entry point for the MNMX cross-chain routing protocol.
 * It coordinates path discovery, minimax search, scoring, and execution.
 */
export class MnmxRouter {
  private config: RouterConfig;
  private registry: BridgeRegistry;
  private pathDiscovery: PathDiscovery;
  private minimaxEngine: MinimaxEngine;

  constructor(config?: Partial<RouterConfig>) {
    this.config = this._mergeConfig(config);
    this.registry = new BridgeRegistry();
    this.pathDiscovery = new PathDiscovery(this.registry, {
      maxHops: this.config.maxHops,
      excludeBridges: this.config.excludeBridges,
      minLiquidity: this.config.minLiquidity,
    });
    this.minimaxEngine = new MinimaxEngine({
      weights: this.config.weights,
      adversarialModel: this.config.adversarialModel,
      strategy: this.config.strategy,
      timeoutMs: this.config.timeout,
    });
    logger.setLevel(this.config.logLevel);
  }

  /**
   * Register a bridge adapter with the router.
   */
  registerBridge(adapter: BridgeAdapter): void {
    this.registry.register(adapter);
