"""Python mirror of the bounded local evidence research copilot receipt."""

from __future__ import annotations

from dataclasses import dataclass
import re
from typing import Any, Mapping

from .research_contracts import (
    EVIDENCE_RESEARCH_COPILOT_CONTRACT_VERSION,
    EVIDENCE_RESEARCH_COPILOT_FEATURE_ID,
    PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
    ResearchContractError,
    research_artifact_digest,
)


@dataclass(frozen=True)
class BrainEvidenceResearchCopilotReceipt:
    request_id: str
    operator_id: str
    study_id: str
    scope: str
    disposition: str
    plan_order: tuple[str, ...]
    action_order: tuple[str, ...]
    candidate_order: tuple[str, ...]
    qualified_order: tuple[str, ...]
    blocked_order: tuple[str, ...]
    unknown_order: tuple[str, ...]
    evidence_receipt_digest: str
    plan_digest: str
    replay_identity: str
    budget_units: int
    omissions: tuple[str, ...]
    uncertainty: tuple[str, ...]
    negative_evidence: tuple[str, ...]
    effect_receipts: tuple[str, ...]
    artifact: Mapping[str, Any]
    feature_id: str = EVIDENCE_RESEARCH_COPILOT_FEATURE_ID
    contract_version: str = EVIDENCE_RESEARCH_COPILOT_CONTRACT_VERSION
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    raw_data_local: bool = True
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION or self.feature_id != EVIDENCE_RESEARCH_COPILOT_FEATURE_ID or self.contract_version != EVIDENCE_RESEARCH_COPILOT_CONTRACT_VERSION:
            raise ResearchContractError("evidence copilot schema, feature, or version mismatch")
        if self.boundary != PRECLINICAL_BOUNDARY or not self.raw_data_local or not self.request_id.strip() or not self.operator_id.strip() or not self.study_id.strip() or not self.scope.strip() or not self.plan_order or not self.action_order or len(self.plan_order) != len(self.action_order) or not self.effect_receipts or self.budget_units <= 0:
            raise ResearchContractError("evidence copilot identity, bounded plan, locality, budget, or effects are incomplete")
        if self.disposition not in {"qualified", "partial", "unknown", "blocked"}:
            raise ResearchContractError("evidence copilot disposition is unknown")
        if any(value not in self.candidate_order for value in (*self.qualified_order, *self.blocked_order, *self.unknown_order)):
            raise ResearchContractError("evidence copilot state is not covered by candidate order")
        for values in (self.plan_order, self.action_order, self.candidate_order, self.qualified_order, self.blocked_order, self.unknown_order, self.omissions, self.uncertainty, self.negative_evidence, self.effect_receipts):
            if tuple(sorted(set(values))) != values:
                raise ResearchContractError("evidence copilot ordering is invalid")
        for value in (self.evidence_receipt_digest, self.plan_digest, self.replay_identity, self.artifact.get("content_hash")):
            if not isinstance(value, str) or not re.fullmatch(r"[0-9a-f]{64}", value):
                raise ResearchContractError("evidence copilot digest is invalid")
        if any(not effect.startswith("read:local-research-artifacts:") and effect != "block:unsafe-release" for effect in self.effect_receipts):
            raise ResearchContractError("evidence copilot effect is outside local read/compute gate")
        if self.qualified_order and not any(effect.startswith("read:local-research-artifacts:") for effect in self.effect_receipts):
            raise ResearchContractError("qualified copilot plan requires a local read receipt")
        if not self.qualified_order and self.effect_receipts != ("block:unsafe-release",):
            raise ResearchContractError("non-qualified copilot plan must be explicitly blocked")

    def digest(self) -> str:
        self.validate()
        return research_artifact_digest({
            "schema_version": self.schema_version, "contract_version": self.contract_version, "feature_id": self.feature_id,
            "request_id": self.request_id, "operator_id": self.operator_id, "study_id": self.study_id, "scope": self.scope,
            "disposition": self.disposition, "plan_order": list(self.plan_order), "action_order": list(self.action_order),
            "candidate_order": list(self.candidate_order), "qualified_order": list(self.qualified_order), "blocked_order": list(self.blocked_order),
            "unknown_order": list(self.unknown_order), "evidence_receipt_digest": self.evidence_receipt_digest, "plan_digest": self.plan_digest,
            "replay_identity": self.replay_identity, "budget_units": self.budget_units, "omissions": list(self.omissions),
            "uncertainty": list(self.uncertainty), "negative_evidence": list(self.negative_evidence), "effect_receipts": list(self.effect_receipts),
            "artifact": dict(self.artifact), "raw_data_local": self.raw_data_local, "boundary": self.boundary,
        })
