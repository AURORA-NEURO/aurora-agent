"""Multimodal context research workbench parity contract."""
from __future__ import annotations

from dataclasses import dataclass
import re
from typing import Any, Mapping, Sequence

from .research_contracts import (
    MULTIMODAL_CONTEXT_WORKBENCH_CONTRACT_VERSION,
    MULTIMODAL_CONTEXT_WORKBENCH_FEATURE_ID,
    PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
    ResearchContractError,
    research_artifact_digest,
)


@dataclass(frozen=True)
class MultimodalContextWorkbenchCell:
    study_id: str
    modality: str
    artifact_digest: str
    semantic_digest: str
    replay_identity: str
    state: str = "supported"
    comparable: bool = True
    raw_data_local: bool = True
    boundary: str = PRECLINICAL_BOUNDARY


@dataclass(frozen=True)
class MultimodalContextWorkbenchReceipt:
    session_id: str
    query_id: str
    goal: str
    disposition: str
    study_order: tuple[str, ...]
    modality_order: tuple[str, ...]
    cell_order: tuple[str, ...]
    qualified_cell_order: tuple[str, ...]
    missing_cell_order: tuple[str, ...]
    incompatible_cell_order: tuple[str, ...]
    unknown_cell_order: tuple[str, ...]
    view_order: tuple[str, ...]
    action_order: tuple[str, ...]
    blocked_action_order: tuple[str, ...]
    context_digest: str
    section_digest: str
    replay_identity: str
    omissions: tuple[str, ...]
    uncertainty: tuple[str, ...]
    negative_evidence: tuple[str, ...]
    effect_receipts: tuple[str, ...]
    artifact: Mapping[str, Any]
    feature_id: str = MULTIMODAL_CONTEXT_WORKBENCH_FEATURE_ID
    contract_version: str = MULTIMODAL_CONTEXT_WORKBENCH_CONTRACT_VERSION
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    raw_data_local: bool = True
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION or self.contract_version != MULTIMODAL_CONTEXT_WORKBENCH_CONTRACT_VERSION or self.feature_id != MULTIMODAL_CONTEXT_WORKBENCH_FEATURE_ID:
            raise ResearchContractError("multimodal workbench schema, feature, or version mismatch")
        if self.boundary != PRECLINICAL_BOUNDARY or not self.raw_data_local or not self.session_id.strip() or not self.query_id.strip() or not self.goal.strip() or len(self.study_order) < 2 or len(self.modality_order) < 2 or not self.cell_order or not self.view_order or not self.action_order or not self.effect_receipts or self.disposition not in {"ready", "needs_refinement", "blocked"}:
            raise ResearchContractError("multimodal workbench identity, cell closure, view, action, locality, disposition, or effects are incomplete")
        for values in (self.study_order, self.modality_order, self.cell_order, self.qualified_cell_order, self.missing_cell_order, self.incompatible_cell_order, self.unknown_cell_order, self.view_order, self.action_order, self.blocked_action_order, self.omissions, self.uncertainty, self.negative_evidence, self.effect_receipts):
            if tuple(sorted(set(values))) != values:
                raise ResearchContractError("multimodal workbench vectors are not canonical")
        classified = set(self.qualified_cell_order) | set(self.missing_cell_order) | set(self.incompatible_cell_order) | set(self.unknown_cell_order)
        if classified != set(self.cell_order):
            raise ResearchContractError("multimodal cells do not partition outcomes")
        for value in (self.context_digest, self.section_digest, self.replay_identity, self.artifact.get("content_hash")):
            if not isinstance(value, str) or not re.fullmatch(r"[0-9a-f]{64}", value):
                raise ResearchContractError("multimodal workbench digest is invalid")
        if any(not effect.startswith("view:local-multimodal-workbench:") and effect != "block:unsafe-release" for effect in self.effect_receipts):
            raise ResearchContractError("multimodal workbench effect is outside read-only view gate")

    def digest(self) -> str:
        self.validate()
        return research_artifact_digest({"schema_version": self.schema_version, "contract_version": self.contract_version, "feature_id": self.feature_id, "session_id": self.session_id, "query_id": self.query_id, "goal": self.goal, "disposition": self.disposition, "study_order": list(self.study_order), "modality_order": list(self.modality_order), "cell_order": list(self.cell_order), "qualified_cell_order": list(self.qualified_cell_order), "missing_cell_order": list(self.missing_cell_order), "incompatible_cell_order": list(self.incompatible_cell_order), "unknown_cell_order": list(self.unknown_cell_order), "view_order": list(self.view_order), "action_order": list(self.action_order), "blocked_action_order": list(self.blocked_action_order), "context_digest": self.context_digest, "section_digest": self.section_digest, "replay_identity": self.replay_identity, "omissions": list(self.omissions), "uncertainty": list(self.uncertainty), "negative_evidence": list(self.negative_evidence), "effect_receipts": list(self.effect_receipts), "artifact": dict(self.artifact), "raw_data_local": self.raw_data_local, "boundary": self.boundary})


def render_multimodal_context_workbench(*, session_id: str, query_id: str, goal: str, projection_disposition: str, study_ids: Sequence[str], required_modalities: Sequence[str], cells: Sequence[MultimodalContextWorkbenchCell], context_digest: str, section_digest: str, replay_identity: str, policy_allow: bool = True, raw_data_local: bool = True) -> MultimodalContextWorkbenchReceipt:
    if not session_id.strip() or not query_id.strip() or not goal.strip() or len(study_ids) < 2 or len(required_modalities) < 2 or any(not re.fullmatch(r"[0-9a-f]{64}", value) for value in (context_digest, section_digest, replay_identity)):
        raise ResearchContractError("multimodal workbench identity, closure, digest, or boundary is invalid")
    studies = tuple(sorted(set(study_ids))); modalities = tuple(sorted(set(required_modalities)))
    if len(studies) != len(study_ids) or len(modalities) != len(required_modalities) or any(not value.strip() for value in (*studies, *modalities)):
        raise ResearchContractError("study and modality identifiers must be unique and non-empty")
    cell_map: dict[str, MultimodalContextWorkbenchCell] = {}
    for cell in cells:
        key = f"{cell.study_id}|{cell.modality}"
        if key in cell_map:
            raise ResearchContractError("multimodal workbench cells must be unique")
        cell_map[key] = cell
    cell_order = {f"{study}|{modality}" for study in studies for modality in modalities}; qualified: set[str] = set(); missing: set[str] = set(); incompatible: set[str] = set(); unknown: set[str] = set(); views = {"view:multimodal-summary", "view:comparability-matrix", "view:evidence-lineage", "view:replay-identity"}; actions = {"action:inspect-cell", "action:replay-local-projection"}; blocked_actions: set[str] = set(); omissions: set[str] = set(); uncertainty: set[str] = set(); negative: set[str] = set()
    for key in cell_order:
        cell = cell_map.get(key)
        if cell is None:
            missing.add(key); omissions.add(f"cell:{key}:missing-modality")
        elif not policy_allow or not raw_data_local or not cell.raw_data_local or cell.boundary != PRECLINICAL_BOUNDARY:
            incompatible.add(key); omissions.add(f"cell:{key}:policy-locality-blocked")
        elif not cell.comparable:
            incompatible.add(key); negative.add(f"cell:{key}:incomparable")
        elif cell.replay_identity != replay_identity:
            unknown.add(key); uncertainty.add(f"cell:{key}:replay-mismatch")
        elif cell.state in {"proven", "supported"}:
            qualified.add(key)
        elif cell.state in {"speculative", "unknown"}:
            unknown.add(key); uncertainty.add(f"cell:{key}:evidence-uncertain")
        else:
            incompatible.add(key); negative.add(f"cell:{key}:contradicted")
    if not policy_allow or not raw_data_local:
        omissions.add("workbench:policy-or-locality-blocked"); disposition = "blocked"; blocked_actions.update({"action:open-decision-section", "action:export-local-research-object", "action:replay-local-projection"}); actions = {"action:inspect-block-reason"}
    elif projection_disposition == "admitted" and len(qualified) == len(cell_order):
        actions.update({"action:open-decision-section", "action:export-local-research-object"}); disposition = "ready"
    else:
        actions.update({"action:review-comparability", "action:request-modality-refinement"}); uncertainty.add("workbench:multimodal-projection-not-admitted"); disposition = "needs_refinement"
    if missing: views.add("view:missing-modalities")
    if incompatible: views.add("view:incompatibility-evidence")
    if unknown: views.add("view:uncertain-cells")
    effects = ("block:unsafe-release",) if disposition == "blocked" else (f"view:local-multimodal-workbench:{session_id}",); artifact = {"content_hash": research_artifact_digest({"session_id": session_id, "section_digest": section_digest}), "media_type": "application/vnd.aurora.multimodal-context-workbench+json"}
    receipt = MultimodalContextWorkbenchReceipt(session_id=session_id, query_id=query_id, goal=goal, disposition=disposition, study_order=studies, modality_order=modalities, cell_order=tuple(sorted(cell_order)), qualified_cell_order=tuple(sorted(qualified)), missing_cell_order=tuple(sorted(missing)), incompatible_cell_order=tuple(sorted(incompatible)), unknown_cell_order=tuple(sorted(unknown)), view_order=tuple(sorted(views)), action_order=tuple(sorted(actions)), blocked_action_order=tuple(sorted(blocked_actions)), context_digest=context_digest, section_digest=section_digest, replay_identity=replay_identity, omissions=tuple(sorted(omissions)), uncertainty=tuple(sorted(uncertainty)), negative_evidence=tuple(sorted(negative)), effect_receipts=effects, artifact=artifact)
    receipt.validate(); return receipt
