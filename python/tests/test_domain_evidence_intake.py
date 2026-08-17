from __future__ import annotations

import asyncio
import unittest
from unittest.mock import patch

from prism_sdk import (
    ApiClient,
    AsyncApiClient,
    ArgumentError,
    DomainEvidenceIntakeCoverageReport,
    DomainEvidenceIntakeCoverageRequest,
    DomainEvidenceIntakeReport,
    DomainEvidenceIntakeRequest,
    DomainEvidenceSourcePlanReport,
    DomainEvidenceSourcePlanRequest,
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


def coverage_payload() -> dict:
    return {
        "ok": True,
        "schema": "bioprism-devplat-domain-evidence-intake-coverage/0.1",
        "workflow": "domain_evidence_intake_coverage",
        "catalogue_digest": "e" * 64,
        "coverage_digest": "f" * 64,
        "filters": {"max_groups": 64, "include_intake_digests": True},
        "group_count": 1,
        "reported_group_count": 1,
        "missing_group_count": 0,
        "missing_group_ids": [],
        "complete": True,
        "tool_coverage_complete": False,
        "missing_tool_group_ids": ["biological_domains"],
        "domain_coverage_complete": True,
        "missing_domain_group_ids": [],
        "groups": [
            {
                "id": "biological_domains",
                "domains": ["modalities"],
                "status": "active",
                "declared_tool_count": 1,
                "declared_tools": ["modality_catalog"],
                "intake_count": 1,
                "subject_ids": ["subject-python"],
                "source_tools": ["modality_catalog"],
                "outcomes": ["observed"],
                "reported_domains": ["modalities"],
                "missing_source_tools": [],
                "source_tool_coverage": [{"tool": "modality_catalog", "intake_count": 1, "outcomes": ["observed"], "coverage_state": "reported"}],
                "missing_domains": [],
                "tool_coverage_state": "complete",
                "domain_coverage_state": "complete",
                "intake_digests": ["c" * 64],
                "coverage_state": "reported",
            }
        ],
        "domain_summary": {"modalities": {"group_count": 1, "reported_group_count": 1, "missing_group_count": 0, "intake_count": 1}},
        "readiness_claimed": False,
        "execution": "not_started",
        "guarantees": [],
        "does_not_claim": [],
    }


def source_plan_payload() -> dict:
    return {
        "ok": True,
        "schema": "bioprism-devplat-domain-evidence-source-plan/0.1",
        "workflow": "domain_evidence_source_plan",
        "plan_digest": "b" * 64,
        "group_id": "biological_domains",
        "domains": ["modalities"],
        "subject_id": "source-python",
        "source_tool": "modality_catalog",
        "connector_kind": "literature",
        "locator_kind": "uri",
        "locator": "https://example.org/article/1",
        "retrieval_mode": "metadata_only",
        "expected_content_digest": "a" * 64,
        "parent_digests": [],
        "retrieval_policy": {"network": "caller_managed", "max_bytes": 4096, "cache": "content_addressed", "credentials": "caller_managed_not_supplied"},
        "plan": {"retrieval_status": "not_started"},
        "artifact_registry": {"indexed": True, "kind": "domain_evidence_source_plan", "content_digest": "h" * 64},
        "catalogue_digest": "c" * 64,
        "readiness_claimed": False,
        "execution": "not_started",
        "retrieval_status": "not_started",
        "guarantees": [],
        "does_not_claim": ["retrieval occurred"],
    }


def source_request() -> DomainEvidenceSourcePlanRequest:
    return DomainEvidenceSourcePlanRequest(
        group_id="biological_domains",
        domains=("modalities",),
        subject_id="source-python",
        source_tool="modality_catalog",
        connector_kind="literature",
        locator_kind="uri",
        locator="https://example.org/article/1",
        retrieval_mode="metadata_only",
        expected_content_digest="a" * 64,
        retrieval_policy={"network": "caller_managed", "max_bytes": 4096, "cache": "content_addressed"},
        does_not_claim=("retrieval occurred",),
    )


class DomainEvidenceIntakeModelTests(unittest.TestCase):
    def test_request_and_response_keep_request_presence_and_outcome(self) -> None:
        normalized = request()
        self.assertTrue(normalized.request_supplied)
        self.assertIn("request", normalized.to_arguments())
        report = DomainEvidenceIntakeReport.from_wire(intake_payload())
        self.assertEqual(report.outcome, "observed")
        self.assertIsNone(report.source_plan_digest)
        self.assertEqual(report.artifact_registry["kind"], "domain_evidence_intake")
        bound = DomainEvidenceIntakeRequest(**{**request().__dict__, "source_plan_digest": "f" * 64})
        self.assertEqual(bound.to_arguments()["source_plan_digest"], "f" * 64)
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

    def test_coverage_model_preserves_filters_and_missingness(self) -> None:
        request = DomainEvidenceIntakeCoverageRequest(
            group_id="biological_domains",
            domain="modalities",
            include_intake_digests=True,
        )
        self.assertEqual(request.to_query_params()["include_intake_digests"], "true")
        self.assertEqual(request.to_arguments()["group_id"], "biological_domains")
        report = DomainEvidenceIntakeCoverageReport.from_wire(coverage_payload())
        self.assertTrue(report.complete)
        self.assertFalse(report.tool_coverage_complete)
        self.assertEqual(report.groups[0]["outcomes"], ["observed"])
        with self.assertRaises(ArgumentError):
            DomainEvidenceIntakeCoverageRequest(max_groups=129)

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

    def test_coverage_rest_tool_async_and_workspace_helpers(self) -> None:
        coverage_request = DomainEvidenceIntakeCoverageRequest(include_intake_digests=True)
        with patch.object(ApiClient, "request", return_value=coverage_payload()) as rest:
            report = ApiClient("http://127.0.0.1:8787").domain_evidence_coverage(coverage_request)
        self.assertTrue(report.complete)
        self.assertIn("include_intake_digests=true", rest.call_args.args[1])
        with patch.object(ApiClient, "call_tool", return_value=coverage_payload()) as tool:
            report = ApiClient("http://127.0.0.1:8787").domain_evidence_coverage_tool(coverage_request)
        self.assertEqual(tool.call_args.args[0], "domain_evidence_coverage")
        with patch.object(ApiClient, "request", return_value=coverage_payload()):
            report = asyncio.run(
                AsyncApiClient(ApiClient("http://127.0.0.1:8787")).domain_evidence_coverage(coverage_request)
            )
        self.assertEqual(report.missing_group_count, 0)
        with patch.object(Workspace, "tool", return_value=coverage_payload()):
            report = Workspace(None).domain_evidence_coverage_report(coverage_request)
        self.assertEqual(report.coverage_digest, "f" * 64)

    def test_source_plan_preserves_non_fetching_posture_across_clients(self) -> None:
        report = DomainEvidenceSourcePlanReport.from_wire(source_plan_payload())
        self.assertEqual(report.plan_digest, "b" * 64)
        self.assertEqual(report.retrieval_status, "not_started")
        with self.assertRaises(ArgumentError):
            DomainEvidenceSourcePlanRequest(
                **{**source_request().__dict__, "locator": "https://user:secret@example.org/source"}
            )
        with patch.object(ApiClient, "request", return_value=source_plan_payload()) as rest:
            report = ApiClient("http://127.0.0.1:8787").domain_evidence_source_plan(source_request())
        self.assertEqual(rest.call_args.args[:2], ("POST", "/v1/domain-evidence/sources"))
        with patch.object(ApiClient, "call_tool", return_value=source_plan_payload()) as tool:
            report = ApiClient("http://127.0.0.1:8787").domain_evidence_source_plan_tool(source_request())
        self.assertEqual(tool.call_args.args[0], "domain_evidence_source_plan")
        with patch.object(ApiClient, "request", return_value=source_plan_payload()):
            report = asyncio.run(
                AsyncApiClient(ApiClient("http://127.0.0.1:8787")).domain_evidence_source_plan(source_request())
            )
        self.assertEqual(report.connector_kind, "literature")
        with patch.object(Workspace, "tool", return_value=source_plan_payload()):
            report = Workspace(None).domain_evidence_source_plan_report(source_request())
        self.assertEqual(report.artifact_registry["kind"], "domain_evidence_source_plan")


if __name__ == "__main__":
    unittest.main()
