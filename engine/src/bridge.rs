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
