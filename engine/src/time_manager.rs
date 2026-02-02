use crate::types::*;

/// Phase of the "game" — determines how time budget is distributed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GamePhase {
    /// Early in the block: many possible actions, exploration is key.
    Opening,
    /// Mid-block: some information, focus on promising branches.
    Midgame,
    /// Late in the block or near slot boundary: must decide quickly.
    Endgame,
}

/// Reason to extend the time budget for the current search.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtendReason {
    /// The best move changed at the last completed depth — the position
    /// is unstable and cutting the search short may pick a bad move.
    Instability,
    /// There is only one legal reply, so the extra time can deepen search.
    SingleReply,
    /// The score dropped significantly from the previous iteration,
    /// indicating danger.
    ScoreDrop,
}

/// Manages the time budget for iterative-deepening search.
///
/// Allocates time across depths, supports extension, and provides
/// `should_stop` / `emergency_stop` queries that the search loop checks
/// at each node.
#[derive(Debug, Clone)]
pub struct TimeManager {
    base_time_ms: u64,
    max_time_ms: u64,
    extension_factor: f64,
    allocation: TimeAllocation,
    extensions_granted: u32,
    max_extensions: u32,
}

impl TimeManager {
    pub fn new(config: &SearchConfig) -> Self {
        let base = config.time_limit_ms;
        let allocation = TimeAllocation::new(base, config.max_depth);

        Self {
            base_time_ms: base,
            max_time_ms: base * 2, // Hard ceiling: 2x the base budget
            extension_factor: 1.0,
            allocation,
            extensions_granted: 0,
            max_extensions: 3,
        }
    }

    /// Produce a time allocation tailored to the current game phase.
    ///
    /// - **Opening**: allocate more time to shallow depths (broad survey).
    /// - **Midgame**: balanced allocation.
    /// - **Endgame**: allocate most time to the first few depths (fast decision).
    pub fn allocate(&self, game_phase: GamePhase) -> TimeAllocation {
        let total = (self.base_time_ms as f64 * self.extension_factor) as u64;
        let depths = self.allocation.per_depth.len() as u32;

        match game_phase {
            GamePhase::Opening => {
                // Flatten allocation: spread time more evenly
                let per_depth = self.flat_allocation(total, depths, 0.6);
                TimeAllocation {
                    total_ms: total,
                    per_depth,
                    emergency_stop_ms: total.saturating_sub(total / 10),
                }
            }
            GamePhase::Midgame => {
                // Exponential (default) allocation
                let mut alloc = TimeAllocation::new(total, depths);
                alloc.emergency_stop_ms = total.saturating_sub(total / 10);
                alloc
            }
            GamePhase::Endgame => {
                // Front-loaded: most time in depths 0-2
                let per_depth = self.front_loaded_allocation(total, depths);
                TimeAllocation {
                    total_ms: total,
                    per_depth,
                    emergency_stop_ms: total.saturating_sub(total / 20),
                }
            }
        }
    }

    /// Check whether the search should stop at the current depth.
    ///
    /// Returns `true` if:
    /// - Elapsed time exceeds the allocation for the given depth, OR
    /// - Elapsed time exceeds the total budget (with extensions).
    pub fn should_stop(&self, elapsed_ms: u64, depth: u32) -> bool {
        let effective_total =
            (self.base_time_ms as f64 * self.extension_factor) as u64;

        if elapsed_ms >= effective_total {
            return true;
        }

        // Check per-depth allocation: sum of allocations up to this depth
        let cumulative: u64 = self
            .allocation
            .per_depth
            .iter()
            .take(depth as usize + 1)
            .sum();
        let cumulative_extended = (cumulative as f64 * self.extension_factor) as u64;

        elapsed_ms >= cumulative_extended
    }

    /// Extend the time budget.
    ///
    /// Each extension multiplies the remaining budget by a factor that
    /// depends on the reason:
    /// - Instability: +50% remaining
    /// - SingleReply: +25% remaining
    /// - ScoreDrop: +75% remaining
    pub fn extend(&mut self, reason: ExtendReason) {
        if self.extensions_granted >= self.max_extensions {
            return;
        }

        let bonus = match reason {
            ExtendReason::Instability => 0.50,
            ExtendReason::SingleReply => 0.25,
            ExtendReason::ScoreDrop => 0.75,
        };

        self.extension_factor += bonus;

        // Cap at max_time_ms / base_time_ms
        let max_factor = if self.base_time_ms > 0 {
            self.max_time_ms as f64 / self.base_time_ms as f64
        } else {
            2.0
        };
        if self.extension_factor > max_factor {
            self.extension_factor = max_factor;
        }

        self.extensions_granted += 1;
    }

    /// Emergency stop: always abort regardless of extensions.
    ///
    /// Returns `true` if elapsed time exceeds the hard maximum.
    pub fn emergency_stop(&self, elapsed_ms: u64) -> bool {
        elapsed_ms >= self.max_time_ms
    }

    /// Produce a flattened allocation where each depth gets a more
    /// even share. `flatness` in [0, 1]: 1.0 = perfectly even.
    fn flat_allocation(&self, total_ms: u64, depths: u32, flatness: f64) -> Vec<u64> {
        if depths == 0 {
            return Vec::new();
        }

        let even_share = total_ms / depths as u64;
        let expo = TimeAllocation::new(total_ms, depths);

        expo.per_depth
            .iter()
            .map(|&exp_alloc| {
                let blended = even_share as f64 * flatness + exp_alloc as f64 * (1.0 - flatness);
                blended as u64
            })
            .collect()
    }

    /// Front-loaded allocation: ~60% in first third of depths.
    fn front_loaded_allocation(&self, total_ms: u64, depths: u32) -> Vec<u64> {
        if depths == 0 {
            return Vec::new();
        }

        let front_count = (depths / 3).max(1);
        let front_budget = total_ms * 60 / 100;
        let back_budget = total_ms - front_budget;
        let back_count = depths - front_count;

        let mut per_depth = Vec::with_capacity(depths as usize);

        let front_each = if front_count > 0 {
            front_budget / front_count as u64
        } else {
            0
        };
        let back_each = if back_count > 0 {
            back_budget / back_count as u64
        } else {
            0
        };

        for i in 0..depths {
            if i < front_count {
                per_depth.push(front_each);
            } else {
                per_depth.push(back_each);
            }
        }

        per_depth
    }

    /// Get the current extension factor (1.0 = no extensions).
    pub fn extension_factor(&self) -> f64 {
        self.extension_factor
    }
