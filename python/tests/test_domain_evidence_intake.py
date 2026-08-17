from __future__ import annotations

import asyncio
import unittest
from unittest.mock import patch

from prism_sdk import (
    ApiClient,
    AsyncApiClient,
    ArgumentError,
    DomainEvidenceIntakeReport,
    DomainEvidenceIntakeRequest,
    Workspace,
)


def intake_payload() -> dict:
    return {
        "ok": True,
        "schema": "bioprism-devplat-domain-evidence-intake/0.1",
        "workflow": "domain_evidence_intake",
        "group_id": "biological_domains",
        "domains": ["modalities"],
        "subject_id": "subject-python",
        "source_tool": "modality_catalog",
        "request_supplied": True,
        "request_digest": "a" * 64,
        "response_digest": "b" * 64,
        "intake_digest": "c" * 64,
        "outcome": "observed",
        "parent_digests": [],
        "report": {"schema": "bioprism-devplat-domain-report/0.1", "group_id": "biological_domains"},
        "intake": {"response": {"status": "bounded"}},
        "artifact_registry": {
            "indexed": True,
            "kind": "domain_evidence_intake",
            "content_digest": "d" * 64,
        },
        "catalogue_digest": "e" * 64,
        "readiness_claimed": False,
        "execution": "not_started",
    }


def request() -> DomainEvidenceIntakeRequest:
    return DomainEvidenceIntakeRequest(
        group_id="biological_domains",
        domains=("modalities",),
        subject_id="subject-python",
        source_tool="modality_catalog",
        request={"modality": "single_cell"},
        response={"status": "bounded"},
        outcome="observed",
        claim_posture={"status": "observed", "does_not_claim": ["truth"]},
    )


class DomainEvidenceIntakeModelTests(unittest.TestCase):
    def test_request_and_response_keep_request_presence_and_outcome(self) -> None:
        normalized = request()
        self.assertTrue(normalized.request_supplied)
        self.assertIn("request", normalized.to_arguments())
        report = DomainEvidenceIntakeReport.from_wire(intake_payload())
        self.assertEqual(report.outcome, "observed")
        self.assertEqual(report.artifact_registry["kind"], "domain_evidence_intake")
        with self.assertRaises(ArgumentError):
            DomainEvidenceIntakeRequest(
                group_id="biological_domains",
                domains=("modalities",),
                subject_id="subject-python",
                source_tool="modality_catalog",
                response={},
                outcome="success",
                claim_posture={"status": "observed", "does_not_claim": ["truth"]},
            )

    def test_sync_rest_and_tool_helpers(self) -> None:
        with patch.object(ApiClient, "request", return_value=intake_payload()) as rest:
            report = ApiClient("http://127.0.0.1:8787").domain_evidence_intake(request())
        self.assertEqual(report.intake["response"]["status"], "bounded")
        self.assertEqual(rest.call_args.args[:2], ("POST", "/v1/domain-evidence/intake"))
        with patch.object(ApiClient, "call_tool", return_value=intake_payload()) as tool:
            report = ApiClient("http://127.0.0.1:8787").domain_evidence_intake_tool(request())
        self.assertEqual(report.intake_digest, "c" * 64)
        self.assertEqual(tool.call_args.args[0], "domain_evidence_intake")

    def test_async_and_workspace_helpers_share_wire_shape(self) -> None:
        with patch.object(ApiClient, "request", return_value=intake_payload()):
            report = asyncio.run(
                AsyncApiClient(ApiClient("http://127.0.0.1:8787")).domain_evidence_intake(request())
            )
        self.assertEqual(report.subject_id, "subject-python")
        with patch.object(Workspace, "tool", return_value=intake_payload()):
            report = Workspace(None).domain_evidence_intake_report(request())
        self.assertEqual(report.response_digest, "b" * 64)


if __name__ == "__main__":
    unittest.main()
