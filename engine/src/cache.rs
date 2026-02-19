use std::collections::HashMap;

/// Simple LRU-style cache for route scoring results.
pub struct ScoreCache {
    entries: HashMap<u64, f64>,
    max_entries: usize,
}

impl ScoreCache {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: HashMap::with_capacity(max_entries),
            max_entries,
        }
    }

    pub fn get(&self, key: u64) -> Option<f64> {
        self.entries.get(&key).copied()
    }

    pub fn insert(&mut self, key: u64, score: f64) {
        if self.entries.len() >= self.max_entries {
            // Simple eviction: clear half the cache
            let keys: Vec<u64> = self.entries.keys().take(self.max_entries / 2).copied().collect();
            for k in keys {
                self.entries.remove(&k);
            }
        }
        self.entries.insert(key, score);
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}
