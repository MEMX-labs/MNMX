use std::collections::HashMap;

use crate::types::*;

/// A hash table that caches previously evaluated positions to avoid
/// redundant work during the minimax search.
///
/// Uses a depth-preferred replacement policy: an existing entry is only
/// overwritten if the new entry was searched to an equal or greater depth,
/// or if the existing entry is sufficiently old.
#[derive(Debug, Clone)]
pub struct TranspositionTable {
    table: HashMap<String, TranspositionEntry>,
    max_entries: usize,
    hits: u64,
    misses: u64,
    overwrites: u64,
    current_age: u64,
}

impl TranspositionTable {
    pub fn new(max_entries: usize) -> Self {
        Self {
            table: HashMap::with_capacity(max_entries.min(1_000_000)),
            max_entries,
            hits: 0,
            misses: 0,
            overwrites: 0,
            current_age: 0,
        }
    }

    /// Look up a position hash and return a usable score if the stored
    /// entry is deep enough and the bounds match.
    ///
    /// Returns `Some(score)` if the entry can produce a cutoff or exact value,
    /// `None` if the entry is missing, too shallow, or the bounds don't allow
    /// a cutoff.
    pub fn lookup(
        &mut self,
        hash: &str,
        depth: u32,
        alpha: f64,
        beta: f64,
    ) -> Option<f64> {
        match self.table.get(hash) {
            Some(entry) => {
                if entry.depth < depth {
                    self.misses += 1;
                    return None;
                }

                self.hits += 1;

                match entry.flag {
                    TranspositionFlag::Exact => Some(entry.score),
                    TranspositionFlag::LowerBound => {
                        if entry.score >= beta {
                            Some(entry.score)
                        } else {
                            None
                        }
                    }
                    TranspositionFlag::UpperBound => {
                        if entry.score <= alpha {
                            Some(entry.score)
                        } else {
                            None
                        }
                    }
                }
            }
            None => {
                self.misses += 1;
                None
            }
        }
    }
