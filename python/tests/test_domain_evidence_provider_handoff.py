from __future__ import annotations

import asyncio
from unittest.mock import AsyncMock, patch

import pytest

from prism_sdk import (
    AsyncApiClient,
    AsyncWorkspace,
    ApiClient,
    ArgumentError,
    DomainEvidenceProviderAuthPosture,
    DomainEvidenceProviderConnectorManifest,
    DomainEvidenceProviderHandoffRequest,
    Workspace,
    domain_evidence_provider_handoff_report,
)


def request() -> DomainEvidenceProviderHandoffRequest:
    return DomainEvidenceProviderHandoffRequest(
        group_id="biological_domains",
        domains=("oncology", "genomics"),
        subject_id="connector-subject",
        source_tool="literature_bind_check",
        provider="pubmed",
        connector_kind="literature",
        manifest=DomainEvidenceProviderConnectorManifest(
            connector_id="caller.pubmed",
            version="1.2.0",
            provider="pubmed",
            connector_kind="literature",
            domains=("genomics", "oncology"),
            capabilities=("query", "retain"),
            auth_posture=DomainEvidenceProviderAuthPosture(
                status="caller_asserted",
                secret_refs=("secret://caller/pubmed",),
                does_not_claim=("provider authentication",),
            ),
        ),
        status="prepared",
        request_digest="a" * 64,
        payload_digest="b" * 64,
        source_plan_digest="c" * 64,
        parent_digests=("d" * 64,),
        attempt_id="attempt-1",
    )


def payload() -> dict:
    handoff = request().to_mcp_arguments()
    handoff.update(
        {
            "schema": "bioprism-devplat-domain-evidence-provider-connector-handoff/0.1",
            "workflow": "domain_evidence_provider_connector_handoff",
            "manifest_digest": "e" * 64,
            "handoff_digest": "f" * 64,
            "execution": "not_started",
            "readiness_claimed": False,
            "guarantees": ["scope is validated"],
            "limitations": ["no provider contact"],
        }
    )
    return {
        "ok": True,
        "schema": "bioprism-devplat-domain-evidence-provider-connector-handoff/0.1",
        "workflow": "domain_evidence_provider_connector_handoff",
        "handoff": handoff,
        "manifest_digest": "e" * 64,
        "handoff_digest": "f" * 64,
        "artifact_registry": {"created": True, "indexed": True},
        "execution": "not_started",
        "readiness_claimed": False,
        "guarantees": ["scope is validated"],
        "does_not_claim": ["no provider contact"],
    }


def test_handoff_is_typed_scoped_and_does_not_model_credentials() -> None:
    normalized = request().to_mcp_arguments()
    assert normalized["manifest"]["transport"] == "caller_managed"
    assert normalized["manifest"]["auth_posture"]["secret_refs"] == ["secret://caller/pubmed"]
    assert "credential_material" not in normalized
    report = domain_evidence_provider_handoff_report(payload())
    assert report.handoff_digest == "f" * 64
    assert report.manifest.connector_id == "caller.pubmed"
    assert report.readiness_claimed is False


def test_handoff_rejects_scope_and_credential_fields() -> None:
    with pytest.raises(ArgumentError):
        DomainEvidenceProviderHandoffRequest.from_wire(
            {**request().to_mcp_arguments(), "credential_material": "never"}
        )
    with pytest.raises(ArgumentError):
        DomainEvidenceProviderHandoffRequest(
            **{**request().__dict__, "domains": ("imaging",)}
        )


def test_sync_and_async_handoff_helpers_parse_rest_tool_and_workspace() -> None:
    with patch.object(ApiClient, "request", return_value=payload()) as rest:
        assert ApiClient("http://127.0.0.1:8787").domain_evidence_provider_connector_handoff(request()).handoff_digest == "f" * 64
        assert rest.call_args.args[1] == "/v1/tools/domain_evidence_provider_connector_handoff"
    with patch.object(ApiClient, "call_tool", return_value=payload()) as tool:
        assert ApiClient("http://127.0.0.1:8787").domain_evidence_provider_connector_handoff_tool(request()).provider == "pubmed"
        assert tool.call_args.args[0] == "domain_evidence_provider_connector_handoff"
    with patch.object(Workspace, "tool", return_value=payload()):
        assert Workspace(None).domain_evidence_provider_connector_handoff_report(request()).status == "prepared"

    async def run() -> None:
        client = AsyncApiClient(ApiClient("http://127.0.0.1:8787"))
        with patch.object(ApiClient, "request", return_value=payload()):
            assert (await client.domain_evidence_provider_connector_handoff(request())).provider == "pubmed"
        with patch.object(AsyncWorkspace, "tool", new_callable=AsyncMock, return_value=payload()):
            assert (
                await AsyncWorkspace(None).domain_evidence_provider_connector_handoff_report(request())
            ).handoff_digest == "f" * 64

    asyncio.run(run())
