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

    /// Convert accumulated stats into the public SearchStats type.
    pub fn to_search_stats(&self) -> SearchStats {
        let total_time: u64 = self
            .depths_completed
            .last()
            .map(|(_, t)| *t)
            .unwrap_or(0);

        SearchStats {
            nodes_explored: self.nodes_visited,
            nodes_pruned: self.nodes_pruned,
            max_depth_reached: self.max_depth_reached,
            time_ms: total_time,
            tt_hits: self.tt_hits,
            tt_misses: self.tt_misses,
            branching_factor: self.effective_branching_factor(),
        }
    }

    /// Fraction of explored nodes that were pruned.
    pub fn pruning_rate(&self) -> f64 {
        let total = self.nodes_visited + self.nodes_pruned;
        if total == 0 {
            0.0
        } else {
            self.nodes_pruned as f64 / total as f64
        }
    }

    /// Throughput: nodes explored per millisecond of search time.
    pub fn nodes_per_second(&self, elapsed_ms: u64) -> f64 {
        if elapsed_ms == 0 {
            return self.nodes_visited as f64;
        }
        (self.nodes_visited as f64 / elapsed_ms as f64) * 1000.0
    }

    /// Effective branching factor: average number of children per interior node.
    pub fn effective_branching_factor(&self) -> f64 {
        if self.total_interior_nodes == 0 {
            return 0.0;
        }
        self.total_children_generated as f64 / self.total_interior_nodes as f64
    }

    /// Number of times the best move changed (instability indicator).
    pub fn best_move_changes(&self) -> u32 {
        self.best_move_changes
    }

    /// Total nodes visited.
    pub fn total_nodes(&self) -> u64 {
        self.nodes_visited
    }

    /// Total nodes pruned.
    pub fn total_pruned(&self) -> u64 {
        self.nodes_pruned
    }

    /// Time profile: how long each depth took.
    pub fn depth_times(&self) -> &[(u32, u64)] {
        &self.depths_completed
    }

    /// TT hit rate.
    pub fn tt_hit_rate(&self) -> f64 {
        let total = self.tt_hits + self.tt_misses;
        if total == 0 {
            0.0
        } else {
            self.tt_hits as f64 / total as f64
        }
    }
}

impl Default for SearchStatistics {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_stats_are_zero() {
        let stats = SearchStatistics::new();
        assert_eq!(stats.total_nodes(), 0);
        assert_eq!(stats.total_pruned(), 0);
        assert_eq!(stats.pruning_rate(), 0.0);
        assert_eq!(stats.effective_branching_factor(), 0.0);
    }

    #[test]
    fn test_node_tracking() {
        let mut stats = SearchStatistics::new();
        stats.record_node_visit();
        stats.record_node_visit();
        stats.record_prune();
        assert_eq!(stats.total_nodes(), 2);
        assert_eq!(stats.total_pruned(), 1);
    }

    #[test]
    fn test_pruning_rate() {
        let mut stats = SearchStatistics::new();
        for _ in 0..7 {
            stats.record_node_visit();
        }
        for _ in 0..3 {
            stats.record_prune();
        }
        // pruning_rate = 3 / (7+3) = 0.3
        assert!((stats.pruning_rate() - 0.3).abs() < 0.001);
    }

    #[test]
    fn test_nps() {
        let mut stats = SearchStatistics::new();
        for _ in 0..10_000 {
            stats.record_node_visit();
        }
        let nps = stats.nodes_per_second(100);
        assert!((nps - 100_000.0).abs() < 1.0);
    }

    #[test]
    fn test_branching_factor() {
        let mut stats = SearchStatistics::new();
        stats.record_children(5);
        stats.record_children(3);
        stats.record_children(4);
        // Average: (5+3+4)/3 = 4.0
        assert!((stats.effective_branching_factor() - 4.0).abs() < 0.001);
    }

    #[test]
    fn test_depth_tracking() {
        let mut stats = SearchStatistics::new();
        stats.record_depth_completed(1, 10);
        stats.record_depth_completed(2, 50);
        stats.record_depth_completed(3, 200);
        let ss = stats.to_search_stats();
        assert_eq!(ss.max_depth_reached, 3);
        assert_eq!(ss.time_ms, 200);
    }

    #[test]
    fn test_tt_hit_rate() {
        let mut stats = SearchStatistics::new();
        stats.record_tt_hit();
        stats.record_tt_hit();
        stats.record_tt_miss();
        assert!((stats.tt_hit_rate() - 2.0 / 3.0).abs() < 0.001);
    }

    #[test]
    fn test_to_search_stats() {
        let mut stats = SearchStatistics::new();
        stats.record_node_visit();
        stats.record_prune();
        stats.record_tt_hit();
        stats.record_tt_miss();
        stats.record_depth_completed(2, 100);
        stats.record_children(4);

        let ss = stats.to_search_stats();
        assert_eq!(ss.nodes_explored, 1);
        assert_eq!(ss.nodes_pruned, 1);
        assert_eq!(ss.tt_hits, 1);
        assert_eq!(ss.tt_misses, 1);
        assert_eq!(ss.max_depth_reached, 2);
        assert_eq!(ss.time_ms, 100);
        assert!((ss.branching_factor - 4.0).abs() < 0.001);
    }
}
