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
