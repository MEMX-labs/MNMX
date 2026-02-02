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
