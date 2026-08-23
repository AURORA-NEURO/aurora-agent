from __future__ import annotations

import asyncio
import json
import unittest
from unittest.mock import patch

from prism_sdk import (
    ApiClient,
    AsyncWorkspace,
    INTERWEAVE_WORKFLOW_IDS,
    WorkflowExecutionReport,
    WorkflowExecutionRequest,
    WorkflowExecutionEvidenceReport,
    WorkflowExecutionEvidenceRequest,
    Workspace,
    workflow_execution_evidence_report,
    workflow_execution_report,
)
from prism_sdk.authoring import content_digest
from prism_sdk.errors import ArgumentError
from prism_sdk.models import ToolResult


def request_wire(workflow: str = INTERWEAVE_WORKFLOW_IDS[0]) -> dict:
    return {
        "workflow": workflow,
        "problem": {"actions": ["choose-m0", "choose-m1"], "models": ["m0", "m1"], "loss": [0.0, 1.0, 1.0, 0.0]},
        "belief": {"mass": [0.9, 0.1]},
        "acquisitions": [{
            "id": "screen",
            "cost": 0.01,
            "outcomes": [
                {"label": "positive", "likelihood": [0.9, 0.2]},
                {"label": "negative", "likelihood": [0.1, 0.8]},
            ],
        }],
        "budget": 0.1,
        "max_steps": 1,
        "authorization": {"grant_id": "g-1", "provider": "mcp-simulated"},
        "observations": [{"acquisition_id": "screen", "outcome_label": "negative"}],
        "evidence": {"subject_id": "subject-1", "domains": ["software"]},
    }


def payload() -> dict:
    plan_digest = "a" * 64
    binding_digest = "b" * 64
    return {
        "ok": True,
        "schema": "bioprism-interweave/workflow-execution/0.1",
        "mode": "simulate",
        "workflow": "reliable_software_repair",
        "plan_digest": plan_digest,
        "binding_digest": binding_digest,
        "binding": {"workflow": "reliable_software_repair", "binding_digest": binding_digest},
        "completed": False,
        "release_posture": "workflow_receipt_only_external_release_not_authorized",
        "receipt": {
            "schema": "bioprism-interweave/workflow-execution/0.1",
            "workflow": "reliable_software_repair",
            "binding_digest": binding_digest,
            "adaptive": {
                "plan_digest": plan_digest,
                "status": "refused",
                "observations": [],
                "refusal": "authorization_required",
            },
        },
        "provenance_counts": {"observed": 0, "simulated": 0, "replayed": 0},
        "guarantees": ["no external effect"],
        "limitations": ["simulator"],
    }


def evidence_payload() -> dict:
    evidence = {
        "schema": "bioprism-devplat-workflow-execution-evidence/0.1",
        "workflow": "interweave_workflow_execution_evidence",
        "workflow_id": "reliable_software_repair",
        "subject_id": "subject-1",
        "domains": ["software"],
        "binding_digest": "b" * 64,
        "plan_digest": "a" * 64,
        "workflow_spec_digest": "c" * 64,
        "provider_id": "mcp-simulated",
        "receipt_digest": "d" * 64,
        "receipt_status": "refused",
        "completed": False,
        "provenance": {"mode": "none", "observed": 0, "simulated": 0, "replayed": 0, "observation_count": 0},
        "binding": {},
        "receipt": {},
        "parent_digests": [],
        "claim_posture": {"status": "review_required"},
        "readiness_claimed": False,
        "execution": "not_started",
    }
    evidence["evidence_digest"] = content_digest(evidence)
    return {
        "ok": True,
        "schema": "bioprism-devplat-workflow-execution-evidence/0.1",
        "workflow": "interweave_workflow_execution_evidence",
        "evidence_digest": evidence["evidence_digest"],
        "evidence": evidence,
        "registry": {"created": True},
    }


class _SyncTool:
    def call_tool(self, name: str, arguments: dict) -> ToolResult:
        return ToolResult(name, {"isError": False, "content": [{"type": "text", "text": json.dumps(payload())}]})


class _AsyncTool:
    async def call_tool(self, name: str, arguments: dict) -> ToolResult:
        return ToolResult(name, {"isError": False, "content": [{"type": "text", "text": json.dumps(payload())}]})


class WorkflowExecutionTests(unittest.TestCase):
    def test_all_six_workflow_ids_round_trip_and_replay_requires_receipt(self) -> None:
        for workflow in INTERWEAVE_WORKFLOW_IDS:
            with self.subTest(workflow=workflow):
                request = WorkflowExecutionRequest.from_wire(request_wire(workflow))
                self.assertEqual(request.to_mcp_arguments()["workflow"], workflow)
                self.assertEqual(request.to_mcp_arguments()["evidence"]["subject_id"], "subject-1")
        replay = request_wire()
        replay["mode"] = "replay"
        with self.assertRaises(ArgumentError):
            WorkflowExecutionRequest.from_wire(replay)

    def test_report_keeps_refusal_and_provenance_counts_typed(self) -> None:
        report = workflow_execution_report(payload())
        self.assertIsInstance(report, WorkflowExecutionReport)
        self.assertEqual(report.status, "refused")
        self.assertEqual(report.refusal, "authorization_required")
        self.assertEqual(report.provenance_counts["simulated"], 0)
        forged = payload()
        forged["provenance_counts"]["simulated"] = 1
        with self.assertRaises(ArgumentError):
            workflow_execution_report(forged)

    def test_sync_async_workspace_and_http_facades_use_workflow_tool(self) -> None:
        request = WorkflowExecutionRequest.from_wire(request_wire())
        self.assertEqual(Workspace(_SyncTool()).interweave_workflow_execute_report(request).status, "refused")
        self.assertEqual(asyncio.run(AsyncWorkspace(_AsyncTool()).interweave_workflow_execute_report(request)).status, "refused")
        with patch.object(ApiClient, "call_tool", return_value=payload()) as call:
            self.assertEqual(ApiClient("http://127.0.0.1:1").interweave_workflow_execute_report(request).workflow, "reliable_software_repair")
        call.assert_called_once_with("interweave_workflow_execute", request.to_mcp_arguments())

    def test_evidence_request_report_and_facades_keep_digest_posture(self) -> None:
        request = WorkflowExecutionEvidenceRequest.from_wire({
            "binding": {"binding_digest": "b" * 64},
            "receipt": {"schema": "receipt"},
            "subject_id": "subject-1",
            "domains": ["software"],
            "parent_digests": ["e" * 64],
        })
        self.assertEqual(request.to_mcp_arguments()["subject_id"], "subject-1")
        report = workflow_execution_evidence_report(evidence_payload())
        self.assertIsInstance(report, WorkflowExecutionEvidenceReport)
        self.assertEqual(report.evidence_digest, evidence_payload()["evidence_digest"])
        forged = evidence_payload()
        forged["evidence"]["subject_id"] = "tampered"
        with self.assertRaises(ArgumentError):
            workflow_execution_evidence_report(forged)
        with patch.object(ApiClient, "call_tool", return_value=evidence_payload()) as call:
            result = ApiClient("http://127.0.0.1:1").interweave_workflow_execution_evidence_report(request)
            self.assertEqual(result.evidence_digest, evidence_payload()["evidence_digest"])
        call.assert_called_once_with("interweave_workflow_execution_evidence", request.to_mcp_arguments())
