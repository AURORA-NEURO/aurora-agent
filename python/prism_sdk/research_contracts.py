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

