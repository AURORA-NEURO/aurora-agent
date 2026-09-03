"""Python parity contract for federated retrieval synthesis."""
from __future__ import annotations

from dataclasses import dataclass
import re
from typing import Any, Mapping

from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError, research_artifact_digest

FEDERATED_RETRIEVAL_SYNTHESIS_FEATURE_ID = "AFA-brain-P02-F04"
FEDERATED_RETRIEVAL_SYNTHESIS_CONTRACT_VERSION = "brain-federated-retrieval-synthesis/1.0"


@dataclass(frozen=True)
class BrainFederatedEvidenceSynthesis:
    request_id: str
    federation_id: str
    institution_id: str
    purpose: str
    semantic_profile: str
    endpoint: str
    study_order: tuple[str, ...]
    modality_order: tuple[str, ...]
    scope: str
    disposition: str
    candidate_order: tuple[str, ...]
    ranked_order: tuple[str, ...]
    qualified_order: tuple[str, ...]
    blocked_order: tuple[str, ...]
    unknown_order: tuple[str, ...]
    aggregate_order: tuple[str, ...]
    support_order: tuple[int, ...]
    comparability_digest: str
    envelope_digest: str
    synthesis_digest: str
    omissions: tuple[str, ...]
    uncertainty: tuple[str, ...]
    negative_evidence: tuple[str, ...]
    replay_identity: str
    effect_receipts: tuple[str, ...]
    artifact: Mapping[str, Any]
    feature_id: str = FEDERATED_RETRIEVAL_SYNTHESIS_FEATURE_ID
    contract_version: str = FEDERATED_RETRIEVAL_SYNTHESIS_CONTRACT_VERSION
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    raw_data_local: bool = True
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION or self.feature_id != FEDERATED_RETRIEVAL_SYNTHESIS_FEATURE_ID or self.contract_version != FEDERATED_RETRIEVAL_SYNTHESIS_CONTRACT_VERSION:
            raise ResearchContractError("federated retrieval schema mismatch")
        if self.boundary != PRECLINICAL_BOUNDARY or not self.raw_data_local or not self.request_id.strip() or not self.federation_id.strip() or not self.institution_id.strip() or not self.purpose.strip() or not self.semantic_profile.strip() or not self.endpoint.strip() or len(self.study_order) < 2 or len(self.modality_order) < 2 or not self.scope.strip() or self.disposition not in ("qualified", "partial", "unknown", "blocked") or not self.candidate_order or not self.ranked_order or len(self.ranked_order) != len(self.support_order) or not self.effect_receipts:
            raise ResearchContractError("federated retrieval identity incomplete")
        if any(value not in self.candidate_order for value in (*self.ranked_order, *self.qualified_order, *self.blocked_order, *self.unknown_order)):
            raise ResearchContractError("federated retrieval state is not covered")
        for values in (self.study_order, self.modality_order, self.candidate_order, self.qualified_order, self.blocked_order, self.unknown_order, self.aggregate_order, self.omissions, self.uncertainty, self.negative_evidence, self.effect_receipts):
            if tuple(sorted(set(values))) != values:
                raise ResearchContractError("federated retrieval ordering invalid")
        for value in (self.comparability_digest, self.envelope_digest, self.synthesis_digest, self.replay_identity, self.artifact.get("content_hash"), *self.aggregate_order):
            if not isinstance(value, str) or not re.fullmatch(r"[0-9a-f]{64}", value):
                raise ResearchContractError("federated retrieval digest invalid")
        if any(not effect.startswith("exchange:permitted-artifacts:") and effect != "block:unsafe-release" for effect in self.effect_receipts):
            raise ResearchContractError("federated retrieval effect invalid")

    def digest(self) -> str:
        self.validate()
        return research_artifact_digest({"schema_version": self.schema_version, "contract_version": self.contract_version, "feature_id": self.feature_id, "request_id": self.request_id, "federation_id": self.federation_id, "institution_id": self.institution_id, "purpose": self.purpose, "semantic_profile": self.semantic_profile, "endpoint": self.endpoint, "study_order": list(self.study_order), "modality_order": list(self.modality_order), "scope": self.scope, "disposition": self.disposition, "candidate_order": list(self.candidate_order), "ranked_order": list(self.ranked_order), "qualified_order": list(self.qualified_order), "blocked_order": list(self.blocked_order), "unknown_order": list(self.unknown_order), "aggregate_order": list(self.aggregate_order), "support_order": list(self.support_order), "comparability_digest": self.comparability_digest, "envelope_digest": self.envelope_digest, "synthesis_digest": self.synthesis_digest, "omissions": list(self.omissions), "uncertainty": list(self.uncertainty), "negative_evidence": list(self.negative_evidence), "replay_identity": self.replay_identity, "effect_receipts": list(self.effect_receipts), "artifact": dict(self.artifact), "raw_data_local": self.raw_data_local, "boundary": self.boundary})
