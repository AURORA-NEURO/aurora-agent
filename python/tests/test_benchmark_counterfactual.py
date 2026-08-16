from __future__ import annotations

import asyncio
import json
import unittest
from unittest.mock import patch

from prism_sdk import (
    ApiClient,
    AsyncApiClient,
    AsyncWorkspace,
    BenchmarkCounterfactualCheckArgs,
    BenchmarkCounterfactualCheckReport,
    Workspace,
    benchmark_counterfactual_check_report,
)
from prism_sdk.errors import ArgumentError
from prism_sdk.models import ToolResult


def counterfactual_request() -> dict:
    cell = {
        "schema_version": "bioprism-decision-cell/0.1",
        "cell_id": "source",
        "decision_point": "choose evidence",
        "world": {"locator": "world", "sha256": "a" * 64},
        "query": {"locator": "query", "sha256": "b" * 64},
        "acceptable_verdicts": ["pass"],
        "required_witnesses": ["evidence"],
        "require_protected_closure": True,
    }
    followup = dict(cell)
    followup["cell_id"] = "followup"
    followup["query"] = {"locator": "query-2", "sha256": "c" * 64}
    return {
        "source": cell,
        "followup": followup,
        "intervention": {"factor": "fresh evidence", "target": "evidence_availability", "from": {"available": False}, "to": {"available": True}, "changes": ["query"]},
        "expected": {"expect": "invariant", "rationale": "same correct verdict"},
        "source_verdict": "pass",
        "followup_verdict": "pass",
    }


def counterfactual_payload() -> dict:
    return {
        "ok": True,
        "schema": "bioprism-mcp/benchmark-counterfactual/0.1",
        "pair": {"differing_fields": ["query"], "realism_reviewed": False},
        "outcome": {"outcome": "as_predicted"},
        "satisfied": True,
        "source_verdict": "pass",
        "followup_verdict": "pass",
        "cell_digests": {"source": "a" * 64, "followup": "b" * 64},
        "allowed_cell_fields": ["world", "query", "acceptable_verdicts", "required_witnesses", "require_protected_closure"],
        "guarantees": ["one factor"],
        "limitations": ["no realism validator"],
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


class BenchmarkCounterfactualTests(unittest.TestCase):
    def test_args_and_report_preserve_one_factor_and_no_realism_review(self) -> None:
        request = BenchmarkCounterfactualCheckArgs.from_wire(counterfactual_request())
        self.assertEqual(request.to_mcp_arguments()["intervention"]["changes"], ["query"])
        report = benchmark_counterfactual_check_report(counterfactual_payload())
        self.assertIsInstance(report, BenchmarkCounterfactualCheckReport)
        self.assertTrue(report.accepted)
        self.assertEqual(report.outcome_kind, "as_predicted")
        self.assertFalse(report.pair["realism_reviewed"])  # type: ignore[index]
        with self.assertRaises(ArgumentError):
            BenchmarkCounterfactualCheckArgs.from_wire({**counterfactual_request(), "source_verdict": ""})

    def test_refusal_and_all_facades_preserve_fail_closed_state(self) -> None:
        refusal = {"ok": False, "stage": "matched_pair", "refusal": "field moved without declaration", "fail_closed": True, "guarantees": ["no partial pair"]}
        report = benchmark_counterfactual_check_report(refusal)
        self.assertTrue(report.refused)
        self.assertTrue(report.fail_closed)
        request = BenchmarkCounterfactualCheckArgs.from_wire(counterfactual_request())
        self.assertEqual(Workspace(_SyncTool(counterfactual_payload())).benchmark_counterfactual_check_report(request).outcome_kind, "as_predicted")
        self.assertEqual(asyncio.run(AsyncWorkspace(_AsyncTool(counterfactual_payload())).benchmark_counterfactual_check_report(request)).satisfied, True)
        with patch.object(ApiClient, "call_tool", return_value=counterfactual_payload()) as call:
            result = ApiClient("http://127.0.0.1:1").benchmark_counterfactual_check_report(request)
        self.assertEqual(result.cell_digests["source"], "a" * 64)
        call.assert_called_once_with("benchmark_counterfactual_check", request.to_mcp_arguments())

