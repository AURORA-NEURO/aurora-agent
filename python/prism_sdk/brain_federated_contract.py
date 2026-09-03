"""Python mirror of the federated evidence contract receipt."""

from __future__ import annotations

from dataclasses import dataclass
import re
from typing import Any, Mapping

from .research_contracts import (
    FEDERATED_CONTRACT_MODEL_CONTRACT_VERSION,
    FEDERATED_CONTRACT_MODEL_FEATURE_ID,
    PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
    ResearchContractError,
    research_artifact_digest,
)


@dataclass(frozen=True)
class BrainFederatedContractModelReceipt:
    request_id: str
    federation_id: str
    institution_id: str
    purpose: str
    endpoint: str
    semantic_profile: str
    disposition: str
    compatibility: str
    input_schema: str
    output_schema: str
    required_order: tuple[str, ...]
    provided_order: tuple[str, ...]
    missing_order: tuple[str, ...]
    semantic_loss_order: tuple[str, ...]
    allowed_artifact_order: tuple[str, ...]
    export_scope: str
    semantic_digest: str
    provenance_digest: str
    contract_digest: str
    envelope_digest: str
    replay_identity: str
    omissions: tuple[str, ...]
    uncertainty: tuple[str, ...]
    negative_evidence: tuple[str, ...]
    effect_receipts: tuple[str, ...]
    artifact: Mapping[str, Any]
    feature_id: str = FEDERATED_CONTRACT_MODEL_FEATURE_ID
    contract_version: str = FEDERATED_CONTRACT_MODEL_CONTRACT_VERSION
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    raw_data_local: bool = True
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION or self.feature_id != FEDERATED_CONTRACT_MODEL_FEATURE_ID or self.contract_version != FEDERATED_CONTRACT_MODEL_CONTRACT_VERSION:
            raise ResearchContractError("federated contract schema, feature, or version mismatch")
        if self.boundary != PRECLINICAL_BOUNDARY or not self.raw_data_local or not self.request_id.strip() or not self.federation_id.strip() or not self.institution_id.strip() or not self.purpose.strip() or not self.endpoint.strip() or not self.semantic_profile.strip() or self.input_schema != "EvidenceFeed4@1" or self.output_schema != "QualifiedEvidenceSet2@1" or not self.required_order or not self.provided_order or not self.allowed_artifact_order or not self.export_scope.strip() or not self.effect_receipts:
            raise ResearchContractError("federated identity, schemas, fields, artifact policy, export scope, locality, or effects are incomplete")
        if self.disposition not in {"qualified", "partial", "unknown", "blocked"} or self.compatibility not in {"additive", "migration_required", "breaking", "unknown"}:
            raise ResearchContractError("federated disposition or compatibility is unknown")
        if any(value not in self.required_order for value in self.missing_order) or any(value not in self.provided_order for value in self.semantic_loss_order):
            raise ResearchContractError("federated loss state is outside declared fields")
        for values in (self.required_order, self.provided_order, self.missing_order, self.semantic_loss_order, self.allowed_artifact_order, self.omissions, self.uncertainty, self.negative_evidence, self.effect_receipts):
            if tuple(sorted(set(values))) != values:
                raise ResearchContractError("federated contract ordering is invalid")
        for value in (self.semantic_digest, self.provenance_digest, self.contract_digest, self.envelope_digest, self.replay_identity, self.artifact.get("content_hash")):
            if not isinstance(value, str) or not re.fullmatch(r"[0-9a-f]{64}", value):
                raise ResearchContractError("federated contract digest is invalid")
        if self.disposition == "qualified" and any(not effect.startswith("exchange:permitted-artifacts:") for effect in self.effect_receipts):
            raise ResearchContractError("qualified federation requires a permitted-artifact exchange receipt")
        if self.disposition != "qualified" and self.effect_receipts != ("block:unsafe-release",):
            raise ResearchContractError("non-qualified federation must be explicitly blocked")

    def digest(self) -> str:
        self.validate()
        return research_artifact_digest({
            "schema_version": self.schema_version, "contract_version": self.contract_version, "feature_id": self.feature_id,
            "request_id": self.request_id, "federation_id": self.federation_id, "institution_id": self.institution_id,
            "purpose": self.purpose, "endpoint": self.endpoint, "semantic_profile": self.semantic_profile, "disposition": self.disposition,
            "compatibility": self.compatibility, "input_schema": self.input_schema, "output_schema": self.output_schema,
            "required_order": list(self.required_order), "provided_order": list(self.provided_order), "missing_order": list(self.missing_order),
            "semantic_loss_order": list(self.semantic_loss_order), "allowed_artifact_order": list(self.allowed_artifact_order), "export_scope": self.export_scope,
            "semantic_digest": self.semantic_digest, "provenance_digest": self.provenance_digest, "contract_digest": self.contract_digest,
            "envelope_digest": self.envelope_digest, "replay_identity": self.replay_identity, "omissions": list(self.omissions),
            "uncertainty": list(self.uncertainty), "negative_evidence": list(self.negative_evidence), "effect_receipts": list(self.effect_receipts),
            "artifact": dict(self.artifact), "raw_data_local": self.raw_data_local, "boundary": self.boundary,
        })
