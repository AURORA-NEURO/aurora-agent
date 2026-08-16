from __future__ import annotations

import asyncio
import json
import unittest
from unittest.mock import patch

from prism_sdk import (
    ApiClient,
    AsyncWorkspace,
    BenchmarkOracleReviewArgs,
    BenchmarkOracleReviewReport,
    Workspace,
    benchmark_oracle_review_report,
)
from prism_sdk.errors import ArgumentError
from prism_sdk.models import ToolResult


def oracle_request() -> dict:
    return {
        "proposal": {
            "oracle_id": "oracle-demo",
            "decision_point": "choose evidence",
            "strength": "exact_state_predicate",
            "acceptable_verdicts": ["pass"],
            "required_witnesses": ["evidence"],
            "can_see": ["declared world"],
            "blind_spots": ["hidden grader state"],
            "exploits": [],
        },
        "reviewer": "reviewer-1",
        "grade": {"verdict": "pass", "witnesses": ["evidence"], "closure_complete": True},
        "cell": {
            "cell_id": "cell-reviewed",
            "world": {"locator": "world.json", "sha256": "a" * 64},
            "query": {"locator": "query.json", "sha256": "b" * 64},
        },
    }


def oracle_payload() -> dict:
    return {
        "ok": True,
        "schema": "bioprism-mcp/benchmark-oracle-review/0.1",
        "proposal": oracle_request()["proposal"],
        "reviewed_oracle": {"inner": oracle_request()["proposal"], "reviewer": "reviewer-1", "review_digest": "c" * 64},
        "reviewer": "reviewer-1",
        "review_digest": "c" * 64,
        "strength": "exact_state_predicate",
        "deterministic": True,
        "grade": {
            "verdict": "pass",
            "witnesses": ["evidence"],
            "closure_complete": True,
            "acceptance": {"outcome": "passed"},
            "passed": True,
            "reason": "passed",
        },
        "cell": {"schema_version": "bioprism-decision-cell/0.1", "cell_id": "cell-reviewed", "acceptable_verdicts": ["pass"], "required_witnesses": ["evidence"]},
        "synthesis_order": ["exact_state_predicate", "execution_test", "property_relation", "trajectory_constraint", "statistical_tolerance", "model_judge"],
        "guarantees": ["only the kernel review gate creates a ReviewedOracle"],
        "limitations": ["declarative contract"],
    }


class _SyncTool:
    def __init__(self, payload: dict) -> None:
        self.payload = payload

    def call_tool(self, name: str, arguments: dict) -> ToolResult:
        return ToolResult(name, {"isError": False, "content": [{"type": "text", "text": json.dumps(self.payload)}]})


class _AsyncTool:
    def __init__(self, payload: dict) -> None:
        self.payload = payload

    async def call_tool(self, name: str, arguments: dict) -> ToolResult:
        return ToolResult(name, {"isError": False, "content": [{"type": "text", "text": json.dumps(self.payload)}]})


class BenchmarkOracleTests(unittest.TestCase):
    def test_args_report_grade_and_cell_keep_kernel_contract_visible(self) -> None:
        request = BenchmarkOracleReviewArgs.from_wire(oracle_request())
        self.assertEqual(request.to_mcp_arguments()["proposal"]["acceptable_verdicts"], ["pass"])
        report = benchmark_oracle_review_report(oracle_payload())
        self.assertIsInstance(report, BenchmarkOracleReviewReport)
        self.assertTrue(report.accepted)
        self.assertTrue(report.graded)
        self.assertTrue(report.packaged)
        self.assertTrue(report.passed)
        self.assertEqual(report.acceptance_outcome, "passed")
        self.assertEqual(report.review_digest, "c" * 64)
        with self.assertRaises(ArgumentError):
            BenchmarkOracleReviewArgs.from_wire({**oracle_request(), "reviewer": ""})

    def test_refusals_require_fail_closed_and_facades_preserve_report(self) -> None:
        refusal = {"ok": False, "schema": "bioprism-mcp/benchmark-oracle-review/0.1", "stage": "oracle_review", "refusal": "unreviewed", "fail_closed": True, "guarantees": ["no grading"]}
        report = benchmark_oracle_review_report(refusal)
        self.assertTrue(report.refused)
        self.assertTrue(report.fail_closed)
        request = BenchmarkOracleReviewArgs.from_wire(oracle_request())
        self.assertEqual(Workspace(_SyncTool(oracle_payload())).benchmark_oracle_review_report(request).acceptance_outcome, "passed")
        self.assertEqual(asyncio.run(AsyncWorkspace(_AsyncTool(oracle_payload())).benchmark_oracle_review_report(request)).reviewer, "reviewer-1")
        with patch.object(ApiClient, "call_tool", return_value=oracle_payload()) as call:
            result = ApiClient("http://127.0.0.1:1").benchmark_oracle_review_report(request)
        self.assertEqual(result.cell["cell_id"], "cell-reviewed")  # type: ignore[index]
        call.assert_called_once_with("benchmark_oracle_review", request.to_mcp_arguments())

    def test_refusal_without_fail_closed_is_rejected(self) -> None:
        with self.assertRaises(ArgumentError):
            benchmark_oracle_review_report({"ok": False, "stage": "oracle_review", "refusal": "unreviewed"})

