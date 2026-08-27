"""Python parity for ``AFA-epistemic-P02-F31``.

Ranks caller-provided evidence summaries and peer attestations under bounded,
local-only, fail-closed release gates.  It never retrieves raw documents or
turns unknown evidence into a conclusion.
"""
from __future__ import annotations

import hashlib
import json
import re
from dataclasses import dataclass
from typing import Any, Mapping, Sequence

from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError

FEATURE_ID = "AFA-epistemic-P02-F31"
CONTRACT_VERSION = "epistemic-prospective-high-throughput-retrieval-synthesis-federated-control-plane/1.0"
INPUT_SCHEMA = "ScopedRetrievalQuery3@1"
OUTPUT_SCHEMA = "EvidenceSynthesis8@1"
CONTENT_TYPE = "application/vnd.aurora.evidence-synthesis-8+json"
MAX_CANDIDATES = 8192


def _hash(value: Any) -> str:
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()).hexdigest()


def _digest(value: Any) -> bool:
    return isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) is not None


def _ordered(values: list[str]) -> bool:
    return tuple(values) == tuple(sorted(set(values)))


@dataclass(frozen=True)
class EvidenceSynthesis8:
    value: dict[str, Any]

    def to_dict(self) -> dict[str, Any]:
        return dict(self.value)

    def validate(self) -> None:
        v = self.value
        required = ("request_id", "corpus_id", "requester", "purpose", "semantic_profile")
        if (v.get("schema_version") != RESEARCH_CONTRACT_SCHEMA_VERSION or v.get("contract_version") != CONTRACT_VERSION or v.get("feature_id") != FEATURE_ID or v.get("boundary") != PRECLINICAL_BOUNDARY or v.get("raw_data_local") is not True or v.get("aggregate_only") is not True or not all(str(v.get(k, "")).strip() for k in required) or int(v.get("checkpoint", 0)) <= 0 or not v.get("candidate_order") or not v.get("source_order") or not v.get("peer_order") or not v.get("effect_receipts") or v.get("disposition") not in {"qualified", "unresolved", "blocked"}):
            raise ResearchContractError("retrieval identity, checkpoint, locality, candidates, sources, peers, or effects are incomplete")
        fields = ("candidate_order", "qualified_order", "unresolved_order", "blocked_order", "source_order", "qualified_source_order", "missing_source_order", "peer_order", "qualified_peer_order", "missing_peer_order", "omission_order", "uncertainty_order", "negative_evidence_order", "effect_receipts")
        if any(not _ordered(v.get(k, [])) for k in fields):
            raise ResearchContractError("retrieval synthesis ordering is not canonical")
        if set(v["candidate_order"]) != set(v["qualified_order"]) | set(v["unresolved_order"]) | set(v["blocked_order"]):
            raise ResearchContractError("candidate dispositions do not partition")
        if set(v["source_order"]) != set(v["qualified_source_order"]) | set(v["missing_source_order"]):
            raise ResearchContractError("source dispositions do not partition")
        if set(v["peer_order"]) != set(v["qualified_peer_order"]) | set(v["missing_peer_order"]):
            raise ResearchContractError("peer dispositions do not partition")
        artifact = v.get("artifact", {})
        if not all(_digest(x) for x in [v.get("replay_identity"), v.get("synthesis_digest"), artifact.get("content_hash"), *artifact.get("provenance_digests", [])]) or artifact.get("content_type") != CONTENT_TYPE or artifact.get("boundary") != PRECLINICAL_BOUNDARY or artifact.get("content_hash") != v.get("synthesis_digest") or len(v["qualified_order"]) != len(v.get("ranked_scores_milli", [])):
            raise ResearchContractError("retrieval artifact, digest, or ranked scores are invalid")
        if any(not x.startswith(("exchange:permitted-summaries:", "manage:local-capability:")) and x != "block:unsafe-release" for x in v["effect_receipts"]):
            raise ResearchContractError("retrieval effect is outside governed gate")


def retrieval_synthesis_manifest() -> dict[str, Any]:
    return {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "capability_id": FEATURE_ID, "version": CONTRACT_VERSION, "owner_crate": "epistemic", "consumers": ["downstream AURORA crate maintainer", "retrieval scientist", "federation steward"], "behavior": "ranks bounded typed retrieval candidates and peer synthesis summaries under evidence, replay, provenance, policy, quorum, and locality gates", "value": "turns high-throughput evidence retrieval into an auditable, federated, fail-closed product capability", "input_schema": INPUT_SCHEMA, "output_schema": OUTPUT_SCHEMA, "effects": ["manage:local-capability", "exchange:permitted-summaries"], "permissions": ["operate:institution-node"], "autonomy_tier": "A2", "boundary": PRECLINICAL_BOUNDARY}


def operate_retrieval_synthesis(request: Mapping[str, Any], candidates: Sequence[Mapping[str, Any]], peers: Sequence[Mapping[str, Any]]) -> EvidenceSynthesis8:
    required = ("request_id", "corpus_id", "requester", "purpose", "semantic_profile")
    if not all(str(request.get(k, "")).strip() for k in required) or not request.get("required_terms") or int(request.get("candidate_limit", 0)) <= 0 or int(request.get("candidate_limit", 0)) > MAX_CANDIDATES or int(request.get("minimum_source_quorum", 0)) <= 0 or int(request.get("minimum_peer_quorum", 0)) <= 0 or int(request.get("budget_units", 0)) <= 0 or int(request.get("checkpoint", 0)) <= 0 or request.get("boundary") != PRECLINICAL_BOUNDARY or request.get("raw_data_local") is not True or request.get("aggregate_only") is not True or not _digest(request.get("replay_identity")) or not candidates or not peers:
        raise ResearchContractError("request identity, terms, bounds, checkpoint, budget, locality, candidates, peers, replay, or boundary is invalid")
    rows = sorted((dict(x) for x in candidates), key=lambda x: (-int(x.get("relevance_milli", 0)), -int(x.get("freshness_milli", 0)), str(x.get("evidence_id", ""))))
    ids = [str(x.get("evidence_id", "")) for x in rows]
    if len(set(ids)) != len(ids) or any(not x.get("evidence_id") or not x.get("source_id") or not x.get("origin") or not x.get("title") or not all(_digest(x.get(k)) for k in ("content_digest", "provenance_digest", "replay_identity")) for x in rows):
        raise ResearchContractError("candidate identity, uniqueness, origin, title, or digest is invalid")
    ps = sorted((dict(x) for x in peers), key=lambda x: str(x.get("peer_id", "")))
    peer_ids = [str(x.get("peer_id", "")) for x in ps]
    if len(set(peer_ids)) != len(ps) or any(not x.get("peer_id") or not x.get("origin") or not _digest(x.get("synthesis_digest")) for x in ps):
        raise ResearchContractError("peer identity, uniqueness, origin, or digest is invalid")
    qualified_peers = {x["peer_id"] for x in ps if x.get("corpus_id") == request["corpus_id"] and x.get("semantic_profile") == request["semantic_profile"] and int(x.get("checkpoint", 0)) == int(request["checkpoint"]) and int(x.get("source_count", 0)) >= int(request["minimum_source_quorum"]) and x.get("signed") is True and x.get("aggregate_only") is True and x.get("raw_data_local") is True and x.get("evidence_state") in {"proven", "supported"}}
    missing_peers = set(peer_ids) - qualified_peers
    qualified: set[str] = set(); unresolved: set[str] = set(); blocked: set[str] = set(); qualified_sources: set[str] = set(); omissions: set[str] = set(); uncertainty = {f"peer:{x}:not-qualified" for x in missing_peers}; negative: set[str] = set(); scores: list[int] = []
    for x in rows:
        eid = x["evidence_id"]
        if x.get("negative_result"): negative.add(f"{eid}:negative-result")
        omissions.update(f"{eid}:{reason}" for reason in x.get("omission_reasons", []))
        reasons = []
        if x.get("semantic_profile") != request["semantic_profile"]: reasons.append("semantic-profile-mismatch")
        missing = [term for term in request["required_terms"] if term not in x.get("terms", [])]
        if missing: reasons.append("required-term-missing"); omissions.add(f"{eid}:missing-terms:{len(missing)}")
        if x.get("replay_identity") != request["replay_identity"]: reasons.append("replay-identity-mismatch")
        if x.get("signed") is not True or x.get("permitted") is not True: reasons.append("authorization-missing")
        if x.get("raw_data_local") is not True or x.get("aggregate_only") is not True: reasons.append("locality-or-aggregate-only-failed")
        if x.get("evidence_state") == "contradicted": blocked.add(eid); negative.add(f"{eid}:contradicted")
        elif x.get("evidence_state") not in {"proven", "supported"} or reasons: unresolved.add(eid); uncertainty.add(f"{eid}:unresolved")
        else: qualified.add(eid); qualified_sources.add(x["source_id"]); scores.append(int(x.get("relevance_milli", 0)) + int(x.get("freshness_milli", 0)))
    global_block = not all(request.get(k) is True for k in ("policy_allow", "protected_closure", "signed_approval", "federation_approved", "raw_data_local", "aggregate_only"))
    if request.get("policy_allow") is not True: negative.add("request:policy-denied")
    if request.get("protected_closure") is not True: uncertainty.add("request:protected-closure-incomplete")
    if request.get("signed_approval") is not True: uncertainty.add("request:signed-approval-missing")
    if request.get("federation_approved") is not True: uncertainty.add("request:federation-approval-missing")
    if len(qualified_sources) < int(request["minimum_source_quorum"]): uncertainty.add("source:minimum-quorum-unmet")
    disposition = "blocked" if global_block or blocked else "unresolved" if not qualified or len(qualified_sources) < int(request["minimum_source_quorum"]) or len(qualified_peers) < int(request["minimum_peer_quorum"]) else "qualified"
    if global_block: blocked.update(ids); qualified.clear(); unresolved.clear(); scores.clear()
    if disposition != "qualified": omissions.add("request:retrieval-gates-incomplete")
    source_order = sorted({x["source_id"] for x in rows}); qualified_source_order = sorted(qualified_sources); missing_source_order = sorted(set(source_order) - set(qualified_source_order))
    payload = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request["request_id"], "corpus_id": request["corpus_id"], "requester": request["requester"], "purpose": request["purpose"], "semantic_profile": request["semantic_profile"], "checkpoint": int(request["checkpoint"]), "disposition": disposition, "candidate_order": ids, "qualified_order": sorted(qualified), "unresolved_order": sorted(unresolved), "blocked_order": sorted(blocked), "source_order": source_order, "qualified_source_order": qualified_source_order, "missing_source_order": missing_source_order, "peer_order": peer_ids, "qualified_peer_order": sorted(qualified_peers), "missing_peer_order": sorted(missing_peers), "omission_order": sorted(omissions), "uncertainty_order": sorted(uncertainty), "negative_evidence_order": sorted(negative), "ranked_scores_milli": scores, "replay_identity": request["replay_identity"], "boundary": PRECLINICAL_BOUNDARY}
    digest = _hash(payload)
    result = {**payload, "synthesis_digest": digest, "artifact": {"artifact_id": f"evidence-synthesis-8:{request['request_id']}", "content_type": CONTENT_TYPE, "content_hash": digest, "semantic_loss": [], "provenance_digests": sorted({x["provenance_digest"] for x in rows}), "boundary": PRECLINICAL_BOUNDARY}, "effect_receipts": [f"exchange:permitted-summaries:{request['request_id']}", f"manage:local-capability:{request['request_id']}"] if disposition == "qualified" else ["block:unsafe-release"], "raw_data_local": True, "aggregate_only": True}
    receipt = EvidenceSynthesis8(result); receipt.validate(); return receipt


def epistemicRetrievalSynthesisDigest(receipt: EvidenceSynthesis8) -> str:
    receipt.validate(); return _hash(receipt.to_dict())


__all__ = ["FEATURE_ID", "CONTRACT_VERSION", "INPUT_SCHEMA", "OUTPUT_SCHEMA", "CONTENT_TYPE", "EvidenceSynthesis8", "retrieval_synthesis_manifest", "operate_retrieval_synthesis", "epistemicRetrievalSynthesisDigest"]
