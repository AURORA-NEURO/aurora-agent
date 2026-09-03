"""Deterministic, local-only retrieval-and-synthesis inference for Worldgen P02 F01–F04."""
from __future__ import annotations

from dataclasses import dataclass
import hashlib
import json
import re
from typing import Any, Iterable

from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError


_HEX = re.compile(r"^[0-9a-f]{64}$")


@dataclass(frozen=True)
class RetrievalCandidate:
    candidate_id: str
    source_id: str
    title: str
    study_id: str
    modality: str
    relevance_milli: int
    freshness_milli: int
    evidence_state: str
    content_digest: str
    provenance_digest: str
    replay_identity: str
    estimated_units: int
    permitted: bool = True
    comparable: bool = True
    negative_result: bool = False


@dataclass(frozen=True)
class RetrievalQuery:
    request_id: str
    researcher: str
    corpus_id: str
    purpose: str
    semantic_profile: str
    query_terms: tuple[str, ...]
    candidates: tuple[RetrievalCandidate, ...]
    minimum_relevance_milli: int
    minimum_freshness_milli: int
    max_budget_units: int
    replay_identity: str
    policy_allow: bool = True
    protected_closure: bool = True
    raw_data_local: bool = True
    aggregate_only: bool = True
    boundary: str = PRECLINICAL_BOUNDARY


@dataclass(frozen=True)
class RetrievalReceipt:
    value: dict[str, Any]

    def validate(self, *, feature_id: str, contract_version: str) -> None:
        value = self.value
        artifact = value.get("artifact", {})
        if (
            value.get("schema_version") != RESEARCH_CONTRACT_SCHEMA_VERSION
            or value.get("contract_version") != contract_version
            or value.get("feature_id") != feature_id
            or value.get("boundary") != PRECLINICAL_BOUNDARY
            or artifact.get("boundary") != PRECLINICAL_BOUNDARY
            or value.get("raw_data_local") is not True
            or value.get("aggregate_only") is not True
            or not value.get("request_id", "").strip()
            or not value.get("researcher", "").strip()
            or not value.get("corpus_id", "").strip()
            or not value.get("semantic_profile", "").strip()
            or not value.get("candidate_order")
            or not value.get("ranked_order")
            or len(value.get("ranked_order", [])) != len(value.get("ranked_scores_milli", []))
            or not value.get("effect_receipts")
        ):
            raise ResearchContractError("worldgen retrieval identity, locality, ranking, or effects are incomplete")
        for key in (
            "candidate_order", "selected_order", "unresolved_order", "blocked_order", "source_order",
            "omission_order", "uncertainty_order", "negative_evidence_order", "effect_receipts",
        ):
            values = tuple(value.get(key, ()))
            if values != tuple(sorted(set(values))):
                raise ResearchContractError("worldgen retrieval ordering is not canonical")
        candidates = set(value["candidate_order"])
        parts = list(value.get("selected_order", ())) + list(value.get("unresolved_order", ())) + list(value.get("blocked_order", ()))
        if len(candidates) != len(value["candidate_order"]) or len(parts) != len(candidates) or len(set(parts)) != len(parts) or set(parts) != candidates:
            raise ResearchContractError("worldgen retrieval states do not partition candidates")
        if set(value["ranked_order"]) != candidates or len(value["ranked_order"]) != len(candidates):
            raise ResearchContractError("worldgen retrieval ranking does not cover candidates")
        for digest in (value.get("replay_identity"), value.get("synthesis_digest"), artifact.get("content_hash")):
            if not isinstance(digest, str) or not _HEX.fullmatch(digest):
                raise ResearchContractError("worldgen retrieval digest is invalid")
        if artifact.get("content_hash") != value.get("synthesis_digest"):
            raise ResearchContractError("worldgen retrieval artifact digest is inconsistent")

    def digest(self, *, feature_id: str, contract_version: str) -> str:
        self.validate(feature_id=feature_id, contract_version=contract_version)
        return _digest(self.value)


def _digest(value: Any) -> str:
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()).hexdigest()


def manifest(*, feature_id: str, contract_version: str, input_schema: str, scale: str, autonomy_tier: str) -> dict[str, Any]:
    return {
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "capability_id": feature_id,
        "version": contract_version,
        "owner_crate": "worldgen",
        "consumers": ["imaging core scientist", "benchmark curator", "research program lead", "preclinical neuroscientist"],
        "behavior": f"rank typed evidence summaries for {scale} retrieval and synthesis without network retrieval",
        "value": "turns bounded evidence candidates into auditable, omission-aware synthesis receipts",
        "input_schema": input_schema,
        "output_schema": "EvidenceSynthesis1@1",
        "effects": ["retain:local-evidence-synthesis", "block:unsafe-release"],
        "permissions": ["read:local-research-artifacts"],
        "determinism": "byte_stable",
        "autonomy_tier": autonomy_tier,
        "boundary": PRECLINICAL_BOUNDARY,
    }


def infer(query: RetrievalQuery, *, feature_id: str, contract_version: str) -> RetrievalReceipt:
    if (
        not all(isinstance(item, str) and item.strip() for item in (query.request_id, query.researcher, query.corpus_id, query.purpose, query.semantic_profile))
        or not query.query_terms or not query.candidates or query.max_budget_units <= 0
        or query.boundary != PRECLINICAL_BOUNDARY or not query.raw_data_local or not query.aggregate_only
        or not _HEX.fullmatch(query.replay_identity)
    ):
        raise ResearchContractError("worldgen retrieval request identity, terms, budget, replay, locality, or boundary is invalid")
    ranked = sorted(query.candidates, key=lambda candidate: (-(candidate.relevance_milli * 7 + candidate.freshness_milli * 3), candidate.candidate_id))
    if len({candidate.candidate_id for candidate in ranked}) != len(ranked):
        raise ResearchContractError("worldgen retrieval candidate ids must be unique")
    for candidate in ranked:
        if (
            not all(isinstance(item, str) and item.strip() for item in (candidate.candidate_id, candidate.source_id, candidate.title, candidate.study_id, candidate.modality))
            or not all(isinstance(item, int) and 0 <= item <= 1000 for item in (candidate.relevance_milli, candidate.freshness_milli))
            or candidate.estimated_units < 0
            or not all(isinstance(item, str) and _HEX.fullmatch(item) for item in (candidate.content_digest, candidate.provenance_digest, candidate.replay_identity))
        ):
            raise ResearchContractError("worldgen retrieval candidate labels, scores, units, or digests are invalid")

    canonical = sorted(candidate.candidate_id for candidate in ranked)
    ranked_order = [candidate.candidate_id for candidate in ranked]
    ranked_scores = [candidate.relevance_milli * 7 + candidate.freshness_milli * 3 for candidate in ranked]
    selected: set[str] = set(); unresolved: set[str] = set(); blocked: set[str] = set(); sources: set[str] = set()
    omissions: set[str] = set(); uncertainty: set[str] = set(); negative: set[str] = set(); total_units = 0
    for candidate in ranked:
        fits_budget = total_units + candidate.estimated_units <= query.max_budget_units
        qualified = (
            query.policy_allow and query.protected_closure and candidate.permitted and candidate.comparable
            and candidate.evidence_state == "supported" and candidate.relevance_milli >= query.minimum_relevance_milli
            and candidate.freshness_milli >= query.minimum_freshness_milli and candidate.replay_identity == query.replay_identity
            and fits_budget
        )
        if qualified:
            selected.add(candidate.candidate_id); sources.add(candidate.source_id); total_units += candidate.estimated_units; continue
        unresolved_state = candidate.evidence_state in {"unknown", "unmeasured", "speculative"}
        if unresolved_state:
            unresolved.add(candidate.candidate_id); uncertainty.add(f"candidate:{candidate.candidate_id}:evidence-unresolved")
        else:
            blocked.add(candidate.candidate_id)
        if candidate.negative_result: negative.add(f"candidate:{candidate.candidate_id}:negative-result-retained")
        if candidate.relevance_milli < query.minimum_relevance_milli: uncertainty.add(f"candidate:{candidate.candidate_id}:low-relevance")
        if candidate.freshness_milli < query.minimum_freshness_milli: uncertainty.add(f"candidate:{candidate.candidate_id}:stale")
        if not candidate.comparable: omissions.add(f"candidate:{candidate.candidate_id}:incomparable")
        if candidate.replay_identity != query.replay_identity: uncertainty.add(f"candidate:{candidate.candidate_id}:replay-mismatch")
        if not fits_budget: omissions.add(f"candidate:{candidate.candidate_id}:budget-exceeded")
        if not candidate.permitted: omissions.add(f"candidate:{candidate.candidate_id}:permission-denied")
        if candidate.evidence_state != "supported": uncertainty.add(f"candidate:{candidate.candidate_id}:evidence-state-{candidate.evidence_state}")
    if not query.policy_allow: omissions.add("request:policy-denied")
    if not query.protected_closure: uncertainty.add("request:protected-closure-incomplete")
    disposition = "blocked" if not query.policy_allow or not query.protected_closure else "unknown" if not selected else "qualified" if not blocked and not omissions and not uncertainty and not negative else "partial"
    effects = [f"retain:local-evidence-synthesis:{query.request_id}"] if disposition == "qualified" else ["block:unsafe-release"]
    payload: dict[str, Any] = {
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": contract_version, "feature_id": feature_id,
        "request_id": query.request_id, "researcher": query.researcher, "corpus_id": query.corpus_id, "semantic_profile": query.semantic_profile,
        "disposition": disposition, "candidate_order": canonical, "ranked_order": ranked_order, "selected_order": sorted(selected),
        "unresolved_order": sorted(unresolved), "blocked_order": sorted(blocked), "source_order": sorted(sources), "omission_order": sorted(omissions),
        "uncertainty_order": sorted(uncertainty), "negative_evidence_order": sorted(negative), "ranked_scores_milli": ranked_scores,
        "total_units": total_units, "replay_identity": query.replay_identity, "boundary": PRECLINICAL_BOUNDARY,
    }
    synthesis_digest = _digest(payload)
    payload["synthesis_digest"] = synthesis_digest
    payload["artifact"] = {"artifact_id": f"evidence-synthesis:{query.request_id}", "content_type": "application/vnd.aurora.worldgen.evidence-synthesis-1+json", "content_hash": synthesis_digest, "boundary": PRECLINICAL_BOUNDARY}
    payload["effect_receipts"] = effects; payload["raw_data_local"] = True; payload["aggregate_only"] = True
    receipt = RetrievalReceipt(payload)
    receipt.validate(feature_id=feature_id, contract_version=contract_version)
    return receipt


__all__ = ["RetrievalCandidate", "RetrievalQuery", "RetrievalReceipt", "infer", "manifest"]
