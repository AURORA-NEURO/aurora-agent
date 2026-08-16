from __future__ import annotations

import asyncio
import json
import unittest
from unittest.mock import patch

from prism_sdk import ApiClient, AsyncApiClient, AsyncWorkspace, HubSubmissionReviewArgs, Workspace, hub_submission_review
from prism_sdk.errors import ArgumentError
from prism_sdk.models import ToolResult


SUBMISSION = "sub-1"


def event(kind: object, at: int, actor: str = "reviewer", **extra: object) -> dict:
    return {"submission": SUBMISSION, "kind": kind, "actor": actor, "at": at, "reason": None, "superseded_by": None, **extra}


def ledger() -> dict:
    events = [
        event("opened", 1, "hub"),
        event({"kind": "transition", "from": "submitted", "to": "under-review"}, 2),
        event({"kind": "transition", "from": "under-review", "to": "accepted"}, 3),
        event({"kind": "attestation", "from": "self-reported", "to": "reproduced"}, 4),
    ]
    record = {"submission": {"id": SUBMISSION, "content": "digest-1", "submitter": "lab-a"}, "state": "accepted", "verification": "reproduced", "history": events, "tombstone": None}
    return {"records": {SUBMISSION: record}, "events": events, "last_epoch": 4}


def request() -> HubSubmissionReviewArgs:
    return HubSubmissionReviewArgs({"id": SUBMISSION, "content": "digest-1"}, {"id": "lab-a", "standing": {"standing": "unverified"}, "conflicts_declared": True, "conflicts": []}, {"actor": "hub", "at": 1, "transitions": [], "attestations": [], "revocations": []})


def payload() -> dict:
    return {
        "ok": True,
        "schema": "bioprism-mcp/hub-submission/0.1",
        "stage": "moderation_ledger",
        "submission": {"id": SUBMISSION, "content": "digest-1", "submitter": "lab-a"},
        "limitation_card": "Does not establish clinical validity or generalisation.",
        "state": "accepted",
        "verification": "reproduced",
        "published": [SUBMISSION],
        "event_count": 4,
        "ledger": ledger(),
        "guarantees": [
            "moderation is an append-only in-memory state machine with monotonic epochs",
            "rejection, withdrawal, reopening, and supersession carry typed transition rules and reasons",
            "self-review and self-asserted verification are refused",
            "this call does not persist, authenticate, publish to a network, or render a public web page",
        ],
    }


class _SyncTool:
    def call_tool(self, name: str, arguments: dict) -> ToolResult:
        return ToolResult(name, {"isError": False, "content": [{"type": "text", "text": json.dumps(payload())}]})


class _AsyncTool:
    async def call_tool(self, name: str, arguments: dict) -> ToolResult:
        return ToolResult(name, {"isError": False, "content": [{"type": "text", "text": json.dumps(payload())}]})


class HubSubmissionProjectionTests(unittest.TestCase):
    def test_args_preserve_nested_draft_submitter_and_moderation(self) -> None:
        args = request()
        self.assertEqual(args.to_mcp_arguments()["moderation"]["transitions"], [])
        with self.assertRaises(ArgumentError):
            HubSubmissionReviewArgs([], {})

    def test_report_preserves_append_only_events_and_verification(self) -> None:
        report = hub_submission_review(payload())
        self.assertTrue(report.accepted)
        self.assertTrue(report.moderation_replayed)
        self.assertEqual(report.state, "accepted")
        self.assertEqual(report.verification, "reproduced")
        self.assertEqual(report.event_count, 4)
        self.assertEqual(report.moderation.events[1].kind, "transition")
        self.assertEqual(report.moderation.events[3].to_verification, "reproduced")
        self.assertEqual(report.moderation.published_ids, (SUBMISSION,))
        self.assertTrue(report.append_only_history_is_visible)
        self.assertTrue(report.self_review_is_refused)
        self.assertTrue(report.network_publication_is_not_claimed)

    def test_fail_closed_acceptance_refusal_and_all_python_facades(self) -> None:
        refused = {"ok": False, "schema": "bioprism-mcp/hub-submission/0.1", "stage": "submission_acceptance", "refusal": "conflicts undeclared", "fail_closed": True, "submission": None, "ledger": None, "guarantees": ["a refused draft is never represented as submitted or publishable"]}
        report = hub_submission_review(refused)
        self.assertFalse(report.accepted)
        self.assertEqual(report.stage, "submission_acceptance")
        self.assertTrue(report.fail_closed)
        args = request()
        self.assertTrue(Workspace(_SyncTool()).hub_submission_review_report(args).accepted)
        self.assertTrue(asyncio.run(AsyncWorkspace(_AsyncTool()).hub_submission_review_report(args)).moderation_replayed)
        with patch.object(ApiClient, "call_tool", return_value=payload()) as call:
            report = ApiClient("http://127.0.0.1:1").hub_submission_review_report(args)
        self.assertEqual(report.moderation.withdrawn_count, 0)
        call.assert_called_once_with("hub_submission_review", args.to_mcp_arguments())

        async def run() -> None:
            with patch.object(ApiClient, "call_tool", return_value=payload()) as async_call:
                result = await AsyncApiClient(ApiClient("http://127.0.0.1:1")).hub_submission_review_report(args)
            self.assertEqual(result.schema, "bioprism-mcp/hub-submission/0.1")
            async_call.assert_called_once_with("hub_submission_review", args.to_mcp_arguments())

        asyncio.run(run())


if __name__ == "__main__":
    unittest.main()
