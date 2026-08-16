from __future__ import annotations

import unittest

from prism_sdk import (
    ArgumentError,
    BioevalReferenceAuditReport,
    EvaluationReproductionReport,
    EvaluationTrajectoryReport,
    EvaluationWorldlineReport,
    OracleCombineReport,
    OracleMissingnessReport,
    OracleReferencePanelReport,
    bioeval_reference_audit_report,
    evaluation_reproduction_check_report,
    evaluation_trajectory_check_report,
    evaluation_worldline_audit_report,
    oracle_combine_report,
    oracle_missingness_report,
    oracle_reference_panel_report,
)


class EvaluationReportTests(unittest.TestCase):
    def test_oracle_combine_keeps_undertermination_and_ledgers_visible(self) -> None:
        report = oracle_combine_report({
            "ok": True,
            "subject": "artifact-1",
            "at": "2026-08-14T00:00:00Z",
            "status": "underdetermined",
            "underdetermined": True,
            "deciding_tier": "deterministic",
            "judge_only": False,
            "suppressed_override": True,
            "acceptable": False,
            "basis": {"basis": "disagreement"},
            "confidence": {"low": 0.5, "high": 0.9},
            "establishes": ["identity"],
            "does_not_establish": ["biology"],
            "contributing": [{"oracle": "a"}],
            "omitted_contributing": 0,
            "withheld": [{"oracle": "b"}],
            "omitted_withheld": 1,
            "inadmissible": [],
            "omitted_inadmissible": 0,
            "suppressed": [{"oracle": "c"}],
            "omitted_suppressed": 0,
            "disagreements": [{"positions": ["supported", "contradicted"]}],
            "omitted_disagreements": 0,
            "guarantees": ["set-valued"],
            "limitations": ["caller supplied"],
        })
        self.assertTrue(report.underdetermined)
        self.assertTrue(report.release_blocked)
        self.assertEqual(report.omitted_withheld, 1)
        self.assertEqual(report.contributing_records[0].oracle.id, "a")
        self.assertEqual(report.suppressed_records[0].oracle.id, "c")
        self.assertEqual(report.basis_record.kind, "disagreement")

    def test_reference_panel_refusal_and_missingness_resolution_are_not_errors(self) -> None:
        refusal = oracle_reference_panel_report({
            "ok": False,
            "stage": "reference",
            "refusal": "empty independent panel",
            "fail_closed": True,
            "guarantee": "no default reference",
        })
        self.assertFalse(refusal.ok)
        self.assertTrue(refusal.unresolved)
        missingness = oracle_missingness_report({
            "ok": True,
            "groups": [["site-a", 0, 10]],
            "informativeness": {"determination": "contradicted"},
            "field": {"kind": "individual", "name": "genomics"},
            "boundary": {"kind": "aggregate_only", "policy": "site"},
            "small_cell_floor": 5,
            "egress": {"determination": "contradicted"},
            "mechanism": {"kind": "depends_on_unobserved"},
            "complete_case": {"determination": "contradicted"},
            "guarantees": ["no imputation"],
            "limitations": ["caller policy"],
        })
        self.assertTrue(missingness.complete_case_resolved)
        self.assertEqual(missingness.small_cell_floor, 5)

    def test_reference_truth_preserves_distribution_metrics(self) -> None:
        report = bioeval_reference_audit_report({
            "ok": True,
            "reference": {"standard": "distribution", "mass": {"progression": 0.6, "stable": 0.4}},
            "reference_kind": "distribution",
            "can_certify_clean_pass": False,
            "resolution": {"resolution": "distributed"},
            "modal_state": "progression",
            "modal_mass": 0.6,
            "modal_confidence": 0.6,
            "entropy_bits": 0.97,
            "dispersion": "mixed",
            "queried_state": "progression",
            "queried_state_mass": 0.6,
            "guarantees": ["mass normalized"],
            "limitations": ["not a scorer"],
        })
        self.assertIsInstance(report, BioevalReferenceAuditReport)
        self.assertEqual(report.modal_state, "progression")
        self.assertAlmostEqual(report.queried_state_mass, 0.6)
        self.assertTrue(report.is_distributed)
        self.assertEqual(report.reference_record.mass["progression"], 0.6)
        self.assertEqual(report.dispersion_record.kind, "mixed")
        self.assertFalse(report.reference_is_actionable)

    def test_evaluation_reports_reconcile_leakage_reproduction_and_trajectory(self) -> None:
        worldline = evaluation_worldline_audit_report({
            "ok": True,
            "decisions": 1,
            "leak_count": 1,
            "leaks": [{"decision": "d1", "observation": "future", "clock": "accessible", "decision_at": "2026-01-01T00:00:00Z", "available_at": "2026-01-02T00:00:00Z"}],
            "dangling_count": 1,
            "dangling_references": [["d1", "missing"]],
            "admissible_at": ["early"],
            "guarantees": ["accessibility clock"],
            "limitations": ["no denominator"],
        })
        self.assertTrue(worldline.leakage_detected)
        self.assertTrue(worldline.dangling_context_detected)
        self.assertTrue(worldline.accessibility_leakage_is_separate)
        self.assertTrue(worldline.admissibility_cut_is_explicit)
        self.assertEqual(worldline.leak_records[0].available_at, "2026-01-02T00:00:00Z")
        reproduction = evaluation_reproduction_check_report({
            "ok": True,
            "schema": "bioprism-mcp/evaluation-reproduction-check/0.1",
            "certificate": {
                "workflow": "w1",
                "environment_pinned": True,
                "verdicts": [["score", {"verdict": "diverged", "detail": "delta exceeds tolerance"}]],
            },
            "verdicts": [{"output": "score", "verdict": "diverged", "detail": "delta exceeds tolerance"}],
            "verdict_count": 1,
            "matched_count": 0,
            "diverged_count": 1,
            "missing_count": 0,
            "reproduced": False,
            "first_divergence": {"output": "score", "verdict": {"verdict": "diverged", "detail": "delta exceeds tolerance"}},
            "missing_outputs": [],
            "portability_demonstrated": False,
            "validity_claim": {"ok": False, "refusal": "not biological validity", "fail_closed": True},
            "guarantees": ["separate validity"],
            "limitations": ["no execution"],
        })
        self.assertIsInstance(reproduction, EvaluationReproductionReport)
        self.assertFalse(reproduction.reproduced_and_portable)
        self.assertTrue(reproduction.has_divergence)
        self.assertTrue(reproduction.validity_is_separate)
        self.assertEqual(reproduction.first_divergence_record.output, "score")
        trajectory = evaluation_trajectory_check_report({
            "ok": True,
            "steps": 2,
            "acts": ["edit", "verify"],
            "properties": [{"kind": "preceded_by"}],
            "property_outcomes": [{"held": False, "violations": [0]}],
            "recovery": [{"failure": 0, "changed_to": 1}],
            "bounded_suffix": {"complete": True, "value": {"downstream": 1.0}},
            "guarantees": ["nonvacuous"],
            "limitations": ["declared path"],
        })
        self.assertIsInstance(trajectory, EvaluationTrajectoryReport)
        self.assertTrue(trajectory.bounded_suffix_complete)

    def test_evaluation_parsers_reject_forged_state_reconciliations(self) -> None:
        with self.assertRaises(ArgumentError):
            oracle_combine_report({
                "ok": True, "subject": "s", "at": "2026-01-01T00:00:00Z", "status": "valid",
                "underdetermined": True, "deciding_tier": "judge", "judge_only": False,
                "suppressed_override": False, "acceptable": True, "basis": None, "confidence": 1,
                "establishes": [], "does_not_establish": [], "contributing": [], "omitted_contributing": 0,
                "withheld": [], "omitted_withheld": 0, "inadmissible": [], "omitted_inadmissible": 0,
                "suppressed": [], "omitted_suppressed": 0, "disagreements": [], "omitted_disagreements": 0,
                "guarantees": [], "limitations": [],
            })
        with self.assertRaises(ArgumentError):
            evaluation_worldline_audit_report({
                "ok": True, "decisions": 1, "leak_count": 1, "leaks": [], "dangling_count": 0,
                "dangling_references": [], "admissible_at": None, "guarantees": [], "limitations": [],
            })
        refused = evaluation_reproduction_check_report({
            "ok": False, "stage": "certification", "refusal": "empty", "fail_closed": True,
        })
        self.assertTrue(refused.fail_closed)
        valid_reproduction = {
            "ok": True,
            "schema": "bioprism-mcp/evaluation-reproduction-check/0.1",
            "certificate": {
                "workflow": "w1",
                "environment_pinned": True,
                "verdicts": [["score", {"verdict": "diverged", "detail": "delta exceeds tolerance"}]],
            },
            "verdicts": [{"output": "score", "verdict": "diverged", "detail": "delta exceeds tolerance"}],
            "verdict_count": 1,
            "matched_count": 0,
            "diverged_count": 1,
            "missing_count": 0,
            "reproduced": False,
            "first_divergence": {"output": "score", "verdict": {"verdict": "diverged", "detail": "delta exceeds tolerance"}},
            "missing_outputs": [],
            "portability_demonstrated": False,
            "validity_claim": {"ok": False, "refusal": "not biological validity", "fail_closed": True},
            "guarantees": [],
            "limitations": [],
        }
        for mutation in (
            {"matched_count": 1},
            {"missing_outputs": ["score"]},
            {"first_divergence": {"output": "other", "verdict": {"verdict": "diverged", "detail": "delta exceeds tolerance"}}},
            {"validity_claim": {"ok": False, "refusal": "not biological validity", "fail_closed": False}},
        ):
            with self.assertRaises(ArgumentError):
                evaluation_reproduction_check_report({**valid_reproduction, **mutation})
        no_decision = oracle_combine_report({
            "ok": True, "subject": "s", "at": "2026-01-01T00:00:00Z", "status": "underdetermined",
            "underdetermined": True, "deciding_tier": None, "judge_only": False,
            "suppressed_override": False, "acceptable": False, "basis": {"basis": "no_admissible_oracle"},
            "confidence": None, "establishes": [], "does_not_establish": [], "contributing": [],
            "omitted_contributing": 0, "withheld": [], "omitted_withheld": 0, "inadmissible": [],
            "omitted_inadmissible": 0, "suppressed": [], "omitted_suppressed": 0, "disagreements": [],
            "omitted_disagreements": 0, "guarantees": [], "limitations": [],
        })
        self.assertIsNone(no_decision.deciding_tier)
        with self.assertRaises(ArgumentError):
            evaluation_trajectory_check_report({
                "ok": True, "steps": 0, "acts": [], "properties": [], "property_outcomes": [], "recovery": [],
                "bounded_suffix": {"ok": False, "refusal": "out of range", "fail_closed": False},
                "guarantees": [], "limitations": [],
            })


if __name__ == "__main__":
    unittest.main()
