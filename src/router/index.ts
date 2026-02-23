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
      return { bestRoute: null, alternatives: [], stats: EMPTY_STATS, requestId };
    }

    logger.debug('Built ' + candidates.length + ' candidate paths with quotes');

    // Step 4: Apply strategy
    const result = this._applyStrategy(
      strategy, candidates, parseFloat(request.from.amount), request,
    );

    if (result.bestRoute) {
      logger.info(
        'Route found [' + requestId + ']: score=' +
        result.bestRoute.minimaxScore.toFixed(4) +
        ', alternatives=' + (result.allRoutes.length - 1) +
        ', explored=' + result.stats.nodesExplored +
        ', pruned=' + result.stats.nodesPruned
      );
    }

    return {
      bestRoute: result.bestRoute,
      alternatives: result.allRoutes.slice(1),
      stats: result.stats,
      requestId,
    };
  }

  /**
   * Find all routes sorted by score.
   */
  async findAllRoutes(request: RouteRequest): Promise<Route[]> {
    const result = await this.findRoute(request);
    const allRoutes: Route[] = [];
    if (result.bestRoute) allRoutes.push(result.bestRoute);
    allRoutes.push(...result.alternatives);
    return allRoutes;
  }

  /**
   * Execute a route by sending transactions through each bridge hop.
   */
  async execute(route: Route, opts: ExecOpts): Promise<ExecutionResult> {
    logger.info('Executing route ' + route.routeId + ' (' + route.path.length + ' hops)');

    const startTime = Date.now();
    const hopTxHashes: string[] = [];
    let currentStatus: BridgeStatus = 'pending';
    let lastError: string | undefined;

    // Validate route expiry
    if (Date.now() > route.expiresAt) {
      return {
        txHash: '',
        route,
        actualOutput: '0',
        executionTime: Date.now() - startTime,
        status: 'failed',
        hopTxHashes: [],
        error: 'Route has expired',
      };
    }

    // Dry run
    if (opts.dryRun) {
      logger.info('Dry run mode - simulating execution');
      for (let i = 0; i < route.path.length; i++) {
        const hop = route.path[i];
        this._emitProgress(opts, i, route.path.length, 'completed', undefined,
          '[DRY RUN] Hop ' + (i + 1) + ': ' + hop.fromChain + ' -> ' + hop.toChain + ' via ' + hop.bridge);
        hopTxHashes.push('0x' + '0'.repeat(64));
      }
      return {
        txHash: hopTxHashes[0] ?? '',
        route,
        actualOutput: route.expectedOutput,
        executionTime: Date.now() - startTime,
        status: 'completed',
        hopTxHashes,
      };
    }

    // Execute hops sequentially
    for (let i = 0; i < route.path.length; i++) {
      const hop = route.path[i];
      const bridge = this.registry.get(hop.bridge);

      if (!bridge) {
        lastError = 'Bridge not found: ' + hop.bridge;
        currentStatus = 'failed';
        this._emitProgress(opts, i, route.path.length, 'failed', undefined, lastError);
        break;
      }

      this._emitProgress(opts, i, route.path.length, 'pending', undefined,
        'Initiating hop ' + (i + 1) + ': ' + hop.fromChain + ' -> ' + hop.toChain + ' via ' + hop.bridge);

      try {
        // Fresh quote
        const freshQuote = await bridge.getQuote({
          fromChain: hop.fromChain,
          toChain: hop.toChain,
          fromToken: hop.fromToken,
          toToken: hop.toToken,
          amount: hop.inputAmount,
          slippageTolerance: this.config.slippageTolerance,
        });

        // Execute
