from __future__ import annotations

import unittest

from prism_sdk import (
    ArgumentError,
    ContradictionReviewArgs,
    LabPlanRequest,
    OncoBoundaryArgs,
    LineageAuditArgs,
    PreanalyticApplyArgs,
    contradiction_review_report,
    lab_plan_report,
    lineage_audit_report,
    onco_boundary_report,
    preanalytic_apply_report,
)


class LineageReportTests(unittest.TestCase):
    def payload(self) -> dict:
        mismatch = {
            "fingerprint": "mismatch",
            "specimen": "s1",
            "declared_donor": "donor-a",
            "fingerprint_donor": "donor-b",
        }
        return {
            "ok": True,
            "specimen_count": 3,
            "artifact_count": 1,
            "finding_count": 2,
            "clean": False,
            "identity_complete": False,
            "fingerprint_count": 3,
            "fingerprints": [mismatch, {"fingerprint": "no_evidence_available", "specimen": "s2"}],
            "omitted_fingerprints": 1,
            "unchecked_identity_count": 2,
            "unchecked_identity": ["s2"],
            "finding_count_returned": 2,
            "findings": [
                {"finding": "identity_mismatch", "specimen": "s1", "fingerprint": mismatch},
                {"finding": "mass_not_conserved", "parent": "s1", "parent_mass_ug": 10, "child_total_ug": 11},
            ],
            "omitted_findings": 0,
            "guarantees": ["typed"],
            "limitations": ["declared"],
        }

    def test_report_separates_clean_material_from_identity_completeness(self) -> None:
        report = lineage_audit_report(self.payload())
        self.assertFalse(report.clean)
        self.assertFalse(report.identity_complete)
        self.assertFalse(report.ready_for_identity_claim)
        self.assertEqual(report.findings[0].fingerprint.fingerprint_donor, "donor-b")
        self.assertEqual(report.omitted_fingerprints, 1)

    def test_report_rejects_count_or_identity_forgery(self) -> None:
        payload = self.payload()
        payload["finding_count_returned"] = 1
        with self.assertRaises(ArgumentError):
            lineage_audit_report(payload)
        payload = self.payload()
        payload["identity_complete"] = True
        with self.assertRaises(ArgumentError):
            lineage_audit_report(payload)

    def test_request_bounds_registry_and_max_items(self) -> None:
        request = LineageAuditArgs({"nodes": {}, "artifacts": {}}, 10)
        self.assertEqual(request.to_mcp_arguments()["max_items"], 10)
        with self.assertRaises(ArgumentError):
            LineageAuditArgs({"nodes": {}, "artifacts": {}}, 0)


class PreanalyticReportTests(unittest.TestCase):
    def success(self) -> dict:
        return {
            "ok": True,
            "applied": True,
            "mutation": {"id": "cold-30", "family": "cold", "edits": []},
            "stage": "collection",
            "faulted": {
                "mutation": "cold-30",
                "specimen": {"id": "sp-1"},
                "qc_signature": {"drift": -5},
                "measurability_lost": {"rna": 20},
                "stage": "collection",
            },
            "biology_digest_before": "a" * 64,
            "biology_digest_after": "a" * 64,
            "biology_unchanged": True,
            "specimen_digest_before": "b" * 64,
            "specimen_digest_after": "c" * 64,
            "has_signature": True,
            "response_check": {"ok": True},
            "family_validation": {"ok": True, "family": "cold"},
            "detectability": {"qc_field": "drift", "alert_at": 3, "intensity": 5000},
            "guarantees": ["biology"],
            "limitations": ["abstraction"],
        }

    def test_success_preserves_biology_and_signed_qc(self) -> None:
        report = preanalytic_apply_report(self.success())
        self.assertTrue(report.applied)
        self.assertTrue(report.biology_preserved)
        self.assertEqual(report.faulted.qc_signature["drift"], -5)
        self.assertEqual(report.detectability.intensity, 5000)

    def test_refusal_is_not_transport_failure(self) -> None:
        report = preanalytic_apply_report({
            "ok": False,
            "applied": False,
            "mutation": {"id": "bad"},
            "biology_digest_before": "a" * 64,
            "specimen_digest_before": "b" * 64,
            "response_check": None,
            "family_validation": None,
            "detectability": None,
            "refusal": "biological state changed",
            "fail_closed": True,
        })
        self.assertTrue(report.refused)
        self.assertTrue(report.fail_closed)
        self.assertEqual(report.refusal, "biological state changed")

    def test_success_cannot_claim_biology_unchanged_with_different_digests(self) -> None:
        payload = self.success()
        payload["biology_digest_after"] = "d" * 64
        with self.assertRaises(ArgumentError):
            preanalytic_apply_report(payload)


class ContradictionReportTests(unittest.TestCase):
    def success(self) -> dict:
        reading = {
            "modality": "imaging",
            "quantity": "marker",
            "lens": {"id": "mri", "spatial_extent": {"extent": "whole"}, "assay_scope": "macro"},
            "scope": {"specimen": "S1", "time": "T1"},
            "reported": {"reported": "value", "value": {"value": "interval", "low": 50, "high": 60}},
            "annotations": [],
        }
        right = dict(reading)
        right["modality"] = "pathology"
        return {
            "ok": True,
            "validated": True,
            "contradiction": {"left": reading, "right": right, "quantity": "marker", "overlap": {}},
            "intent": "resolvable",
            "declared_hypothesis_count": 2,
            "admissible_hypothesis_count": 2,
            "admissible_hypotheses": {
                "h1": {"discordance": "assay_scope"},
                "h2": {"discordance": "irreducible_discordance"},
            },
            "validation_intent_check": {"intent_check": "consistent"},
            "post_examination_intent_check": {"intent_check": "consistent"},
            "examined": [],
            "state": {
                "state": "not_yet_examined",
                "available": [{"evidence": "review", "refutes": ["h1"], "cost": 1}],
            },
            "state_name": "not_yet_examined",
            "live_hypothesis_count": 2,
            "next_actions": [{"evidence": "review", "refutes_live": 1, "cost": 1}],
            "omitted_next_actions": 0,
            "cue_count": 0,
            "cues": [],
            "omitted_cues": 0,
            "expectedness": {"ok": True, "value": {"expectedness": "notable", "rate_per_ten_thousand": 100}, "threshold": 2000},
            "guarantees": ["set-valued"],
            "limitations": ["declared"],
        }

    def test_not_yet_examined_is_preserved(self) -> None:
        report = contradiction_review_report(self.success())
        self.assertTrue(report.resolution_pending)
        self.assertFalse(report.unresolvable)
        self.assertEqual(report.state.available[0].evidence, "review")
        self.assertEqual(report.expectedness.kind, "notable")

    def test_pose_refusal_is_fail_closed(self) -> None:
        report = contradiction_review_report({
            "ok": False,
            "stage": "pose",
            "refusal": "readings agree",
            "fail_closed": True,
        })
        self.assertTrue(report.refused)
        self.assertEqual(report.stage, "pose")

    def test_args_reject_duplicate_hypotheses(self) -> None:
        with self.assertRaises(ArgumentError):
            ContradictionReviewArgs(
                {}, {}, "resolvable", ({"id": "h", "account": {}}, {"id": "h", "account": {}})
            )


class LabAndOncoReportTests(unittest.TestCase):
    def test_lab_report_keeps_privacy_exclusion_and_escalation(self) -> None:
        report = lab_plan_report({
            "ok": True,
            "goal": "choose a safe assay",
            "obligation_count": 1,
            "frontier": [{"id": "identity"}],
            "omitted_frontier": 0,
            "separation": None,
            "ordered": [{"action": "inspect", "kind": {"kind": "inspect_metadata"}, "targets": ["identity"], "value_per_unit_cost": 10.0, "cost": {"tokens": 1, "latency_units": 0}}],
            "omitted_ordered": 0,
            "excluded": [["private", {"excluded_because": "crosses_boundary", "policy": "no-private-db"}]],
            "omitted_excluded": 0,
            "spent": {"tokens": 1, "latency_units": 0},
            "stop": {"stopped_because": "evidence_unreachable", "outstanding": ["identity"]},
            "should_escalate": True,
            "guarantees": ["privacy"],
            "limitations": ["no execution"],
        })
        self.assertTrue(report.should_escalate)
        self.assertEqual(report.excluded[0].reason_kind, "crosses_boundary")
        self.assertFalse(report.execution_started)

    def test_lab_refusal_and_onco_partial_release_remain_structured(self) -> None:
        refused = lab_plan_report({"ok": False, "stage": "planning", "refusal": "crosses boundary", "fail_closed": True, "guarantee": "bounded"})
        self.assertTrue(refused.refused)
        onco = onco_boundary_report({
            "ok": True,
            "permitted": ["cohort_analysis", "method_development"],
            "disposition": {
                "disposition": "release_partial",
                "released": ["cohort_analysis"],
                "refused": ["treatment_recommendation"],
                "escalation": {"trigger": "individual_clinical_request", "route": "treating_clinical_team"},
            },
            "released": ["cohort_analysis"],
            "refused": ["treatment_recommendation"],
            "terminal_action": "escalate",
            "escalation": {"trigger": "individual_clinical_request", "route": "treating_clinical_team"},
            "research_statement": "Research use only.",
            "guarantees": ["split"],
            "limitations": ["declared"],
        })
        self.assertTrue(onco.ok)
        self.assertTrue(onco.refused_individual_use)
        self.assertFalse(onco.research_only)

    def test_lab_and_onco_requests_bound_inputs(self) -> None:
        self.assertEqual(LabPlanRequest({}, [], {}).to_mcp_arguments()["max_items"], 100)
        self.assertEqual(OncoBoundaryArgs({"requested_uses": ["cohort_analysis"]}).to_mcp_arguments()["request"]["requested_uses"], ["cohort_analysis"])
        with self.assertRaises(ArgumentError):
            OncoBoundaryArgs({"requested_uses": ["treatment"]})


if __name__ == "__main__":
    unittest.main()
