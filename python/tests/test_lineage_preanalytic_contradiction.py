from __future__ import annotations

import unittest

from prism_sdk import (
    ArgumentError,
    ContradictionReviewArgs,
    LabPlanRequest,
    OncoBoundaryArgs,
    OncoClassificationArgs,
    OncoIdentityJoinArgs,
    OncoOutcomeAnalyzeArgs,
    OncoResponseAssessArgs,
    OncoWorldlineViewArgs,
    LineageAuditArgs,
    PreanalyticApplyArgs,
    contradiction_review_report,
    lab_plan_report,
    lineage_audit_report,
    onco_boundary_report,
    onco_classification_report,
    onco_identity_join_report,
    onco_outcome_report,
    onco_response_report,
    onco_worldline_report,
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

    def test_response_report_keeps_post_treatment_progression_withheld(self) -> None:
        report = onco_response_report({
            "ok": True,
            "assessment": {
                "unconfirmed_reading": "progression",
                "call": {"call": "not_evaluable"},
                "hypotheses": {"entries": []},
            },
            "call_label": "not evaluable",
            "withheld_progression": True,
            "hypothesis_count": 3,
            "evidence_requests": ["histopathology", "interval_follow_up"],
            "guarantees": ["withheld"],
            "limitations": ["research"],
        })
        self.assertTrue(report.withheld_progression)
        self.assertEqual(report.call_label, "not evaluable")
        self.assertEqual(report.hypothesis_count, 3)
        with self.assertRaises(ArgumentError):
            onco_response_report({
                "ok": True,
                "assessment": {},
                "call_label": "progression",
                "withheld_progression": True,
                "hypothesis_count": 1,
                "evidence_requests": [],
                "guarantees": [],
                "limitations": [],
            })

    def test_worldline_report_reconciles_orders_and_visibility_partitions(self) -> None:
        report = onco_worldline_report({
            "ok": True,
            "schema": "bioprism-mcp/onco-worldline-view/0.1",
            "subject": "S-1",
            "baseline": "baseline",
            "timepoint_count": 2,
            "biological_order": ["baseline", "future"],
            "record_order": ["future", "baseline"],
            "record_order_differs": True,
            "clock_axes": ["acquired", "recorded", "released", "visible"],
            "clock_order_guaranteed": True,
            "baseline_biological_index": 0,
            "baseline_record_index": 1,
            "visibility_cutoff": "2026-01-10T12:00:00Z",
            "visibility_filter_applied": True,
            "visible_timepoints": ["future"],
            "hidden_from_agent": ["baseline"],
            "visibility_partition": {
                "cutoff": "2026-01-10T12:00:00Z",
                "filter_applied": True,
                "visible": ["future"],
                "hidden": ["baseline"],
                "visible_count": 1,
                "hidden_count": 1,
            },
            "visible_count": 1,
            "hidden_count": 1,
            "timepoints": [
                {
                    "label": "baseline",
                    "biological_index": 0,
                    "record_index": 1,
                    "clocks": {
                        "acquired": "2026-01-01T00:00:00Z",
                        "recorded": "2026-01-10T00:00:00Z",
                        "released": "2026-01-11T00:00:00Z",
                        "visible": "2026-01-11T00:00:00Z",
                    },
                    "acquired": "2026-01-01T00:00:00Z",
                    "recorded": "2026-01-10T00:00:00Z",
                    "released": "2026-01-11T00:00:00Z",
                    "visible": "2026-01-11T00:00:00Z",
                    "days_from_baseline": 0,
                    "observation": {"kind": "molecular"},
                    "visibility_state": "hidden_from_agent",
                    "visible_at_cutoff": False,
                },
                {
                    "label": "future",
                    "biological_index": 1,
                    "record_index": 0,
                    "clocks": {
                        "acquired": "2026-01-05T00:00:00Z",
                        "recorded": "2026-01-06T00:00:00Z",
                        "released": "2026-01-07T00:00:00Z",
                        "visible": "2026-01-07T00:00:00Z",
                    },
                    "acquired": "2026-01-05T00:00:00Z",
                    "recorded": "2026-01-06T00:00:00Z",
                    "released": "2026-01-07T00:00:00Z",
                    "visible": "2026-01-07T00:00:00Z",
                    "days_from_baseline": 4,
                    "observation": {"kind": "molecular"},
                    "visibility_state": "visible",
                    "visible_at_cutoff": True,
                },
            ],
            "guarantees": ["separate clocks"],
            "limitations": ["exact dates"],
        })
        self.assertTrue(report.record_order_differs)
        self.assertEqual(report.visible_timepoints, ("future",))
        self.assertEqual(report.timepoint_records[1].clocks.acquired, "2026-01-05T00:00:00Z")
        self.assertEqual(report.timepoint_records[1].record_index, 0)
        self.assertEqual(report.visibility_partition.visible_count, 1)
        self.assertEqual(len(report.timepoints), 2)

    def test_worldline_report_rejects_forged_clock_or_visibility_reconciliation(self) -> None:
        with self.assertRaises(ArgumentError):
            onco_worldline_report({
                "ok": True,
                "schema": "bioprism-mcp/onco-worldline-view/0.1",
                "subject": "S-1",
                "baseline": "baseline",
                "timepoint_count": 1,
                "biological_order": ["baseline"],
                "record_order": ["baseline"],
                "record_order_differs": False,
                "clock_axes": ["acquired", "recorded", "released", "visible"],
                "clock_order_guaranteed": True,
                "baseline_biological_index": 0,
                "baseline_record_index": 0,
                "visibility_cutoff": "2026-01-01T00:00:00Z",
                "visibility_filter_applied": True,
                "visible_timepoints": ["baseline"],
                "hidden_from_agent": [],
                "visible_count": 1,
                "hidden_count": 0,
                "timepoints": [{
                    "label": "baseline",
                    "biological_index": 0,
                    "record_index": 0,
                    "clocks": {
                        "acquired": "2026-01-01T00:00:00Z",
                        "recorded": "2026-01-01T00:00:00Z",
                        "released": "2026-01-01T00:00:00Z",
                        "visible": "2026-01-01T00:00:00Z",
                    },
                    "days_from_baseline": 0,
                    "observation": {},
                    "visibility_state": "hidden_from_agent",
                    "visible_at_cutoff": False,
                }],
                "guarantees": [],
                "limitations": [],
            })

    def test_classification_identity_and_outcome_reports_keep_negative_states_typed(self) -> None:
        classification = onco_classification_report({
            "ok": True,
            "schema": "bioprism-mcp/onco-classification-check/0.1",
            "histology": "diffuse_glioma",
            "resolution": {
                "resolution": "unresolved",
                "candidates": ["astrocytoma_idh_mutant", "oligodendroglioma_idh_mutant1p19q_codeleted"],
                "obligations": [{
                    "marker": "idh_mutation",
                    "role": "required",
                    "state": {"unobserved": "not_collected"},
                    "discriminates": 2,
                }],
            },
            "resolution_kind": "unresolved",
            "is_integrated": False,
            "entity": None,
            "obligations": [{
                "marker": "idh_mutation",
                "role": "required",
                "state": {"unobserved": "not_collected"},
                "discriminates": 2,
            }],
            "obligation_count": 1,
            "panel_states": [{"marker": "idh_mutation", "state": {"unobserved": "not_collected"}}],
            "panel_state_count": 1,
            "observed_panel_state_count": 0,
            "unobserved_panel_state_count": 1,
            "guarantees": ["unresolved"],
            "limitations": ["bounded criteria"],
        })
        self.assertTrue(classification.unresolved)
        self.assertEqual(len(classification.obligations), 1)
        self.assertEqual(classification.resolution_record.kind, "unresolved")
        self.assertEqual(classification.obligation_records[0].discriminates, 2)
        self.assertEqual(classification.unobserved_panel_state_count, 1)
        identity = onco_identity_join_report({
            "ok": True,
            "joinable": False,
            "report": {"verdict": {"declined": {"reason": "no_identity_evidence"}}},
            "bridge_declared": False,
            "guarantees": ["auditable"],
            "limitations": ["caller evidence"],
        })
        self.assertTrue(identity.declined)
        outcome = onco_outcome_report({
            "ok": True,
            "schema": "bioprism-mcp/onco-outcome-analyze/0.1",
            "analysis": {
                "subject": "P-1",
                "estimand": {
                    "endpoint": "time_to_progression",
                    "population": "intention_to_treat",
                    "variable": "time from entry to progression",
                    "summary_measure": "median_time_to_event",
                    "intercurrent_event_strategies": [["death", "hypothetical"], ["loss_to_follow_up", "hypothetical"]],
                    "censoring_assumption": {"censoring": "potentially_informative", "concern": "loss is prognosis-dependent"},
                },
                "at_risk_days": 10,
                "immortal_time_days": 10,
                "outcome": {"outcome": "censored", "lost_to_follow_up": None},
                "bias_flags": ["left_truncation", "informative_loss_to_follow_up"],
            },
            "outcome": {"outcome": "censored", "lost_to_follow_up": None},
            "bias_flags": ["left_truncation", "informative_loss_to_follow_up"],
            "bias_count": 2,
            "informative_bias_count": 1,
            "at_risk_days": 10,
            "immortal_time_days": 10,
            "left_truncated": True,
            "event": False,
            "censoring_reason": "lost_to_follow_up",
            "censoring_informative": True,
            "informative_bias_flags": ["informative_loss_to_follow_up"],
            "guarantees": ["censoring"],
            "limitations": ["one subject"],
        })
        self.assertTrue(outcome.left_truncated)
        self.assertEqual(outcome.censoring_reason, "lost_to_follow_up")
        self.assertEqual(outcome.analysis_record.estimand.endpoint, "time_to_progression")
        self.assertEqual(outcome.outcome_record.censoring_reason, "lost_to_follow_up")
        self.assertEqual(outcome.bias_count, 2)

        with self.assertRaises(ArgumentError):
            onco_outcome_report({
                "ok": True,
                "analysis": {
                    "subject": "P-1",
                    "estimand": {
                        "endpoint": "time_to_progression",
                        "population": "intention_to_treat",
                        "variable": "time from entry to progression",
                        "summary_measure": "median_time_to_event",
                        "intercurrent_event_strategies": [],
                        "censoring_assumption": "noninformative_assumed",
                    },
                    "at_risk_days": 10,
                    "immortal_time_days": 0,
                    "outcome": {"outcome": "censored", "lost_to_follow_up": None},
                    "bias_flags": [],
                },
                "outcome": {"outcome": "censored", "lost_to_follow_up": None},
                "bias_flags": [],
                "bias_count": 0,
                "informative_bias_count": 0,
                "at_risk_days": 10,
                "immortal_time_days": 0,
                "left_truncated": False,
                "event": False,
                "censoring_reason": "lost_to_follow_up",
                "censoring_informative": False,
                "informative_bias_flags": [],
                "guarantees": [],
                "limitations": [],
            })

    def test_new_onco_args_preserve_optional_warrants_and_bound_numbers(self) -> None:
        response = OncoResponseAssessArgs(
            {"id": "criterion"}, {"id": "baseline"}, {"id": "current"}, "2026-01-01T00:00:00Z",
            {"trend": "stable"}, {"trend": "stable"}, {"modality": "none"}, {"confirmatory": None}, 25.0, 0.1
        )
        self.assertEqual(response.to_mcp_arguments()["nadir_spd_mm2"], 25.0)
        self.assertEqual(OncoWorldlineViewArgs({"timepoints": []}, "2026-01-01T00:00:00Z").to_mcp_arguments()["visible_at"], "2026-01-01T00:00:00Z")
        self.assertEqual(OncoClassificationArgs("diffuse_glioma", {}).to_mcp_arguments()["histology"], "diffuse_glioma")
        self.assertEqual(OncoIdentityJoinArgs({}, {}, "specimen").to_mcp_arguments()["unit"], "specimen")
        self.assertEqual(OncoOutcomeAnalyzeArgs({}, {}).to_mcp_arguments(), {"follow_up": {}, "estimand": {}})
        with self.assertRaises(ArgumentError):
            OncoIdentityJoinArgs({}, {}, "sample")
        with self.assertRaises(ArgumentError):
            OncoResponseAssessArgs({}, {}, {}, "now", {}, {}, {}, measurement_error_fraction=-1)


if __name__ == "__main__":
    unittest.main()
