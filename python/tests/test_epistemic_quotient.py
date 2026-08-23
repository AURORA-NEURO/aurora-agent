from __future__ import annotations

import asyncio
import json
import unittest
from unittest.mock import patch

from prism_sdk import (
    ApiClient,
    AsyncApiClient,
    AsyncWorkspace,
    EpistemicDecisionProblemArgs,
    EpistemicDecisionQuotientArgs,
    EpistemicDecisionQuotientReport,
    Workspace,
    epistemic_decision_quotient_report,
)
from prism_sdk.errors import ArgumentError
from prism_sdk.models import ToolResult


def request() -> dict:
    return {
        "problem": {
            "actions": ["accept", "defer", "reject"],
            "models": ["m-a", "m-b", "m-c"],
            "loss": [0.0, 7.0, 0.0, 4.0, 11.0, 5.0, 8.0, 15.0, 8.0],
        },
        "permitted_actions": ["reject", "accept", "defer"],
    }


def payload() -> dict:
    return {
        "ok": True,
        "schema": "bioprism-mcp/epistemic-decision-quotient/0.1",
        "quotient": {
            "schema_version": "bioprism-epistemic-decision-quotient/0.1",
            "basis": "permitted_loss_difference_profile",
            "permitted_actions": ["accept", "defer", "reject"],
            "original_model_count": 3,
            "quotient_model_count": 2,
            "merged_model_count": 1,
            "model_to_class": {"m-a": 0, "m-b": 0, "m-c": 1},
            "classes": [
                {
                    "class_index": 0,
                    "representative_model": "m-a",
                    "members": ["m-a", "m-b"],
                    "loss_differences": {"accept": 0.0, "defer": 4.0, "reject": 8.0},
                    "preferred_actions": ["accept"],
                },
                {
                    "class_index": 1,
                    "representative_model": "m-c",
                    "members": ["m-c"],
                    "loss_differences": {"accept": 0.0, "defer": 5.0, "reject": 8.0},
                    "preferred_actions": ["accept"],
                },
            ],
        },
        "summary": {
            "original_model_count": 3,
            "quotient_model_count": 2,
            "merged_model_count": 1,
            "compressed": True,
            "compression_fraction": 2 / 3,
        },
        "guarantees": ["loss-difference profiles are exact"],
        "limitations": ["decision-relative only"],
    }


class _SyncTool:
    def call_tool(self, name: str, arguments: dict) -> ToolResult:
        return ToolResult(name, {"isError": False, "content": [{"type": "text", "text": json.dumps(payload())}]})


class _AsyncTool:
    async def call_tool(self, name: str, arguments: dict) -> ToolResult:
        return ToolResult(name, {"isError": False, "content": [{"type": "text", "text": json.dumps(payload())}]})


class EpistemicQuotientTests(unittest.TestCase):
    def test_args_validate_action_boundary_and_preserve_problem(self) -> None:
        parsed = EpistemicDecisionQuotientArgs.from_wire(request())
        self.assertIsInstance(parsed.problem, EpistemicDecisionProblemArgs)
        self.assertEqual(parsed.to_mcp_arguments()["permitted_actions"], ["reject", "accept", "defer"])
        with self.assertRaises(ArgumentError):
            EpistemicDecisionQuotientArgs.from_wire({**request(), "permitted_actions": []})
        with self.assertRaises(ArgumentError):
            EpistemicDecisionQuotientArgs.from_wire({**request(), "permitted_actions": ["accept", "accept"]})
        with self.assertRaises(ArgumentError):
            EpistemicDecisionQuotientArgs.from_wire({**request(), "permitted_actions": ["missing"]})

    def test_report_validates_exact_classes_and_compression(self) -> None:
        report = epistemic_decision_quotient_report(payload())
        self.assertIsInstance(report, EpistemicDecisionQuotientReport)
        self.assertTrue(report.accepted)
        self.assertEqual(report.quotient_model_count, 2)
        self.assertEqual(report.classes[0].members, ("m-a", "m-b"))
        self.assertTrue(report.compressed)
        broken = payload()
        broken["quotient"]["merged_model_count"] = 0
        with self.assertRaises(ArgumentError):
            epistemic_decision_quotient_report(broken)

    def test_sync_async_http_and_envelope_facades_are_typed(self) -> None:
        parsed = EpistemicDecisionQuotientArgs.from_wire(request())
        self.assertTrue(Workspace(_SyncTool()).epistemic_decision_quotient_report(parsed).accepted)
        self.assertTrue(asyncio.run(AsyncWorkspace(_AsyncTool()).epistemic_decision_quotient_report(parsed)).accepted)
        envelope = {"ok": True, "tool": "epistemic_decision_quotient", "mcp": {"result": {"structuredContent": payload()}}}
        self.assertEqual(epistemic_decision_quotient_report(envelope).merged_model_count, 1)
        with patch.object(ApiClient, "call_tool", return_value=payload()) as call:
            report = ApiClient("http://127.0.0.1:1").epistemic_decision_quotient_report(parsed)
        self.assertTrue(report.accepted)
        call.assert_called_once_with("epistemic_decision_quotient", parsed.to_mcp_arguments())

        async def run() -> None:
            with patch.object(ApiClient, "call_tool", return_value=payload()) as async_call:
                report = await AsyncApiClient(ApiClient("http://127.0.0.1:1")).epistemic_decision_quotient_report(parsed)
            self.assertEqual(report.original_model_count, 3)
            async_call.assert_called_once_with("epistemic_decision_quotient", parsed.to_mcp_arguments())

        asyncio.run(run())

    def test_structured_refusal_remains_refused(self) -> None:
        refusal = {
            "ok": False,
            "schema": "bioprism-mcp/epistemic-decision-quotient/0.1",
            "stage": "decision_quotient",
            "refusal": "unknown permitted action",
            "fail_closed": True,
            "guarantees": ["no default action set"],
        }
        report = epistemic_decision_quotient_report(refusal)
        self.assertTrue(report.refused)
        self.assertIsNotNone(report.refusal)
