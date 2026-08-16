from __future__ import annotations

import asyncio
import json
import unittest
from unittest.mock import patch

from prism_sdk import (
    ApiClient,
    AsyncApiClient,
    AsyncWorkspace,
    HubDisclosureReviewArgs,
    HubDisclosureReviewReport,
    Workspace,
    hub_disclosure_review,
)
from prism_sdk.errors import ArgumentError
from prism_sdk.models import ToolResult


PACK = "sha256:held-out-pack"
CONTAMINATED = "sha256:contaminated-pack"


def request() -> HubDisclosureReviewArgs:
    return HubDisclosureReviewArgs(
        actions=[
            {"kind": "declare_held_out", "pack": PACK},
            {"kind": "disclose", "pack": PACK, "at": 5},
            {"kind": "headline_eligibility", "pack": PACK, "computed_at": 6},
            {"kind": "headline_eligibility", "pack": PACK, "computed_at": 6, "acknowledges_disclosure": True},
            {"kind": "contaminate", "pack": CONTAMINATED, "witness": {"kind": "training_corpus_overlap", "detail": "public instances are in the training snapshot", "observed_at": 7, "reported_by": "audit-1"}},
        ]
    )


def payload() -> dict:
    held_out = {"disclosure": "held_out"}
    disclosed = {"disclosure": "disclosed", "since": 5}
    contaminated = {"disclosure": "contaminated", "witness": {"kind": "training_corpus_overlap", "detail": "public instances are in the training snapshot", "observed_at": 7, "reported_by": "audit-1"}}
    return {
        "ok": False,
        "schema": "bioprism-mcp/hub-disclosure/0.1",
        "action_count": 7,
        "action_failures": 1,
        "trace": [
            {"index": 0, "kind": "declare_held_out", "ok": True, "result": {"pack": PACK, "state": held_out}},
            {"index": 1, "kind": "disclose", "ok": True, "result": {"pack": PACK, "state": disclosed}},
            {"index": 2, "kind": "headline_eligibility", "ok": True, "result": {"pack": PACK, "eligible": False, "refusal": "disclosure is not acknowledged", "fail_closed": True}},
            {"index": 3, "kind": "headline_eligibility", "ok": True, "result": {"pack": PACK, "eligible": True, "label": {"label": "disclosed_pack", "disclosed_at": 5, "caveat": "This is a visible benchmark caveat."}}},
            {"index": 4, "kind": "contaminate", "ok": True, "result": {"pack": CONTAMINATED, "state": contaminated}},
            {"index": 5, "kind": "headline_eligibility", "ok": True, "result": {"pack": CONTAMINATED, "eligible": False, "refusal": "pack is contaminated", "fail_closed": True}},
            {"index": 6, "kind": "unknown_action", "ok": False, "refusal": "unknown disclosure action", "fail_closed": True},
        ],
        "entries": [{"pack": PACK, "state": disclosed}, {"pack": CONTAMINATED, "state": contaminated}],
        "ledger": {"packs": {PACK: disclosed, CONTAMINATED: contaminated}},
        "guarantees": [
            "disclosure is keyed by immutable pack digest rather than a mutable name",
            "unknown, held-out, disclosed, and contaminated remain distinct states",
            "disclosure is a ratchet and contamination cannot be walked back",
            "headline eligibility returns a caveat or a typed refusal instead of a bare score",
            "the review is in-memory and records supplied findings; it does not detect leaks or publish data",
        ],
    }


class _SyncTool:
    def call_tool(self, name: str, arguments: dict) -> ToolResult:
        return ToolResult(name, {"isError": False, "content": [{"type": "text", "text": json.dumps(payload())}]})


class _AsyncTool:
    async def call_tool(self, name: str, arguments: dict) -> ToolResult:
        return ToolResult(name, {"isError": False, "content": [{"type": "text", "text": json.dumps(payload())}]})


class HubDisclosureProjectionTests(unittest.TestCase):
    def test_args_are_bounded_and_keep_opaque_ordered_actions(self) -> None:
        args = request()
        self.assertEqual(args.to_mcp_arguments()["actions"][1]["kind"], "disclose")
        with self.assertRaises(ArgumentError):
            HubDisclosureReviewArgs([{} for _ in range(257)])

    def test_report_preserves_ratchet_caveat_withholding_and_fail_closed_refusal(self) -> None:
        report = hub_disclosure_review(payload())
        self.assertIsInstance(report, HubDisclosureReviewReport)
        self.assertEqual(report.disclosed_count, 1)
        self.assertEqual(report.contaminated_count, 1)
        self.assertEqual(report.headline_check_count, 3)
        self.assertEqual(report.headline_eligible_count, 1)
        self.assertEqual(report.headline_withheld_count, 2)
        self.assertEqual(report.fail_closed_refusal_count, 1)
        self.assertEqual(report.trace[3].label.kind, "disclosed_pack")
        self.assertEqual(report.trace[4].state.witness.kind, "training_corpus_overlap")
        self.assertTrue(report.digest_bound)
        self.assertTrue(report.ratchet_is_explicit)
        self.assertTrue(report.caveats_are_required)
        self.assertTrue(report.leak_detection_is_not_claimed)
        self.assertTrue(report.all_refusals_are_fail_closed)

    def test_mcp_http_and_all_python_facades_delegate(self) -> None:
        envelope = {"ok": True, "tool": "hub_disclosure_review", "mcp": {"result": {"structuredContent": payload()}}}
        self.assertEqual(hub_disclosure_review(envelope).ledger.state_for(PACK).kind, "disclosed")
        args = request()
        self.assertEqual(Workspace(_SyncTool()).hub_disclosure_review_report(args).headline_withheld_count, 2)
        self.assertEqual(asyncio.run(AsyncWorkspace(_AsyncTool()).hub_disclosure_review_report(args)).contaminated_count, 1)
        with patch.object(ApiClient, "call_tool", return_value=payload()) as call:
            report = ApiClient("http://127.0.0.1:1").hub_disclosure_review_report(args)
        self.assertEqual(report.split_integrity_failure_count, 0)
        call.assert_called_once_with("hub_disclosure_review", args.to_mcp_arguments())

        async def run() -> None:
            with patch.object(ApiClient, "call_tool", return_value=payload()) as async_call:
                report = await AsyncApiClient(ApiClient("http://127.0.0.1:1")).hub_disclosure_review_report(args)
            self.assertEqual(report.schema, "bioprism-mcp/hub-disclosure/0.1")
            async_call.assert_called_once_with("hub_disclosure_review", args.to_mcp_arguments())

        asyncio.run(run())


if __name__ == "__main__":
    unittest.main()
