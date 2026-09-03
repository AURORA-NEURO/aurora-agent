"""Python parity contract for multimodal typed context compilation."""
from __future__ import annotations

from dataclasses import dataclass
import re
from typing import Any, Mapping

from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError, research_artifact_digest

MULTIMODAL_CONTEXT_COMPILATION_FEATURE_ID = "AFA-brain-P03-F02"
MULTIMODAL_CONTEXT_COMPILATION_CONTRACT_VERSION = "brain-multimodal-context-compilation/1.0"


@dataclass(frozen=True)
class BrainMultimodalContextCompilationReceipt:
    request_id: str
    objective: str
    scope: str
    study_order: tuple[str, ...]
    modality_order: tuple[str, ...]
    disposition: str
    required_fact_order: tuple[str, ...]
    resolved_fact_order: tuple[str, ...]
    missing_fact_order: tuple[str, ...]
    blocked_fact_order: tuple[str, ...]
    unknown_fact_order: tuple[str, ...]
    comparability_digest: str
    context_digest: str
    replay_identity: str
    omissions: tuple[str, ...]
    uncertainty: tuple[str, ...]
    negative_evidence: tuple[str, ...]
    effect_receipts: tuple[str, ...]
    artifact: Mapping[str, Any]
    feature_id: str = MULTIMODAL_CONTEXT_COMPILATION_FEATURE_ID
    contract_version: str = MULTIMODAL_CONTEXT_COMPILATION_CONTRACT_VERSION
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    raw_data_local: bool = True
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION or self.feature_id != MULTIMODAL_CONTEXT_COMPILATION_FEATURE_ID or self.contract_version != MULTIMODAL_CONTEXT_COMPILATION_CONTRACT_VERSION:
            raise ResearchContractError("multimodal context compilation schema, feature, or version mismatch")
        if self.boundary != PRECLINICAL_BOUNDARY or not self.raw_data_local or not self.request_id.strip() or not self.objective.strip() or not self.scope.strip() or len(self.study_order) < 2 or len(self.modality_order) < 2 or not self.required_fact_order or not self.effect_receipts or self.disposition not in {"qualified", "partial", "unknown", "blocked"}:
            raise ResearchContractError("multimodal context identity, closure, disposition, locality, or effects are incomplete")
        for values in (self.study_order, self.modality_order, self.required_fact_order, self.resolved_fact_order, self.missing_fact_order, self.blocked_fact_order, self.unknown_fact_order, self.omissions, self.uncertainty, self.negative_evidence, self.effect_receipts):
            if tuple(sorted(set(values))) != values:
                raise ResearchContractError("multimodal context vectors are not canonical")
        required = set(self.required_fact_order)
        classified = set(self.resolved_fact_order) | set(self.missing_fact_order) | set(self.blocked_fact_order) | set(self.unknown_fact_order)
        if classified != required or sum(len(set(values)) for values in (self.resolved_fact_order, self.missing_fact_order, self.blocked_fact_order, self.unknown_fact_order)) != len(required):
            raise ResearchContractError("multimodal context fact states do not partition required facts")
        for value in (self.comparability_digest, self.context_digest, self.replay_identity, self.artifact.get("content_hash")):
            if not isinstance(value, str) or not re.fullmatch(r"[0-9a-f]{64}", value):
                raise ResearchContractError("multimodal context digest is invalid")
        if any(not effect.startswith("compile:local-multimodal-research-context:") and effect != "block:unsafe-release" for effect in self.effect_receipts):
            raise ResearchContractError("multimodal context effect is outside local compilation gate")

    def digest(self) -> str:
        self.validate()
        return research_artifact_digest({"schema_version": self.schema_version, "contract_version": self.contract_version, "feature_id": self.feature_id, "request_id": self.request_id, "objective": self.objective, "scope": self.scope, "study_order": list(self.study_order), "modality_order": list(self.modality_order), "disposition": self.disposition, "required_fact_order": list(self.required_fact_order), "resolved_fact_order": list(self.resolved_fact_order), "missing_fact_order": list(self.missing_fact_order), "blocked_fact_order": list(self.blocked_fact_order), "unknown_fact_order": list(self.unknown_fact_order), "comparability_digest": self.comparability_digest, "context_digest": self.context_digest, "replay_identity": self.replay_identity, "omissions": list(self.omissions), "uncertainty": list(self.uncertainty), "negative_evidence": list(self.negative_evidence), "effect_receipts": list(self.effect_receipts), "artifact": dict(self.artifact), "raw_data_local": self.raw_data_local, "boundary": self.boundary})
