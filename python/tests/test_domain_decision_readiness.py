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


if __name__ == "__main__":
    unittest.main()
