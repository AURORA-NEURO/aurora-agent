"""Python mirror of the aggregate-only federated evidence copilot receipt."""

from __future__ import annotations

from dataclasses import dataclass
import re
from typing import Any, Mapping

from .research_contracts import (
    FEDERATED_EVIDENCE_COPILOT_CONTRACT_VERSION,
    FEDERATED_EVIDENCE_COPILOT_FEATURE_ID,
    PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
    ResearchContractError,
    research_artifact_digest,
)


@dataclass(frozen=True)
class BrainFederatedEvidenceResearchCopilotReceipt:
    request_id: str
    operator_id: str
    federation_id: str
    institution_id: str
    purpose: str
    semantic_profile: str
    endpoint: str
    disposition: str
    plan_order: tuple[str, ...]
    action_order: tuple[str, ...]
    tool_order: tuple[str, ...]
    candidate_order: tuple[str, ...]
    admitted_order: tuple[str, ...]
    blocked_order: tuple[str, ...]
    unknown_order: tuple[str, ...]
    aggregate_order: tuple[str, ...]
    envelope_digest: str
    evidence_receipt_digest: str
    plan_digest: str
    approval_reference: str
    replay_identity: str
    budget_units: int
    omissions: tuple[str, ...]
    uncertainty: tuple[str, ...]
    negative_evidence: tuple[str, ...]
    effect_receipts: tuple[str, ...]
    artifact: Mapping[str, Any]
    feature_id: str = FEDERATED_EVIDENCE_COPILOT_FEATURE_ID
    contract_version: str = FEDERATED_EVIDENCE_COPILOT_CONTRACT_VERSION
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    raw_data_local: bool = True
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION or self.feature_id != FEDERATED_EVIDENCE_COPILOT_FEATURE_ID or self.contract_version != FEDERATED_EVIDENCE_COPILOT_CONTRACT_VERSION:
            raise ResearchContractError("federated copilot schema, feature, or version mismatch")
        if self.boundary != PRECLINICAL_BOUNDARY or not self.raw_data_local or not self.request_id.strip() or not self.operator_id.strip() or not self.federation_id.strip() or not self.institution_id.strip() or not self.purpose.strip() or not self.semantic_profile.strip() or not self.endpoint.strip() or not self.candidate_order or not self.plan_order or not self.action_order or len(self.plan_order) != len(self.action_order) or not self.tool_order or not self.effect_receipts or self.budget_units <= 0:
            raise ResearchContractError("federated copilot identity, envelope, bounded plan, tool, locality, budget, or effects are incomplete")
        if self.disposition not in {"qualified", "partial", "unknown", "blocked"}:
            raise ResearchContractError("federated copilot disposition is unknown")
        if any(value not in self.candidate_order for value in (*self.admitted_order, *self.blocked_order, *self.unknown_order)):
            raise ResearchContractError("federated copilot state is not covered by candidate order")
        for values in (self.plan_order, self.action_order, self.tool_order, self.candidate_order, self.admitted_order, self.blocked_order, self.unknown_order, self.aggregate_order, self.omissions, self.uncertainty, self.negative_evidence, self.effect_receipts):
            if tuple(sorted(set(values))) != values:
                raise ResearchContractError("federated copilot ordering is invalid")
        for value in (self.aggregate_order, (self.envelope_digest, self.evidence_receipt_digest, self.plan_digest, self.approval_reference, self.replay_identity, self.artifact.get("content_hash"))):
            values = value if isinstance(value, tuple) else (value,)
            for digest in values:
                if not isinstance(digest, str) or not re.fullmatch(r"[0-9a-f]{64}", digest):
                    raise ResearchContractError("federated copilot digest is invalid")
        if any(not effect.startswith("invoke:declared-tool:") and effect != "block:unsafe-release" for effect in self.effect_receipts):
            raise ResearchContractError("federated copilot effect is outside declared-tool gate")
        if self.disposition != "blocked" and self.admitted_order and not any(effect.startswith("invoke:declared-tool:") for effect in self.effect_receipts):
            raise ResearchContractError("admitted federation requires a declared-tool receipt")
        if self.disposition not in {"qualified", "partial"} and self.effect_receipts != ("block:unsafe-release",):
            raise ResearchContractError("non-admitted federation must be explicitly blocked")

    def digest(self) -> str:
        self.validate()
        return research_artifact_digest({
            "schema_version": self.schema_version, "contract_version": self.contract_version, "feature_id": self.feature_id,
            "request_id": self.request_id, "operator_id": self.operator_id, "federation_id": self.federation_id, "institution_id": self.institution_id,
            "purpose": self.purpose, "semantic_profile": self.semantic_profile, "endpoint": self.endpoint, "disposition": self.disposition,
            "plan_order": list(self.plan_order), "action_order": list(self.action_order), "tool_order": list(self.tool_order), "candidate_order": list(self.candidate_order),
            "admitted_order": list(self.admitted_order), "blocked_order": list(self.blocked_order), "unknown_order": list(self.unknown_order), "aggregate_order": list(self.aggregate_order),
            "envelope_digest": self.envelope_digest, "evidence_receipt_digest": self.evidence_receipt_digest, "plan_digest": self.plan_digest, "approval_reference": self.approval_reference,
            "replay_identity": self.replay_identity, "budget_units": self.budget_units, "omissions": list(self.omissions), "uncertainty": list(self.uncertainty), "negative_evidence": list(self.negative_evidence),
            "effect_receipts": list(self.effect_receipts), "artifact": dict(self.artifact), "raw_data_local": self.raw_data_local, "boundary": self.boundary,
        })
