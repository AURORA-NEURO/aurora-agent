"""Federated continual context workbench parity contract."""
from __future__ import annotations

from dataclasses import dataclass
import re
from typing import Any, Mapping, Sequence

from .research_contracts import (
    FEDERATED_CONTEXT_WORKBENCH_CONTRACT_VERSION,
    FEDERATED_CONTEXT_WORKBENCH_FEATURE_ID,
    PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
    ResearchContractError,
    research_artifact_digest,
)


@dataclass(frozen=True)
class FederatedContextWorkbenchPeer:
    institution_id: str
    epoch: int
    semantic_profile: str
    context_digest: str
    section_digest: str
    replay_identity: str
    evidence_state: str = "supported"
    policy_allow: bool = True
    protected_closure: bool = True
    signed_approval: bool = True
    raw_data_local: bool = True
    aggregate_only: bool = True
    boundary: str = PRECLINICAL_BOUNDARY


@dataclass(frozen=True)
class FederatedContextWorkbenchReceipt:
    session_id: str
    federation_id: str
    query_id: str
    goal: str
    semantic_profile: str
    disposition: str
    institution_order: tuple[str, ...]
    qualified_institution_order: tuple[str, ...]
    stale_institution_order: tuple[str, ...]
    blocked_institution_order: tuple[str, ...]
    unknown_institution_order: tuple[str, ...]
    view_order: tuple[str, ...]
    action_order: tuple[str, ...]
    blocked_action_order: tuple[str, ...]
    aggregate_order: tuple[str, ...]
    quorum: int
    minimum_quorum: int
    current_epoch: int
    budget_units: int
    consumed_budget_units: int
    checkpoint_digest: str
    federation_envelope_digest: str
    replay_identity: str
    omissions: tuple[str, ...]
    uncertainty: tuple[str, ...]
    negative_evidence: tuple[str, ...]
    effect_receipts: tuple[str, ...]
    artifact: Mapping[str, Any]
    feature_id: str = FEDERATED_CONTEXT_WORKBENCH_FEATURE_ID
    contract_version: str = FEDERATED_CONTEXT_WORKBENCH_CONTRACT_VERSION
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    raw_data_local: bool = True
    aggregate_only: bool = True
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if (self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
                or self.contract_version != FEDERATED_CONTEXT_WORKBENCH_CONTRACT_VERSION
                or self.feature_id != FEDERATED_CONTEXT_WORKBENCH_FEATURE_ID):
            raise ResearchContractError("federated workbench schema, feature, or version mismatch")
        if (self.boundary != PRECLINICAL_BOUNDARY or not self.raw_data_local or not self.aggregate_only
                or not self.session_id.strip() or not self.federation_id.strip() or not self.query_id.strip()
                or not self.goal.strip() or not self.semantic_profile.strip() or len(self.institution_order) < 2
                or not self.view_order or not self.action_order or self.minimum_quorum < 1
                or self.quorum != len(self.qualified_institution_order) or self.quorum > len(self.institution_order)
                or self.budget_units < 1 or self.consumed_budget_units > self.budget_units
                or not self.effect_receipts or self.disposition not in {"ready", "needs_refinement", "blocked"}):
            raise ResearchContractError("federated workbench identity, quorum, budget, locality, view, action, or disposition is incomplete")
        for values in (self.institution_order, self.qualified_institution_order, self.stale_institution_order,
                       self.blocked_institution_order, self.unknown_institution_order, self.view_order,
                       self.action_order, self.blocked_action_order, self.aggregate_order, self.omissions,
                       self.uncertainty, self.negative_evidence, self.effect_receipts):
            if tuple(sorted(set(values))) != values:
                raise ResearchContractError("federated workbench vectors are not canonical")
        classified = set(self.qualified_institution_order) | set(self.stale_institution_order) | set(self.blocked_institution_order) | set(self.unknown_institution_order)
        if classified != set(self.institution_order):
            raise ResearchContractError("federated peer states do not partition institutions")
        for value in (*self.aggregate_order, self.checkpoint_digest, self.federation_envelope_digest, self.replay_identity, self.artifact.get("content_hash")):
            if not isinstance(value, str) or not re.fullmatch(r"[0-9a-f]{64}", value):
                raise ResearchContractError("federated workbench digest is invalid")
        if any(not effect.startswith("view:local-federated-context-workbench:") and effect != "block:unsafe-release" for effect in self.effect_receipts):
            raise ResearchContractError("federated workbench effect is outside read-only view gate")

    def digest(self) -> str:
        self.validate()
        return research_artifact_digest({
            "schema_version": self.schema_version, "contract_version": self.contract_version,
            "feature_id": self.feature_id, "session_id": self.session_id, "federation_id": self.federation_id,
            "query_id": self.query_id, "goal": self.goal, "semantic_profile": self.semantic_profile,
            "disposition": self.disposition, "institution_order": list(self.institution_order),
            "qualified_institution_order": list(self.qualified_institution_order),
            "stale_institution_order": list(self.stale_institution_order),
            "blocked_institution_order": list(self.blocked_institution_order),
            "unknown_institution_order": list(self.unknown_institution_order), "view_order": list(self.view_order),
            "action_order": list(self.action_order), "blocked_action_order": list(self.blocked_action_order),
            "aggregate_order": list(self.aggregate_order), "quorum": self.quorum, "minimum_quorum": self.minimum_quorum,
            "current_epoch": self.current_epoch, "budget_units": self.budget_units,
            "consumed_budget_units": self.consumed_budget_units, "checkpoint_digest": self.checkpoint_digest,
            "federation_envelope_digest": self.federation_envelope_digest, "replay_identity": self.replay_identity,
            "omissions": list(self.omissions), "uncertainty": list(self.uncertainty),
            "negative_evidence": list(self.negative_evidence), "effect_receipts": list(self.effect_receipts),
            "artifact": dict(self.artifact), "raw_data_local": self.raw_data_local,
            "aggregate_only": self.aggregate_only, "boundary": self.boundary,
        })


def render_federated_context_workbench(*, session_id: str, federation_id: str, query_id: str, goal: str,
                                       semantic_profile: str, required_institution_ids: Sequence[str],
                                       peers: Sequence[FederatedContextWorkbenchPeer], minimum_quorum: int,
                                       current_epoch: int, max_epoch_lag: int, budget_units: int,
                                       replay_identity: str, policy_allow: bool = True,
                                       protected_closure: bool = True, signed_approval: bool = True,
                                       raw_data_local: bool = True, aggregate_only: bool = True) -> FederatedContextWorkbenchReceipt:
    if (not session_id.strip() or not federation_id.strip() or not query_id.strip() or not goal.strip()
            or not semantic_profile.strip() or len(required_institution_ids) < 2 or minimum_quorum < 1
            or minimum_quorum > len(required_institution_ids) or budget_units < 1
            or not re.fullmatch(r"[0-9a-f]{64}", replay_identity)):
        raise ResearchContractError("federated workbench identity, institutions, quorum, budget, or replay is invalid")
    institutions = tuple(sorted(set(required_institution_ids)))
    if len(institutions) != len(required_institution_ids) or any(not value.strip() for value in institutions):
        raise ResearchContractError("federated institution identifiers must be unique and non-empty")
    peer_map: dict[str, FederatedContextWorkbenchPeer] = {}
    for peer in peers:
        if peer.institution_id in peer_map:
            raise ResearchContractError("federated peer attestations must be unique")
        peer_map[peer.institution_id] = peer
    qualified: set[str] = set(); stale: set[str] = set(); blocked: set[str] = set(); unknown: set[str] = set(); aggregate: set[str] = set()
    views = {"view:peer-quorum", "view:replay-identity", "view:provenance-and-omissions"}; actions = {"action:inspect-peer-attestation", "action:replay-local-federated-view"}; blocked_actions: set[str] = set()
    omissions: set[str] = set(); uncertainty: set[str] = set(); negative: set[str] = set()
    for institution in institutions:
        peer = peer_map.get(institution)
        if peer is None:
            unknown.add(institution); omissions.add(f"institution:{institution}:missing-attestation"); continue
        if (not policy_allow or not protected_closure or not signed_approval or not raw_data_local or not aggregate_only
                or not peer.policy_allow or not peer.protected_closure or not peer.signed_approval or not peer.raw_data_local
                or not peer.aggregate_only or peer.boundary != PRECLINICAL_BOUNDARY):
            blocked.add(institution); omissions.add(f"institution:{institution}:federation-gate-blocked")
        elif peer.semantic_profile != semantic_profile:
            blocked.add(institution); negative.add(f"institution:{institution}:semantic-profile-mismatch")
        elif peer.replay_identity != replay_identity:
            unknown.add(institution); uncertainty.add(f"institution:{institution}:replay-mismatch")
        elif peer.epoch > current_epoch or current_epoch - peer.epoch > max_epoch_lag:
            stale.add(institution); omissions.add(f"institution:{institution}:stale-epoch")
        elif peer.evidence_state in {"proven", "supported"}:
            qualified.add(institution); aggregate.add(research_artifact_digest({"institution_id": institution, "epoch": peer.epoch, "semantic_profile": semantic_profile, "context_digest": peer.context_digest, "section_digest": peer.section_digest, "replay_identity": peer.replay_identity}))
        elif peer.evidence_state in {"speculative", "unknown"}:
            unknown.add(institution); uncertainty.add(f"institution:{institution}:evidence-uncertain")
        else:
            blocked.add(institution); negative.add(f"institution:{institution}:contradicted")
    quorum = len(qualified); required_budget = minimum_quorum; gates_open = policy_allow and protected_closure and signed_approval and raw_data_local and aggregate_only
    disposition = "blocked" if not gates_open else ("ready" if quorum >= minimum_quorum and budget_units >= required_budget else "needs_refinement")
    consumed = min(budget_units, required_budget)
    if budget_units < required_budget: omissions.add("workbench:budget-exhausted")
    if not policy_allow: omissions.add("workbench:policy-denied")
    if not protected_closure: omissions.add("workbench:protected-closure-incomplete")
    if not signed_approval: omissions.add("workbench:signed-approval-missing")
    if not raw_data_local: omissions.add("workbench:raw-data-locality-failed")
    if not aggregate_only: omissions.add("workbench:aggregate-only-required")
    if disposition == "ready":
        actions.update({"action:open-decision-section", "action:export-digest-only-context"})
    elif disposition == "blocked":
        blocked_actions.update({"action:open-decision-section", "action:export-digest-only-context", "action:replay-local-federated-view"}); actions = {"action:inspect-block-reason"}
    else:
        actions.update({"action:review-peer-outcomes", "action:request-federation-refinement"}); uncertainty.add("workbench:quorum-not-admitted")
    if stale: views.add("view:stale-peers")
    if unknown: views.add("view:uncertain-peers")
    if blocked: views.add("view:blocked-peers")
    checkpoint = research_artifact_digest({"session_id": session_id, "institution_order": list(institutions), "qualified_order": sorted(qualified), "replay_identity": replay_identity})
    envelope = research_artifact_digest({"federation_id": federation_id, "aggregate_order": sorted(aggregate), "checkpoint_digest": checkpoint, "raw_data_local": True, "aggregate_only": True})
    artifact = {"content_hash": research_artifact_digest({"session_id": session_id, "federation_envelope_digest": envelope}), "media_type": "application/vnd.aurora.federated-context-workbench+json"}
    receipt = FederatedContextWorkbenchReceipt(session_id=session_id, federation_id=federation_id, query_id=query_id, goal=goal, semantic_profile=semantic_profile, disposition=disposition, institution_order=institutions, qualified_institution_order=tuple(sorted(qualified)), stale_institution_order=tuple(sorted(stale)), blocked_institution_order=tuple(sorted(blocked)), unknown_institution_order=tuple(sorted(unknown)), view_order=tuple(sorted(views)), action_order=tuple(sorted(actions)), blocked_action_order=tuple(sorted(blocked_actions)), aggregate_order=tuple(sorted(aggregate)), quorum=quorum, minimum_quorum=minimum_quorum, current_epoch=current_epoch, budget_units=budget_units, consumed_budget_units=consumed, checkpoint_digest=checkpoint, federation_envelope_digest=envelope, replay_identity=replay_identity, omissions=tuple(sorted(omissions)), uncertainty=tuple(sorted(uncertainty)), negative_evidence=tuple(sorted(negative)), effect_receipts=("block:unsafe-release",) if disposition == "blocked" else (f"view:local-federated-context-workbench:{session_id}",), artifact=artifact)
    receipt.validate(); return receipt
