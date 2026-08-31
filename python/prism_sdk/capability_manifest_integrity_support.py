"""Python parity for Weave P32 capability-manifest admission integrity cards."""
from __future__ import annotations

import hashlib
import json
import re
from typing import Any, Mapping

from .research_contracts import RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError

BOUNDARY = "preclinical-research-only; no human-subject or clinical-source data; no diagnosis, treatment, triage, enrollment, or clinical decisions"
CONTENT_TYPE = "application/vnd.aurora.weave.capability-manifest-integrity-card-1+json"
CapabilityCandidate4 = dict[str, Any]
CapabilityManifestRequest4 = dict[str, Any]
CapabilityManifestCard7 = dict[str, Any]
CapabilityArtifact4 = dict[str, Any]
CapabilityManifestIntegrityError = ResearchContractError


def _hash(value: Any) -> str:
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()).hexdigest()


def _digest(value: Any) -> bool:
    return isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) is not None


def _ordered(value: list[str]) -> bool:
    return isinstance(value, list) and value == sorted(set(value))


def manifest(*, feature_id: str, contract_version: str, scale: str, mode: str) -> dict[str, Any]:
    return {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "capability_id": feature_id, "version": contract_version, "owner_crate": "weave", "consumers": ["weave kernel", "policy gate", "workflow compiler"], "behavior": f"admit typed capability manifests at {scale} ({mode})", "value": "prevents untyped or unauditable capabilities from entering research workflows while preserving refusal and uncertainty evidence", "input_schema": "CapabilityManifestRequest4@1", "output_schema": "CapabilityManifestCard7@1", "effects": ["emit:capability-admission-card", "retain:rejected-and-unresolved-manifests", "block:unsafe-capability-effect"], "permissions": ["read:local-capability-manifests"], "determinism": "byte_stable", "autonomy_tier": "A1", "boundary": BOUNDARY}


def validate(card: Mapping[str, Any], *, feature_id: str | None = None) -> None:
    artifact = card.get("artifact", {})
    if card.get("schema_version") != RESEARCH_CONTRACT_SCHEMA_VERSION or (feature_id is not None and card.get("feature_id") != feature_id) or not card.get("request_id") or not card.get("purpose") or card.get("boundary") != BOUNDARY or artifact.get("boundary") != BOUNDARY or card.get("raw_data_local") is not True or card.get("aggregate_only") is not True or not _digest(card.get("replay_identity")) or not _digest(card.get("closure_digest")) or artifact.get("content_type") != CONTENT_TYPE or artifact.get("content_hash") != card.get("closure_digest") or card.get("admitted_capability_count", 0) > card.get("total_capability_count", 0):
        raise ResearchContractError("capability identity, locality, artifact, digest, boundary, or count is incomplete")
    for key in ("capability_order", "admitted_order", "rejected_order", "unknown_order", "omitted_order", "consumer_order", "owner_order", "schema_order", "effect_order", "effect_receipts"):
        if not _ordered(card.get(key, [])):
            raise ResearchContractError("capability vectors are not canonical")
    ids = set(card["capability_order"]); states = set(card["admitted_order"]) | set(card["rejected_order"]) | set(card["unknown_order"]) | set(card["omitted_order"])
    if len(card["capability_order"]) != len(ids) or states != ids:
        raise ResearchContractError("capability states do not partition manifests")
    if card["admitted_capability_count"] != len(card["admitted_order"]):
        raise ResearchContractError("admitted capability count does not match admitted order")


def admit(request: Mapping[str, Any], *, feature_id: str, contract_version: str, scale: str, mode: str) -> dict[str, Any]:
    if request.get("schema_version") != RESEARCH_CONTRACT_SCHEMA_VERSION or not request.get("request_id", "").strip() or not request.get("purpose", "").strip() or not request.get("candidates") or not request.get("required_capability_order") or not _ordered(request["required_capability_order"]) or not _digest(request.get("replay_identity")) or request.get("boundary") != BOUNDARY or request.get("raw_data_local") is not True or request.get("aggregate_only") is not True or not _ordered(request.get("adversarial_events", [])) or request.get("capability_budget", 0) <= 0:
        raise ResearchContractError("capability identity, ordering, replay, locality, boundary, or budget is invalid")
    rows = sorted(request["candidates"], key=lambda item: item.get("capability_id", "")); seen: set[str] = set(); admitted: set[str] = set(); rejected: set[str] = set(); unknown: set[str] = set(); omitted: set[str] = set(); consumers: set[str] = set(); owners: set[str] = set(); schemas: set[str] = set(); effects: set[str] = set(); evidence: set[str] = set(); semantic_loss: list[str] = []
    global_block = request.get("policy_allowed") is not True or request.get("protected_closure") is not True or request.get("signed_manifest") is not True or request.get("raw_data_local") is not True or request.get("aggregate_only") is not True or bool(request.get("adversarial_events")) or len(rows) > request["capability_budget"]
    for candidate in rows:
        cid = candidate.get("capability_id", "")
        if not cid.strip() or not candidate.get("version", "").strip() or not candidate.get("owner_crate", "").strip() or not candidate.get("consumer", "").strip() or not candidate.get("behavior", "").strip() or not candidate.get("input_schema", "").strip() or not candidate.get("output_schema", "").strip() or not candidate.get("effect", "").strip() or not candidate.get("determinism", "").strip() or not _digest(candidate.get("evidence_digest")):
            raise ResearchContractError("capability identity, consumer, typed ports, effect, determinism, or evidence is incomplete")
        if cid in seen:
            raise ResearchContractError(f"duplicate capability {cid}")
        seen.add(cid); consumers.add(candidate["consumer"]); owners.add(candidate["owner_crate"]); schemas.add(f"{candidate['input_schema']}→{candidate['output_schema']}"); effects.add(candidate["effect"]); evidence.add(candidate["evidence_digest"])
        if candidate.get("local") is not True or candidate.get("aggregate_only") is not True:
            global_block = True
        state = candidate.get("evidence_state")
        if state in ("supported", "proven") and candidate.get("required") is True and candidate.get("determinism") == "byte_stable":
            admitted.add(cid)
        elif state in ("contradicted", "rejected"):
            rejected.add(cid); semantic_loss.append(cid)
        elif state in ("unknown", "speculative", "unmeasured"):
            unknown.add(cid); semantic_loss.append(cid)
        else:
            omitted.add(cid); semantic_loss.append(cid)
    if set(request["required_capability_order"]) != seen:
        raise ResearchContractError("required capability order is not the canonical capability set")
    if global_block:
        omitted.update(seen); admitted.clear(); rejected.clear(); unknown.clear()
    disposition = "blocked" if global_block else "unknown" if unknown else "partial" if rejected or omitted else "admitted"
    body = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": contract_version, "feature_id": feature_id, "request_id": request["request_id"], "purpose": request["purpose"], "disposition": disposition, "capability_order": sorted(seen)}; closure_digest = _hash(body)
    admitted_order = sorted(admitted); rejected_order = sorted(rejected); unknown_order = sorted(unknown); omitted_order = sorted(omitted)
    card = {**body, "admitted_order": admitted_order, "rejected_order": rejected_order, "unknown_order": unknown_order, "omitted_order": omitted_order, "consumer_order": sorted(consumers), "owner_order": sorted(owners), "schema_order": sorted(schemas), "effect_order": sorted(effects), "replay_identity": request["replay_identity"], "closure_digest": closure_digest, "admitted_capability_count": len(admitted_order), "total_capability_count": len(rows), "raw_data_local": True, "aggregate_only": True, "boundary": BOUNDARY, "effect_receipts": [f"prepare:capability-admission:{request['request_id']}"] if disposition == "admitted" else ["block:unsafe-capability-effect"], "artifact": {"artifact_id": f"weave-capability-manifest:{request['request_id']}", "content_type": CONTENT_TYPE, "content_hash": closure_digest, "semantic_loss": sorted(semantic_loss), "evidence_digests": sorted(evidence), "boundary": BOUNDARY}}
    validate(card, feature_id=feature_id)
    return card


__all__ = ["BOUNDARY", "CONTENT_TYPE", "CapabilityCandidate4", "CapabilityManifestRequest4", "CapabilityManifestCard7", "CapabilityArtifact4", "CapabilityManifestIntegrityError", "manifest", "admit", "validate"]
