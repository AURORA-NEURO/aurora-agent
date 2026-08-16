from __future__ import annotations

import asyncio
import json
import unittest
from unittest.mock import patch

from prism_sdk import (
    ApiClient,
    AsyncApiClient,
    AsyncWorkspace,
    PosteriorGateArgs,
    PosteriorGateReport,
    Workspace,
    posterior_gate_report,
)
from prism_sdk.errors import ArgumentError
from prism_sdk.models import ToolResult


def request() -> PosteriorGateArgs:
    return PosteriorGateArgs(
        observations=[{"capability": "capability-a", "parent": "parent-1", "result": {"conclusion": "pass"}}],
        credit_policy={"unsupported_ceiling": 0.5, "contradicted_ceiling": 0.25, "unknown_credit": "leave"},
        gate={"gate": "release-a", "rationale": "named release decision", "formula": "weighted mean", "floors": {}},
        other_observations=[],
        tolerance=0.01,
        min_effective=2.0,
    )


def estimate(label: str, mean: float) -> dict:
    return {
        "label": label,
        "mean": mean,
        "naive_instance_mean": mean,
        "instances": 2,
        "clusters": 2,
        "largest_cluster": 1,
        "icc": {"icc": "not_applicable"},
        "effective_sample_size": 2.0,
        "unknown_instances": 0,
        "unknown_fraction": 0.0,
    }


def capability(name: str = "capability-a") -> dict:
    return {
        "capability": name,
        "pass_rate": estimate(f"{name}::pass_rate", 0.75),
        "credit": estimate(f"{name}::credit", 0.75),
        "outcome_rate": estimate(f"{name}::outcome_rate", 0.9),
        "vetoes": [],
        "disputed": 1,
        "abstained": 0,
        "optimistic_weak_evidence": 1,
        "weakest_tier": "execution",
    }


def payload() -> dict:
    return {
        "ok": True,
        "schema": "bioprism-mcp/posterior-gate/0.1",
        "schema_version": "07.0.1",
        "observations": 4,
        "unprovenanced_observations": 2,
        "capabilities": {"capability-a": capability()},
        "gate": {
            "ok": True,
            "value": {
                "gate": "release-a",
                "value": 0.75,
                "formula": "weighted mean",
                "rationale": "named release decision",
                "terms": [["capability-a", 0.75, 1.0]],
                "sensitivity": [["capability-a", 0.75]],
                "weakest_tier": "execution",
                "min_effective_sample": 2.0,
            },
        },
        "comparison": {
            "ok": True,
            "dominance": {"dominance": "incomparable", "better": ["capability-a"], "worse": [], "uncertain": ["capability-b"]},
            "tolerance": 0.01,
            "min_effective": 2.0,
        },
        "guarantees": ["vectors remain separate from release scalars"],
        "limitations": ["point estimates are not posterior distributions"],
    }


def refusal_payload() -> dict:
    return {
        "ok": False,
        "schema": "bioprism-mcp/posterior-gate/0.1",
        "stage": "credit_policy",
        "refusal": "unsupported and contradicted credit ceilings must both be finite values in [0,1)",
        "fail_closed": True,
    }


class _SyncTool:
    def __init__(self, value: dict | None = None) -> None:
        self.value = value or payload()

    def call_tool(self, name: str, arguments: dict) -> ToolResult:
        return ToolResult(name, {"isError": False, "content": [{"type": "text", "text": json.dumps(self.value)}]})


class _AsyncTool(_SyncTool):
    async def call_tool(self, name: str, arguments: dict) -> ToolResult:
        return ToolResult(name, {"isError": False, "content": [{"type": "text", "text": json.dumps(self.value)}]})


class PosteriorGateTests(unittest.TestCase):
    def test_request_keeps_policies_and_comparison_controls_bounded(self) -> None:
        args = request()
        self.assertEqual(args.to_mcp_arguments()["observations"][0]["capability"], "capability-a")
        self.assertEqual(args.to_mcp_arguments()["min_effective"], 2.0)
        with self.assertRaises(ArgumentError):
            PosteriorGateArgs([], tolerance=-1.0)
        with self.assertRaises(ArgumentError):
            PosteriorGateArgs([{}] * 10_001)

    def test_report_keeps_vector_scalar_sensitivity_and_incomparability(self) -> None:
        report = posterior_gate_report(payload())
        self.assertIsInstance(report, PosteriorGateReport)
        self.assertEqual(report.capabilities["capability-a"].pass_rate.effective_sample_size, 2.0)
        self.assertAlmostEqual(report.capabilities["capability-a"].unsupported_pass_gap, 0.15)
        self.assertEqual(report.capabilities["capability-a"].disputed, 1)
        self.assertTrue(report.release_is_eligible)
        self.assertEqual(report.gate.value.largest_sensitivity, 0.0)
        self.assertTrue(report.comparison_is_incomparable)
        self.assertTrue(report.has_provenance_gaps)
        envelope = {"ok": True, "mcp": {"result": {"structuredContent": payload()}}}
        self.assertEqual(posterior_gate_report(envelope).schema, "bioprism-mcp/posterior-gate/0.1")

    def test_refusal_is_typed_and_fail_closed(self) -> None:
        report = posterior_gate_report(refusal_payload())
        self.assertFalse(report.ok)
        self.assertEqual(report.stage, "credit_policy")
        self.assertTrue(report.fail_closed)
        self.assertFalse(report.release_is_eligible)

    def test_all_python_facades_return_typed_posterior_reports(self) -> None:
        args = request()
        self.assertTrue(asyncio.run(AsyncWorkspace(_AsyncTool()).posterior_gate_report(args)).release_is_eligible)
        self.assertTrue(Workspace(_SyncTool()).posterior_gate_report(args).has_provenance_gaps)
        with patch.object(ApiClient, "call_tool", return_value=payload()) as call:
            report = ApiClient("http://127.0.0.1:1").posterior_gate_report(args)
        self.assertTrue(report.comparison_is_incomparable)
        call.assert_called_once_with("posterior_gate", args.to_mcp_arguments())

        async def run() -> None:
            with patch.object(ApiClient, "call_tool", return_value=payload()) as async_call:
                result = await AsyncApiClient(ApiClient("http://127.0.0.1:1")).posterior_gate_report(args)
            self.assertEqual(result.schema, "bioprism-mcp/posterior-gate/0.1")
            async_call.assert_called_once_with("posterior_gate", args.to_mcp_arguments())

        asyncio.run(run())


if __name__ == "__main__":
    unittest.main()
