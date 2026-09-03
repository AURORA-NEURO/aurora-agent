"""Python parity for Hub P32 submission-release integrity cards."""
from __future__ import annotations

import hashlib
import json
import re
from typing import Any, Mapping

from .research_contracts import RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError

BOUNDARY = "preclinical-research-only; no human-subject or clinical-source data; no diagnosis, treatment, triage, enrollment, or clinical decisions"
CONTENT_TYPE = "application/vnd.aurora.hub.submission-release-integrity-card-1+json"
SubmissionCandidate4 = dict[str, Any]
SubmissionReleaseRequest4 = dict[str, Any]
SubmissionReleaseCard7 = dict[str, Any]
SubmissionArtifact4 = dict[str, Any]
SubmissionReleaseIntegrityError = ResearchContractError


def _hash(value: Any) -> str:
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()).hexdigest()


def _digest(value: Any) -> bool:
    return isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) is not None


def _ordered(value: list[str]) -> bool:
    return isinstance(value, list) and value == sorted(set(value))


def manifest(*, feature_id: str, contract_version: str, scale: str, mode: str) -> dict[str, Any]:
    return {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "capability_id": feature_id, "version": contract_version, "owner_crate": "hub", "consumers": ["hub moderator", "research workbench", "downstream registry"], "behavior": f"prepare signed submission-release cards at {scale} ({mode})", "value": "makes public result admission reproducible without silently promoting unverifiable, contradictory, or negative evidence", "input_schema": "SubmissionReleaseRequest4@1", "output_schema": "SubmissionReleaseCard7@1", "effects": ["emit:submission-release-card", "retain:rejected-and-unresolved-evidence", "block:unsafe-publication"], "permissions": ["read:local-submission-manifests"], "determinism": "byte_stable", "autonomy_tier": "A1", "boundary": BOUNDARY}


def validate(card: Mapping[str, Any], *, feature_id: str | None = None) -> None:
    artifact = card.get("artifact", {})
    if card.get("schema_version") != RESEARCH_CONTRACT_SCHEMA_VERSION or (feature_id is not None and card.get("feature_id") != feature_id) or not card.get("request_id") or not card.get("purpose") or card.get("boundary") != BOUNDARY or artifact.get("boundary") != BOUNDARY or card.get("raw_data_local") is not True or card.get("aggregate_only") is not True or not _digest(card.get("replay_identity")) or not _digest(card.get("closure_digest")) or artifact.get("content_type") != CONTENT_TYPE or artifact.get("content_hash") != card.get("closure_digest") or card.get("released_candidate_count", 0) > card.get("total_candidate_count", 0):
        raise ResearchContractError("submission identity, locality, artifact, digest, boundary, or count is incomplete")
    for key in ("candidate_order", "released_order", "rejected_order", "unknown_order", "omitted_order", "scope_order", "provenance_order", "licence_order", "verification_order", "effect_receipts"):
        if not _ordered(card.get(key, [])):
            raise ResearchContractError("submission vectors are not canonical")
    ids = set(card["candidate_order"]); states = set(card["released_order"]) | set(card["rejected_order"]) | set(card["unknown_order"]) | set(card["omitted_order"])
    if len(card["candidate_order"]) != len(ids) or states != ids:
        raise ResearchContractError("submission states do not partition candidates")
    if card["released_candidate_count"] != len(card["released_order"]):
        raise ResearchContractError("released candidate count does not match released order")


def release(request: Mapping[str, Any], *, feature_id: str, contract_version: str, scale: str, mode: str) -> dict[str, Any]:
    if request.get("schema_version") != RESEARCH_CONTRACT_SCHEMA_VERSION or not request.get("request_id", "").strip() or not request.get("purpose", "").strip() or not request.get("candidates") or not request.get("required_candidate_order") or not _ordered(request["required_candidate_order"]) or not _digest(request.get("replay_identity")) or request.get("boundary") != BOUNDARY or request.get("raw_data_local") is not True or request.get("aggregate_only") is not True or not _ordered(request.get("adversarial_events", [])) or request.get("candidate_budget", 0) <= 0:
        raise ResearchContractError("submission identity, ordering, replay, locality, boundary, or budget is invalid")
    rows = sorted(request["candidates"], key=lambda item: item.get("candidate_id", "")); seen: set[str] = set(); released: set[str] = set(); rejected: set[str] = set(); unknown: set[str] = set(); omitted: set[str] = set(); scopes: set[str] = set(); provenance: set[str] = set(); licences: set[str] = set(); verification: set[str] = set(); evidence: set[str] = set(); semantic_loss: list[str] = []
    global_block = request.get("policy_allowed") is not True or request.get("protected_closure") is not True or request.get("signed_approval") is not True or request.get("raw_data_local") is not True or request.get("aggregate_only") is not True or bool(request.get("adversarial_events")) or len(rows) > request["candidate_budget"]
    for candidate in rows:
        cid = candidate.get("candidate_id", "")
        if not cid.strip() or not _digest(candidate.get("artifact_digest")) or not _digest(candidate.get("provenance_digest")) or not candidate.get("scope", "").strip() or not candidate.get("licence", "").strip() or not candidate.get("verification_state", "").strip():
            raise ResearchContractError("candidate identity, scope, licence, provenance, or verification is incomplete")
        if cid in seen:
            raise ResearchContractError(f"duplicate candidate {cid}")
        seen.add(cid); scopes.add(candidate["scope"]); provenance.add(candidate["provenance_digest"]); licences.add(candidate["licence"]); verification.add(candidate["verification_state"]); evidence.update((candidate["artifact_digest"], candidate["provenance_digest"]))
        if candidate.get("negative_result") is True:
            semantic_loss.append(f"{cid}:negative-result")
        if candidate.get("local") is not True or candidate.get("aggregate_only") is not True:
            global_block = True
        state = candidate.get("evidence_state")
        if state in ("supported", "proven") and candidate.get("reproducible") is True and candidate.get("required") is True and candidate.get("negative_result") is not True:
            released.add(cid)
        elif state in ("contradicted", "rejected"):
            rejected.add(cid); semantic_loss.append(cid)
        elif state in ("unknown", "speculative", "unmeasured"):
            unknown.add(cid); semantic_loss.append(cid)
        else:
            omitted.add(cid); semantic_loss.append(cid)
    if set(request["required_candidate_order"]) != seen:
        raise ResearchContractError("required candidate order is not the canonical candidate set")
    if global_block:
        omitted.update(seen); released.clear(); rejected.clear(); unknown.clear()
    disposition = "blocked" if global_block else "unknown" if unknown else "partial" if rejected or omitted else "released"
    body = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": contract_version, "feature_id": feature_id, "request_id": request["request_id"], "purpose": request["purpose"], "disposition": disposition, "candidate_order": sorted(seen)}; closure_digest = _hash(body)
    released_order = sorted(released); rejected_order = sorted(rejected); unknown_order = sorted(unknown); omitted_order = sorted(omitted)
    card = {**body, "released_order": released_order, "rejected_order": rejected_order, "unknown_order": unknown_order, "omitted_order": omitted_order, "scope_order": sorted(scopes), "provenance_order": sorted(provenance), "licence_order": sorted(licences), "verification_order": sorted(verification), "replay_identity": request["replay_identity"], "closure_digest": closure_digest, "released_candidate_count": len(released_order), "total_candidate_count": len(rows), "raw_data_local": True, "aggregate_only": True, "boundary": BOUNDARY, "effect_receipts": [f"prepare:submission-release:{request['request_id']}"] if disposition == "released" else ["block:unsafe-publication"], "artifact": {"artifact_id": f"hub-submission-release:{request['request_id']}", "content_type": CONTENT_TYPE, "content_hash": closure_digest, "semantic_loss": sorted(semantic_loss), "evidence_digests": sorted(evidence), "boundary": BOUNDARY}}
    validate(card, feature_id=feature_id)
    return card


__all__ = ["BOUNDARY", "CONTENT_TYPE", "SubmissionCandidate4", "SubmissionReleaseRequest4", "SubmissionReleaseCard7", "SubmissionArtifact4", "SubmissionReleaseIntegrityError", "manifest", "release", "validate"]
