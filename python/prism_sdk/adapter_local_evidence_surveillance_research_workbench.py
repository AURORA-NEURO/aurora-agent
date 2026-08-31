"""A0 local single-study evidence-surveillance researcher workbench."""
from __future__ import annotations

from dataclasses import dataclass
import re
from typing import Any, Mapping, Sequence

from .adapter_local_evidence_surveillance_research_copilot import (
    CopilotEvidenceObservation,
    run_local_evidence_surveillance_research_copilot,
)
from .research_contracts import (
    PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
    ResearchContractError,
    research_artifact_digest,
)

FEATURE_ID = "AFA-adapter-P01-F17"
CONTRACT_VERSION = "adapter-local-evidence-surveillance-research-workbench/1.0"
INPUT_SCHEMA = "EvidenceFeed1@1"
OUTPUT_SCHEMA = "QualifiedEvidenceSet5@1"
CANONICAL_VIEWS = ("view:overview", "view:evidence", "view:omissions", "view:provenance")
CANONICAL_PANELS = ("panel:negative", "panel:provenance", "panel:qualified", "panel:unknown")


@dataclass(frozen=True)
class LocalEvidenceSurveillanceResearchWorkbenchReceipt:
    request_id: str
    workspace_id: str
    study_id: str
    scope: str
    intent: str
    disposition: str
    view_order: tuple[str, ...]
    panel_order: tuple[str, ...]
    candidate_order: tuple[str, ...]
    qualified_order: tuple[str, ...]
    unknown_order: tuple[str, ...]
    blocked_order: tuple[str, ...]
    replay_identity: str
    copilot_run_digest: str
    workbench_digest: str
    omissions: tuple[str, ...]
    uncertainty: tuple[str, ...]
    negative_evidence: tuple[str, ...]
    effect_receipts: tuple[str, ...]
    artifact: Mapping[str, Any]
    raw_data_local: bool = True
    boundary: str = PRECLINICAL_BOUNDARY
    feature_id: str = FEATURE_ID
    contract_version: str = CONTRACT_VERSION
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION

    def validate(self) -> None:
        if (self.schema_version, self.contract_version, self.feature_id) != (RESEARCH_CONTRACT_SCHEMA_VERSION, CONTRACT_VERSION, FEATURE_ID):
            raise ResearchContractError("workbench schema, contract, or feature mismatch")
        if self.boundary != PRECLINICAL_BOUNDARY or not self.raw_data_local or not self.request_id.strip() or not self.workspace_id.strip() or not self.study_id.strip() or not self.scope.strip() or not self.intent.strip() or self.view_order != CANONICAL_VIEWS or self.panel_order != CANONICAL_PANELS or not self.candidate_order or not self.effect_receipts:
            raise ResearchContractError("workbench identity, locality, canonical views, candidates, or effects are incomplete")
        for values in (self.candidate_order, self.qualified_order, self.unknown_order, self.blocked_order, self.omissions, self.uncertainty, self.negative_evidence, self.effect_receipts):
            if tuple(sorted(set(values))) != values:
                raise ResearchContractError("workbench ordering is not canonical")
        if set(self.qualified_order) | set(self.unknown_order) | set(self.blocked_order) != set(self.candidate_order):
            raise ResearchContractError("workbench evidence states do not partition candidates")
        for value in (self.replay_identity, self.copilot_run_digest, self.workbench_digest, self.artifact.get("content_hash")):
            if not isinstance(value, str) or not re.fullmatch(r"[0-9a-f]{64}", value):
                raise ResearchContractError("workbench digest is invalid")
        if any(not effect.startswith("view:local-evidence-workbench:") and effect != "block:unsafe-release" for effect in self.effect_receipts):
            raise ResearchContractError("workbench effect is outside read-only gate")

    def digest(self) -> str:
        self.validate()
        return research_artifact_digest({"schema_version": self.schema_version, "contract_version": self.contract_version, "feature_id": self.feature_id, "request_id": self.request_id, "workspace_id": self.workspace_id, "study_id": self.study_id, "scope": self.scope, "intent": self.intent, "disposition": self.disposition, "view_order": list(self.view_order), "panel_order": list(self.panel_order), "candidate_order": list(self.candidate_order), "qualified_order": list(self.qualified_order), "unknown_order": list(self.unknown_order), "blocked_order": list(self.blocked_order), "replay_identity": self.replay_identity, "copilot_run_digest": self.copilot_run_digest, "workbench_digest": self.workbench_digest, "omissions": list(self.omissions), "uncertainty": list(self.uncertainty), "negative_evidence": list(self.negative_evidence), "effect_receipts": list(self.effect_receipts), "artifact": dict(self.artifact), "raw_data_local": self.raw_data_local, "boundary": self.boundary})


def render_local_evidence_surveillance_research_workbench(*, request_id: str, workspace_id: str, study_id: str, scope: str, intent: str, agent_id: str, declared_tools: Sequence[str], requested_tool: str, max_tool_calls: int, dry_run: bool, required_source_ids: Sequence[str], observations: Sequence[CopilotEvidenceObservation], replay_identity: str, min_relevance_score: int = 0, policy_allow: bool = True, protected_closure: bool = True, raw_data_local: bool = True, view_order: Sequence[str] = CANONICAL_VIEWS, panel_order: Sequence[str] = CANONICAL_PANELS) -> LocalEvidenceSurveillanceResearchWorkbenchReceipt:
    if not dry_run or not raw_data_local or not re.fullmatch(r"[0-9a-f]{64}", replay_identity) or tuple(view_order) != CANONICAL_VIEWS or tuple(panel_order) != CANONICAL_PANELS:
        raise ResearchContractError("workbench requires a local dry-run request with canonical views and panels")
    copilot = run_local_evidence_surveillance_research_copilot(request_id=request_id, agent_id=agent_id, study_id=study_id, intent=intent, declared_tools=declared_tools, requested_tool=requested_tool, max_tool_calls=max_tool_calls, dry_run=True, required_source_ids=required_source_ids, observations=observations, min_relevance_score=min_relevance_score, policy_allow=policy_allow, protected_closure=protected_closure, raw_data_local=True, replay_identity=replay_identity)
    candidate = tuple(sorted(copilot.candidate_order)); qualified = tuple(sorted(copilot.selected_order)); unknown = tuple(sorted(copilot.unresolved_order)); blocked = tuple(sorted(copilot.denied_order))
    omissions = tuple(sorted(set(copilot.omissions) | {"workbench:read-only-local-view"}))
    copilot_digest = copilot.digest()
    workbench_digest = research_artifact_digest({"workspace_id": workspace_id, "study_id": study_id, "scope": scope, "views": list(view_order), "panels": list(panel_order), "candidate": list(candidate), "qualified": list(qualified), "unknown": list(unknown), "blocked": list(blocked), "replay_identity": replay_identity, "copilot_run_digest": copilot_digest})
    payload = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request_id, "workspace_id": workspace_id, "study_id": study_id, "scope": scope, "intent": intent, "disposition": copilot.disposition, "view_order": list(view_order), "panel_order": list(panel_order), "candidate_order": list(candidate), "qualified_order": list(qualified), "unknown_order": list(unknown), "blocked_order": list(blocked), "replay_identity": replay_identity, "copilot_run_digest": copilot_digest, "workbench_digest": workbench_digest, "omissions": list(omissions), "uncertainty": list(copilot.uncertainty), "negative_evidence": list(copilot.negative_evidence), "boundary": PRECLINICAL_BOUNDARY, "raw_data_local": True}
    receipt = LocalEvidenceSurveillanceResearchWorkbenchReceipt(request_id=request_id, workspace_id=workspace_id, study_id=study_id, scope=scope, intent=intent, disposition=copilot.disposition, view_order=tuple(view_order), panel_order=tuple(panel_order), candidate_order=candidate, qualified_order=qualified, unknown_order=unknown, blocked_order=blocked, replay_identity=replay_identity, copilot_run_digest=copilot_digest, workbench_digest=workbench_digest, omissions=omissions, uncertainty=tuple(sorted(copilot.uncertainty)), negative_evidence=tuple(sorted(copilot.negative_evidence)), effect_receipts=(f"view:local-evidence-workbench:{workspace_id}",), artifact={"content_hash": research_artifact_digest(payload), "media_type": "application/vnd.aurora.local-evidence-workbench+json"})
    receipt.validate()
    return receipt


__all__ = ["CANONICAL_PANELS", "CANONICAL_VIEWS", "FEATURE_ID", "CONTRACT_VERSION", "INPUT_SCHEMA", "OUTPUT_SCHEMA", "LocalEvidenceSurveillanceResearchWorkbenchReceipt", "render_local_evidence_surveillance_research_workbench"]
