from __future__ import annotations

import asyncio
import json
import unittest
from unittest.mock import patch

from prism_sdk import (
    ApiClient,
    AsyncApiClient,
    AsyncWorkspace,
    SecurityRedteamReport,
    SecurityRedteamSimulateArgs,
    Workspace,
    security_redteam_simulate_report,
)
from prism_sdk.errors import ArgumentError
from prism_sdk.models import ToolResult


def request() -> SecurityRedteamSimulateArgs:
    return SecurityRedteamSimulateArgs(
        findings=({"id": "F-confirmed", "campaign": "sandbox-escape", "boundary": "agent_sandbox", "class": "sandbox_bypass", "status": "confirmed"},),
        vulnerabilities=({"id": "V-holdout", "class": "hidden_test_exposure", "severity": "high", "epoch": 1, "transitions": []},),
        deliveries=({"id": "sealed-output", "kind": "agent_output", "origin": "agent_sandbox", "to": "artifact_service", "via": "sealed_output_bundle"},),
        incidents=({"id": "I-holdout", "class": "hidden_holdout_leak", "opened_at": 5, "requests": []},),
        audit_records=({"event": "security_quarantine", "actor": "operator:red-team", "subject": "hidden-oracle", "epoch": 6, "statement": {"kind": "asserted", "by": "operator:red-team", "claim": "quarantined"}},),
        attestations=({"kind": "digests_compared", "component": "holdout", "observed": True},),
        boundary_universe=("agent_sandbox", "evaluator_sandbox"),
        include_details=True,
        max_items=10,
    )


def finding(ok: bool = True) -> dict:
    if not ok:
        return {"index": 1, "ok": False, "refusal": "invalid finding", "fail_closed": True}
    return {
        "index": 0,
        "ok": True,
        "finding": {"id": "F-confirmed", "campaign": "sandbox-escape", "boundary": "agent_sandbox", "status": "confirmed", "class": "sandbox_bypass", "reproduction": "probe-17"},
        "regression_gate": {
            "eligible": True,
            "cell": {"finding": "F-confirmed", "campaign": "sandbox-escape", "boundary": "agent_sandbox", "class": "sandbox_bypass", "minimised": True, "embargoed": True},
            "public_summary": "F-confirmed against agent_sandbox (sandbox_bypass) — detail embargoed",
        },
    }


def vulnerability(disclosed: bool = True) -> dict:
    return {
        "index": 0,
        "ok": True,
        "vulnerability": {"id": "V-holdout", "class": "hidden_test_exposure", "severity": "high", "impact": {"infrastructure": False, "data": True, "result_integrity": True}, "stage": "disclosed" if disclosed else "fixed", "entered_at": 4, "embargoed": not disclosed, "history": []},
        "transitions": [
            {"index": 0, "ok": True, "to": "triaged", "epoch": 2, "stage_after": "triaged"},
            {"index": 1, "ok": True, "to": "fixed", "epoch": 3, "stage_after": "fixed"},
            {"index": 2, "ok": True, "to": "disclosed", "epoch": 4, "stage_after": "disclosed"},
        ],
        "transition_count": 3,
        "stopped_after_refusal": False,
        "advisory_present": True,
        "advisory_missing_fields": [],
        "independent_verification_required": True,
        "disclosed": disclosed,
    }


def boundary() -> dict:
    return {
        "model": "evaluation_model",
        "within_trial_agent_to_evaluator": [],
        "within_trial_evaluator_to_agent": [],
        "all_scope_agent_to_evaluator": [["agent_sandbox", "artifact_service", "evaluator_sandbox"]],
        "feedback_loops": [{"from": "evaluator_sandbox", "to": "catalog", "scope": "across_trials"}],
        "delivery_rows": [
            {"index": 0, "ok": True, "crossing": {"artifact": "sealed-output", "kind": "agent_output", "from": "agent_sandbox", "to": "artifact_service", "via": "sealed_output_bundle"}, "honest_label": "the model permits sealed-output; nothing observed the transfer", "scope": "within_trial"},
            {"index": 1, "ok": False, "refusal": "hidden oracle may never enter agent sandbox", "fail_closed": True, "requested": {"artifact": {"id": "hidden-oracle", "kind": "hidden_oracle_asset", "origin": "artifact_service"}, "to": "agent_sandbox", "via": "hidden_oracle_mount"}},
        ],
        "delivery_rows_omitted": 0,
        "allowed_delivery_count": 1,
        "refused_delivery_count": 1,
    }


def incident(allowed: bool) -> dict:
    claim = {"allowed": True, "report": {"incident": "I-holdout"}, "caveat": "requested actions are not observed executions"}
    if not allowed:
        claim = {"allowed": False, "refusal": "blast radius is partial", "fail_closed": True}
    return {
        "index": 0,
        "ok": True,
        "incident": {"id": "I-holdout", "class": "hidden_holdout_leak", "opened_at": 5, "blast_radius": {"completeness": "complete", "dispositions": {"run-1": "invalidated"}}},
        "requests": [{"index": 0, "ok": True, "request": {"action": "freeze_publication", "requested_at": 6, "requested_by": "operator:red-team"}, "honest_label": "freeze requested"}],
        "timeline": [{"index": 0, "ok": True, "epoch": 5}],
        "containment_claim": claim,
        "unrequested_actions": ["rotate_keys"],
        "result_tainting_class": True,
    }


def payload() -> dict:
    return {
        "ok": True,
        "workflow": "section_13_redteam_incident_evidence",
        "input_counts": {"findings": 1, "vulnerabilities": 1, "deliveries": 1, "incidents": 1, "audit_records": 1, "attestations": 1},
        "findings": [finding()],
        "findings_omitted": 0,
        "regression_corpus": {"sentinel_count": 1, "covered_boundaries": ["agent_sandbox"], "unminimised_count": 0, "uncovered_boundaries": ["public_api"], "cells": [{"finding": "F-confirmed"}], "omitted_cells": 0},
        "vulnerabilities": [vulnerability()],
        "vulnerabilities_omitted": 0,
        "boundary": boundary(),
        "incidents": [incident(True)],
        "incidents_omitted": 0,
        "audit": {"rows": [{"index": 0, "ok": True, "linked": {"index": 0, "digest": "a" * 64}}], "rows_omitted": 0, "chain_length": 1, "head": "a" * 64, "verified": True, "verification_refusal": None, "assertion_count": 1, "public_view_count": 1, "records": [{"event": "security_quarantine"}]},
        "attestations": [{"index": 0, "ok": True, "observed": True, "attestation": {"claim": "digests_compared", "statement": {"kind": "observed"}}}],
        "attestations_omitted": 0,
        "guarantees": ["only confirmed findings can become regression cells", "partial or unresolved blast radius cannot produce a containment report", "audit records distinguish observations from assertions and verify a hash-linked chain"],
        "limitations": ["this endpoint replays typed contracts; it does not run fuzzers, detectors, sandboxes, processes, containers, network controls, credential revocation, quarantine, or publication freezes"],
    }


class _SyncTool:
    def call_tool(self, name: str, arguments: dict) -> ToolResult:
        return ToolResult(name, {"isError": False, "content": [{"type": "text", "text": json.dumps(payload())}]})


class _AsyncTool:
    async def call_tool(self, name: str, arguments: dict) -> ToolResult:
        return ToolResult(name, {"isError": False, "content": [{"type": "text", "text": json.dumps(payload())}]})


class SecurityRedteamProjectionTests(unittest.TestCase):
    def test_args_bound_all_six_evidence_planes(self) -> None:
        args = request()
        wire = args.to_mcp_arguments()
        self.assertEqual(wire["max_items"], 10)
        self.assertTrue(wire["include_details"])
        self.assertEqual(len(wire["audit_records"]), 1)
        with self.assertRaises(ArgumentError):
            SecurityRedteamSimulateArgs(max_items=0)
        with self.assertRaises(ArgumentError):
            SecurityRedteamSimulateArgs(findings=({"id": object()},))

    def test_report_keeps_regression_disclosure_boundary_incident_audit_attestation_planes(self) -> None:
        report = security_redteam_simulate_report(payload())
        self.assertIsInstance(report, SecurityRedteamReport)
        self.assertFalse(report.refused)
        self.assertEqual(report.confirmed_finding_count, 1)
        self.assertEqual(report.regression_corpus.sentinel_count, 1)
        self.assertEqual(report.disclosed_vulnerability_count, 1)
        self.assertEqual(report.refused_delivery_count, 1)
        self.assertTrue(report.boundary.within_trial_feedback_is_absent)
        self.assertEqual(report.containment_allowed_count, 1)
        self.assertTrue(report.audit_chain_verified)
        self.assertEqual(report.observed_attestation_count, 1)
        self.assertEqual(report.asserted_attestation_count, 0)
        self.assertTrue(report.execution_claims_absent)

    def test_incomplete_lifecycle_and_containment_remain_fail_closed_rows(self) -> None:
        value = payload()
        value["vulnerabilities"] = [vulnerability(False)]
        value["vulnerabilities"][0]["transitions"][2] = {"index": 2, "ok": False, "refusal": "advisory omits result implications", "fail_closed": True, "stage_after": "fixed"}
        value["incidents"] = [incident(False)]
        report = SecurityRedteamReport.from_wire(value)
        self.assertEqual(report.failed_vulnerability_count, 1)
        self.assertEqual(report.containment_withheld_count, 1)
        self.assertTrue(report.vulnerabilities[0].transitions[-1].fail_closed)
        self.assertTrue(report.incidents[0].containment_claim.fail_closed)

    def test_mcp_http_envelopes_and_all_facades_delegate(self) -> None:
        envelope = {"ok": True, "tool": "security_redteam_simulate", "mcp": {"result": {"structuredContent": payload()}}}
        self.assertTrue(security_redteam_simulate_report(envelope).audit_chain_verified)
        args = request()
        self.assertEqual(Workspace(_SyncTool()).security_redteam_simulate_report(args).confirmed_finding_count, 1)
        self.assertEqual(asyncio.run(AsyncWorkspace(_AsyncTool()).security_redteam_simulate_report(args)).allowed_delivery_count, 1)
        with patch.object(ApiClient, "call_tool", return_value=payload()) as call:
            report = ApiClient("http://127.0.0.1:1").security_redteam_simulate_report(args)
        self.assertTrue(report.audit_chain_verified)
        call.assert_called_once_with("security_redteam_simulate", args.to_mcp_arguments())

        async def run() -> None:
            with patch.object(ApiClient, "call_tool", return_value=payload()) as async_call:
                report = await AsyncApiClient(ApiClient("http://127.0.0.1:1")).security_redteam_simulate_report(args)
            self.assertEqual(report.observed_attestation_count, 1)
            async_call.assert_called_once_with("security_redteam_simulate", args.to_mcp_arguments())

        asyncio.run(run())


if __name__ == "__main__":
    unittest.main()
