from __future__ import annotations

import asyncio
import json
import unittest
from unittest.mock import patch

from prism_sdk import (
    ApiClient,
    AsyncApiClient,
    AsyncWorkspace,
    CiProviderEvidenceReport,
    CiProviderEvidenceRequest,
    Workspace,
    ci_provider_evidence_report,
)
from prism_sdk.errors import ArgumentError
from prism_sdk.models import ToolResult


def payload() -> dict:
    digest = "a" * 64
    audit = {
        "schema": "bioprism-devplat-ci-provider-evidence/0.1",
        "workflow": "ci_provider_evidence_audit",
        "provider": "github_actions",
        "source": "provider_observed",
        "run_id": "run-42",
        "payload_digest": digest,
        "plan_digest": digest,
        "evidence_digest": digest,
        "artifact_record_digest": digest,
        "log_record_digest": digest,
        "attestation_record_digest": digest,
        "artifact_count": 1,
        "log_count": 1,
        "attestation_count": 1,
        "linked_artifact_count": 1,
        "linked_log_count": 1,
        "attestation_subject_count": 1,
        "ci_evidence": {"run_id": "run-42"},
        "artifacts": [{"id": "artifact-1", "kind": "junit", "digest": digest}],
        "logs": [{"id": "log-1", "digest": digest}],
        "attestations": [{"id": "attestation-1", "subject": "artifact-1", "issuer": "caller", "statement_digest": digest, "method": "declared"}],
        "structurally_valid": True,
        "conformance_ready": True,
        "execution": "evidence_supplied_not_executed_here",
        "verification": "structural_only",
        "findings": [],
        "guarantees": [],
        "limitations": [],
    }
    return {
        "ok": True,
        "workflow": "ci_provider_evidence_audit",
        "schema": "bioprism-devplat-ci-provider-evidence/0.1",
        "valid": True,
        "conformance_ready": True,
        "evidence": {"run_id": "run-42", "provider": "github_actions", "checks": []},
        "audit": audit,
        "guarantees": [],
        "limitations": [],
    }


class _SyncTool:
    def call_tool(self, name: str, arguments: dict) -> ToolResult:
        return ToolResult(name, {"isError": False, "content": [{"type": "text", "text": json.dumps(payload())}]})


class _AsyncTool:
    async def call_tool(self, name: str, arguments: dict) -> ToolResult:
        return ToolResult(name, {"isError": False, "content": [{"type": "text", "text": json.dumps(payload())}]})


def request() -> CiProviderEvidenceRequest:
    digest = "b" * 64
    return CiProviderEvidenceRequest(
        ci={"workflow": "contracts", "checks": []},
        provider="github_actions",
        payload={"run": {"id": 42, "conclusion": "success"}, "jobs": []},
        artifacts=[{"id": "artifact-1", "kind": "junit", "digest": digest, "run_id": "42", "provider": "github_actions"}],
        logs=[{"id": "log-1", "digest": digest, "run_id": "42", "provider": "github_actions"}],
        attestations=[{"id": "attestation-1", "subject": "artifact-1", "issuer": "caller", "statement_digest": digest, "method": "declared"}],
    )


class CiProviderEvidenceTests(unittest.TestCase):
    def test_request_and_report_preserve_rows_and_structural_boundary(self) -> None:
        args = request()
        wire = args.to_mcp_arguments()
        self.assertEqual(wire["artifacts"][0]["id"], "artifact-1")
        report = ci_provider_evidence_report(payload())
        self.assertIsInstance(report, CiProviderEvidenceReport)
        self.assertTrue(report.conformance_ready)
        self.assertEqual(report.attestation_subject_count, 1)
        self.assertEqual(report.evidence["run_id"], "run-42")
        self.assertEqual(report.verification, "structural_only")
        with self.assertRaises(ArgumentError):
            CiProviderEvidenceRequest({}, "generic", {})
        with self.assertRaises(ArgumentError):
            CiProviderEvidenceRequest({}, "generic", {}, artifacts=[{} for _ in range(129)])

    def test_all_facades_keep_provider_evidence_typed(self) -> None:
        args = request()
        self.assertTrue(Workspace(_SyncTool()).ci_provider_evidence_report(args).conformance_ready)
        self.assertTrue(asyncio.run(AsyncWorkspace(_AsyncTool()).ci_provider_evidence_report(args)).structurally_valid)
        with patch.object(ApiClient, "call_tool", return_value=payload()) as call:
            result = ApiClient("http://127.0.0.1:1").ci_provider_evidence_report(args)
        self.assertEqual(result.artifact_count, 1)
        call.assert_called_once_with("ci_provider_evidence_audit", args.to_mcp_arguments())

        async def run() -> None:
            with patch.object(ApiClient, "call_tool", return_value=payload()) as async_call:
                result = await AsyncApiClient(ApiClient("http://127.0.0.1:1")).ci_provider_evidence_report(args)
            self.assertEqual(result.execution, "evidence_supplied_not_executed_here")
            async_call.assert_called_once_with("ci_provider_evidence_audit", args.to_mcp_arguments())

        asyncio.run(run())


if __name__ == "__main__":
    unittest.main()
