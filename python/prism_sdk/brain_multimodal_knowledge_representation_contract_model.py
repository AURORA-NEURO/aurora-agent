"""Multimodal study×modality knowledge-representation contract parity surface."""
from __future__ import annotations

from dataclasses import dataclass
import re
from typing import Any, Mapping, Sequence

from .research_contracts import (
    MULTIMODAL_KNOWLEDGE_REPRESENTATION_CONTRACT_MODEL_CONTRACT_VERSION,
    MULTIMODAL_KNOWLEDGE_REPRESENTATION_CONTRACT_MODEL_FEATURE_ID,
    PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
    ResearchContractError,
    research_artifact_digest,
)

@dataclass(frozen=True)
class MultimodalKnowledgeContractCell:
    cell_id: str
    study_id: str
    modality: str
    claim_id: str
    semantic_profile: str
    evidence_digest: str | None
    provenance_digest: str | None
    state: str = "supported"
    boundary: str = PRECLINICAL_BOUNDARY

@dataclass(frozen=True)
class MultimodalKnowledgeContractReceipt:
    request_id: str
    workspace_id: str
    disposition: str
    input_schema: str
    output_schema: str
    source_revision: int
    target_revision: int
    study_order: tuple[str, ...]
    modality_order: tuple[str, ...]
    candidate_order: tuple[str, ...]
    admitted_order: tuple[str, ...]
    unresolved_order: tuple[str, ...]
    denied_order: tuple[str, ...]
    missing_order: tuple[str, ...]
    incomparable_order: tuple[str, ...]
    semantic_loss_order: tuple[str, ...]
    contract_digest: str
    comparability_digest: str
    migration_digest: str
    replay_identity: str
    omissions: tuple[str, ...]
    uncertainty: tuple[str, ...]
    negative_evidence: tuple[str, ...]
    effect_receipts: tuple[str, ...]
    artifact: Mapping[str, Any]
    feature_id: str = MULTIMODAL_KNOWLEDGE_REPRESENTATION_CONTRACT_MODEL_FEATURE_ID
    contract_version: str = MULTIMODAL_KNOWLEDGE_REPRESENTATION_CONTRACT_MODEL_CONTRACT_VERSION
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    raw_data_local: bool = True
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION or self.feature_id != MULTIMODAL_KNOWLEDGE_REPRESENTATION_CONTRACT_MODEL_FEATURE_ID or self.contract_version != MULTIMODAL_KNOWLEDGE_REPRESENTATION_CONTRACT_MODEL_CONTRACT_VERSION:
            raise ResearchContractError("multimodal knowledge contract schema, feature, or version mismatch")
        if self.boundary != PRECLINICAL_BOUNDARY or not self.raw_data_local or not self.request_id.strip() or not self.workspace_id.strip() or self.input_schema != "ScopedResearchClaims1@1" or self.output_schema != "TypedKnowledgeWorld1@1" or len(self.study_order) < 2 or len(self.modality_order) < 2 or self.source_revision <= 0 or self.target_revision < self.source_revision or not self.candidate_order or not self.effect_receipts:
            raise ResearchContractError("multimodal contract identity, study/modality coverage, schema, revision, locality, or effects are incomplete")
        for values in (self.study_order, self.modality_order, self.candidate_order, self.admitted_order, self.unresolved_order, self.denied_order, self.missing_order, self.incomparable_order, self.semantic_loss_order, self.omissions, self.uncertainty, self.negative_evidence, self.effect_receipts):
            if tuple(sorted(set(values))) != values:
                raise ResearchContractError("multimodal contract ordering is not canonical")
        classified = set(self.admitted_order) | set(self.unresolved_order) | set(self.denied_order)
        if classified != set(self.candidate_order) or any(value not in self.candidate_order for value in (*self.missing_order, *self.incomparable_order)):
            raise ResearchContractError("multimodal contract states do not partition cells")
        for value in (self.contract_digest, self.comparability_digest, self.migration_digest, self.replay_identity, self.artifact.get("content_hash")):
            if not isinstance(value, str) or not re.fullmatch(r"[0-9a-f]{64}", value):
                raise ResearchContractError("multimodal contract digest is invalid")
        if any(not effect.startswith("read:local-multimodal-knowledge-contract:") and effect != "block:unsafe-release" for effect in self.effect_receipts):
            raise ResearchContractError("multimodal contract effect is outside local read gate")

    def digest(self) -> str:
        self.validate()
        return research_artifact_digest({"schema_version": self.schema_version, "contract_version": self.contract_version, "feature_id": self.feature_id, "request_id": self.request_id, "workspace_id": self.workspace_id, "disposition": self.disposition, "input_schema": self.input_schema, "output_schema": self.output_schema, "source_revision": self.source_revision, "target_revision": self.target_revision, "study_order": list(self.study_order), "modality_order": list(self.modality_order), "candidate_order": list(self.candidate_order), "admitted_order": list(self.admitted_order), "unresolved_order": list(self.unresolved_order), "denied_order": list(self.denied_order), "missing_order": list(self.missing_order), "incomparable_order": list(self.incomparable_order), "semantic_loss_order": list(self.semantic_loss_order), "contract_digest": self.contract_digest, "comparability_digest": self.comparability_digest, "migration_digest": self.migration_digest, "replay_identity": self.replay_identity, "omissions": list(self.omissions), "uncertainty": list(self.uncertainty), "negative_evidence": list(self.negative_evidence), "effect_receipts": list(self.effect_receipts), "artifact": dict(self.artifact), "raw_data_local": self.raw_data_local, "boundary": self.boundary})

def model_multimodal_knowledge_representation_contract(*, request_id: str, workspace_id: str, study_order: Sequence[str], modality_order: Sequence[str], cells: Sequence[MultimodalKnowledgeContractCell], required_cell_ids: Sequence[str], input_schema: str = "ScopedResearchClaims1@1", output_schema: str = "TypedKnowledgeWorld1@1", source_revision: int = 1, target_revision: int = 1, migration_requested: bool = False, comparability_required: bool = True, replay_identity: str, policy_allow: bool = True, protected_closure: bool = True, raw_data_local: bool = True) -> MultimodalKnowledgeContractReceipt:
    if not request_id.strip() or not workspace_id.strip() or len(study_order) < 2 or len(modality_order) < 2 or not cells or input_schema != "ScopedResearchClaims1@1" or output_schema != "TypedKnowledgeWorld1@1" or source_revision <= 0 or target_revision < source_revision or (target_revision > source_revision and not migration_requested) or not re.fullmatch(r"[0-9a-f]{64}", replay_identity) or not raw_data_local:
        raise ResearchContractError("multimodal contract identity, schemas, coverage, revisions, migration, replay, locality, or boundary is invalid")
    studies = tuple(sorted(set(study_order))); modalities = tuple(sorted(set(modality_order))); ordered = tuple(sorted(cell.cell_id for cell in cells))
    if len(studies) != len(study_order) or len(modalities) != len(modality_order) or len(set(ordered)) != len(cells) or any(not value.strip() for value in ordered):
        raise ResearchContractError("study, modality, and cell identifiers must be unique and non-empty")
    cell_map = {cell.cell_id: cell for cell in cells}; profiles = tuple(sorted({cell.semantic_profile for cell in cells})); profile_conflict = comparability_required and len(profiles) > 1
    admitted: set[str] = set(); unresolved: set[str] = set(); denied: set[str] = set(); missing: set[str] = set(); incomparable: set[str] = set(); semantic_loss: set[str] = set(); omissions: set[str] = set(); uncertainty = {"gate:study-modality-coverage", "gate:schema-compatibility", "gate:unknown-is-not-asserted", "gate:locality"}; negative: set[str] = set()
    for cell_id in ordered:
        cell = cell_map[cell_id]
        if cell.study_id not in studies or cell.modality not in modalities or not policy_allow or not protected_closure or cell.boundary != PRECLINICAL_BOUNDARY:
            denied.add(cell_id); negative.add(f"cell:{cell_id}:scope-policy-closure")
        elif cell.evidence_digest is None or cell.provenance_digest is None:
            unresolved.add(cell_id); missing.add(cell_id); omissions.add(f"cell:{cell_id}:evidence-or-provenance-missing")
        elif cell.state in {"unknown", "speculative"}:
            unresolved.add(cell_id); uncertainty.add(f"cell:{cell_id}:unknown-not-asserted")
        elif cell.state == "contradicted":
            denied.add(cell_id); negative.add(f"cell:{cell_id}:contradicted")
        elif profile_conflict:
            unresolved.add(cell_id); incomparable.add(cell_id); uncertainty.add(f"cell:{cell_id}:semantic-profile-conflict")
        else:
            admitted.add(cell_id)
            if target_revision > source_revision: semantic_loss.add(cell_id)
    for required_id in sorted(set(required_cell_ids)):
        if required_id not in cell_map:
            omissions.add(f"cell:{required_id}:required-missing"); uncertainty.add(f"cell:{required_id}:required-unresolved")
        elif required_id not in admitted:
            uncertainty.add(f"cell:{required_id}:required-not-admitted")
    if target_revision > source_revision: uncertainty.add(f"migration:{source_revision}-to-{target_revision}")
    if profile_conflict: omissions.add("control:semantic-profile-incompatibility")
    if not policy_allow: omissions.add("control:policy-denied")
    if not protected_closure: omissions.add("control:protected-closure-incomplete")
    comparability_digest = research_artifact_digest({"study_order": list(studies), "modality_order": list(modalities), "semantic_profiles": list(profiles), "comparability_required": comparability_required})
    contract_digest = research_artifact_digest({"workspace_id": workspace_id, "candidate_order": list(ordered), "admitted_order": sorted(admitted), "unresolved_order": sorted(unresolved), "denied_order": sorted(denied), "incomparable_order": sorted(incomparable), "source_revision": source_revision, "target_revision": target_revision, "comparability_digest": comparability_digest})
    migration_digest = research_artifact_digest({"source_revision": source_revision, "target_revision": target_revision, "migration_requested": migration_requested, "semantic_loss_order": sorted(semantic_loss)})
    disposition = "blocked" if not policy_allow or not protected_closure or not raw_data_local else "partial" if not admitted or unresolved or denied or profile_conflict else "migrated" if target_revision > source_revision else "compatible"
    artifact = {"content_hash": research_artifact_digest({"request_id": request_id, "contract_digest": contract_digest}), "media_type": "application/vnd.aurora.typed-knowledge-world+json"}
    receipt = MultimodalKnowledgeContractReceipt(request_id=request_id, workspace_id=workspace_id, disposition=disposition, input_schema=input_schema, output_schema=output_schema, source_revision=source_revision, target_revision=target_revision, study_order=studies, modality_order=modalities, candidate_order=ordered, admitted_order=tuple(sorted(admitted)), unresolved_order=tuple(sorted(unresolved)), denied_order=tuple(sorted(denied)), missing_order=tuple(sorted(missing)), incomparable_order=tuple(sorted(incomparable)), semantic_loss_order=tuple(sorted(semantic_loss)), contract_digest=contract_digest, comparability_digest=comparability_digest, migration_digest=migration_digest, replay_identity=replay_identity, omissions=tuple(sorted(omissions)), uncertainty=tuple(sorted(uncertainty)), negative_evidence=tuple(sorted(negative)), effect_receipts=(f"read:local-multimodal-knowledge-contract:{request_id}",) if disposition != "blocked" else ("block:unsafe-release",), artifact=artifact, raw_data_local=raw_data_local)
    receipt.validate(); return receipt
