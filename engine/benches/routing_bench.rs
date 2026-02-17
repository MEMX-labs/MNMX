use criterion::{black_box, criterion_group, criterion_main, Criterion};
use mnmx_engine::bridge::build_mock_registry;
use mnmx_engine::minimax::MinimaxSearcher;
use mnmx_engine::path_discovery::PathDiscovery;
use mnmx_engine::scoring::RouteScorer;
use mnmx_engine::types::*;

fn eth_usdc() -> Token {
    Token::new("USDC", Chain::Ethereum, 6, "0xA0b86991")
}

fn arb_usdc() -> Token {
    Token::new("USDC", Chain::Arbitrum, 6, "0xaf88d065")
}

fn base_usdc() -> Token {
    Token::new("USDC", Chain::Base, 6, "0x833589fC")
}

fn bench_path_discovery(c: &mut Criterion) {
    let registry = build_mock_registry();

    c.bench_function("path_discovery_2hop", |b| {
        b.iter(|| {
            let pd = PathDiscovery::new(&registry, 2);
            let paths = pd.discover_paths(
                Chain::Ethereum,
                &eth_usdc(),
                Chain::Base,
                &base_usdc(),
            );
            black_box(paths);
        });
    });

    c.bench_function("path_discovery_3hop", |b| {
        b.iter(|| {
            let pd = PathDiscovery::new(&registry, 3);
            let paths = pd.discover_paths(
                Chain::Ethereum,
                &eth_usdc(),
                Chain::Base,
                &base_usdc(),
            );
            black_box(paths);
        });
    });
}

fn bench_minimax_search(c: &mut Criterion) {
    let registry = build_mock_registry();

    c.bench_function("minimax_1hop", |b| {
        b.iter(|| {
            let config = RouterConfig {
                max_hops: 1,
                ..RouterConfig::default()
            };
            let mut searcher = MinimaxSearcher::new(config);
            let result = searcher.search(
                &registry,
                Chain::Ethereum,
                &eth_usdc(),
                Chain::Arbitrum,
                &arb_usdc(),
                black_box(10000.0),
            );
            black_box(result);
        });
    });

    c.bench_function("minimax_2hop", |b| {
        b.iter(|| {
            let config = RouterConfig {
                max_hops: 2,
