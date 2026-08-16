from __future__ import annotations

import asyncio
import json
import unittest
from unittest.mock import patch

from prism_sdk import (
    ApiClient,
    AsyncApiClient,
    AsyncWorkspace,
    LabEvolutionAuditArgs,
    LabEvolutionAuditReport,
    Workspace,
    lab_evolution_audit_report,
)
from prism_sdk.errors import ArgumentError
from prism_sdk.models import ToolResult


def evolution_request() -> dict:
    candidate = lambda identifier: {
        "id": identifier,
        "components": [
            {"id": "select", "kind": "context_selector"},
            {"id": "run", "kind": "executor"},
            {"id": "stop", "kind": "terminator"},
        ],
        "cost_units": 0,
    }
    return {
        "cost_ceiling": 100,
        "candidates": [candidate("v1"), {**candidate("v2"), "derived_from": "v1"}],
        "baseline": "v1",
        "candidate": "v2",
        "holdout": {"id": "private-a", "partition": "rotating_private_certification", "query_budget": 4},
        "measurements": [
            {"configuration": "v1", "metric": "rate", "value": 0.7},
            {"configuration": "v2", "metric": "rate", "value": 0.83},
        ],
        "card_id": "card-v2",
        "proposal": {"id": "proposal-v2", "rationale": "widen closure", "changed_artifacts": ["select"], "touches_protected": []},
        "rollback_handle": "v1",
        "direction": "higher_is_better",
        "would_have_to_be_true": ["the gain survives a second holdout"],
        "max_rows": 1,
    }


def evolution_payload() -> dict:
    return {
        "ok": True,
        "schema": "bioprism-mcp/lab-evolution-audit/0.1",
        "status": "improvement_claimed",
        "claimable": True,
        "card": {"id": "card-v2", "surface": {"surface": "clean"}},
        "claim": {"card": "card-v2", "delta": 0.13},
        "sentence": "v2 moved rate",
        "measurement_count": 2,
        "measurement_rows": [{"index": 0, "result": "clean_measurement"}],
        "measurement_rows_omitted": 1,
        "max_rows": 1,
        "guarantees": ["clean measurements only"],
        "limitations": ["point delta"],
    }


class _SyncTool:
    def call_tool(self, name: str, arguments: dict) -> ToolResult:
        return ToolResult(name, {"isError": False, "content": [{"type": "text", "text": json.dumps(evolution_payload())}]})


class _AsyncTool:
    async def call_tool(self, name: str, arguments: dict) -> ToolResult:
        return ToolResult(name, {"isError": False, "content": [{"type": "text", "text": json.dumps(evolution_payload())}]})


class LabEvolutionTests(unittest.TestCase):
    def test_args_validate_two_candidates_direction_and_measurements(self) -> None:
        request = LabEvolutionAuditArgs.from_wire(evolution_request())
        self.assertEqual(request.to_mcp_arguments()["max_rows"], 1)
        with self.assertRaises(ArgumentError):
            LabEvolutionAuditArgs.from_wire({**evolution_request(), "direction": "unknown"})
        with self.assertRaises(ArgumentError):
            LabEvolutionAuditArgs.from_wire({**evolution_request(), "candidates": [evolution_request()["candidates"][0]]})

    def test_report_preserves_claim_and_measurement_omission(self) -> None:
        report = lab_evolution_audit_report(evolution_payload())
        self.assertIsInstance(report, LabEvolutionAuditReport)
        self.assertTrue(report.accepted)
        self.assertTrue(report.claimable)
        self.assertEqual(report.status, "improvement_claimed")
        self.assertEqual(report.measurement_rows_omitted, 1)

    def test_fail_closed_refusal_and_all_facades(self) -> None:
        refusal = {
            "ok": False,
            "schema": "bioprism-mcp/lab-evolution-audit/0.1",
            "stage": "measurement_completeness",
            "refusal": "both clean measurements are required",
            "fail_closed": True,
            "measurement_count": 1,
            "measurement_rows": [{"result": "clean_measurement"}],
            "measurement_rows_omitted": 0,
            "max_rows": 1,
            "guarantees": ["no partial claim"],
        }
        report = lab_evolution_audit_report({"mcp": {"result": {"structuredContent": refusal}}})
        self.assertTrue(report.refused)
        self.assertTrue(report.fail_closed)
        request = LabEvolutionAuditArgs.from_wire(evolution_request())
        self.assertTrue(Workspace(_SyncTool()).lab_evolution_audit_report(request).claimable)
        self.assertTrue(asyncio.run(AsyncWorkspace(_AsyncTool()).lab_evolution_audit_report(request)).claimable)
        with patch.object(ApiClient, "call_tool", return_value=evolution_payload()) as call:
            result = ApiClient("http://127.0.0.1:1").lab_evolution_audit_report(request)
        self.assertEqual(result.claim["delta"], 0.13)  # type: ignore[index]
        call.assert_called_once_with("lab_evolution_audit", request.to_mcp_arguments())

        async def run() -> None:
            with patch.object(ApiClient, "call_tool", return_value=evolution_payload()) as async_call:
                result = await AsyncApiClient(ApiClient("http://127.0.0.1:1")).lab_evolution_audit_report(request)
            self.assertEqual(result.measurement_count, 2)
            async_call.assert_called_once_with("lab_evolution_audit", request.to_mcp_arguments())

        asyncio.run(run())
