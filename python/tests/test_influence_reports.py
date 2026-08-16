from __future__ import annotations

import unittest

from prism_sdk import ArgumentError, InfluenceAnalysisReport, InfluenceAnalyzeArgs, influence_analysis_report


def influence_payload(*, kind: str = "bounded") -> dict:
    estimate = (
        {
            "kind": "bounded",
            "value": 0.25,
            "metric": "total_variation_on_normalised_answer",
            "method": "dynamic_range",
            "approximation": "conservative_upper_bound",
            "validity": "removal of f.a in small-region",
        }
        if kind == "bounded"
        else {"kind": "unknown", "reason": {"reason": "no_factor_table", "factor": "f.a"}}
    )
    return {
        "ok": True,
        "region": {
            "label": "small-region",
            "variables": {"a": 2},
            "free": ["a"],
            "bound": [],
            "factors": [{"id": "f.a", "scope": ["a"], "arity": 1, "has_table": kind == "bounded"}],
            "has_tables": kind == "bounded",
            "joint_entries": 2,
            "free_entries": 2,
            "assumed_cardinality_fraction": 0.0,
        },
        "execute": False,
        "analysis": {
            "subject": ["f.a"],
            "perturbation": {"class": "removal"},
            "estimate": estimate,
            "attempted": [
                {"method": "dynamic_range", "value": 0.25}
                if kind == "bounded"
                else {"method": "dynamic_range", "declined": {"reason": "no_factor_table", "factor": "f.a"}}
            ],
        },
        "looseness": None,
        "guarantees": ["unknown remains unknown"],
    }


class InfluenceReportTests(unittest.TestCase):
    def test_bounded_influence_keeps_method_metric_and_structural_posture(self) -> None:
        report = influence_analysis_report(influence_payload())
        self.assertIsInstance(report, InfluenceAnalysisReport)
        self.assertTrue(report.bounded)
        self.assertFalse(report.execute)
        self.assertEqual(report.bound_value, 0.25)
        self.assertEqual(report.method, "dynamic_range")
        self.assertFalse(report.exact)
        self.assertEqual(report.region["free"], ["a"])

    def test_unknown_influence_is_not_promoted_to_a_numeric_infinity(self) -> None:
        report = influence_analysis_report(influence_payload(kind="unknown"))
        self.assertTrue(report.unknown)
        self.assertIsNone(report.bound_value)
        self.assertEqual(report.estimate["reason"]["reason"], "no_factor_table")

    def test_influence_requests_enforce_factor_selection_and_region_bounds(self) -> None:
        request = InfluenceAnalyzeArgs(
            "small-region",
            {"a": 2},
            ({"id": "f.a", "scope": ["a"], "table": [1.0, 2.0]},),
            ("a",),
            {"class": "removal"},
            factor="f.a",
        )
        wire = request.to_mcp_arguments()
        self.assertEqual(wire["factor"], "f.a")
        self.assertFalse(wire["execute"])
        with self.assertRaises(ArgumentError):
            InfluenceAnalyzeArgs("bad", {"a": 2}, ({"id": "f.a", "scope": ["a"]},), ("a",), {"class": "removal"}, factor="f.a", factor_group=("f.a",))
        with self.assertRaises(ArgumentError):
            InfluenceAnalyzeArgs("bad", {"a": 0}, ({"id": "f.a", "scope": ["a"]},), ("a",), {"class": "removal"}, factor="f.a")
        with self.assertRaises(ArgumentError):
            influence_analysis_report({**influence_payload(), "analysis": {**influence_payload()["analysis"], "estimate": {"kind": "bounded", "value": 2, "metric": "total_variation_on_normalised_answer", "method": "dynamic_range", "approximation": "exact", "validity": "bad"}}})


if __name__ == "__main__":
    unittest.main()
