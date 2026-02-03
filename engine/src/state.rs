use crate::types::{Chain, CongestionLevel, Token};
use std::collections::HashMap;

/// State of a single blockchain.
#[derive(Debug, Clone)]
pub struct ChainState {
    pub chain: Chain,
    pub block_number: u64,
    pub gas_price: f64,
    pub congestion_level: CongestionLevel,
}

/// State of a single bridge.
#[derive(Debug, Clone)]
pub struct BridgeState {
    pub bridge: String,
    pub online: bool,
    pub liquidity_available: f64,
    pub pending_transactions: u32,
    pub average_confirmation_time: u64,
}

/// Aggregate market state across all chains.
#[derive(Debug, Clone)]
pub struct MarketState {
    pub token_prices: HashMap<String, f64>,
    pub gas_prices: HashMap<Chain, f64>,
    pub bridge_states: HashMap<String, BridgeState>,
    pub chain_states: HashMap<Chain, ChainState>,
}

impl MarketState {
    pub fn new() -> Self {
        Self {
            token_prices: HashMap::new(),
            gas_prices: HashMap::new(),
            bridge_states: HashMap::new(),
            chain_states: HashMap::new(),
        }
    }
}

impl Default for MarketState {
    fn default() -> Self {
        Self::new()
    }
}

/// Collects and maintains market state for routing decisions.
pub struct StateCollector {
    state: MarketState,
}

impl StateCollector {
    pub fn new() -> Self {
        Self {
            state: MarketState::new(),
        }
    }

    /// Build a complete market state snapshot. In production this would query
    /// RPC nodes and oracles; here we use deterministic simulated data.
    pub fn collect_state(&mut self) -> &MarketState {
        self.populate_token_prices();
        self.populate_gas_prices();
        self.populate_chain_states();
        self.populate_bridge_states();
        &self.state
    }

    /// Get the USD price for a token (by symbol).
    pub fn get_token_price(&self, token: &Token) -> f64 {
        let key = format!("{}_{}", token.symbol, token.chain);
        self.state
            .token_prices
            .get(&key)
            .or_else(|| self.state.token_prices.get(&token.symbol))
            .copied()
            .unwrap_or_else(|| Self::default_token_price(&token.symbol))
    }

    /// Get gas price for a chain in gwei.
    pub fn get_gas_price(&self, chain: Chain) -> f64 {
        self.state
            .gas_prices
            .get(&chain)
            .copied()
            .unwrap_or_else(|| chain.typical_gas_price_gwei())
    }

    /// Get available liquidity for a bridge.
    pub fn get_bridge_liquidity(&self, bridge_name: &str) -> f64 {
        self.state
            .bridge_states
            .get(bridge_name)
            .map(|bs| bs.liquidity_available)
            .unwrap_or(1_000_000.0)
    }

    /// Estimate slippage for a given trade amount against available liquidity.
    /// Uses a constant-product AMM model: slippage = amount / (liquidity + amount).
    pub fn estimate_slippage(&self, amount: f64, liquidity: f64) -> f64 {
        if liquidity <= 0.0 || amount <= 0.0 {
            return 0.0;
        }
        // Constant-product slippage model
        let k = liquidity * liquidity;
        let new_reserve = liquidity + amount;
        let output = liquidity - k / new_reserve;
        let ideal_output = amount; // For stablecoins, 1:1 is ideal
        if ideal_output <= 0.0 {
            return 0.0;
        }
        let slippage = 1.0 - output / ideal_output;
        if slippage < 0.0 { 0.0 } else { slippage }
    }

    /// Get congestion level for a chain.
    pub fn get_congestion(&self, chain: Chain) -> CongestionLevel {
        self.state
            .chain_states
            .get(&chain)
            .map(|cs| cs.congestion_level)
            .unwrap_or(CongestionLevel::Low)
    }

    // ------- Internal population methods -------

    fn populate_token_prices(&mut self) {
        let prices = vec![
            ("ETH", 3200.0),
            ("BTC", 62000.0),
            ("SOL", 145.0),
            ("MATIC", 0.72),
            ("POL", 0.72),
            ("BNB", 580.0),
            ("AVAX", 35.0),
            ("USDC", 1.0),
            ("USDT", 1.0),
            ("DAI", 1.0),
            ("WETH", 3200.0),
            ("WBTC", 62000.0),
            ("ARB", 1.15),
            ("OP", 2.40),
            ("LINK", 14.50),
            ("UNI", 7.80),
        ];
        for (symbol, price) in prices {
            self.state.token_prices.insert(symbol.to_string(), price);
        }
    }

    fn populate_gas_prices(&mut self) {
        for chain in Chain::all() {
            // Simulate slight variance around typical gas price
            let base = chain.typical_gas_price_gwei();
            // Deterministic "variance" based on chain id
            let variance_factor = 1.0 + (chain.chain_id() % 7) as f64 * 0.02;
            self.state.gas_prices.insert(*chain, base * variance_factor);
        }
    }

    fn populate_chain_states(&mut self) {
        for chain in Chain::all() {
            let congestion = match chain {
                Chain::Ethereum => CongestionLevel::Medium,
                Chain::Polygon => CongestionLevel::Low,
                Chain::BnbChain => CongestionLevel::Low,
                Chain::Solana => CongestionLevel::Low,
                _ => CongestionLevel::Low,
            };
            let block = match chain {
                Chain::Ethereum => 19_500_000,
                Chain::Solana => 250_000_000,
                Chain::Arbitrum => 180_000_000,
                Chain::Base => 12_000_000,
                Chain::Polygon => 55_000_000,
                Chain::BnbChain => 37_000_000,
                Chain::Optimism => 118_000_000,
                Chain::Avalanche => 44_000_000,
            };
            self.state.chain_states.insert(
                *chain,
                ChainState {
                    chain: *chain,
                    block_number: block,
                    gas_price: self.get_gas_price(*chain),
                    congestion_level: congestion,
                },
            );
        }
    }

    fn populate_bridge_states(&mut self) {
        let bridges = vec![
            ("Wormhole", true, 5_000_000.0, 12, 180),
            ("deBridge", true, 2_000_000.0, 5, 120),
            ("LayerZero", true, 10_000_000.0, 20, 60),
            ("Allbridge", true, 1_000_000.0, 3, 90),
        ];
        for (name, online, liq, pending, confirm_time) in bridges {
            self.state.bridge_states.insert(
                name.to_string(),
                BridgeState {
                    bridge: name.to_string(),
                    online,
                    liquidity_available: liq,
                    pending_transactions: pending,
                    average_confirmation_time: confirm_time,
                },
            );
        }
    }

    fn default_token_price(symbol: &str) -> f64 {
        match symbol.to_uppercase().as_str() {
            "ETH" | "WETH" => 3200.0,
            "BTC" | "WBTC" => 62000.0,
            "SOL" => 145.0,
            "BNB" => 580.0,
            "AVAX" => 35.0,
            "MATIC" | "POL" => 0.72,
            "USDC" | "USDT" | "DAI" | "BUSD" | "FRAX" => 1.0,
            _ => 1.0,
        }
    }
}

impl Default for StateCollector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_state_collection() {
        let mut collector = StateCollector::new();
        let state = collector.collect_state();
        assert!(!state.token_prices.is_empty());
        assert!(!state.gas_prices.is_empty());
        assert!(!state.bridge_states.is_empty());
    }

    #[test]
    fn test_token_price() {
        let mut collector = StateCollector::new();
        collector.collect_state();
        let eth = Token::new("ETH", Chain::Ethereum, 18, "0x0");
        let price = collector.get_token_price(&eth);
        assert!(price > 1000.0);
    }

    #[test]
    fn test_slippage_estimation() {
        let collector = StateCollector::new();
        // Small trade relative to liquidity -> low slippage
        let slip_small = collector.estimate_slippage(100.0, 1_000_000.0);
        assert!(slip_small < 0.001);
        // Large trade -> higher slippage
        let slip_large = collector.estimate_slippage(500_000.0, 1_000_000.0);
        assert!(slip_large > slip_small);
    }

    #[test]
    fn test_gas_price() {
        let mut collector = StateCollector::new();
        collector.collect_state();
        let gas = collector.get_gas_price(Chain::Ethereum);
        assert!(gas > 0.0);
    }
}
