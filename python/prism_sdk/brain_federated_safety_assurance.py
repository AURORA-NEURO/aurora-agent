"""Python parity contract for the federated safety assurance harness."""
from __future__ import annotations

from dataclasses import dataclass
import re
from typing import Any, Mapping

from .research_contracts import (
    PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
    ResearchContractError,
    research_artifact_digest,
)

FEDERATED_SAFETY_ASSURANCE_FEATURE_ID = "AFA-brain-P01-F28"
FEDERATED_SAFETY_ASSURANCE_CONTRACT_VERSION = "brain-federated-evidence-assurance/1.0"


@dataclass(frozen=True)
class BrainFederatedAssuranceReceipt:
    request_id: str
    federation_id: str
    institution_id: str
    purpose: str
    semantic_profile: str
    endpoint: str
    verdict: str
    candidate_order: tuple[str, ...]
    admitted_order: tuple[str, ...]
    blocked_order: tuple[str, ...]
    unknown_order: tuple[str, ...]
    aggregate_order: tuple[str, ...]
    witness_order: tuple[str, ...]
    counterexample_order: tuple[str, ...]
    envelope_digest: str
    verification_digest: str
    replay_identity: str
    omissions: tuple[str, ...]
    uncertainty: tuple[str, ...]
    negative_evidence: tuple[str, ...]
    effect_receipts: tuple[str, ...]
    artifact: Mapping[str, Any]
    feature_id: str = FEDERATED_SAFETY_ASSURANCE_FEATURE_ID
    contract_version: str = FEDERATED_SAFETY_ASSURANCE_CONTRACT_VERSION
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    raw_data_local: bool = True
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if (
            self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            or self.feature_id != FEDERATED_SAFETY_ASSURANCE_FEATURE_ID
            or self.contract_version != FEDERATED_SAFETY_ASSURANCE_CONTRACT_VERSION
        ):
            raise ResearchContractError("federated assurance schema mismatch")
        if (
            self.boundary != PRECLINICAL_BOUNDARY
            or not self.raw_data_local
            or not self.request_id.strip()
            or not self.federation_id.strip()
            or not self.institution_id.strip()
            or not self.purpose.strip()
            or not self.semantic_profile.strip()
            or not self.endpoint.strip()
            or self.verdict not in ("qualified", "unresolved", "blocked")
            or not self.candidate_order
            or not self.witness_order
            or not self.effect_receipts
        ):
            raise ResearchContractError("federated assurance identity incomplete")
        if any(
            value not in self.candidate_order
            for value in (*self.admitted_order, *self.blocked_order, *self.unknown_order)
        ):
            raise ResearchContractError("federated assurance state is not covered")
        for values in (
            self.candidate_order,
            self.admitted_order,
            self.blocked_order,
            self.unknown_order,
            self.witness_order,
            self.counterexample_order,
            self.omissions,
            self.uncertainty,
            self.negative_evidence,
            self.effect_receipts,
        ):
            if tuple(sorted(set(values))) != values:
                raise ResearchContractError("federated assurance ordering invalid")
        if tuple(sorted(set(self.aggregate_order))) != self.aggregate_order:
            raise ResearchContractError("federated aggregate ordering invalid")
        for value in (
            self.envelope_digest,
            self.verification_digest,
            self.replay_identity,
            self.artifact.get("content_hash"),
            *self.aggregate_order,
        ):
            if not isinstance(value, str) or not re.fullmatch(r"[0-9a-f]{64}", value):
                raise ResearchContractError("federated assurance digest invalid")
        if any(
            not effect.startswith("assurance:federated:")
            and effect != "block:unsafe-release"
            for effect in self.effect_receipts
        ):
            raise ResearchContractError("federated assurance effect invalid")

    def digest(self) -> str:
        self.validate()
        return research_artifact_digest(
            {
                "schema_version": self.schema_version,
                "contract_version": self.contract_version,
                "feature_id": self.feature_id,
                "request_id": self.request_id,
                "federation_id": self.federation_id,
                "institution_id": self.institution_id,
                "purpose": self.purpose,
                "semantic_profile": self.semantic_profile,
                "endpoint": self.endpoint,
                "verdict": self.verdict,
                "candidate_order": list(self.candidate_order),
                "admitted_order": list(self.admitted_order),
                "blocked_order": list(self.blocked_order),
                "unknown_order": list(self.unknown_order),
                "aggregate_order": list(self.aggregate_order),
                "witness_order": list(self.witness_order),
                "counterexample_order": list(self.counterexample_order),
                "envelope_digest": self.envelope_digest,
                "verification_digest": self.verification_digest,
                "replay_identity": self.replay_identity,
                "omissions": list(self.omissions),
                "uncertainty": list(self.uncertainty),
                "negative_evidence": list(self.negative_evidence),
                "effect_receipts": list(self.effect_receipts),
                "artifact": dict(self.artifact),
                "raw_data_local": self.raw_data_local,
                "boundary": self.boundary,
            }
        )
