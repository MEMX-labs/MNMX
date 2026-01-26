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
