from __future__ import annotations

import asyncio
import unittest
from unittest.mock import patch

from prism_sdk import (
    ApiClient,
    AsyncApiClient,
    ArgumentError,
    DomainReportCoverageReport,
    DomainReportCoverageRequest,
    DomainReportProjectReport,
    DomainReportProjectRequest,
    Workspace,
)


def project_payload() -> dict:
    return {
        "ok": True,
        "schema": "bioprism-devplat-domain-report-project/0.1",
        "workflow": "domain_report_project",
        "report": {"schema": "bioprism-devplat-domain-report/0.1", "group_id": "biological_domains"},
        "artifact_registry": {
            "indexed": True,
            "kind": "domain_report",
            "subject_id": "subject-python",
            "content_digest": "a" * 64,
        },
        "coverage": {"group_id": "biological_domains", "declared_tool_count": 20},
        "readiness_claimed": False,
        "execution": "not_started",
    }


def coverage_payload() -> dict:
    return {
        "ok": True,
        "schema": "bioprism-devplat-domain-report-coverage/0.1",
        "workflow": "domain_report_coverage",
        "catalogue_digest": "b" * 64,
        "coverage_digest": "c" * 64,
        "filters": {},
        "group_count": 29,
        "reported_group_count": 1,
        "missing_group_count": 28,
        "missing_group_ids": ["documentation_and_knowledge"],
        "complete": False,
        "groups": [{"id": "biological_domains", "coverage_state": "reported"}],
        "domain_summary": {},
        "readiness_claimed": False,
        "execution": "not_started",
    }


class DomainReportModelTests(unittest.TestCase):
    def test_request_is_bounded_and_reports_are_typed(self) -> None:
        request = DomainReportProjectRequest(
            group_id="biological_domains",
            domains=("modalities",),
            subject_id="subject-python",
            source_tool="modality_catalog",
            report={"observations": []},
            claim_posture={"status": "review_required", "does_not_claim": ["truth"]},
        )
        self.assertEqual(request.to_arguments()["source_tool"], "modality_catalog")
        self.assertEqual(DomainReportProjectReport.from_wire(project_payload()).content_digest, "a" * 64)
        coverage = DomainReportCoverageReport.from_wire(coverage_payload())
        self.assertFalse(coverage.complete)
        self.assertEqual(coverage.missing_group_count, 28)
        with self.assertRaises(ArgumentError):
            DomainReportProjectRequest(
                group_id="biological_domains",
                domains=("modalities",),
                subject_id="subject-python",
                source_tool="modality_catalog",
                report={},
                claim_posture={"status": "derived", "does_not_claim": []},
            )

    def test_sync_rest_and_tool_routes_preserve_typed_contract(self) -> None:
        request = DomainReportProjectRequest(
            group_id="biological_domains",
            domains=("modalities",),
            subject_id="subject-python",
            source_tool="modality_catalog",
            report={"observations": []},
            claim_posture={"status": "review_required", "does_not_claim": ["truth"]},
        )
        with patch.object(ApiClient, "request", side_effect=[project_payload(), coverage_payload()]) as rest:
            client = ApiClient("http://127.0.0.1:8787")
            self.assertEqual(client.domain_report_project(request).content_digest, "a" * 64)
            self.assertEqual(client.domain_report_coverage(DomainReportCoverageRequest()).group_count, 29)
        self.assertEqual(rest.call_args_list[0].args[:2], ("POST", "/v1/domain-reports"))
        self.assertIn("/v1/domain-reports/coverage?", rest.call_args_list[1].args[1])

        with patch.object(ApiClient, "call_tool", return_value=project_payload()) as tool:
            self.assertEqual(ApiClient("http://127.0.0.1:8787").domain_report_project_tool(request).content_digest, "a" * 64)
        self.assertEqual(tool.call_args.args[0], "domain_report_project")

    def test_async_and_workspace_helpers_share_the_same_wire_shapes(self) -> None:
        request = DomainReportProjectRequest(
            group_id="biological_domains",
            domains=("modalities",),
            subject_id="subject-python",
            source_tool="modality_catalog",
            report={"observations": []},
            claim_posture={"status": "review_required", "does_not_claim": ["truth"]},
        )
        with patch.object(ApiClient, "request", return_value=project_payload()):
            report = asyncio.run(AsyncApiClient(ApiClient("http://127.0.0.1:8787")).domain_report_project(request))
        self.assertEqual(report.content_digest, "a" * 64)

        with patch.object(Workspace, "tool", return_value=project_payload()):
            self.assertEqual(
                Workspace(None).domain_report_project_report(request).content_digest, "a" * 64
            )


if __name__ == "__main__":
    unittest.main()
