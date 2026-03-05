/**
 * MNMX Search Statistics
 *
 * Collects fine-grained telemetry during minimax search to support
 * performance analysis, tuning, and debugging. Tracks node visits,
 * pruning rates, transposition table effectiveness, branching factors,
 * and per-depth timing.
 */

// ── Event Types ──────────────────────────────────────────────────────

export type SearchEventKind =
  | 'node_visited'
  | 'node_pruned'
  | 'tt_hit'
  | 'tt_miss'
  | 'depth_completed'
  | 'best_move_changed';

export interface SearchEvent {
  readonly kind: SearchEventKind;
  readonly depth: number;
  readonly timestamp: number;
  /** Optional metadata attached to the event. */
  readonly meta?: Record<string, number | string>;
}

// ── Report Types ─────────────────────────────────────────────────────

export interface DepthReport {
  readonly depth: number;
  readonly nodesVisited: number;
  readonly nodesPruned: number;
  readonly ttHits: number;
  readonly ttMisses: number;
  readonly bestMoveChanges: number;
  readonly startTimestamp: number;
  readonly endTimestamp: number;
  readonly durationMs: number;
}

export interface SearchReport {
  readonly totalNodes: number;
  readonly totalPruned: number;
  readonly pruningRate: number;
  readonly totalTtHits: number;
  readonly totalTtMisses: number;
  readonly ttHitRate: number;
  readonly effectiveBranchingFactor: number;
  readonly maxDepthReached: number;
  readonly depthReports: DepthReport[];
  readonly totalTimeMs: number;
  readonly nodesPerSecond: number;
  readonly bestMoveChanges: number;
}

// ── Search Statistics ────────────────────────────────────────────────

export class SearchStatistics {
  private readonly events: SearchEvent[] = [];

  // Per-depth accumulators for efficient report generation
  private readonly depthNodes: Map<number, number> = new Map();
  private readonly depthPruned: Map<number, number> = new Map();
  private readonly depthTtHits: Map<number, number> = new Map();
  private readonly depthTtMisses: Map<number, number> = new Map();
  private readonly depthBestMoveChanges: Map<number, number> = new Map();
  private readonly depthFirstTimestamp: Map<number, number> = new Map();
  private readonly depthLastTimestamp: Map<number, number> = new Map();

  private totalNodes = 0;
  private totalPruned = 0;
  private totalTtHits = 0;
  private totalTtMisses = 0;
  private totalBestMoveChanges = 0;
  private maxDepthSeen = 0;
  private firstEventTimestamp = 0;
  private lastEventTimestamp = 0;

  /**
   * Record a search event. Incrementally updates internal counters
   * so that report generation is O(depth) rather than O(events).
   */
  track(event: SearchEvent): void {
    this.events.push(event);

    if (this.firstEventTimestamp === 0) {
      this.firstEventTimestamp = event.timestamp;
    }
    this.lastEventTimestamp = event.timestamp;

    if (event.depth > this.maxDepthSeen) {
      this.maxDepthSeen = event.depth;
    }

    // Update depth-level first/last timestamps
    if (!this.depthFirstTimestamp.has(event.depth)) {
      this.depthFirstTimestamp.set(event.depth, event.timestamp);
    }
    this.depthLastTimestamp.set(event.depth, event.timestamp);

    switch (event.kind) {
      case 'node_visited':
        this.totalNodes++;
        this.increment(this.depthNodes, event.depth);
        break;

      case 'node_pruned':
        this.totalPruned++;
        this.increment(this.depthPruned, event.depth);
        break;

      case 'tt_hit':
        this.totalTtHits++;
        this.increment(this.depthTtHits, event.depth);
        break;

      case 'tt_miss':
        this.totalTtMisses++;
        this.increment(this.depthTtMisses, event.depth);
        break;

      case 'best_move_changed':
        this.totalBestMoveChanges++;
        this.increment(this.depthBestMoveChanges, event.depth);
        break;

      case 'depth_completed':
        // No special accumulator; the depth timestamp tracking is sufficient.
        break;
    }
  }

  /**
   * Generate a comprehensive search report from all tracked events.
   */
  getReport(): SearchReport {
    const totalTimeMs = this.lastEventTimestamp - this.firstEventTimestamp;
    const nodesPerSecond =
      totalTimeMs > 0 ? (this.totalNodes / totalTimeMs) * 1000 : 0;
