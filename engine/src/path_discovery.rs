use crate::bridge::BridgeRegistry;
use crate::types::{Chain, Token};

/// Candidate path: a sequence of (from_chain, to_chain, bridge_name) tuples.
#[derive(Debug, Clone)]
pub struct CandidatePath {
    pub steps: Vec<PathStep>,
}

#[derive(Debug, Clone)]
pub struct PathStep {
    pub from_chain: Chain,
    pub to_chain: Chain,
    pub from_token: Token,
    pub to_token: Token,
    pub bridge_name: String,
}

/// Discovers all possible paths between two chain/token pairs.
pub struct PathDiscovery<'a> {
    registry: &'a BridgeRegistry,
    max_hops: usize,
}

impl<'a> PathDiscovery<'a> {
    pub fn new(registry: &'a BridgeRegistry, max_hops: usize) -> Self {
        Self { registry, max_hops }
    }

    /// Enumerate all candidate paths from source to destination.
    pub fn discover_paths(
        &self,
        from_chain: Chain,
        from_token: &Token,
        to_chain: Chain,
        to_token: &Token,
    ) -> Vec<CandidatePath> {
        let mut all_paths = Vec::new();

        // Direct paths (1 hop)
        let direct = self.expand_direct_paths(from_chain, from_token, to_chain, to_token);
        all_paths.extend(direct);

        // Multi-hop paths (2-3 hops)
        if self.max_hops >= 2 {
            let multi = self.expand_multi_hop_paths(from_chain, from_token, to_chain, to_token);
            all_paths.extend(multi);
        }

        // Remove dominated and duplicate paths
        let filtered = self.filter_dominated_paths(all_paths);
        self.deduplicate_paths(filtered)
    }

    /// Find all single-hop direct bridges between two chains.
    pub fn expand_direct_paths(
        &self,
        from_chain: Chain,
        from_token: &Token,
        to_chain: Chain,
        to_token: &Token,
    ) -> Vec<CandidatePath> {
        let bridges = self.registry.get_bridges_for_pair(from_chain, to_chain);
        bridges
            .into_iter()
            .map(|bridge| {
                CandidatePath {
                    steps: vec![PathStep {
                        from_chain,
                        to_chain,
                        from_token: from_token.clone(),
                        to_token: to_token.clone(),
                        bridge_name: bridge.name().to_string(),
                    }],
                }
            })
            .collect()
    }

    /// Expand multi-hop paths through intermediate chains.
    pub fn expand_multi_hop_paths(
        &self,
        from_chain: Chain,
        from_token: &Token,
        to_chain: Chain,
        to_token: &Token,
    ) -> Vec<CandidatePath> {
        let mut paths = Vec::new();
        let intermediates = self.get_intermediate_chains(from_chain, to_chain);

        // 2-hop paths: from -> intermediate -> to
        for &mid_chain in &intermediates {
            let mid_token = self.infer_intermediate_token(from_token, mid_chain);
            let first_leg = self.registry.get_bridges_for_pair(from_chain, mid_chain);
            let second_leg = self.registry.get_bridges_for_pair(mid_chain, to_chain);

            for bridge1 in &first_leg {
                for bridge2 in &second_leg {
                    paths.push(CandidatePath {
                        steps: vec![
                            PathStep {
                                from_chain,
                                to_chain: mid_chain,
                                from_token: from_token.clone(),
                                to_token: mid_token.clone(),
                                bridge_name: bridge1.name().to_string(),
                            },
                            PathStep {
                                from_chain: mid_chain,
                                to_chain,
                                from_token: mid_token.clone(),
                                to_token: to_token.clone(),
                                bridge_name: bridge2.name().to_string(),
                            },
                        ],
                    });
                }
            }
        }

        // 3-hop paths (only if max_hops >= 3)
        if self.max_hops >= 3 {
            for &mid1 in &intermediates {
                let mid1_token = self.infer_intermediate_token(from_token, mid1);
                let second_intermediates = self.get_intermediate_chains(mid1, to_chain);

                for &mid2 in &second_intermediates {
                    if mid2 == from_chain || mid2 == mid1 {
                        continue;
                    }
                    let mid2_token = self.infer_intermediate_token(from_token, mid2);

                    let leg1 = self.registry.get_bridges_for_pair(from_chain, mid1);
                    let leg2 = self.registry.get_bridges_for_pair(mid1, mid2);
                    let leg3 = self.registry.get_bridges_for_pair(mid2, to_chain);

                    if leg1.is_empty() || leg2.is_empty() || leg3.is_empty() {
                        continue;
                    }

                    // For 3-hop, only use the best bridge per leg to limit combinatorics
                    let b1_name = leg1[0].name().to_string();
