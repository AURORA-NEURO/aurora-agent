from __future__ import annotations

import asyncio
import json
import unittest
from unittest.mock import patch

from prism_sdk import ApiClient, AsyncApiClient, AsyncWorkspace, HubCardRenderArgs, HubCardRenderReport, Workspace, hub_card_render
from prism_sdk.errors import ArgumentError
from prism_sdk.models import ToolResult


def request() -> HubCardRenderArgs:
    return HubCardRenderArgs(moderation={"records": {}}, submission="sub-card", score={"value": 0.82, "interval": None}, pack="pack-1", computed_at=4, disclosure={"packs": {}})


def card(score: dict) -> dict:
    return {
        "resource_type": "bioatlas-card",
        "resource_id": "digest-card",
        "version": "bioatlas-card/0.1",
        "submission": "sub-card",
        "scope": {"decision_family": ["ranking"]},
        "provenance": ["digest-parent"],
        "access": "public",
        "state": "available",
        "verification": "self-reported",
        "score": score,
        "non_claims": [{"kind": "clinical-validity"}],
        "attributions": [{"name": "AURORA"}],
        "limitations": "This card is bounded to its declared scope.",
    }


def payload() -> dict:
    return {
        "ok": True,
        "schema": "bioprism-mcp/hub-card/0.1",
        "card": card({"display": "published", "score": {"value": 0.82, "interval": None}, "label": {"label": "held_out"}}),
        "score": {"attached": True, "pack": "pack-1", "computed_at": 4},
        "moderation_state": "accepted",
        "verification": "self-reported",
        "guarantees": [
            "the card state is derived from moderation history, verification, withdrawal, supersession, and access terms",
            "a card starts with a withheld score and never uses zero or blank as a failure state",
            "scores require disclosure eligibility and an available publication state",
            "the result is a renderer-facing object; it does not render HTML, resolve links, or publish a page",
        ],
    }


class _SyncTool:
    def call_tool(self, name: str, arguments: dict) -> ToolResult:
        return ToolResult(name, {"isError": False, "content": [{"type": "text", "text": json.dumps(payload())}]})


class _AsyncTool:
    async def call_tool(self, name: str, arguments: dict) -> ToolResult:
        return ToolResult(name, {"isError": False, "content": [{"type": "text", "text": json.dumps(payload())}]})


class HubCardProjectionTests(unittest.TestCase):
    def test_args_preserve_disclosure_gate_inputs_and_bounds(self) -> None:
        args = request()
        self.assertEqual(args.to_mcp_arguments()["computed_at"], 4)
        with self.assertRaises(ArgumentError):
            HubCardRenderArgs({}, "", score={"value": 0.2})

    def test_report_distinguishes_published_number_from_withheld_card(self) -> None:
        report = hub_card_render(payload())
        self.assertIsInstance(report, HubCardRenderReport)
        self.assertTrue(report.numeric_score_exposed)
        self.assertFalse(report.score_withheld)
        self.assertEqual(report.card.score.numeric_value, 0.82)
        self.assertEqual(report.card.score.label.kind, "held_out")
        self.assertTrue(report.state_gate_is_visible)
        self.assertTrue(report.withholding_is_not_zero)
        self.assertTrue(report.renderer_is_not_a_publisher)

        withheld = dict(payload())
        withheld["card"] = card({"display": "withheld", "state": "controlled", "why": "under access control"})
        withheld["score"] = {"attached": False}
        withheld_report = hub_card_render(withheld)
        self.assertTrue(withheld_report.score_withheld)
        self.assertFalse(withheld_report.numeric_score_exposed)
        self.assertEqual(withheld_report.card.score.state, "controlled")

    def test_fail_closed_card_refusal_and_all_python_facades(self) -> None:
        refused = {
            "ok": False,
            "schema": "bioprism-mcp/hub-card/0.1",
            "stage": "card_disclosure_gate",
            "refusal": "pack is unknown",
            "fail_closed": True,
            "card": card({"display": "withheld", "state": "available", "why": "no score"}),
            "score": None,
        }
        report = hub_card_render(refused)
        self.assertFalse(report.ok)
        self.assertTrue(report.fail_closed)
        self.assertEqual(report.stage, "card_disclosure_gate")
        args = request()
        self.assertTrue(Workspace(_SyncTool()).hub_card_render_report(args).numeric_score_exposed)
        self.assertTrue(asyncio.run(AsyncWorkspace(_AsyncTool()).hub_card_render_report(args)).numeric_score_exposed)
        with patch.object(ApiClient, "call_tool", return_value=payload()) as call:
            self.assertTrue(ApiClient("http://127.0.0.1:1").hub_card_render_report(args).numeric_score_exposed)
        call.assert_called_once_with("hub_card_render", args.to_mcp_arguments())

        async def run() -> None:
            with patch.object(ApiClient, "call_tool", return_value=payload()) as async_call:
                report = await AsyncApiClient(ApiClient("http://127.0.0.1:1")).hub_card_render_report(args)
            self.assertEqual(report.schema, "bioprism-mcp/hub-card/0.1")
            async_call.assert_called_once_with("hub_card_render", args.to_mcp_arguments())

        asyncio.run(run())


if __name__ == "__main__":
    unittest.main()
