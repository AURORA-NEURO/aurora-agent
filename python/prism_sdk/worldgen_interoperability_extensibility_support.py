"""Deterministic Python parity for Worldgen P22 interoperability/extensibility."""
from __future__ import annotations

import hashlib
import json
import re
from typing import Any, Mapping

from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError

TARGET_VERSION = "1.0.0"
COMPATIBLE_VERSION = "0.9.0"
CONTENT_TYPE = "application/vnd.aurora.worldgen.interoperability-extensibility-receipt-1+json"


def _hash(value: Any) -> str:
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()).hexdigest()


def _digest(value: Any) -> bool:
    return isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) is not None


def _ordered(values: list[str]) -> bool:
    return values == sorted(set(values))


def manifest(*, feature_id: str, contract_version: str, scale: str, mode: str) -> dict[str, Any]:
    return {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "capability_id": feature_id, "version": contract_version, "owner_crate": "worldgen", "consumers": ["schema steward", "connector developer", "research workflow orchestrator", "federation operator"], "behavior": f"negotiate version-pinned capabilities and extensibility for {scale} at {mode} scale", "value": "prevents incompatible schemas, undeclared extensions, migration loss, and unsafe effects from crossing a research boundary", "input_schema": "ExtensibilityRequest4@1", "output_schema": "ExtensibilityReceipt7@1", "effects": ["exchange:capability-manifest", "block:unsafe-release"], "permissions": ["negotiate:declared-extension"], "determinism": "byte_stable", "autonomy_tier": "A1", "boundary": PRECLINICAL_BOUNDARY}


def validate(output: Mapping[str, Any]) -> None:
    artifact = output.get("artifact", {})
    if output.get("schema_version") != RESEARCH_CONTRACT_SCHEMA_VERSION or output.get("boundary") != PRECLINICAL_BOUNDARY or not output.get("raw_data_local") is True or output.get("aggregate_only") is not True or not output.get("capability_order") or not _digest(output.get("replay_identity")) or not _digest(output.get("schema_digest")) or not _digest(output.get("receipt_digest")) or artifact.get("content_type") != CONTENT_TYPE or artifact.get("content_hash") != output.get("receipt_digest") or artifact.get("boundary") != PRECLINICAL_BOUNDARY or artifact.get("semantic_loss", []) != output.get("semantic_loss_order", []):
        raise ResearchContractError("interoperability receipt identity, locality, digest, artifact, or boundary is incomplete")
    for key in ("capability_order", "extension_order", "missing_capability_order", "unsupported_extension_order", "omission_order", "uncertainty_order", "semantic_loss_order", "stage_order", "completed_stage_order", "pending_stage_order", "artifact_digest_order", "effect_receipts"):
        if not _ordered(output.get(key, [])):
            raise ResearchContractError("interoperability vectors are not canonical")
    stages = set(output.get("stage_order", []))
    if stages != set(output.get("completed_stage_order", [])) | set(output.get("pending_stage_order", [])):
        raise ResearchContractError("workflow stages do not partition")


def negotiate(request: Mapping[str, Any], *, feature_id: str, contract_version: str, scale: str, mode: str) -> dict[str, Any]:
    required = ("request_id", "source_contract_version", "target_contract_version")
    if any(not isinstance(request.get(key), str) or not request[key].strip() for key in required) or not request.get("supported_contract_versions") or not request.get("offered_capability_order") or not request.get("required_capability_order") or not request.get("extension_order") or not request.get("artifact_digest_order") or not _digest(request.get("schema_digest")) or not _digest(request.get("replay_identity")) or request.get("boundary") != PRECLINICAL_BOUNDARY or request.get("raw_data_local") is not True or request.get("aggregate_only") is not True or any(not _ordered(request.get(key, [])) for key in ("supported_contract_versions", "offered_capability_order", "required_capability_order", "extension_order")):
        raise ResearchContractError("interoperability request identity, versions, order, digests, locality, or boundary is invalid")
    offered = set(request["offered_capability_order"])
    required_caps = set(request["required_capability_order"])
    capability_order = sorted(offered | required_caps)
    missing = sorted(required_caps - offered)
    unsupported = sorted(item for item in request["extension_order"] if item not in offered and item not in required_caps)
    omission: set[str] = set()
    uncertainty: set[str] = set()
    loss: set[str] = set()
    if request["target_contract_version"] == TARGET_VERSION and request["source_contract_version"] == TARGET_VERSION:
        negotiated_version, disposition = TARGET_VERSION, "accepted"
    elif request["target_contract_version"] == TARGET_VERSION and request["source_contract_version"] == COMPATIBLE_VERSION and TARGET_VERSION in request["supported_contract_versions"]:
        negotiated_version, disposition = TARGET_VERSION, "migrated"
        loss.add("legacy-extension-semantics")
        omission.add("migration:legacy-fields-not-inferred")
    else:
        negotiated_version, disposition = request["target_contract_version"], "incompatible"
        uncertainty.add("contract-version-outside-compatibility-window")
    if missing:
        omission.add("required-capability-missing")
        disposition = "unknown"
    if unsupported:
        loss.add("undeclared-extension-rejected")
        uncertainty.add("extension-not-offered-by-source")
        if disposition == "accepted":
            disposition = "unknown"
    if not request.get("policy_allowed") or request.get("raw_data_local") is not True or request.get("aggregate_only") is not True:
        omission.add("policy-or-locality-denied")
        disposition = "blocked"
    if not request.get("protected_closure") or not request.get("signed_approval"):
        omission.add("protected-closure-or-approval-missing")
        if mode in {"copilot", "workflow"}:
            disposition = "approval_required"
    if mode == "copilot" and (request.get("action_budget", 0) <= 0 or request.get("action_count", 0) > request["action_budget"]):
        omission.add("copilot:action-budget-exceeded")
        disposition = "blocked"
    completed = sorted(set(request.get("completed_stage_order", [])))
    pending = sorted(set(request.get("stage_order", [])) - set(completed))
    if mode == "workflow" and pending and disposition == "accepted":
        disposition = "partial"
    if disposition not in {"accepted", "migrated"}:
        omission.add("release:unsafe-interoperability-state")
    payload = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": contract_version, "feature_id": feature_id, "mode": mode, "scale": scale, "request_id": request["request_id"], "negotiated_version": negotiated_version, "disposition": disposition, "capability_order": capability_order, "extension_order": sorted(request["extension_order"]), "missing_capability_order": missing, "unsupported_extension_order": unsupported, "omission_order": sorted(omission), "uncertainty_order": sorted(uncertainty), "semantic_loss_order": sorted(loss), "stage_order": sorted(set(request.get("stage_order", []))), "completed_stage_order": completed, "pending_stage_order": pending, "replay_identity": request["replay_identity"], "schema_digest": request["schema_digest"], "artifact_digest_order": sorted(request["artifact_digest_order"]), "raw_data_local": True, "aggregate_only": True, "boundary": PRECLINICAL_BOUNDARY}
    receipt_digest = _hash(payload)
    payload["receipt_digest"] = receipt_digest
    payload["artifact"] = {"artifact_id": f"worldgen-interoperability-extensibility:{request['request_id']}", "content_type": CONTENT_TYPE, "content_hash": receipt_digest, "semantic_loss": sorted(loss), "boundary": PRECLINICAL_BOUNDARY}
    payload["effect_receipts"] = [f"exchange:capability-manifest:{request['request_id']}"] if disposition in {"accepted", "migrated"} else ["approval-required:interoperability"] if disposition == "approval_required" else ["block:unsafe-release"]
    validate(payload)
    return payload


ExtensibilityRequest4 = dict[str, Any]
ExtensibilityReceipt7 = dict[str, Any]
InteroperabilityExtensibilityError = ResearchContractError
__all__ = ["TARGET_VERSION", "COMPATIBLE_VERSION", "CONTENT_TYPE", "ExtensibilityRequest4", "ExtensibilityReceipt7", "InteroperabilityExtensibilityError", "manifest", "negotiate", "validate"]
