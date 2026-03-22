# Roadmap

---

## Core Engine

- [x] Minimax search with alpha-beta pruning and move ordering
- [x] 5-dimension scoring (fees, slippage, speed, reliability, MEV exposure)
- [x] 5 adversarial scenarios with configurable stress multipliers
- [x] Strategy profiles — minimax, cheapest, fastest, safest
- [x] Path discovery — direct, 2-hop, 3-hop with pruning
- [x] Chain-specific MEV risk factors

## Bridge Adapters

- [x] Wormhole — guardian network, VAA, relayer
- [x] deBridge — DLN intent-based fills
- [x] LayerZero — DVN verification, OFT support
- [x] Allbridge — liquidity pools, CCTP
- [x] Unified quote format and concurrent fetching
- [x] Adapter health monitoring

## Infrastructure

- [x] Multi-chain RPC with failover (8 chains)
- [x] Token registry with cross-chain address mapping
- [x] Gas oracle per chain (EIP-1559, Solana priority fees)
- [x] Real-time state tracking and cache management
- [x] Transaction builder (EVM + Solana)

## Execution

- [x] Hop-by-hop sequential execution with monitoring
- [x] Transaction simulation before submission
- [x] Automatic retry with exponential backoff
- [x] MEV protection (Flashbots, Jito bundles)
- [x] Partial execution recovery

## Testing & Docs

- [x] 41+ unit tests, integration tests, backtest framework
- [x] CI/CD pipeline
- [x] Full documentation, architecture diagrams, API reference
- [x] Open source — MIT license

---

## In Progress

- [x] Server-side route processing
- [x] Real-time multi-bridge quote aggregation
- [x] Incremental search
- [ ] Adapter circuit breaker and failover
- [ ] Structured logging with trace IDs
- [ ] Latency monitoring and alerting
- [ ] Dynamic adversarial calibration (per-chain, per-bridge multiplier tuning from execution data)
- [ ] Probabilistic path exploration (discover multi-hop routes beyond static connectivity graph)

## Planned

- [ ] Circle CCTP v2, Stargate, Hop, Across adapters
- [ ] Portfolio-aware routing and split transfers
- [ ] Sui, Aptos, Monad, Berachain support
- [ ] Python SDK and Rust SDK
- [ ] WebSocket API for real-time route streaming
- [ ] On-chain route verification and governance
- [ ] Staking for premium routing access

---

Last updated: 2026-03-22
