// ─────────────────────────────────────────────────────────────
// Path Discovery
// Finds candidate paths between source and destination chains
// ─────────────────────────────────────────────────────────────

import type { Chain, CandidatePath, Token, BridgeQuote } from '../types/index.js';
import { ALL_CHAINS } from '../types/index.js';
import type { BridgeAdapter } from '../bridges/adapter.js';
import { BridgeRegistry } from '../bridges/adapter.js';
import { findToken } from '../chains/index.js';

export interface PathDiscoveryOptions {
  maxHops: number;
  excludeBridges: string[];
  excludeChains: Chain[];
  minLiquidity: number;
}

/**
 * Chain connectivity map.
 * Defines which chains have strong bridge connectivity to each other.
 * This is used for heuristic path ordering, not hard filtering.
 */
export const CHAIN_CONNECTIVITY: Record<Chain, Chain[]> = {
  ethereum: ['arbitrum', 'optimism', 'base', 'polygon', 'bnb', 'avalanche', 'solana'],
  solana: ['ethereum', 'polygon', 'bnb', 'avalanche'],
  arbitrum: ['ethereum', 'optimism', 'base', 'polygon'],
  base: ['ethereum', 'arbitrum', 'optimism', 'polygon'],
  polygon: ['ethereum', 'arbitrum', 'bnb', 'avalanche', 'solana'],
  bnb: ['ethereum', 'polygon', 'avalanche', 'solana'],
  optimism: ['ethereum', 'arbitrum', 'base', 'polygon'],
  avalanche: ['ethereum', 'polygon', 'bnb'],
};

/**
 * Preferred intermediate tokens per chain.
 * These are the most liquid tokens available for bridging on each chain.
 */
export const INTERMEDIATE_TOKENS: Record<Chain, string[]> = {
  ethereum: ['USDC', 'USDT', 'WETH', 'DAI'],
  solana: ['USDC', 'USDT', 'SOL'],
  arbitrum: ['USDC', 'USDT', 'WETH'],
  base: ['USDC', 'WETH'],
  polygon: ['USDC', 'USDT', 'WETH'],
  bnb: ['USDC', 'USDT', 'BNB'],
  optimism: ['USDC', 'WETH', 'OP'],
  avalanche: ['USDC', 'USDT', 'AVAX'],
};

/**
 * Build an adjacency map of chain -> chain -> bridge adapters.
 */
function buildGraph(
  registry: BridgeRegistry,
  excludeBridges: string[],
): Map<Chain, Map<Chain, BridgeAdapter[]>> {
  const graph = new Map<Chain, Map<Chain, BridgeAdapter[]>>();
  for (const adapter of registry.getAll()) {
    if (excludeBridges.includes(adapter.name)) continue;
    const chains = adapter.supportedChains;
    for (const from of chains) {
      for (const to of chains) {
        if (from === to) continue;
        if (!adapter.supportsRoute(from, to)) continue;
        if (!graph.has(from)) graph.set(from, new Map());
        const inner = graph.get(from)!;
        if (!inner.has(to)) inner.set(to, []);
        inner.get(to)!.push(adapter);
      }
    }
  }
  return graph;
}

/**
 * Discover all viable chain-level paths from source to destination.
 * Returns arrays of chain sequences, e.g. [ethereum, arbitrum, solana].
 * Uses DFS with cycle detection and hop limits.
 */
export function discoverChainPaths(
  fromChain: Chain,
  toChain: Chain,
  registry: BridgeRegistry,
  options: PathDiscoveryOptions,
): Chain[][] {
  if (fromChain === toChain) return [];

  const graph = buildGraph(registry, options.excludeBridges);
  const results: Chain[][] = [];
  const visited = new Set<Chain>();

  function dfs(current: Chain, path: Chain[]): void {
    // path includes fromChain, so maxHops+1 is the length limit
    if (path.length > options.maxHops + 1) return;
    if (current === toChain) {
      results.push([...path]);
      return;
    }
    const neighbors = graph.get(current);
    if (!neighbors) return;

    // Sort neighbors by connectivity heuristic: prefer chains that are
    // well-connected to the destination
    const neighborChains = Array.from(neighbors.keys());
    const sorted = sortByConnectivity(neighborChains, toChain);

    for (const next of sorted) {
      if (visited.has(next)) continue;
      if (options.excludeChains.includes(next) && next !== toChain) continue;
      visited.add(next);
      path.push(next);
      dfs(next, path);
      path.pop();
      visited.delete(next);
    }
  }

  visited.add(fromChain);
  dfs(fromChain, [fromChain]);
  return results;
}

/**
 * Sort chains by how well-connected they are to the target chain.
 * Chains directly connected to the target come first.
 */
function sortByConnectivity(chains: Chain[], target: Chain): Chain[] {
  return [...chains].sort((a, b) => {
    // Direct connection to target is best
    const aDirectly = a === target ? -100 : 0;
    const bDirectly = b === target ? -100 : 0;
    if (aDirectly !== bDirectly) return aDirectly - bDirectly;

    // Prefer chains connected to target
    const aConnected = CHAIN_CONNECTIVITY[a]?.includes(target) ? -10 : 0;
    const bConnected = CHAIN_CONNECTIVITY[b]?.includes(target) ? -10 : 0;
    if (aConnected !== bConnected) return aConnected - bConnected;

    // Prefer chains with more overall connectivity
    const aConns = CHAIN_CONNECTIVITY[a]?.length ?? 0;
    const bConns = CHAIN_CONNECTIVITY[b]?.length ?? 0;
    return bConns - aConns;
  });
}

/**
 * Filter dominated paths: remove paths that are strictly worse than another
 * (more hops through same intermediate chains).
 */
export function filterDominatedPaths(paths: Chain[][]): Chain[][] {
  const dominated = new Set<number>();
  for (let i = 0; i < paths.length; i++) {
    if (dominated.has(i)) continue;
    for (let j = 0; j < paths.length; j++) {
      if (i === j || dominated.has(j)) continue;
      if (paths[j].length > paths[i].length && isSubpath(paths[i], paths[j])) {
        dominated.add(j);
      }
    }
  }
  return paths.filter((_, idx) => !dominated.has(idx));
}

/**
 * Check if shorter path is a subsequence of longer path (same start/end).
 */
function isSubpath(shorter: Chain[], longer: Chain[]): boolean {
  if (shorter[0] !== longer[0]) return false;
  if (shorter[shorter.length - 1] !== longer[longer.length - 1]) return false;
  let si = 0;
  for (let li = 0; li < longer.length && si < shorter.length; li++) {
    if (longer[li] === shorter[si]) si++;
  }
  return si === shorter.length;
}

/**
 * Get the list of intermediate chains commonly used between two chains.
 * Useful for suggesting multi-hop paths.
 */
export function getIntermediateChains(from: Chain, to: Chain): Chain[] {
  const fromConns = new Set(CHAIN_CONNECTIVITY[from] ?? []);
  const toConns = new Set(CHAIN_CONNECTIVITY[to] ?? []);
  const intermediates: Chain[] = [];

  for (const chain of ALL_CHAINS) {
    if (chain === from || chain === to) continue;
    // A good intermediate is connected to both source and destination
    if (fromConns.has(chain) && toConns.has(chain)) {
      intermediates.push(chain);
    }
  }

  return intermediates;
}

/**
 * Get all bridge adapters that can handle a specific chain pair.
 */
export function getBridgesForPair(
  from: Chain,
  to: Chain,
  registry: BridgeRegistry,
  excludeBridges: string[] = [],
): BridgeAdapter[] {
  return registry
    .getForPair(from, to)
    .filter((a) => !excludeBridges.includes(a.name));
}

/**
 * Resolve a token for a chain. First tries the chain registry,
 * then falls back to constructing a placeholder.
 */
function resolveToken(chain: Chain, symbol: string): Token {
  const found = findToken(chain, symbol);
  if (found) return found;
  // Fallback: construct a token with common defaults
  const isStable = ['USDC', 'USDT', 'DAI'].includes(symbol.toUpperCase());
  return {
    symbol: symbol.toUpperCase(),
    chain,
    decimals: isStable ? 6 : 18,
    address: `0x${chain}_${symbol.toLowerCase()}`,
  };
}

/**
 * Select the best intermediate token for a hop between two chains.
 * Prefers stablecoins, then the same token as the source.
 */
function selectIntermediateToken(chain: Chain, preferredSymbol: string): Token {
  // If the preferred token exists on this chain, use it
  const preferred = findToken(chain, preferredSymbol);
  if (preferred) return preferred;

  // Otherwise, use USDC as the universal intermediate
  const usdc = findToken(chain, 'USDC');
  if (usdc) return usdc;

  // Fallback
  return resolveToken(chain, 'USDC');
}

/**
 * Compute the cartesian product of arrays.
 */
function cartesianProduct<T>(arrays: T[][]): T[][] {
  if (arrays.length === 0) return [[]];
  const [first, ...rest] = arrays;
  const restProduct = cartesianProduct(rest);
  const result: T[][] = [];
  for (const item of first) {
    for (const combo of restProduct) {
      result.push([item, ...combo]);
    }
  }
  return result;
}

/**
 * Limit the number of bridge combinations to prevent combinatorial explosion.
 * For each hop, keep only the top N bridges sorted by estimated output.
 */
function limitCombinations(
  adaptersPerHop: BridgeAdapter[][],
  maxPerHop: number = 4,
): BridgeAdapter[][] {
  return adaptersPerHop.map((adapters) => {
    if (adapters.length <= maxPerHop) return adapters;
    // Keep only the first N (they should already be in a reasonable order)
    return adapters.slice(0, maxPerHop);
  });
}

/**
 * Assign bridges and build candidate paths with quotes.
 * Fetches quotes from each bridge for each hop and constructs
 * fully-quoted candidate paths.
 */
export async function buildCandidatePaths(
  chainPaths: Chain[][],
  fromToken: Token,
  toToken: Token,
  amount: string,
  registry: BridgeRegistry,
  options: PathDiscoveryOptions,
): Promise<CandidatePath[]> {
  const candidates: CandidatePath[] = [];

  for (const chainPath of chainPaths) {
    const adaptersPerHop: BridgeAdapter[][] = [];
    let viable = true;

    for (let i = 0; i < chainPath.length - 1; i++) {
      const from = chainPath[i];
      const to = chainPath[i + 1];
      const bridgeAdapters = getBridgesForPair(from, to, registry, options.excludeBridges);
      if (bridgeAdapters.length === 0) {
        viable = false;
