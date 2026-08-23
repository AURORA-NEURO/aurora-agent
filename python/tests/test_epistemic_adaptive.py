from __future__ import annotations

import asyncio
import json
import unittest
from unittest.mock import patch

from prism_sdk import (
    ApiClient,
    AsyncApiClient,
    AsyncWorkspace,
    EpistemicAdaptiveArgs,
    EpistemicAdaptiveReport,
    Workspace,
    epistemic_adaptive_report,
)
from prism_sdk.errors import ArgumentError
from prism_sdk.models import ToolResult


def problem() -> dict:
    return {
        "actions": ["choose-m0", "choose-m1"],
        "models": ["m0", "m1"],
        "loss": [0.0, 1.0, 1.0, 0.0],
    }


def acquisitions() -> list[dict]:
    return [
        {
            "id": "screen",
            "cost": 0.01,
            "outcomes": [
                {"label": "positive", "likelihood": [0.9, 0.2]},
                {"label": "negative", "likelihood": [0.1, 0.8]},
            ],
        },
        {
            "id": "confirm",
            "cost": 0.1,
            "outcomes": [
                {"label": "positive", "likelihood": [0.01, 0.99]},
                {"label": "negative", "likelihood": [0.99, 0.01]},
            ],
        },
    ]


def adaptive_payload() -> dict:
    stop = {
        "kind": "stop",
        "action_index": 0,
        "action": "choose-m0",
        "risk": 0.1,
    }
    confirm = {
        "kind": "acquire",
        "acquisition_index": 1,
        "id": "confirm",
        "cost": 0.1,
        "expected_total": 0.12,
        "expected_terminal_risk": 0.02,
        "expected_acquisition_cost": 0.1,
        "outcomes": [
            {"label": "positive", "probability": 0.5, "posterior": [0.9, 0.1], "next": {"kind": "stop", "action_index": 0, "action": "choose-m0", "risk": 0.01}},
            {"label": "negative", "probability": 0.5, "posterior": [0.1, 0.9], "next": {"kind": "stop", "action_index": 1, "action": "choose-m1", "risk": 0.03}},
        ],
    }
    root = {
        "kind": "acquire",
        "acquisition_index": 0,
        "id": "screen",
        "cost": 0.01,
        "expected_total": 0.12,
        "expected_terminal_risk": 0.06,
        "expected_acquisition_cost": 0.06,
        "outcomes": [
            {"label": "positive", "probability": 0.5, "posterior": [0.8, 0.2], "next": confirm},
            {"label": "negative", "probability": 0.5, "posterior": [0.2, 0.8], "next": stop},
        ],
    }
    return {
        "ok": True,
        "schema": "bioprism-mcp/epistemic-adaptive-acquisition/0.1",
        "budget": 0.11,
        "max_steps": 2,
        "problem": {
            "actions": ["choose-m0", "choose-m1"],
            "models": ["m0", "m1"],
            "action_count": 2,
            "model_count": 2,
        },
        "acquisitions": [
            {"id": "screen", "cost": 0.01, "outcomes": [{"label": "positive"}, {"label": "negative"}]},
            {"id": "confirm", "cost": 0.1, "outcomes": [{"label": "positive"}, {"label": "negative"}]},
        ],
        "policy": {
            "expected_total": 0.12,
            "expected_terminal_risk": 0.06,
            "expected_acquisition_cost": 0.06,
            "nodes_evaluated": 7,
            "selected_depth": 2,
            "root": root,
        },
        "guarantees": ["exact under explicit caps"],
        "limitations": ["conditional independence is assumed"],
    }


class _SyncTool:
    def __init__(self, payload: dict) -> None:
        self.payload = payload
        self.arguments = None

    def call_tool(self, name: str, arguments: dict) -> ToolResult:
        self.arguments = (name, arguments)
        return ToolResult(name, {"isError": False, "content": [{"type": "text", "text": json.dumps(self.payload)}]})


class _AsyncTool:
    def __init__(self, payload: dict) -> None:
        self.payload = payload
        self.arguments = None

    async def call_tool(self, name: str, arguments: dict) -> ToolResult:
        self.arguments = (name, arguments)
        return ToolResult(name, {"isError": False, "content": [{"type": "text", "text": json.dumps(self.payload)}]})


class EpistemicAdaptiveProjectionTests(unittest.TestCase):
    def request(self) -> EpistemicAdaptiveArgs:
        return EpistemicAdaptiveArgs.from_wire(
            {"problem": problem(), "belief": {"mass": [0.9, 0.1]}, "acquisitions": acquisitions(), "budget": 0.11, "max_steps": 2}
        )

    def test_args_enforce_unique_ids_partitions_and_exact_horizon(self) -> None:
        request = self.request()
        self.assertEqual(request.to_mcp_arguments()["max_steps"], 2)
        self.assertEqual(len(request.acquisitions), 2)
        duplicate = acquisitions()
        duplicate[1]["id"] = "screen"
        with self.assertRaises(ArgumentError):
            EpistemicAdaptiveArgs.from_wire({"problem": problem(), "belief": {"mass": [1, 1]}, "acquisitions": duplicate, "budget": 1.0, "max_steps": 2})
        with self.assertRaises(ArgumentError):
            EpistemicAdaptiveArgs.from_wire({"problem": problem(), "belief": {"mass": [1, 1]}, "acquisitions": acquisitions(), "budget": 1.0, "max_steps": 17})

    def test_report_validates_named_branch_dependent_tree_and_accounting(self) -> None:
        report = epistemic_adaptive_report(adaptive_payload())
        self.assertIsInstance(report, EpistemicAdaptiveReport)
        self.assertTrue(report.accepted)
        self.assertEqual(report.expected_total, 0.12)
        self.assertEqual(report.policy.selected_depth, 2)  # type: ignore[union-attr]
        self.assertTrue(report.branch_dependent)
        self.assertEqual(report.policy.root.id, "screen")  # type: ignore[union-attr]

        broken = adaptive_payload()
        broken["policy"]["expected_total"] = 99.0
        with self.assertRaises(ArgumentError):
            epistemic_adaptive_report(broken)

    def test_refusal_and_mcp_envelopes_remain_typed(self) -> None:
        refusal = {
            "ok": False,
            "schema": "bioprism-mcp/epistemic-adaptive-acquisition/0.1",
            "stage": "adaptive_policy",
            "refusal": "state cap exceeded",
            "fail_closed": True,
            "guarantees": ["refusals are not sampled policies"],
            "limitations": ["conditional independence"],
        }
        report = epistemic_adaptive_report(refusal)
        self.assertTrue(report.refused)
        self.assertTrue(report.fail_closed)
        self.assertEqual(report.refusal.refusal, "state cap exceeded")  # type: ignore[union-attr]
        envelope = {"ok": True, "mcp": {"result": {"structuredContent": adaptive_payload()}}}
        self.assertTrue(epistemic_adaptive_report(envelope).accepted)

    def test_sync_async_workspace_and_http_facades_delegate_the_same_tool(self) -> None:
        request = self.request()
        sync_client = _SyncTool(adaptive_payload())
        sync_report = Workspace(sync_client).epistemic_adaptive_acquisition_report(request)
        self.assertTrue(sync_report.accepted)
        self.assertEqual(sync_client.arguments[0], "epistemic_adaptive_acquisition")  # type: ignore[index]

        async_client = _AsyncTool(adaptive_payload())
        async_report = asyncio.run(AsyncWorkspace(async_client).epistemic_adaptive_acquisition_report(request))
        self.assertTrue(async_report.accepted)
        self.assertEqual(async_client.arguments[0], "epistemic_adaptive_acquisition")  # type: ignore[index]

        with patch.object(ApiClient, "call_tool", return_value=adaptive_payload()) as call:
            report = ApiClient("http://127.0.0.1:1").epistemic_adaptive_acquisition_report(request)
        self.assertEqual(report.expected_total, 0.12)
        call.assert_called_once_with("epistemic_adaptive_acquisition", request.to_mcp_arguments())

        async def run() -> None:
            with patch.object(ApiClient, "call_tool", return_value=adaptive_payload()) as async_call:
                result = await AsyncApiClient(ApiClient("http://127.0.0.1:1")).epistemic_adaptive_acquisition_report(request)
            self.assertTrue(result.branch_dependent)
            async_call.assert_called_once_with("epistemic_adaptive_acquisition", request.to_mcp_arguments())

        asyncio.run(run())
