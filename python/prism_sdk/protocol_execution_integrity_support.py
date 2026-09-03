"""Python parity for Choreography P32 protocol-execution integrity cards."""
from __future__ import annotations

import hashlib
import json
import re
from typing import Any, Mapping

from .research_contracts import RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError

BOUNDARY = "preclinical-research-only; no human-subject or clinical-source data; no diagnosis, treatment, triage, enrollment, or clinical decisions"
CONTENT_TYPE = "application/vnd.aurora.choreography.protocol-execution-integrity-card-1+json"
ProtocolStep4 = dict[str, Any]
ProtocolExecutionRequest4 = dict[str, Any]
ProtocolExecutionCard7 = dict[str, Any]
ProtocolExecutionArtifact4 = dict[str, Any]
ProtocolExecutionIntegrityError = ResearchContractError


def _hash(value: Any) -> str:
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()).hexdigest()


def _digest(value: Any) -> bool:
    return isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) is not None


def _ordered(value: list[str]) -> bool:
    return isinstance(value, list) and value == sorted(set(value))


def manifest(*, feature_id: str, contract_version: str, scale: str, mode: str) -> dict[str, Any]:
    return {
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "capability_id": feature_id,
        "version": contract_version,
        "owner_crate": "choreography",
        "consumers": ["protocol steward", "execution ledger", "research workbench", "downstream crate"],
        "behavior": f"execute projected protocol steps at {scale} ({mode})",
        "value": "turns a multiparty protocol run into an omission-aware, replayable execution artifact while retaining refusal and uncertainty states",
        "input_schema": "ProtocolExecutionRequest4@1",
        "output_schema": "ProtocolExecutionCard7@1",
        "effects": ["emit:protocol-card", "retain:blocked-and-omitted-steps", "block:unsafe-protocol-effect"],
        "permissions": ["read:local-protocol-traces"],
        "determinism": "byte_stable",
        "autonomy_tier": "A1",
        "boundary": BOUNDARY,
    }


def validate(card: Mapping[str, Any], *, feature_id: str | None = None) -> None:
    artifact = card.get("artifact", {})
    bad = (
        card.get("schema_version") != RESEARCH_CONTRACT_SCHEMA_VERSION
        or (feature_id is not None and card.get("feature_id") != feature_id)
        or not card.get("request_id")
        or not card.get("purpose")
        or card.get("boundary") != BOUNDARY
        or artifact.get("boundary") != BOUNDARY
        or card.get("raw_data_local") is not True
        or card.get("aggregate_only") is not True
        or not _digest(card.get("replay_identity"))
        or not _digest(card.get("closure_digest"))
        or artifact.get("content_type") != CONTENT_TYPE
        or artifact.get("content_hash") != card.get("closure_digest")
        or card.get("completed_step_count", 0) > card.get("total_step_count", 0)
    )
    if bad:
        raise ResearchContractError("protocol identity, locality, artifact, digest, boundary, or count is incomplete")
    for key in ("step_order", "completed_order", "blocked_order", "unknown_order", "omitted_order", "role_order", "operation_order", "state_order", "effect_receipts"):
        if not _ordered(card.get(key, [])):
            raise ResearchContractError("protocol vectors are not canonical")
    ids = set(card["step_order"])
    states = set(card["completed_order"]) | set(card["blocked_order"]) | set(card["unknown_order"]) | set(card["omitted_order"])
    if len(card["step_order"]) != len(ids) or states != ids:
        raise ResearchContractError("protocol states do not partition steps")
    if card["completed_step_count"] != len(card["completed_order"]):
        raise ResearchContractError("completed step count does not match completed order")


def execute(request: Mapping[str, Any], *, feature_id: str, contract_version: str, scale: str, mode: str) -> dict[str, Any]:
    if (
        request.get("schema_version") != RESEARCH_CONTRACT_SCHEMA_VERSION
        or not request.get("request_id", "").strip()
        or not request.get("purpose", "").strip()
        or not request.get("steps")
        or not request.get("required_step_order")
        or not _ordered(request["required_step_order"])
        or not _digest(request.get("replay_identity"))
        or request.get("boundary") != BOUNDARY
        or request.get("raw_data_local") is not True
        or request.get("aggregate_only") is not True
        or not _ordered(request.get("adversarial_events", []))
        or request.get("step_budget", 0) <= 0
    ):
        raise ResearchContractError("protocol identity, ordering, replay digest, locality, boundary, or budget is invalid")
    rows = sorted(request["steps"], key=lambda step: step.get("step_id", ""))
    seen: set[str] = set(); completed: set[str] = set(); blocked: set[str] = set(); unknown: set[str] = set(); omitted: set[str] = set()
    roles: set[str] = set(); operations: set[str] = set(); states: set[str] = set(); evidence: set[str] = set(); semantic_loss: list[str] = []
    global_block = request.get("policy_allowed") is not True or request.get("protected_closure") is not True or request.get("signed_manifest") is not True or request.get("raw_data_local") is not True or request.get("aggregate_only") is not True or bool(request.get("adversarial_events")) or len(rows) > request["step_budget"]
    for step in rows:
        step_id = step.get("step_id", ""); output_digest = step.get("output_digest")
        if not step_id.strip() or not step.get("role", "").strip() or not step.get("operation", "").strip() or not _digest(step.get("input_digest")) or (output_digest is not None and not _digest(output_digest)):
            raise ResearchContractError("step identity, role, operation, or digest is incomplete")
        if step_id in seen:
            raise ResearchContractError(f"duplicate step {step_id}")
        seen.add(step_id); roles.add(step["role"]); operations.add(step["operation"]); states.add(step.get("state", "")); evidence.add(step["input_digest"])
        if output_digest:
            evidence.add(output_digest)
        if step.get("local") is not True or step.get("aggregate_only") is not True or step.get("deterministic") is not True:
            global_block = True
        if step.get("evidence_state") in ("supported", "proven") and step.get("required") is True and output_digest:
            completed.add(step_id)
        elif step.get("evidence_state") in ("contradicted", "rejected"):
            blocked.add(step_id); semantic_loss.append(step_id)
        elif step.get("evidence_state") in ("unknown", "speculative", "unmeasured"):
            unknown.add(step_id); semantic_loss.append(step_id)
        else:
            omitted.add(step_id); semantic_loss.append(step_id)
    if set(request["required_step_order"]) != seen:
        raise ResearchContractError("required step order is not the canonical step set")
    if global_block:
        omitted.update(seen); completed.clear(); blocked.clear(); unknown.clear()
    disposition = "blocked" if global_block else "unknown" if unknown else "partial" if blocked or omitted else "completed"
    body = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": contract_version, "feature_id": feature_id, "request_id": request["request_id"], "purpose": request["purpose"], "disposition": disposition, "step_order": sorted(seen)}
    closure_digest = _hash(body); completed_order = sorted(completed); blocked_order = sorted(blocked); unknown_order = sorted(unknown); omitted_order = sorted(omitted)
    card = {**body, "completed_order": completed_order, "blocked_order": blocked_order, "unknown_order": unknown_order, "omitted_order": omitted_order, "role_order": sorted(roles), "operation_order": sorted(operations), "state_order": sorted(states), "replay_identity": request["replay_identity"], "closure_digest": closure_digest, "completed_step_count": len(completed_order), "total_step_count": len(rows), "raw_data_local": True, "aggregate_only": True, "boundary": BOUNDARY, "effect_receipts": [f"emit:protocol-execution:{request['request_id']}"] if disposition == "completed" else ["block:unsafe-protocol-effect"], "artifact": {"artifact_id": f"choreography-protocol-execution:{request['request_id']}", "content_type": CONTENT_TYPE, "content_hash": closure_digest, "semantic_loss": sorted(semantic_loss), "evidence_digests": sorted(evidence), "boundary": BOUNDARY}}
    validate(card, feature_id=feature_id)
    return card


__all__ = ["BOUNDARY", "CONTENT_TYPE", "ProtocolStep4", "ProtocolExecutionRequest4", "ProtocolExecutionCard7", "ProtocolExecutionArtifact4", "ProtocolExecutionIntegrityError", "manifest", "execute", "validate"]
