use crate::types::SearchStats;
use std::time::Instant;

/// Collects statistics during a minimax search.
#[derive(Debug)]
pub struct SearchStatsCollector {
    nodes_explored: u64,
    nodes_pruned: u64,
    max_depth_reached: u32,
    start_time: Instant,
    end_time: Option<Instant>,
    depth_histogram: Vec<u64>,
}

impl SearchStatsCollector {
    pub fn new() -> Self {
        Self {
            nodes_explored: 0,
            nodes_pruned: 0,
            max_depth_reached: 0,
            start_time: Instant::now(),
            end_time: None,
            depth_histogram: vec![0; 16],
        }
    }

    /// Record that a node was explored at the given depth.
    pub fn record_node(&mut self, depth: u32) {
        self.nodes_explored += 1;
        if depth > self.max_depth_reached {
            self.max_depth_reached = depth;
        }
        let idx = depth as usize;
        if idx < self.depth_histogram.len() {
            self.depth_histogram[idx] += 1;
        }
    }

    /// Record that a node was pruned.
    pub fn record_pruned(&mut self) {
        self.nodes_pruned += 1;
    }

    /// Record the max depth reached.
    pub fn record_depth(&mut self, depth: u32) {
        if depth > self.max_depth_reached {
            self.max_depth_reached = depth;
        }
    }

    /// Finalize the stats, recording end time.
    pub fn finalize(&mut self) {
        self.end_time = Some(Instant::now());
    }

    /// Get elapsed time in milliseconds.
    pub fn elapsed_ms(&self) -> u64 {
        let end = self.end_time.unwrap_or_else(Instant::now);
        end.duration_since(self.start_time).as_millis() as u64
    }

    /// Convert to SearchStats.
    pub fn to_search_stats(&self) -> SearchStats {
        SearchStats {
            nodes_explored: self.nodes_explored,
            nodes_pruned: self.nodes_pruned,
            max_depth_reached: self.max_depth_reached,
            search_time_ms: self.elapsed_ms(),
        }
    }

    /// Merge another collector into this one (e.g., from parallel searches).
    pub fn merge(&mut self, other: &SearchStatsCollector) {
        self.nodes_explored += other.nodes_explored;
        self.nodes_pruned += other.nodes_pruned;
        if other.max_depth_reached > self.max_depth_reached {
            self.max_depth_reached = other.max_depth_reached;
        }
        for (i, count) in other.depth_histogram.iter().enumerate() {
            if i < self.depth_histogram.len() {
                self.depth_histogram[i] += count;
            }
        }
    }

    /// Get the count of nodes explored at a given depth.
    pub fn nodes_at_depth(&self, depth: u32) -> u64 {
        let idx = depth as usize;
        if idx < self.depth_histogram.len() {
            self.depth_histogram[idx]
        } else {
            0
        }
    }

    /// Branching factor: average children per internal node.
    pub fn average_branching_factor(&self) -> f64 {
        if self.max_depth_reached == 0 || self.nodes_explored <= 1 {
            return 0.0;
        }
        // Estimate from total nodes and depth: b^d = N => b = N^(1/d)
        let n = self.nodes_explored as f64;
        let d = self.max_depth_reached as f64;
        n.powf(1.0 / d)
    }

    /// Pruning efficiency: fraction of nodes pruned out of total candidate nodes.
    pub fn pruning_efficiency(&self) -> f64 {
        let total = self.nodes_explored + self.nodes_pruned;
        if total == 0 {
            return 0.0;
        }
        self.nodes_pruned as f64 / total as f64
    }
}

impl Default for SearchStatsCollector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stats_collector() {
        let mut collector = SearchStatsCollector::new();
        collector.record_node(0);
        collector.record_node(1);
        collector.record_node(1);
        collector.record_node(2);
        collector.record_pruned();
        collector.record_pruned();
        collector.finalize();

        let stats = collector.to_search_stats();
        assert_eq!(stats.nodes_explored, 4);
        assert_eq!(stats.nodes_pruned, 2);
        assert_eq!(stats.max_depth_reached, 2);
    }

    #[test]
    fn test_merge() {
        let mut a = SearchStatsCollector::new();
        a.record_node(0);
        a.record_node(1);

        let mut b = SearchStatsCollector::new();
        b.record_node(0);
        b.record_node(2);
        b.record_pruned();

        a.merge(&b);
        assert_eq!(a.nodes_explored, 4);
        assert_eq!(a.nodes_pruned, 1);
        assert_eq!(a.max_depth_reached, 2);
    }
}
