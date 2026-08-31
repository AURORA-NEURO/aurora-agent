"""Multimodal multi-study context workflow fabric parity contract."""
from __future__ import annotations

from dataclasses import dataclass
import re
from typing import Any, Mapping, Sequence

from .research_contracts import (
    MULTIMODAL_CONTEXT_WORKFLOW_FABRIC_CONTRACT_VERSION,
    MULTIMODAL_CONTEXT_WORKFLOW_FABRIC_FEATURE_ID,
    PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
    ResearchContractError,
    research_artifact_digest,
)


@dataclass(frozen=True)
class ModalContextInput:
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
class MultimodalContextWorkflowReceipt:
    request_id: str
    workflow_id: str
    query_id: str
    goal: str
    disposition: str
    study_order: tuple[str, ...]
    modality_order: tuple[str, ...]
    cell_order: tuple[str, ...]
    accepted_order: tuple[str, ...]
    missing_order: tuple[str, ...]
    incompatible_order: tuple[str, ...]
    unknown_order: tuple[str, ...]
    plan_order: tuple[str, ...]
    checkpoint_digest: str
    workflow_digest: str
    replay_identity: str
    budget_units: int
    consumed_budget_units: int
    omissions: tuple[str, ...]
    uncertainty: tuple[str, ...]
    negative_evidence: tuple[str, ...]
    effect_receipts: tuple[str, ...]
    artifact: Mapping[str, Any]
    feature_id: str = MULTIMODAL_CONTEXT_WORKFLOW_FABRIC_FEATURE_ID
    contract_version: str = MULTIMODAL_CONTEXT_WORKFLOW_FABRIC_CONTRACT_VERSION
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    raw_data_local: bool = True
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION or self.contract_version != MULTIMODAL_CONTEXT_WORKFLOW_FABRIC_CONTRACT_VERSION or self.feature_id != MULTIMODAL_CONTEXT_WORKFLOW_FABRIC_FEATURE_ID:
            raise ResearchContractError("multimodal workflow schema, feature, or version mismatch")
        if self.boundary != PRECLINICAL_BOUNDARY or not self.raw_data_local or not self.request_id.strip() or not self.workflow_id.strip() or not self.query_id.strip() or not self.goal.strip() or len(self.study_order) < 2 or len(self.modality_order) < 2 or not self.cell_order or not self.plan_order or self.budget_units < 1 or self.consumed_budget_units > self.budget_units or not self.effect_receipts:
            raise ResearchContractError("multimodal workflow identity, closure, budget, locality, or effects are incomplete")
        for values in (self.study_order, self.modality_order, self.cell_order, self.accepted_order, self.missing_order, self.incompatible_order, self.unknown_order, self.plan_order, self.omissions, self.uncertainty, self.negative_evidence, self.effect_receipts):
            if tuple(sorted(set(values))) != values:
                raise ResearchContractError("multimodal workflow vectors are not canonical")
        classified = set(self.accepted_order) | set(self.missing_order) | set(self.incompatible_order) | set(self.unknown_order)
        if classified != set(self.cell_order):
            raise ResearchContractError("multimodal cells do not partition outcomes")
        for value in (self.checkpoint_digest, self.workflow_digest, self.replay_identity, self.artifact.get("content_hash")):
            if not isinstance(value, str) or not re.fullmatch(r"[0-9a-f]{64}", value):
                raise ResearchContractError("multimodal workflow digest is invalid")
        if any(not effect.startswith("schedule:multimodal-context-workflow:") and not effect.startswith("compensate:multimodal-context-workflow:") and effect != "block:unsafe-release" for effect in self.effect_receipts):
            raise ResearchContractError("multimodal workflow effect is outside schedule/compensation gate")

    def digest(self) -> str:
        self.validate()
        return research_artifact_digest({
            "schema_version": self.schema_version, "contract_version": self.contract_version, "feature_id": self.feature_id,
            "request_id": self.request_id, "workflow_id": self.workflow_id, "query_id": self.query_id, "goal": self.goal,
            "disposition": self.disposition, "study_order": list(self.study_order), "modality_order": list(self.modality_order),
            "cell_order": list(self.cell_order), "accepted_order": list(self.accepted_order), "missing_order": list(self.missing_order),
            "incompatible_order": list(self.incompatible_order), "unknown_order": list(self.unknown_order), "plan_order": list(self.plan_order),
            "checkpoint_digest": self.checkpoint_digest, "workflow_digest": self.workflow_digest, "replay_identity": self.replay_identity,
            "budget_units": self.budget_units, "consumed_budget_units": self.consumed_budget_units, "omissions": list(self.omissions),
            "uncertainty": list(self.uncertainty), "negative_evidence": list(self.negative_evidence), "effect_receipts": list(self.effect_receipts),
            "artifact": dict(self.artifact), "raw_data_local": self.raw_data_local, "boundary": self.boundary,
        })


def compile_multimodal_context_workflow(*, request_id: str, workflow_id: str, query_id: str, goal: str, study_ids: Sequence[str], required_modalities: Sequence[str], inputs: Sequence[ModalContextInput], budget_units: int, replay_identity: str, policy_allow: bool = True, protected_closure: bool = True, raw_data_local: bool = True) -> MultimodalContextWorkflowReceipt:
    if not request_id.strip() or not workflow_id.strip() or not query_id.strip() or not goal.strip() or len(study_ids) < 2 or len(required_modalities) < 2 or budget_units < 1 or not re.fullmatch(r"[0-9a-f]{64}", replay_identity):
        raise ResearchContractError("multimodal workflow identity, closure, budget, replay, or boundary is invalid")
    studies = tuple(sorted(set(study_ids))); modalities = tuple(sorted(set(required_modalities)))
    if len(studies) != len(study_ids) or len(modalities) != len(required_modalities) or any(not value.strip() for value in (*studies, *modalities)):
        raise ResearchContractError("study and modality identifiers must be unique and non-empty")
    input_map: dict[str, ModalContextInput] = {}
    for item in inputs:
        key = f"{item.study_id}|{item.modality}"
        if key in input_map:
            raise ResearchContractError("multimodal input cells must be unique")
        input_map[key] = item
    cells = {f"{study}|{modality}" for study in studies for modality in modalities}; accepted: set[str] = set(); missing: set[str] = set(); incompatible: set[str] = set(); unknown: set[str] = set(); omissions: set[str] = set(); uncertainty: set[str] = set(); negative: set[str] = set()
    for key in cells:
        item = input_map.get(key)
        if item is None:
            missing.add(key); omissions.add(f"cell:{key}:missing-modality")
        elif not policy_allow or not protected_closure or not raw_data_local or not item.raw_data_local or item.boundary != PRECLINICAL_BOUNDARY:
            incompatible.add(key); omissions.add(f"cell:{key}:local-policy-gate-blocked")
        elif not item.comparable:
            incompatible.add(key); negative.add(f"cell:{key}:incomparable")
        elif item.replay_identity != replay_identity:
            unknown.add(key); uncertainty.add(f"cell:{key}:replay-mismatch")
        elif item.state in {"proven", "supported"}:
            accepted.add(key)
        elif item.state in {"speculative", "unknown"}:
            unknown.add(key); uncertainty.add(f"cell:{key}:evidence-uncertain")
        else:
            incompatible.add(key); negative.add(f"cell:{key}:contradicted")
    required_budget = len(cells) + 2; gates_open = policy_allow and protected_closure and raw_data_local; disposition = "blocked" if not gates_open else ("admitted" if len(accepted) == len(cells) and budget_units >= required_budget else "refinement_required"); consumed = min(budget_units, required_budget)
    if budget_units < required_budget: omissions.add("workflow:budget-exhausted")
    if not policy_allow: omissions.add("workflow:policy-denied")
    if not protected_closure: omissions.add("workflow:protected-closure-incomplete")
    if not raw_data_local: omissions.add("workflow:raw-data-locality-failed")
    plan = tuple(f"plan:multimodal-context-stage:{index:02}" for index in range(required_budget)); checkpoint = research_artifact_digest({"workflow_id": workflow_id, "cell_order": sorted(cells), "accepted_order": sorted(accepted), "replay_identity": replay_identity}); workflow = research_artifact_digest({"workflow_id": workflow_id, "plan_order": list(plan), "checkpoint_digest": checkpoint, "budget_units": budget_units, "consumed_budget_units": consumed, "replay_identity": replay_identity}); effects = (f"schedule:multimodal-context-workflow:{workflow_id}",) if disposition == "admitted" else ("block:unsafe-release",); artifact = {"content_hash": research_artifact_digest({"request_id": request_id, "workflow_digest": workflow}), "media_type": "application/vnd.aurora.multimodal-context-workflow+json"}
    receipt = MultimodalContextWorkflowReceipt(request_id=request_id, workflow_id=workflow_id, query_id=query_id, goal=goal, disposition=disposition, study_order=studies, modality_order=modalities, cell_order=tuple(sorted(cells)), accepted_order=tuple(sorted(accepted)), missing_order=tuple(sorted(missing)), incompatible_order=tuple(sorted(incompatible)), unknown_order=tuple(sorted(unknown)), plan_order=plan, checkpoint_digest=checkpoint, workflow_digest=workflow, replay_identity=replay_identity, budget_units=budget_units, consumed_budget_units=consumed, omissions=tuple(sorted(omissions)), uncertainty=tuple(sorted(uncertainty)), negative_evidence=tuple(sorted(negative)), effect_receipts=effects, artifact=artifact)
    receipt.validate(); return receipt
