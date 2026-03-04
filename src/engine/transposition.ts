/**
 * MNMX Transposition Table
 *
 * Caches previously evaluated game-tree positions so that when the
 * same on-chain state is reached via a different move order we can
 * skip re-evaluation.  Uses depth-preferred replacement with an aging
 * mechanism to evict stale entries when the table is at capacity.
 */

import type { ExecutionAction } from '../types/index.js';

// ── Entry Types ─────────────────────────────────────────────────────

export type BoundFlag = 'exact' | 'lower' | 'upper';

export interface TranspositionEntry {
  hash: string;
  depth: number;
  score: number;
  flag: BoundFlag;
  bestAction?: ExecutionAction;
  age: number;
}

export interface TableStats {
  entries: number;
  hits: number;
  misses: number;
  hitRate: number;
  overwrites: number;
}

export interface LookupResult {
  score: number;
  found: boolean;
  bestAction?: ExecutionAction;
}

// ── Transposition Table ─────────────────────────────────────────────

export class TranspositionTable {
  private readonly entries = new Map<string, TranspositionEntry>();
  private readonly maxEntries: number;
  private currentAge = 0;
  private hits = 0;
  private misses = 0;
  private overwrites = 0;

  constructor(maxEntries: number = 100_000) {
    this.maxEntries = maxEntries;
  }

  // ── Public API ──────────────────────────────────────────────────

  /**
   * Probe the table for a usable cached evaluation.
   *
   * Returns `found: true` only if the stored entry was searched to at
   * least the requested depth AND the bound flag permits narrowing the
   * alpha-beta window:
   *
   *  - exact  =>  return the score directly
   *  - lower  =>  usable when stored score >= beta  (fail-high cutoff)
   *  - upper  =>  usable when stored score <= alpha (fail-low cutoff)
   */
  lookup(
    hash: string,
    depth: number,
    alpha: number,
    beta: number,
  ): LookupResult {
    const entry = this.entries.get(hash);
    if (!entry || entry.depth < depth) {
      this.misses++;
      return { score: 0, found: false };
    }

    let usable = false;
    let score = entry.score;

    switch (entry.flag) {
      case 'exact':
        usable = true;
        break;
      case 'lower':
        if (entry.score >= beta) {
          usable = true;
          score = entry.score;
        }
        break;
      case 'upper':
        if (entry.score <= alpha) {
          usable = true;
          score = entry.score;
        }
        break;
    }

    if (usable) {
      this.hits++;
      return { score, found: true, bestAction: entry.bestAction };
    }

    this.misses++;
    return { score: 0, found: false, bestAction: entry.bestAction };
  }

  /**
   * Store or overwrite an entry.
   *
   * Replacement policy:
   *  1. Always replace if slot is empty.
   *  2. Replace if new entry has greater or equal depth.
   *  3. Replace if existing entry belongs to an older search generation.
   *  4. Otherwise keep the deeper entry.
   */
  store(
    hash: string,
    depth: number,
    score: number,
    flag: BoundFlag,
    bestAction?: ExecutionAction,
  ): void {
    const existing = this.entries.get(hash);

    if (existing) {
      const shouldReplace =
        depth >= existing.depth || existing.age < this.currentAge;
      if (!shouldReplace) return;
      this.overwrites++;
    }

    // Evict oldest entries if at capacity
    if (!existing && this.entries.size >= this.maxEntries) {
      this.evictStaleEntries();
    }

    this.entries.set(hash, {
      hash,
      depth,
      score,
      flag,
      bestAction,
      age: this.currentAge,
    });
  }

  /** Advance the generation counter.  Call at the start of each new search. */
  incrementAge(): void {
    this.currentAge++;
  }

  /** Wipe the entire table and reset statistics. */
  clear(): void {
    this.entries.clear();
    this.hits = 0;
    this.misses = 0;
    this.overwrites = 0;
    this.currentAge = 0;
  }

  /** Return current performance statistics. */
  getStats(): TableStats {
    const total = this.hits + this.misses;
    return {
      entries: this.entries.size,
      hits: this.hits,
      misses: this.misses,
      hitRate: total === 0 ? 0 : this.hits / total,
      overwrites: this.overwrites,
    };
  }

  /** Retrieve an entry without affecting hit/miss counters (for testing). */
  peek(hash: string): TranspositionEntry | undefined {
    return this.entries.get(hash);
  }

  get size(): number {
    return this.entries.size;
  }

  // ── Private ─────────────────────────────────────────────────────

  /**
   * Evict entries from older generations until we are below 90% capacity.
   * Falls back to evicting the shallowest entries if all are current-gen.
   */
  private evictStaleEntries(): void {
    const target = Math.floor(this.maxEntries * 0.9);

    // First pass: remove entries from previous generations
    if (this.entries.size > target) {
      for (const [key, entry] of this.entries) {
        if (entry.age < this.currentAge) {
          this.entries.delete(key);
          if (this.entries.size <= target) return;
        }
      }
    }

    // Second pass: remove shallowest entries
    if (this.entries.size > target) {
      const sorted = [...this.entries.entries()].sort(
        (a, b) => a[1].depth - b[1].depth,
      );
      for (const [key] of sorted) {
        this.entries.delete(key);
        if (this.entries.size <= target) return;
      }
    }
  }
}
