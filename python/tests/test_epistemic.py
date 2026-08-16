from __future__ import annotations

import asyncio
import json
import unittest
from unittest.mock import patch

from prism_sdk import (
    ApiClient,
    AsyncWorkspace,
    AsyncApiClient,
    EpistemicVoiArgs,
    EpistemicVoiReport,
    Workspace,
    epistemic_voi_report,
)
from prism_sdk.errors import ArgumentError
from prism_sdk.models import ToolResult


def problem() -> dict:
    return {
        "actions": ["treat", "abstain"],
        "models": ["responsive", "resistant"],
        "loss": [0.0, 10.0, 10.0, 0.0],
    }


def acquisition(identifier: str = "assay", cost: float = 0.1) -> dict:
    return {
        "id": identifier,
        "cost": cost,
        "outcomes": [
            {"label": "positive", "likelihood": [0.9, 0.1]},
            {"label": "negative", "likelihood": [0.1, 0.9]},
        ],
    }


def single_payload() -> dict:
    return {
        "ok": True,
        "mode": "single",
        "value": {
            "gross": 4.0,
            "cost": 0.1,
            "net": 3.9,
            "outcome_probabilities": [0.5, 0.5],
            "action_without": 0,
            "action_after": [0, 1],
        },
        "actions": {"without": "treat", "after": ["treat", "abstain"]},
        "complementarity": None,
        "guarantees": [
            "gross risk reduction and declared acquisition cost remain separate",
            "action changes are reported by action identity rather than rounded numeric value",
        ],
    }


def bundle_payload() -> dict:
    payload = single_payload()
    payload.update(
        {
            "mode": "non_adaptive_joint_bundle",
            "value": {
                "gross": 5.0,
                "cost": 0.2,
                "net": 4.8,
                "outcome_probabilities": [0.25, 0.25, 0.25, 0.25],
                "action_without": 0,
                "action_after": [0, 1, 1, 0],
            },
            "actions": {"without": "treat", "after": ["treat", "abstain", "abstain", "treat"]},
            "complementarity": {"joint_gross": 5.0, "sum_of_singletons": 3.0, "excess": 2.0},
        }
    )
    return payload


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


class EpistemicProjectionTests(unittest.TestCase):
    def test_args_preserve_explicit_problem_and_reject_improper_partitions(self) -> None:
        request = EpistemicVoiArgs.from_wire({"problem": problem(), "belief": {"mass": [1, 1]}, "acquisition": acquisition()})
        wire = request.to_mcp_arguments()
        self.assertEqual(wire["problem"]["loss"], [0.0, 10.0, 10.0, 0.0])
        self.assertEqual(wire["belief"], {"mass": [1.0, 1.0]})
        self.assertEqual(wire["acquisition"]["id"], "assay")
        invalid = acquisition()
        invalid["outcomes"][1]["likelihood"] = [0.2, 0.2]
        with self.assertRaises(ArgumentError):
            EpistemicVoiArgs.from_wire({"problem": problem(), "belief": {"mass": [0.5, 0.5]}, "acquisition": invalid})

    def test_single_report_separates_gross_cost_net_and_action_change(self) -> None:
        report = epistemic_voi_report(single_payload())
        self.assertIsInstance(report, EpistemicVoiReport)
        self.assertTrue(report.accepted)
        self.assertEqual(report.gross_value, 4.0)
        self.assertEqual(report.declared_cost, 0.1)
        self.assertEqual(report.net_value, 3.9)
        self.assertTrue(report.action_changed)
        self.assertTrue(report.worth_acquiring)
        self.assertFalse(report.is_bundle)

    def test_bundle_report_preserves_complementarity_and_repeated_action_identities(self) -> None:
        report = EpistemicVoiReport.from_wire(bundle_payload())
        self.assertTrue(report.non_adaptive)
        self.assertEqual(report.actions.after, ("treat", "abstain", "abstain", "treat"))
        self.assertIsNotNone(report.complementarity)
        assert report.complementarity is not None
        self.assertTrue(report.complementarity_detected)

    def test_structured_refusal_is_typed_and_fail_closed(self) -> None:
        refusal = {
            "ok": False,
            "stage": "value_of_information",
            "refusal": "improper likelihood partition",
            "fail_closed": True,
            "guarantees": ["gross value is not reported as net value"],
        }
        report = epistemic_voi_report(refusal)
        self.assertTrue(report.refused)
        self.assertTrue(report.fail_closed)
        self.assertEqual(report.refusal.refusal, "improper likelihood partition")  # type: ignore[union-attr]
        self.assertIsNone(report.gross_value)

    def test_mcp_and_http_envelopes_are_both_parseable(self) -> None:
        envelope = {"ok": True, "tool": "epistemic_voi", "mcp": {"result": {"structuredContent": single_payload()}}}
        self.assertEqual(epistemic_voi_report(envelope).actions.without, "treat")  # type: ignore[union-attr]

    def test_sync_and_async_workspaces_preserve_structured_refusal(self) -> None:
        sync_client = _SyncTool(single_payload())
        sync_report = Workspace(sync_client).epistemic_voi_report(EpistemicVoiArgs.from_wire({"problem": problem(), "belief": {"mass": [0.5, 0.5]}, "acquisition": acquisition()}))
        self.assertEqual(sync_report.mode, "single")
        self.assertEqual(sync_client.arguments[0], "epistemic_voi")  # type: ignore[index]

        async_client = _AsyncTool({"ok": False, "stage": "value_of_information", "refusal": "outcome cap", "fail_closed": True, "guarantees": []})
        report = asyncio.run(AsyncWorkspace(async_client).epistemic_voi_report({"problem": problem(), "belief": {"mass": [0.5, 0.5]}, "acquisition": acquisition()}))
        self.assertTrue(report.refused)
        self.assertEqual(async_client.arguments[0], "epistemic_voi")  # type: ignore[index]

    def test_http_sync_and_async_facades_delegate_the_same_typed_boundary(self) -> None:
        request = EpistemicVoiArgs.from_wire({"problem": problem(), "belief": {"mass": [0.5, 0.5]}, "acquisition": acquisition()})
        with patch.object(ApiClient, "call_tool", return_value=single_payload()) as call:
            report = ApiClient("http://127.0.0.1:1").epistemic_voi_report(request)
        self.assertEqual(report.net_value, 3.9)
        call.assert_called_once_with("epistemic_voi", request.to_mcp_arguments())

        async def run() -> None:
            with patch.object(ApiClient, "call_tool", return_value=single_payload()) as async_call:
                report = await AsyncApiClient(ApiClient("http://127.0.0.1:1")).epistemic_voi_report(request)
            self.assertTrue(report.action_changed)
            async_call.assert_called_once_with("epistemic_voi", request.to_mcp_arguments())

        asyncio.run(run())

    def test_report_rejects_a_net_value_that_does_not_reconcile(self) -> None:
        payload = single_payload()
        payload["value"]["net"] = 100.0
        with self.assertRaises(ArgumentError):
            EpistemicVoiReport.from_wire(payload)
