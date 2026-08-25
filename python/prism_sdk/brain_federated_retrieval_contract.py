"""Python parity contract for the federated retrieval contract model."""
from __future__ import annotations

from dataclasses import dataclass
import re
from typing import Any, Mapping

from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError, research_artifact_digest

FEDERATED_RETRIEVAL_CONTRACT_MODEL_FEATURE_ID = "AFA-brain-P02-F08"
FEDERATED_RETRIEVAL_CONTRACT_MODEL_CONTRACT_VERSION = "brain-federated-retrieval-contract-model/1.0"
FEDERATED_RETRIEVAL_INPUT_SCHEMA = "FederatedRetrievalQuery1@1"
FEDERATED_RETRIEVAL_OUTPUT_SCHEMA = "FederatedEvidenceSynthesis1@1"
PERMITTED_FEDERATED_ARTIFACT = "qualified-evidence-summary"


@dataclass(frozen=True)
class BrainFederatedRetrievalContractModelReceipt:
    request_id: str
    federation_id: str
    institution_id: str
    purpose: str
    semantic_profile: str
    endpoint: str
    study_order: tuple[str, ...]
    modality_order: tuple[str, ...]
    disposition: str
    compatibility: str
    input_schema: str
    output_schema: str
    permitted_artifact: str
    comparability_digest: str
    envelope_digest: str
    semantic_digest: str
    artifact_digest: str
    provenance_digest: str
    contract_digest: str
    replay_identity: str
    omissions: tuple[str, ...]
    uncertainty: tuple[str, ...]
    negative_evidence: tuple[str, ...]
    effect_receipts: tuple[str, ...]
    artifact: Mapping[str, Any]
    feature_id: str = FEDERATED_RETRIEVAL_CONTRACT_MODEL_FEATURE_ID
    contract_version: str = FEDERATED_RETRIEVAL_CONTRACT_MODEL_CONTRACT_VERSION
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    raw_data_local: bool = True
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION or self.feature_id != FEDERATED_RETRIEVAL_CONTRACT_MODEL_FEATURE_ID or self.contract_version != FEDERATED_RETRIEVAL_CONTRACT_MODEL_CONTRACT_VERSION or self.input_schema != FEDERATED_RETRIEVAL_INPUT_SCHEMA or self.output_schema != FEDERATED_RETRIEVAL_OUTPUT_SCHEMA:
            raise ResearchContractError("federated retrieval contract schema mismatch")
        if self.boundary != PRECLINICAL_BOUNDARY or not self.raw_data_local or not self.request_id.strip() or not self.federation_id.strip() or not self.institution_id.strip() or not self.purpose.strip() or not self.semantic_profile.strip() or not self.endpoint.strip() or len(self.study_order) < 2 or len(self.modality_order) < 2 or self.disposition not in ("qualified", "partial", "unknown", "blocked") or self.compatibility not in ("additive", "migration_required", "breaking", "unknown") or self.permitted_artifact != PERMITTED_FEDERATED_ARTIFACT or not self.effect_receipts:
            raise ResearchContractError("federated retrieval contract identity incomplete")
        for values in (self.study_order, self.modality_order, self.omissions, self.uncertainty, self.negative_evidence, self.effect_receipts):
            if tuple(sorted(set(values))) != values:
                raise ResearchContractError("federated retrieval contract ordering invalid")
        for value in (self.comparability_digest, self.envelope_digest, self.semantic_digest, self.artifact_digest, self.provenance_digest, self.contract_digest, self.replay_identity, self.artifact.get("content_hash")):
            if not isinstance(value, str) or not re.fullmatch(r"[0-9a-f]{64}", value):
                raise ResearchContractError("federated retrieval contract digest invalid")
        if any(not effect.startswith("exchange:permitted-artifacts:") and effect != "block:unsafe-release" for effect in self.effect_receipts):
            raise ResearchContractError("federated retrieval contract effect invalid")

    def digest(self) -> str:
        self.validate()
        return research_artifact_digest({"schema_version": self.schema_version, "contract_version": self.contract_version, "feature_id": self.feature_id, "request_id": self.request_id, "federation_id": self.federation_id, "institution_id": self.institution_id, "purpose": self.purpose, "semantic_profile": self.semantic_profile, "endpoint": self.endpoint, "study_order": list(self.study_order), "modality_order": list(self.modality_order), "disposition": self.disposition, "compatibility": self.compatibility, "input_schema": self.input_schema, "output_schema": self.output_schema, "permitted_artifact": self.permitted_artifact, "comparability_digest": self.comparability_digest, "envelope_digest": self.envelope_digest, "semantic_digest": self.semantic_digest, "artifact_digest": self.artifact_digest, "provenance_digest": self.provenance_digest, "contract_digest": self.contract_digest, "replay_identity": self.replay_identity, "omissions": list(self.omissions), "uncertainty": list(self.uncertainty), "negative_evidence": list(self.negative_evidence), "effect_receipts": list(self.effect_receipts), "artifact": dict(self.artifact), "raw_data_local": self.raw_data_local, "boundary": self.boundary})
