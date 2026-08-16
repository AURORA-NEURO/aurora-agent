from __future__ import annotations

import asyncio
import json
import unittest
from unittest.mock import patch

from prism_sdk import (
    ApiClient,
    AsyncApiClient,
    AsyncWorkspace,
    CiExecutionEvidenceReport,
    CiExecutionEvidenceRequest,
    Workspace,
    ci_execution_evidence_report,
)
from prism_sdk.errors import ArgumentError
from prism_sdk.models import ToolResult


def payload() -> dict:
    return {
        "ok": True,
        "workflow": "ci_execution_evidence_audit",
        "schema": "bioprism-devplat-ci-execution-evidence/0.1",
        "valid": True,
        "ci_evidence_ready": True,
        "plan_digest": "a" * 64,
        "evidence_digest": "b" * 64,
        "audit": {
            "schema": "bioprism-devplat-ci-execution-evidence/0.1",
            "workflow": "contracts",
            "plan_digest": "a" * 64,
            "evidence_digest": "b" * 64,
            "run_id": "run-42",
            "provider": "github_actions",
            "source": "provider_observed",
            "conclusion": "success",
            "expected_check_count": 2,
            "observed_check_count": 2,
            "passed_check_count": 2,
            "failed_check_count": 0,
            "skipped_check_count": 0,
            "unknown_check_count": 0,
            "required_missing": [],
            "required_failed": [],
            "optional_nonpassing": [],
            "complete": True,
            "structurally_valid": True,
            "release_candidate": True,
            "execution": "evidence_supplied_not_executed_here",
            "verification": "structural_only",
            "findings": [],
            "guarantees": [],
            "limitations": [],
        },
        "guarantees": [],
        "limitations": [],
    }


class _SyncTool:
    def call_tool(self, name: str, arguments: dict) -> ToolResult:
        return ToolResult(name, {"isError": False, "content": [{"type": "text", "text": json.dumps(payload())}]})


class _AsyncTool:
    async def call_tool(self, name: str, arguments: dict) -> ToolResult:
        return ToolResult(name, {"isError": False, "content": [{"type": "text", "text": json.dumps(payload())}]})


def request() -> CiExecutionEvidenceRequest:
    return CiExecutionEvidenceRequest(
        ci={"workflow": "contracts", "triggers": ["push"], "rust_toolchain": "stable", "checks": []},
        evidence={"run_id": "run-42", "provider": "github_actions"},
    )


class CiEvidenceTests(unittest.TestCase):
    def test_request_and_report_preserve_structural_boundary(self) -> None:
        args = request()
        self.assertEqual(args.to_mcp_arguments()["evidence"]["run_id"], "run-42")
        report = ci_execution_evidence_report(payload())
        self.assertIsInstance(report, CiExecutionEvidenceReport)
        self.assertTrue(report.release_candidate)
        self.assertTrue(report.complete)
        self.assertEqual(report.verification, "structural_only")
        self.assertEqual(report.blocking_findings, ())
        with self.assertRaises(ArgumentError):
            CiExecutionEvidenceRequest({}, {"run_id": "x"})

    def test_all_facades_keep_ci_evidence_typed(self) -> None:
        args = request()
        self.assertTrue(Workspace(_SyncTool()).ci_execution_evidence_report(args).release_candidate)
        self.assertTrue(asyncio.run(AsyncWorkspace(_AsyncTool()).ci_execution_evidence_report(args)).complete)
        with patch.object(ApiClient, "call_tool", return_value=payload()) as call:
            result = ApiClient("http://127.0.0.1:1").ci_execution_evidence_report(args)
        self.assertEqual(result.plan_digest, "a" * 64)
        call.assert_called_once_with("ci_execution_evidence_audit", args.to_mcp_arguments())

        async def run() -> None:
            with patch.object(ApiClient, "call_tool", return_value=payload()) as async_call:
                result = await AsyncApiClient(ApiClient("http://127.0.0.1:1")).ci_execution_evidence_report(args)
            self.assertEqual(result.execution, "evidence_supplied_not_executed_here")
            async_call.assert_called_once_with("ci_execution_evidence_audit", args.to_mcp_arguments())

        asyncio.run(run())


if __name__ == "__main__":
    unittest.main()
