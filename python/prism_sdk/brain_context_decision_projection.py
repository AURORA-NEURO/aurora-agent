"""Python parity contract for context-to-Decision-Section projection."""
from __future__ import annotations

from dataclasses import dataclass
import re
from typing import Any, Mapping, Sequence

from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError, research_artifact_digest

CONTEXT_DECISION_PROJECTION_FEATURE_ID = "AFA-brain-P03-F11"
CONTEXT_DECISION_PROJECTION_CONTRACT_VERSION = "brain-context-decision-projection/1.0"


@dataclass(frozen=True)
class BrainContextDecisionProjectionReceipt:
    request_id: str
    query_id: str
    goal: str
    disposition: str
    selected_order: tuple[str, ...]
    dependency_order: tuple[str, ...]
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
    feature_id: str = CONTEXT_DECISION_PROJECTION_FEATURE_ID
    contract_version: str = CONTEXT_DECISION_PROJECTION_CONTRACT_VERSION
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    raw_data_local: bool = True
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION or self.feature_id != CONTEXT_DECISION_PROJECTION_FEATURE_ID or self.contract_version != CONTEXT_DECISION_PROJECTION_CONTRACT_VERSION:
            raise ResearchContractError("decision projection schema, feature, or version mismatch")
        if self.boundary != PRECLINICAL_BOUNDARY or not self.raw_data_local or not self.request_id.strip() or not self.query_id.strip() or not self.goal.strip() or not self.selected_order or not self.refinement_frontier_order or not self.effect_receipts or self.disposition not in {"admitted", "refinement_required", "blocked"}:
            raise ResearchContractError("decision projection identity, obligations, frontier, locality, disposition, or effects are incomplete")
        if self.disposition != "admitted" and not self.unresolved_obligation_order:
            raise ResearchContractError("non-admitted projection must retain an unresolved obligation")
        for values in (self.selected_order, self.dependency_order, self.unresolved_obligation_order, self.refinement_frontier_order, self.omissions, self.uncertainty, self.negative_evidence, self.effect_receipts):
            if tuple(sorted(set(values))) != values:
                raise ResearchContractError("decision projection vectors are not canonical")
        for value in (self.context_digest, self.section_digest, self.replay_identity, self.artifact.get("content_hash")):
            if not isinstance(value, str) or not re.fullmatch(r"[0-9a-f]{64}", value):
                raise ResearchContractError("decision projection digest is invalid")
        if any(not effect.startswith("project:local-decision-section:") and effect != "block:unsafe-release" for effect in self.effect_receipts):
            raise ResearchContractError("decision projection effect is outside local projection gate")

    def digest(self) -> str:
        self.validate()
        return research_artifact_digest({"schema_version": self.schema_version, "contract_version": self.contract_version, "feature_id": self.feature_id, "request_id": self.request_id, "query_id": self.query_id, "goal": self.goal, "disposition": self.disposition, "selected_order": list(self.selected_order), "dependency_order": list(self.dependency_order), "unresolved_obligation_order": list(self.unresolved_obligation_order), "refinement_frontier_order": list(self.refinement_frontier_order), "context_digest": self.context_digest, "section_digest": self.section_digest, "replay_identity": self.replay_identity, "omissions": list(self.omissions), "uncertainty": list(self.uncertainty), "negative_evidence": list(self.negative_evidence), "effect_receipts": list(self.effect_receipts), "artifact": dict(self.artifact), "raw_data_local": self.raw_data_local, "boundary": self.boundary})


def project_context_to_decision_section(*, request_id: str, query_id: str, goal: str, context_disposition: str, selected_context_ids: Sequence[str], omission_certificate_ids: Sequence[str], uncertainty_ids: Sequence[str], dependency_order: Sequence[str], context_digest: str, replay_identity: str, policy_allow: bool = True, protected_closure: bool = True, raw_data_local: bool = True) -> BrainContextDecisionProjectionReceipt:
    if not request_id.strip() or not query_id.strip() or not goal.strip() or not selected_context_ids or not all(isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) for value in (context_digest, replay_identity)):
        raise ResearchContractError("decision projection identity, selected context, or digest is invalid")
    selected = tuple(sorted(set(selected_context_ids))); dependencies = tuple(sorted(set(dependency_order)))
    if len(selected) != len(selected_context_ids) or len(dependencies) != len(dependency_order) or any(not value.strip() for value in selected):
        raise ResearchContractError("decision projection identifiers must be unique and non-empty")
    obligations = {f"obligation:omission-certificate:{value}" for value in omission_certificate_ids} | {f"obligation:uncertainty:{value}" for value in uncertainty_ids}; frontier: set[str] = set(); omissions = {f"omission-certificate:{value}" for value in omission_certificate_ids}; uncertainty = {f"uncertainty:{value}" for value in uncertainty_ids}; negative: set[str] = set()
    if omissions: frontier.add("refine:resolve-omission-certificates")
    if uncertainty: frontier.add("refine:resolve-uncertainty")
    if context_disposition != "qualified": obligations.add(f"obligation:context-disposition:{context_disposition}"); frontier.add("refine:compile-qualified-context")
    if len(dependencies) < len(selected): obligations.add("obligation:dependency-closure-incomplete"); frontier.add("refine:complete-dependency-closure")
    if not policy_allow or not protected_closure or not raw_data_local: obligations.add("obligation:policy-protected-closure-locality-blocked"); frontier.add("refine:obtain-policy-and-closure")
    if not obligations: frontier.add("frontier:none")
    disposition = "blocked" if not policy_allow or not protected_closure or not raw_data_local else ("admitted" if not obligations else "refinement_required")
    section_digest = research_artifact_digest({"query_id": query_id, "goal": goal, "selected_order": list(selected), "dependency_order": list(dependencies), "obligation_order": sorted(obligations), "frontier_order": sorted(frontier), "context_digest": context_digest, "replay_identity": replay_identity, "disposition": disposition}); effects = (f"project:local-decision-section:{request_id}",) if disposition != "blocked" else ("block:unsafe-release",); artifact = {"content_hash": research_artifact_digest({"request_id": request_id, "section_digest": section_digest}), "media_type": "application/vnd.aurora.context-decision-projection+json"}
    receipt = BrainContextDecisionProjectionReceipt(request_id=request_id, query_id=query_id, goal=goal, disposition=disposition, selected_order=selected, dependency_order=dependencies, unresolved_obligation_order=tuple(sorted(obligations)), refinement_frontier_order=tuple(sorted(frontier)), context_digest=context_digest, section_digest=section_digest, replay_identity=replay_identity, omissions=tuple(sorted(omissions)), uncertainty=tuple(sorted(uncertainty)), negative_evidence=tuple(sorted(negative)), effect_receipts=effects, artifact=artifact)
    receipt.validate(); return receipt
