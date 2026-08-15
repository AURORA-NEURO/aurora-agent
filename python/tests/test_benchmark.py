from __future__ import annotations

import unittest

from prism_sdk import (
    AnalyticsDirection,
    AnalyticsEvidence,
    BenchmarkObservation,
    PairedBenchmarkObservation,
    ResamplingUnit,
    bootstrap_mean,
    paired_effect,
    summarize_distribution,
)
from prism_sdk.errors import ArgumentError


def row(identifier: str, value: float | None, evidence: AnalyticsEvidence | str, group: str | None = None) -> BenchmarkObservation:
    return BenchmarkObservation(identifier, "oncology", "verification", "agent-a", value, evidence, group)


class BenchmarkStatisticsTests(unittest.TestCase):
    def test_summary_separates_measured_and_unmeasured_evidence(self) -> None:
        report = summarize_distribution(
            [
                row("a", 1, AnalyticsEvidence.OBSERVED, "world-1"),
                row("b", 2, AnalyticsEvidence.REPRODUCED, "world-1"),
                row("c", 3, AnalyticsEvidence.OBSERVED, "world-2"),
                row("declared", 100, AnalyticsEvidence.DECLARED),
                row("missing", None, AnalyticsEvidence.MISSING),
                row("blocked", None, AnalyticsEvidence.BLOCKED),
            ],
            quantiles=(0.25, 0.5, 0.75),
        )
        self.assertEqual(report.total_count, 6)
        self.assertEqual(report.measured_count, 3)
        self.assertEqual(report.excluded_count, 3)
        self.assertEqual(report.declared_count, 1)
        self.assertEqual(report.missing_count, 1)
        self.assertEqual(report.blocked_count, 1)
        self.assertEqual(report.mean, 2.0)
        self.assertEqual(report.variance_sample, 1.0)
        self.assertEqual(report.interquartile_range, 1.0)
        self.assertEqual(report.quantiles["0.500000"], 2.0)
        self.assertEqual(len(report.values_digest), 64)

    def test_empty_and_invalid_series_are_explicit(self) -> None:
        empty = summarize_distribution([])
        self.assertIsNone(empty.mean)
        self.assertIsNone(empty.interquartile_range)
        with self.assertRaises(ArgumentError):
            row("bad", float("nan"), AnalyticsEvidence.OBSERVED)
        with self.assertRaises(ArgumentError):
            row("bad", None, AnalyticsEvidence.OBSERVED)
        with self.assertRaises(ArgumentError):
            summarize_distribution([], quantiles=(0.5, 0.5))

    def test_bootstrap_is_deterministic_and_cluster_aware(self) -> None:
        values = [1.0, 2.0, 10.0]
        first = bootstrap_mean(
            values,
            groups=["a", "a", "b"],
            resamples=200,
            seed=42,
            resampling_unit=ResamplingUnit.REPLICATE_GROUP,
        )
        second = bootstrap_mean(
            values,
            groups=["a", "a", "b"],
            resamples=200,
            seed=42,
            resampling_unit=ResamplingUnit.REPLICATE_GROUP,
        )
        self.assertEqual(first.to_dict(), second.to_dict())
        self.assertEqual(first.cluster_count, 2)
        self.assertEqual(first.estimate, sum(values) / 3)
        with self.assertRaises(ArgumentError):
            bootstrap_mean(values, resamples=200, resampling_unit=ResamplingUnit.REPLICATE_GROUP)
        with self.assertRaises(ArgumentError):
            bootstrap_mean(values, resamples=99)

    def test_summary_bootstrap_preserves_declared_replicate_groups(self) -> None:
        report = summarize_distribution(
            [
                row("a", 1, "observed", "parent-a"),
                row("b", 2, "observed", "parent-a"),
                row("c", 8, "observed", "parent-b"),
            ],
            bootstrap_resamples=150,
            bootstrap_seed=7,
            resampling_unit="replicate_group",
        )
        self.assertEqual(report.bootstrap.cluster_count, 2)
        self.assertEqual(report.bootstrap.resampling_unit, ResamplingUnit.REPLICATE_GROUP)

    def test_paired_effect_orients_lower_is_better_and_respects_tolerance(self) -> None:
        report = paired_effect(
            [
                PairedBenchmarkObservation("a", "ops", "latency", 10, 8, AnalyticsDirection.LOWER_IS_BETTER, 0.5),
                PairedBenchmarkObservation("b", "ops", "latency", 10, 10.2, AnalyticsDirection.LOWER_IS_BETTER, 0.5),
                PairedBenchmarkObservation("c", "ops", "latency", 10, 12, AnalyticsDirection.LOWER_IS_BETTER, 0.5),
                PairedBenchmarkObservation("missing", "ops", "latency", None, None, AnalyticsDirection.LOWER_IS_BETTER, 0.5, "missing"),
            ]
        )
        self.assertEqual(report.measured_count, 3)
        self.assertEqual(report.improved_count, 1)
        self.assertEqual(report.degraded_count, 1)
        self.assertEqual(report.within_tolerance_count, 1)
        self.assertAlmostEqual(report.retention, 1 / 3)
        self.assertEqual(report.direction, AnalyticsDirection.LOWER_IS_BETTER.value)

    def test_paired_effect_rejects_mixed_directions(self) -> None:
        with self.assertRaises(ArgumentError):
            paired_effect(
                [
                    PairedBenchmarkObservation("a", "d", "x", 1, 2, "higher_is_better", 0),
                    PairedBenchmarkObservation("b", "d", "x", 1, 2, "lower_is_better", 0),
                ]
            )


if __name__ == "__main__":
    unittest.main()
