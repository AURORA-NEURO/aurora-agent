from __future__ import annotations

import asyncio
import json
import unittest
from unittest.mock import patch

from prism_sdk import (
    ApiClient,
    AsyncApiClient,
    AsyncWorkspace,
    BenchmarkTraceAnalyzeArgs,
    BenchmarkTraceAnalysisReport,
    Workspace,
    benchmark_trace_analysis_report,
)
from prism_sdk.errors import ArgumentError
from prism_sdk.models import ToolResult


def failing_trace() -> dict:
    return {
        "trace_id": "failed-run",
        "succeeded": False,
        "events": [
            {"step": 0, "kind": "goal", "payload": {"summary": "solve"}},
            {"step": 1, "kind": "choice", "payload": {"summary": "choose route", "alternatives": ["safe", "unsafe"]}, "visible": ["task"]},
            {"step": 2, "kind": "termination", "payload": {"summary": "failed"}, "caused_by": 1, "visible": ["task"]},
        ],
    }


def reference_trace() -> dict:
    return {
        "trace_id": "reference-run",
        "succeeded": True,
        "events": [
            {"step": 0, "kind": "goal", "payload": {"summary": "solve"}},
            {"step": 1, "kind": "choice", "payload": {"summary": "choose safe", "alternatives": ["safe", "unsafe"]}, "visible": ["task"]},
            {"step": 2, "kind": "termination", "payload": {"summary": "succeeded"}, "caused_by": 1, "visible": ["task"]},
        ],
    }


def analysis_payload() -> dict:
    return {
        "ok": True,
        "trace_id": "failed-run",
        "succeeded": False,
        "event_count": 3,
        "reference_trace_id": "reference-run",
        "analysis": {
            "trace_id": "failed-run",
            "textual": {
                "kind": "diverged",
                "failing_step": 1,
                "passing_step": 1,
                "common_prefix": 1,
                "failing_did": "choice choose route",
                "passing_did": "choice choose safe",
                "visibility_gap": [],
            },
            "textual_is_actionable": True,
            "reference": "reference-run",
            "terminal_step": 2,
            "ancestry": [1],
            "candidates": [{
                "step": 1,
                "kind": "choice",
                "summary": "choose route",
                "score": {"necessity": 1.0, "counterfactual_effect": 1.0, "irreversibility": 0.25, "explanatory_simplicity": 0.5, "total": 0.775, "irreversibility_declared": False},
            }],
            "verdict": {"verdict": "first_causal", "step": 1, "score": 0.775},
        },
        "episodes": [{"index": 0, "goal_step": 0, "label": "solve", "steps": [0, 1, 2]}],
        "boundaries": [{
            "step": 1,
            "summary": "choose route",
            "decision_type": "unclassified",
            "type_evidence": "unknown, not defaulted",
            "reversibility": {"source": "assumed", "irreversible": False, "basis": "choice did not move the world"},
            "rank": {"alternatives": 2, "newly_visible": 1, "downstream_steps": 1, "is_divergence": True, "total": 1.25},
        }],
        "repetitions": [{
            "summary": "retry assay",
            "steps": [3, 5],
            "classification": {"kind": "iterative_refinement", "evidence_gained": ["new-result"]},
        }],
        "summary": {"episode_count": 1, "boundary_count": 1, "extractable_boundaries": 1, "repetition_groups": 1},
        "guarantees": ["causal ranking, boundary ranking, repetition, and episode segmentation remain separate evidence layers"],
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


class BenchmarkTraceProjectionTests(unittest.TestCase):
    def test_args_preserve_trace_evidence_and_bound_duplicate_steps(self) -> None:
        request = BenchmarkTraceAnalyzeArgs.from_wire({"failing": failing_trace(), "reference": reference_trace()})
        wire = request.to_mcp_arguments()
        self.assertEqual(wire["failing"]["events"][1]["visible"], ["task"])
        self.assertEqual(wire["reference"]["trace_id"], "reference-run")
        invalid = failing_trace()
        invalid["events"].append({"step": 1, "kind": "claim", "payload": {}})
        with self.assertRaises(ArgumentError):
            BenchmarkTraceAnalyzeArgs.from_wire({"failing": invalid})

    def test_report_separates_causal_localization_from_boundary_and_episode_layers(self) -> None:
        report = benchmark_trace_analysis_report(analysis_payload())
        self.assertIsInstance(report, BenchmarkTraceAnalysisReport)
        self.assertTrue(report.accepted)
        self.assertTrue(report.localized)
        self.assertEqual(report.analysis.first_causal_step, 1)  # type: ignore[union-attr]
        self.assertEqual(report.analysis.textual.kind, "diverged")  # type: ignore[union-attr]
        self.assertEqual(report.boundaries[0].decision_type, "unclassified")
        self.assertTrue(report.boundaries[0].extractable)
        self.assertEqual(report.episodes[0].steps, (0, 1, 2))
        self.assertEqual(report.repetitions[0].classification, "iterative_refinement")
        self.assertEqual(report.summary.repetition_groups, 1)  # type: ignore[union-attr]

    def test_environment_divergence_and_no_divergence_variants_are_not_agent_blame(self) -> None:
        payload = analysis_payload()
        payload["analysis"]["textual"] = {"kind": "diverged", "failing_step": 2, "passing_step": 2, "common_prefix": 2, "failing_did": "result x", "passing_did": "result y", "visibility_gap": []}
        payload["analysis"]["textual_is_actionable"] = False
        payload["analysis"]["verdict"] = {"verdict": "environment_divergence", "at_step": 2, "kind": "result", "nearest_controlled_ancestor": 1}
        report = BenchmarkTraceAnalysisReport.from_wire(payload)
        self.assertFalse(report.localized)
        self.assertTrue(report.analysis.refuses_to_localise)  # type: ignore[union-attr]
        self.assertEqual(report.analysis.verdict.at_step, 2)  # type: ignore[union-attr]

    def test_structured_refusal_is_fail_closed(self) -> None:
        refusal = {"ok": False, "stage": "benchmark_causal_analysis", "refusal": "no decision-bearing step", "fail_closed": True, "guarantees": ["no fabricated cell"]}
        report = benchmark_trace_analysis_report(refusal)
        self.assertTrue(report.refused)
        self.assertTrue(report.fail_closed)
        self.assertEqual(report.refusal, "no decision-bearing step")

    def test_http_and_mcp_envelopes_parse_and_all_facades_delegate(self) -> None:
        envelope = {"ok": True, "tool": "benchmark_trace_analyze", "mcp": {"result": {"structuredContent": analysis_payload()}}}
        self.assertEqual(benchmark_trace_analysis_report(envelope).trace_id, "failed-run")
        request = BenchmarkTraceAnalyzeArgs.from_wire({"failing": failing_trace(), "reference": reference_trace()})
        sync_report = Workspace(_SyncTool(analysis_payload())).benchmark_trace_analysis_report(request)
        self.assertEqual(sync_report.extractable_boundary_count, 1)
        async_report = asyncio.run(AsyncWorkspace(_AsyncTool(analysis_payload())).benchmark_trace_analysis_report(request))
        self.assertEqual(async_report.episodes[0].label, "solve")
        with patch.object(ApiClient, "call_tool", return_value=analysis_payload()) as call:
            report = ApiClient("http://127.0.0.1:1").benchmark_trace_analysis_report(request)
        self.assertEqual(report.reference_trace_id, "reference-run")
        call.assert_called_once_with("benchmark_trace_analyze", request.to_mcp_arguments())

        async def run() -> None:
            with patch.object(ApiClient, "call_tool", return_value=analysis_payload()) as async_call:
                report = await AsyncApiClient(ApiClient("http://127.0.0.1:1")).benchmark_trace_analysis_report(request)
            self.assertEqual(report.analysis.verdict.kind, "first_causal")  # type: ignore[union-attr]
            async_call.assert_called_once_with("benchmark_trace_analyze", request.to_mcp_arguments())

        asyncio.run(run())

    def test_summary_count_mismatch_is_rejected(self) -> None:
        payload = analysis_payload()
        payload["summary"]["episode_count"] = 99
        with self.assertRaises(ArgumentError):
            BenchmarkTraceAnalysisReport.from_wire(payload)
