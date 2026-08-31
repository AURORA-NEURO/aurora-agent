"""Multimodal evidence-surveillance contract model parity surface."""
from __future__ import annotations

from dataclasses import dataclass
import re
from typing import Any, Mapping, Sequence

from .research_contracts import (
    ADAPTER_MULTIMODAL_EVIDENCE_SURVEILLANCE_CONTRACT_MODEL_CONTRACT_VERSION,
    ADAPTER_MULTIMODAL_EVIDENCE_SURVEILLANCE_CONTRACT_MODEL_FEATURE_ID,
    PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
    ResearchContractError,
    research_artifact_digest,
)


@dataclass(frozen=True)
class MultimodalContractClaim:
    claim_id: str
    study_id: str
    modality: str
    semantic_profile: str
    value_digest: str
    evidence_state: str = "supported"
    omitted: bool = False
    negative_result: bool = False


@dataclass(frozen=True)
class MultimodalEvidenceSurveillanceContractReceipt:
    request_id: str
    input_schema: str
    output_schema: str
    semantic_profile: str
    compatibility: str
    disposition: str
    study_order: tuple[str, ...]
    modality_order: tuple[str, ...]
    candidate_order: tuple[str, ...]
    retained_order: tuple[str, ...]
    unknown_order: tuple[str, ...]
    denied_order: tuple[str, ...]
    incomparable_order: tuple[str, ...]
    migration_order: tuple[str, ...]
    semantic_loss: tuple[str, ...]
    comparability_digest: str
    contract_digest: str
    canonical_digest: str
    provenance_digest: str
    replay_identity: str
    effect_receipts: tuple[str, ...]
    artifact: Mapping[str, Any]
    feature_id: str = ADAPTER_MULTIMODAL_EVIDENCE_SURVEILLANCE_CONTRACT_MODEL_FEATURE_ID
    contract_version: str = ADAPTER_MULTIMODAL_EVIDENCE_SURVEILLANCE_CONTRACT_MODEL_CONTRACT_VERSION
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    raw_data_local: bool = True
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION or self.contract_version != ADAPTER_MULTIMODAL_EVIDENCE_SURVEILLANCE_CONTRACT_MODEL_CONTRACT_VERSION or self.feature_id != ADAPTER_MULTIMODAL_EVIDENCE_SURVEILLANCE_CONTRACT_MODEL_FEATURE_ID:
            raise ResearchContractError("multimodal contract schema, feature, or version mismatch")
        if self.boundary != PRECLINICAL_BOUNDARY or not self.raw_data_local or not self.request_id.strip() or self.input_schema != "EvidenceFeed2@1" or self.output_schema != "QualifiedEvidenceSet2@1" or not self.semantic_profile.strip() or len(self.study_order) < 2 or len(self.modality_order) < 2 or not self.candidate_order or not self.effect_receipts:
            raise ResearchContractError("multimodal contract identity, schemas, closure, locality, candidates, or effects are incomplete")
        for values in (self.study_order, self.modality_order, self.candidate_order, self.retained_order, self.unknown_order, self.denied_order, self.incomparable_order, self.migration_order, self.semantic_loss, self.effect_receipts):
            if tuple(sorted(set(values))) != values:
                raise ResearchContractError("multimodal contract ordering is not canonical")
        if set(self.retained_order) | set(self.unknown_order) | set(self.denied_order) != set(self.candidate_order):
            raise ResearchContractError("multimodal contract states do not partition candidates")
        for value in (self.comparability_digest, self.contract_digest, self.canonical_digest, self.provenance_digest, self.replay_identity, self.artifact.get("content_hash")):
            if not isinstance(value, str) or not re.fullmatch(r"[0-9a-f]{64}", value):
                raise ResearchContractError("multimodal contract digest is invalid")
        if any(not effect.startswith("read:local-multimodal-contract:") and effect != "block:unsafe-release" for effect in self.effect_receipts):
            raise ResearchContractError("multimodal contract effect is outside local-read gate")
        if self.disposition == "blocked" and self.effect_receipts != ("block:unsafe-release",):
            raise ResearchContractError("blocked multimodal contract must be explicitly blocked")

    def digest(self) -> str:
        self.validate()
        return research_artifact_digest({"schema_version": self.schema_version, "contract_version": self.contract_version, "feature_id": self.feature_id, "request_id": self.request_id, "input_schema": self.input_schema, "output_schema": self.output_schema, "semantic_profile": self.semantic_profile, "compatibility": self.compatibility, "disposition": self.disposition, "study_order": list(self.study_order), "modality_order": list(self.modality_order), "candidate_order": list(self.candidate_order), "retained_order": list(self.retained_order), "unknown_order": list(self.unknown_order), "denied_order": list(self.denied_order), "incomparable_order": list(self.incomparable_order), "migration_order": list(self.migration_order), "semantic_loss": list(self.semantic_loss), "comparability_digest": self.comparability_digest, "contract_digest": self.contract_digest, "canonical_digest": self.canonical_digest, "provenance_digest": self.provenance_digest, "replay_identity": self.replay_identity, "effect_receipts": list(self.effect_receipts), "artifact": dict(self.artifact), "raw_data_local": self.raw_data_local, "boundary": self.boundary})


def model_multimodal_evidence_surveillance_contract(*, request_id: str, input_schema: str, output_schema: str, semantic_profile: str, required_studies: Sequence[str], required_modalities: Sequence[str], claims: Sequence[MultimodalContractClaim], policy_allow: bool = True, protected_closure: bool = True, raw_data_local: bool = True, replay_identity: str) -> MultimodalEvidenceSurveillanceContractReceipt:
    if not request_id.strip() or not input_schema.strip() or not output_schema.strip() or not semantic_profile.strip() or len(set(required_studies)) < 2 or len(set(required_modalities)) < 2 or not claims or not re.fullmatch(r"[0-9a-f]{64}", replay_identity) or not raw_data_local:
        raise ResearchContractError("multimodal contract identity, schemas, semantic profile, closure, claims, replay, or locality is invalid")
    studies = tuple(sorted(set(required_studies))); modalities = tuple(sorted(set(required_modalities))); ordered = tuple(sorted(claims, key=lambda claim: claim.claim_id)); claim_ids = tuple(claim.claim_id for claim in ordered)
    if len(set(claim_ids)) != len(claim_ids) or any(not value.strip() for value in claim_ids):
        raise ResearchContractError("multimodal claim identities must be unique and non-empty")
    compatibility = "additive_migration" if input_schema == "EvidenceFeed2@1" and output_schema == "QualifiedEvidenceSet2@1" else "compatible" if input_schema == output_schema else "breaking"
    required_cells = {f"{study}::{modality}::required" for study in studies for modality in modalities}; candidate = tuple(sorted(set(claim_ids) | required_cells)); retained: set[str] = set(); unknown: set[str] = set(); denied: set[str] = set(); incomparable: set[str] = set(); migration: set[str] = set(); loss: set[str] = set(); covered: set[str] = set()
    for claim in ordered:
        if compatibility == "breaking":
            denied.add(claim.claim_id); loss.add(f"claim:{claim.claim_id}:breaking-schema")
        elif not claim.study_id.strip() or not claim.modality.strip() or claim.study_id not in studies or claim.modality not in modalities:
            denied.add(claim.claim_id); loss.add(f"claim:{claim.claim_id}:scope-mismatch")
        elif claim.semantic_profile != semantic_profile:
            denied.add(claim.claim_id); incomparable.add(claim.claim_id); loss.add(f"claim:{claim.claim_id}:semantic-profile-incomparable")
        elif claim.omitted or claim.evidence_state in {"unknown", "speculative"}:
            unknown.add(claim.claim_id); loss.add(f"claim:{claim.claim_id}:unknown-not-asserted")
        elif claim.evidence_state == "contradicted":
            denied.add(claim.claim_id); loss.add(f"claim:{claim.claim_id}:contradicted-retained")
        else:
            retained.add(claim.claim_id); covered.add(f"{claim.study_id}::{claim.modality}::required")
            if compatibility == "additive_migration": migration.add(f"claim:{claim.claim_id}:study-modality-preserved")
            if claim.negative_result: loss.add(f"claim:{claim.claim_id}:negative-result-retained")
    for cell in required_cells:
        if cell not in covered: unknown.add(cell); loss.add(f"cell:{cell}:comparability-incomplete")
        else: retained.add(cell)
    if not policy_allow: loss.add("control:policy-denied")
    if not protected_closure: loss.add("control:protected-closure-incomplete")
    disposition = "blocked" if not policy_allow or not protected_closure or not raw_data_local else "unknown" if not retained else "partial" if unknown or denied else "compatible"
    retained_order = tuple(sorted(retained)); unknown_order = tuple(sorted(unknown)); denied_order = tuple(sorted(denied)); incomparable_order = tuple(sorted(incomparable)); migration_order = tuple(sorted(migration)); loss_order = tuple(sorted(loss)); study_order = tuple(studies); modality_order = tuple(modalities)
    comparability_digest = research_artifact_digest({"study_order": list(study_order), "modality_order": list(modality_order), "semantic_profile": semantic_profile, "covered_cells": sorted(covered)})
    contract_digest = research_artifact_digest({"input_schema": input_schema, "output_schema": output_schema, "compatibility": compatibility, "candidate_order": list(candidate)})
    canonical_digest = research_artifact_digest({"retained_order": list(retained_order), "unknown_order": list(unknown_order), "denied_order": list(denied_order), "incomparable_order": list(incomparable_order), "migration_order": list(migration_order), "semantic_loss": list(loss_order)})
    provenance_digest = research_artifact_digest({"request_id": request_id, "replay_identity": replay_identity, "comparability_digest": comparability_digest, "contract_digest": contract_digest})
    payload = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": ADAPTER_MULTIMODAL_EVIDENCE_SURVEILLANCE_CONTRACT_MODEL_CONTRACT_VERSION, "feature_id": ADAPTER_MULTIMODAL_EVIDENCE_SURVEILLANCE_CONTRACT_MODEL_FEATURE_ID, "request_id": request_id, "input_schema": input_schema, "output_schema": output_schema, "semantic_profile": semantic_profile, "compatibility": compatibility, "disposition": disposition, "study_order": list(study_order), "modality_order": list(modality_order), "candidate_order": list(candidate), "retained_order": list(retained_order), "unknown_order": list(unknown_order), "denied_order": list(denied_order), "incomparable_order": list(incomparable_order), "migration_order": list(migration_order), "semantic_loss": list(loss_order), "comparability_digest": comparability_digest, "contract_digest": contract_digest, "canonical_digest": canonical_digest, "provenance_digest": provenance_digest, "replay_identity": replay_identity, "raw_data_local": raw_data_local, "boundary": PRECLINICAL_BOUNDARY}
    artifact = {"content_hash": research_artifact_digest(payload), "media_type": "application/vnd.aurora.qualified-multimodal-evidence-set+json"}
    receipt = MultimodalEvidenceSurveillanceContractReceipt(request_id=request_id, input_schema="EvidenceFeed2@1", output_schema="QualifiedEvidenceSet2@1", semantic_profile=semantic_profile, compatibility=compatibility, disposition=disposition, study_order=study_order, modality_order=modality_order, candidate_order=candidate, retained_order=retained_order, unknown_order=unknown_order, denied_order=denied_order, incomparable_order=incomparable_order, migration_order=migration_order, semantic_loss=loss_order, comparability_digest=comparability_digest, contract_digest=contract_digest, canonical_digest=canonical_digest, provenance_digest=provenance_digest, replay_identity=replay_identity, effect_receipts=(f"read:local-multimodal-contract:{request_id}",) if disposition != "blocked" else ("block:unsafe-release",), artifact=artifact, raw_data_local=raw_data_local)
    receipt.validate(); return receipt
