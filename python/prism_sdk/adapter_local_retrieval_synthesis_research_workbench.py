"""Python parity surface for AFA-adapter-P02-F17, a local read-only workbench."""
from __future__ import annotations

from dataclasses import dataclass
import re
from typing import Any, Sequence

from .adapter_local_retrieval_synthesis_research_copilot import (
    LocalRetrievalSynthesisCandidate,
    run_local_retrieval_synthesis_research_copilot,
)
from .research_contracts import (
    PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError,
    research_artifact_digest,
)

FEATURE_ID = "AFA-adapter-P02-F17"
CONTRACT_VERSION = "adapter-local-retrieval-synthesis-research-workbench/1.0"
INPUT_SCHEMA = "ScopedRetrievalQuery1@1"
OUTPUT_SCHEMA = "EvidenceSynthesis5@1"
CANONICAL_VIEWS = ("view:overview", "view:evidence", "view:omissions", "view:provenance")
CANONICAL_PANELS = ("panel:negative", "panel:provenance", "panel:qualified", "panel:unknown")


@dataclass(frozen=True)
class LocalRetrievalSynthesisResearchWorkbenchReceipt:
    request_id: str; workspace_id: str; query_id: str; scope: str; intent: str; disposition: str
    view_order: tuple[str, ...]; panel_order: tuple[str, ...]; candidate_order: tuple[str, ...]
    selected_order: tuple[str, ...]; omitted_order: tuple[str, ...]; uncertainty_order: tuple[str, ...]
    negative_order: tuple[str, ...]; contradictory_order: tuple[str, ...]; replay_identity: str
    copilot_run_digest: str; workbench_digest: str; omissions: tuple[str, ...]
    uncertainty: tuple[str, ...]; negative_evidence: tuple[str, ...]; effect_receipts: tuple[str, ...]
    artifact: dict[str, Any]; schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    contract_version: str = CONTRACT_VERSION; feature_id: str = FEATURE_ID
    raw_data_local: bool = True; boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if ((self.schema_version, self.contract_version, self.feature_id) != (RESEARCH_CONTRACT_SCHEMA_VERSION, CONTRACT_VERSION, FEATURE_ID)
            or self.boundary != PRECLINICAL_BOUNDARY or not self.raw_data_local or not self.request_id.strip()
            or not self.workspace_id.strip() or not self.query_id.strip() or not self.scope.strip() or not self.intent.strip()
            or self.view_order != CANONICAL_VIEWS or self.panel_order != CANONICAL_PANELS or not self.candidate_order or not self.effect_receipts):
            raise ResearchContractError("workbench identity, views, candidates, locality, or effects are incomplete")
        for values in (self.candidate_order, self.selected_order, self.omitted_order, self.uncertainty_order, self.negative_order, self.contradictory_order, self.omissions, self.uncertainty, self.negative_evidence, self.effect_receipts):
            if tuple(sorted(set(values))) != values: raise ResearchContractError("workbench ordering is not canonical")
        if set(self.selected_order) | set(self.omitted_order) != set(self.candidate_order): raise ResearchContractError("workbench evidence states do not partition candidates")
        for value in (self.replay_identity, self.copilot_run_digest, self.workbench_digest, self.artifact.get("content_hash")):
            if not isinstance(value, str) or not re.fullmatch(r"[0-9a-f]{64}", value): raise ResearchContractError("workbench digest is invalid")
        if any(not effect.startswith("view:local-retrieval-workbench:") and effect != "block:unsafe-release" for effect in self.effect_receipts): raise ResearchContractError("workbench effect is outside read-only gate")

    def digest(self) -> str:
        self.validate(); return research_artifact_digest(self.__dict__)


def render_local_retrieval_synthesis_research_workbench(*, request_id: str, workspace_id: str, query_id: str, requester: str, intent: str, study_ids: Sequence[str], required_modalities: Sequence[str], comparability_profile: str, max_results: int, candidates: Sequence[LocalRetrievalSynthesisCandidate], copilot_id: str, agent_id: str, algorithm_version: str, replay_identity: str, budget_units: int, scope: str, view_order: Sequence[str] = CANONICAL_VIEWS, panel_order: Sequence[str] = CANONICAL_PANELS, policy_allow: bool = True, protected_closure: bool = True, raw_data_local: bool = True, boundary: str = PRECLINICAL_BOUNDARY) -> LocalRetrievalSynthesisResearchWorkbenchReceipt:
    if (not request_id.strip() or not workspace_id.strip() or not query_id.strip() or not requester.strip() or not intent.strip() or not scope.strip() or not study_ids or not required_modalities or not candidates or max_results <= 0 or budget_units <= 0 or tuple(view_order) != CANONICAL_VIEWS or tuple(panel_order) != CANONICAL_PANELS or not raw_data_local or boundary != PRECLINICAL_BOUNDARY or not re.fullmatch(r"[0-9a-f]{64}", replay_identity)):
        raise ResearchContractError("workbench identity, query, views, budget, locality, replay, or boundary is invalid")
    copilot = run_local_retrieval_synthesis_research_copilot(request_id=request_id, query_id=query_id, requester=requester, intent=intent, study_ids=study_ids, required_modalities=required_modalities, comparability_profile=comparability_profile, max_results=max_results, candidates=candidates, copilot_id=copilot_id, agent_id=agent_id, algorithm_version=algorithm_version, budget_units=budget_units, replay_identity=replay_identity, policy_allow=policy_allow, protected_closure_satisfied=protected_closure, raw_data_local=True, boundary=boundary)
    candidate = tuple(sorted(copilot.candidate_order)); selected = tuple(sorted(copilot.selected_order)); omitted = tuple(sorted(copilot.omitted_order)); uncertainty = tuple(sorted(copilot.uncertainty_order)); negative = tuple(sorted(copilot.negative_order)); contradictory = tuple(sorted(copilot.contradictory_order))
    omissions = tuple(sorted({f"evidence:{x}:omitted" for x in omitted} | {f"evidence:{x}:contradictory" for x in contradictory}))
    copilot_digest = research_artifact_digest(copilot.__dict__)
    workbench_digest = research_artifact_digest({"workspace_id": workspace_id, "scope": scope, "views": list(view_order), "panels": list(panel_order), "candidate": list(candidate), "selected": list(selected), "omitted": list(omitted), "replay_identity": replay_identity, "copilot_run_digest": copilot_digest})
    payload = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request_id, "workspace_id": workspace_id, "query_id": query_id, "scope": scope, "intent": intent, "disposition": copilot.disposition, "view_order": list(view_order), "panel_order": list(panel_order), "candidate_order": list(candidate), "selected_order": list(selected), "omitted_order": list(omitted), "uncertainty_order": list(uncertainty), "negative_order": list(negative), "contradictory_order": list(contradictory), "replay_identity": replay_identity, "copilot_run_digest": copilot_digest, "workbench_digest": workbench_digest, "omissions": list(omissions), "uncertainty": list(uncertainty), "negative_evidence": list(negative), "raw_data_local": True, "boundary": PRECLINICAL_BOUNDARY}
    receipt = LocalRetrievalSynthesisResearchWorkbenchReceipt(request_id=request_id, workspace_id=workspace_id, query_id=query_id, scope=scope, intent=intent, disposition=copilot.disposition, view_order=tuple(view_order), panel_order=tuple(panel_order), candidate_order=candidate, selected_order=selected, omitted_order=omitted, uncertainty_order=uncertainty, negative_order=negative, contradictory_order=contradictory, replay_identity=replay_identity, copilot_run_digest=copilot_digest, workbench_digest=workbench_digest, omissions=omissions, uncertainty=uncertainty, negative_evidence=negative, effect_receipts=(f"view:local-retrieval-workbench:{workspace_id}",), artifact={"content_hash": research_artifact_digest(payload), "media_type": "application/vnd.aurora.local-retrieval-synthesis-research-workbench+json"})
    receipt.validate(); return receipt


__all__ = ["CANONICAL_VIEWS", "CANONICAL_PANELS", "FEATURE_ID", "CONTRACT_VERSION", "LocalRetrievalSynthesisResearchWorkbenchReceipt", "render_local_retrieval_synthesis_research_workbench"]
