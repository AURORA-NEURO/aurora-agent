from __future__ import annotations

import unittest

from prism_sdk import (
    ArgumentError,
    StressProfileArgs,
    StressProfileReport,
    StressReportArgs,
    StressReportProjection,
    stress_profile_report,
    stress_report_projection,
)


def profile_payload(*, identifiability: str = "separable", defects: list[dict] | None = None) -> dict:
    return {
        "family": "batch_effect",
        "blueprint_module": "32.06",
        "stress_id": "site-offset",
        "cohort_id": "cohort-1",
        "parent_digest": "sha256:parent",
        "identifiability": {"identifiability": identifiability, "batch": "site-a", "overlap": 0.5}
        if identifiability == "separable"
        else {"identifiability": identifiability, "batch": "site-a", "only": "positive"},
        "sweep": [
            {"magnitude": 125, "effective_n": 4.0, "nominal_n": 4, "unresolved": 0, "analysable_prevalence": 0.5, "abandoned": False},
            {"magnitude": 1000, "effective_n": 3.8, "nominal_n": 4, "unresolved": 1, "analysable_prevalence": 0.33, "abandoned": False},
        ],
        "findings": [
            {
                "conclusion_id": "marker_ranking",
                "character": "discriminative",
                "obligation": "probed",
                "relation": "order unchanged",
                "rationale": "stress tests rank stability",
                "held_through": 125,
                "broke_at": 1000,
                "expected_at_break": "order unchanged",
                "observed_at_break": "order changed",
            }
        ],
        "generator_defects": defects or [],
        "caveat": "upper bound on this declared ladder",
    }


class StressReportTests(unittest.TestCase):
    def test_profile_preserves_breaking_point_and_generator_posture(self) -> None:
        report = stress_profile_report({
            "ok": True,
            "headline": "batch effect against cohort-1: first failure",
            "profile": profile_payload(),
            "guarantees": ["defects are not scored"],
            "limitations": ["finite ladder"],
        })
        self.assertIsInstance(report, StressProfileReport)
        self.assertTrue(report.informative)
        self.assertTrue(report.generator_sound)
        self.assertEqual(report.sweep[-1]["unresolved"], 1)
        self.assertEqual(report.findings[0]["broke_at"], 1000)

        confounded = stress_profile_report({
            "ok": True,
            "headline": "not identifiable",
            "profile": profile_payload(identifiability="confounded"),
            "guarantees": [],
            "limitations": [],
        })
        self.assertFalse(confounded.informative)

    def test_report_keeps_guarded_worst_family_separate(self) -> None:
        body = profile_payload()
        report = stress_report_projection({
            "ok": True,
            "headline": "family summaries",
            "report": {"cohort_id": "cohort-1", "profiles": [body]},
            "worst_family": body,
            "guarantees": [],
            "limitations": [],
        })
        self.assertIsInstance(report, StressReportProjection)
        self.assertTrue(report.comparable)
        self.assertEqual(report.cohort_id, "cohort-1")

        refused = stress_report_projection({
            "ok": False,
            "stage": "stress_report",
            "refusal": "partial stress program",
            "fail_closed": True,
            "guarantee": "no partial report",
        })
        self.assertFalse(refused.ok)
        self.assertTrue(refused.fail_closed)
        self.assertFalse(refused.comparable)

    def test_stress_requests_bound_programs_and_procedures(self) -> None:
        profile = StressProfileArgs({"id": "cohort"}, {"id": "stress"}, ({"procedure": "marker_ranking"},))
        self.assertEqual(profile.to_mcp_arguments()["procedures"][0]["procedure"], "marker_ranking")
        program = StressReportArgs({"id": "cohort"}, ({"id": "stress"},))
        self.assertEqual(program.to_mcp_arguments()["stresses"][0]["id"], "stress")
        with self.assertRaises(ArgumentError):
            StressReportArgs({"id": "cohort"}, tuple({"id": str(i)} for i in range(101)))
        with self.assertRaises(ArgumentError):
            stress_profile_report({"ok": True, "headline": "bad", "profile": {"family": "unknown"}})


if __name__ == "__main__":
    unittest.main()
