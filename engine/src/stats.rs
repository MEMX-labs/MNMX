use crate::types::SearchStats;

/// Accumulates statistics during a minimax search for diagnostic purposes.
///
/// The engine calls `record_*` methods as it traverses the game tree, and
/// at the end of the search converts to a `SearchStats` for inclusion in
/// the `ExecutionPlan`.
#[derive(Debug, Clone)]
pub struct SearchStatistics {
    nodes_visited: u64,
    nodes_pruned: u64,
    tt_hits: u64,
    tt_misses: u64,
    max_depth_reached: u32,
    depths_completed: Vec<(u32, u64)>, // (depth, time_ms)
    best_move_changes: u32,
    total_children_generated: u64,
    total_interior_nodes: u64,
}

impl SearchStatistics {
    pub fn new() -> Self {
        Self {
            nodes_visited: 0,
            nodes_pruned: 0,
            tt_hits: 0,
            tt_misses: 0,
            max_depth_reached: 0,
            depths_completed: Vec::new(),
            best_move_changes: 0,
            total_children_generated: 0,
            total_interior_nodes: 0,
        }
    }

    /// Record that the search visited a node.
    pub fn record_node_visit(&mut self) {
        self.nodes_visited += 1;
    }

    /// Record that a subtree was pruned (alpha-beta cutoff).
    pub fn record_prune(&mut self) {
        self.nodes_pruned += 1;
    }

    /// Record a transposition table hit.
    pub fn record_tt_hit(&mut self) {
        self.tt_hits += 1;
    }

    /// Record a transposition table miss.
    pub fn record_tt_miss(&mut self) {
        self.tt_misses += 1;
    }

    /// Record that an iterative-deepening iteration completed.
    pub fn record_depth_completed(&mut self, depth: u32, time_ms: u64) {
        if depth > self.max_depth_reached {
            self.max_depth_reached = depth;
        }
        self.depths_completed.push((depth, time_ms));
    }

    /// Record that the best root move changed during iterative deepening.
    pub fn record_best_move_change(&mut self) {
        self.best_move_changes += 1;
    }

    /// Record children generated at an interior node (for branching factor).
    pub fn record_children(&mut self, count: u64) {
        self.total_children_generated += count;
        self.total_interior_nodes += 1;
    }
