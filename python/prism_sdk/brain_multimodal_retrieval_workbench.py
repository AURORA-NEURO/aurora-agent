"""Python parity contract for the multimodal retrieval researcher workbench."""
from __future__ import annotations

from dataclasses import dataclass
import re
from typing import Any, Mapping

from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError, research_artifact_digest

MULTIMODAL_RETRIEVAL_WORKBENCH_FEATURE_ID = "AFA-brain-P02-F18"
MULTIMODAL_RETRIEVAL_WORKBENCH_CONTRACT_VERSION = "brain-multimodal-retrieval-research-workbench/1.0"


@dataclass(frozen=True)
class BrainMultimodalRetrievalWorkbenchReceipt:
    request_id: str
    workspace_id: str
    scope: str
    study_order: tuple[str, ...]
    modality_order: tuple[str, ...]
    disposition: str
    view_order: tuple[str, ...]
    panel_order: tuple[str, ...]
    action_receipts: tuple[str, ...]
    candidate_order: tuple[str, ...]
    ranked_order: tuple[str, ...]
    qualified_order: tuple[str, ...]
    blocked_order: tuple[str, ...]
    unknown_order: tuple[str, ...]
    comparability_digest: str
    synthesis_digest: str
    workbench_digest: str
    replay_identity: str
    budget_units: int
    omissions: tuple[str, ...]
    uncertainty: tuple[str, ...]
    negative_evidence: tuple[str, ...]
    effect_receipts: tuple[str, ...]
    artifact: Mapping[str, Any]
    feature_id: str = MULTIMODAL_RETRIEVAL_WORKBENCH_FEATURE_ID
    contract_version: str = MULTIMODAL_RETRIEVAL_WORKBENCH_CONTRACT_VERSION
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    raw_data_local: bool = True
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION or self.feature_id != MULTIMODAL_RETRIEVAL_WORKBENCH_FEATURE_ID or self.contract_version != MULTIMODAL_RETRIEVAL_WORKBENCH_CONTRACT_VERSION:
            raise ResearchContractError("multimodal workbench schema, feature, or version mismatch")
        if self.boundary != PRECLINICAL_BOUNDARY or not self.raw_data_local or not self.request_id.strip() or not self.workspace_id.strip() or not self.scope.strip() or len(self.study_order) < 2 or len(self.modality_order) < 2 or not self.view_order or not self.panel_order or not self.action_receipts or not self.candidate_order or not self.effect_receipts or self.budget_units <= 0:
            raise ResearchContractError("multimodal workbench identity, coverage, views, panels, retrieval, locality, budget, or effects are incomplete")
        if any(value not in self.candidate_order for value in (*self.ranked_order, *self.qualified_order, *self.blocked_order, *self.unknown_order)):
            raise ResearchContractError("multimodal workbench state is not covered by candidates")
        for values in (self.study_order, self.modality_order, self.view_order, self.panel_order, self.action_receipts, self.candidate_order, self.ranked_order, self.qualified_order, self.blocked_order, self.unknown_order, self.omissions, self.uncertainty, self.negative_evidence, self.effect_receipts):
            if tuple(sorted(set(values))) != values:
                raise ResearchContractError("multimodal workbench ordering is not canonical")
        for value in (self.comparability_digest, self.synthesis_digest, self.workbench_digest, self.replay_identity, self.artifact.get("content_hash")):
            if not isinstance(value, str) or not re.fullmatch(r"[0-9a-f]{64}", value):
                raise ResearchContractError("multimodal workbench digest is invalid")
        if any(not effect.startswith("view:local-multimodal-retrieval-artifacts:") and effect != "block:unsafe-release" for effect in self.effect_receipts):
            raise ResearchContractError("multimodal workbench effect is not read-only")

    def digest(self) -> str:
        self.validate()
        return research_artifact_digest({
            "schema_version": self.schema_version, "contract_version": self.contract_version, "feature_id": self.feature_id,
            "request_id": self.request_id, "workspace_id": self.workspace_id, "scope": self.scope, "study_order": list(self.study_order), "modality_order": list(self.modality_order), "disposition": self.disposition,
            "view_order": list(self.view_order), "panel_order": list(self.panel_order), "action_receipts": list(self.action_receipts), "candidate_order": list(self.candidate_order), "ranked_order": list(self.ranked_order), "qualified_order": list(self.qualified_order), "blocked_order": list(self.blocked_order), "unknown_order": list(self.unknown_order), "comparability_digest": self.comparability_digest, "synthesis_digest": self.synthesis_digest, "workbench_digest": self.workbench_digest, "replay_identity": self.replay_identity, "budget_units": self.budget_units, "omissions": list(self.omissions), "uncertainty": list(self.uncertainty), "negative_evidence": list(self.negative_evidence), "effect_receipts": list(self.effect_receipts), "artifact": dict(self.artifact), "raw_data_local": self.raw_data_local, "boundary": self.boundary,
        })
