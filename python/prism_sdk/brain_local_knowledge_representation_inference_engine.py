"""Local single-study knowledge-representation inference parity contract."""
from __future__ import annotations

from dataclasses import dataclass
import re
from typing import Any, Mapping, Sequence

from .research_contracts import (
    LOCAL_KNOWLEDGE_REPRESENTATION_INFERENCE_ENGINE_CONTRACT_VERSION,
    LOCAL_KNOWLEDGE_REPRESENTATION_INFERENCE_ENGINE_FEATURE_ID,
    PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
    ResearchContractError,
    research_artifact_digest,
)


@dataclass(frozen=True)
class KnowledgeRepresentationClaim:
    claim_id: str
    subject: str
    predicate: str
    object: str
    evidence_digest: str | None
    provenance_digest: str | None
    state: str = "supported"
    study_id: str = "study:one"
    boundary: str = PRECLINICAL_BOUNDARY


@dataclass(frozen=True)
class KnowledgeRepresentationReceipt:
    request_id: str
    study_id: str
    disposition: str
    candidate_order: tuple[str, ...]
    admitted_order: tuple[str, ...]
    unresolved_order: tuple[str, ...]
    denied_order: tuple[str, ...]
    world_digest: str
    evidence_digest: str
    provenance_digest: str
    replay_identity: str
    witness_order: tuple[str, ...]
    counterexample_order: tuple[str, ...]
    omissions: tuple[str, ...]
    uncertainty: tuple[str, ...]
    negative_evidence: tuple[str, ...]
    effect_receipts: tuple[str, ...]
    artifact: Mapping[str, Any]
    feature_id: str = LOCAL_KNOWLEDGE_REPRESENTATION_INFERENCE_ENGINE_FEATURE_ID
    contract_version: str = LOCAL_KNOWLEDGE_REPRESENTATION_INFERENCE_ENGINE_CONTRACT_VERSION
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    raw_data_local: bool = True
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION or self.contract_version != LOCAL_KNOWLEDGE_REPRESENTATION_INFERENCE_ENGINE_CONTRACT_VERSION or self.feature_id != LOCAL_KNOWLEDGE_REPRESENTATION_INFERENCE_ENGINE_FEATURE_ID:
            raise ResearchContractError("knowledge representation schema, feature, or version mismatch")
        if self.boundary != PRECLINICAL_BOUNDARY or not self.raw_data_local or not self.request_id.strip() or not self.study_id.strip() or not self.candidate_order or not self.effect_receipts:
            raise ResearchContractError("knowledge representation identity, locality, candidates, or effects are incomplete")
        for values in (self.candidate_order, self.admitted_order, self.unresolved_order, self.denied_order, self.witness_order, self.counterexample_order, self.omissions, self.uncertainty, self.negative_evidence, self.effect_receipts):
            if tuple(sorted(set(values))) != values:
                raise ResearchContractError("knowledge representation ordering is not canonical")
        classified = set(self.admitted_order) | set(self.unresolved_order) | set(self.denied_order)
        if classified != set(self.candidate_order):
            raise ResearchContractError("knowledge representation states do not partition claims")
        for value in (self.world_digest, self.evidence_digest, self.provenance_digest, self.replay_identity, self.artifact.get("content_hash")):
            if not isinstance(value, str) or not re.fullmatch(r"[0-9a-f]{64}", value):
                raise ResearchContractError("knowledge representation digest is invalid")
        if any(not effect.startswith("read:local-knowledge-world:") and effect != "block:unsafe-release" for effect in self.effect_receipts):
            raise ResearchContractError("knowledge representation effect is outside read-only gate")

    def digest(self) -> str:
        self.validate()
        return research_artifact_digest({"schema_version": self.schema_version, "contract_version": self.contract_version, "feature_id": self.feature_id, "request_id": self.request_id, "study_id": self.study_id, "disposition": self.disposition, "candidate_order": list(self.candidate_order), "admitted_order": list(self.admitted_order), "unresolved_order": list(self.unresolved_order), "denied_order": list(self.denied_order), "world_digest": self.world_digest, "evidence_digest": self.evidence_digest, "provenance_digest": self.provenance_digest, "replay_identity": self.replay_identity, "witness_order": list(self.witness_order), "counterexample_order": list(self.counterexample_order), "omissions": list(self.omissions), "uncertainty": list(self.uncertainty), "negative_evidence": list(self.negative_evidence), "effect_receipts": list(self.effect_receipts), "artifact": dict(self.artifact), "raw_data_local": self.raw_data_local, "boundary": self.boundary})


def infer_local_knowledge_representation(*, request_id: str, study_id: str, claims: Sequence[KnowledgeRepresentationClaim], required_claim_ids: Sequence[str], replay_identity: str, policy_allow: bool = True, protected_closure: bool = True, raw_data_local: bool = True) -> KnowledgeRepresentationReceipt:
    if not request_id.strip() or not study_id.strip() or not claims or not re.fullmatch(r"[0-9a-f]{64}", replay_identity) or not raw_data_local:
        raise ResearchContractError("knowledge representation identity, claims, replay, or locality is invalid")
    ordered = tuple(sorted(claim.claim_id for claim in claims))
    if len(set(ordered)) != len(claims) or any(not value.strip() for value in ordered):
        raise ResearchContractError("claim identifiers must be unique and non-empty")
    claim_map = {claim.claim_id: claim for claim in claims}; admitted: set[str] = set(); unresolved: set[str] = set(); denied: set[str] = set(); facts: list[dict[str, Any]] = []; witnesses = {"gate:typed-scoped-claims", "gate:study-scope", "gate:evidence-provenance", "gate:unknown-is-not-asserted", "gate:locality"}; counter: set[str] = set(); omissions: set[str] = set(); uncertainty: set[str] = set(); negative: set[str] = set()
    for claim_id in ordered:
        claim = claim_map[claim_id]
        if not policy_allow or not protected_closure or claim.study_id != study_id or claim.boundary != PRECLINICAL_BOUNDARY:
            denied.add(claim_id); counter.add(f"counterexample:{claim_id}:scope-policy-closure")
        elif claim.evidence_digest is None or claim.provenance_digest is None:
            unresolved.add(claim_id); omissions.add(f"claim:{claim_id}:evidence-or-provenance-missing")
        elif claim.state in {"unknown", "speculative"}:
            unresolved.add(claim_id); uncertainty.add(f"claim:{claim_id}:unknown-not-asserted")
        elif claim.state == "contradicted":
            denied.add(claim_id); negative.add(f"claim:{claim_id}:contradicted")
        else:
            admitted.add(claim_id); facts.append({"claim_id": claim.claim_id, "subject": claim.subject, "predicate": claim.predicate, "object": claim.object, "evidence_digest": claim.evidence_digest, "provenance_digest": claim.provenance_digest})
    for required_id in required_claim_ids:
        if required_id not in claim_map:
            omissions.add(f"claim:{required_id}:required-missing"); uncertainty.add(f"claim:{required_id}:required-unresolved")
        elif required_id not in admitted:
            uncertainty.add(f"claim:{required_id}:required-not-admitted")
    if not policy_allow: omissions.add("control:policy-denied")
    if not protected_closure: omissions.add("control:protected-closure-incomplete")
    if unresolved: witnesses.add("gate:omissions-retained")
    facts.sort(key=lambda fact: fact["claim_id"]); world = research_artifact_digest({"study_id": study_id, "facts": facts}); evidence = research_artifact_digest({"candidate_order": list(ordered), "admitted_order": sorted(admitted), "unresolved_order": sorted(unresolved), "denied_order": sorted(denied)}); provenance = research_artifact_digest({"request_id": request_id, "study_id": study_id, "world_digest": world, "replay_identity": replay_identity}); disposition = "denied" if not policy_allow or not protected_closure or (denied and not admitted) else "unresolved" if not admitted else "partial" if unresolved or denied else "completed"; artifact = {"content_hash": research_artifact_digest({"request_id": request_id, "world_digest": world}), "media_type": "application/vnd.aurora.typed-knowledge-world+json"}
    receipt = KnowledgeRepresentationReceipt(request_id=request_id, study_id=study_id, disposition=disposition, candidate_order=ordered, admitted_order=tuple(sorted(admitted)), unresolved_order=tuple(sorted(unresolved)), denied_order=tuple(sorted(denied)), world_digest=world, evidence_digest=evidence, provenance_digest=provenance, replay_identity=replay_identity, witness_order=tuple(sorted(witnesses)), counterexample_order=tuple(sorted(counter)), omissions=tuple(sorted(omissions)), uncertainty=tuple(sorted(uncertainty)), negative_evidence=tuple(sorted(negative)), effect_receipts=(f"read:local-knowledge-world:{request_id}",) if disposition in {"completed", "partial"} else ("block:unsafe-release",), artifact=artifact, raw_data_local=raw_data_local); receipt.validate(); return receipt
