"""Small standard-library mirror of the Rust research-contract boundary.

The Python SDK transports receipts and hashes payloads; it never interprets an unknown receipt as a
positive scientific conclusion or moves protected source bytes out of an institution.
"""

from __future__ import annotations

from dataclasses import dataclass, field
import hashlib
import json
from typing import Any, Mapping, Sequence

RESEARCH_CONTRACT_SCHEMA_VERSION = "aurora-research-contract/1.0"
PRECLINICAL_BOUNDARY = "preclinical-research-only; no human-subject or clinical-source data; no diagnosis, treatment, triage, enrollment, or clinical decisions"
RESEARCH_FEATURE_ID = "AFA-bioir-P02-F01"
RELEASE_REVIEW_FEATURE_ID = "AFA-evalengine-P13-F01"
RESEARCH_INGESTION_FEATURE_ID = "AFA-adapter-P06-F01"


class ResearchContractError(ValueError):
    """A receipt would violate the shared safety or evidence boundary."""


def canonical_json(value: Any) -> bytes:
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False).encode("utf-8")


def research_artifact_digest(payload: Any) -> str:
    return hashlib.sha256(canonical_json(payload)).hexdigest()


@dataclass(frozen=True)
class PolicyReceipt:
    receipt_id: str
    decision: str
    reasons: tuple[str, ...]
    evaluated_artifacts: tuple[str, ...] = ()
    authority_reference: str | None = None
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION or self.boundary != PRECLINICAL_BOUNDARY:
            raise ResearchContractError("research contract schema or boundary mismatch")
        if not self.receipt_id.strip() or not self.reasons:
            raise ResearchContractError("policy receipt needs an id and reason")
        if self.decision in {"approval_required", "unresolved"} and self.authority_reference:
            raise ResearchContractError("authority is premature for unresolved policy")
        if self.decision == "allow" and "unresolved" in self.reasons:
            raise ResearchContractError("unresolved policy cannot allow")


@dataclass(frozen=True)
class EvidenceReceipt:
    receipt_id: str
    intent: str
    sources: tuple[Mapping[str, Any], ...]
    derivation: tuple[str, ...]
    uncertainty: tuple[Mapping[str, str], ...]
    omissions: tuple[Mapping[str, str], ...]
    conclusion_state: str
    competing_explanations: tuple[Mapping[str, Any], ...] = ()
    negative_evidence: tuple[Mapping[str, Any], ...] = ()
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION or self.boundary != PRECLINICAL_BOUNDARY:
            raise ResearchContractError("research contract schema or boundary mismatch")
        if not self.receipt_id.strip() or not self.intent.strip() or not self.derivation:
            raise ResearchContractError("evidence receipt is incomplete")
        if not self.sources and (self.conclusion_state != "unknown" or not self.omissions or not self.uncertainty):
            raise ResearchContractError("empty evidence must be explicit unknown")
        if self.conclusion_state == "proven" and any(item.get("could_change_decision") != "no_known_impact" for item in self.omissions):
            raise ResearchContractError("protected omission blocks proven conclusion")


@dataclass(frozen=True)
class ReleaseReview:
    """Transport-level mirror of the Rust fail-closed production review."""

    capability_id: str
    card_digest: str
    verdict: str
    reasons: tuple[str, ...]
    replications: tuple[Mapping[str, Any], ...] = ()
    checks: tuple[Mapping[str, Any], ...] = ()
    provenance_complete: bool = False
    feature_id: str = RELEASE_REVIEW_FEATURE_ID
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION or self.boundary != PRECLINICAL_BOUNDARY:
            raise ResearchContractError("research contract schema or boundary mismatch")
        if self.feature_id != RELEASE_REVIEW_FEATURE_ID or not self.capability_id.strip():
            raise ResearchContractError("release review feature or capability is missing")
        if len(self.card_digest) != 64 or any(char not in "0123456789abcdef" for char in self.card_digest):
            raise ResearchContractError("release review card digest is not a canonical sha256")
        if self.verdict not in {"pass", "conditional", "blocked", "not_evaluated"} or not self.reasons:
            raise ResearchContractError("release review verdict and reasons are required")
        if self.verdict == "pass" and not self.provenance_complete:
            raise ResearchContractError("a passing release review requires complete provenance")

    def digest(self) -> str:
        self.validate()
        return research_artifact_digest({
            "schema_version": self.schema_version,
            "feature_id": self.feature_id,
            "capability_id": self.capability_id,
            "card_digest": self.card_digest,
            "verdict": self.verdict,
            "reasons": list(self.reasons),
            "replications": list(self.replications),
            "checks": list(self.checks),
            "provenance_complete": self.provenance_complete,
            "boundary": self.boundary,
        })


@dataclass(frozen=True)
class ResearchIngestionBundle:
    """Portable QC certificate; raw source bytes stay institution-local."""

    source_id: str
    adapter: str
    adapter_version: str
    source_digest: str
    ingestion_digest: str
    artifact: Mapping[str, Any]
    conformance: Mapping[str, Any]
    raw_data_local: bool = True
    feature_id: str = RESEARCH_INGESTION_FEATURE_ID
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION or self.boundary != PRECLINICAL_BOUNDARY:
            raise ResearchContractError("research contract schema or boundary mismatch")
        if self.feature_id != RESEARCH_INGESTION_FEATURE_ID or not self.source_id.strip():
            raise ResearchContractError("research ingestion feature or source is missing")
        for digest in (self.source_digest, self.ingestion_digest, self.artifact.get("content_hash")):
            if not isinstance(digest, str) or len(digest) != 64 or any(char not in "0123456789abcdef" for char in digest):
                raise ResearchContractError("research ingestion digest is not a canonical sha256")
        if not self.raw_data_local:
            raise ResearchContractError("raw research data must remain local")
        if not self.conformance.get("verified", False):
            raise ResearchContractError("research ingestion is not conformance verified")

    def digest(self) -> str:
        self.validate()
        return research_artifact_digest({
            "schema_version": self.schema_version,
            "feature_id": self.feature_id,
            "source_id": self.source_id,
            "adapter": self.adapter,
            "adapter_version": self.adapter_version,
            "source_digest": self.source_digest,
            "ingestion_digest": self.ingestion_digest,
            "artifact": dict(self.artifact),
            "conformance": dict(self.conformance),
            "raw_data_local": self.raw_data_local,
            "boundary": self.boundary,
        })
