"""Versioned local knowledge-representation contract model parity surface."""
from __future__ import annotations

from dataclasses import dataclass
import re
from typing import Any, Mapping, Sequence

from .research_contracts import (
    LOCAL_KNOWLEDGE_REPRESENTATION_CONTRACT_MODEL_CONTRACT_VERSION,
    LOCAL_KNOWLEDGE_REPRESENTATION_CONTRACT_MODEL_FEATURE_ID,
    PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
    ResearchContractError,
    research_artifact_digest,
)


@dataclass(frozen=True)
class KnowledgeContractClaim:
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
class KnowledgeContractReceipt:
    request_id: str
    study_id: str
    disposition: str
    input_schema: str
    output_schema: str
    source_revision: int
    target_revision: int
    candidate_order: tuple[str, ...]
    admitted_order: tuple[str, ...]
    unresolved_order: tuple[str, ...]
    denied_order: tuple[str, ...]
    missing_order: tuple[str, ...]
    semantic_loss_order: tuple[str, ...]
    contract_digest: str
    migration_digest: str
    replay_identity: str
    omissions: tuple[str, ...]
    uncertainty: tuple[str, ...]
    negative_evidence: tuple[str, ...]
    effect_receipts: tuple[str, ...]
    artifact: Mapping[str, Any]
    feature_id: str = LOCAL_KNOWLEDGE_REPRESENTATION_CONTRACT_MODEL_FEATURE_ID
    contract_version: str = LOCAL_KNOWLEDGE_REPRESENTATION_CONTRACT_MODEL_CONTRACT_VERSION
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    raw_data_local: bool = True
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION or self.feature_id != LOCAL_KNOWLEDGE_REPRESENTATION_CONTRACT_MODEL_FEATURE_ID or self.contract_version != LOCAL_KNOWLEDGE_REPRESENTATION_CONTRACT_MODEL_CONTRACT_VERSION:
            raise ResearchContractError("knowledge contract schema, feature, or version mismatch")
        if self.boundary != PRECLINICAL_BOUNDARY or not self.raw_data_local or not self.request_id.strip() or not self.study_id.strip() or self.input_schema != "ScopedResearchClaims1@1" or self.output_schema != "TypedKnowledgeWorld1@1" or self.source_revision <= 0 or self.target_revision < self.source_revision or not self.candidate_order or not self.effect_receipts:
            raise ResearchContractError("knowledge contract identity, schema, revision, locality, candidates, or effects are incomplete")
        for values in (self.candidate_order, self.admitted_order, self.unresolved_order, self.denied_order, self.missing_order, self.semantic_loss_order, self.omissions, self.uncertainty, self.negative_evidence, self.effect_receipts):
            if tuple(sorted(set(values))) != values:
                raise ResearchContractError("knowledge contract ordering is not canonical")
        classified = set(self.admitted_order) | set(self.unresolved_order) | set(self.denied_order)
        if classified != set(self.candidate_order) or any(value not in self.candidate_order for value in self.missing_order):
            raise ResearchContractError("knowledge contract states do not partition claims")
        for value in (self.contract_digest, self.migration_digest, self.replay_identity, self.artifact.get("content_hash")):
            if not isinstance(value, str) or not re.fullmatch(r"[0-9a-f]{64}", value):
                raise ResearchContractError("knowledge contract digest is invalid")
        if any(not effect.startswith("read:local-knowledge-contract:") and effect != "block:unsafe-release" for effect in self.effect_receipts):
            raise ResearchContractError("knowledge contract effect is outside local read gate")

    def digest(self) -> str:
        self.validate()
        return research_artifact_digest({"schema_version": self.schema_version, "contract_version": self.contract_version, "feature_id": self.feature_id, "request_id": self.request_id, "study_id": self.study_id, "disposition": self.disposition, "input_schema": self.input_schema, "output_schema": self.output_schema, "source_revision": self.source_revision, "target_revision": self.target_revision, "candidate_order": list(self.candidate_order), "admitted_order": list(self.admitted_order), "unresolved_order": list(self.unresolved_order), "denied_order": list(self.denied_order), "missing_order": list(self.missing_order), "semantic_loss_order": list(self.semantic_loss_order), "contract_digest": self.contract_digest, "migration_digest": self.migration_digest, "replay_identity": self.replay_identity, "omissions": list(self.omissions), "uncertainty": list(self.uncertainty), "negative_evidence": list(self.negative_evidence), "effect_receipts": list(self.effect_receipts), "artifact": dict(self.artifact), "raw_data_local": self.raw_data_local, "boundary": self.boundary})


def model_local_knowledge_representation_contract(*, request_id: str, study_id: str, claims: Sequence[KnowledgeContractClaim], required_claim_ids: Sequence[str], input_schema: str = "ScopedResearchClaims1@1", output_schema: str = "TypedKnowledgeWorld1@1", source_revision: int = 1, target_revision: int = 1, migration_requested: bool = False, replay_identity: str, policy_allow: bool = True, protected_closure: bool = True, raw_data_local: bool = True) -> KnowledgeContractReceipt:
    if not request_id.strip() or not study_id.strip() or not claims or input_schema != "ScopedResearchClaims1@1" or output_schema != "TypedKnowledgeWorld1@1" or source_revision <= 0 or target_revision < source_revision or (target_revision > source_revision and not migration_requested) or not re.fullmatch(r"[0-9a-f]{64}", replay_identity) or not raw_data_local:
        raise ResearchContractError("knowledge contract identity, schemas, revisions, migration, replay, or locality is invalid")
    ordered = tuple(sorted(claim.claim_id for claim in claims))
    if len(set(ordered)) != len(claims) or any(not value.strip() for value in ordered):
        raise ResearchContractError("knowledge contract claim identifiers must be unique and non-empty")
    claim_map = {claim.claim_id: claim for claim in claims}
    admitted: set[str] = set(); unresolved: set[str] = set(); denied: set[str] = set(); missing: set[str] = set(); semantic_loss: set[str] = set(); omissions: set[str] = set(); uncertainty = {"gate:schema-compatibility", "gate:unknown-is-not-asserted", "gate:locality"}; negative: set[str] = set()
    for claim_id in ordered:
        claim = claim_map[claim_id]
        if not policy_allow or not protected_closure or claim.study_id != study_id or claim.boundary != PRECLINICAL_BOUNDARY:
            denied.add(claim_id); negative.add(f"claim:{claim_id}:scope-policy-closure")
        elif claim.evidence_digest is None or claim.provenance_digest is None:
            unresolved.add(claim_id); missing.add(claim_id); omissions.add(f"claim:{claim_id}:evidence-or-provenance-missing")
        elif claim.state in {"unknown", "speculative"}:
            unresolved.add(claim_id); uncertainty.add(f"claim:{claim_id}:unknown-not-asserted")
        elif claim.state == "contradicted":
            denied.add(claim_id); negative.add(f"claim:{claim_id}:contradicted")
        else:
            admitted.add(claim_id)
            if target_revision > source_revision: semantic_loss.add(claim_id)
    for required_id in sorted(set(required_claim_ids)):
        if required_id not in claim_map:
            omissions.add(f"claim:{required_id}:required-missing"); uncertainty.add(f"claim:{required_id}:required-unresolved")
        elif required_id not in admitted:
            uncertainty.add(f"claim:{required_id}:required-not-admitted")
    if target_revision > source_revision: uncertainty.add(f"migration:{source_revision}-to-{target_revision}")
    if not policy_allow: omissions.add("control:policy-denied")
    if not protected_closure: omissions.add("control:protected-closure-incomplete")
    disposition = "blocked" if not policy_allow or not protected_closure or not raw_data_local else "partial" if not admitted or unresolved or denied or missing else "migrated" if target_revision > source_revision else "compatible"
    contract_digest = research_artifact_digest({"study_id": study_id, "input_schema": input_schema, "output_schema": output_schema, "source_revision": source_revision, "target_revision": target_revision, "candidate_order": list(ordered), "admitted_order": sorted(admitted), "unresolved_order": sorted(unresolved), "denied_order": sorted(denied), "missing_order": sorted(missing), "semantic_loss_order": sorted(semantic_loss)})
    migration_digest = research_artifact_digest({"source_revision": source_revision, "target_revision": target_revision, "migration_requested": migration_requested, "semantic_loss_order": sorted(semantic_loss)})
    artifact = {"content_hash": research_artifact_digest({"request_id": request_id, "contract_digest": contract_digest}), "media_type": "application/vnd.aurora.typed-knowledge-world+json"}
    receipt = KnowledgeContractReceipt(request_id=request_id, study_id=study_id, disposition=disposition, input_schema=input_schema, output_schema=output_schema, source_revision=source_revision, target_revision=target_revision, candidate_order=ordered, admitted_order=tuple(sorted(admitted)), unresolved_order=tuple(sorted(unresolved)), denied_order=tuple(sorted(denied)), missing_order=tuple(sorted(missing)), semantic_loss_order=tuple(sorted(semantic_loss)), contract_digest=contract_digest, migration_digest=migration_digest, replay_identity=replay_identity, omissions=tuple(sorted(omissions)), uncertainty=tuple(sorted(uncertainty)), negative_evidence=tuple(sorted(negative)), effect_receipts=(f"read:local-knowledge-contract:{request_id}",) if disposition != "blocked" else ("block:unsafe-release",), artifact=artifact, raw_data_local=raw_data_local)
    receipt.validate()
    return receipt
