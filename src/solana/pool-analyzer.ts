/**
 * MNMX Pool Analyzer
 *
 * Analyzes Solana liquidity pools using constant-product AMM mathematics.
 * Provides depth estimation, price impact calculation, and multi-hop
 * arbitrage route discovery across a set of pools.
 */

import { PublicKey } from '@solana/web3.js';

// ── Types ────────────────────────────────────────────────────────────

export interface PoolAnalysis {
  readonly address: string;
  readonly tokenMintA: string;
  readonly tokenMintB: string;
  readonly reserveA: bigint;
  readonly reserveB: bigint;
  readonly feeRateBps: number;
  readonly spotPrice: number;
  readonly inversePrice: number;
  readonly totalValueLocked: bigint;
  readonly depth: LiquidityDepth;
}

export interface LiquidityDepth {
  /** Amount of token A that can be sold before 1% price impact. */
  readonly depthA1Pct: bigint;
  /** Amount of token B that can be sold before 1% price impact. */
  readonly depthB1Pct: bigint;
  /** Amount of token A that can be sold before 5% price impact. */
  readonly depthA5Pct: bigint;
  /** Amount of token B that can be sold before 5% price impact. */
  readonly depthB5Pct: bigint;
  /** Effective liquidity parameter k = reserveA * reserveB. */
  readonly invariant: bigint;
}

export interface PriceImpact {
  /** Fraction of price moved (0.01 = 1%). */
  readonly impactFraction: number;
  /** Absolute output amount after fees. */
  readonly outputAmount: bigint;
  /** Effective execution price (output / input). */
  readonly executionPrice: number;
  /** Difference between spot and execution price as a fraction. */
  readonly slippage: number;
}

export interface ArbitrageRoute {
  /** Ordered list of pool addresses in the route. */
  readonly path: string[];
  /** Token mints along the route, starting and ending with the same mint. */
  readonly tokenPath: string[];
  /** Expected profit in the starting token, after fees. */
  readonly expectedProfit: bigint;
  /** Total fee cost across all hops expressed in basis points. */
  readonly totalFeeBps: number;
  /** Optimal input amount for maximum profit. */
  readonly optimalInput: bigint;
}

// ── Pool Analyzer ────────────────────────────────────────────────────

export class PoolAnalyzer {
  private readonly poolCache: Map<string, PoolAnalysis> = new Map();

  /**
   * Analyze a pool given its address. In production this would fetch
   * on-chain account data; here it accepts pre-fetched reserve data
   * and computes derived analytics.
   */
  analyzePool(
    poolAddress: PublicKey,
    tokenMintA: string,
    tokenMintB: string,
    reserveA: bigint,
    reserveB: bigint,
    feeRateBps: number,
  ): PoolAnalysis {
    if (reserveA <= 0n || reserveB <= 0n) {
      throw new Error('Reserves must be positive');
    }

    const spotPrice = Number(reserveB) / Number(reserveA);
    const inversePrice = Number(reserveA) / Number(reserveB);
    const depth = this.calculateDepth([reserveA, reserveB], feeRateBps);
    const totalValueLocked = reserveA + reserveB;

    const analysis: PoolAnalysis = {
      address: poolAddress.toBase58(),
      tokenMintA,
      tokenMintB,
      reserveA,
      reserveB,
      feeRateBps,
      spotPrice,
      inversePrice,
      totalValueLocked,
      depth,
    };

    this.poolCache.set(analysis.address, analysis);
    return analysis;
  }

  /**
   * Calculate liquidity depth at 1% and 5% price impact thresholds.
   *
   * For a constant-product AMM with reserves (x, y) and invariant k = x * y,
   * selling dx of token A yields dy = y - k / (x + dx * (1 - fee)).
   * The new spot price is k / (x + dx)^2 compared to the original k / x^2.
   * Price impact = 1 - (x / (x + dx))^2.
   *
   * Solving for dx at a target impact t:
   *   (x / (x + dx))^2 = 1 - t
   *   x / (x + dx) = sqrt(1 - t)
   *   dx = x * (1 / sqrt(1 - t) - 1)
   */
  calculateDepth(
    reserves: [bigint, bigint],
    feeRateBps: number,
  ): LiquidityDepth {
    const [reserveA, reserveB] = reserves;
    const invariant = reserveA * reserveB;
    const feeMultiplier = 1 - feeRateBps / 10_000;

    const depthAtImpact = (reserve: bigint, impactFraction: number): bigint => {
      const sqrtFactor = 1 / Math.sqrt(1 - impactFraction);
      const rawAmount = Number(reserve) * (sqrtFactor - 1);
      const adjustedAmount = rawAmount / feeMultiplier;
      return BigInt(Math.floor(adjustedAmount));
    };

    return {
      depthA1Pct: depthAtImpact(reserveA, 0.01),
      depthB1Pct: depthAtImpact(reserveB, 0.01),
      depthA5Pct: depthAtImpact(reserveA, 0.05),
      depthB5Pct: depthAtImpact(reserveB, 0.05),
      invariant,
    };
  }

  /**
   * Estimate the price impact of swapping a given amount through a pool.
   *
   * Uses the constant-product formula:
   *   dy = y - k / (x + dx * (1 - fee))
   *
   * where dx is the input amount, x is the input reserve, y is the
   * output reserve, and k = x * y.
   */
  estimateImpact(amount: bigint, pool: PoolAnalysis): PriceImpact {
    if (amount <= 0n) {
      return {
        impactFraction: 0,
        outputAmount: 0n,
        executionPrice: pool.spotPrice,
        slippage: 0,
      };
    }

    const feeMultiplier = 10_000n - BigInt(pool.feeRateBps);
    const effectiveInput = (amount * feeMultiplier) / 10_000n;

    const newReserveA = pool.reserveA + effectiveInput;
    const invariant = pool.reserveA * pool.reserveB;
    const newReserveB = invariant / newReserveA;
    const outputAmount = pool.reserveB - newReserveB;

    const executionPrice =
      outputAmount > 0n ? Number(outputAmount) / Number(amount) : 0;

    const slippage =
      pool.spotPrice > 0
        ? Math.abs(pool.spotPrice - executionPrice) / pool.spotPrice
        : 0;

    // Price impact: how much the marginal price moved
    const priceAfter = Number(newReserveB) / Number(newReserveA);
    const impactFraction = Math.abs(pool.spotPrice - priceAfter) / pool.spotPrice;

    return {
      impactFraction,
      outputAmount,
      executionPrice,
      slippage,
    };
  }

  /**
   * Find circular arbitrage routes across a set of analyzed pools.
   *
   * Searches for 2-hop and 3-hop cycles where the output of the last
   * swap exceeds the input of the first, net of fees. Uses a
   * brute-force enumeration bounded by pool count (practical for the
   * typical set of 10-50 monitored pools).
   */
  findArbitrageRoutes(pools: PoolAnalysis[]): ArbitrageRoute[] {
    const routes: ArbitrageRoute[] = [];

    // Build adjacency: token -> list of pools that trade that token
    const tokenToPools = new Map<string, PoolAnalysis[]>();
    for (const pool of pools) {
      const listA = tokenToPools.get(pool.tokenMintA) ?? [];
      listA.push(pool);
      tokenToPools.set(pool.tokenMintA, listA);

      const listB = tokenToPools.get(pool.tokenMintB) ?? [];
      listB.push(pool);
      tokenToPools.set(pool.tokenMintB, listB);
    }

    // Search 2-hop cycles: A->B via pool1, B->A via pool2
    for (const pool1 of pools) {
      const connectedPools = tokenToPools.get(pool1.tokenMintB) ?? [];
      for (const pool2 of connectedPools) {
        if (pool2.address === pool1.address) continue;

        // Check if pool2 can route back to pool1's starting token
        const returnsToA =
          (pool2.tokenMintA === pool1.tokenMintB &&
            pool2.tokenMintB === pool1.tokenMintA) ||
          (pool2.tokenMintB === pool1.tokenMintB &&
            pool2.tokenMintA === pool1.tokenMintA);

        if (!returnsToA) continue;

        const route = this.evaluate2HopRoute(pool1, pool2);
        if (route !== null && route.expectedProfit > 0n) {
          routes.push(route);
        }
      }
    }

    // Search 3-hop cycles: A->B via pool1, B->C via pool2, C->A via pool3
    for (const pool1 of pools) {
      const hop2Pools = tokenToPools.get(pool1.tokenMintB) ?? [];
      for (const pool2 of hop2Pools) {
        if (pool2.address === pool1.address) continue;

        const midToken =
          pool2.tokenMintA === pool1.tokenMintB
            ? pool2.tokenMintB
            : pool2.tokenMintA;

        if (midToken === pool1.tokenMintA) continue; // would be a 2-hop

        const hop3Pools = tokenToPools.get(midToken) ?? [];
        for (const pool3 of hop3Pools) {
          if (pool3.address === pool1.address || pool3.address === pool2.address) {
            continue;
          }
