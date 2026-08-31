"""Python mirror of the multimodal evidence contract-model receipt."""

from __future__ import annotations

from dataclasses import dataclass
import re
from typing import Any, Mapping

from .research_contracts import (
    MULTIMODAL_CONTRACT_MODEL_CONTRACT_VERSION,
    MULTIMODAL_CONTRACT_MODEL_FEATURE_ID,
    PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
    ResearchContractError,
    research_artifact_digest,
)


@dataclass(frozen=True)
class BrainMultimodalContractModelReceipt:
    request_id: str
    study_order: tuple[str, ...]
    scope: str
    comparability_profile: str
    disposition: str
    compatibility: str
    input_schema: str
    output_schema: str
    modality_order: tuple[str, ...]
    binding_order: tuple[str, ...]
    missing_order: tuple[str, ...]
    semantic_disagreement_order: tuple[str, ...]
    schema_order: tuple[str, ...]
    unit_order: tuple[str, ...]
    coordinate_order: tuple[str, ...]
    semantic_order: tuple[str, ...]
    artifact_order: tuple[str, ...]
    provenance_order: tuple[str, ...]
    contract_digest: str
    replay_identity: str
    omissions: tuple[str, ...]
    uncertainty: tuple[str, ...]
    negative_evidence: tuple[str, ...]
    effect_receipts: tuple[str, ...]
    artifact: Mapping[str, Any]
    feature_id: str = MULTIMODAL_CONTRACT_MODEL_FEATURE_ID
    contract_version: str = MULTIMODAL_CONTRACT_MODEL_CONTRACT_VERSION
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    raw_data_local: bool = True
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION or self.feature_id != MULTIMODAL_CONTRACT_MODEL_FEATURE_ID or self.contract_version != MULTIMODAL_CONTRACT_MODEL_CONTRACT_VERSION:
            raise ResearchContractError("multimodal contract schema, feature, or version mismatch")
        if self.boundary != PRECLINICAL_BOUNDARY or not self.raw_data_local or not self.request_id.strip() or len(self.study_order) < 2 or not self.scope.strip() or not self.comparability_profile.strip() or self.input_schema != "EvidenceFeed2@1" or self.output_schema != "QualifiedEvidenceSet2@1" or len(self.modality_order) < 2 or not self.binding_order or not self.effect_receipts:
            raise ResearchContractError("multimodal identity, schemas, study/modality closure, locality, or effects are incomplete")
        if self.disposition not in {"qualified", "partial", "unknown", "blocked"} or self.compatibility not in {"additive", "migration_required", "breaking", "unknown"}:
            raise ResearchContractError("multimodal disposition or compatibility is unknown")
        for values in (self.study_order, self.modality_order, self.binding_order, self.missing_order, self.semantic_disagreement_order, self.schema_order, self.unit_order, self.coordinate_order, self.omissions, self.uncertainty, self.negative_evidence, self.effect_receipts):
            if tuple(sorted(set(values))) != values:
                raise ResearchContractError("multimodal contract ordering is invalid")
        for values in (self.semantic_order, self.artifact_order, self.provenance_order):
            if tuple(sorted(set(values))) != values:
                raise ResearchContractError("multimodal contract digest ordering is invalid")
        for value in (*self.semantic_order, *self.artifact_order, *self.provenance_order, self.contract_digest, self.replay_identity, self.artifact.get("content_hash")):
            if not isinstance(value, str) or not re.fullmatch(r"[0-9a-f]{64}", value):
                raise ResearchContractError("multimodal contract digest is invalid")
        if self.disposition == "qualified" and any(not effect.startswith("read:local-research-artifacts:") for effect in self.effect_receipts):
            raise ResearchContractError("qualified multimodal contract requires a local-read receipt")
        if self.disposition != "qualified" and self.effect_receipts != ("block:unsafe-release",):
            raise ResearchContractError("non-qualified multimodal contract must be explicitly blocked")

    def digest(self) -> str:
        self.validate()
        return research_artifact_digest({
            "schema_version": self.schema_version, "contract_version": self.contract_version, "feature_id": self.feature_id,
            "request_id": self.request_id, "study_order": list(self.study_order), "scope": self.scope, "comparability_profile": self.comparability_profile,
            "disposition": self.disposition, "compatibility": self.compatibility, "input_schema": self.input_schema, "output_schema": self.output_schema,
            "modality_order": list(self.modality_order), "binding_order": list(self.binding_order), "missing_order": list(self.missing_order),
            "semantic_disagreement_order": list(self.semantic_disagreement_order), "schema_order": list(self.schema_order), "unit_order": list(self.unit_order),
            "coordinate_order": list(self.coordinate_order), "semantic_order": list(self.semantic_order), "artifact_order": list(self.artifact_order),
            "provenance_order": list(self.provenance_order), "contract_digest": self.contract_digest, "replay_identity": self.replay_identity,
            "omissions": list(self.omissions), "uncertainty": list(self.uncertainty), "negative_evidence": list(self.negative_evidence),
            "effect_receipts": list(self.effect_receipts), "artifact": dict(self.artifact), "raw_data_local": self.raw_data_local, "boundary": self.boundary,
        })
