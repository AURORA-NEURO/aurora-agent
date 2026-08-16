from __future__ import annotations

import asyncio
import json
import unittest
from unittest.mock import patch

from prism_sdk import (
    ApiClient,
    AsyncApiClient,
    AsyncWorkspace,
    BioevalEstimandAuditArgs,
    BioevalEstimandAuditReport,
    Workspace,
    bioeval_estimand_audit_report,
)
from prism_sdk.errors import ArgumentError
from prism_sdk.models import ToolResult


def request() -> dict:
    return {
        "estimand": {
            "intervention": "knockdown",
            "comparator": "control",
            "unit": "cell line",
            "outcome": "viability",
            "horizon": "72h",
            "scope": "pdac-twin",
        },
        "kind": "intervention",
        "basis": {"evidentiary": "model_conditional", "model": "pdac-twin-v2"},
        "identification": {
            "identification": "probed",
            "strategy": "backdoor",
            "assumptions": ["no unmeasured confounding"],
            "checks": [{"name": "negative-control", "passed": False, "detail": "signal remained"}],
        },
        "corroborations": [{"source": "GSE-14520", "kind": "intervention", "detail": "external replication"}],
        "transport_requests": [{"target": "pdac-twin", "declared_scopes": ["pdac-twin"]}],
        "require_identification": True,
    }


def payload() -> dict:
    return {
        "ok": True,
        "schema": "bioprism-mcp/bioeval-estimand-audit/0.1",
        "workflow": "bioeval_estimand_audit",
        "estimand": {"five_elements_complete": True, "scope": "pdac-twin"},
        "claim": {
            "kind": "intervention",
            "still_model_conditional": False,
            "claim_language": "knockdown changes viability",
            "identification_summary": {"status": "probed", "failed_check_count": 1},
        },
        "policies": {"require_identification": True},
        "transport": {"status": "all_declared", "accepted": 1, "refused": 0},
        "guarantees": ["qualifier retained"],
        "limitations": ["no causal engine"],
    }


class _SyncTool:
    def call_tool(self, name: str, arguments: dict) -> ToolResult:
        return ToolResult(name, {"isError": False, "content": [{"type": "text", "text": json.dumps(payload())}]})


class _AsyncTool:
    async def call_tool(self, name: str, arguments: dict) -> ToolResult:
        return ToolResult(name, {"isError": False, "content": [{"type": "text", "text": json.dumps(payload())}]})


class BioevalEstimandTests(unittest.TestCase):
    def test_args_preserve_five_elements_identification_and_union_shapes(self) -> None:
        parsed = BioevalEstimandAuditArgs.from_wire(request())
        self.assertEqual(parsed.estimand.scope, "pdac-twin")
        self.assertEqual(parsed.identification.identification, "probed")  # type: ignore[union-attr]
        self.assertEqual(parsed.to_mcp_arguments()["basis"]["model"], "pdac-twin-v2")
        with self.assertRaises(ArgumentError):
            BioevalEstimandAuditArgs.from_wire({**request(), "kind": "causal"})
        with self.assertRaises(ArgumentError):
            BioevalEstimandAuditArgs.from_wire({**request(), "identification": {"identification": "declared"}})

    def test_report_preserves_model_qualifier_and_identification_status(self) -> None:
        report = bioeval_estimand_audit_report(payload())
        self.assertIsInstance(report, BioevalEstimandAuditReport)
        self.assertTrue(report.accepted)
        self.assertFalse(report.still_model_conditional)
        self.assertEqual(report.identification_status, "probed")
        self.assertEqual(report.transport_refused_count, 0)

    def test_all_facades_keep_estimand_audit_typed(self) -> None:
        parsed = BioevalEstimandAuditArgs.from_wire(request())
        self.assertTrue(Workspace(_SyncTool()).bioeval_estimand_audit_report(parsed).accepted)
        self.assertTrue(asyncio.run(AsyncWorkspace(_AsyncTool()).bioeval_estimand_audit_report(parsed)).accepted)
        with patch.object(ApiClient, "call_tool", return_value=payload()) as call:
            report = ApiClient("http://127.0.0.1:1").bioeval_estimand_audit_report(parsed)
        self.assertEqual(report.claim["kind"], "intervention")  # type: ignore[index]
        call.assert_called_once_with("bioeval_estimand_audit", parsed.to_mcp_arguments())

        async def run() -> None:
            with patch.object(ApiClient, "call_tool", return_value=payload()) as async_call:
                report = await AsyncApiClient(ApiClient("http://127.0.0.1:1")).bioeval_estimand_audit_report(parsed)
            self.assertEqual(report.transport["status"], "all_declared")  # type: ignore[index]
            async_call.assert_called_once_with("bioeval_estimand_audit", parsed.to_mcp_arguments())

        asyncio.run(run())
