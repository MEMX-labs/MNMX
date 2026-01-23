use serde::{Deserialize, Serialize};
use std::fmt;

/// Supported blockchain networks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Chain {
    Ethereum,
    Solana,
    Arbitrum,
    Base,
    Polygon,
    BnbChain,
    Optimism,
    Avalanche,
}

impl Chain {
    pub fn all() -> &'static [Chain] {
        &[
            Chain::Ethereum,
            Chain::Solana,
            Chain::Arbitrum,
            Chain::Base,
            Chain::Polygon,
            Chain::BnbChain,
            Chain::Optimism,
            Chain::Avalanche,
        ]
    }

    pub fn chain_id(&self) -> u64 {
        match self {
            Chain::Ethereum => 1,
            Chain::Solana => 0,
            Chain::Arbitrum => 42161,
            Chain::Base => 8453,
            Chain::Polygon => 137,
            Chain::BnbChain => 56,
            Chain::Optimism => 10,
            Chain::Avalanche => 43114,
        }
    }

    pub fn is_evm(&self) -> bool {
        !matches!(self, Chain::Solana)
    }

    pub fn average_block_time_ms(&self) -> u64 {
        match self {
            Chain::Ethereum => 12_000,
            Chain::Solana => 400,
            Chain::Arbitrum => 250,
            Chain::Base => 2_000,
            Chain::Polygon => 2_000,
            Chain::BnbChain => 3_000,
            Chain::Optimism => 2_000,
            Chain::Avalanche => 2_000,
        }
    }

    pub fn typical_gas_price_gwei(&self) -> f64 {
        match self {
            Chain::Ethereum => 30.0,
            Chain::Solana => 0.000005,
            Chain::Arbitrum => 0.1,
            Chain::Base => 0.01,
            Chain::Polygon => 40.0,
            Chain::BnbChain => 3.0,
            Chain::Optimism => 0.01,
            Chain::Avalanche => 25.0,
        }
    }
}

impl fmt::Display for Chain {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Chain::Ethereum => "Ethereum",
            Chain::Solana => "Solana",
            Chain::Arbitrum => "Arbitrum",
            Chain::Base => "Base",
            Chain::Polygon => "Polygon",
            Chain::BnbChain => "BNB Chain",
            Chain::Optimism => "Optimism",
            Chain::Avalanche => "Avalanche",
        };
        write!(f, "{}", name)
    }
}

/// A token on a specific chain.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Token {
    pub symbol: String,
    pub chain: Chain,
    pub decimals: u8,
    pub address: String,
}

impl Token {
    pub fn new(symbol: &str, chain: Chain, decimals: u8, address: &str) -> Self {
        Self {
            symbol: symbol.to_string(),
            chain,
            decimals,
            address: address.to_string(),
        }
    }

    pub fn is_stablecoin(&self) -> bool {
        let sym = self.symbol.to_uppercase();
        sym == "USDC" || sym == "USDT" || sym == "DAI" || sym == "BUSD" || sym == "FRAX"
    }

    pub fn is_native(&self) -> bool {
        let sym = self.symbol.to_uppercase();
        match self.chain {
            Chain::Ethereum | Chain::Arbitrum | Chain::Base | Chain::Optimism => sym == "ETH",
            Chain::Solana => sym == "SOL",
            Chain::Polygon => sym == "MATIC" || sym == "POL",
            Chain::BnbChain => sym == "BNB",
            Chain::Avalanche => sym == "AVAX",
        }
    }

    pub fn amount_from_human(&self, human_amount: f64) -> u128 {
        let factor = 10u128.pow(self.decimals as u32);
        (human_amount * factor as f64) as u128
    }

    pub fn amount_to_human(&self, raw_amount: u128) -> f64 {
        let factor = 10u128.pow(self.decimals as u32);
        raw_amount as f64 / factor as f64
    }
}

/// A complete route from source to destination.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Route {
    pub hops: Vec<RouteHop>,
    pub expected_output: f64,
    pub guaranteed_minimum: f64,
    pub total_fees: f64,
    pub estimated_time: u64,
    pub minimax_score: f64,
}

impl Route {
    pub fn new() -> Self {
        Self {
            hops: Vec::new(),
            expected_output: 0.0,
            guaranteed_minimum: 0.0,
            total_fees: 0.0,
            estimated_time: 0,
            minimax_score: 0.0,
        }
    }

    pub fn hop_count(&self) -> usize {
        self.hops.len()
    }

    pub fn total_fee_percentage(&self, input_amount: f64) -> f64 {
        if input_amount <= 0.0 {
            return 0.0;
        }
        (self.total_fees / input_amount) * 100.0
    }

    pub fn value_retention(&self, input_amount: f64) -> f64 {
        if input_amount <= 0.0 {
            return 0.0;
        }
        self.expected_output / input_amount
    }

    pub fn bridges_used(&self) -> Vec<String> {
        self.hops.iter().map(|h| h.bridge.clone()).collect()
    }

    pub fn chains_traversed(&self) -> Vec<Chain> {
        let mut chains = Vec::new();
        for hop in &self.hops {
            if chains.last() != Some(&hop.from_chain) {
                chains.push(hop.from_chain);
            }
            chains.push(hop.to_chain);
        }
        chains
    }
}

impl Default for Route {
    fn default() -> Self {
        Self::new()
    }
}

/// A single hop in a route.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteHop {
    pub from_chain: Chain,
    pub to_chain: Chain,
    pub from_token: Token,
    pub to_token: Token,
    pub bridge: String,
    pub input_amount: f64,
    pub output_amount: f64,
    pub fee: f64,
    pub estimated_time: u64,
}

impl RouteHop {
    pub fn fee_percentage(&self) -> f64 {
        if self.input_amount <= 0.0 {
            return 0.0;
        }
        (self.fee / self.input_amount) * 100.0
    }

    pub fn slippage(&self) -> f64 {
        if self.input_amount <= 0.0 {
            return 0.0;
        }
        1.0 - (self.output_amount + self.fee) / self.input_amount
    }
}

/// A quote from a bridge for a specific transfer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeQuote {
    pub bridge_name: String,
    pub input_amount: f64,
    pub output_amount: f64,
    pub fee: f64,
    pub estimated_time: u64,
    pub liquidity_depth: f64,
    pub expires_at: u64,
}

impl BridgeQuote {
    pub fn effective_rate(&self) -> f64 {
        if self.input_amount <= 0.0 {
            return 0.0;
        }
        self.output_amount / self.input_amount
    }

    pub fn is_expired(&self, current_time: u64) -> bool {
        current_time >= self.expires_at
    }
}

/// Health status of a bridge.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeHealth {
    pub online: bool,
    pub congestion: CongestionLevel,
    pub success_rate: f64,
    pub median_confirm_time: u64,
}

impl BridgeHealth {
    pub fn reliability_score(&self) -> f64 {
        if !self.online {
            return 0.0;
        }
        let congestion_factor = match self.congestion {
            CongestionLevel::Low => 1.0,
            CongestionLevel::Medium => 0.8,
            CongestionLevel::High => 0.5,
        };
        self.success_rate * congestion_factor
    }
}

/// Network congestion level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CongestionLevel {
    Low,
    Medium,
    High,
}

/// Risk classification for a route.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

impl fmt::Display for RiskLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RiskLevel::Low => write!(f, "Low"),
            RiskLevel::Medium => write!(f, "Medium"),
            RiskLevel::High => write!(f, "High"),
            RiskLevel::Critical => write!(f, "Critical"),
        }
    }
}

/// A request for route finding.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteRequest {
    pub from_chain: Chain,
    pub from_token: Token,
    pub to_chain: Chain,
    pub to_token: Token,
    pub amount: f64,
    pub strategy: Strategy,
    pub max_hops: usize,
}

/// Routing strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Strategy {
    Minimax,
    Cheapest,
    Fastest,
    Safest,
}

impl fmt::Display for Strategy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Strategy::Minimax => write!(f, "Minimax"),
            Strategy::Cheapest => write!(f, "Cheapest"),
            Strategy::Fastest => write!(f, "Fastest"),
            Strategy::Safest => write!(f, "Safest"),
        }
    }
}

/// Weights for multi-objective scoring.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoringWeights {
    pub fees: f64,
    pub slippage: f64,
    pub speed: f64,
    pub reliability: f64,
    pub mev_exposure: f64,
}

impl ScoringWeights {
    pub fn sum(&self) -> f64 {
        self.fees + self.slippage + self.speed + self.reliability + self.mev_exposure
    }

    pub fn is_valid(&self) -> bool {
        let sum = self.sum();
        (sum - 1.0).abs() < 1e-6
            && self.fees >= 0.0
            && self.slippage >= 0.0
            && self.speed >= 0.0
            && self.reliability >= 0.0
            && self.mev_exposure >= 0.0
