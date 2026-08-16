from __future__ import annotations

import asyncio
import json
import unittest
from unittest.mock import patch

from prism_sdk import (
    ApiClient,
    AsyncApiClient,
    AsyncWorkspace,
    BioevalAcquisitionAuditArgs,
    BioevalAcquisitionAuditReport,
    Workspace,
    bioeval_acquisition_audit_report,
)
from prism_sdk.errors import ArgumentError
from prism_sdk.models import ToolResult


def request() -> dict:
    return {
        "obligations": [
            {"id": "subtype", "required": True},
            {"id": "context", "required": False},
        ],
        "actions": [
            {"id": "read-notes", "kind": "metadata", "cost": 2, "closes": ["context"]},
            {"id": "panel", "kind": "assay", "cost": 40, "closes": ["subtype"]},
        ],
        "stopped_after": True,
        "reference_policy": {"name": "random", "cost": 30, "admissible": True},
    }


def payload() -> dict:
    return {
        "ok": True,
        "schema": "bioprism-mcp/bioeval-acquisition-audit/0.1",
        "workflow": "bioeval_acquisition_audit",
        "status": "admissible",
        "stopped_after": True,
        "admissible": True,
        "obligations": [{"id": "subtype", "required": True, "closed": True, "open": False}],
        "open_obligations": [],
        "actions": [{"id": "panel", "kind": "assay", "cost": 40}],
        "cost": 42,
        "cost_by_kind": [{"kind": "assay", "cost": 40}],
        "findings": {"redundant_action_ids": [], "unnecessary_action_ids": [], "deferred_decisive_cost": 2},
        "reference_policy": {"name": "random", "cost": 30, "admissible": True},
        "regret": {"policy": "random", "cost_difference": 12, "like_for_like": True},
        "guarantees": ["required obligations gate admissibility"],
        "limitations": ["no acquisition executed"],
    }


class _SyncTool:
    def call_tool(self, name: str, arguments: dict) -> ToolResult:
        return ToolResult(name, {"isError": False, "content": [{"type": "text", "text": json.dumps(payload())}]})


class _AsyncTool:
    async def call_tool(self, name: str, arguments: dict) -> ToolResult:
        return ToolResult(name, {"isError": False, "content": [{"type": "text", "text": json.dumps(payload())}]})


class BioevalAcquisitionTests(unittest.TestCase):
    def test_args_keep_order_and_reference_requirement_explicit(self) -> None:
        parsed = BioevalAcquisitionAuditArgs.from_wire(request())
        self.assertEqual(parsed.to_mcp_arguments()["actions"][1]["kind"], "assay")
        with self.assertRaises(ArgumentError):
            BioevalAcquisitionAuditArgs.from_wire({**request(), "require_reference": True, "reference_policy": None})
        with self.assertRaises(ArgumentError):
            BioevalAcquisitionAuditArgs.from_wire({**request(), "actions": [{"id": "bad", "kind": "unknown", "cost": 1}]})

    def test_report_preserves_stopping_and_like_for_like_regret(self) -> None:
        report = bioeval_acquisition_audit_report(payload())
        self.assertIsInstance(report, BioevalAcquisitionAuditReport)
        self.assertTrue(report.accepted)
        self.assertTrue(report.admissible)
        self.assertTrue(report.stopped_after)
        self.assertTrue(report.like_for_like)

    def test_all_facades_keep_acquisition_audit_typed(self) -> None:
        parsed = BioevalAcquisitionAuditArgs.from_wire(request())
        self.assertTrue(Workspace(_SyncTool()).bioeval_acquisition_audit_report(parsed).accepted)
        self.assertTrue(asyncio.run(AsyncWorkspace(_AsyncTool()).bioeval_acquisition_audit_report(parsed)).accepted)
        with patch.object(ApiClient, "call_tool", return_value=payload()) as call:
            report = ApiClient("http://127.0.0.1:1").bioeval_acquisition_audit_report(parsed)
        self.assertEqual(report.findings["deferred_decisive_cost"], 2)  # type: ignore[index]
        call.assert_called_once_with("bioeval_acquisition_audit", parsed.to_mcp_arguments())

        async def run() -> None:
            with patch.object(ApiClient, "call_tool", return_value=payload()) as async_call:
                report = await AsyncApiClient(ApiClient("http://127.0.0.1:1")).bioeval_acquisition_audit_report(parsed)
            self.assertEqual(report.status, "admissible")
            async_call.assert_called_once_with("bioeval_acquisition_audit", parsed.to_mcp_arguments())

        asyncio.run(run())
