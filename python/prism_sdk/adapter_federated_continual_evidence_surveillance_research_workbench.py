"""Python parity surface for AFA-adapter-P01-F20.

This surface renders a read-only, federated continual evidence workbench.  It
delegates qualification to F12 and exposes only signed aggregate metadata;
raw observations stay at their originating institution.
"""
from __future__ import annotations

from dataclasses import dataclass
import re
from typing import Any, Sequence

from .adapter_federated_continual_evidence_surveillance_research_copilot import (
    FederatedCopilotEvidenceContribution,
    run_federated_continual_evidence_surveillance_research_copilot,
)
from .research_contracts import (
    ADAPTER_FEDERATED_CONTINUAL_EVIDENCE_SURVEILLANCE_RESEARCH_WORKBENCH_CONTRACT_VERSION,
    ADAPTER_FEDERATED_CONTINUAL_EVIDENCE_SURVEILLANCE_RESEARCH_WORKBENCH_FEATURE_ID,
    PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
    ResearchContractError,
    research_artifact_digest,
)

FEATURE_ID = ADAPTER_FEDERATED_CONTINUAL_EVIDENCE_SURVEILLANCE_RESEARCH_WORKBENCH_FEATURE_ID
CONTRACT_VERSION = ADAPTER_FEDERATED_CONTINUAL_EVIDENCE_SURVEILLANCE_RESEARCH_WORKBENCH_CONTRACT_VERSION
INPUT_SCHEMA = "EvidenceFeed4@1"
OUTPUT_SCHEMA = "QualifiedEvidenceSet5@1"
VIEWS = ("view:peers", "view:aggregate", "view:omissions", "view:provenance")
PANELS = ("panel:denied", "panel:negative", "panel:qualified", "panel:unknown")


@dataclass(frozen=True)
class FederatedContinualEvidenceSurveillanceResearchWorkbenchReceipt:
    request_id: str
    workbench_id: str
    scope: str
    federation_id: str
    purpose: str
    endpoint: str
    disposition: str
    view_order: tuple[str, ...]
    panel_order: tuple[str, ...]
    peer_order: tuple[str, ...]
    candidate_order: tuple[str, ...]
    qualified_order: tuple[str, ...]
    unknown_order: tuple[str, ...]
    blocked_order: tuple[str, ...]
    aggregate_order: tuple[str, ...]
    replay_identity: str
    copilot_run_digest: str
    workbench_digest: str
    federation_digest: str
    envelope_digest: str
    omissions: tuple[str, ...]
    uncertainty: tuple[str, ...]
    negative_evidence: tuple[str, ...]
    effect_receipts: tuple[str, ...]
    artifact: dict[str, Any]
    raw_data_local: bool = True
    boundary: str = PRECLINICAL_BOUNDARY
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    contract_version: str = CONTRACT_VERSION
    feature_id: str = FEATURE_ID

    def validate(self) -> None:
        if (
            self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            or self.contract_version != CONTRACT_VERSION
            or self.feature_id != FEATURE_ID
            or self.boundary != PRECLINICAL_BOUNDARY
            or not self.raw_data_local
            or not self.request_id.strip()
            or not self.workbench_id.strip()
            or not self.scope.strip()
            or not self.federation_id.strip()
            or not self.purpose.strip()
            or not self.endpoint.strip()
            or self.view_order != VIEWS
            or self.panel_order != PANELS
            or not self.candidate_order
            or not self.effect_receipts
        ):
            raise ResearchContractError("federated workbench identity, canonical views, locality, candidates, or effects are incomplete")
        for values in (
            self.peer_order, self.candidate_order, self.qualified_order,
            self.unknown_order, self.blocked_order, self.aggregate_order,
            self.omissions, self.uncertainty, self.negative_evidence,
            self.effect_receipts,
        ):
            if tuple(sorted(set(values))) != values:
                raise ResearchContractError("federated workbench ordering is not canonical")
        if set(self.qualified_order) | set(self.unknown_order) | set(self.blocked_order) != set(self.candidate_order):
            raise ResearchContractError("federated workbench states do not partition candidates")
        for value in (
            self.replay_identity, self.copilot_run_digest, self.workbench_digest,
            self.federation_digest, self.envelope_digest,
            self.artifact.get("content_hash"),
        ):
            if not isinstance(value, str) or not re.fullmatch(r"[0-9a-f]{64}", value):
                raise ResearchContractError("federated workbench digest is invalid")
        if any(
            not effect.startswith("view:federated-evidence-workbench:")
            and effect != "block:unsafe-release"
            for effect in self.effect_receipts
        ):
            raise ResearchContractError("federated workbench effect is outside read-only gate")


def render_federated_continual_evidence_surveillance_research_workbench(
    *,
    request_id: str,
    agent_id: str,
    federation_id: str,
    purpose: str,
    endpoint: str,
    semantic_profile: str,
    allowed_artifacts: Sequence[str],
    min_peer_quorum: int,
    declared_tools: Sequence[str],
    requested_tool: str,
    max_tool_calls: int,
    dry_run: bool,
    approval_reference: str | None,
    approval_granted: bool,
    contributions: Sequence[FederatedCopilotEvidenceContribution],
    policy_allow: bool = True,
    protected_closure: bool = True,
    raw_data_local: bool = True,
    replay_identity: str,
    workbench_id: str,
    scope: str,
    budget_units: int,
    boundary: str = PRECLINICAL_BOUNDARY,
) -> FederatedContinualEvidenceSurveillanceResearchWorkbenchReceipt:
    if not workbench_id.strip() or not scope.strip() or budget_units <= 0 or not dry_run or boundary != PRECLINICAL_BOUNDARY:
        raise ResearchContractError("federated workbench identity, budget, dry-run, or boundary is invalid")
    if not re.fullmatch(r"[0-9a-f]{64}", replay_identity):
        raise ResearchContractError("federated workbench replay identity is invalid")
    copilot = run_federated_continual_evidence_surveillance_research_copilot(
        request_id=request_id, agent_id=agent_id, federation_id=federation_id,
        purpose=purpose, endpoint=endpoint, semantic_profile=semantic_profile,
        allowed_artifacts=allowed_artifacts, min_peer_quorum=min_peer_quorum,
        declared_tools=declared_tools, requested_tool=requested_tool,
        max_tool_calls=max_tool_calls, dry_run=dry_run,
        approval_reference=approval_reference, approval_granted=approval_granted,
        contributions=contributions, policy_allow=policy_allow,
        protected_closure=protected_closure, raw_data_local=raw_data_local,
        replay_identity=replay_identity,
    )
    omissions = tuple(sorted(set(copilot.omissions) | {"workbench:read-only-federated-view"}))
    views = VIEWS
    panels = PANELS
    workbench_digest = research_artifact_digest({
        "workbench_id": workbench_id, "scope": scope, "views": list(views),
        "panels": list(panels), "candidate_order": list(copilot.candidate_order),
        "qualified_order": list(copilot.selected_order),
        "unknown_order": list(copilot.unresolved_order),
        "blocked_order": list(copilot.denied_order),
        "aggregate_order": list(copilot.aggregate_order),
        "replay_identity": replay_identity, "copilot_run_digest": copilot.run_digest,
    })
    payload = {
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID,
        "request_id": request_id, "workbench_id": workbench_id, "scope": scope,
        "federation_id": federation_id, "purpose": purpose, "endpoint": endpoint,
        "disposition": copilot.disposition, "view_order": list(views),
        "panel_order": list(panels), "peer_order": list(copilot.peer_order),
        "candidate_order": list(copilot.candidate_order),
        "qualified_order": list(copilot.selected_order),
        "unknown_order": list(copilot.unresolved_order),
        "blocked_order": list(copilot.denied_order),
        "aggregate_order": list(copilot.aggregate_order),
        "replay_identity": replay_identity, "copilot_run_digest": copilot.run_digest,
        "workbench_digest": workbench_digest,
        "federation_digest": copilot.federation_digest,
        "envelope_digest": copilot.envelope_digest, "omissions": list(omissions),
        "uncertainty": list(copilot.uncertainty),
        "negative_evidence": list(copilot.negative_evidence),
        "raw_data_local": True, "boundary": PRECLINICAL_BOUNDARY,
    }
    receipt = FederatedContinualEvidenceSurveillanceResearchWorkbenchReceipt(
        request_id=request_id, workbench_id=workbench_id, scope=scope,
        federation_id=federation_id, purpose=purpose, endpoint=endpoint,
        disposition=copilot.disposition, view_order=views, panel_order=panels,
        peer_order=copilot.peer_order, candidate_order=copilot.candidate_order,
        qualified_order=copilot.selected_order, unknown_order=copilot.unresolved_order,
        blocked_order=copilot.denied_order, aggregate_order=copilot.aggregate_order,
        replay_identity=replay_identity,
        copilot_run_digest=copilot.run_digest, workbench_digest=workbench_digest,
        federation_digest=copilot.federation_digest, envelope_digest=copilot.envelope_digest,
        omissions=omissions, uncertainty=copilot.uncertainty,
        negative_evidence=copilot.negative_evidence,
        effect_receipts=(f"view:federated-evidence-workbench:{workbench_id}",),
        artifact={"content_hash": research_artifact_digest(payload),
                  "media_type": "application/vnd.aurora.federated-evidence-workbench+json"},
    )
    receipt.validate()
    return receipt
