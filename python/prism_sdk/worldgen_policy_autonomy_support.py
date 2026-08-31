"""Deterministic Python parity for Worldgen P19 policy/autonomy admission."""
from __future__ import annotations

import hashlib
import json
import re
from dataclasses import dataclass
from typing import Any, Mapping

from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError

FEATURE_ID = "AFA-worldgen-P19-F01"
CONTRACT_VERSION = "worldgen-local-policy-autonomy/1.0"
INPUT_SCHEMA = "ActionAndAuthority3@1"
OUTPUT_SCHEMA = "PolicyReceipt1@1"
CONTENT_TYPE = "application/vnd.aurora.worldgen.policy-autonomy-receipt-1+json"


def _hash(value: Any) -> str:
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()).hexdigest()


def _digest(value: Any) -> bool:
    return isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) is not None


def _ordered(values: list[str]) -> bool:
    return values == sorted(set(values))


def _validate(request: Mapping[str, Any]) -> None:
    if (
        request.get("schema_version") != INPUT_SCHEMA
        or any(not isinstance(request.get(key), str) or not request[key].strip() for key in ("request_id", "consumer", "purpose", "required_scope", "policy_epoch"))
        or not _digest(request.get("replay_identity"))
        or request.get("boundary") != PRECLINICAL_BOUNDARY
        or request.get("federated_summary_only") is not True
        or not request.get("actions")
    ):
        raise ResearchContractError("policy identity, replay, epoch, boundary, locality, or action closure is invalid")
    ids: set[str] = set()
    for action in request["actions"]:
        if (
            not isinstance(action.get("action_id"), str)
            or not action["action_id"].strip()
            or action["action_id"] in ids
            or not isinstance(action.get("actor"), str)
            or not action["actor"].strip()
            or not isinstance(action.get("autonomy_tier"), str)
            or not action["autonomy_tier"].strip()
            or not isinstance(action.get("scope"), str)
            or not action["scope"].strip()
            or not _ordered(action.get("requested_effect_order", []))
            or not _digest(action.get("artifact_digest"))
            or not _digest(action.get("provenance_digest"))
            or action.get("replay_identity") != request["replay_identity"]
        ):
            raise ResearchContractError("action identity, effect ordering, digest, or replay is invalid")
        ids.add(action["action_id"])


@dataclass(frozen=True)
class SignedPolicyAutonomyEnvelope1:
    value: dict[str, Any]

    def to_dict(self) -> dict[str, Any]:
        return dict(self.value)

    def validate(self) -> None:
        validate_policy_receipt(self.value, allow_feature_variants=True)


def manifest(*, feature_id: str, contract_version: str, scale: str) -> dict[str, Any]:
    return {
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "capability_id": feature_id,
        "version": contract_version,
        "owner_crate": "worldgen",
        "consumers": ["consortium administrator", "policy steward", "workflow operator"],
        "behavior": f"classify bounded research actions into allow, approval-required, local-only, deny, or unresolved policy receipts at {scale}",
        "value": "prevents missing evidence or authority from becoming permission and keeps autonomy tier decisions auditable",
        "input_schema": INPUT_SCHEMA,
        "output_schema": OUTPUT_SCHEMA,
        "effects": ["emit:policy-receipt", "block:unsafe-policy"],
        "permissions": ["read:local-research-artifacts"],
        "determinism": "byte_stable",
        "autonomy_tier": "A1",
        "boundary": PRECLINICAL_BOUNDARY,
    }


def validate_policy_receipt(output: Mapping[str, Any], *, allow_feature_variants: bool = False) -> None:
    artifact = output.get("artifact", {})
    if (
        output.get("schema_version") != RESEARCH_CONTRACT_SCHEMA_VERSION
        or (not allow_feature_variants and output.get("contract_version") != CONTRACT_VERSION)
        or (not allow_feature_variants and output.get("feature_id") != FEATURE_ID)
        or output.get("boundary") != PRECLINICAL_BOUNDARY
        or artifact.get("boundary") != PRECLINICAL_BOUNDARY
        or artifact.get("content_type") != CONTENT_TYPE
        or output.get("raw_data_local") is not True
        or output.get("disposition") not in {"qualified", "partial", "blocked"}
        or not output.get("action_order")
    ):
        raise ResearchContractError("policy receipt identity, locality, disposition, or actions are incomplete")
    fields = ("action_order", "allowed_order", "approval_required_order", "local_only_order", "denied_order", "unresolved_order", "omission_order", "uncertainty_order", "negative_evidence_order")
    if any(not _ordered(output.get(key, [])) for key in fields):
        raise ResearchContractError("policy receipt ordering is not canonical")
    ids = set(output["action_order"])
    parts = sum((output.get(key, []) for key in ("allowed_order", "approval_required_order", "local_only_order", "denied_order", "unresolved_order")), [])
    if len(ids) != len(output["action_order"]) or len(parts) != len(ids) or set(parts) != ids:
        raise ResearchContractError("policy action states do not partition")
    if any(not _digest(output.get(key)) for key in ("replay_identity", "receipt_digest")) or artifact.get("content_hash") != output.get("receipt_digest") or any(not _digest(value) for value in artifact.get("provenance_digests", [])):
        raise ResearchContractError("policy receipt digest is inconsistent")


def qualify(request: Mapping[str, Any], *, feature_id: str = FEATURE_ID, contract_version: str = CONTRACT_VERSION) -> SignedPolicyAutonomyEnvelope1:
    _validate(request)
    rows = sorted((dict(action) for action in request["actions"]), key=lambda action: action["action_id"])
    order = [action["action_id"] for action in rows]
    allowed: set[str] = set(); approval: set[str] = set(); local_only: set[str] = set(); denied: set[str] = set(); unresolved: set[str] = set(); omissions: set[str] = set(); uncertainty: set[str] = set(); negative = {action["action_id"] for action in rows if action.get("negative_result")}
    for action in rows:
        action_id = action["action_id"]
        if action.get("scope") != request["required_scope"] or not action.get("policy_allowed") or not action.get("authority_present"):
            denied.add(action_id); omissions.add(f"{action_id}:scope-policy-or-authority")
        elif action.get("approval_required"):
            approval.add(action_id)
        elif action.get("local_only"):
            local_only.add(action_id)
        elif action.get("evidence_state") in {"unknown", "speculative", "contradicted"}:
            unresolved.add(action_id); uncertainty.add(f"{action_id}:evidence-state")
        else:
            allowed.add(action_id)
    if not request.get("protected_closure"):
        omissions.add("request:protected-closure-incomplete")
    if not request.get("raw_data_local"):
        omissions.add("request:raw-data-not-local")
    global_block = not all(request.get(key) is True for key in ("protected_closure", "raw_data_local", "federated_summary_only"))
    disposition = "blocked" if global_block or denied else "partial" if unresolved or approval or local_only or not allowed else "qualified"
    if global_block:
        denied.update(order); allowed.clear(); approval.clear(); local_only.clear(); unresolved.clear()
    if disposition != "qualified":
        omissions.add("request:policy-closure-not-ready")
    payload = {"action_order": order, "allowed_order": sorted(allowed), "approval_required_order": sorted(approval), "local_only_order": sorted(local_only), "denied_order": sorted(denied), "unresolved_order": sorted(unresolved), "omission_order": sorted(omissions), "uncertainty_order": sorted(uncertainty), "negative_evidence_order": sorted(negative), "replay_identity": request["replay_identity"]}
    receipt_digest = _hash(payload)
    output = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": contract_version, "feature_id": feature_id, "request_id": request["request_id"], "consumer": request["consumer"], "purpose": request["purpose"], "required_scope": request["required_scope"], "policy_epoch": request["policy_epoch"], "disposition": disposition, **payload, "receipt_digest": receipt_digest, "artifact": {"artifact_id": f"worldgen-policy-receipt:{request['request_id']}", "content_type": CONTENT_TYPE, "content_hash": receipt_digest, "semantic_loss": [] if disposition == "qualified" else ["action-not-executed"], "provenance_digests": sorted({action["provenance_digest"] for action in rows}), "boundary": PRECLINICAL_BOUNDARY}, "raw_data_local": True, "boundary": PRECLINICAL_BOUNDARY}
    validate_policy_receipt(output, allow_feature_variants=True)
    return SignedPolicyAutonomyEnvelope1(output)


ArtifactAndDerivation = dict[str, Any]
ArtifactCandidate = dict[str, Any]
PolicyAutonomyEvidenceState = str
PolicyAutonomyError = ResearchContractError

__all__ = ["FEATURE_ID", "CONTRACT_VERSION", "INPUT_SCHEMA", "OUTPUT_SCHEMA", "CONTENT_TYPE", "SignedPolicyAutonomyEnvelope1", "ArtifactAndDerivation", "ArtifactCandidate", "PolicyAutonomyEvidenceState", "PolicyAutonomyError", "manifest", "qualify", "validate_policy_receipt"]
