from __future__ import annotations

import asyncio
import json
import unittest
from unittest.mock import patch

from prism_sdk import (
    ApiClient,
    AsyncApiClient,
    AsyncWorkspace,
    CiProviderEvidenceReport,
    CiProviderEvidenceRequest,
    CiProviderEvidenceRegistryQueryRequest,
    ci_provider_evidence_registry_get_report,
    ci_provider_evidence_registry_import_report,
    ci_provider_evidence_registry_query_report,
    Workspace,
    ci_provider_evidence_report,
)
from prism_sdk.errors import ArgumentError
from prism_sdk.models import ToolResult


def payload() -> dict:
    digest = "a" * 64
    audit = {
        "schema": "bioprism-devplat-ci-provider-evidence/0.1",
        "workflow": "ci_provider_evidence_audit",
        "provider": "github_actions",
        "source": "provider_observed",
        "run_id": "run-42",
        "payload_digest": digest,
        "plan_digest": digest,
        "evidence_digest": digest,
        "artifact_record_digest": digest,
        "log_record_digest": digest,
        "attestation_record_digest": digest,
        "artifact_count": 1,
        "log_count": 1,
        "attestation_count": 1,
        "linked_artifact_count": 1,
        "linked_log_count": 1,
        "attestation_subject_count": 1,
        "local_byte_hash_artifact_count": 1,
        "local_byte_hash_log_count": 0,
        "attestation_subject_digest_binding_count": 1,
        "ci_evidence": {"run_id": "run-42"},
        "artifacts": [{"id": "artifact-1", "kind": "junit", "digest": digest, "digest_scope": "local_response_bytes"}],
        "logs": [{"id": "log-1", "digest": digest}],
        "attestations": [{"id": "attestation-1", "subject": "artifact-1", "issuer": "caller", "statement_digest": digest, "method": "declared", "subject_digest": digest}],
        "structurally_valid": True,
        "conformance_ready": True,
        "execution": "evidence_supplied_not_executed_here",
        "verification": "structural_only_with_digest_bindings",
        "findings": [],
        "guarantees": [],
        "limitations": [],
    }
    return {
        "ok": True,
        "workflow": "ci_provider_evidence_audit",
        "schema": "bioprism-devplat-ci-provider-evidence/0.1",
        "valid": True,
        "conformance_ready": True,
        "evidence": {"run_id": "run-42", "provider": "github_actions", "checks": []},
        "audit": audit,
        "guarantees": [],
        "limitations": [],
    }


def registry_payload(name: str) -> dict:
    digest = "c" * 64
    if name == "ci_provider_evidence_import":
        return {
            "ok": True, "workflow": name, "provider_evidence_digest": digest,
            "provider": "github_actions", "run_id": "run-42", "plan_digest": digest,
            "evidence_digest": digest, "artifact_record_digest": digest,
            "log_record_digest": digest, "attestation_record_digest": digest,
            "structurally_valid": True, "conformance_ready": True,
            "created": True, "already_present": False, "registry_generation": 1,
            "registry_size": 1,
        }
    if name == "ci_provider_evidence_query":
        return {
            "ok": True, "workflow": name, "rows": [{"provider_evidence_digest": digest, "provider": "github_actions"}],
            "next_after": None, "has_more": False, "registry_generation": 1, "registry_size": 1,
        }
    return {
        "ok": True, "workflow": name, "provider_evidence_digest": digest,
        "provider": "github_actions", "run_id": "run-42", "audit": {"run_id": "run-42"},
        "registry_generation": 1, "registry_size": 1,
    }
class _SyncTool:
    def call_tool(self, name: str, arguments: dict) -> ToolResult:
        value = payload() if name == "ci_provider_evidence_audit" else registry_payload(name)
        return ToolResult(name, {"isError": False, "content": [{"type": "text", "text": json.dumps(value)}]})


class _AsyncTool:
    async def call_tool(self, name: str, arguments: dict) -> ToolResult:
        value = payload() if name == "ci_provider_evidence_audit" else registry_payload(name)
        return ToolResult(name, {"isError": False, "content": [{"type": "text", "text": json.dumps(value)}]})


def request() -> CiProviderEvidenceRequest:
    digest = "b" * 64
    return CiProviderEvidenceRequest(
        ci={"workflow": "contracts", "checks": []},
        provider="github_actions",
        payload={"run": {"id": 42, "conclusion": "success"}, "jobs": []},
        artifacts=[{"id": "artifact-1", "kind": "junit", "digest": digest, "digest_scope": "local_response_bytes", "run_id": "42", "provider": "github_actions", "uri": "https://example.test/artifact"}],
        logs=[{"id": "log-1", "digest": digest, "digest_scope": "caller_declared", "run_id": "42", "provider": "github_actions"}],
        attestations=[{"id": "attestation-1", "subject": "artifact-1", "issuer": "caller", "statement_digest": digest, "method": "declared", "subject_digest": digest}],
    )


class CiProviderEvidenceTests(unittest.TestCase):
    def test_request_and_report_preserve_rows_and_structural_boundary(self) -> None:
        args = request()
        wire = args.to_mcp_arguments()
        self.assertEqual(wire["artifacts"][0]["id"], "artifact-1")
        report = ci_provider_evidence_report(payload())
        self.assertIsInstance(report, CiProviderEvidenceReport)
        self.assertTrue(report.conformance_ready)
        self.assertEqual(report.attestation_subject_count, 1)
        self.assertEqual(report.local_byte_hash_artifact_count, 1)
        self.assertEqual(report.attestation_subject_digest_binding_count, 1)
        self.assertEqual(report.evidence["run_id"], "run-42")
        self.assertEqual(report.verification, "structural_only_with_digest_bindings")
        with self.assertRaises(ArgumentError):
            CiProviderEvidenceRequest({}, "generic", {})
        with self.assertRaises(ArgumentError):
            CiProviderEvidenceRequest({}, "generic", {}, artifacts=[{} for _ in range(129)])

    def test_all_facades_keep_provider_evidence_typed(self) -> None:
        args = request()
        self.assertTrue(Workspace(_SyncTool()).ci_provider_evidence_report(args).conformance_ready)
        self.assertTrue(asyncio.run(AsyncWorkspace(_AsyncTool()).ci_provider_evidence_report(args)).structurally_valid)
        with patch.object(ApiClient, "call_tool", return_value=payload()) as call:
            result = ApiClient("http://127.0.0.1:1").ci_provider_evidence_report(args)
        self.assertEqual(result.artifact_count, 1)
        call.assert_called_once_with("ci_provider_evidence_audit", args.to_mcp_arguments())

        async def run() -> None:
            with patch.object(ApiClient, "call_tool", return_value=payload()) as async_call:
                result = await AsyncApiClient(ApiClient("http://127.0.0.1:1")).ci_provider_evidence_report(args)
            self.assertEqual(result.execution, "evidence_supplied_not_executed_here")
            async_call.assert_called_once_with("ci_provider_evidence_audit", args.to_mcp_arguments())

        asyncio.run(run())

    def test_registry_request_reports_and_all_transport_planes(self) -> None:
        args = request()
        query = CiProviderEvidenceRegistryQueryRequest(
            provider="github_actions", conformance_ready=True, max_items=12, include_records=True
        )
        self.assertEqual(query.to_mcp_arguments()["max_items"], 12)
        self.assertEqual(query.to_http_query()["conformance_ready"], "true")
        self.assertIsInstance(ci_provider_evidence_registry_import_report(registry_payload("ci_provider_evidence_import")), object)
        self.assertEqual(ci_provider_evidence_registry_query_report(registry_payload("ci_provider_evidence_query")).rows[0]["provider"], "github_actions")
        self.assertEqual(ci_provider_evidence_registry_get_report(registry_payload("ci_provider_evidence_get")).run_id, "run-42")

        workspace = Workspace(_SyncTool())
        self.assertTrue(workspace.ci_provider_evidence_import_report(args).created)
        self.assertEqual(workspace.ci_provider_evidence_query_report(query).rows[0]["provider"], "github_actions")
        self.assertEqual(workspace.ci_provider_evidence_get_report("c" * 64).run_id, "run-42")

        with patch.object(ApiClient, "call_tool", side_effect=lambda name, arguments: registry_payload(name)) as call, \
             patch.object(ApiClient, "request", side_effect=lambda method, path, payload=None: registry_payload("ci_provider_evidence_import" if method == "POST" else ("ci_provider_evidence_get" if path.count("/") >= 4 else "ci_provider_evidence_query"))) as rest:
            client = ApiClient("http://127.0.0.1:1")
            self.assertTrue(client.ci_provider_evidence_import_report(args).created)
            self.assertTrue(client.ci_provider_evidence_import_rest_report(args).created)
            self.assertFalse(client.ci_provider_evidence_query_rest_report(query).has_more)
            self.assertEqual(client.ci_provider_evidence_get_rest_report("c" * 64).run_id, "run-42")
            self.assertEqual(call.call_args_list[0].args[0], "ci_provider_evidence_import")
            self.assertEqual(rest.call_args_list[0].args[0], "POST")

        async def run_registry() -> None:
            async_client = AsyncApiClient(ApiClient("http://127.0.0.1:1"))
            with patch.object(ApiClient, "call_tool", side_effect=lambda name, arguments: registry_payload(name)), \
                 patch.object(ApiClient, "request", side_effect=lambda method, path, payload=None: registry_payload("ci_provider_evidence_query")):
                self.assertTrue((await async_client.ci_provider_evidence_import_report(args)).created)
                self.assertEqual((await async_client.ci_provider_evidence_query_rest_report(query)).registry_size, 1)
                self.assertEqual((await async_client.ci_provider_evidence_get_report("c" * 64)).run_id, "run-42")

        asyncio.run(run_registry())


if __name__ == "__main__":
    unittest.main()
