"""Deterministic Python parity for Tokens P32 compression-integrity cards."""
from __future__ import annotations
import hashlib, json, re
from typing import Any, Mapping
from .research_contracts import RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError

BOUNDARY = "preclinical-research-only; no human-subject or clinical-source data; no diagnosis, treatment, triage, enrollment, or clinical decisions"
CONTENT_TYPE = "application/vnd.aurora.tokens.compression-integrity-card-1+json"

def _hash(value: Any) -> str:
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()).hexdigest()
def _digest(value: Any) -> bool:
    return isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) is not None
def _ordered(value: list[str]) -> bool:
    return isinstance(value, list) and value == sorted(set(value))

def manifest(*, feature_id: str, contract_version: str, scale: str, mode: str) -> dict[str, Any]:
    return {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "capability_id": feature_id, "version": contract_version, "owner_crate": "tokens", "consumers": ["context compiler", "obligation gate", "research workbench", "release auditor"], "behavior": f"qualify bounded context compression at {scale} ({mode})", "value": "reduces context cost while proving protected obligations and uncertainty remain visible", "input_schema": "CompressionIntegrityRequest4@1", "output_schema": "CompressionIntegrityCard7@1", "effects": ["emit:compression-card", "retain:semantic-loss", "block:unsafe-compression"], "permissions": ["read:local-context"], "determinism": "byte_stable", "autonomy_tier": "A1", "boundary": BOUNDARY}

def validate(card: Mapping[str, Any], *, feature_id: str | None = None) -> None:
    artifact = card.get("artifact", {})
    if card.get("schema_version") != RESEARCH_CONTRACT_SCHEMA_VERSION or (feature_id is not None and card.get("feature_id") != feature_id) or card.get("boundary") != BOUNDARY or card.get("raw_data_local") is not True or card.get("aggregate_only") is not True or not card.get("claim_order") or not _digest(card.get("replay_identity")) or not _digest(card.get("closure_digest")) or artifact.get("content_type") != CONTENT_TYPE or artifact.get("content_hash") != card.get("closure_digest") or artifact.get("boundary") != BOUNDARY:
        raise ResearchContractError("compression identity, locality, digest, artifact, or boundary is incomplete")
    for key in ("claim_order", "qualified_order", "over_budget_order", "unknown_order", "omitted_order", "preserved_obligation_order", "dropped_obligation_order", "token_savings_order", "negative_evidence_order", "effect_receipts"):
        if not _ordered(card.get(key, [])):
            raise ResearchContractError("compression vectors are not canonical")
    ids = set(card["claim_order"])
    states = set(card["qualified_order"]) | set(card["over_budget_order"]) | set(card["unknown_order"]) | set(card["omitted_order"])
    if len(card["claim_order"]) != len(ids) or states != ids:
        raise ResearchContractError("claim states do not partition")

def qualify(request: Mapping[str, Any], *, feature_id: str, contract_version: str, scale: str, mode: str) -> dict[str, Any]:
    if not isinstance(request.get("request_id"), str) or not request["request_id"].strip() or not isinstance(request.get("purpose"), str) or not request["purpose"].strip() or not request.get("claims") or not request.get("required_claim_order") or not request.get("required_obligation_order") or not isinstance(request.get("max_tokens"), int) or request["max_tokens"] <= 0 or not _digest(request.get("replay_identity")) or request.get("boundary") != BOUNDARY or request.get("raw_data_local") is not True or request.get("aggregate_only") is not True or not _ordered(request["required_claim_order"]) or not _ordered(request["required_obligation_order"]) or not _ordered(request.get("adversarial_events", [])):
        raise ResearchContractError("compression identity, requirements, budget, digest, ordering, locality, or boundary is invalid")
    rows = sorted(request["claims"], key=lambda claim: claim.get("claim_id", "")); order: list[str] = []; qualified: set[str] = set(); over: set[str] = set(); unknown: set[str] = set(); omitted: set[str] = set(); preserved: set[str] = set(); dropped: set[str] = set(); savings: set[str] = set(); negative: set[str] = set(); sources: set[str] = set(); seen: set[str] = set()
    for claim in rows:
        cid = claim.get("claim_id", "")
        if cid in seen or not isinstance(cid, str) or not cid.strip() or not _digest(claim.get("source_digest")) or not isinstance(claim.get("statement"), str) or not claim["statement"].strip() or not isinstance(claim.get("token_count"), int) or not isinstance(claim.get("baseline_tokens"), int) or claim["baseline_tokens"] <= 0 or claim.get("local") is not True or claim.get("aggregate_only") is not True or not _ordered(claim.get("preserved_obligation_order", [])) or not _ordered(claim.get("dropped_obligation_order", [])):
            raise ResearchContractError("claim identity, source, baseline, obligation ordering, or locality is invalid")
        seen.add(cid); order.append(cid); sources.add(claim["source_digest"]); preserved.update(claim.get("preserved_obligation_order", [])); dropped.update(claim.get("dropped_obligation_order", [])); savings.add(f"{cid}:{max(0, claim['baseline_tokens'] - claim['token_count'])}")
        if claim.get("negative_result") is True: negative.add(f"{cid}:negative-result")
        if claim["token_count"] > request["max_tokens"] or claim["token_count"] > claim["baseline_tokens"]: over.add(cid)
        elif claim.get("policy_epoch", 0) == 0 or claim.get("evidence_state") == "unknown": unknown.add(cid)
        elif claim.get("evidence_state") == "contradicted" or any(item in request["required_obligation_order"] for item in claim.get("dropped_obligation_order", [])) or not all(item in claim.get("preserved_obligation_order", []) for item in request["required_obligation_order"]): omitted.add(cid)
        elif cid not in request["required_claim_order"] or claim["source_digest"] == request["replay_identity"]: omitted.add(cid)
        else: qualified.add(cid)
    global_block = not all(request.get(key) is True for key in ("policy_allowed", "protected_closure", "signed_approval", "raw_data_local", "aggregate_only")) or bool(request.get("adversarial_events")) or request.get("action_count", 0) > request.get("action_budget", 0)
    if global_block: omitted.update(order); qualified.clear(); over.clear(); unknown.clear()
    missing = not set(request["required_claim_order"]) <= seen or not set(request["required_obligation_order"]) <= preserved
    disposition = "blocked" if global_block else "unknown" if missing else "partial" if over or unknown or omitted else "qualified"
    payload = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": contract_version, "feature_id": feature_id, "mode": mode, "scale": scale, "request_id": request["request_id"], "purpose": request["purpose"], "disposition": disposition, "claim_order": order, "qualified_order": sorted(qualified), "over_budget_order": sorted(over), "unknown_order": sorted(unknown), "omitted_order": sorted(omitted), "preserved_obligation_order": sorted(preserved), "dropped_obligation_order": sorted(dropped), "token_savings_order": sorted(savings), "negative_evidence_order": sorted(negative), "replay_identity": request["replay_identity"], "raw_data_local": True, "aggregate_only": True, "boundary": BOUNDARY}
    digest = _hash(payload); payload["closure_digest"] = digest; payload["artifact"] = {"artifact_id": f"tokens-compression:{request['request_id']}", "content_type": CONTENT_TYPE, "content_hash": digest, "semantic_loss": sorted(payload["omitted_order"]), "source_digests": sorted(sources), "boundary": BOUNDARY}; payload["effect_receipts"] = [f"emit:compression-card:{request['request_id']}"] if disposition == "qualified" else ["block:unsafe-compression"]; validate(payload, feature_id=feature_id); return payload

CompressionClaim4 = dict[str, Any]
CompressionIntegrityRequest4 = dict[str, Any]
CompressionIntegrityCard7 = dict[str, Any]
CompressionIntegrityArtifact4 = dict[str, Any]
CompressionIntegrityError = ResearchContractError
__all__ = ["BOUNDARY", "CONTENT_TYPE", "CompressionClaim4", "CompressionIntegrityRequest4", "CompressionIntegrityCard7", "CompressionIntegrityArtifact4", "CompressionIntegrityError", "manifest", "qualify", "validate"]
