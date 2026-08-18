from __future__ import annotations

import asyncio
import json
import unittest
from unittest.mock import patch

from prism_sdk import (
    AdaptiveCostedReport,
    AdaptiveCostedRequest,
    AdaptiveExecutionReport,
    AdaptiveExecutionRequest,
    ApiClient,
    AsyncApiClient,
    AsyncWorkspace,
    Workspace,
    adaptive_execution_report,
    adaptive_costed_report,
)
from prism_sdk.errors import ArgumentError
from prism_sdk.models import ToolResult


def request_wire() -> dict:
    return {
        "problem": {"actions": ["m0", "m1"], "models": ["m0", "m1"], "loss": [0.0, 1.0, 1.0, 0.0]},
        "belief": {"mass": [0.9, 0.1]},
        "acquisitions": [{
            "id": "screen",
            "cost": 0.01,
            "outcomes": [
                {"label": "positive", "likelihood": [0.9, 0.2]},
                {"label": "negative", "likelihood": [0.1, 0.8]},
            ],
        }],
        "budget": 0.1,
        "max_steps": 1,
        "authorization": {"grant_id": "g-1", "provider": "mcp-simulated"},
        "observations": [{"acquisition_id": "screen", "outcome_label": "negative"}],
    }


def payload() -> dict:
    digest = "a" * 64
    return {
        "ok": True,
        "schema": "bioprism-epistemic/adaptive-execution/0.1",
        "mode": "simulate",
        "completed": True,
        "provenance_counts": {"observed": 0, "simulated": 1, "replayed": 0},
        "receipt": {
            "schema": "bioprism-epistemic/adaptive-execution/0.1",
            "plan_digest": digest,
            "provider": "mcp-simulated",
            "status": "completed",
            "authorization": {"granted": True, "grant_id": "g-1", "provider": "mcp-simulated"},
            "observations": [{
                "sequence": 0,
                "request": {"plan_digest": digest, "sequence": 0, "acquisition_id": "screen", "declared_cost": 0.01},
                "observation": {
                    "provider": "mcp-simulated",
                    "acquisition_id": "screen",
                    "outcome_label": "negative",
                    "evidence_digest": "b" * 64,
                    "provenance": "simulated",
                },
            }],
            "actual_acquisition_cost": 0.01,
            "terminal_action": 0,
            "terminal_risk": 0.1,
            "refusal": None,
            "refusal_detail": None,
        },
    }


class _SyncTool:
    def __init__(self) -> None:
        self.arguments = None

    def call_tool(self, name: str, arguments: dict) -> ToolResult:
        self.arguments = (name, arguments)
        return ToolResult(name, {"isError": False, "content": [{"type": "text", "text": json.dumps(payload())}]})


class _AsyncTool:
    def __init__(self) -> None:
        self.arguments = None

    async def call_tool(self, name: str, arguments: dict) -> ToolResult:
        self.arguments = (name, arguments)
        return ToolResult(name, {"isError": False, "content": [{"type": "text", "text": json.dumps(payload())}]})


class AdaptiveExecutionTests(unittest.TestCase):
    def test_costed_request_and_report_preserve_all_dimensions(self) -> None:
        dimensions = {
            "tokens": 100.0,
            "compute_ms": 100.0,
            "latency_ms": 10.0,
            "money_usd": 1.0,
            "privacy_loss": 1.0,
            "specimen_units": 1.0,
            "expert_minutes": 10.0,
        }
        request = AdaptiveCostedRequest.from_wire({
            "problem": {"actions": ["m0"], "models": ["m0"], "loss": [0.0]},
            "belief": {"mass": [1.0]},
            "acquisitions": [{
                "acquisition": {"id": "screen", "cost": 0.1, "outcomes": [{"label": "negative", "likelihood": [1.0]}]},
                "cost": {**dimensions, "latency_ms": 100.0},
            }],
            "budget": dimensions,
            "weights": {"tokens": 0.0, "compute_ms": 0.0, "latency_ms": 1.0, "money_usd": 0.0, "privacy_loss": 0.0, "specimen_units": 0.0, "expert_minutes": 0.0},
            "max_steps": 1,
        })
        self.assertEqual(request.to_mcp_arguments()["budget"]["latency_ms"], 10.0)
        report = adaptive_costed_report({
            "ok": True,
            "schema": "bioprism-mcp/epistemic-adaptive-costed/0.1",
            "cost_dimensions": ["tokens", "compute_ms", "latency_ms", "money_usd", "privacy_loss", "specimen_units", "expert_minutes"],
            "policy": {"expected_scalarized_cost": 0.0},
            "guarantees": [],
        })
        self.assertIsInstance(report, AdaptiveCostedReport)
        self.assertTrue(report.ok)
        self.assertEqual(report.cost_dimensions[2], "latency_ms")

    def test_request_and_report_preserve_simulated_provenance(self) -> None:
        request = AdaptiveExecutionRequest.from_wire(request_wire())
        self.assertEqual(request.to_mcp_arguments()["mode"], "simulate")
        report = adaptive_execution_report(payload())
        self.assertIsInstance(report, AdaptiveExecutionReport)
        self.assertTrue(report.completed)
        self.assertEqual(report.provenance_counts["simulated"], 1)
        self.assertEqual(report.observations[0].outcome_label, "negative")

    def test_request_refuses_missing_replay_receipt_and_report_refuses_forged_counts(self) -> None:
        broken = request_wire()
        broken["mode"] = "replay"
        broken.pop("receipt", None)
        with self.assertRaises(ArgumentError):
            AdaptiveExecutionRequest.from_wire(broken)
        forged = payload()
        forged["provenance_counts"]["simulated"] = 0
        with self.assertRaises(ArgumentError):
            adaptive_execution_report(forged)

    def test_sync_async_workspace_and_http_facades_use_execution_tool(self) -> None:
        request = AdaptiveExecutionRequest.from_wire(request_wire())
        sync_tool = _SyncTool()
        self.assertTrue(Workspace(sync_tool).epistemic_adaptive_execute_report(request).completed)
        self.assertEqual(sync_tool.arguments[0], "epistemic_adaptive_execute")  # type: ignore[index]
        async_tool = _AsyncTool()
        self.assertTrue(asyncio.run(AsyncWorkspace(async_tool).epistemic_adaptive_execute_report(request)).completed)
        self.assertEqual(async_tool.arguments[0], "epistemic_adaptive_execute")  # type: ignore[index]
        with patch.object(ApiClient, "call_tool", return_value=payload()) as call:
            self.assertTrue(ApiClient("http://127.0.0.1:1").epistemic_adaptive_execute_report(request).completed)
        call.assert_called_once_with("epistemic_adaptive_execute", request.to_mcp_arguments())
