use std::collections::HashMap;

use sha2::{Digest, Sha256};

use crate::types::{Chain, RouteHop};

/// State for alpha-beta pruning during minimax search.
#[derive(Debug, Clone)]
pub struct PruningState {
    pub alpha: f64,
    pub beta: f64,
    pub killer_moves: Vec<Vec<MoveKey>>,
    pub history_table: HashMap<MoveKey, u64>,
    pub nodes_pruned: u64,
}

/// A compact representation of a move for the history/killer tables.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MoveKey {
    pub from_chain: u64,
    pub to_chain: u64,
    pub bridge_hash: u64,
}

impl MoveKey {
    pub fn from_hop(hop: &RouteHop) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(hop.bridge.as_bytes());
        let hash_bytes = hasher.finalize();
        let bridge_hash = u64::from_le_bytes(hash_bytes[0..8].try_into().unwrap_or([0; 8]));

        Self {
            from_chain: hop.from_chain.chain_id(),
            to_chain: hop.to_chain.chain_id(),
            bridge_hash,
        }
    }

    pub fn from_chains_and_bridge(from: Chain, to: Chain, bridge: &str) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(bridge.as_bytes());
        let hash_bytes = hasher.finalize();
        let bridge_hash = u64::from_le_bytes(hash_bytes[0..8].try_into().unwrap_or([0; 8]));

        Self {
            from_chain: from.chain_id(),
            to_chain: to.chain_id(),
            bridge_hash,
        }
    }
}

impl PruningState {
    pub fn new(max_depth: usize) -> Self {
        let mut killer_moves = Vec::with_capacity(max_depth);
        for _ in 0..max_depth {
            killer_moves.push(Vec::with_capacity(2));
        }
        Self {
            alpha: f64::NEG_INFINITY,
            beta: f64::INFINITY,
            killer_moves,
            history_table: HashMap::new(),
            nodes_pruned: 0,
        }
    }

    /// Check if the current branch can be pruned (beta cutoff).
    pub fn should_prune(&self, score: f64, is_maximizing: bool) -> bool {
        if is_maximizing {
            score >= self.beta
        } else {
            score <= self.alpha
        }
    }

    /// Update alpha or beta after evaluating a node.
    pub fn update_bounds(&mut self, score: f64, is_maximizing: bool) {
        if is_maximizing {
            if score > self.alpha {
                self.alpha = score;
            }
        } else {
            if score < self.beta {
                self.beta = score;
            }
        }
    }

    /// Record a move that caused a cutoff at a given depth (killer move heuristic).
    pub fn record_killer_move(&mut self, depth: usize, move_key: MoveKey) {
        if depth < self.killer_moves.len() {
            let killers = &mut self.killer_moves[depth];
            // Keep at most 2 killer moves per depth
            if !killers.contains(&move_key) {
                if killers.len() >= 2 {
                    killers.remove(0);
                }
                killers.push(move_key);
            }
        }
    }

    /// Record a move in the history table for move ordering.
    pub fn record_history(&mut self, move_key: MoveKey, depth: u32) {
        let bonus = 1u64 << depth.min(16);
        *self.history_table.entry(move_key).or_insert(0) += bonus;
    }

    /// Order moves for better pruning: killer moves first, then by history score.
    pub fn get_move_ordering(&self, moves: &[RouteHop], depth: usize) -> Vec<usize> {
        let mut indices: Vec<usize> = (0..moves.len()).collect();

        let killer_set: Vec<MoveKey> = if depth < self.killer_moves.len() {
            self.killer_moves[depth].clone()
        } else {
            Vec::new()
        };

        indices.sort_by(|&a, &b| {
            let key_a = MoveKey::from_hop(&moves[a]);
            let key_b = MoveKey::from_hop(&moves[b]);

            let a_is_killer = killer_set.contains(&key_a);
            let b_is_killer = killer_set.contains(&key_b);

            // Killer moves first
            if a_is_killer && !b_is_killer {
                return std::cmp::Ordering::Less;
            }
            if !a_is_killer && b_is_killer {
                return std::cmp::Ordering::Greater;
            }

            // Then by history score (higher = first)
            let hist_a = self.history_table.get(&key_a).copied().unwrap_or(0);
            let hist_b = self.history_table.get(&key_b).copied().unwrap_or(0);
            hist_b.cmp(&hist_a)
        });

        indices
    }

    /// Create a child pruning state with inherited alpha/beta.
    pub fn child(&self, max_depth: usize) -> Self {
        Self {
            alpha: self.alpha,
            beta: self.beta,
            killer_moves: {
                let mut km = Vec::with_capacity(max_depth);
                for _ in 0..max_depth {
                    km.push(Vec::new());
                }
                km
            },
            history_table: self.history_table.clone(),
            nodes_pruned: 0,
        }
    }
}

/// An entry in the transposition table for caching evaluated positions.
#[derive(Debug, Clone)]
pub struct TranspositionEntry {
    pub hash: u64,
    pub depth: u32,
    pub score: f64,
    pub flag: TranspositionFlag,
    pub best_move: Option<MoveKey>,
}

/// Flag indicating the type of score stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TranspositionFlag {
    Exact,
    LowerBound,
    UpperBound,
}

/// Transposition table for caching previously evaluated positions.
pub struct TranspositionTable {
    entries: HashMap<u64, TranspositionEntry>,
    max_entries: usize,
    hits: u64,
    misses: u64,
}

impl TranspositionTable {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: HashMap::with_capacity(max_entries),
            max_entries,
            hits: 0,
            misses: 0,
        }
    }

    /// Insert or replace an entry. Replacement policy: always replace if new depth >= old depth.
    pub fn insert(&mut self, entry: TranspositionEntry) {
        if self.entries.len() >= self.max_entries {
            // Evict entries searched at shallower depths
            if let Some(existing) = self.entries.get(&entry.hash) {
                if existing.depth > entry.depth {
                    return; // Don't replace deeper search with shallower
                }
            }
            // If at capacity and this is a new entry, evict a random old one
            if !self.entries.contains_key(&entry.hash) && self.entries.len() >= self.max_entries {
                // Remove the first entry we find (approximation of LRU)
                if let Some(&first_key) = self.entries.keys().next() {
                    self.entries.remove(&first_key);
                }
            }
        }
        self.entries.insert(entry.hash, entry);
    }

    /// Look up a position in the table.
    pub fn lookup(&mut self, hash: u64, depth: u32, alpha: f64, beta: f64) -> Option<f64> {
