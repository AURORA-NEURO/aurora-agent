"""Python parity for Megafactory P32 factory-lineage integrity cards."""
from __future__ import annotations

import hashlib
import json
import re
from typing import Any, Mapping

from .research_contracts import RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError

BOUNDARY = "preclinical-research-only; no human-subject or clinical-source data; no diagnosis, treatment, triage, enrollment, or clinical decisions"
CONTENT_TYPE = "application/vnd.aurora.megafactory.factory-lineage-integrity-card-1+json"
FactoryStage4 = dict[str, Any]
FactoryLineageRequest4 = dict[str, Any]
FactoryLineageCard7 = dict[str, Any]
FactoryLineageArtifact4 = dict[str, Any]
FactoryLineageIntegrityError = ResearchContractError


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
        "owner_crate": "megafactory",
        "consumers": ["factory planner", "workflow compiler", "federation steward", "replay auditor"],
        "behavior": f"qualify deterministic factory lineage at {scale} ({mode})",
        "value": "prevents orphaned, cyclic, or unauditable factory stages from entering research execution",
        "input_schema": "FactoryLineageRequest4@1",
        "output_schema": "FactoryLineageCard7@1",
        "effects": ["emit:lineage-admission-card", "retain:rejected-and-unresolved-stages", "block:unsafe-factory-plan"],
        "permissions": ["read:local-factory-manifests", "exchange:aggregate-lineage"],
        "determinism": "byte_stable",
        "autonomy_tier": "A1",
        "boundary": BOUNDARY,
    }


def _has_cycle(stage_map: Mapping[str, str]) -> bool:
    visiting: set[str] = set()
    finished: set[str] = set()

    def visit(node: str) -> bool:
        if node in finished:
            return False
        if node in visiting:
            return True
        visiting.add(node)
        parent = stage_map.get(node)
        if parent and parent != "root" and visit(parent):
            return True
        visiting.remove(node)
        finished.add(node)
        return False

    return any(visit(node) for node in stage_map)


def validate(card: Mapping[str, Any], *, feature_id: str | None = None) -> None:
    artifact = card.get("artifact", {})
    if (
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
        or card.get("admitted_stage_count", 0) > card.get("total_stage_count", 0)
    ):
        raise ResearchContractError("factory identity, locality, artifact, digest, boundary, or count is incomplete")
    for key in ("stage_order", "admitted_order", "rejected_order", "unknown_order", "omitted_order", "lineage_order", "consumer_order", "contract_order", "effect_order", "effect_receipts"):
        if not _ordered(card.get(key, [])):
            raise ResearchContractError("factory vectors are not canonical")
    ids = set(card["stage_order"])
    states = set(card["admitted_order"]) | set(card["rejected_order"]) | set(card["unknown_order"]) | set(card["omitted_order"])
    if len(card["stage_order"]) != len(ids) or states != ids:
        raise ResearchContractError("factory stage states do not partition stages")
    if card["admitted_stage_count"] != len(card["admitted_order"]):
        raise ResearchContractError("admitted stage count does not match admitted order")


def qualify(request: Mapping[str, Any], *, feature_id: str, contract_version: str, scale: str, mode: str) -> dict[str, Any]:
    if (
        request.get("schema_version") != RESEARCH_CONTRACT_SCHEMA_VERSION
        or not request.get("request_id", "").strip()
        or not request.get("purpose", "").strip()
        or not request.get("stages")
        or request.get("stage_budget", 0) <= 0
        or not _digest(request.get("replay_identity"))
        or request.get("boundary") != BOUNDARY
        or request.get("raw_data_local") is not True
        or request.get("aggregate_only") is not True
        or not _ordered(request.get("required_stage_order", []))
        or not _ordered(request.get("adversarial_events", []))
    ):
        raise ResearchContractError("factory identity, ordering, replay, locality, boundary, or budget is invalid")
    rows = sorted(request["stages"], key=lambda item: item.get("stage_id", ""))
    seen: set[str] = set(); stage_map: dict[str, str] = {}; admitted: set[str] = set(); rejected: set[str] = set(); unknown: set[str] = set(); omitted: set[str] = set(); lineage: set[str] = set(); consumers: set[str] = set(); contracts: set[str] = set(); effects: set[str] = set(); evidence: set[str] = set(); semantic_loss: list[str] = []
    for stage in rows:
        sid = stage.get("stage_id", "")
        if not sid.strip() or not stage.get("parent_stage", "").strip() or not stage.get("owner_crate", "").strip() or not stage.get("consumer", "").strip() or not stage.get("behavior", "").strip() or not stage.get("input_schema", "").strip() or not stage.get("output_schema", "").strip() or not stage.get("effect", "").strip() or not _digest(stage.get("artifact_digest")) or not stage.get("evidence_state", "").strip() or stage.get("local") is not True or stage.get("aggregate_only") is not True:
            raise ResearchContractError("stage identity, lineage, consumer, typed ports, effect, evidence, or locality is incomplete")
        if sid in seen:
            raise ResearchContractError(f"duplicate factory stage {sid}")
        seen.add(sid); stage_map[sid] = stage["parent_stage"]; lineage.add(f"{sid}<-{stage['parent_stage']}"); consumers.add(stage["consumer"]); contracts.add(f"{stage['input_schema']}→{stage['output_schema']}"); effects.add(stage["effect"]); evidence.add(stage["artifact_digest"])
        state = stage["evidence_state"]
        if state in ("supported", "proven") and stage.get("required") is True and stage.get("deterministic") is True and stage.get("idempotent") is True:
            admitted.add(sid)
        elif state in ("contradicted", "rejected"):
            rejected.add(sid); semantic_loss.append(sid)
        elif state in ("unknown", "speculative", "unmeasured"):
            unknown.add(sid); semantic_loss.append(sid)
        else:
            omitted.add(sid); semantic_loss.append(sid)
    if any(parent != "root" and parent not in seen for parent in stage_map.values()) or _has_cycle(stage_map):
        raise ResearchContractError("factory lineage has an orphan parent or cycle")
    if set(request["required_stage_order"]) != seen:
        raise ResearchContractError("required stage order is not the canonical stage set")
    global_block = request.get("policy_allowed") is not True or request.get("protected_closure") is not True or request.get("signed_manifest") is not True or request.get("raw_data_local") is not True or request.get("aggregate_only") is not True or bool(request.get("adversarial_events")) or len(rows) > request["stage_budget"]
    if global_block:
        omitted.update(seen); admitted.clear(); rejected.clear(); unknown.clear()
    disposition = "blocked" if global_block else "unknown" if unknown else "partial" if rejected or omitted else "qualified"
    body = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": contract_version, "feature_id": feature_id, "request_id": request["request_id"], "purpose": request["purpose"], "disposition": disposition, "stage_order": sorted(seen)}
    closure_digest = _hash(body)
    admitted_order = sorted(admitted); rejected_order = sorted(rejected); unknown_order = sorted(unknown); omitted_order = sorted(omitted)
    card = {**body, "admitted_order": admitted_order, "rejected_order": rejected_order, "unknown_order": unknown_order, "omitted_order": omitted_order, "lineage_order": sorted(lineage), "consumer_order": sorted(consumers), "contract_order": sorted(contracts), "effect_order": sorted(effects), "replay_identity": request["replay_identity"], "closure_digest": closure_digest, "admitted_stage_count": len(admitted_order), "total_stage_count": len(rows), "raw_data_local": True, "aggregate_only": True, "boundary": BOUNDARY, "effect_receipts": [f"prepare:factory-lineage:{request['request_id']}"] if disposition == "qualified" else ["block:unsafe-factory-plan"], "artifact": {"artifact_id": f"megafactory-lineage:{request['request_id']}", "content_type": CONTENT_TYPE, "content_hash": closure_digest, "semantic_loss": sorted(semantic_loss), "evidence_digests": sorted(evidence), "boundary": BOUNDARY}}
    validate(card, feature_id=feature_id)
    return card


__all__ = ["BOUNDARY", "CONTENT_TYPE", "FactoryStage4", "FactoryLineageRequest4", "FactoryLineageCard7", "FactoryLineageArtifact4", "FactoryLineageIntegrityError", "manifest", "qualify", "validate"]
