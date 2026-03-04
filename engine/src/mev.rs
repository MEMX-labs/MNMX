use crate::math;
use crate::types::*;

/// Known MEV bot address prefixes on Solana.
/// These are patterns commonly seen in MEV bot program addresses.
const KNOWN_MEV_PREFIXES: &[&str] = &[
    "JUP",    // Jupiter aggregator bots
    "MEV",    // Generic MEV label
    "ARB",    // Arbitrage bots
    "SAND",   // Sandwich bots
    "FL4SH",  // Flash loan bots
    "B0T",    // Common bot naming
    "SNIP3R", // Sniper bots
    "jito",   // Jito MEV
    "BLXR",   // bloXroute relayers
];

/// Minimum trade size (as fraction of pool reserve) to consider MEV-worthy.
const MIN_MEV_FRACTION: f64 = 0.001;

/// Detects potential MEV threats for a given action by analyzing the
/// mempool, pool states, and known bot patterns.
#[derive(Debug, Clone)]
pub struct MevDetector {
    /// Sensitivity multiplier. Higher = more conservative (flags more threats).
    sensitivity: f64,
    /// Minimum probability threshold to report a threat.
    min_probability: f64,
}

impl MevDetector {
    pub fn new() -> Self {
        Self {
            sensitivity: 1.0,
            min_probability: 0.05,
        }
    }

    /// Create a detector with custom sensitivity.
    pub fn with_sensitivity(sensitivity: f64) -> Self {
        Self {
            sensitivity: math::clamp_f64(sensitivity, 0.1, 10.0),
            min_probability: 0.05,
        }
    }

    /// Analyze all MEV threat vectors for a given action.
    pub fn detect_threats(
        &self,
        action: &ExecutionAction,
        state: &OnChainState,
    ) -> Vec<MevThreat> {
        let mut threats = Vec::new();

        // Only pool-interactive actions are MEV targets
        match action.kind {
            ActionKind::Swap | ActionKind::AddLiquidity | ActionKind::RemoveLiquidity => {}
            _ => return threats,
        }

        let pools = &state.pool_states;
        let pending = &state.pending_transactions;

        if let Some(threat) = self.analyze_sandwich_risk(action, pending, pools) {
            if threat.probability >= self.min_probability {
                threats.push(threat);
            }
        }

        if let Some(threat) = self.analyze_frontrun_risk(action, pending) {
            if threat.probability >= self.min_probability {
                threats.push(threat);
            }
        }

        if let Some(threat) = self.analyze_backrun_risk(action, pending) {
            if threat.probability >= self.min_probability {
                threats.push(threat);
            }
        }

        if let Some(pool) = pools.iter().find(|p| p.address == action.pool_address) {
            if let Some(threat) = self.analyze_jit_risk(action, pool) {
                if threat.probability >= self.min_probability {
                    threats.push(threat);
                }
            }
        }

        threats
    }

    /// Analyze sandwich attack risk.
    ///
    /// A sandwich attack occurs when an attacker:
    /// 1. Front-runs the victim's swap to move the price
    /// 2. The victim's swap executes at a worse price
    /// 3. The attacker back-runs to capture the price difference
    ///
    /// Risk factors:
    /// - Large trade relative to pool reserves
    /// - Known MEV bots in the mempool
    /// - High pending transaction volume on the same pool
    pub fn analyze_sandwich_risk(
        &self,
        action: &ExecutionAction,
        pending_txs: &[PendingTx],
        pools: &[PoolState],
    ) -> Option<MevThreat> {
        if action.kind != ActionKind::Swap {
            return None;
        }

        let pool = pools.iter().find(|p| p.address == action.pool_address)?;

        // Check if trade is large enough to sandwich
        let trade_fraction = action.amount as f64 / pool.reserve_a.max(1) as f64;
        if trade_fraction < MIN_MEV_FRACTION {
            return None;
        }

        // Signal 1: trade size relative to pool
        let size_signal = math::clamp_f64(trade_fraction * 20.0, 0.0, 1.0);

        // Signal 2: known MEV bots in pending txs for this pool
        let bot_txs: Vec<&PendingTx> = pending_txs
            .iter()
            .filter(|tx| tx.to == pool.address && Self::is_known_mev_bot(&tx.from))
            .collect();
        let bot_signal = math::clamp_f64(bot_txs.len() as f64 * 0.3, 0.0, 1.0);

        // Signal 3: pending transaction volume targeting same pool
        let pool_pending: Vec<&PendingTx> = pending_txs
            .iter()
            .filter(|tx| tx.to == pool.address)
            .collect();
        let volume_signal = if pool_pending.is_empty() {
            0.0
        } else {
            let total_pending_amount: u64 = pool_pending.iter().map(|tx| tx.amount).sum();
            math::clamp_f64(
                total_pending_amount as f64 / pool.reserve_a.max(1) as f64,
                0.0,
                1.0,
            )
        };

        // Signal 4: high-fee transactions (MEV bots pay more to get priority)
        let high_fee_txs = pool_pending
            .iter()
            .filter(|tx| tx.fee > action.priority_fee * 2)
            .count();
        let fee_signal = math::clamp_f64(high_fee_txs as f64 * 0.25, 0.0, 1.0);

        let signals = vec![size_signal, bot_signal, volume_signal, fee_signal];
        let probability = Self::calculate_probability(&signals) * self.sensitivity;
        let probability = math::clamp_f64(probability, 0.0, 0.99);

        let estimated_cost = Self::estimate_sandwich_cost(action.amount, pool);

        let source = bot_txs
            .first()
            .map(|tx| tx.from.clone())
            .unwrap_or_else(|| "unknown_sandwich_bot".to_string());

        Some(MevThreat::new(
            MevKind::Sandwich,
            probability,
            estimated_cost,
            &source,
            &pool.address,
        ))
    }

    /// Analyze front-running risk.
    ///
    /// Front-running occurs when a bot sees a pending profitable transaction
    /// and submits its own transaction with a higher fee to execute first.
    pub fn analyze_frontrun_risk(
        &self,
        action: &ExecutionAction,
        pending_txs: &[PendingTx],
    ) -> Option<MevThreat> {
        // Find pending txs targeting the same destination with higher fees
        let competing: Vec<&PendingTx> = pending_txs
            .iter()
            .filter(|tx| {
                tx.to == action.pool_address && tx.fee > action.priority_fee
            })
            .collect();

        if competing.is_empty() {
            return None;
        }

        // Signal 1: number of competing high-fee transactions
        let count_signal = math::clamp_f64(competing.len() as f64 * 0.2, 0.0, 1.0);

        // Signal 2: fee premium of competing transactions
        let max_competing_fee = competing.iter().map(|tx| tx.fee).max().unwrap_or(0);
        let fee_ratio = if action.priority_fee > 0 {
            max_competing_fee as f64 / action.priority_fee as f64
        } else {
            5.0 // High signal if our fee is zero
        };
        let fee_signal = math::clamp_f64((fee_ratio - 1.0) * 0.5, 0.0, 1.0);

        // Signal 3: known bots
        let bot_count = competing
            .iter()
            .filter(|tx| Self::is_known_mev_bot(&tx.from))
            .count();
        let bot_signal = math::clamp_f64(bot_count as f64 * 0.4, 0.0, 1.0);

        let signals = vec![count_signal, fee_signal, bot_signal];
        let probability = Self::calculate_probability(&signals) * self.sensitivity;
        let probability = math::clamp_f64(probability, 0.0, 0.99);

        let cost = Self::estimate_frontrun_cost(action, competing[0]);

        let source = competing[0].from.clone();

        Some(MevThreat::new(
            MevKind::Frontrun,
            probability,
            cost,
            &source,
            &action.pool_address,
        ))
    }

    /// Analyze back-running risk.
    ///
    /// Back-running occurs when a bot submits a transaction immediately
    /// after the victim's transaction to profit from the price movement.
    pub fn analyze_backrun_risk(
        &self,
        action: &ExecutionAction,
        pending_txs: &[PendingTx],
    ) -> Option<MevThreat> {
        if action.kind != ActionKind::Swap {
            return None;
        }

        // Back-runners watch for large swaps that create arbitrage opportunities
        // Check if there are pending txs from known bots
        let watchers: Vec<&PendingTx> = pending_txs
            .iter()
            .filter(|tx| Self::is_known_mev_bot(&tx.from))
            .collect();

        if watchers.is_empty() {
            return None;
        }

        // Signal 1: number of watching bots
        let watcher_signal = math::clamp_f64(watchers.len() as f64 * 0.15, 0.0, 1.0);

        // Signal 2: action amount (larger = more backrun profit)
        let amount_signal = math::clamp_f64(action.amount as f64 / 1_000_000.0, 0.0, 1.0);

        let signals = vec![watcher_signal, amount_signal];
        let probability = Self::calculate_probability(&signals) * self.sensitivity;
        let probability = math::clamp_f64(probability, 0.0, 0.95);

        // Backrun cost is typically lower than sandwich
        let cost = action.amount / 500;

        let source = watchers[0].from.clone();

        Some(MevThreat::new(
            MevKind::Backrun,
            probability,
            cost,
            &source,
            &action.pool_address,
        ))
    }

    /// Analyze JIT (Just-In-Time) liquidity risk.
    ///
    /// JIT liquidity providers add concentrated liquidity right before a
    /// large swap and remove it immediately after, capturing most of the
    /// trading fees while diluting existing LPs.
    pub fn analyze_jit_risk(
        &self,
        action: &ExecutionAction,
        pool: &PoolState,
    ) -> Option<MevThreat> {
        if action.kind != ActionKind::Swap {
            return None;
        }

        // JIT is profitable when the swap fee income exceeds the JIT gas cost
        let fee_income = math::bps_to_decimal(pool.fee_rate_bps) * action.amount as f64;

        // If the fee income is less than ~10_000 lamports, JIT isn't worth it
        if fee_income < 10_000.0 {
            return None;
        }

        // Signal 1: swap fee income potential
        let fee_signal = math::clamp_f64(fee_income / 100_000.0, 0.0, 1.0);

        // Signal 2: trade size relative to pool liquidity
        let size_signal = if pool.reserve_a > 0 {
            math::clamp_f64(
                action.amount as f64 / pool.reserve_a as f64 * 5.0,
                0.0,
                1.0,
            )
        } else {
            0.0
        };

        // Signal 3: pool has concentrated liquidity features (sqrt_price > 0)
        let cl_signal = if pool.sqrt_price > 0 { 0.6 } else { 0.2 };

        let signals = vec![fee_signal, size_signal, cl_signal];
        let probability = Self::calculate_probability(&signals) * self.sensitivity;
        let probability = math::clamp_f64(probability, 0.0, 0.90);

        // JIT cost to the victim = portion of fees captured by the JIT provider
        let cost = (fee_income * 0.8) as u64; // JIT captures ~80% of fees

        Some(MevThreat::new(
            MevKind::JitLiquidity,
            probability,
            cost,
            "jit_provider",
            &pool.address,
        ))
    }

    /// Estimate the cost of a sandwich attack on a given swap.
    ///
    /// The sandwich profit ≈ price_impact * amount * multiplier.
    /// The victim bears this as additional slippage.
    pub fn estimate_sandwich_cost(action_amount: u64, pool: &PoolState) -> u64 {
        let impact = math::calculate_price_impact(
            action_amount,
            pool.reserve_a,
            pool.reserve_b,
        );
        // Sandwich extracts roughly 2x the price impact as profit
        let cost = action_amount as f64 * impact * 2.0;
        if cost > u64::MAX as f64 {
            u64::MAX
        } else {
            cost as u64
        }
    }

    /// Estimate the cost of a front-run to the agent.
    ///
    /// The front-runner captures the price improvement that the agent
    /// would have received.
    pub fn estimate_frontrun_cost(action: &ExecutionAction, competing_tx: &PendingTx) -> u64 {
        // Cost ≈ the amount of price improvement stolen
        // Heuristic: proportional to the competing tx amount relative to ours
        let ratio = if action.amount > 0 {
            competing_tx.amount as f64 / action.amount as f64
        } else {
            1.0
        };
        let cost = action.amount as f64 * ratio.min(1.0) * 0.01; // ~1% cost
        cost as u64
    }

    /// Check if an address matches known MEV bot patterns.
    pub fn is_known_mev_bot(address: &str) -> bool {
        // Check prefixes
        for prefix in KNOWN_MEV_PREFIXES {
            if address.starts_with(prefix) {
                return true;
            }
        }

        // Check for common bot naming patterns (case-insensitive)
        let lower = address.to_lowercase();
        if lower.contains("bot")
            || lower.contains("mev")
            || lower.contains("arb")
            || lower.contains("sniper")
            || lower.contains("sandwich")
            || lower.contains("flash")
        {
            return true;
        }

        false
    }

    /// Combine multiple signals into a single probability using a
    /// logistic combination.
    ///
    /// Each signal is in [0, 1]. The combination:
    /// 1. Compute the weighted sum of log-odds
    /// 2. Pass through logistic function
    ///
    /// This ensures the output is always in (0, 1) and combines
    /// evidence from multiple independent signals.
    pub fn calculate_probability(signals: &[f64]) -> f64 {
        if signals.is_empty() {
            return 0.0;
        }

        // Convert each signal to log-odds, sum, then back to probability
        let mut log_odds_sum = 0.0;
        let mut valid_count = 0;

        for &signal in signals {
            let clamped = math::clamp_f64(signal, 0.01, 0.99);
            let log_odds = (clamped / (1.0 - clamped)).ln();
            log_odds_sum += log_odds;
            valid_count += 1;
        }

        if valid_count == 0 {
            return 0.0;
        }

        // Average the log-odds to prevent extreme values
        let avg_log_odds = log_odds_sum / valid_count as f64;

        // Apply logistic to get back to probability
        math::logistic(avg_log_odds)
    }
}

impl Default for MevDetector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_pool() -> PoolState {
        PoolState::new("pool1", 10_000_000, 20_000_000, 30, "SOL", "USDC")
    }

    fn test_swap(amount: u64) -> ExecutionAction {
        ExecutionAction::new(ActionKind::Swap, "SOL", amount, "USDC", 50, "pool1", 5000)
    }

    #[test]
    fn test_is_known_mev_bot() {
        assert!(MevDetector::is_known_mev_bot("MEVbot123"));
        assert!(MevDetector::is_known_mev_bot("JUPaggregator"));
        assert!(MevDetector::is_known_mev_bot("my_mev_program"));
        assert!(MevDetector::is_known_mev_bot("sandwich_attacker"));
        assert!(!MevDetector::is_known_mev_bot("normalUser123"));
        assert!(!MevDetector::is_known_mev_bot("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"));
    }

    #[test]
    fn test_no_threats_for_transfer() {
        let detector = MevDetector::new();
        let action = ExecutionAction::new(
            ActionKind::Transfer,
            "SOL",
            1000,
            "wallet2",
            0,
            "",
            5000,
        );
        let state = OnChainState::new(100, 1700000000);
        let threats = detector.detect_threats(&action, &state);
        assert!(threats.is_empty());
    }

    #[test]
    fn test_sandwich_detection_large_trade() {
        let detector = MevDetector::new();
        let pool = test_pool();
        let action = test_swap(500_000); // 5% of reserve

        let pending = vec![PendingTx::new(
            "sig1",
            "MEVbot1",
            "pool1",
            200_000,
            100,
            20_000,
        )];

        let threat =
            detector.analyze_sandwich_risk(&action, &pending, &[pool]);
        assert!(threat.is_some());
        let t = threat.unwrap();
        assert_eq!(t.kind, MevKind::Sandwich);
        assert!(t.probability > 0.0);
        assert!(t.estimated_cost > 0);
    }

    #[test]
    fn test_no_sandwich_tiny_trade() {
        let detector = MevDetector::new();
        let pool = test_pool();
        let action = test_swap(100); // Tiny trade

        let threat =
            detector.analyze_sandwich_risk(&action, &[], &[pool]);
        assert!(threat.is_none());
    }

    #[test]
    fn test_frontrun_detection() {
        let detector = MevDetector::new();
        let action = test_swap(100_000);
        let pending = vec![PendingTx::new(
            "sig1",
            "ARBbot",
            "pool1",
            50_000,
            100,
            50_000, // Higher fee than our 5000
        )];

        let threat = detector.analyze_frontrun_risk(&action, &pending);
        assert!(threat.is_some());
        assert_eq!(threat.unwrap().kind, MevKind::Frontrun);
    }

    #[test]
    fn test_jit_detection() {
        let detector = MevDetector::new();
        let pool = test_pool();
        // Large enough swap to make JIT profitable
        let action = test_swap(5_000_000);

        let threat = detector.analyze_jit_risk(&action, &pool);
        assert!(threat.is_some());
        assert_eq!(threat.unwrap().kind, MevKind::JitLiquidity);
    }

    #[test]
    fn test_probability_calculation() {
        // All zero signals => probability near 0.01 (clamped minimum)
        let p = MevDetector::calculate_probability(&[0.01, 0.01]);
        assert!(p < 0.1);

        // All high signals => probability near 1.0
        let p = MevDetector::calculate_probability(&[0.9, 0.9, 0.9]);
        assert!(p > 0.8);

        // Mixed signals => moderate probability
        let p = MevDetector::calculate_probability(&[0.8, 0.2]);
        assert!(p > 0.3 && p < 0.7);
    }

    #[test]
    fn test_estimate_sandwich_cost() {
        let pool = test_pool();
        let cost = MevDetector::estimate_sandwich_cost(500_000, &pool);
        assert!(cost > 0);
        // Larger trade => higher cost
        let cost2 = MevDetector::estimate_sandwich_cost(1_000_000, &pool);
        assert!(cost2 > cost);
    }

    #[test]
    fn test_detect_threats_full() {
        let detector = MevDetector::new();
        let mut state = OnChainState::new(100, 1700000000);
        state.pool_states.push(test_pool());
        state.pending_transactions.push(PendingTx::new(
            "sig1",
            "MEVbot1",
            "pool1",
            200_000,
            100,
            50_000,
        ));

        let action = test_swap(500_000);
        let threats = detector.detect_threats(&action, &state);
        // Should detect at least sandwich and possibly others
        assert!(!threats.is_empty());
    }
}
