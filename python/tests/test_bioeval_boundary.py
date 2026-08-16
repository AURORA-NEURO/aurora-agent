from __future__ import annotations

import asyncio
import json
import unittest
from unittest.mock import patch

from prism_sdk import (
    ApiClient,
    AsyncApiClient,
    AsyncWorkspace,
    BioevalBoundaryAuditArgs,
    BioevalBoundaryAuditReport,
    Workspace,
    bioeval_boundary_audit_report,
)
from prism_sdk.errors import ArgumentError
from prism_sdk.models import ToolResult


def request() -> dict:
    return {
        "policies": [{
            "id": "consent-study",
            "recipient": "evaluator",
            "information_type": "deidentified",
            "purpose": "study",
            "transmission_principle": "consent",
            "channels": ["inter_agent_messages"],
        }],
        "flows": [
            {
                "id": "authorized",
                "sender": "agent",
                "subject": "participant-1",
                "recipient": "evaluator",
                "information_type": "deidentified",
                "purpose": "study",
                "transmission_principle": "consent",
                "channel": "inter_agent_messages",
                "effect": {"effect": "materialized"},
            },
            {
                "id": "veto",
                "sender": "agent",
                "subject": "participant-1",
                "recipient": "public",
                "information_type": "identifier",
                "purpose": "publication",
                "transmission_principle": "none",
                "channel": "final_output",
                "effect": {"effect": "materialized"},
                "irreversible": True,
            },
        ],
        "utility": 0.8,
    }


def payload() -> dict:
    return {
        "ok": True,
        "schema": "bioprism-mcp/bioeval-boundary-audit/0.1",
        "workflow": "bioeval_boundary_audit",
        "boundary": {"policy_count": 1, "flow_count": 5, "authorised_count": 1, "compliant_count": 1, "violation_count": 3, "veto_count": 2},
        "policies": {"rows": [], "returned": 0, "total": 1, "omitted": 1},
        "flows": {"rows": [], "returned": 0, "total": 5, "omitted": 5},
        "violations_by_channel": {"final_output": 1, "external_queries": 1, "logs": 1},
        "pareto": {"utility": 0.8, "violations": 3},
        "composite": {"status": "refused", "value": None, "refusal": "composite refused"},
        "findings": {
            "violating_flows": {"ids": ["materialized-violation", "irreversible-veto", "bypass"], "total": 3, "omitted": 0},
            "veto_flows": {"ids": ["irreversible-veto", "bypass"], "total": 2, "omitted": 0},
            "composite_refused": True,
        },
        "guarantees": ["bypass remains a veto"],
        "limitations": ["no payload detector"],
    }


class _SyncTool:
    def call_tool(self, name: str, arguments: dict) -> ToolResult:
        return ToolResult(name, {"isError": False, "content": [{"type": "text", "text": json.dumps(payload())}]})


class _AsyncTool:
    async def call_tool(self, name: str, arguments: dict) -> ToolResult:
        return ToolResult(name, {"isError": False, "content": [{"type": "text", "text": json.dumps(payload())}]})


class BioevalBoundaryTests(unittest.TestCase):
    def test_args_preserve_effect_kinds_and_wildcard_policies(self) -> None:
        parsed = BioevalBoundaryAuditArgs.from_wire(request())
        self.assertEqual(parsed.flows[0].effect.kind, "materialized")
        self.assertEqual(parsed.policies[0].channels, ("inter_agent_messages",))
        self.assertEqual(parsed.to_mcp_arguments()["flows"][1]["irreversible"], True)
        with self.assertRaises(ArgumentError):
            BioevalBoundaryAuditArgs.from_wire({**request(), "flows": [{**request()["flows"][0], "channel": "invented"}]})
        with self.assertRaises(ArgumentError):
            BioevalBoundaryAuditArgs.from_wire({**request(), "flows": [{**request()["flows"][0], "effect": {"effect": "proposed"}}]})

    def test_report_preserves_violation_veto_and_composite_posture(self) -> None:
        report = bioeval_boundary_audit_report(payload())
        self.assertIsInstance(report, BioevalBoundaryAuditReport)
        self.assertTrue(report.accepted)
        self.assertEqual(report.violation_count, 3)
        self.assertEqual(report.veto_count, 2)
        self.assertTrue(report.composite_refused)
        self.assertEqual(report.veto_flows, ("irreversible-veto", "bypass"))

    def test_fail_closed_refusal_is_typed(self) -> None:
        report = bioeval_boundary_audit_report({
            "ok": False,
            "schema": "bioprism-mcp/bioeval-boundary-audit/0.1",
            "workflow": "bioeval_boundary_audit",
            "stage": "veto_policy",
            "refusal": "veto remains",
            "fail_closed": True,
            "guarantees": [],
            "limitations": [],
        })
        self.assertTrue(report.refused)
        self.assertEqual(report.stage, "veto_policy")

    def test_all_facades_keep_boundary_audit_typed(self) -> None:
        parsed = BioevalBoundaryAuditArgs.from_wire(request())
        self.assertTrue(Workspace(_SyncTool()).bioeval_boundary_audit_report(parsed).accepted)
        self.assertTrue(asyncio.run(AsyncWorkspace(_AsyncTool()).bioeval_boundary_audit_report(parsed)).accepted)
        with patch.object(ApiClient, "call_tool", return_value=payload()) as call:
            report = ApiClient("http://127.0.0.1:1").bioeval_boundary_audit_report(parsed)
        self.assertTrue(report.composite_refused)
        call.assert_called_once_with("bioeval_boundary_audit", parsed.to_mcp_arguments())

        async def run() -> None:
            with patch.object(ApiClient, "call_tool", return_value=payload()) as async_call:
                report = await AsyncApiClient(ApiClient("http://127.0.0.1:1")).bioeval_boundary_audit_report(parsed)
            self.assertEqual(report.violation_count, 3)
            async_call.assert_called_once_with("bioeval_boundary_audit", parsed.to_mcp_arguments())

        asyncio.run(run())
