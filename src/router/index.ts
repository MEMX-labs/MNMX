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
    logger.info('Registered bridge: ' + adapter.name);
  }

  /**
   * Remove a bridge adapter by name.
   */
  removeBridge(name: string): void {
    this.registry.remove(name);
    logger.info('Removed bridge: ' + name);
  }

  /**
   * Get all supported chains from registered bridges.
   */
  getSupportedChains(): Chain[] {
    const chainSet = new Set<Chain>();
    for (const adapter of this.registry.getAll()) {
      for (const chain of adapter.supportedChains) {
        chainSet.add(chain);
      }
    }
    return Array.from(chainSet);
  }

  /**
   * Get all registered bridge names.
   */
  getSupportedBridges(): string[] {
    return this.registry.getNames();
  }

  /**
   * Get the current router configuration.
   */
  getConfig(): RouterConfig {
    return { ...this.config };
  }

  /**
   * Update router configuration.
   */
  updateConfig(config: Partial<RouterConfig>): void {
    this.config = this._mergeConfig(config);
    this.minimaxEngine.setOptions({
      weights: this.config.weights,
      adversarialModel: this.config.adversarialModel,
      strategy: this.config.strategy,
      timeoutMs: this.config.timeout,
    });
    logger.setLevel(this.config.logLevel);
  }

  /**
   * Find the optimal route for a given request.
   * Uses minimax search with alpha-beta pruning.
   */
  async findRoute(request: RouteRequest): Promise<RouteResult> {
    const requestId = generateRequestId();
    logger.info(
      'Finding route [' + requestId + ']: ' +
      request.from.chain + '/' + request.from.token +
      ' -> ' + request.to.chain + '/' + request.to.token
    );

    this._validateRequest(request);

    const strategy = request.options?.strategy ?? this.config.strategy;
    const maxHops = request.options?.maxHops ?? this.config.maxHops;
    const excludeBridges = [
      ...this.config.excludeBridges,
      ...(request.options?.excludeBridges ?? []),
    ];
    const excludeChains = request.options?.excludeChains ?? [];

    const fromToken = this._resolveToken(request.from.chain, request.from.token);
    const toToken = this._resolveToken(request.to.chain, request.to.token);

    // Step 1: Discover chain-level paths
    const chainPaths = discoverChainPaths(
      request.from.chain,
      request.to.chain,
      this.registry,
      { maxHops, excludeBridges, excludeChains, minLiquidity: this.config.minLiquidity },
    );

    if (chainPaths.length === 0) {
      logger.warn('No chain paths found for ' + request.from.chain + ' -> ' + request.to.chain);
      return { bestRoute: null, alternatives: [], stats: EMPTY_STATS, requestId };
    }

    logger.debug('Found ' + chainPaths.length + ' chain-level paths');

    // Step 2: Filter dominated paths
    const filteredPaths = filterDominatedPaths(chainPaths);
    logger.debug('After filtering: ' + filteredPaths.length + ' paths');

    // Step 3: Build candidate paths with quotes
    const candidates = await buildCandidatePaths(
      filteredPaths,
      fromToken,
      toToken,
      request.from.amount,
      this.registry,
      { maxHops, excludeBridges, excludeChains, minLiquidity: this.config.minLiquidity },
    );

    if (candidates.length === 0) {
      logger.warn('No viable candidate paths with quotes');
