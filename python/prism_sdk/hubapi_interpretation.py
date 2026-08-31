"""Python mirror of the hubapi multimodal interpretation assurance receipt."""

from __future__ import annotations

from dataclasses import dataclass
import re
from typing import Any, Mapping

from .research_contracts import (
    PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
    ResearchContractError,
    HUBAPI_INTERPRETATION_ASSURANCE_FEATURE_ID,
    HUBAPI_INTERPRETATION_ASSURANCE_CONTRACT_VERSION,
    research_artifact_digest,
)


@dataclass(frozen=True)
class HubapiMultimodalInterpretationAssuranceReceipt:
    """Cross-language validator for hub-facing multimodal interpretation release."""

    request_id: str
    workflow_id: str
    objective_id: str
    scope: str
    disposition: str
    ranked_order: tuple[str, ...]
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
    baseline_order: tuple[str, ...]
    omissions: tuple[str, ...]
    uncertainty: tuple[str, ...]
    negative_evidence: tuple[str, ...]
    replay_identity: str
    benchmark_digest: str | None
    effect_receipts: tuple[str, ...]
    artifact: Mapping[str, Any]
    feature_id: str = HUBAPI_INTERPRETATION_ASSURANCE_FEATURE_ID
    contract_version: str = HUBAPI_INTERPRETATION_ASSURANCE_CONTRACT_VERSION
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    raw_data_local: bool = True
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION or self.feature_id != HUBAPI_INTERPRETATION_ASSURANCE_FEATURE_ID or self.contract_version != HUBAPI_INTERPRETATION_ASSURANCE_CONTRACT_VERSION:
            raise ResearchContractError("hubapi interpretation assurance schema, feature, or version mismatch")
        if self.boundary != PRECLINICAL_BOUNDARY or not self.raw_data_local or not self.request_id.strip() or not self.workflow_id.strip() or not self.objective_id.strip() or not self.scope.strip() or not self.ranked_order or not self.study_order or not self.modality_order or not self.effect_receipts:
            raise ResearchContractError("hubapi interpretation identity, coverage, locality, or effects are incomplete")
        if self.disposition not in {"qualified", "partial", "unknown", "blocked"}:
            raise ResearchContractError("hubapi interpretation disposition is unknown")
        if len(self.support_order) != len(self.ranked_order) or any(value not in self.ranked_order for value in (*self.admitted_order, *self.blocked_order, *self.unknown_order)):
            raise ResearchContractError("hubapi interpretation support or disposition linkage is incomplete")
        for values in (self.ranked_order, self.admitted_order, self.blocked_order, self.unknown_order, self.result_order, self.visualization_order, self.study_order, self.modality_order, self.omissions, self.uncertainty, self.negative_evidence, self.effect_receipts):
            if tuple(sorted(set(values))) != values:
                raise ResearchContractError("hubapi interpretation ordering is invalid")
        for values in (self.semantic_order, self.artifact_order, self.evidence_order, self.provenance_order, self.comparability_order, self.baseline_order):
            if tuple(sorted(set(values))) != values:
                raise ResearchContractError("hubapi interpretation digest ordering is invalid")
        digests = (*self.semantic_order, *self.artifact_order, *self.evidence_order, *self.provenance_order, *self.comparability_order, *self.baseline_order, self.replay_identity, self.artifact.get("content_hash"))
        if self.benchmark_digest is not None:
            digests += (self.benchmark_digest,)
        if any(not isinstance(value, str) or not re.fullmatch(r"[0-9a-f]{64}", value) for value in digests):
            raise ResearchContractError("hubapi interpretation digest is invalid")
        if self.admitted_order and any(not effect.startswith("evaluate:interpretation-assurance:") for effect in self.effect_receipts):
            raise ResearchContractError("admitted interpretations require an evaluation receipt")
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
            "objective_id": self.objective_id,
            "scope": self.scope,
            "disposition": self.disposition,
            "ranked_order": list(self.ranked_order),
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
            "baseline_order": list(self.baseline_order),
            "omissions": list(self.omissions),
            "uncertainty": list(self.uncertainty),
            "negative_evidence": list(self.negative_evidence),
            "replay_identity": self.replay_identity,
            "benchmark_digest": self.benchmark_digest,
            "effect_receipts": list(self.effect_receipts),
            "artifact": dict(self.artifact),
            "raw_data_local": self.raw_data_local,
            "boundary": self.boundary,
        })
