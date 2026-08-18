from __future__ import annotations

import asyncio
import unittest
from unittest.mock import patch

from prism_sdk import (
    ApiClient,
    AsyncApiClient,
    ArgumentError,
    DomainEvidenceHarmonizationReport,
    DomainEvidenceHarmonizeRequest,
    DomainEvidenceLink,
    Workspace,
)


def harmonization_payload() -> dict:
    return {
        "ok": True,
        "schema": "bioprism-devplat-domain-evidence-harmonization/0.1",
        "workflow": "domain_evidence_harmonize",
        "catalogue_digest": "a" * 64,
        "readiness_claimed": False,
        "execution": "not_started",
        "harmonization": {
            "schema": "bioprism-devplat-domain-evidence-harmonization/0.1",
            "workflow": "domain_evidence_harmonize",
            "subject_id": "subject-python",
            "claim": {"id": "claim-1"},
            "report_count": 1,
            "reports": [],
            "links": [],
            "coverage": {
                "traceability_state": "complete",
                "all_reports_linked": True,
                "bridge_summary": {
                    "report_classes": {"provider_normalization_external_payload": 1},
                    "modes": {"external_payload": 1},
                    "lineage": {
                        "parent_digest_count": 2,
                        "reports_with_lineage_parents": 1,
                        "reports_without_lineage_parents": 0,
                    },
                },
            },
            "posture": {"explicit_contradiction_declared": False},
            "readiness_claimed": False,
            "execution": "not_started",
            "harmonization_digest": "b" * 64,
        },
        "artifact_registry": {
            "indexed": True,
            "kind": "domain_evidence_harmonization",
            "content_digest": "c" * 64,
        },
    }


def request() -> DomainEvidenceHarmonizeRequest:
    return DomainEvidenceHarmonizeRequest(
        subject_id="subject-python",
        claim={"id": "claim-1", "statement": "opaque"},
        reports=({"schema": "canonical-report"},),
        links=(DomainEvidenceLink(report_index=0, role="supports"),),
        required_group_ids=("biological_domains",),
    )


class DomainEvidenceModelTests(unittest.TestCase):
    def test_request_and_response_preserve_explicit_review_posture(self) -> None:
        normalized = request()
        self.assertEqual(normalized.to_arguments()["links"], [{"report_index": 0, "role": "supports"}])
        report = DomainEvidenceHarmonizationReport.from_wire(harmonization_payload())
        self.assertEqual(report.traceability_state, "complete")
        self.assertFalse(report.contradiction_declared)
        self.assertEqual(report.harmonization_digest, "b" * 64)
        self.assertEqual(report.bridge_summary["modes"]["external_payload"], 1)
        with self.assertRaises(ArgumentError):
            DomainEvidenceLink(report_index=0, role="qualifies")
        with self.assertRaises(ArgumentError):
            DomainEvidenceHarmonizeRequest(
                subject_id="subject-python",
                claim={"id": "claim-1"},
                reports=(),
                links=(DomainEvidenceLink(report_index=0, role="context"),),
            )

    def test_sync_rest_and_tool_routes_use_typed_harmonization(self) -> None:
        with patch.object(ApiClient, "request", return_value=harmonization_payload()) as rest:
            report = ApiClient("http://127.0.0.1:8787").domain_evidence_harmonize(request())
        self.assertEqual(report.artifact_registry["kind"], "domain_evidence_harmonization")
        self.assertEqual(rest.call_args.args[:2], ("POST", "/v1/domain-evidence/harmonize"))

        with patch.object(ApiClient, "call_tool", return_value=harmonization_payload()) as tool:
            report = ApiClient("http://127.0.0.1:8787").domain_evidence_harmonize_tool(request())
        self.assertEqual(report.harmonization_digest, "b" * 64)
        self.assertEqual(tool.call_args.args[0], "domain_evidence_harmonize")

    def test_async_and_workspace_helpers_share_wire_contract(self) -> None:
        with patch.object(ApiClient, "request", return_value=harmonization_payload()):
            report = asyncio.run(
                AsyncApiClient(ApiClient("http://127.0.0.1:8787")).domain_evidence_harmonize(request())
            )
        self.assertEqual(report.traceability_state, "complete")
        with patch.object(Workspace, "tool", return_value=harmonization_payload()):
            report = Workspace(None).domain_evidence_harmonize_report(request())
        self.assertEqual(report.artifact_registry["content_digest"], "c" * 64)


if __name__ == "__main__":
    unittest.main()
