"""Python mirror of the aggregate-only federated evidence receipt."""

from __future__ import annotations

from dataclasses import dataclass
import re
from typing import Any, Mapping

from .research_contracts import (
    FEDERATED_EVIDENCE_SURVEILLANCE_CONTRACT_VERSION,
    FEDERATED_EVIDENCE_SURVEILLANCE_FEATURE_ID,
    PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
    ResearchContractError,
    research_artifact_digest,
)


@dataclass(frozen=True)
class BrainFederatedEvidenceReceipt:
    request_id: str
    federation_id: str
    institution_id: str
    purpose: str
    semantic_profile: str
    endpoint: str
    disposition: str
    candidate_order: tuple[str, ...]
    admitted_order: tuple[str, ...]
    blocked_order: tuple[str, ...]
    unknown_order: tuple[str, ...]
    aggregate_order: tuple[str, ...]
    omissions: tuple[str, ...]
    uncertainty: tuple[str, ...]
    negative_evidence: tuple[str, ...]
    replay_identity: str
    effect_receipts: tuple[str, ...]
    artifact: Mapping[str, Any]
    feature_id: str = FEDERATED_EVIDENCE_SURVEILLANCE_FEATURE_ID
    contract_version: str = FEDERATED_EVIDENCE_SURVEILLANCE_CONTRACT_VERSION
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    raw_data_local: bool = True
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION or self.feature_id != FEDERATED_EVIDENCE_SURVEILLANCE_FEATURE_ID or self.contract_version != FEDERATED_EVIDENCE_SURVEILLANCE_CONTRACT_VERSION:
            raise ResearchContractError("federated evidence schema, feature, or version mismatch")
        if self.boundary != PRECLINICAL_BOUNDARY or not self.raw_data_local or not self.request_id.strip() or not self.federation_id.strip() or not self.institution_id.strip() or not self.purpose.strip() or not self.semantic_profile.strip() or not self.endpoint.strip() or not self.candidate_order or not self.effect_receipts:
            raise ResearchContractError("federated identity, envelope, locality, ranking, or effects are incomplete")
        if self.disposition not in {"qualified", "partial", "unknown", "blocked"}:
            raise ResearchContractError("federated disposition is unknown")
        if any(value not in self.candidate_order for value in (*self.admitted_order, *self.blocked_order, *self.unknown_order)):
            raise ResearchContractError("federated state is not covered by candidate order")
        for values in (self.candidate_order, self.admitted_order, self.blocked_order, self.unknown_order, self.aggregate_order, self.omissions, self.uncertainty, self.negative_evidence, self.effect_receipts):
            if tuple(sorted(set(values))) != values:
                raise ResearchContractError("federated ordering is invalid")
        for value in (*self.aggregate_order, self.replay_identity, self.artifact.get("content_hash")):
            if not isinstance(value, str) or not re.fullmatch(r"[0-9a-f]{64}", value):
                raise ResearchContractError("federated digest is invalid")
        if self.admitted_order and any(not effect.startswith("exchange:permitted-artifacts:") for effect in self.effect_receipts):
            raise ResearchContractError("admitted federation requires a permitted-artifact exchange receipt")
        if not self.admitted_order and self.effect_receipts != ("block:unsafe-release",):
            raise ResearchContractError("empty federation result must be explicitly blocked")

    def digest(self) -> str:
        self.validate()
        return research_artifact_digest({
            "schema_version": self.schema_version, "contract_version": self.contract_version, "feature_id": self.feature_id,
            "request_id": self.request_id, "federation_id": self.federation_id, "institution_id": self.institution_id,
            "purpose": self.purpose, "semantic_profile": self.semantic_profile, "endpoint": self.endpoint,
            "disposition": self.disposition, "candidate_order": list(self.candidate_order), "admitted_order": list(self.admitted_order),
            "blocked_order": list(self.blocked_order), "unknown_order": list(self.unknown_order), "aggregate_order": list(self.aggregate_order),
            "omissions": list(self.omissions), "uncertainty": list(self.uncertainty), "negative_evidence": list(self.negative_evidence),
            "replay_identity": self.replay_identity, "effect_receipts": list(self.effect_receipts), "artifact": dict(self.artifact),
            "raw_data_local": self.raw_data_local, "boundary": self.boundary,
        })
