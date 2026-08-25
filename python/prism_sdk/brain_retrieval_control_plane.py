"""Python parity contract for the local retrieval federated control plane."""
from __future__ import annotations

from dataclasses import dataclass
import re
from typing import Any, Mapping

from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError, research_artifact_digest

RETRIEVAL_CONTROL_PLANE_FEATURE_ID = "AFA-brain-P02-F29"
RETRIEVAL_CONTROL_PLANE_CONTRACT_VERSION = "brain-retrieval-federated-control-plane/1.0"
RETRIEVAL_CONTROL_ACTION_ORDER = ("control:observe", "control:reconcile", "control:authorize", "control:publish")


@dataclass(frozen=True)
class BrainRetrievalFederatedControlPlaneReceipt:
    request_id: str
    plane_id: str
    session_id: str
    disposition: str
    action_order: tuple[str, ...]
    completed_action_order: tuple[str, ...]
    blocked_action_order: tuple[str, ...]
    compensation_order: tuple[str, ...]
    candidate_order: tuple[str, ...]
    ranked_order: tuple[str, ...]
    qualified_order: tuple[str, ...]
    blocked_order: tuple[str, ...]
    unknown_order: tuple[str, ...]
    synthesis_digest: str
    control_digest: str
    replay_identity: str
    budget_units: int
    omissions: tuple[str, ...]
    uncertainty: tuple[str, ...]
    negative_evidence: tuple[str, ...]
    effect_receipts: tuple[str, ...]
    artifact: Mapping[str, Any]
    feature_id: str = RETRIEVAL_CONTROL_PLANE_FEATURE_ID
    contract_version: str = RETRIEVAL_CONTROL_PLANE_CONTRACT_VERSION
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    raw_data_local: bool = True
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION or self.feature_id != RETRIEVAL_CONTROL_PLANE_FEATURE_ID or self.contract_version != RETRIEVAL_CONTROL_PLANE_CONTRACT_VERSION:
            raise ResearchContractError("retrieval control-plane schema, feature, or version mismatch")
        if self.boundary != PRECLINICAL_BOUNDARY or not self.raw_data_local or not self.request_id.strip() or not self.plane_id.strip() or not self.session_id.strip() or self.disposition not in {"qualified", "partial", "unknown", "blocked"} or self.action_order != RETRIEVAL_CONTROL_ACTION_ORDER or not self.completed_action_order or not self.candidate_order or not self.effect_receipts or self.budget_units <= 0:
            raise ResearchContractError("control-plane identity, actions, retrieval, locality, budget, or effects are incomplete")
        if any(value not in self.candidate_order for value in (*self.ranked_order, *self.qualified_order, *self.blocked_order, *self.unknown_order)):
            raise ResearchContractError("control-plane evidence state is not covered by candidates")
        positions = {action: index for index, action in enumerate(RETRIEVAL_CONTROL_ACTION_ORDER)}
        for values in (self.completed_action_order, self.blocked_action_order):
            if any(value not in positions for value in values) or any(positions[left] >= positions[right] for left, right in zip(values, values[1:])) or set(self.completed_action_order).intersection(self.blocked_action_order):
                raise ResearchContractError("control-plane action transcript is not canonical")
        for values in (self.compensation_order, self.candidate_order, self.ranked_order, self.qualified_order, self.blocked_order, self.unknown_order, self.omissions, self.uncertainty, self.negative_evidence, self.effect_receipts):
            if tuple(sorted(set(values))) != values:
                raise ResearchContractError("control-plane ordering is not canonical")
        for value in (self.synthesis_digest, self.control_digest, self.replay_identity, self.artifact.get("content_hash")):
            if not isinstance(value, str) or not re.fullmatch(r"[0-9a-f]{64}", value):
                raise ResearchContractError("control-plane digest is invalid")
        if any(not effect.startswith("manage:local-retrieval-control:") and effect != "block:unsafe-release" for effect in self.effect_receipts):
            raise ResearchContractError("control-plane effect is outside local management gate")

    def digest(self) -> str:
        self.validate()
        return research_artifact_digest({"schema_version": self.schema_version, "contract_version": self.contract_version, "feature_id": self.feature_id, "request_id": self.request_id, "plane_id": self.plane_id, "session_id": self.session_id, "disposition": self.disposition, "action_order": list(self.action_order), "completed_action_order": list(self.completed_action_order), "blocked_action_order": list(self.blocked_action_order), "compensation_order": list(self.compensation_order), "candidate_order": list(self.candidate_order), "ranked_order": list(self.ranked_order), "qualified_order": list(self.qualified_order), "blocked_order": list(self.blocked_order), "unknown_order": list(self.unknown_order), "synthesis_digest": self.synthesis_digest, "control_digest": self.control_digest, "replay_identity": self.replay_identity, "budget_units": self.budget_units, "omissions": list(self.omissions), "uncertainty": list(self.uncertainty), "negative_evidence": list(self.negative_evidence), "effect_receipts": list(self.effect_receipts), "artifact": dict(self.artifact), "raw_data_local": self.raw_data_local, "boundary": self.boundary})
