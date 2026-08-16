from __future__ import annotations

import unittest

from prism_sdk import (
    ArgumentError,
    BioethicsActionReviewArgs,
    BioethicsActionReviewReport,
    BioethicsDualUseReviewArgs,
    BioethicsRepresentationAuditReport,
    BioethicsValidationCheckReport,
    HumanSubjectScreenReport,
    RuntimeEffectCheckArgs,
    RuntimeEffectReport,
    RuntimeExecutionSimulateArgs,
    RuntimeTapeVerifyReport,
    bioethics_action_review_report,
    bioethics_dual_use_review_report,
    bioethics_representation_audit_report,
    bioethics_validation_check_report,
    human_subject_screen_report,
    runtime_effect_check_report,
    runtime_execution_simulate_report,
    runtime_tape_verify_report,
)


class RuntimeBioethicsReportTests(unittest.TestCase):
    def test_runtime_authorization_keeps_simulation_and_refusal_non_executing(self) -> None:
        args = RuntimeEffectCheckArgs(
            {"declared": ["file_read"]},
            {"kind": "file_read", "path": "/work/input.txt"},
        )
        self.assertEqual(args.to_mcp_arguments()["request"]["kind"], "file_read")
        allowed = runtime_effect_check_report({
            "ok": True,
            "request": {"kind": "file_read", "path": "/work/input.txt"},
            "kind": "file_read",
            "class": "pure",
            "class_label": "pure",
            "target_host": None,
            "target_path": "/work/input.txt",
            "authorization": "perform",
            "simulated_outcome": None,
            "guarantees": ["no effect"],
            "limitations": ["inspection only"],
        })
        self.assertIsInstance(allowed, RuntimeEffectReport)
        self.assertFalse(allowed.executed)
        self.assertFalse(allowed.simulated)
        refused = runtime_effect_check_report({
            "ok": False,
            "stage": "authorization",
            "request": {"kind": "file_read", "path": "/etc/passwd"},
            "kind": "file_read",
            "class": "pure",
            "class_label": "pure",
            "target_host": None,
            "target_path": "/etc/passwd",
            "refusal": "path denied",
            "fail_closed": True,
            "guarantee": "fail closed",
        })
        self.assertTrue(refused.refused)
        self.assertFalse(refused.executed)
        with self.assertRaises(ArgumentError):
            RuntimeEffectCheckArgs({}, {"kind": "unknown"})

    def test_runtime_tape_and_simulation_preserve_divergence_partial_runs_and_budget(self) -> None:
        tape = runtime_tape_verify_report({
            "ok": True,
            "schema": "bioprism-mcp/runtime-tape-verify/0.1",
            "run": "run-1",
            "lineage": None,
            "entries": 2,
            "head": "digest-2",
            "chain_verified": True,
            "checkpoint_results": [{
                "id": "ckpt", "step": 2, "tape_head": "digest-2", "provider": "runtime",
                "restoration": {"portable": True, "requires_provider": None, "notes": "tape"}, "ok": True,
            }],
            "checkpoint_count": 1, "checkpoint_pass_count": 1, "checkpoint_failure_count": 0,
            "artifacts": {"consumed": ["/work/in.txt"], "created": {}},
            "artifact_consumed_count": 1, "artifact_created_count": 0,
            "simulated_steps": [1],
            "simulated_step_count": 1,
            "first_divergence": 1,
            "comparison_supplied": True,
            "guarantees": ["hash chain"],
            "limitations": ["no provider"],
        })
        self.assertIsInstance(tape, RuntimeTapeVerifyReport)
        self.assertTrue(tape.diverged)
        self.assertTrue(tape.has_simulated_steps)
        self.assertEqual(len(tape.checkpoint_failures), 0)
        with self.assertRaises(ArgumentError):
            runtime_tape_verify_report({
                "ok": True,
                "schema": "bioprism-mcp/runtime-tape-verify/0.1",
                "run": "run-1", "lineage": None, "entries": 2, "head": "digest-2", "chain_verified": True,
                "checkpoint_results": [{
                    "id": "ckpt", "step": 2, "tape_head": "digest-2", "provider": "runtime",
                    "restoration": {"portable": True, "requires_provider": None, "notes": "tape"}, "ok": True,
                }],
                "checkpoint_count": 1, "checkpoint_pass_count": 2, "checkpoint_failure_count": 0,
                "artifacts": {"consumed": [], "created": {}}, "artifact_consumed_count": 0, "artifact_created_count": 0,
                "simulated_steps": [], "simulated_step_count": 0, "first_divergence": None,
                "comparison_supplied": False, "guarantees": [], "limitations": [],
            })
        args = RuntimeExecutionSimulateArgs(
            {"declared": ["clock_now"]},
            [{"kind": "clock_now"}],
            run="run-1",
            budget={"limits": {"tool_calls": {"hard": 1}}},
        )
        self.assertEqual(args.to_mcp_arguments()["requests"], [{"kind": "clock_now"}])
        simulation = runtime_execution_simulate_report({
            "ok": True,
            "run": "run-1",
            "request_count": 2,
            "recorded_requests": 1,
            "live_outcomes": [{"clock_millis": 0}],
            "execution_error": "budget exhausted for tool_calls",
            "tape": {"entries": []},
            "world": {"calls": 1, "file_changes": []},
            "policy_journal": [],
            "budget": {"aborted_on": "tool_calls", "accounting": {}},
            "replay": {"verified": True, "matched": True, "outcomes": [], "error": None},
            "fork": None,
            "guarantees": ["no host effects"],
            "limitations": ["bounded"],
        })
        self.assertTrue(simulation.partial_recording)
        self.assertTrue(simulation.budget_exhausted)
        self.assertTrue(simulation.replay_verified)
        self.assertFalse(simulation.live_effects_reachable)

    def test_bioethics_action_and_human_subject_reports_keep_gates_separate(self) -> None:
        action_args = BioethicsActionReviewArgs(
            {"subject": "study", "steps": [], "declared_use": "cohort_analysis"},
            authorisation={"human_approver": "a", "institutional_safety_review_body": "irb"},
        )
        self.assertIn("authorisation", action_args.to_mcp_arguments())
        action = bioethics_action_review_report({
            "ok": True,
            "subject": "study",
            "declared_use": "cohort_analysis",
            "permitted_uses": ["cohort_analysis"],
            "disposition": {"in_silico": [], "physical": []},
            "physical_step_count": 0,
            "in_silico_step_count": 0,
            "requires_external_authorisation": False,
            "referral": {"status": "not_required", "executes_physical_action": False},
            "guarantees": ["never executes"],
        })
        self.assertIsInstance(action, BioethicsActionReviewReport)
        self.assertFalse(action.physical_execution_reachable)
        self.assertFalse(action.referral_ready)
        human = human_subject_screen_report({
            "ok": True,
            "subject": "study",
            "determination": {"determination": "review_required", "triggers": ["expert_performance_study"]},
            "requires_institutional_review": True,
            "triggers": ["expert_performance_study"],
            "consent": {"status": "admitted", "at": "2026-01-01T00:00:00Z"},
            "return_of_results": {"status": "refused", "fail_closed": True},
            "clearance_issued": False,
            "guarantees": ["screening is not clearance"],
        })
        self.assertIsInstance(human, HumanSubjectScreenReport)
        self.assertTrue(human.review_required)
        self.assertEqual(human.consent_status, "admitted")
        self.assertFalse(human.clearance_issued)

    def test_bioethics_dual_use_validation_and_representation_refusals_are_typed(self) -> None:
        dual = bioethics_dual_use_review_report({
            "ok": False,
            "stage": "dual_use_release",
            "refusal": "no misuse-surface assessment",
            "fail_closed": True,
        })
        self.assertFalse(dual.ok)
        self.assertFalse(dual.risk_gate_reached)
        validation = bioethics_validation_check_report({
            "ok": True,
            "subject": "module",
            "author": "author",
            "maturity": "experimental",
            "missing": ["design_review"],
            "missing_count": 1,
            "verification": {"status": "refused", "refusal": "missing evidence", "fail_closed": True},
            "guarantees": ["missing is not verified"],
        })
        self.assertIsInstance(validation, BioethicsValidationCheckReport)
        self.assertFalse(validation.verified)
        representation = bioethics_representation_audit_report({
            "ok": True,
            "summary": {
                "subject": "cohort",
                "measured": [{"axis": "age_and_sex", "label": "adult"}],
                "unmeasured": [{"axis": "geography", "label": "rural"}],
                "suppressed": [{"axis": "site_resources", "label": "low"}],
            },
            "measured_count": 1,
            "unmeasured_count": 1,
            "suppressed_count": 1,
            "complete": False,
            "incomplete_axes": ["geography", "site_resources"],
            "attribution": {"status": "not_requested"},
            "guarantees": ["coverage retained"],
        })
        self.assertIsInstance(representation, BioethicsRepresentationAuditReport)
        self.assertTrue(representation.coverage_preserved)
        self.assertTrue(representation.incomplete)
        with self.assertRaises(ArgumentError):
            bioethics_representation_audit_report({
                "ok": True,
                "summary": {"measured": [], "unmeasured": [], "suppressed": []},
                "measured_count": 1,
                "unmeasured_count": 0,
                "suppressed_count": 0,
                "complete": True,
                "incomplete_axes": [],
                "attribution": {"status": "not_requested"},
                "guarantees": [],
            })


if __name__ == "__main__":
    unittest.main()
