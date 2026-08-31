"""Local context research workbench parity contract."""
from __future__ import annotations

from dataclasses import dataclass
import re
from typing import Any, Mapping, Sequence

from .research_contracts import (
    CONTEXT_RESEARCH_WORKBENCH_CONTRACT_VERSION,
    CONTEXT_RESEARCH_WORKBENCH_FEATURE_ID,
    PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
    ResearchContractError,
    research_artifact_digest,
)


@dataclass(frozen=True)
class ContextWorkbenchReceipt:
    session_id: str
    query_id: str
    goal: str
    disposition: str
    view_order: tuple[str, ...]
    action_order: tuple[str, ...]
    blocked_action_order: tuple[str, ...]
    selected_context_order: tuple[str, ...]
    unresolved_obligation_order: tuple[str, ...]
    refinement_frontier_order: tuple[str, ...]
    context_digest: str
    section_digest: str
    replay_identity: str
    omissions: tuple[str, ...]
    uncertainty: tuple[str, ...]
    negative_evidence: tuple[str, ...]
    effect_receipts: tuple[str, ...]
    artifact: Mapping[str, Any]
    feature_id: str = CONTEXT_RESEARCH_WORKBENCH_FEATURE_ID
    contract_version: str = CONTEXT_RESEARCH_WORKBENCH_CONTRACT_VERSION
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    raw_data_local: bool = True
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION or self.contract_version != CONTEXT_RESEARCH_WORKBENCH_CONTRACT_VERSION or self.feature_id != CONTEXT_RESEARCH_WORKBENCH_FEATURE_ID:
            raise ResearchContractError("workbench schema, feature, or version mismatch")
        if self.boundary != PRECLINICAL_BOUNDARY or not self.raw_data_local or not self.session_id.strip() or not self.query_id.strip() or not self.goal.strip() or not self.view_order or not self.action_order or not self.effect_receipts or self.disposition not in {"ready", "needs_refinement", "blocked"}:
            raise ResearchContractError("workbench identity, view, action, locality, disposition, or effects are incomplete")
        for values in (self.view_order, self.action_order, self.blocked_action_order, self.selected_context_order, self.unresolved_obligation_order, self.refinement_frontier_order, self.omissions, self.uncertainty, self.negative_evidence, self.effect_receipts):
            if tuple(sorted(set(values))) != values:
                raise ResearchContractError("workbench vectors are not canonical")
        for value in (self.context_digest, self.section_digest, self.replay_identity, self.artifact.get("content_hash")):
            if not isinstance(value, str) or not re.fullmatch(r"[0-9a-f]{64}", value):
                raise ResearchContractError("workbench digest is invalid")
        if any(not effect.startswith("view:local-context-workbench:") and effect != "block:unsafe-release" for effect in self.effect_receipts):
            raise ResearchContractError("workbench effect is outside read-only view gate")

    def digest(self) -> str:
        self.validate()
        return research_artifact_digest({"schema_version": self.schema_version, "contract_version": self.contract_version, "feature_id": self.feature_id, "session_id": self.session_id, "query_id": self.query_id, "goal": self.goal, "disposition": self.disposition, "view_order": list(self.view_order), "action_order": list(self.action_order), "blocked_action_order": list(self.blocked_action_order), "selected_context_order": list(self.selected_context_order), "unresolved_obligation_order": list(self.unresolved_obligation_order), "refinement_frontier_order": list(self.refinement_frontier_order), "context_digest": self.context_digest, "section_digest": self.section_digest, "replay_identity": self.replay_identity, "omissions": list(self.omissions), "uncertainty": list(self.uncertainty), "negative_evidence": list(self.negative_evidence), "effect_receipts": list(self.effect_receipts), "artifact": dict(self.artifact), "raw_data_local": self.raw_data_local, "boundary": self.boundary})


def render_context_workbench(*, session_id: str, query_id: str, goal: str, projection_disposition: str, selected_context_ids: Sequence[str], unresolved_obligation_ids: Sequence[str], refinement_frontier_ids: Sequence[str], context_digest: str, section_digest: str, replay_identity: str, policy_allow: bool = True, raw_data_local: bool = True) -> ContextWorkbenchReceipt:
    if not session_id.strip() or not query_id.strip() or not goal.strip() or not selected_context_ids or any(not re.fullmatch(r"[0-9a-f]{64}", value) for value in (context_digest, section_digest, replay_identity)):
        raise ResearchContractError("workbench identity, selected context, digest, or boundary is invalid")
    selected = tuple(sorted(set(selected_context_ids))); obligations = tuple(sorted(set(unresolved_obligation_ids))); frontier = tuple(sorted(set(refinement_frontier_ids)))
    if len(selected) != len(selected_context_ids) or any(not value.strip() for value in selected):
        raise ResearchContractError("workbench selected context identifiers must be unique and non-empty")
    views = {"view:context-summary", "view:evidence-lineage", "view:replay-identity", "view:uncertainty-and-omissions"}; actions = {"action:inspect-context", "action:replay-local-projection"}; blocked_actions: set[str] = set(); omissions: set[str] = set(); uncertainty: set[str] = set(); negative: set[str] = set()
    if not policy_allow or not raw_data_local:
        omissions.add("workbench:policy-or-locality-blocked"); disposition = "blocked"; blocked_actions.update({"action:open-decision-section", "action:export-local-research-object", "action:replay-local-projection"}); actions = {"action:inspect-block-reason"}
    elif projection_disposition == "admitted" and not obligations:
        actions.update({"action:open-decision-section", "action:export-local-research-object"}); disposition = "ready"
    else:
        actions.update({"action:review-omissions", "action:request-context-refinement"}); uncertainty.add("workbench:projection-not-admitted"); disposition = "needs_refinement"
    if obligations: views.add("view:unresolved-obligations")
    if frontier: views.add("view:refinement-frontier")
    effects = ("block:unsafe-release",) if disposition == "blocked" else (f"view:local-context-workbench:{session_id}",)
    artifact = {"content_hash": research_artifact_digest({"session_id": session_id, "section_digest": section_digest}), "media_type": "application/vnd.aurora.context-workbench+json"}
    receipt = ContextWorkbenchReceipt(session_id=session_id, query_id=query_id, goal=goal, disposition=disposition, view_order=tuple(sorted(views)), action_order=tuple(sorted(actions)), blocked_action_order=tuple(sorted(blocked_actions)), selected_context_order=selected, unresolved_obligation_order=obligations, refinement_frontier_order=frontier, context_digest=context_digest, section_digest=section_digest, replay_identity=replay_identity, omissions=tuple(sorted(omissions)), uncertainty=tuple(sorted(uncertainty)), negative_evidence=tuple(sorted(negative)), effect_receipts=effects, artifact=artifact)
    receipt.validate(); return receipt
