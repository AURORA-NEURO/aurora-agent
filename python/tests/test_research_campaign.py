from __future__ import annotations

import asyncio
from copy import deepcopy
import json

import pytest

from prism_sdk import (
    RESEARCH_CAMPAIGN_CHECKPOINT_SCHEMA,
    RESEARCH_CAMPAIGN_OFFLINE_LIMITATIONS,
    RESEARCH_CAMPAIGN_OFFLINE_RESULT_SCHEMA,
    RESEARCH_CAMPAIGN_OFFLINE_TOOL,
    AsyncResearchCampaignClient,
    ResearchCampaignCheckpointMetadata,
    ResearchCampaignClient,
    ResearchCampaignManifestMetadata,
    ResearchCampaignOfflineExecution,
    ResearchCampaignOfflineRunArgs,
    ResearchCampaignOfflineRunRequest,
    ResearchCampaignOfflineRunResult,
    ResearchCampaignOfflineStage,
    ResearchCampaignTrustedHeadMetadata,
    research_campaign_offline_result,
)
from prism_sdk.errors import ArgumentError, ProtocolError, ToolRefusal
from prism_sdk.models import ToolResult


SPEC_DIGEST = "a" * 64
INPUT_DIGEST = "b" * 64
ARTIFACT_DIGEST = "c" * 64
RECEIPT_DIGEST = "d" * 64
FILE_DIGEST = "e" * 64
SNAPSHOT_DIGEST = "f" * 64
AUTHORIZATION_DIGEST = "1" * 64
MANIFEST_DIGEST = "2" * 64


def request(*, confirm: bool = False) -> ResearchCampaignOfflineRunRequest:
    return ResearchCampaignOfflineRunRequest(
        spec_path="campaign/spec.json",
        stage_input_paths={"measure": "campaign/inputs/measure.json"},
        output_dir="campaign/output",
        confirm=confirm,
    )


def not_started_stage(stage_id: str = "measure") -> dict[str, object]:
    return {
        "state": "not_started",
        "stage_id": stage_id,
        "kind": "synthetic_research",
        "input_digest": INPUT_DIGEST,
        "artifact_locator": "artifacts/0001-research-dossier.json",
    }


def preview_result() -> dict[str, object]:
    return {
        "schema": RESEARCH_CAMPAIGN_OFFLINE_RESULT_SCHEMA,
        "workflow": RESEARCH_CAMPAIGN_OFFLINE_TOOL,
        "execution": {"state": "not_started"},
        "campaign_id": "campaign-1",
        "spec_digest": SPEC_DIGEST,
        "campaign_status": "planned",
        "actions_used": 0,
        "stages": [not_started_stage()],
        "checkpoint": None,
        "trusted_head": None,
        "manifest": None,
        "written": [],
        "limitations": list(RESEARCH_CAMPAIGN_OFFLINE_LIMITATIONS),
    }


def terminal_result(state: str = "completed") -> dict[str, object]:
    disposition = {
        "completed": "succeeded",
        "awaiting_human_review": "awaiting_human_review",
        "refused": "refused",
        "needs_input": "missing_input",
        "exhausted": "exhausted",
    }[state]
    execution = {"state": state}
    if state != "completed":
        execution["stage_id"] = "measure"
    return {
        "schema": RESEARCH_CAMPAIGN_OFFLINE_RESULT_SCHEMA,
        "workflow": RESEARCH_CAMPAIGN_OFFLINE_TOOL,
        "execution": execution,
        "campaign_id": "campaign-1",
        "spec_digest": SPEC_DIGEST,
        "campaign_status": state,
        "actions_used": 1,
        "stages": [
            {
                "state": "settled",
                "stage_id": "measure",
                "kind": "synthetic_research",
                "input_digest": INPUT_DIGEST,
                "action_ordinal": 1,
                "disposition": disposition,
                "artifact_digest": ARTIFACT_DIGEST,
                "receipt_digest": RECEIPT_DIGEST,
                "artifact_locator": "artifacts/0001-research-dossier.json",
                "file_sha256": FILE_DIGEST,
            }
        ],
        "checkpoint": {
            "locator": "campaign.checkpoint.json",
            "schema": RESEARCH_CAMPAIGN_CHECKPOINT_SCHEMA,
            "generation": 2,
            "snapshot_digest": SNAPSHOT_DIGEST,
        },
        "trusted_head": {
            "locator": "campaign.head.json",
            "campaign_id": "campaign-1",
            "spec_digest": SPEC_DIGEST,
            "generation": 2,
            "snapshot_digest": SNAPSHOT_DIGEST,
        },
        "manifest": {
            "locator": "campaign.manifest.json",
            "digest": MANIFEST_DIGEST,
            "file_sha256": MANIFEST_DIGEST,
        },
        "written": [
            "campaign/output/authority/0001-authorization.json",
            "campaign/output/authority/0002-terminal.json",
            "campaign/output/artifacts/0001-research-dossier.json",
            "campaign/output/campaign.checkpoint.json",
            "campaign/output/campaign.head.json",
            "campaign/output/campaign.manifest.json",
        ],
        "limitations": list(RESEARCH_CAMPAIGN_OFFLINE_LIMITATIONS),
    }


def authorized_reconciliation_result() -> dict[str, object]:
    return {
        "schema": RESEARCH_CAMPAIGN_OFFLINE_RESULT_SCHEMA,
        "workflow": RESEARCH_CAMPAIGN_OFFLINE_TOOL,
        "execution": {
            "state": "reconciliation_required",
            "reason": "the authorized action has no verified native receipt",
        },
        "campaign_id": "campaign-1",
        "spec_digest": SPEC_DIGEST,
        "campaign_status": "reconciliation_required",
        "actions_used": 1,
        "stages": [
            {
                "state": "reconciliation_required",
                "stage_id": "measure",
                "kind": "synthetic_research",
                "input_digest": INPUT_DIGEST,
                "action_ordinal": 1,
                "authorization_digest": AUTHORIZATION_DIGEST,
                "artifact_locator": "artifacts/0001-research-dossier.json",
                "reason": "native execution outcome is unknown",
            }
        ],
        "checkpoint": {
            "locator": "authority/0001-authorization.json#/checkpoint",
            "schema": RESEARCH_CAMPAIGN_CHECKPOINT_SCHEMA,
            "generation": 1,
            "snapshot_digest": SNAPSHOT_DIGEST,
        },
        "trusted_head": {
            "locator": "authority/0001-authorization.json#/candidate_checkpoint_head",
            "campaign_id": "campaign-1",
            "spec_digest": SPEC_DIGEST,
            "generation": 1,
            "snapshot_digest": SNAPSHOT_DIGEST,
        },
        "manifest": None,
        "written": ["campaign/output/authority/0001-authorization.json"],
        "limitations": list(RESEARCH_CAMPAIGN_OFFLINE_LIMITATIONS),
    }


def committed_reconciliation_result() -> dict[str, object]:
    value = terminal_result()
    value["execution"] = {
        "state": "reconciliation_required",
        "reason": "native execution returned without a trustworthy completion receipt",
    }
    value["campaign_status"] = "reconciliation_required"
    value["stages"] = [
        {
            "state": "reconciliation_required",
            "stage_id": "measure",
            "kind": "synthetic_research",
            "input_digest": INPUT_DIGEST,
            "action_ordinal": 1,
            "authorization_digest": AUTHORIZATION_DIGEST,
            "artifact_locator": "artifacts/0001-research-dossier.json",
            "reason": "completion is unknown and requires reconciliation",
        }
    ]
    value["written"] = [
        "campaign/output/authority/0001-authorization.json",
        "campaign/output/authority/0002-terminal.json",
        "campaign/output/campaign.checkpoint.json",
        "campaign/output/campaign.head.json",
        "campaign/output/campaign.manifest.json",
    ]
    return value


def unestablished_reconciliation_result() -> dict[str, object]:
    value = preview_result()
    value["execution"] = {
        "state": "reconciliation_required",
        "reason": "no valid durable authorization could be established",
    }
    value["campaign_status"] = "reconciliation_required"
    return value


def http_envelope(payload: dict[str, object]) -> dict[str, object]:
    return {
        "ok": True,
        "tool": RESEARCH_CAMPAIGN_OFFLINE_TOOL,
        "request_id": "request-1",
        "mcp": {
            "jsonrpc": "2.0",
            "id": "request-1",
            "result": {
                "isError": False,
                "structuredContent": payload,
                "content": [{"type": "text", "text": json.dumps(payload)}],
            }
        },
        "guarantee": "REST and MCP calls share the same in-process tool dispatcher",
    }


class SyncHttpTransport:
    def __init__(self, payload: dict[str, object]) -> None:
        self.payload = payload
        self.calls: list[tuple[str, dict[str, object]]] = []

    def call_tool(self, name: str, arguments: dict[str, object]) -> dict[str, object]:
        self.calls.append((name, arguments))
        return http_envelope(self.payload)


class SyncMcpTransport:
    def __init__(self, payload: dict[str, object]) -> None:
        self.payload = payload
        self.calls: list[tuple[str, dict[str, object]]] = []

    def call_tool(self, name: str, arguments: dict[str, object]) -> ToolResult:
        self.calls.append((name, arguments))
        return ToolResult(
            name,
            {
                "isError": False,
                "content": [{"type": "text", "text": json.dumps(self.payload)}],
            },
        )


class AsyncHttpTransport(SyncHttpTransport):
    async def call_tool(self, name: str, arguments: dict[str, object]) -> dict[str, object]:
        self.calls.append((name, arguments))
        return http_envelope(self.payload)


class AsyncMcpTransport(SyncMcpTransport):
    async def call_tool(self, name: str, arguments: dict[str, object]) -> ToolResult:
        self.calls.append((name, arguments))
        return ToolResult(
            name,
            {
                "isError": False,
                "content": [{"type": "text", "text": json.dumps(self.payload)}],
            },
        )


def test_request_is_exact_path_only_and_confirmation_is_never_promoted() -> None:
    value = request()
    assert isinstance(value, ResearchCampaignOfflineRunArgs)
    assert value.to_mcp_arguments() == {
        "spec_path": "campaign/spec.json",
        "stage_input_paths": {"measure": "campaign/inputs/measure.json"},
        "output_dir": "campaign/output",
        "confirm": False,
    }
    assert not ({"objective", "question", "prompt", "content"} & value.to_mcp_arguments().keys())
    with pytest.raises(TypeError):
        value.stage_input_paths["extra"] = "campaign/inputs/extra.json"  # type: ignore[index]


@pytest.mark.parametrize(
    "wire",
    [
        {"spec_path": "/campaign/spec.json", "stage_input_paths": {"measure": "inputs/a.json"}, "output_dir": "out"},
        {"spec_path": "C:/campaign/spec.json", "stage_input_paths": {"measure": "inputs/a.json"}, "output_dir": "out"},
        {"spec_path": "campaign\\spec.json", "stage_input_paths": {"measure": "inputs/a.json"}, "output_dir": "out"},
        {"spec_path": "campaign/../spec.json", "stage_input_paths": {"measure": "inputs/a.json"}, "output_dir": "out"},
        {"spec_path": "campaign//spec.json", "stage_input_paths": {"measure": "inputs/a.json"}, "output_dir": "out"},
        {"spec_path": "campaign/./spec.json", "stage_input_paths": {"measure": "inputs/a.json"}, "output_dir": "out"},
        {"spec_path": "campaign/spec.json", "stage_input_paths": {}, "output_dir": "out"},
        {"spec_path": "campaign/spec.json", "stage_input_paths": {" measure": "inputs/a.json"}, "output_dir": "out"},
        {"spec_path": "campaign/spec.json", "stage_input_paths": {"measure": "../outside.json"}, "output_dir": "out"},
        {"spec_path": "campaign/spec.json", "stage_input_paths": {"measure": "inputs/a.json"}, "output_dir": "out", "confirm": 1},
        {"spec_path": "campaign/spec.json", "stage_input_paths": {"measure": "inputs/a.json"}, "output_dir": "out", "verify_existing": True},
        {"spec_path": "campaign/spec.json", "stage_input_paths": {"measure": "inputs/a.json"}, "output_dir": "out", "prompt": "private"},
        {"spec_path": "campaign/spec.json", "stage_input_paths": {f"s{i}": f"inputs/{i}.json" for i in range(9)}, "output_dir": "out"},
    ],
)
def test_request_rejects_ambiguous_paths_maps_types_and_unknown_fields(
    wire: dict[str, object],
) -> None:
    with pytest.raises(ArgumentError):
        ResearchCampaignOfflineRunRequest.from_wire(wire)


def test_preview_and_completed_results_are_strict_metadata_only_models() -> None:
    preview = research_campaign_offline_result(preview_result())
    assert isinstance(preview, ResearchCampaignOfflineRunResult)
    assert isinstance(preview.execution, ResearchCampaignOfflineExecution)
    assert isinstance(preview.stages[0], ResearchCampaignOfflineStage)
    assert preview.preview and not preview.completed
    assert preview.checkpoint is None and preview.manifest is None

    completed = research_campaign_offline_result(terminal_result())
    completed.validate_request(request(confirm=True))
    assert completed.completed and not completed.preview
    assert isinstance(completed.checkpoint, ResearchCampaignCheckpointMetadata)
    assert isinstance(completed.trusted_head, ResearchCampaignTrustedHeadMetadata)
    assert isinstance(completed.manifest, ResearchCampaignManifestMetadata)
    assert completed.to_dict() == terminal_result()
    assert not ({"objective", "question", "sources", "artifact"} & completed.to_dict().keys())


def test_all_reconciliation_shapes_remain_non_complete_domain_results() -> None:
    authorized = research_campaign_offline_result(authorized_reconciliation_result())
    authorized.validate_request(request(confirm=True))
    assert authorized.execution.state == "reconciliation_required"
    assert not authorized.completed
    assert authorized.stages[0].reconciliation_required
    assert authorized.stages[0].authorization_digest == AUTHORIZATION_DIGEST
    assert authorized.manifest is None

    committed = research_campaign_offline_result(committed_reconciliation_result())
    committed.validate_request(request(confirm=True))
    assert committed.execution.state == "reconciliation_required"
    assert not committed.completed
    assert committed.manifest is not None
    assert committed.checkpoint is not None
    assert committed.checkpoint.locator == "campaign.checkpoint.json"

    unestablished = research_campaign_offline_result(unestablished_reconciliation_result())
    unestablished.validate_request(request(confirm=True))
    assert unestablished.actions_used == 0
    assert unestablished.written == ()
    assert all(stage.state == "not_started" for stage in unestablished.stages)


def test_reconciliation_action_follows_contiguous_verified_stages() -> None:
    payload = authorized_reconciliation_result()
    prior = terminal_result()["stages"][0]  # type: ignore[index]
    prior["stage_id"] = "collect"
    payload["actions_used"] = 2
    payload["stages"][0]["action_ordinal"] = 2  # type: ignore[index]
    payload["stages"][0]["artifact_locator"] = "artifacts/0002-research-dossier.json"  # type: ignore[index]
    payload["stages"] = [prior, payload["stages"][0]]  # type: ignore[index]
    payload["checkpoint"]["generation"] = 2  # type: ignore[index]
    payload["checkpoint"]["locator"] = "authority/0002-authorization.json#/checkpoint"  # type: ignore[index]
    payload["trusted_head"]["generation"] = 2  # type: ignore[index]
    payload["trusted_head"]["locator"] = "authority/0002-authorization.json#/candidate_checkpoint_head"  # type: ignore[index]
    payload["written"] = [
        "campaign/output/artifacts/0001-research-dossier.json",
        "campaign/output/authority/0002-authorization.json",
    ]
    result = research_campaign_offline_result(payload)
    assert [stage.state for stage in result.stages] == [
        "settled",
        "reconciliation_required",
    ]
    assert result.actions_used == 2


def test_action_ordinals_are_bound_to_exact_campaign_stage_order() -> None:
    payload = terminal_result()
    first = deepcopy(payload["stages"][0])  # type: ignore[index]
    second = deepcopy(first)
    second.update(
        {
            "stage_id": "plan",
            "kind": "brain_plan",
            "input_digest": "3" * 64,
            "action_ordinal": 2,
            "artifact_digest": "4" * 64,
            "receipt_digest": "5" * 64,
            "artifact_locator": "artifacts/0002-brain-plan-report.json",
            "file_sha256": "6" * 64,
        }
    )
    payload["actions_used"] = 2
    payload["stages"] = [first, second]
    payload["checkpoint"]["generation"] = 3  # type: ignore[index]
    payload["trusted_head"]["generation"] = 3  # type: ignore[index]
    payload["written"] = [
        "campaign/output/authority/0001-authorization.json",
        "campaign/output/authority/0002-authorization.json",
        "campaign/output/authority/0003-terminal.json",
        "campaign/output/artifacts/0001-research-dossier.json",
        "campaign/output/artifacts/0002-brain-plan-report.json",
        "campaign/output/campaign.checkpoint.json",
        "campaign/output/campaign.head.json",
        "campaign/output/campaign.manifest.json",
    ]
    canonical = research_campaign_offline_result(payload)
    assert [stage.action_ordinal for stage in canonical.stages] == [1, 2]

    swapped = deepcopy(payload)
    swapped["stages"][0]["action_ordinal"] = 2  # type: ignore[index]
    swapped["stages"][1]["action_ordinal"] = 1  # type: ignore[index]
    with pytest.raises(ProtocolError, match="campaign order"):
        research_campaign_offline_result(swapped)


@pytest.mark.parametrize(
    "state",
    [
        "not_started",
        "completed",
        "awaiting_human_review",
        "refused",
        "needs_input",
        "exhausted",
        "reconciliation_required",
    ],
)
def test_execution_states_are_preserved_without_deriving_success(state: str) -> None:
    if state == "not_started":
        payload = preview_result()
    elif state == "reconciliation_required":
        payload = authorized_reconciliation_result()
    else:
        payload = terminal_result(state)
    result = research_campaign_offline_result(payload)
    assert result.execution.state == state
    assert result.completed is (state == "completed")


def test_sync_http_and_mcp_clients_normalize_the_same_typed_result() -> None:
    http = SyncHttpTransport(preview_result())
    mcp = SyncMcpTransport(preview_result())
    from_http = ResearchCampaignClient.from_http(http).run_offline(request())
    from_mcp = ResearchCampaignClient.from_mcp(mcp).run_offline(request())
    assert from_http == from_mcp
    expected = (RESEARCH_CAMPAIGN_OFFLINE_TOOL, request().to_mcp_arguments())
    assert http.calls == [expected]
    assert mcp.calls == [expected]


def test_async_http_and_mcp_clients_mirror_sync_behavior() -> None:
    async def run() -> None:
        http = AsyncHttpTransport(preview_result())
        mcp = AsyncMcpTransport(preview_result())
        from_http = await AsyncResearchCampaignClient.from_http(http).run_offline(request())
        from_mcp = await AsyncResearchCampaignClient.from_mcp(mcp).run_offline(request())
        assert from_http == from_mcp
        assert http.calls[0][1]["confirm"] is False
        assert mcp.calls[0][1]["confirm"] is False

    asyncio.run(run())


def test_transport_refusal_is_distinct_from_valid_refused_campaign_state() -> None:
    refused = ResearchCampaignClient(lambda _name, _args: terminal_result("refused"))
    result = refused.run_offline(request(confirm=True))
    assert result.execution.state == "refused"

    transport_refusal = {
        "ok": True,
        "tool": RESEARCH_CAMPAIGN_OFFLINE_TOOL,
        "request_id": "request-refused",
        "mcp": {
            "jsonrpc": "2.0",
            "id": "request-refused",
            "result": {
                "isError": True,
                "content": [
                    {"type": "text", "text": json.dumps({"ok": False, "error": "invalid input"})}
                ],
            }
        },
        "guarantee": "REST and MCP calls share the same in-process tool dispatcher",
    }
    with pytest.raises(ToolRefusal) as error:
        ResearchCampaignClient(lambda _name, _args: transport_refusal).run_offline(request())
    assert error.value.payload == {"ok": False, "error": "invalid input"}

    forged = deepcopy(transport_refusal)
    forged["mcp"]["result"]["structuredContent"] = terminal_result()  # type: ignore[index]
    with pytest.raises(ProtocolError, match="structured success"):
        ResearchCampaignClient(lambda _name, _args: forged).run_offline(request())


def test_result_parser_handles_http_and_mcp_envelopes_and_rejects_conflicts() -> None:
    payload = preview_result()
    assert research_campaign_offline_result(http_envelope(payload)).to_dict() == payload

    conflicting = http_envelope(payload)
    other = deepcopy(payload)
    other["campaign_id"] = "other-campaign"
    conflicting["mcp"]["result"]["content"] = [  # type: ignore[index]
        {"type": "text", "text": json.dumps(other)}
    ]
    with pytest.raises(ProtocolError, match="disagree"):
        research_campaign_offline_result(conflicting)

    wrong_tool = http_envelope(payload)
    wrong_tool["tool"] = "another_tool"
    with pytest.raises(ProtocolError, match="different tool"):
        research_campaign_offline_result(wrong_tool)

    wrong_id = http_envelope(payload)
    wrong_id["mcp"]["id"] = "cross-wired"  # type: ignore[index]
    with pytest.raises(ProtocolError, match="identity"):
        research_campaign_offline_result(wrong_id)

    leaked_block = http_envelope(payload)
    leaked_block["mcp"]["result"]["content"][0]["raw_objective"] = "secret"  # type: ignore[index]
    with pytest.raises(ProtocolError):
        research_campaign_offline_result(leaked_block)


def test_malformed_positive_metadata_and_union_tampering_fail_closed() -> None:
    malformed: list[dict[str, object]] = []

    extra = terminal_result()
    extra["ok"] = True
    malformed.append(extra)

    execution_extra = terminal_result()
    execution_extra["execution"] = {"state": "completed", "reason": "trust me"}
    malformed.append(execution_extra)

    missing_digest = terminal_result()
    del missing_digest["stages"][0]["receipt_digest"]  # type: ignore[index]
    malformed.append(missing_digest)

    drifted_head = terminal_result()
    drifted_head["trusted_head"]["snapshot_digest"] = "0" * 64  # type: ignore[index]
    malformed.append(drifted_head)

    missing_manifest = terminal_result()
    missing_manifest["manifest"] = None
    malformed.append(missing_manifest)

    duplicate_write = terminal_result()
    duplicate_write["written"].append(duplicate_write["written"][0])  # type: ignore[union-attr,index]
    malformed.append(duplicate_write)

    bad_reconciliation = authorized_reconciliation_result()
    bad_reconciliation["stages"][0]["disposition"] = "succeeded"  # type: ignore[index]
    malformed.append(bad_reconciliation)

    wrong_ordinal = authorized_reconciliation_result()
    wrong_ordinal["stages"][0]["action_ordinal"] = 2  # type: ignore[index]
    malformed.append(wrong_ordinal)

    split_reconciliation_head = authorized_reconciliation_result()
    split_reconciliation_head["trusted_head"]["locator"] = (  # type: ignore[index]
        "authority/0002-authorization.json#/candidate_checkpoint_head"
    )
    split_reconciliation_head["written"].append(  # type: ignore[union-attr]
        "campaign/output/authority/0002-authorization.json"
    )
    malformed.append(split_reconciliation_head)

    changed_limitations = terminal_result()
    changed_limitations["limitations"] = ["offline only"]
    malformed.append(changed_limitations)

    split_manifest_digest = terminal_result()
    split_manifest_digest["manifest"]["file_sha256"] = "3" * 64  # type: ignore[index]
    malformed.append(split_manifest_digest)

    noncanonical_artifact = terminal_result()
    noncanonical_artifact["stages"][0]["artifact_locator"] = "artifacts/result.json"  # type: ignore[index]
    noncanonical_artifact["written"][2] = "campaign/output/artifacts/result.json"  # type: ignore[index]
    malformed.append(noncanonical_artifact)

    for payload in malformed:
        with pytest.raises(ProtocolError):
            research_campaign_offline_result(payload)


def test_client_binds_confirmation_stage_set_and_output_directory() -> None:
    with pytest.raises(ProtocolError, match="unconfirmed"):
        ResearchCampaignClient(lambda _name, _args: terminal_result()).run_offline(request())

    with pytest.raises(ProtocolError, match="preview"):
        ResearchCampaignClient(lambda _name, _args: preview_result()).run_offline(
            request(confirm=True)
        )

    wrong_stage = terminal_result()
    wrong_stage["stages"][0]["stage_id"] = "other"  # type: ignore[index]
    with pytest.raises(ProtocolError, match="stage ids"):
        ResearchCampaignClient(lambda _name, _args: wrong_stage).run_offline(
            request(confirm=True)
        )

    escaped_write = terminal_result()
    escaped_write["written"][0] = "other-output/artifact.json"  # type: ignore[index]
    with pytest.raises(ProtocolError, match="outside"):
        ResearchCampaignClient(lambda _name, _args: escaped_write).run_offline(
            request(confirm=True)
        )


def test_invalid_request_never_reaches_transport() -> None:
    calls = 0

    def transport(_name: str, _arguments: dict[str, object]) -> dict[str, object]:
        nonlocal calls
        calls += 1
        return preview_result()

    with pytest.raises(ArgumentError):
        ResearchCampaignClient(transport).run_offline(
            {
                "spec_path": "campaign/../spec.json",
                "stage_input_paths": {"measure": "inputs/measure.json"},
                "output_dir": "campaign/output",
            }
        )
    assert calls == 0
