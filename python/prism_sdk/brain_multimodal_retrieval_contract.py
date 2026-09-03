"""Python parity contract for the multimodal retrieval contract model."""
from __future__ import annotations

from dataclasses import dataclass
import re
from typing import Any, Mapping

from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError, research_artifact_digest

MULTIMODAL_RETRIEVAL_CONTRACT_MODEL_FEATURE_ID = "AFA-brain-P02-F06"
MULTIMODAL_RETRIEVAL_CONTRACT_MODEL_CONTRACT_VERSION = "brain-multimodal-retrieval-contract-model/1.0"
MULTIMODAL_RETRIEVAL_INPUT_SCHEMA = "ScopedRetrievalQuery2@1"
MULTIMODAL_RETRIEVAL_OUTPUT_SCHEMA = "EvidenceSynthesis2@1"


@dataclass(frozen=True)
class BrainMultimodalRetrievalContractModelReceipt:
    request_id: str
    study_order: tuple[str, ...]
    scope: str
    disposition: str
    compatibility: str
    input_schema: str
    output_schema: str
    modality_required_order: tuple[str, ...]
    modality_provided_order: tuple[str, ...]
    modality_missing_order: tuple[str, ...]
    semantic_loss_order: tuple[str, ...]
    semantic_digest: str
    comparability_digest: str
    artifact_digest: str
    provenance_digest: str
    contract_digest: str
    replay_identity: str
    omissions: tuple[str, ...]
    uncertainty: tuple[str, ...]
    negative_evidence: tuple[str, ...]
    effect_receipts: tuple[str, ...]
    artifact: Mapping[str, Any]
    feature_id: str = MULTIMODAL_RETRIEVAL_CONTRACT_MODEL_FEATURE_ID
    contract_version: str = MULTIMODAL_RETRIEVAL_CONTRACT_MODEL_CONTRACT_VERSION
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    raw_data_local: bool = True
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION or self.feature_id != MULTIMODAL_RETRIEVAL_CONTRACT_MODEL_FEATURE_ID or self.contract_version != MULTIMODAL_RETRIEVAL_CONTRACT_MODEL_CONTRACT_VERSION or self.input_schema != MULTIMODAL_RETRIEVAL_INPUT_SCHEMA or self.output_schema != MULTIMODAL_RETRIEVAL_OUTPUT_SCHEMA:
            raise ResearchContractError("multimodal retrieval contract schema mismatch")
        if self.boundary != PRECLINICAL_BOUNDARY or not self.raw_data_local or not self.request_id.strip() or len(self.study_order) < 2 or not self.scope.strip() or self.disposition not in ("qualified", "partial", "unknown", "blocked") or self.compatibility not in ("additive", "migration_required", "breaking", "unknown") or len(self.modality_required_order) < 2 or not self.modality_provided_order or not self.effect_receipts:
            raise ResearchContractError("multimodal retrieval contract identity incomplete")
        if any(value not in self.modality_required_order for value in self.modality_missing_order) or any(value not in self.modality_provided_order for value in self.semantic_loss_order):
            raise ResearchContractError("multimodal retrieval contract loss state is not covered")
        for values in (self.study_order, self.modality_required_order, self.modality_provided_order, self.modality_missing_order, self.semantic_loss_order, self.omissions, self.uncertainty, self.negative_evidence, self.effect_receipts):
            if tuple(sorted(set(values))) != values:
                raise ResearchContractError("multimodal retrieval contract ordering invalid")
        for value in (self.semantic_digest, self.comparability_digest, self.artifact_digest, self.provenance_digest, self.contract_digest, self.replay_identity, self.artifact.get("content_hash")):
            if not isinstance(value, str) or not re.fullmatch(r"[0-9a-f]{64}", value):
                raise ResearchContractError("multimodal retrieval contract digest invalid")
        if any(not effect.startswith("read:local-multimodal-artifacts:") and effect != "block:unsafe-release" for effect in self.effect_receipts):
            raise ResearchContractError("multimodal retrieval contract effect invalid")

    def digest(self) -> str:
        self.validate()
        return research_artifact_digest({"schema_version": self.schema_version, "contract_version": self.contract_version, "feature_id": self.feature_id, "request_id": self.request_id, "study_order": list(self.study_order), "scope": self.scope, "disposition": self.disposition, "compatibility": self.compatibility, "input_schema": self.input_schema, "output_schema": self.output_schema, "modality_required_order": list(self.modality_required_order), "modality_provided_order": list(self.modality_provided_order), "modality_missing_order": list(self.modality_missing_order), "semantic_loss_order": list(self.semantic_loss_order), "semantic_digest": self.semantic_digest, "comparability_digest": self.comparability_digest, "artifact_digest": self.artifact_digest, "provenance_digest": self.provenance_digest, "contract_digest": self.contract_digest, "replay_identity": self.replay_identity, "omissions": list(self.omissions), "uncertainty": list(self.uncertainty), "negative_evidence": list(self.negative_evidence), "effect_receipts": list(self.effect_receipts), "artifact": dict(self.artifact), "raw_data_local": self.raw_data_local, "boundary": self.boundary})
