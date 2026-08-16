from __future__ import annotations

import asyncio
import json
import unittest
from unittest.mock import patch

from prism_sdk import (
    ApiClient,
    AsyncApiClient,
    AsyncWorkspace,
    BenchmarkCompileArgs,
    BenchmarkCompileReport,
    BenchmarkCompileReviewArgs,
    BenchmarkCompileReviewReport,
    Workspace,
    benchmark_compile_report,
    benchmark_compile_review_report,
)
from prism_sdk.errors import ArgumentError
from prism_sdk.models import ToolResult


def compile_request() -> dict:
    def trace(trace_id: str, tool: str, succeeded: bool) -> dict:
        return {
            "trace_id": trace_id,
            "succeeded": succeeded,
            "events": [
                {"step": 0, "kind": "goal", "payload": {"summary": "rank"}},
                {"step": 1, "kind": "action", "payload": {"tool": "choose_assay", "irreversible": True}, "caused_by": 0},
                {"step": 2, "kind": "result", "payload": {"summary": "selected"}, "caused_by": 1},
                {"step": 3, "kind": "action", "payload": {"tool": tool}, "caused_by": 2},
                {"step": 4, "kind": "claim", "payload": {"summary": "hit"}, "caused_by": 3},
                {"step": 5, "kind": "termination", "payload": {"summary": "done"}, "caused_by": 4},
            ],
        }

    def row(kept: list[str], invalid: bool) -> dict:
        return {"kept": kept, "signature": {"verdict": "invalid" if invalid else "valid", "witnesses": ["identity_leakage"] if invalid else [], "divergence_step": 3}}

    return {
        "trace": trace("run_fail", "wrong_panel", False),
        "reference": trace("run_pass", "right_panel", True),
        "context": [
            {"id": "panel_manifest", "tier": "artifact", "guard": "removable"},
            {"id": "unused_service", "tier": "service", "guard": "removable"},
        ],
        "probe_observations": [row([], False), row(["panel_manifest"], True), row(["unused_service"], False), row(["panel_manifest", "unused_service"], True)],
        "budget": {"max_evaluations": 100},
    }


def compile_payload() -> dict:
    return {
        "ok": True,
        "schema": "bioprism-mcp/benchmark-compile/0.1",
        "trace_id": "run_fail",
        "trace_digest": "a" * 64,
        "reference_digest": "b" * 64,
        "compilation": {"trace_id": "run_fail", "class": {"class": "candidate_research_cell"}},
        "class": {"class": "candidate_research_cell"},
        "cell_step": 3,
        "episodes": 1,
        "boundary_count": 2,
        "oracle": {"oracle_id": "or_run_fail#step3", "strength": "exact_state_predicate"},
        "minimization": {"minimal": ["panel_manifest"], "removed": ["unused_service"], "reduction_ratio": 0.5, "evaluations": 8, "passes": 1, "guarantee": "1-minimal"},
        "confidence": {"boundary_detection": {"state": "measured", "value": 0.8}},
        "limiting_stage": ["boundary_detection", 0.8],
        "unmeasured_stages": ["state_reconstruction", "oracle_adequacy", "mutation_validity"],
        "probe": {"provided_rows": 4, "evaluations": 8, "execution": "caller-supplied observation table; no world or architecture was run"},
        "guarantees": ["no execution"],
        "limitations": ["no mutation generation"],
    }


def compile_review_payload() -> dict:
    return {
        "ok": True,
        "schema": "bioprism-mcp/benchmark-compile-review/0.1",
        "compile": {"trace_id": "run_fail", "class": {"class": "candidate_research_cell"}},
        "reviewed_oracle": {"inner": {"oracle_id": "oracle-run-fail"}, "reviewer": "reviewer-1", "review_digest": "d" * 64},
        "reviewer": "reviewer-1",
        "review_digest": "d" * 64,
        "grade": {"acceptance": {"outcome": "passed"}, "passed": True},
        "cell": {"cell_id": "dc_run_fail#step3", "acceptable_verdicts": ["invalid"], "required_witnesses": ["identity_leakage"]},
        "guarantees": ["reviewed before packaging"],
        "limitations": ["no execution"],
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


class BenchmarkCompileTests(unittest.TestCase):
    def test_args_and_report_preserve_pipeline_layers(self) -> None:
        request = BenchmarkCompileArgs.from_wire(compile_request())
        self.assertEqual(len(request.to_mcp_arguments()["probe_observations"]), 4)
        report = benchmark_compile_report(compile_payload())
        self.assertIsInstance(report, BenchmarkCompileReport)
        self.assertTrue(report.accepted)
        self.assertEqual(report.class_name, "candidate_research_cell")
        self.assertTrue(report.has_oracle)
        self.assertEqual(report.reduction_ratio, 0.5)
        self.assertIn("state_reconstruction", report.unmeasured_stages)
        with self.assertRaises(ArgumentError):
            BenchmarkCompileArgs.from_wire({**compile_request(), "budget": {"max_evaluations": 0}})

    def test_fail_closed_refusal_and_all_facades(self) -> None:
        refusal = {"ok": False, "schema": "bioprism-mcp/benchmark-compile/0.1", "stage": "minimization_probe", "refusal": "incomplete observation table", "fail_closed": True, "guarantees": ["no interpolation"]}
        report = benchmark_compile_report(refusal)
        self.assertTrue(report.refused)
        self.assertTrue(report.fail_closed)
        request = BenchmarkCompileArgs.from_wire(compile_request())
        self.assertEqual(Workspace(_SyncTool(compile_payload())).benchmark_compile_report(request).class_name, "candidate_research_cell")
        self.assertEqual(asyncio.run(AsyncWorkspace(_AsyncTool(compile_payload())).benchmark_compile_report(request)).cell_step, 3)
        with patch.object(ApiClient, "call_tool", return_value=compile_payload()) as call:
            result = ApiClient("http://127.0.0.1:1").benchmark_compile_report(request)
        self.assertEqual(result.trace_id, "run_fail")
        call.assert_called_once_with("benchmark_compile", request.to_mcp_arguments())

        async def run() -> None:
            with patch.object(ApiClient, "call_tool", return_value=compile_payload()) as async_call:
                result = await AsyncApiClient(ApiClient("http://127.0.0.1:1")).benchmark_compile_report(request)
            self.assertEqual(result.class_name, "candidate_research_cell")
            async_call.assert_called_once_with("benchmark_compile", request.to_mcp_arguments())

        asyncio.run(run())

    def test_end_to_end_review_args_report_and_workspace_facade(self) -> None:
        wire = compile_request()
        wire.update({"reviewer": "reviewer-1", "world": {"locator": "world", "sha256": "a" * 64}, "query": {"locator": "query", "sha256": "b" * 64}, "grade": {"verdict": "invalid", "witnesses": ["identity_leakage"], "closure_complete": True}})
        request = BenchmarkCompileReviewArgs.from_wire(wire)
        self.assertEqual(request.to_mcp_arguments()["reviewer"], "reviewer-1")
        report = benchmark_compile_review_report(compile_review_payload())
        self.assertIsInstance(report, BenchmarkCompileReviewReport)
        self.assertTrue(report.accepted)
        self.assertTrue(report.packaged)
        self.assertTrue(report.passed)
        self.assertEqual(report.acceptance_outcome, "passed")
        self.assertEqual(Workspace(_SyncTool(compile_review_payload())).benchmark_compile_review_report(request).cell["cell_id"], "dc_run_fail#step3")  # type: ignore[index]
        with patch.object(ApiClient, "call_tool", return_value=compile_review_payload()) as call:
            result = ApiClient("http://127.0.0.1:1").benchmark_compile_review_report(request)
        self.assertEqual(result.reviewer, "reviewer-1")
        call.assert_called_once_with("benchmark_compile_review", request.to_mcp_arguments())
