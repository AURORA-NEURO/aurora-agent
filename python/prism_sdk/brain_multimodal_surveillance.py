"""Python mirror of the multimodal brain evidence-surveillance receipt."""

from __future__ import annotations

from dataclasses import dataclass
import re
from typing import Any, Mapping

from .research_contracts import (
    BRAIN_MULTIMODAL_EVIDENCE_SURVEILLANCE_CONTRACT_VERSION,
    BRAIN_MULTIMODAL_EVIDENCE_SURVEILLANCE_FEATURE_ID,
    PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
    ResearchContractError,
    research_artifact_digest,
)


@dataclass(frozen=True)
class BrainMultimodalEvidenceSurveillanceReceipt:
    request_id: str
    study_order: tuple[str, ...]
    scope: str
    disposition: str
    candidate_order: tuple[str, ...]
    qualified_order: tuple[str, ...]
    blocked_order: tuple[str, ...]
    unknown_order: tuple[str, ...]
    source_order: tuple[str, ...]
    modality_order: tuple[str, ...]
    relevance_order: tuple[int, ...]
    semantic_order: tuple[str, ...]
    artifact_order: tuple[str, ...]
    provenance_order: tuple[str, ...]
    omissions: tuple[str, ...]
    uncertainty: tuple[str, ...]
    negative_evidence: tuple[str, ...]
    replay_identity: str
    effect_receipts: tuple[str, ...]
    artifact: Mapping[str, Any]
    feature_id: str = BRAIN_MULTIMODAL_EVIDENCE_SURVEILLANCE_FEATURE_ID
    contract_version: str = BRAIN_MULTIMODAL_EVIDENCE_SURVEILLANCE_CONTRACT_VERSION
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    raw_data_local: bool = True
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION or self.feature_id != BRAIN_MULTIMODAL_EVIDENCE_SURVEILLANCE_FEATURE_ID or self.contract_version != BRAIN_MULTIMODAL_EVIDENCE_SURVEILLANCE_CONTRACT_VERSION:
            raise ResearchContractError("multimodal brain surveillance schema, feature, or version mismatch")
        if self.boundary != PRECLINICAL_BOUNDARY or not self.raw_data_local or not self.request_id.strip() or not self.scope.strip() or len(self.study_order) < 2 or not self.candidate_order or len(self.relevance_order) != len(self.candidate_order) or not self.effect_receipts:
            raise ResearchContractError("multimodal identity, study coverage, locality, ranking, or effects are incomplete")
        if self.disposition not in {"qualified", "partial", "unknown", "blocked"}:
            raise ResearchContractError("multimodal brain surveillance disposition is unknown")
        if any(value not in self.candidate_order for value in (*self.qualified_order, *self.blocked_order, *self.unknown_order)):
            raise ResearchContractError("multimodal state is not covered by candidate order")
        for values in (self.study_order, self.candidate_order, self.qualified_order, self.blocked_order, self.unknown_order, self.source_order, self.modality_order, self.omissions, self.uncertainty, self.negative_evidence, self.effect_receipts):
            if tuple(sorted(set(values))) != values:
                raise ResearchContractError("multimodal ordering is invalid")
        for values in (self.semantic_order, self.artifact_order, self.provenance_order):
            if tuple(sorted(set(values))) != values:
                raise ResearchContractError("multimodal digest ordering is invalid")
        digests = (*self.semantic_order, *self.artifact_order, *self.provenance_order, self.replay_identity, self.artifact.get("content_hash"))
        if any(not isinstance(value, str) or not re.fullmatch(r"[0-9a-f]{64}", value) for value in digests):
            raise ResearchContractError("multimodal brain surveillance digest is invalid")
        if self.qualified_order and any(not effect.startswith("read:local-research-artifacts:") for effect in self.effect_receipts):
            raise ResearchContractError("qualified multimodal evidence requires a local-read receipt")
        if not self.qualified_order and self.effect_receipts != ("block:unsafe-release",):
            raise ResearchContractError("empty multimodal evidence result must be explicitly blocked")

    def digest(self) -> str:
        self.validate()
        return research_artifact_digest({
            "schema_version": self.schema_version,
            "contract_version": self.contract_version,
            "feature_id": self.feature_id,
            "request_id": self.request_id,
            "study_order": list(self.study_order),
            "scope": self.scope,
            "disposition": self.disposition,
            "candidate_order": list(self.candidate_order),
            "qualified_order": list(self.qualified_order),
            "blocked_order": list(self.blocked_order),
            "unknown_order": list(self.unknown_order),
            "source_order": list(self.source_order),
            "modality_order": list(self.modality_order),
            "relevance_order": list(self.relevance_order),
            "semantic_order": list(self.semantic_order),
            "artifact_order": list(self.artifact_order),
            "provenance_order": list(self.provenance_order),
            "omissions": list(self.omissions),
            "uncertainty": list(self.uncertainty),
            "negative_evidence": list(self.negative_evidence),
            "replay_identity": self.replay_identity,
            "effect_receipts": list(self.effect_receipts),
            "artifact": dict(self.artifact),
            "raw_data_local": self.raw_data_local,
            "boundary": self.boundary,
        })
