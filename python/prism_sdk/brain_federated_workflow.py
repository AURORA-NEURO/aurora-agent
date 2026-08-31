"""Python parity contract for the federated continual evidence workflow fabric."""

from __future__ import annotations

from dataclasses import dataclass
import re
from typing import Any, Mapping

from .research_contracts import (
    FEDERATED_EVIDENCE_WORKFLOW_FABRIC_CONTRACT_VERSION,
    FEDERATED_EVIDENCE_WORKFLOW_FABRIC_FEATURE_ID,
    PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
    ResearchContractError,
    research_artifact_digest,
)


@dataclass(frozen=True)
class BrainFederatedEvidenceWorkflowFabricReceipt:
    request_id: str
    workflow_id: str
    federation_id: str
    institution_id: str
    purpose: str
    endpoint: str
    disposition: str
    stage_order: tuple[str, ...]
    plan_order: tuple[str, ...]
    completed_order: tuple[str, ...]
    blocked_order: tuple[str, ...]
    compensation_order: tuple[str, ...]
    candidate_order: tuple[str, ...]
    admitted_order: tuple[str, ...]
    unknown_order: tuple[str, ...]
    aggregate_order: tuple[str, ...]
    checkpoint_digest: str
    workflow_digest: str
    approval_reference: str
    replay_identity: str
    budget_units: int
    omissions: tuple[str, ...]
    uncertainty: tuple[str, ...]
    negative_evidence: tuple[str, ...]
    effect_receipts: tuple[str, ...]
    artifact: Mapping[str, Any]
    feature_id: str = FEDERATED_EVIDENCE_WORKFLOW_FABRIC_FEATURE_ID
    contract_version: str = FEDERATED_EVIDENCE_WORKFLOW_FABRIC_CONTRACT_VERSION
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    raw_data_local: bool = True
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION or self.feature_id != FEDERATED_EVIDENCE_WORKFLOW_FABRIC_FEATURE_ID or self.contract_version != FEDERATED_EVIDENCE_WORKFLOW_FABRIC_CONTRACT_VERSION:
            raise ResearchContractError("federated workflow schema, feature, or version mismatch")
        if self.boundary != PRECLINICAL_BOUNDARY or not self.raw_data_local or not self.request_id.strip() or not self.workflow_id.strip() or not self.federation_id.strip() or not self.institution_id.strip() or not self.purpose.strip() or not self.endpoint.strip() or not self.stage_order or not self.plan_order or not self.completed_order or not self.effect_receipts or self.budget_units <= 0:
            raise ResearchContractError("federated workflow identity, stages, plan, locality, budget, or effects are incomplete")
        if any(value not in self.candidate_order for value in (*self.admitted_order, *self.blocked_order, *self.unknown_order)):
            raise ResearchContractError("federated workflow state is not covered by candidates")
        for values in (self.stage_order, self.plan_order, self.completed_order, self.blocked_order, self.compensation_order, self.candidate_order, self.admitted_order, self.unknown_order, self.aggregate_order, self.omissions, self.uncertainty, self.negative_evidence, self.effect_receipts):
            if tuple(sorted(set(values))) != values:
                raise ResearchContractError("federated workflow ordering is invalid")
        for value in (self.checkpoint_digest, self.workflow_digest, self.approval_reference, self.replay_identity, self.artifact.get("content_hash")):
            if not isinstance(value, str) or not re.fullmatch(r"[0-9a-f]{64}", value):
                raise ResearchContractError("federated workflow digest is invalid")
        if any(not effect.startswith("schedule:research-work:") and not effect.startswith("compensate:research-work:") and effect != "block:unsafe-release" for effect in self.effect_receipts):
            raise ResearchContractError("federated workflow effect is outside schedule/compensation gate")
        if self.disposition == "qualified" and not any(effect.startswith("schedule:research-work:") for effect in self.effect_receipts):
            raise ResearchContractError("qualified federated workflow requires schedule receipt")
        if self.disposition == "blocked" and self.effect_receipts != ("block:unsafe-release",):
            raise ResearchContractError("blocked federated workflow must be explicitly blocked")

    def digest(self) -> str:
        self.validate()
        return research_artifact_digest({
            "schema_version": self.schema_version, "contract_version": self.contract_version, "feature_id": self.feature_id,
            "request_id": self.request_id, "workflow_id": self.workflow_id, "federation_id": self.federation_id,
            "institution_id": self.institution_id, "purpose": self.purpose, "endpoint": self.endpoint, "disposition": self.disposition,
            "stage_order": list(self.stage_order), "plan_order": list(self.plan_order), "completed_order": list(self.completed_order),
            "blocked_order": list(self.blocked_order), "compensation_order": list(self.compensation_order), "candidate_order": list(self.candidate_order),
            "admitted_order": list(self.admitted_order), "unknown_order": list(self.unknown_order), "aggregate_order": list(self.aggregate_order),
            "checkpoint_digest": self.checkpoint_digest, "workflow_digest": self.workflow_digest, "approval_reference": self.approval_reference,
            "replay_identity": self.replay_identity, "budget_units": self.budget_units, "omissions": list(self.omissions),
            "uncertainty": list(self.uncertainty), "negative_evidence": list(self.negative_evidence), "effect_receipts": list(self.effect_receipts),
            "artifact": dict(self.artifact), "raw_data_local": self.raw_data_local, "boundary": self.boundary,
        })
