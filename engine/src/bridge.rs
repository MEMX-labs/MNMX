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
            success_rate: 0.98,
            _seed: 42,
        }
    }

    pub fn with_pairs(mut self, pairs: Vec<(Chain, Chain)>) -> Self {
        self.supported = pairs;
        self
    }

    pub fn with_online(mut self, online: bool) -> Self {
        self.online = online;
        self
    }

    pub fn with_success_rate(mut self, rate: f64) -> Self {
        self.success_rate = rate;
        self
    }

    fn default_pairs() -> Vec<(Chain, Chain)> {
        let chains = [
            Chain::Ethereum,
            Chain::Arbitrum,
            Chain::Base,
            Chain::Polygon,
            Chain::Optimism,
            Chain::BnbChain,
            Chain::Avalanche,
        ];
        let mut pairs = Vec::new();
        for &a in &chains {
            for &b in &chains {
                if a != b {
                    pairs.push((a, b));
                }
            }
        }
        // Add Solana connections to major chains
        for &c in &[Chain::Ethereum, Chain::BnbChain, Chain::Polygon, Chain::Avalanche] {
            pairs.push((Chain::Solana, c));
            pairs.push((c, Chain::Solana));
        }
        pairs
    }

    /// Compute a deterministic-ish slippage based on amount and liquidity.
    fn compute_slippage(&self, amount: f64) -> f64 {
        if self.liquidity <= 0.0 {
            return 0.05;
        }
        let ratio = amount / self.liquidity;
        // Quadratic slippage model: higher amounts get worse slippage
        let slippage = ratio * ratio * 0.1 + ratio * 0.001;
        if slippage > 0.1 {
            0.1
        } else {
            slippage
        }
    }
}

impl BridgeAdapter for MockBridge {
    fn name(&self) -> &str {
        &self.bridge_name
    }

    fn supported_chains(&self) -> Vec<(Chain, Chain)> {
        self.supported.clone()
    }

    fn get_quote(
        &self,
        from_token: &Token,
        to_token: &Token,
        amount: f64,
    ) -> Option<BridgeQuote> {
        if !self.online {
            return None;
        }
        // Check that the pair is supported
        let pair = (from_token.chain, to_token.chain);
        if !self.supported.contains(&pair) {
            return None;
        }
        if amount <= 0.0 {
            return None;
        }

        let fee = amount * self.fee_rate;
        let slippage = self.compute_slippage(amount);
        let output = (amount - fee) * (1.0 - slippage);

        // Stablecoin-to-stablecoin transfers have tighter spreads
        let output = if from_token.is_stablecoin() && to_token.is_stablecoin() {
            (amount - fee) * (1.0 - slippage * 0.5)
        } else {
            output
        };

        // Time varies by chain pair
        let time_factor = match (from_token.chain, to_token.chain) {
            (Chain::Ethereum, _) | (_, Chain::Ethereum) => 2.0,
            (Chain::Solana, _) | (_, Chain::Solana) => 1.5,
            _ => 1.0,
        };
        let estimated_time = (self.base_time as f64 * time_factor) as u64;

        // Use a pseudo-random expiry based on seed
        let mut rng = rand::thread_rng();
        let expiry_offset: u64 = rng.gen_range(30..120);
        let expires_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
            + expiry_offset;

        Some(BridgeQuote {
            bridge_name: self.bridge_name.clone(),
            input_amount: amount,
            output_amount: output,
            fee,
            estimated_time,
            liquidity_depth: self.liquidity,
            expires_at,
        })
    }

    fn get_health(&self) -> BridgeHealth {
        BridgeHealth {
            online: self.online,
            congestion: if self.online {
                CongestionLevel::Low
            } else {
                CongestionLevel::High
            },
            success_rate: self.success_rate,
            median_confirm_time: self.base_time,
        }
    }
}

/// Build a default bridge registry with mock bridges for testing.
pub fn build_mock_registry() -> BridgeRegistry {
    let mut registry = BridgeRegistry::new();

    registry.register(Box::new(
        MockBridge::new("Wormhole", 0.003, 180, 5_000_000.0),
    ));
    registry.register(Box::new(
        MockBridge::new("deBridge", 0.004, 120, 2_000_000.0),
    ));
    registry.register(Box::new(
        MockBridge::new("LayerZero", 0.002, 60, 10_000_000.0),
    ));
    registry.register(Box::new(
        MockBridge::new("Allbridge", 0.005, 90, 1_000_000.0),
    ));

    registry
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_registration() {
        let registry = build_mock_registry();
        assert_eq!(registry.bridge_count(), 4);
    }

    #[test]
    fn test_get_bridges_for_pair() {
        let registry = build_mock_registry();
        let bridges = registry.get_bridges_for_pair(Chain::Ethereum, Chain::Arbitrum);
        assert!(!bridges.is_empty());
    }

    #[test]
    fn test_mock_bridge_quote() {
        let bridge = MockBridge::new("Test", 0.003, 120, 1_000_000.0);
        let from = Token::new("USDC", Chain::Ethereum, 6, "0xA0b8...");
        let to = Token::new("USDC", Chain::Arbitrum, 6, "0xB0c9...");
        let quote = bridge.get_quote(&from, &to, 1000.0);
        assert!(quote.is_some());
        let q = quote.unwrap();
        assert!(q.output_amount > 0.0);
        assert!(q.output_amount < 1000.0);
        assert!(q.fee > 0.0);
    }

    #[test]
    fn test_offline_bridge_no_quote() {
        let bridge = MockBridge::new("Offline", 0.003, 120, 1_000_000.0).with_online(false);
        let from = Token::new("USDC", Chain::Ethereum, 6, "0xA0b8...");
        let to = Token::new("USDC", Chain::Arbitrum, 6, "0xB0c9...");
        assert!(bridge.get_quote(&from, &to, 1000.0).is_none());
    }
}
