"""Tests for the batch analyzer."""

from __future__ import annotations

import pytest

from mnmx.batch_analyzer import BatchAnalyzer, BatchReport, PairAnalysis
from mnmx.router import MnmxRouter
from mnmx.types import RouterConfig


@pytest.fixture
def analyzer() -> BatchAnalyzer:
    router = MnmxRouter(config=RouterConfig(max_hops=1))
    return BatchAnalyzer(router)


class TestCompareStrategies:
    def test_returns_report(self, analyzer: BatchAnalyzer) -> None:
        pairs = [("ethereum", "USDC", 1000.0, "polygon", "USDC")]
        report = analyzer.compare_strategies(pairs, strategies=["minimax", "aggressive"])
        assert isinstance(report, BatchReport)
        assert report.pair_count == 1
        assert len(report.strategies_tested) == 2

    def test_multiple_pairs(self, analyzer: BatchAnalyzer) -> None:
        pairs = [
            ("ethereum", "USDC", 1000.0, "polygon", "USDC"),
            ("ethereum", "USDC", 500.0, "arbitrum", "USDC"),
        ]
        report = analyzer.compare_strategies(pairs, strategies=["minimax", "balanced"])
        assert report.pair_count == 2

    def test_all_strategies(self, analyzer: BatchAnalyzer) -> None:
        pairs = [("ethereum", "USDC", 1000.0, "polygon", "USDC")]
        report = analyzer.compare_strategies(pairs)
        assert len(report.strategies_tested) == 5

    def test_format_table(self, analyzer: BatchAnalyzer) -> None:
        pairs = [("ethereum", "USDC", 1000.0, "polygon", "USDC")]
        report = analyzer.compare_strategies(pairs, strategies=["minimax", "aggressive"])
        table_str = report.format_table()
        assert "minimax" in table_str
        assert "aggressive" in table_str


class TestAnalyzePair:
    def test_returns_analysis(self, analyzer: BatchAnalyzer) -> None:
        analysis = analyzer.analyze_pair("ethereum", "USDC", 1000.0, "polygon", "USDC")
        assert isinstance(analysis, PairAnalysis)
        assert analysis.from_chain == "ethereum"
        assert analysis.to_chain == "polygon"

    def test_has_routes_for_strategies(self, analyzer: BatchAnalyzer) -> None:
        analysis = analyzer.analyze_pair(
            "ethereum", "USDC", 1000.0, "polygon", "USDC",
            strategies=["minimax", "conservative"],
        )
        assert "minimax" in analysis.routes_by_strategy
        assert "conservative" in analysis.routes_by_strategy

    def test_best_strategy_not_none(self, analyzer: BatchAnalyzer) -> None:
        analysis = analyzer.analyze_pair("ethereum", "USDC", 1000.0, "polygon", "USDC")
        assert analysis.best_strategy is not None

    def test_score_spread_non_negative(self, analyzer: BatchAnalyzer) -> None:
        analysis = analyzer.analyze_pair("ethereum", "USDC", 1000.0, "polygon", "USDC")
        assert analysis.score_spread >= 0.0


class TestBatchReportSummary:
    def test_summary_keys(self, analyzer: BatchAnalyzer) -> None:
        pairs = [
            ("ethereum", "USDC", 1000.0, "polygon", "USDC"),
            ("ethereum", "USDC", 2000.0, "bsc", "USDC"),
        ]
        report = analyzer.compare_strategies(pairs, strategies=["minimax", "balanced"])
        summary = report.summary()
        assert "total_pairs" in summary
        assert "pairs_with_routes" in summary
        assert "strategy_wins" in summary
        assert "average_scores" in summary
        assert "average_score_spread" in summary

    def test_summary_total_pairs(self, analyzer: BatchAnalyzer) -> None:
        pairs = [("ethereum", "USDC", 1000.0, "polygon", "USDC")]
        report = analyzer.compare_strategies(pairs, strategies=["minimax"])
        summary = report.summary()
        assert summary["total_pairs"] == 1

    def test_summary_pairs_with_routes(self, analyzer: BatchAnalyzer) -> None:
        pairs = [("ethereum", "USDC", 1000.0, "polygon", "USDC")]
        report = analyzer.compare_strategies(pairs, strategies=["minimax"])
        summary = report.summary()
        assert summary["pairs_with_routes"] >= 1
