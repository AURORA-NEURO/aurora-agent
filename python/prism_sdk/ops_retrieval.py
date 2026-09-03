"""Python mirror of the ops local retrieval assurance receipt."""

from __future__ import annotations

from dataclasses import dataclass
import re
from typing import Any, Mapping

from .research_contracts import (
    OPS_RETRIEVAL_ASSURANCE_CONTRACT_VERSION,
    OPS_RETRIEVAL_ASSURANCE_FEATURE_ID,
    PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
    ResearchContractError,
    research_artifact_digest,
)


@dataclass(frozen=True)
class OpsRetrievalAssuranceReceipt:
    """Validates deterministic, local-only retrieval and synthesis metadata."""

    request_id: str
    study_id: str
    scope: str
    disposition: str
    candidate_order: tuple[str, ...]
    admitted_order: tuple[str, ...]
    blocked_order: tuple[str, ...]
    unknown_order: tuple[str, ...]
    source_order: tuple[str, ...]
    modality_order: tuple[str, ...]
    support_order: tuple[int, ...]
    semantic_order: tuple[str, ...]
    artifact_order: tuple[str, ...]
    provenance_order: tuple[str, ...]
    omissions: tuple[str, ...]
    uncertainty: tuple[str, ...]
    negative_evidence: tuple[str, ...]
    replay_identity: str
    benchmark_digest: str | None
    effect_receipts: tuple[str, ...]
    artifact: Mapping[str, Any]
    feature_id: str = OPS_RETRIEVAL_ASSURANCE_FEATURE_ID
    contract_version: str = OPS_RETRIEVAL_ASSURANCE_CONTRACT_VERSION
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    raw_data_local: bool = True
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION or self.feature_id != OPS_RETRIEVAL_ASSURANCE_FEATURE_ID or self.contract_version != OPS_RETRIEVAL_ASSURANCE_CONTRACT_VERSION:
            raise ResearchContractError("ops retrieval schema, feature, or version mismatch")
        if self.boundary != PRECLINICAL_BOUNDARY or not self.raw_data_local or not self.request_id.strip() or not self.study_id.strip() or not self.scope.strip() or not self.candidate_order or len(self.support_order) != len(self.candidate_order) or not self.effect_receipts:
            raise ResearchContractError("retrieval identity, ranking, locality, support, or effects are incomplete")
        if self.disposition not in {"qualified", "partial", "unknown", "blocked"}:
            raise ResearchContractError("retrieval disposition is unknown")
        if any(value not in self.candidate_order for value in (*self.admitted_order, *self.blocked_order, *self.unknown_order)):
            raise ResearchContractError("retrieval candidate state is not covered by candidate order")
        for values in (self.candidate_order, self.admitted_order, self.blocked_order, self.unknown_order, self.source_order, self.modality_order, self.omissions, self.uncertainty, self.negative_evidence, self.effect_receipts):
            if tuple(sorted(set(values))) != values:
                raise ResearchContractError("retrieval ordering is invalid")
        for values in (self.semantic_order, self.artifact_order, self.provenance_order):
            if tuple(sorted(set(values))) != values:
                raise ResearchContractError("retrieval digest ordering is invalid")
        digests = (*self.semantic_order, *self.artifact_order, *self.provenance_order, self.replay_identity, self.artifact.get("content_hash"))
        if self.benchmark_digest is not None:
            digests += (self.benchmark_digest,)
        if any(not isinstance(value, str) or not re.fullmatch(r"[0-9a-f]{64}", value) for value in digests):
            raise ResearchContractError("retrieval digest is invalid")
        if self.admitted_order and any(not effect.startswith("evaluate:retrieval-assurance:") for effect in self.effect_receipts):
            raise ResearchContractError("admitted retrieval requires an evaluation receipt")
        if not self.admitted_order and self.effect_receipts != ("block:unsafe-release",):
            raise ResearchContractError("empty retrieval result must be explicitly blocked")

    def digest(self) -> str:
        self.validate()
        return research_artifact_digest({
            "schema_version": self.schema_version,
            "contract_version": self.contract_version,
            "feature_id": self.feature_id,
            "request_id": self.request_id,
            "study_id": self.study_id,
            "scope": self.scope,
            "disposition": self.disposition,
            "candidate_order": list(self.candidate_order),
            "admitted_order": list(self.admitted_order),
            "blocked_order": list(self.blocked_order),
            "unknown_order": list(self.unknown_order),
            "source_order": list(self.source_order),
            "modality_order": list(self.modality_order),
            "support_order": list(self.support_order),
            "semantic_order": list(self.semantic_order),
            "artifact_order": list(self.artifact_order),
            "provenance_order": list(self.provenance_order),
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
