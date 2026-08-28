"""Python parity for ``AFA-ids-P02-F28`` retrieval/synthesis assurance."""
from __future__ import annotations

import hashlib
import json
import re
from dataclasses import dataclass
from typing import Any, Mapping

from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError

FEATURE_ID = "AFA-ids-P02-F28"
CONTRACT_VERSION = "ids-federated-continual-retrieval-synthesis-assurance-harness/1.0"
INPUT_SCHEMA = "ScopedRetrievalQuery6@1"
OUTPUT_SCHEMA = "EvidenceSynthesis11@1"
CONTENT_TYPE = "application/vnd.aurora.evidence-synthesis-11+json"


def _hash(value: Any) -> str:
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()).hexdigest()


def _digest(value: Any) -> bool:
    return isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) is not None


def _ordered(values: list[str]) -> bool:
    return values == sorted(set(values))


@dataclass(frozen=True)
class EvidenceSynthesis11:
    value: dict[str, Any]

    def to_dict(self) -> dict[str, Any]:
        return dict(self.value)

    def validate(self) -> None:
        v = self.value
        if v.get("schema_version") != RESEARCH_CONTRACT_SCHEMA_VERSION or v.get("contract_version") != CONTRACT_VERSION or v.get("feature_id") != FEATURE_ID or v.get("boundary") != PRECLINICAL_BOUNDARY or v.get("raw_data_local") is not True or v.get("aggregate_only") is not True or not all(isinstance(v.get(k), str) and v[k].strip() for k in ("request_id", "corpus_id", "requester", "purpose", "semantic_profile")) or int(v.get("checkpoint", 0)) <= 0 or not v.get("candidate_order") or not v.get("source_order") or not v.get("peer_order") or not v.get("effect_receipts") or v.get("disposition") not in {"qualified", "unresolved", "blocked"}:
            raise ResearchContractError("retrieval identity, checkpoint, locality, candidates, sources, peers, or effects are incomplete")
        fields = ("candidate_order", "qualified_order", "unresolved_order", "blocked_order", "source_order", "qualified_source_order", "missing_source_order", "peer_order", "qualified_peer_order", "missing_peer_order", "low_relevance_order", "stale_order", "incomparable_order", "omission_order", "uncertainty_order", "negative_evidence_order", "effect_receipts")
        if any(not _ordered(v.get(k, [])) for k in fields):
            raise ResearchContractError("retrieval assurance ordering is not canonical")
        candidates = set(v["candidate_order"])
        parts = [*v["qualified_order"], *v["unresolved_order"], *v["blocked_order"]]
        sources = set(v["source_order"])
        source_parts = [*v["qualified_source_order"], *v["missing_source_order"]]
        peers = set(v["peer_order"])
        peer_parts = [*v["qualified_peer_order"], *v["missing_peer_order"]]
        if len(candidates) != len(v["candidate_order"]) or set(parts) != candidates or len(set(parts)) != len(parts) or set(source_parts) != sources or len(set(source_parts)) != len(source_parts) or set(peer_parts) != peers or len(set(peer_parts)) != len(peer_parts) or len(v["ranked_scores_milli"]) != len(v["qualified_order"]):
            raise ResearchContractError("retrieval candidate, source, peer, or score states do not partition")
        a = v.get("artifact", {})
        digests = [v.get("replay_identity"), v.get("synthesis_digest"), a.get("content_hash"), *a.get("provenance_digests", [])]
        if not all(_digest(x) for x in digests) or a.get("content_type") != CONTENT_TYPE or a.get("boundary") != PRECLINICAL_BOUNDARY or a.get("content_hash") != v.get("synthesis_digest"):
            raise ResearchContractError("retrieval artifact metadata or digest is inconsistent")
        if any(not e.startswith(("exchange:permitted-summaries:", "manage:local-capability:")) and e != "block:unsafe-release" for e in v["effect_receipts"]):
            raise ResearchContractError("effect is outside the retrieval assurance gate")


def retrieval_synthesis_assurance_manifest() -> dict[str, Any]:
    return {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "capability_id": FEATURE_ID, "version": CONTRACT_VERSION, "owner_crate": "ids", "consumers": ["downstream AURORA crate maintainer", "retrieval scientist", "federation steward", "release-gate operator"], "behavior": "verifies a bounded federated retrieval corpus and synthesis closure before research-object release", "value": "prevents unsupported, stale, incomparable, contradictory, or policy-denied evidence from becoming an apparently complete synthesis", "input_schema": INPUT_SCHEMA, "output_schema": OUTPUT_SCHEMA, "effects": ["exchange:permitted-summaries", "manage:local-capability"], "permissions": ["read:local-retrieval-manifests", "evaluate:research-evidence"], "autonomy_tier": "A2", "boundary": PRECLINICAL_BOUNDARY}


def _score(candidate: Mapping[str, Any]) -> int:
    return int(candidate["relevance_milli"]) * 3 + int(candidate["freshness_milli"]) * 2 + min(len(candidate["terms"]), 1000)


def _validate_request(request: Mapping[str, Any]) -> None:
    if not all(isinstance(request.get(k), str) and request[k].strip() for k in ("request_id", "corpus_id", "requester", "purpose", "semantic_profile")) or not request.get("query_terms") or request["query_terms"] != sorted(set(request["query_terms"])) or not request.get("candidates") or len(request["candidates"]) > 8192 or not request.get("peers") or len(request["peers"]) > 1024 or int(request.get("checkpoint", 0)) <= 0 or int(request.get("max_budget_units", 0)) <= 0 or int(request.get("minimum_peer_quorum", 0)) <= 0 or not 0 <= int(request.get("minimum_relevance_milli", -1)) <= 1000 or not 0 <= int(request.get("minimum_freshness_milli", -1)) <= 1000 or not _digest(request.get("replay_identity")) or request.get("boundary") != PRECLINICAL_BOUNDARY or request.get("raw_data_local") is not True or request.get("aggregate_only") is not True:
        raise ResearchContractError("query identity, terms, candidates, peers, thresholds, replay, locality, or boundary is invalid")
    ids: set[str] = set()
    for c in request["candidates"]:
        if not all(isinstance(c.get(k), str) and c[k].strip() for k in ("evidence_id", "source_id", "origin", "title")) or c["evidence_id"] in ids or not c.get("terms") or c["terms"] != sorted(set(c["terms"])) or int(c.get("estimated_units", 0)) <= 0 or not all(0 <= int(c.get(k, -1)) <= 1000 for k in ("relevance_milli", "freshness_milli")) or not all(_digest(c.get(k)) for k in ("content_digest", "provenance_digest", "replay_identity")) or c.get("omission_reasons", []) != sorted(set(c.get("omission_reasons", []))):
            raise ResearchContractError("candidate identity, terms, scores, omission ordering, or digests are invalid")
        ids.add(c["evidence_id"])
    peer_ids: set[str] = set()
    for p in request["peers"]:
        if not all(isinstance(p.get(k), str) and p[k].strip() for k in ("peer_id", "origin", "corpus_id", "semantic_profile")) or p["peer_id"] in peer_ids or int(p.get("checkpoint", 0)) <= 0 or int(p.get("source_count", 0)) <= 0 or not _digest(p.get("synthesis_digest")):
            raise ResearchContractError("peer identity, corpus, checkpoint, source count, or digest is invalid")
        peer_ids.add(p["peer_id"])


def assure_retrieval_synthesis(request: Mapping[str, Any]) -> EvidenceSynthesis11:
    _validate_request(request)
    candidates = sorted((dict(item) for item in request["candidates"]), key=lambda item: item["evidence_id"])
    peers = sorted((dict(item) for item in request["peers"]), key=lambda item: item["peer_id"])
    ids = [c["evidence_id"] for c in candidates]
    pids = [p["peer_id"] for p in peers]
    qualified: set[str] = set(); unresolved: set[str] = set(); blocked: set[str] = set(); low: set[str] = set(); stale: set[str] = set(); incomparable: set[str] = set(); omissions: set[str] = set(); uncertainty: set[str] = set(); negative: set[str] = set(); ranked: dict[str, int] = {}; total = 0
    for c in candidates:
        cid = c["evidence_id"]; total += int(c["estimated_units"])
        if c.get("negative_result"): negative.add(f"{cid}:negative-result")
        if c.get("evidence_state") == "contradicted": blocked.add(cid); negative.add(f"{cid}:contradicted"); continue
        if c.get("local_only") is not True or c.get("aggregate_only") is not True: blocked.add(cid); omissions.add(f"{cid}:raw-data-not-local-or-aggregate"); continue
        if c.get("replay_identity") != request["replay_identity"] or c.get("signed") is not True or c.get("permitted") is not True: unresolved.add(cid); omissions.add(f"{cid}:replay-or-authorization"); continue
        if c.get("evidence_state") not in {"proven", "supported"}: unresolved.add(cid); uncertainty.add(f"{cid}:evidence-state"); continue
        failed = False
        if int(c["relevance_milli"]) < int(request["minimum_relevance_milli"]): low.add(cid); failed = True
        if int(c["freshness_milli"]) < int(request["minimum_freshness_milli"]): stale.add(cid); failed = True
        if c.get("comparable") is not True: incomparable.add(cid); failed = True
        if failed: unresolved.add(cid); uncertainty.add(f"{cid}:retrieval-threshold-or-comparability")
        else: qualified.add(cid); ranked[cid] = _score(c)
        omissions.update(f"{cid}:{reason}" for reason in c.get("omission_reasons", []))
    source_order = sorted({c["source_id"] for c in candidates}); qualified_sources = sorted({c["source_id"] for c in candidates if c["evidence_id"] in qualified}); missing_sources = sorted(set(source_order) - set(qualified_sources))
    if missing_sources: omissions.add("source:qualified-closure-incomplete")
    qualified_peers = {p["peer_id"] for p in peers if p["corpus_id"] == request["corpus_id"] and p["semantic_profile"] == request["semantic_profile"] and int(p["checkpoint"]) == int(request["checkpoint"]) and p.get("source_count", 0) > 0 and p.get("signed") is True and p.get("aggregate_only") is True and p.get("raw_data_local") is True and p.get("evidence_state") in {"proven", "supported"}}
    missing_peers = set(pids) - qualified_peers; uncertainty.update(f"peer:{p}:not-qualified" for p in missing_peers)
    if len(qualified_peers) < int(request["minimum_peer_quorum"]): uncertainty.add("peer:minimum-quorum-unmet")
    if total > int(request["max_budget_units"]): omissions.add(f"request:budget-exceeded:{total}")
    if request.get("policy_allow") is not True: negative.add("request:policy-denied")
    if request.get("protected_closure") is not True: uncertainty.add("request:protected-closure-incomplete")
    if request.get("signed_approval") is not True: uncertainty.add("request:signed-approval-missing")
    if request.get("federation_approved") is not True: uncertainty.add("request:federation-approval-missing")
    global_block = not all(request.get(k) is True for k in ("policy_allow", "protected_closure", "signed_approval", "federation_approved", "raw_data_local", "aggregate_only"))
    if global_block: blocked.update(ids); qualified.clear(); unresolved.clear(); omissions.add("request:retrieval-synthesis-not-authorized")
    disposition = "blocked" if global_block or (not qualified and blocked) else "unresolved" if not qualified or len(qualified_peers) < int(request["minimum_peer_quorum"]) or total > int(request["max_budget_units"]) else "qualified"
    if disposition != "qualified": omissions.add("request:synthesis-not-release-ready")
    qorder = sorted(qualified); uorder = sorted(unresolved); border = sorted(blocked); scores = [ranked.get(x, 0) for x in qorder]
    payload = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request["request_id"], "corpus_id": request["corpus_id"], "requester": request["requester"], "purpose": request["purpose"], "semantic_profile": request["semantic_profile"], "checkpoint": int(request["checkpoint"]), "disposition": disposition, "candidate_order": ids, "qualified_order": qorder, "unresolved_order": uorder, "blocked_order": border, "source_order": source_order, "qualified_source_order": qualified_sources, "missing_source_order": missing_sources, "peer_order": pids, "qualified_peer_order": sorted(qualified_peers), "missing_peer_order": sorted(missing_peers), "low_relevance_order": sorted(low), "stale_order": sorted(stale), "incomparable_order": sorted(incomparable), "omission_order": sorted(omissions), "uncertainty_order": sorted(uncertainty), "negative_evidence_order": sorted(negative), "ranked_scores_milli": scores, "total_units": total, "replay_identity": request["replay_identity"], "boundary": PRECLINICAL_BOUNDARY}
    digest = _hash(payload)
    result = {**payload, "synthesis_digest": digest, "artifact": {"artifact_id": f"evidence-synthesis-11:{request['request_id']}", "content_type": CONTENT_TYPE, "content_hash": digest, "semantic_loss": sorted(omissions), "provenance_digests": sorted({c["provenance_digest"] for c in candidates}), "boundary": PRECLINICAL_BOUNDARY}, "effect_receipts": sorted([f"exchange:permitted-summaries:{request['request_id']}", f"manage:local-capability:{request['request_id']}"] if disposition == "qualified" else ["block:unsafe-release"]), "raw_data_local": True, "aggregate_only": True}
    receipt = EvidenceSynthesis11(result); receipt.validate(); return receipt


def idsRetrievalSynthesisAssuranceDigest(receipt: EvidenceSynthesis11) -> str:
    receipt.validate(); return _hash(receipt.to_dict())


__all__ = ["FEATURE_ID", "CONTRACT_VERSION", "INPUT_SCHEMA", "OUTPUT_SCHEMA", "CONTENT_TYPE", "EvidenceSynthesis11", "retrieval_synthesis_assurance_manifest", "assure_retrieval_synthesis", "idsRetrievalSynthesisAssuranceDigest"]
