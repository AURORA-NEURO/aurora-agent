from __future__ import annotations

import unittest

from prism_sdk import (
    ArgumentError,
    DelegatedCheckEvidenceArgs,
    ExecutionProvenanceReport,
    ExecutionProvenanceRequest,
    execution_provenance_report,
)


def payload() -> dict:
    return {
        "ok": True,
        "workflow": "execution_provenance_audit",
        "schema": "bioprism-devplat-execution-provenance/0.1",
        "valid": True,
        "provenance_ready": True,
        "mission_id": "mission-1",
        "plan_digest": "a" * 64,
        "trace_digest": "b" * 64,
        "provenance_digest": "c" * 64,
        "mission_execution": "executed",
        "mission_status": "succeeded",
        "planned_step_count": 1,
        "result_count": 1,
        "trace_event_count": 3,
        "delegated_check_count": 1,
        "succeeded_step_count": 1,
        "refused_step_count": 0,
        "blocked_step_count": 0,
        "cancelled_step_count": 0,
        "required_failure_count": 0,
        "required_check_count": 1,
        "passed_check_count": 1,
        "nonpassing_required_checks": [],
        "missing_step_results": [],
        "unknown_step_results": [],
        "duplicate_trace_sequences": [],
        "trace_identity_errors": [],
        "complete": True,
        "structurally_valid": True,
        "release_candidate": True,
        "execution": "evidence_supplied_not_executed_here",
        "verification": "structural_only",
        "findings": [],
        "guarantees": [],
        "limitations": [],
    }


class ExecutionProvenanceTests(unittest.TestCase):
    def test_request_serializes_typed_delegated_checks(self) -> None:
        request = ExecutionProvenanceRequest(
            {"plan": {"mission_id": "mission-1"}},
            [
                DelegatedCheckEvidenceArgs(
                    "unit_tests",
                    "test",
                    True,
                    "passed",
                    "d" * 64,
                    "caller_attested",
                    trace_sequence=2,
                )
            ],
        )
        self.assertEqual(request.to_mcp_arguments()["delegated_checks"][0]["trace_sequence"], 2)

    def test_report_preserves_ready_structural_provenance(self) -> None:
        report = ExecutionProvenanceReport.from_wire(payload())
        self.assertTrue(report.provenance_ready)
        self.assertTrue(report.release_candidate)
        self.assertEqual(report.blocking_findings, ())
        self.assertEqual(report.trace_event_count, 3)

    def test_http_envelope_and_invalid_digest_are_distinct(self) -> None:
        report = execution_provenance_report(
            {"ok": True, "mcp": {"result": {"structuredContent": payload()}}}
        )
        self.assertEqual(report.mission_id, "mission-1")
        invalid = payload()
        invalid["trace_digest"] = "not-a-digest"
        with self.assertRaises(ArgumentError):
            ExecutionProvenanceReport.from_wire(invalid)


if __name__ == "__main__":
    unittest.main()
