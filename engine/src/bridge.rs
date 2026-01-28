use crate::types::{BridgeHealth, BridgeQuote, Chain, CongestionLevel, Token};
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Identifier for a bridge protocol.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BridgeId {
    Wormhole,
    DeBridge,
    LayerZero,
    Allbridge,
    Custom(String),
}

impl BridgeId {
    pub fn name(&self) -> &str {
        match self {
            BridgeId::Wormhole => "Wormhole",
            BridgeId::DeBridge => "deBridge",
            BridgeId::LayerZero => "LayerZero",
            BridgeId::Allbridge => "Allbridge",
            BridgeId::Custom(name) => name.as_str(),
        }
    }
}

impl std::fmt::Display for BridgeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

/// Trait that all bridge adapters must implement.
pub trait BridgeAdapter: Send + Sync {
    /// Return the name of this bridge.
    fn name(&self) -> &str;

    /// Return the list of chain pairs this bridge supports.
    fn supported_chains(&self) -> Vec<(Chain, Chain)>;

    /// Get a quote for transferring amount of from_token to to_token across chains.
    fn get_quote(
        &self,
        from_token: &Token,
        to_token: &Token,
        amount: f64,
    ) -> Option<BridgeQuote>;

    /// Get the current health status of the bridge.
    fn get_health(&self) -> BridgeHealth;
}

/// Registry that holds all available bridge adapters.
pub struct BridgeRegistry {
    bridges: Vec<Box<dyn BridgeAdapter>>,
    pair_index: HashMap<(Chain, Chain), Vec<usize>>,
}

impl BridgeRegistry {
    pub fn new() -> Self {
        Self {
            bridges: Vec::new(),
            pair_index: HashMap::new(),
        }
    }

    /// Register a bridge adapter and index its supported chain pairs.
    pub fn register(&mut self, adapter: Box<dyn BridgeAdapter>) {
        let idx = self.bridges.len();
        let pairs = adapter.supported_chains();
        self.bridges.push(adapter);
        for pair in pairs {
            self.pair_index.entry(pair).or_insert_with(Vec::new).push(idx);
        }
    }

    /// Get all bridge adapters that support the given chain pair.
    pub fn get_bridges_for_pair(&self, from: Chain, to: Chain) -> Vec<&dyn BridgeAdapter> {
        match self.pair_index.get(&(from, to)) {
            Some(indices) => indices.iter().map(|&i| self.bridges[i].as_ref()).collect(),
            None => Vec::new(),
        }
    }

    /// Return references to all registered bridges.
    pub fn get_all_bridges(&self) -> Vec<&dyn BridgeAdapter> {
        self.bridges.iter().map(|b| b.as_ref()).collect()
    }

    /// Check if any bridge supports the given pair.
    pub fn has_pair(&self, from: Chain, to: Chain) -> bool {
        self.pair_index.contains_key(&(from, to))
    }

    /// Return all supported chain pairs.
    pub fn supported_pairs(&self) -> Vec<(Chain, Chain)> {
        self.pair_index.keys().cloned().collect()
    }

    /// Return the number of registered bridges.
    pub fn bridge_count(&self) -> usize {
        self.bridges.len()
    }
}

impl Default for BridgeRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// A mock bridge for testing that generates deterministic quotes.
pub struct MockBridge {
    bridge_name: String,
    supported: Vec<(Chain, Chain)>,
    fee_rate: f64,
    base_time: u64,
    liquidity: f64,
    online: bool,
    success_rate: f64,
    _seed: u64,
}

impl MockBridge {
    pub fn new(name: &str, fee_rate: f64, base_time: u64, liquidity: f64) -> Self {
        Self {
            bridge_name: name.to_string(),
            supported: Self::default_pairs(),
            fee_rate,
            base_time,
            liquidity,
            online: true,
