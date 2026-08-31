"""Python mirror of the section federated interpretation assurance receipt."""

from __future__ import annotations

from dataclasses import dataclass
import re
from typing import Any, Mapping

from .research_contracts import (
    PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
    ResearchContractError,
    SECTION_INTERPRETATION_ASSURANCE_CONTRACT_VERSION,
    SECTION_INTERPRETATION_ASSURANCE_FEATURE_ID,
    research_artifact_digest,
)


@dataclass(frozen=True)
class SectionInterpretationAssuranceReceipt:
    """Cross-language validator for local-only, evidence-backed interpretation release."""

    request_id: str
    workflow_id: str
    federation_id: str
    scope: str
    disposition: str
    candidate_order: tuple[str, ...]
    admitted_order: tuple[str, ...]
    blocked_order: tuple[str, ...]
    unknown_order: tuple[str, ...]
    result_order: tuple[str, ...]
    visualization_order: tuple[str, ...]
    study_order: tuple[str, ...]
    modality_order: tuple[str, ...]
    support_order: tuple[int, ...]
    semantic_order: tuple[str, ...]
    artifact_order: tuple[str, ...]
    evidence_order: tuple[str, ...]
    provenance_order: tuple[str, ...]
    comparability_order: tuple[str, ...]
    omissions: tuple[str, ...]
    uncertainty: tuple[str, ...]
    negative_evidence: tuple[str, ...]
    replay_identity: str
    benchmark_digest: str | None
    effect_receipts: tuple[str, ...]
    interpretations: tuple[Mapping[str, Any], ...]
    artifact: Mapping[str, Any]
    feature_id: str = SECTION_INTERPRETATION_ASSURANCE_FEATURE_ID
    contract_version: str = SECTION_INTERPRETATION_ASSURANCE_CONTRACT_VERSION
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    raw_data_local: bool = True
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION or self.feature_id != SECTION_INTERPRETATION_ASSURANCE_FEATURE_ID or self.contract_version != SECTION_INTERPRETATION_ASSURANCE_CONTRACT_VERSION:
            raise ResearchContractError("section interpretation schema, feature, or version mismatch")
        if self.boundary != PRECLINICAL_BOUNDARY or not self.raw_data_local or not self.request_id.strip() or not self.workflow_id.strip() or not self.federation_id.strip() or not self.scope.strip() or not self.candidate_order or len(self.support_order) != len(self.candidate_order) or not self.effect_receipts:
            raise ResearchContractError("section interpretation identity, ranking, locality, support, or effects are incomplete")
        if self.disposition not in {"qualified", "partial", "unknown", "blocked"}:
            raise ResearchContractError("section interpretation disposition is unknown")
        if any(value not in self.candidate_order for value in (*self.admitted_order, *self.blocked_order, *self.unknown_order)):
            raise ResearchContractError("section interpretation state is not covered by candidate order")
        for values in (self.candidate_order, self.admitted_order, self.blocked_order, self.unknown_order, self.result_order, self.visualization_order, self.study_order, self.modality_order, self.omissions, self.uncertainty, self.negative_evidence, self.effect_receipts):
            if tuple(sorted(set(values))) != values:
                raise ResearchContractError("section interpretation ordering is invalid")
        for values in (self.semantic_order, self.artifact_order, self.evidence_order, self.provenance_order, self.comparability_order):
            if tuple(sorted(set(values))) != values:
                raise ResearchContractError("section interpretation digest ordering is invalid")
        digests = (*self.semantic_order, *self.artifact_order, *self.evidence_order, *self.provenance_order, *self.comparability_order, self.replay_identity, self.artifact.get("content_hash"))
        if self.benchmark_digest is not None:
            digests += (self.benchmark_digest,)
        if any(not isinstance(value, str) or not re.fullmatch(r"[0-9a-f]{64}", value) for value in digests):
            raise ResearchContractError("section interpretation digest is invalid")
        for interpretation in self.interpretations:
            if interpretation.get("raw_data_local") is not True or interpretation.get("boundary") != PRECLINICAL_BOUNDARY or not interpretation.get("comparability_digest"):
                raise ResearchContractError("interactive interpretation is incomplete or non-local")
        if self.admitted_order and not any(effect.startswith("evaluate:interpretation-assurance:") for effect in self.effect_receipts):
            raise ResearchContractError("admitted interpretations require an assurance effect")
        if not self.admitted_order and self.effect_receipts != ("block:unsafe-release",):
            raise ResearchContractError("empty interpretation result must be explicitly blocked")

    def digest(self) -> str:
        self.validate()
        return research_artifact_digest({
            "schema_version": self.schema_version,
            "contract_version": self.contract_version,
            "feature_id": self.feature_id,
            "request_id": self.request_id,
            "workflow_id": self.workflow_id,
            "federation_id": self.federation_id,
            "scope": self.scope,
            "disposition": self.disposition,
            "candidate_order": list(self.candidate_order),
            "admitted_order": list(self.admitted_order),
            "blocked_order": list(self.blocked_order),
            "unknown_order": list(self.unknown_order),
            "result_order": list(self.result_order),
            "visualization_order": list(self.visualization_order),
            "study_order": list(self.study_order),
            "modality_order": list(self.modality_order),
            "support_order": list(self.support_order),
            "semantic_order": list(self.semantic_order),
            "artifact_order": list(self.artifact_order),
            "evidence_order": list(self.evidence_order),
            "provenance_order": list(self.provenance_order),
            "comparability_order": list(self.comparability_order),
            "omissions": list(self.omissions),
            "uncertainty": list(self.uncertainty),
            "negative_evidence": list(self.negative_evidence),
            "replay_identity": self.replay_identity,
            "benchmark_digest": self.benchmark_digest,
            "effect_receipts": list(self.effect_receipts),
            "interpretations": [dict(item) for item in self.interpretations],
            "artifact": dict(self.artifact),
            "raw_data_local": self.raw_data_local,
            "boundary": self.boundary,
        })
