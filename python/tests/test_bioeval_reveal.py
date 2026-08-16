from __future__ import annotations

import asyncio
import json
import unittest
from unittest.mock import patch

from prism_sdk import (
    ApiClient,
    AsyncApiClient,
    AsyncWorkspace,
    BioevalRevealAuditArgs,
    BioevalRevealAuditReport,
    Workspace,
    bioeval_reveal_audit_report,
)
from prism_sdk.errors import ArgumentError
from prism_sdk.models import ToolResult


def request() -> dict:
    return {
        "study": "prospective-2026",
        "commitments": [
            {"target": "case-a", "prediction": {"class": "stable"}, "analysis_plan": "plan-v1"},
            {"target": "case-b", "prediction": {"class": "progression"}, "analysis_plan": "plan-v1"},
        ],
        "rubric": {"version": 1, "rules": ["predeclared"]},
        "sealed_at": "2026-08-16T12:00:00Z",
        "outcomes": [{"target": "case-a", "observed": {"class": "stable"}}],
        "score_rubric": {"version": 1, "rules": ["predeclared"]},
    }


def payload() -> dict:
    return {
        "ok": True,
        "schema": "bioprism-mcp/bioeval-reveal-audit/0.1",
        "workflow": "bioeval_reveal_audit",
        "study": "prospective-2026",
        "sealed_at": "2026-08-16T12:00:00Z",
        "digests": {"rubric": "rubric-digest", "commitments": "commitment-digest"},
        "commitments": {"rows": [], "returned": 0, "total": 2, "omitted": 2},
        "outcomes": {"rows": [], "returned": 0, "total": 1, "omitted": 1},
        "seal_lock": {"status": "refused", "refusal": "already sealed"},
        "reveal_lock": {"status": "refused", "refusal": "already revealed"},
        "scoring": {"status": "accepted", "value": {"unrevealed": ["case-b"]}, "refusal": None, "complete": False},
        "findings": {
            "unrevealed_commitments": {"ids": ["case-b"], "total": 1, "omitted": 0},
            "selective_publication": True,
            "rubric_match_refused": False,
            "uncommitted_outcome_refused": False,
        },
        "guarantees": ["rubric digest is frozen"],
        "limitations": ["no timestamp attestation"],
    }


class _SyncTool:
    def call_tool(self, name: str, arguments: dict) -> ToolResult:
        return ToolResult(name, {"isError": False, "content": [{"type": "text", "text": json.dumps(payload())}]})


class _AsyncTool:
    async def call_tool(self, name: str, arguments: dict) -> ToolResult:
        return ToolResult(name, {"isError": False, "content": [{"type": "text", "text": json.dumps(payload())}]})


class BioevalRevealTests(unittest.TestCase):
    def test_args_preserve_opaque_predictions_and_rubric(self) -> None:
        parsed = BioevalRevealAuditArgs.from_wire(request())
        self.assertEqual(parsed.commitments[0].target, "case-a")
        self.assertEqual(parsed.to_mcp_arguments()["rubric"]["version"], 1)
        with self.assertRaises(ArgumentError):
            BioevalRevealAuditArgs.from_wire({**request(), "commitments": request()["commitments"] + [request()["commitments"][0]]})
        with self.assertRaises(ArgumentError):
            BioevalRevealAuditArgs.from_wire({**request(), "outcomes": [{"target": "case-a", "observed": object()}]})

    def test_report_preserves_selective_publication_and_locks(self) -> None:
        report = bioeval_reveal_audit_report(payload())
        self.assertIsInstance(report, BioevalRevealAuditReport)
        self.assertTrue(report.accepted)
        self.assertTrue(report.selective_publication)
        self.assertEqual(report.unrevealed_targets, ("case-b",))
        self.assertFalse(report.rubric_match_refused)

    def test_fail_closed_refusal_is_typed(self) -> None:
        report = bioeval_reveal_audit_report({
            "ok": False,
            "schema": "bioprism-mcp/bioeval-reveal-audit/0.1",
            "workflow": "bioeval_reveal_audit",
            "stage": "rubric_integrity_policy",
            "refusal": "changed rubric",
            "fail_closed": True,
            "guarantees": [],
            "limitations": [],
        })
        self.assertTrue(report.refused)
        self.assertEqual(report.stage, "rubric_integrity_policy")

    def test_all_facades_keep_reveal_audit_typed(self) -> None:
        parsed = BioevalRevealAuditArgs.from_wire(request())
        self.assertTrue(Workspace(_SyncTool()).bioeval_reveal_audit_report(parsed).accepted)
        self.assertTrue(asyncio.run(AsyncWorkspace(_AsyncTool()).bioeval_reveal_audit_report(parsed)).accepted)
        with patch.object(ApiClient, "call_tool", return_value=payload()) as call:
            report = ApiClient("http://127.0.0.1:1").bioeval_reveal_audit_report(parsed)
        self.assertEqual(report.unrevealed_targets, ("case-b",))
        call.assert_called_once_with("bioeval_reveal_audit", parsed.to_mcp_arguments())

        async def run() -> None:
            with patch.object(ApiClient, "call_tool", return_value=payload()) as async_call:
                report = await AsyncApiClient(ApiClient("http://127.0.0.1:1")).bioeval_reveal_audit_report(parsed)
            self.assertTrue(report.selective_publication)
            async_call.assert_called_once_with("bioeval_reveal_audit", parsed.to_mcp_arguments())

        asyncio.run(run())
