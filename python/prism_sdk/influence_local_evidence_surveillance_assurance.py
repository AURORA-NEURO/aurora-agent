"""Parity contract for AFA-influence-P01-F25 local evidence surveillance assurance."""
from __future__ import annotations
from dataclasses import dataclass
import re
from typing import Any, Mapping, Sequence
from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError, research_artifact_digest

FEATURE_ID = "AFA-influence-P01-F25"
CONTRACT_VERSION = "influence-local-evidence-surveillance-assurance/1.0"
CONTENT_TYPE = "application/vnd.aurora.influence-local-evidence-surveillance-assurance+json"

@dataclass(frozen=True)
class InfluenceEvidenceObservation:
    evidence_id: str; source_id: str; study_id: str; modality: str; scope: str
    relevance_milli: int; state: str; semantic_digest: str; artifact_digest: str
    provenance_digest: str; replay_identity: str; omissions: tuple[str, ...] = ()
    negative_evidence: tuple[str, ...] = (); raw_data_local: bool = True
    boundary: str = PRECLINICAL_BOUNDARY

@dataclass(frozen=True)
class InfluenceEvidenceFeedRequest:
    request_id: str; study_id: str; scope: str; query: str; minimum_relevance_milli: int
    observations: tuple[InfluenceEvidenceObservation, ...]; replay_identity: str
    policy_allow: bool = True; protected_closure: bool = True; raw_data_local: bool = True
    boundary: str = PRECLINICAL_BOUNDARY

@dataclass(frozen=True)
class InfluenceQualifiedEvidenceSet:
    value: Mapping[str, Any]
    def validate(self) -> None:
        v = self.value
        if v.get("schema_version") != RESEARCH_CONTRACT_SCHEMA_VERSION or v.get("contract_version") != CONTRACT_VERSION or v.get("feature_id") != FEATURE_ID or v.get("boundary") != PRECLINICAL_BOUNDARY or v.get("raw_data_local") is not True or not v.get("candidate_order") or not v.get("effect_receipts"):
            raise ResearchContractError("influence evidence identity, candidates, locality, or effects are incomplete")
        for key in ("candidate_order", "qualified_order", "blocked_order", "unknown_order", "source_order", "modality_order", "omissions", "uncertainty", "negative_evidence", "effect_receipts"):
            values = tuple(v.get(key, ()))
            if tuple(sorted(set(values))) != values: raise ResearchContractError("influence evidence ordering is not canonical")
        classified = set(v.get("qualified_order", ())) | set(v.get("blocked_order", ())) | set(v.get("unknown_order", ()))
        if classified != set(v["candidate_order"]): raise ResearchContractError("influence evidence states do not partition candidates")
        for d in (v.get("replay_identity"), v.get("artifact", {}).get("content_hash")):
            if not isinstance(d, str) or not re.fullmatch(r"[0-9a-f]{64}", d): raise ResearchContractError("influence evidence digest is invalid")
    def digest(self) -> str:
        self.validate(); return research_artifact_digest(dict(self.value))

def influence_local_evidence_surveillance_manifest() -> dict[str, Any]:
    return {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "capability_id": FEATURE_ID, "version": CONTRACT_VERSION, "owner_crate": "influence", "input_schema": "EvidenceFeedRequest1@1", "output_schema": "InfluenceQualifiedEvidenceSet1@1", "autonomy_tier": "A0", "determinism": "byte_stable", "boundary": PRECLINICAL_BOUNDARY}

def assure_local_evidence_surveillance(*, request_id: str, study_id: str, scope: str, query: str, minimum_relevance_milli: int, observations: Sequence[InfluenceEvidenceObservation], replay_identity: str, policy_allow: bool = True, protected_closure: bool = True, raw_data_local: bool = True) -> InfluenceQualifiedEvidenceSet:
    if not all(isinstance(x, str) and x.strip() for x in (request_id, study_id, scope, query)) or not observations or not re.fullmatch(r"[0-9a-f]{64}", replay_identity) or not raw_data_local:
        raise ResearchContractError("influence evidence request identity, observations, replay, locality, or boundary is invalid")
    ordered = sorted(observations, key=lambda x: (-x.relevance_milli, x.evidence_id)); candidates = [x.evidence_id for x in ordered]
    if len(set(candidates)) != len(candidates): raise ResearchContractError("influence evidence identities must be unique")
    qualified: set[str] = set(); blocked: set[str] = set(); unknown: set[str] = set(); sources: set[str] = set(); modalities: set[str] = set(); semantic: set[str] = set(); artifacts: set[str] = set(); provenance: set[str] = set(); omissions: set[str] = set(); uncertainty: set[str] = set(); negative: set[str] = set()
    for item in ordered:
        gate = policy_allow and protected_closure and raw_data_local and item.raw_data_local and item.boundary == PRECLINICAL_BOUNDARY and item.state == "supported" and item.study_id == study_id and item.scope == scope and item.relevance_milli >= minimum_relevance_milli and item.replay_identity == replay_identity and not item.omissions and not item.negative_evidence
        if gate:
            qualified.add(item.evidence_id); sources.add(item.source_id); modalities.add(item.modality); semantic.add(item.semantic_digest); artifacts.add(item.artifact_digest); provenance.add(item.provenance_digest)
        else:
            blocked.add(item.evidence_id)
            if item.state in {"unknown", "speculative"}: unknown.add(item.evidence_id); uncertainty.add(f"evidence:{item.evidence_id}:state-unknown")
            if item.state == "contradicted": negative.add(f"evidence:{item.evidence_id}:contradicted-negative-evidence")
            if item.replay_identity != replay_identity: uncertainty.add(f"evidence:{item.evidence_id}:replay-mismatch")
            if item.omissions: omissions.update(f"evidence:{item.evidence_id}:{x}" for x in item.omissions)
    if not policy_allow: omissions.add("request:policy-denied")
    if not protected_closure: omissions.add("request:protected-closure-incomplete")
    disposition = "blocked" if not policy_allow or not protected_closure or not raw_data_local else "unknown" if not qualified else "qualified"
    effects = (f"read:local-research-artifacts:{request_id}",) if disposition == "qualified" else ("block:unsafe-release",)
    payload = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request_id, "study_id": study_id, "scope": scope, "disposition": disposition, "candidate_order": candidates, "qualified_order": sorted(qualified), "blocked_order": sorted(blocked), "unknown_order": sorted(unknown), "source_order": sorted(sources), "modality_order": sorted(modalities), "relevance_order": [x.relevance_milli for x in ordered], "semantic_order": sorted(semantic), "artifact_order": sorted(artifacts), "provenance_order": sorted(provenance), "omissions": sorted(omissions), "uncertainty": sorted(uncertainty), "negative_evidence": sorted(negative), "replay_identity": replay_identity, "effect_receipts": list(effects), "raw_data_local": raw_data_local, "boundary": PRECLINICAL_BOUNDARY}
    payload["artifact"] = {"content_hash": research_artifact_digest(payload), "content_type": CONTENT_TYPE, "boundary": PRECLINICAL_BOUNDARY}
    receipt = InfluenceQualifiedEvidenceSet(payload); receipt.validate(); return receipt

__all__ = ["FEATURE_ID", "CONTRACT_VERSION", "CONTENT_TYPE", "InfluenceEvidenceObservation", "InfluenceEvidenceFeedRequest", "InfluenceQualifiedEvidenceSet", "influence_local_evidence_surveillance_manifest", "assure_local_evidence_surveillance"]
