from __future__ import annotations

import asyncio
import unittest
from unittest.mock import patch

from prism_sdk import (
    ApiClient,
    AsyncApiClient,
    ArgumentError,
    DomainEvidenceHarmonizationCoverageReport,
    DomainEvidenceHarmonizationCoverageRequest,
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


def coverage_payload() -> dict:
    return {
        "ok": True,
        "schema": "bioprism-devplat-domain-evidence-harmonization-coverage/0.1",
        "workflow": "domain_evidence_harmonization_coverage",
        "filters": {
            "subject_id": "subject-python",
            "domain": "modalities",
            "max_items": 7,
            "include_report_digests": True,
        },
        "registry_size": 4,
        "matching_count": 1,
        "returned_count": 1,
        "has_more": False,
        "next_after": None,
        "rows": [
            {
                "content_digest": "d" * 64,
                "subject_id": "subject-python",
                "domains": ["modalities"],
                "claim_id": "claim-1",
                "report_count": 1,
                "link_count": 1,
                "traceability_state": "complete",
                "requirements_complete": True,
                "all_reports_linked": True,
                "contradiction_declared": False,
                "qualification_declared": False,
                "report_classes": {"ordinary": 1},
                "bridge_modes": {"inline": 1},
                "lineage": {"harmonization_parent_digest_count": 1},
                "missing_group_ids": [],
                "missing_domains": [],
                "report_digests": ["e" * 64],
            }
        ],
        "summary": {"subject_count": 1, "domain_summary": {"modalities": {"report_count": 1}}},
        "coverage_digest": "f" * 64,
        "readiness_claimed": False,
        "execution": "not_started",
        "guarantees": [],
        "does_not_claim": [],
    }


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

    def test_retained_coverage_request_report_and_routes(self) -> None:
        normalized = DomainEvidenceHarmonizationCoverageRequest(
            subject_id="subject-python",
            domain="modalities",
            traceability_state="complete",
            after="a" * 64,
            max_items=7,
            include_report_digests=True,
        )
        self.assertEqual(normalized.to_query_params()["include_report_digests"], "true")
        self.assertEqual(normalized.to_arguments()["max_items"], 7)
        report = DomainEvidenceHarmonizationCoverageReport.from_wire(coverage_payload())
        self.assertEqual(report.rows[0]["claim_id"], "claim-1")
        self.assertEqual(report.coverage_digest, "f" * 64)
        with self.assertRaises(ArgumentError):
            DomainEvidenceHarmonizationCoverageRequest(after="bad")

        with patch.object(ApiClient, "request", return_value=coverage_payload()) as rest:
            result = ApiClient("http://127.0.0.1:8787").domain_evidence_harmonization_coverage(normalized)
        self.assertEqual(result.matching_count, 1)
        self.assertIn("traceability_state=complete", rest.call_args.args[1])
        self.assertIn("max_items=7", rest.call_args.args[1])

        with patch.object(ApiClient, "call_tool", return_value=coverage_payload()) as tool:
            result = ApiClient("http://127.0.0.1:8787").domain_evidence_harmonization_coverage_tool(normalized)
        self.assertEqual(result.rows[0]["report_count"], 1)
        self.assertEqual(tool.call_args.args[0], "domain_evidence_harmonization_coverage")

    def test_async_and_workspace_helpers_share_wire_contract(self) -> None:
        with patch.object(ApiClient, "request", return_value=harmonization_payload()):
            report = asyncio.run(
                AsyncApiClient(ApiClient("http://127.0.0.1:8787")).domain_evidence_harmonize(request())
            )
        self.assertEqual(report.traceability_state, "complete")
        with patch.object(Workspace, "tool", return_value=harmonization_payload()):
            report = Workspace(None).domain_evidence_harmonize_report(request())
        self.assertEqual(report.artifact_registry["content_digest"], "c" * 64)
        with patch.object(Workspace, "tool", return_value=coverage_payload()):
            report = Workspace(None).domain_evidence_harmonization_coverage_report()
        self.assertEqual(report.returned_count, 1)


if __name__ == "__main__":
    unittest.main()
