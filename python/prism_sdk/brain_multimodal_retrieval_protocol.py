"""Python parity contract for the multimodal retrieval protocol gateway."""
from __future__ import annotations

from dataclasses import dataclass
import re
from typing import Any, Mapping

from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError, research_artifact_digest

MULTIMODAL_RETRIEVAL_PROTOCOL_FEATURE_ID = "AFA-brain-P02-F22"
MULTIMODAL_RETRIEVAL_PROTOCOL_CONTRACT_VERSION = "brain-multimodal-retrieval-protocol-gateway/1.0"
MULTIMODAL_RETRIEVAL_PROTOCOL_STAGE_ORDER = ("protocol:open", "protocol:authorize", "protocol:retrieve", "protocol:synthesize", "protocol:close")


@dataclass(frozen=True)
class BrainMultimodalRetrievalProtocolReceipt:
    request_id: str
    protocol_id: str
    session_id: str
    study_order: tuple[str, ...]
    modality_order: tuple[str, ...]
    disposition: str
    offered_capability_order: tuple[str, ...]
    required_capability_order: tuple[str, ...]
    negotiated_capability_order: tuple[str, ...]
    stage_order: tuple[str, ...]
    completed_stage_order: tuple[str, ...]
    blocked_stage_order: tuple[str, ...]
    action_receipts: tuple[str, ...]
    candidate_order: tuple[str, ...]
    ranked_order: tuple[str, ...]
    qualified_order: tuple[str, ...]
    blocked_order: tuple[str, ...]
    unknown_order: tuple[str, ...]
    comparability_digest: str
    negotiation_digest: str
    transcript_digest: str
    synthesis_digest: str
    protocol_digest: str
    replay_identity: str
    budget_units: int
    omissions: tuple[str, ...]
    uncertainty: tuple[str, ...]
    negative_evidence: tuple[str, ...]
    effect_receipts: tuple[str, ...]
    artifact: Mapping[str, Any]
    feature_id: str = MULTIMODAL_RETRIEVAL_PROTOCOL_FEATURE_ID
    contract_version: str = MULTIMODAL_RETRIEVAL_PROTOCOL_CONTRACT_VERSION
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    raw_data_local: bool = True
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION or self.feature_id != MULTIMODAL_RETRIEVAL_PROTOCOL_FEATURE_ID or self.contract_version != MULTIMODAL_RETRIEVAL_PROTOCOL_CONTRACT_VERSION:
            raise ResearchContractError("multimodal retrieval protocol schema, feature, or version mismatch")
        if self.boundary != PRECLINICAL_BOUNDARY or not self.raw_data_local or not self.request_id.strip() or not self.protocol_id.strip() or not self.session_id.strip() or len(self.study_order) < 2 or len(self.modality_order) < 2 or not self.offered_capability_order or not self.required_capability_order or self.stage_order != MULTIMODAL_RETRIEVAL_PROTOCOL_STAGE_ORDER or not self.completed_stage_order or not self.action_receipts or not self.candidate_order or self.budget_units < len(MULTIMODAL_RETRIEVAL_PROTOCOL_STAGE_ORDER) or not self.effect_receipts:
            raise ResearchContractError("multimodal protocol identity, coverage, negotiation, stages, budget, locality, or effects are incomplete")
        for values in (self.study_order, self.modality_order, self.offered_capability_order, self.required_capability_order, self.negotiated_capability_order, self.action_receipts, self.candidate_order, self.ranked_order, self.qualified_order, self.blocked_order, self.unknown_order, self.omissions, self.uncertainty, self.negative_evidence, self.effect_receipts):
            if tuple(sorted(set(values))) != values:
                raise ResearchContractError("multimodal protocol vectors are not canonical")
        if self.disposition != "blocked" and any(value not in self.offered_capability_order for value in self.required_capability_order):
            raise ResearchContractError("required multimodal protocol capability was not offered")
        if any(value not in self.required_capability_order for value in self.negotiated_capability_order) or any(value not in self.candidate_order for value in (*self.ranked_order, *self.qualified_order, *self.blocked_order, *self.unknown_order)):
            raise ResearchContractError("multimodal protocol state is not covered by its declaration")
        if any(value not in self.stage_order for value in (*self.completed_stage_order, *self.blocked_stage_order)) or set(self.completed_stage_order) & set(self.blocked_stage_order):
            raise ResearchContractError("multimodal protocol stage transcript is invalid")
        for value in (self.comparability_digest, self.negotiation_digest, self.transcript_digest, self.synthesis_digest, self.protocol_digest, self.replay_identity, self.artifact.get("content_hash")):
            if not isinstance(value, str) or not re.fullmatch(r"[0-9a-f]{64}", value):
                raise ResearchContractError("multimodal protocol digest is invalid")
        if any(not effect.startswith("read:local-multimodal-protocol:") and effect != "block:unsafe-release" for effect in self.effect_receipts):
            raise ResearchContractError("multimodal protocol effect is not read-only")

    def digest(self) -> str:
        self.validate()
        return research_artifact_digest({"schema_version": self.schema_version, "contract_version": self.contract_version, "feature_id": self.feature_id, "request_id": self.request_id, "protocol_id": self.protocol_id, "session_id": self.session_id, "study_order": list(self.study_order), "modality_order": list(self.modality_order), "disposition": self.disposition, "offered_capability_order": list(self.offered_capability_order), "required_capability_order": list(self.required_capability_order), "negotiated_capability_order": list(self.negotiated_capability_order), "stage_order": list(self.stage_order), "completed_stage_order": list(self.completed_stage_order), "blocked_stage_order": list(self.blocked_stage_order), "action_receipts": list(self.action_receipts), "candidate_order": list(self.candidate_order), "ranked_order": list(self.ranked_order), "qualified_order": list(self.qualified_order), "blocked_order": list(self.blocked_order), "unknown_order": list(self.unknown_order), "comparability_digest": self.comparability_digest, "negotiation_digest": self.negotiation_digest, "transcript_digest": self.transcript_digest, "synthesis_digest": self.synthesis_digest, "protocol_digest": self.protocol_digest, "replay_identity": self.replay_identity, "budget_units": self.budget_units, "omissions": list(self.omissions), "uncertainty": list(self.uncertainty), "negative_evidence": list(self.negative_evidence), "effect_receipts": list(self.effect_receipts), "artifact": dict(self.artifact), "raw_data_local": self.raw_data_local, "boundary": self.boundary})
