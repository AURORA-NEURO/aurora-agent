from __future__ import annotations

import asyncio
import unittest
from unittest.mock import patch

from prism_sdk import (
    ApiClient,
    AsyncApiClient,
    ArgumentError,
    DomainDecisionReadinessReport,
    DomainDecisionReadinessRequest,
    DomainDecisionReadinessQueryReport,
    DomainDecisionReadinessQueryRequest,
    DomainWorkflowReconciliationQueryRequest,
    Workspace,
)


def readiness_payload() -> dict:
    return {
        "ok": True,
        "schema": "bioprism-devplat-domain-decision-readiness/0.1",
        "workflow": "domain_decision_readiness_audit",
        "catalogue_digest": "a" * 64,
        "readiness_claimed": False,
        "execution": "not_started",
        "audit": {
            "schema": "bioprism-devplat-domain-decision-readiness/0.1",
            "workflow": "domain_decision_readiness_audit",
            "decision_state": "ready_for_human_review",
            "policy_satisfied": True,
            "counts": {"reports": 2, "supporting_reports": 2, "qualifying_reports": 1},
            "blockers": [],
            "digest": "b" * 64,
        },
        "artifact_registry": {
            "indexed": True,
            "kind": "domain_decision_readiness",
            "content_digest": "c" * 64,
        },
    }


def request() -> DomainDecisionReadinessRequest:
    return DomainDecisionReadinessRequest(
        subject_id="subject-python",
        claim={"id": "claim-1", "statement": "opaque"},
        reports=({"schema": "canonical-report"},),
        links=({"report_index": 0, "role": "supports"},),
        policy={"required_group_ids": ["biological_domains"]},
    )


def readiness_query_payload() -> dict:
    return {
        "ok": True,
        "schema": "bioprism-devplat-artifact-domain-decision-readiness-query/0.1",
        "workflow": "artifact_registry_domain_decision_readiness_query",
        "filters": {"subject_id": "subject-python", "decision_state": "ready_for_human_review"},
        "registry_generation": 2,
        "registry_size": 1,
        "rows": [{"content_digest": "c" * 64, "audit_digest": "b" * 64, "decision_state": "ready_for_human_review", "policy_satisfied": True}],
        "next_after": None,
        "has_more": False,
        "execution": "not_started",
    }


class DomainDecisionReadinessTests(unittest.TestCase):
    def test_request_and_report_preserve_structural_state_without_readiness_claim(self) -> None:
        normalized = request()
        self.assertEqual(normalized.to_arguments()["policy"]["required_group_ids"], ["biological_domains"])
        report = DomainDecisionReadinessReport.from_wire(readiness_payload())
        self.assertTrue(report.is_ready_for_human_review)
        self.assertTrue(report.policy_satisfied)
        self.assertEqual(report.audit_digest, "b" * 64)
        with self.assertRaises(ArgumentError):
            DomainDecisionReadinessRequest(
                subject_id="subject-python",
                claim={"id": "claim-1"},
                reports=(),
                links=({"report_index": 0, "role": "supports"},),
                policy={},
            )
        tampered = readiness_payload()
        tampered["readiness_claimed"] = True
        with self.assertRaises(ArgumentError):
            DomainDecisionReadinessReport.from_wire(tampered)

    def test_sync_tool_and_workspace_helpers_preserve_the_wire_contract(self) -> None:
        with patch.object(ApiClient, "call_tool", return_value=readiness_payload()) as tool:
            report = ApiClient("http://127.0.0.1:8787").domain_decision_readiness_audit(request())
        self.assertEqual(tool.call_args.args[0], "domain_decision_readiness_audit")
        self.assertEqual(report.artifact_registry["kind"], "domain_decision_readiness")

        with patch.object(Workspace, "tool", return_value=readiness_payload()) as workspace_tool:
            workspace_report = Workspace(None).domain_decision_readiness_audit_report(request())
        self.assertEqual(workspace_tool.call_args.args[0], "domain_decision_readiness_audit")
        self.assertEqual(workspace_report.decision_state, "ready_for_human_review")

        with patch.object(ApiClient, "call_tool", return_value=readiness_payload()):
            async_report = asyncio.run(
                AsyncApiClient(ApiClient("http://127.0.0.1:8787")).domain_decision_readiness_audit(request())
            )
        self.assertTrue(async_report.is_ready_for_human_review)

    def test_retained_query_preserves_exact_filters_and_cursor_posture(self) -> None:
        query = DomainDecisionReadinessQueryRequest(
            subject_id="subject-python",
            decision_state="ready_for_human_review",
            policy_satisfied=True,
            max_items=10,
            include_audits=True,
        )
        self.assertEqual(query.to_query_params()["policy_satisfied"], "true")
        report = DomainDecisionReadinessQueryReport.from_wire(readiness_query_payload())
        self.assertEqual(report.rows[0]["audit_digest"], "b" * 64)
        with patch.object(ApiClient, "request", return_value=readiness_query_payload()) as request_mock:
            queried = ApiClient("http://127.0.0.1:8787").domain_decision_readiness_query(query)
        self.assertEqual(queried.rows[0]["decision_state"], "ready_for_human_review")
        self.assertIn("/v1/domain-decision-readiness?", request_mock.call_args.args[1])

    def test_reconciliation_query_carries_readiness_filters_to_mcp_and_rest(self) -> None:
        normalized = DomainWorkflowReconciliationQueryRequest(
            decision_readiness_state="ready_for_human_review",
            decision_readiness_gate_satisfied=True,
        )
        self.assertEqual(
            normalized.to_arguments()["decision_readiness_state"],
            "ready_for_human_review",
        )
        self.assertTrue(normalized.to_arguments()["decision_readiness_gate_satisfied"])
        self.assertEqual(
            normalized.to_query_params()["decision_readiness_gate_satisfied"],
            "true",
        )


if __name__ == "__main__":
    unittest.main()
