/**
 * MNMX Move Ordering
 *
 * Dramatically improves alpha-beta pruning efficiency by evaluating the
 * most promising actions first.  Combines three heuristics drawn from
 * game-engine research and adapted to on-chain DeFi context:
 *
 *  1. Killer moves   – refutation moves that caused a beta cutoff at the
 *                      same depth in a sibling branch.
 *  2. History table  – actions that have historically produced high scores
 *                      are tried earlier regardless of depth.
 *  3. MVV-LVA        – Most Valuable Victim / Least Valuable Aggressor,
 *                      re-interpreted for DeFi as "highest expected value
 *                      per unit of cost (gas + slippage)".
 */

import type { ExecutionAction, OnChainState } from '../types/index.js';

// ── Killer-Move Storage ─────────────────────────────────────────────

const MAX_KILLER_SLOTS = 2; // two killer slots per depth level

function actionKey(a: ExecutionAction): string {
  return `${a.kind}:${a.pool}:${a.tokenMintIn}:${a.tokenMintOut}:${a.amount}`;
}

// ── Class ───────────────────────────────────────────────────────────

export class MoveOrderer {
  /** killerMoves[depth] holds up to MAX_KILLER_SLOTS action keys. */
  private readonly killerMoves: Map<number, string[]> = new Map();

  /** Cumulative score for every action key that ever caused an improvement. */
  private readonly historyScores: Map<string, number> = new Map();

  // ── Public API ──────────────────────────────────────────────────

  /**
   * Return `actions` sorted so that the most promising candidates
   * appear first.  Does not mutate the input array.
   */
  orderMoves(
    actions: ReadonlyArray<ExecutionAction>,
    state: OnChainState,
    depth: number = 0,
  ): ExecutionAction[] {
    // Score each action with a composite ordering value
    const scored = actions.map((action) => ({
      action,
      orderScore: this.computeOrderScore(action, state, depth),
    }));

    // Sort descending – highest score first
    scored.sort((a, b) => b.orderScore - a.orderScore);

    return scored.map((s) => s.action);
  }

  /**
   * Record a killer move at the given depth.  Called when an action
   * causes a beta cutoff during search.
   */
  updateKillerMove(depth: number, action: ExecutionAction): void {
    const key = actionKey(action);
    let slots = this.killerMoves.get(depth);
    if (!slots) {
      slots = [];
      this.killerMoves.set(depth, slots);
    }

    // Avoid duplicates
    if (slots.includes(key)) return;

    // Shift older killer out if at capacity
    if (slots.length >= MAX_KILLER_SLOTS) {
      slots.shift();
    }
    slots.push(key);
  }

  /**
   * Bump the history score for an action.  Weighted by depth squared
   * so that refutations found deeper in the tree carry more weight.
   */
  updateHistory(action: ExecutionAction, depth: number): void {
    const key = actionKey(action);
    const prev = this.historyScores.get(key) ?? 0;
    this.historyScores.set(key, prev + depth * depth);
  }

  /** Reset all heuristic data (call between unrelated searches). */
  reset(): void {
    this.killerMoves.clear();
    this.historyScores.clear();
  }

  // ── Private ─────────────────────────────────────────────────────

  private computeOrderScore(
    action: ExecutionAction,
    state: OnChainState,
    depth: number,
  ): number {
    let score = 0;

    // 1. Killer-move bonus (highest priority)
    const key = actionKey(action);
    const killers = this.killerMoves.get(depth);
    if (killers?.includes(key)) {
      score += 50_000;
    }
