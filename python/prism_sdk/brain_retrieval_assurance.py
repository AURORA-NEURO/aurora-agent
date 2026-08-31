"""Python parity contract for the local retrieval assurance harness."""
from __future__ import annotations

from dataclasses import dataclass
import re
from typing import Any, Mapping

from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError, research_artifact_digest

RETRIEVAL_ASSURANCE_FEATURE_ID = "AFA-brain-P02-F25"
RETRIEVAL_ASSURANCE_CONTRACT_VERSION = "brain-retrieval-assurance-harness/1.0"


@dataclass(frozen=True)
class BrainRetrievalAssuranceReceipt:
    request_id: str
    study_id: str
    scope: str
    verdict: str
    candidate_order: tuple[str, ...]
    qualified_order: tuple[str, ...]
    blocked_order: tuple[str, ...]
    unknown_order: tuple[str, ...]
    witness_order: tuple[str, ...]
    counterexample_order: tuple[str, ...]
    synthesis_digest: str
    verification_digest: str
    replay_identity: str
    omissions: tuple[str, ...]
    uncertainty: tuple[str, ...]
    negative_evidence: tuple[str, ...]
    effect_receipts: tuple[str, ...]
    artifact: Mapping[str, Any]
    feature_id: str = RETRIEVAL_ASSURANCE_FEATURE_ID
    contract_version: str = RETRIEVAL_ASSURANCE_CONTRACT_VERSION
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    raw_data_local: bool = True
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION or self.feature_id != RETRIEVAL_ASSURANCE_FEATURE_ID or self.contract_version != RETRIEVAL_ASSURANCE_CONTRACT_VERSION:
            raise ResearchContractError("retrieval assurance schema, feature, or version mismatch")
        if self.boundary != PRECLINICAL_BOUNDARY or not self.raw_data_local or not self.request_id.strip() or not self.study_id.strip() or not self.scope.strip() or self.verdict not in {"qualified", "unresolved", "blocked"} or not self.candidate_order or not self.witness_order or not self.effect_receipts:
            raise ResearchContractError("retrieval assurance identity, verdict, witnesses, locality, or effects are incomplete")
        if any(value not in self.candidate_order for value in (*self.qualified_order, *self.blocked_order, *self.unknown_order)):
            raise ResearchContractError("retrieval assurance state is not covered by candidates")
        for values in (self.candidate_order, self.qualified_order, self.blocked_order, self.unknown_order, self.witness_order, self.counterexample_order, self.omissions, self.uncertainty, self.negative_evidence, self.effect_receipts):
            if tuple(sorted(set(values))) != values:
                raise ResearchContractError("retrieval assurance ordering is not canonical")
        for value in (self.synthesis_digest, self.verification_digest, self.replay_identity, self.artifact.get("content_hash")):
            if not isinstance(value, str) or not re.fullmatch(r"[0-9a-f]{64}", value):
                raise ResearchContractError("retrieval assurance digest is invalid")
        if any(not effect.startswith("assurance:local-retrieval:") and effect != "block:unsafe-release" for effect in self.effect_receipts):
            raise ResearchContractError("retrieval assurance effect is outside the local release gate")

    def digest(self) -> str:
        self.validate()
        return research_artifact_digest({"schema_version": self.schema_version, "contract_version": self.contract_version, "feature_id": self.feature_id, "request_id": self.request_id, "study_id": self.study_id, "scope": self.scope, "verdict": self.verdict, "candidate_order": list(self.candidate_order), "qualified_order": list(self.qualified_order), "blocked_order": list(self.blocked_order), "unknown_order": list(self.unknown_order), "witness_order": list(self.witness_order), "counterexample_order": list(self.counterexample_order), "synthesis_digest": self.synthesis_digest, "verification_digest": self.verification_digest, "replay_identity": self.replay_identity, "omissions": list(self.omissions), "uncertainty": list(self.uncertainty), "negative_evidence": list(self.negative_evidence), "effect_receipts": list(self.effect_receipts), "artifact": dict(self.artifact), "raw_data_local": self.raw_data_local, "boundary": self.boundary})
