from __future__ import annotations

import asyncio
import json
import unittest
from unittest.mock import patch

from prism_sdk import (
    ApiClient,
    AsyncApiClient,
    AsyncWorkspace,
    QualityGateRunArgs,
    QualityGateRunReport,
    Workspace,
    quality_gate_run,
)
from prism_sdk.errors import ArgumentError
from prism_sdk.models import ToolResult


def request() -> QualityGateRunArgs:
    return QualityGateRunArgs(
        dataset={"name": "release-quality", "columns": {"age": [41, 42], "subject": ["s-1", "s-1"]}, "rows": 2},
        gate={
            "name": "release-gate",
            "checks": {
                "age_range": {"InRange": {"column": "age", "min": 0.0, "max": 120.0}},
                "subject_unique": {"Unique": {"column": "subject"}},
                "foreign_site": {"ForeignKey": {"column": "site", "reference": "sites"}},
            },
        },
    )


def payload() -> dict:
    return {
        "ok": True,
        "schema": "bioprism-mcp/quality-gate/0.1",
        "verdict": "failed",
        "passed": False,
        "dataset": "release-quality",
        "rows": 2,
        "check_count": 3,
        "report": {
            "gate": "release-gate",
            "dataset": "release-quality",
            "rows": 2,
            "outcomes": {
                "age_range": {"Pass": {"examined": 2}},
                "subject_unique": {
                    "Fail": {
                        "witness": {
                            "row": 1,
                            "column": "subject",
                            "found": "s-1",
                            "expected": "a value not already seen at row 0",
                        }
                    }
                },
                "foreign_site": {"NotRunnable": {"reason": {"MissingReferenceSet": {"reference": "sites"}}}},
            },
            "verdict": {"Failed": {"failing": ["subject_unique"], "not_runnable": ["foreign_site"]}},
        },
        "guarantees": [
            "pass requires every named check to run and hold",
            "failed checks carry a concrete row and expected value witness",
        ],
    }


class _SyncTool:
    def call_tool(self, name: str, arguments: dict) -> ToolResult:
        return ToolResult(name, {"isError": False, "content": [{"type": "text", "text": json.dumps(payload())}]})


class _AsyncTool:
    async def call_tool(self, name: str, arguments: dict) -> ToolResult:
        return ToolResult(name, {"isError": False, "content": [{"type": "text", "text": json.dumps(payload())}]})


class QualityGateTests(unittest.TestCase):
    def test_request_preserves_serialized_dataset_gate_and_bounds(self) -> None:
        args = request()
        self.assertEqual(args.to_mcp_arguments()["dataset"]["rows"], 2)
        self.assertEqual(args.to_mcp_arguments()["gate"]["checks"]["age_range"]["InRange"]["max"], 120.0)
        with self.assertRaises(ArgumentError):
            QualityGateRunArgs({"name": "bad", "columns": {"age": [1]}, "rows": 2}, {"name": "g", "checks": {"x": {}}})
        with self.assertRaises(ArgumentError):
            QualityGateRunArgs({"name": "bad", "columns": {}, "rows": 0}, {"name": "g", "checks": {}})

    def test_report_keeps_witness_not_runnable_and_three_way_verdict(self) -> None:
        report = quality_gate_run(payload())
        self.assertIsInstance(report, QualityGateRunReport)
        self.assertFalse(report.passed)
        self.assertEqual(report.report.passed_checks, ("age_range",))
        self.assertEqual(report.report.failed_checks, ("subject_unique",))
        self.assertEqual(report.report.not_runnable_checks, ("foreign_site",))
        self.assertEqual(report.report.outcomes["subject_unique"].witness.row, 1)
        self.assertEqual(report.report.outcomes["foreign_site"].reason.kind, "MissingReferenceSet")
        self.assertTrue(report.has_data_failures)
        self.assertTrue(report.has_run_obstructions)
        self.assertTrue(report.failures_and_obstructions_are_separate)
        envelope = {"ok": True, "mcp": {"result": {"structuredContent": payload()}}}
        self.assertEqual(quality_gate_run(envelope).verdict, "failed")

    def test_all_python_facades_return_typed_quality_reports(self) -> None:
        args = request()
        self.assertTrue(asyncio.run(AsyncWorkspace(_AsyncTool()).quality_gate_run_report(args)).has_run_obstructions)
        self.assertTrue(Workspace(_SyncTool()).quality_gate_run_report(args).has_data_failures)
        with patch.object(ApiClient, "call_tool", return_value=payload()) as call:
            report = ApiClient("http://127.0.0.1:1").quality_gate_run_report(args)
        self.assertEqual(report.report.outcomes["foreign_site"].reason.reference, "sites")
        call.assert_called_once_with("quality_gate_run", args.to_mcp_arguments())

        async def run() -> None:
            with patch.object(ApiClient, "call_tool", return_value=payload()) as async_call:
                result = await AsyncApiClient(ApiClient("http://127.0.0.1:1")).quality_gate_run_report(args)
            self.assertEqual(result.schema, "bioprism-mcp/quality-gate/0.1")
            async_call.assert_called_once_with("quality_gate_run", args.to_mcp_arguments())

        asyncio.run(run())


if __name__ == "__main__":
    unittest.main()
