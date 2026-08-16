from __future__ import annotations

import asyncio
import json
import unittest
from unittest.mock import patch

from prism_sdk import (
    ApiClient,
    AsyncApiClient,
    AsyncWorkspace,
    FoundationContractCheckArgs,
    FoundationContractCheckReport,
    Workspace,
    foundation_contract_check_report,
)
from prism_sdk.errors import ArgumentError
from prism_sdk.models import ToolResult


def contract() -> dict:
    return {
        "id": "fbc:test:001",
        "intent": "distinguish two declared outcomes",
        "evidence_obligations": ["reference"],
        "actions": ["inspect", "abstain"],
        "claim_schema": "typed-result-v1",
        "falsifiers": ["reference-disagrees"],
        "reference_standard": "deterministic-fixture",
        "minimum_reviewers": 1,
        "uncertainty_required": True,
        "terminations": ["success", "underdetermined"],
    }


def success_payload() -> dict:
    return {
        "ok": True,
        "verdict": "admitted",
        "contract": {
            "ok": True,
            "id": "fbc:test:001",
            "intent": "distinguish two declared outcomes",
            "falsifier_count": 1,
            "action_count": 2,
            "evidence_obligation_count": 1,
            "minimum_reviewers": 1,
            "uncertainty_required": True,
        },
        "parent_relation": {"ok": True, "relation": "refines"},
        "envelope": {"ok": True, "structure": "complete", "maturity": "not_requested_or_admissible", "maturity_rung": "independently_replicated_retrospective", "fail_closed": False},
        "world": {"ok": True, "world_id": "designed-world", "class": "designed_synthetic", "counterfactual_strength": "high", "reveal_policy": "admissible", "claim": "admitted", "fail_closed": False},
        "transition": {"ok": True, "verdict": "plane_consistent"},
        "guarantees": ["contract gates remain separate"],
    }


def refusal_payload() -> dict:
    return {
        "ok": True,
        "verdict": "refused",
        "contract": {"ok": True, "id": "fbc:test:001", "intent": "distinguish two declared outcomes", "falsifier_count": 1, "action_count": 2, "evidence_obligation_count": 1, "minimum_reviewers": 1, "uncertainty_required": True},
        "parent_relation": None,
        "envelope": None,
        "world": {"ok": False, "world_id": "observed-world", "class": "observed_replay", "counterfactual_strength": "low", "reveal_policy": "admissible", "claim": "unsupported real treatment effect", "fail_closed": True},
        "transition": {"ok": False, "verdict": "plane_confusion", "refusal": "latent biological state on observation plane", "fail_closed": True},
        "guarantees": ["declarations are not treatment authority"],
    }


class _SyncTool:
    def __init__(self, payload: dict) -> None:
        self.payload = payload

    def call_tool(self, name: str, arguments: dict) -> ToolResult:
        return ToolResult(name, {"isError": False, "content": [{"type": "text", "text": json.dumps(self.payload)}]})


class _AsyncTool:
    def __init__(self, payload: dict) -> None:
        self.payload = payload

    async def call_tool(self, name: str, arguments: dict) -> ToolResult:
        return ToolResult(name, {"isError": False, "content": [{"type": "text", "text": json.dumps(self.payload)}]})


class FoundationProjectionTests(unittest.TestCase):
    def test_args_keep_optional_gates_and_claim_authority_explicit(self) -> None:
        request = FoundationContractCheckArgs.from_wire({"contract": contract(), "claim": "real_treatment_effect", "present_as_established": True, "world": {"id": "world"}})
        wire = request.to_mcp_arguments()
        self.assertEqual(wire["claim"], "real_treatment_effect")
        self.assertTrue(wire["present_as_established"])
        self.assertIn("world", wire)
        with self.assertRaises(ArgumentError):
            FoundationContractCheckArgs.from_wire({"contract": contract(), "claim": "invented_claim"})

    def test_admitted_report_requires_all_present_gates(self) -> None:
        report = foundation_contract_check_report(success_payload())
        self.assertIsInstance(report, FoundationContractCheckReport)
        self.assertTrue(report.admitted)
        self.assertTrue(report.contract_admissible)
        self.assertTrue(report.optional_gates_clear)
        self.assertTrue(report.world_claim_admitted)
        self.assertTrue(report.transition_plane_consistent)
        self.assertEqual(report.contract.minimum_reviewers, 1)

    def test_refused_world_and_transition_are_not_collapsed_into_contract_admission(self) -> None:
        report = FoundationContractCheckReport.from_wire(refusal_payload())
        self.assertTrue(report.contract_admissible)
        self.assertFalse(report.admitted)
        self.assertTrue(report.refused)
        self.assertTrue(report.fail_closed)
        self.assertFalse(report.world_claim_admitted)
        self.assertFalse(report.transition_plane_consistent)

    def test_mcp_and_http_envelopes_are_parseable(self) -> None:
        envelope = {"ok": True, "tool": "foundation_contract_check", "mcp": {"result": {"structuredContent": success_payload()}}}
        self.assertTrue(foundation_contract_check_report(envelope).admitted)

    def test_sync_async_mcp_and_http_facades_delegate_typed_arguments(self) -> None:
        request = FoundationContractCheckArgs.from_wire({"contract": contract()})
        self.assertTrue(Workspace(_SyncTool(success_payload())).foundation_contract_check_report(request).admitted)
        self.assertTrue(asyncio.run(AsyncWorkspace(_AsyncTool(success_payload())).foundation_contract_check_report(request)).admitted)
        with patch.object(ApiClient, "call_tool", return_value=success_payload()) as call:
            report = ApiClient("http://127.0.0.1:1").foundation_contract_check_report(request)
        self.assertTrue(report.admitted)
        call.assert_called_once_with("foundation_contract_check", request.to_mcp_arguments())

        async def run() -> None:
            with patch.object(ApiClient, "call_tool", return_value=success_payload()) as async_call:
                report = await AsyncApiClient(ApiClient("http://127.0.0.1:1")).foundation_contract_check_report(request)
            self.assertEqual(report.verdict, "admitted")
            async_call.assert_called_once_with("foundation_contract_check", request.to_mcp_arguments())

        asyncio.run(run())

    def test_contract_admissibility_refusal_is_explicit(self) -> None:
        payload = success_payload()
        payload["verdict"] = "refused"
        payload["contract"] = {"ok": False, "refusal": "contract has no falsifier", "fail_closed": True}
        payload["parent_relation"] = None
        payload["envelope"] = None
        payload["world"] = None
        payload["transition"] = None
        report = FoundationContractCheckReport.from_wire(payload)
        self.assertFalse(report.contract_admissible)
        self.assertTrue(report.fail_closed)
        self.assertEqual(report.contract.refusal, "contract has no falsifier")
