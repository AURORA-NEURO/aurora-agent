"""Python parity contract for local typed research-context compilation."""
from __future__ import annotations

from dataclasses import dataclass
import re
from typing import Any, Mapping

from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError, research_artifact_digest

CONTEXT_COMPILATION_FEATURE_ID = "AFA-brain-P03-F01"
CONTEXT_COMPILATION_CONTRACT_VERSION = "brain-research-context-compilation/1.0"


@dataclass(frozen=True)
class BrainResearchContextCompilationReceipt:
    request_id: str
    objective: str
    scope: str
    disposition: str
    required_fact_order: tuple[str, ...]
    resolved_fact_order: tuple[str, ...]
    missing_fact_order: tuple[str, ...]
    blocked_fact_order: tuple[str, ...]
    unknown_fact_order: tuple[str, ...]
    context_digest: str
    replay_identity: str
    omissions: tuple[str, ...]
    uncertainty: tuple[str, ...]
    negative_evidence: tuple[str, ...]
    effect_receipts: tuple[str, ...]
    artifact: Mapping[str, Any]
    feature_id: str = CONTEXT_COMPILATION_FEATURE_ID
    contract_version: str = CONTEXT_COMPILATION_CONTRACT_VERSION
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    raw_data_local: bool = True
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION or self.feature_id != CONTEXT_COMPILATION_FEATURE_ID or self.contract_version != CONTEXT_COMPILATION_CONTRACT_VERSION:
            raise ResearchContractError("context compilation schema, feature, or version mismatch")
        if self.boundary != PRECLINICAL_BOUNDARY or not self.raw_data_local or not self.request_id.strip() or not self.objective.strip() or not self.scope.strip() or not self.required_fact_order or not self.effect_receipts or self.disposition not in {"qualified", "partial", "unknown", "blocked"}:
            raise ResearchContractError("context identity, boundary, disposition, required facts, locality, or effects are incomplete")
        for values in (self.required_fact_order, self.resolved_fact_order, self.missing_fact_order, self.blocked_fact_order, self.unknown_fact_order, self.omissions, self.uncertainty, self.negative_evidence, self.effect_receipts):
            if tuple(sorted(set(values))) != values:
                raise ResearchContractError("context vectors are not canonical")
        required = set(self.required_fact_order)
        resolved = set(self.resolved_fact_order)
        missing = set(self.missing_fact_order)
        blocked = set(self.blocked_fact_order)
        unknown = set(self.unknown_fact_order)
        if resolved | missing | blocked | unknown != required or len(resolved) + len(missing) + len(blocked) + len(unknown) != len(required):
            raise ResearchContractError("context fact states do not partition required facts")
        for value in (self.context_digest, self.replay_identity, self.artifact.get("content_hash")):
            if not isinstance(value, str) or not re.fullmatch(r"[0-9a-f]{64}", value):
                raise ResearchContractError("context digest is invalid")
        if any(not effect.startswith("compile:local-research-context:") and effect != "block:unsafe-release" for effect in self.effect_receipts):
            raise ResearchContractError("context effect is outside local compilation gate")

    def digest(self) -> str:
        self.validate()
        return research_artifact_digest({"schema_version": self.schema_version, "contract_version": self.contract_version, "feature_id": self.feature_id, "request_id": self.request_id, "objective": self.objective, "scope": self.scope, "disposition": self.disposition, "required_fact_order": list(self.required_fact_order), "resolved_fact_order": list(self.resolved_fact_order), "missing_fact_order": list(self.missing_fact_order), "blocked_fact_order": list(self.blocked_fact_order), "unknown_fact_order": list(self.unknown_fact_order), "context_digest": self.context_digest, "replay_identity": self.replay_identity, "omissions": list(self.omissions), "uncertainty": list(self.uncertainty), "negative_evidence": list(self.negative_evidence), "effect_receipts": list(self.effect_receipts), "artifact": dict(self.artifact), "raw_data_local": self.raw_data_local, "boundary": self.boundary})
