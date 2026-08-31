"""Python parity contract for the multimodal retrieval workflow fabric."""
from __future__ import annotations

from dataclasses import dataclass
import re
from typing import Any, Mapping

from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError, research_artifact_digest

MULTIMODAL_RETRIEVAL_WORKFLOW_FABRIC_FEATURE_ID = "AFA-brain-P02-F14"
MULTIMODAL_RETRIEVAL_WORKFLOW_FABRIC_CONTRACT_VERSION = "brain-multimodal-retrieval-workflow-fabric/1.0"


@dataclass(frozen=True)
class BrainMultimodalRetrievalWorkflowFabricReceipt:
    request_id: str
    workflow_id: str
    scope: str
    study_order: tuple[str, ...]
    modality_order: tuple[str, ...]
    disposition: str
    stage_order: tuple[str, ...]
    plan_order: tuple[str, ...]
    completed_order: tuple[str, ...]
    blocked_order: tuple[str, ...]
    compensation_order: tuple[str, ...]
    candidate_order: tuple[str, ...]
    ranked_order: tuple[str, ...]
    qualified_order: tuple[str, ...]
    unknown_order: tuple[str, ...]
    comparability_digest: str
    synthesis_digest: str
    checkpoint_digest: str
    workflow_digest: str
    replay_identity: str
    budget_units: int
    omissions: tuple[str, ...]
    uncertainty: tuple[str, ...]
    negative_evidence: tuple[str, ...]
    effect_receipts: tuple[str, ...]
    artifact: Mapping[str, Any]
    feature_id: str = MULTIMODAL_RETRIEVAL_WORKFLOW_FABRIC_FEATURE_ID
    contract_version: str = MULTIMODAL_RETRIEVAL_WORKFLOW_FABRIC_CONTRACT_VERSION
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    raw_data_local: bool = True
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION or self.feature_id != MULTIMODAL_RETRIEVAL_WORKFLOW_FABRIC_FEATURE_ID or self.contract_version != MULTIMODAL_RETRIEVAL_WORKFLOW_FABRIC_CONTRACT_VERSION:
            raise ResearchContractError("multimodal retrieval workflow schema, feature, or version mismatch")
        if self.boundary != PRECLINICAL_BOUNDARY or not self.raw_data_local or not self.request_id.strip() or not self.workflow_id.strip() or not self.scope.strip() or len(self.study_order) < 2 or len(self.modality_order) < 2 or not self.stage_order or not self.plan_order or not self.completed_order or not self.effect_receipts or self.budget_units <= 0:
            raise ResearchContractError("multimodal workflow identity, coverage, stages, plan, locality, budget, or effects are incomplete")
        if any(value not in self.candidate_order for value in (*self.ranked_order, *self.qualified_order, *self.blocked_order, *self.unknown_order)):
            raise ResearchContractError("multimodal workflow state is not covered by candidates")
        for values in (self.study_order, self.modality_order, self.stage_order, self.plan_order, self.completed_order, self.blocked_order, self.compensation_order, self.candidate_order, self.ranked_order, self.qualified_order, self.unknown_order, self.omissions, self.uncertainty, self.negative_evidence, self.effect_receipts):
            if tuple(sorted(set(values))) != values:
                raise ResearchContractError("multimodal workflow ordering is invalid")
        for value in (self.comparability_digest, self.synthesis_digest, self.checkpoint_digest, self.workflow_digest, self.replay_identity, self.artifact.get("content_hash")):
            if not isinstance(value, str) or not re.fullmatch(r"[0-9a-f]{64}", value):
                raise ResearchContractError("multimodal workflow digest is invalid")
        if any(not effect.startswith("schedule:multimodal-retrieval-work:") and not effect.startswith("compensate:multimodal-retrieval-work:") and effect != "block:unsafe-release" for effect in self.effect_receipts):
            raise ResearchContractError("multimodal workflow effect is outside schedule/compensation gate")
        if self.disposition == "qualified" and not any(effect.startswith("schedule:multimodal-retrieval-work:") for effect in self.effect_receipts):
            raise ResearchContractError("qualified multimodal workflow requires schedule receipt")

    def digest(self) -> str:
        self.validate()
        return research_artifact_digest({
            "schema_version": self.schema_version, "contract_version": self.contract_version, "feature_id": self.feature_id,
            "request_id": self.request_id, "workflow_id": self.workflow_id, "scope": self.scope, "study_order": list(self.study_order), "modality_order": list(self.modality_order), "disposition": self.disposition,
            "stage_order": list(self.stage_order), "plan_order": list(self.plan_order), "completed_order": list(self.completed_order), "blocked_order": list(self.blocked_order), "compensation_order": list(self.compensation_order),
            "candidate_order": list(self.candidate_order), "ranked_order": list(self.ranked_order), "qualified_order": list(self.qualified_order), "unknown_order": list(self.unknown_order), "comparability_digest": self.comparability_digest,
            "synthesis_digest": self.synthesis_digest, "checkpoint_digest": self.checkpoint_digest, "workflow_digest": self.workflow_digest, "replay_identity": self.replay_identity, "budget_units": self.budget_units,
            "omissions": list(self.omissions), "uncertainty": list(self.uncertainty), "negative_evidence": list(self.negative_evidence), "effect_receipts": list(self.effect_receipts), "artifact": dict(self.artifact), "raw_data_local": self.raw_data_local, "boundary": self.boundary,
        })
