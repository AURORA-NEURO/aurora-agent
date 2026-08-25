"""Federated continual context-to-Decision-Section projection parity contract."""
from __future__ import annotations

from dataclasses import dataclass
import re
from typing import Any, Mapping, Sequence

from .research_contracts import (
    FEDERATED_DECISION_PROJECTION_CONTRACT_VERSION,
    FEDERATED_DECISION_PROJECTION_FEATURE_ID,
    PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
    ResearchContractError,
    research_artifact_digest,
)


@dataclass(frozen=True)
class PeerDecisionAttestation:
    institution_id: str
    epoch: int
    context_digest: str
    section_digest: str
    evidence_state: str
    replay_identity: str
    policy_allow: bool = True
    protected_closure: bool = True
    raw_data_local: bool = True
    aggregate_only: bool = True
    boundary: str = PRECLINICAL_BOUNDARY


@dataclass(frozen=True)
class FederatedDecisionProjectionReceipt:
    request_id: str
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
    aggregate_order: tuple[str, ...]
    quorum: int
    minimum_quorum: int
    current_epoch: int
    context_digest: str
    section_digest: str
    federation_envelope_digest: str
    replay_identity: str
    omissions: tuple[str, ...]
    uncertainty: tuple[str, ...]
    negative_evidence: tuple[str, ...]
    effect_receipts: tuple[str, ...]
    artifact: Mapping[str, Any]
    feature_id: str = FEDERATED_DECISION_PROJECTION_FEATURE_ID
    contract_version: str = FEDERATED_DECISION_PROJECTION_CONTRACT_VERSION
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    raw_data_local: bool = True
    aggregate_only: bool = True
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION or self.contract_version != FEDERATED_DECISION_PROJECTION_CONTRACT_VERSION or self.feature_id != FEDERATED_DECISION_PROJECTION_FEATURE_ID:
            raise ResearchContractError("federated projection schema, feature, or version mismatch")
        if self.boundary != PRECLINICAL_BOUNDARY or not self.raw_data_local or not self.aggregate_only or not self.request_id.strip() or not self.federation_id.strip() or not self.query_id.strip() or not self.goal.strip() or not self.semantic_profile.strip() or len(self.institution_order) < 2 or not self.aggregate_order or not self.effect_receipts or not self.disposition:
            raise ResearchContractError("federated projection identity, quorum, locality, aggregate-only, or effects are incomplete")
        if self.minimum_quorum < 1 or self.quorum != len(self.qualified_institution_order) or self.quorum > len(self.institution_order):
            raise ResearchContractError("federated quorum is invalid")
        vectors = (self.institution_order, self.qualified_institution_order, self.stale_institution_order, self.blocked_institution_order, self.unknown_institution_order, self.aggregate_order, self.omissions, self.uncertainty, self.negative_evidence, self.effect_receipts)
        if any(tuple(sorted(set(values))) != values for values in vectors):
            raise ResearchContractError("federated projection vectors are not canonical")
        classified = set(self.qualified_institution_order) | set(self.stale_institution_order) | set(self.blocked_institution_order) | set(self.unknown_institution_order)
        if classified != set(self.institution_order):
            raise ResearchContractError("federated peer states do not partition institutions")
        for value in (self.context_digest, self.section_digest, self.federation_envelope_digest, self.replay_identity):
            if not isinstance(value, str) or not re.fullmatch(r"[0-9a-f]{64}", value):
                raise ResearchContractError("federated projection digest is invalid")
        if any(not isinstance(value, str) or not re.fullmatch(r"[0-9a-f]{64}", value) for value in self.aggregate_order):
            raise ResearchContractError("federated aggregate entries must be digests")
        if any(not effect.startswith("project:federated-decision-section:") and effect != "block:unsafe-release" for effect in self.effect_receipts):
            raise ResearchContractError("federated projection effect is outside release gate")
        if not isinstance(self.artifact.get("content_hash"), str) or not re.fullmatch(r"[0-9a-f]{64}", self.artifact["content_hash"]):
            raise ResearchContractError("federated projection artifact digest is invalid")

    def digest(self) -> str:
        self.validate()
        return research_artifact_digest({
            "schema_version": self.schema_version, "contract_version": self.contract_version, "feature_id": self.feature_id,
            "request_id": self.request_id, "federation_id": self.federation_id, "query_id": self.query_id, "goal": self.goal,
            "semantic_profile": self.semantic_profile, "disposition": self.disposition, "institution_order": list(self.institution_order),
            "qualified_institution_order": list(self.qualified_institution_order), "stale_institution_order": list(self.stale_institution_order),
            "blocked_institution_order": list(self.blocked_institution_order), "unknown_institution_order": list(self.unknown_institution_order),
            "aggregate_order": list(self.aggregate_order), "quorum": self.quorum, "minimum_quorum": self.minimum_quorum,
            "current_epoch": self.current_epoch, "context_digest": self.context_digest, "section_digest": self.section_digest,
            "federation_envelope_digest": self.federation_envelope_digest, "replay_identity": self.replay_identity,
            "omissions": list(self.omissions), "uncertainty": list(self.uncertainty), "negative_evidence": list(self.negative_evidence),
            "effect_receipts": list(self.effect_receipts), "artifact": dict(self.artifact), "raw_data_local": self.raw_data_local,
            "aggregate_only": self.aggregate_only, "boundary": self.boundary,
        })


def project_federated_decision_section(*, request_id: str, federation_id: str, query_id: str, goal: str, semantic_profile: str, required_institution_ids: Sequence[str], attestations: Sequence[PeerDecisionAttestation], minimum_quorum: int, current_epoch: int, max_epoch_lag: int, replay_identity: str, policy_allow: bool = True, protected_closure: bool = True, signed_approval: bool = True, raw_data_local: bool = True, aggregate_only: bool = True) -> FederatedDecisionProjectionReceipt:
    if not request_id.strip() or not federation_id.strip() or not query_id.strip() or not goal.strip() or not semantic_profile.strip() or len(required_institution_ids) < 2 or minimum_quorum < 1 or minimum_quorum > len(required_institution_ids) or not re.fullmatch(r"[0-9a-f]{64}", replay_identity):
        raise ResearchContractError("federated projection identity, quorum, replay, or boundary is invalid")
    institutions = tuple(sorted(set(required_institution_ids)))
    if len(institutions) != len(required_institution_ids) or any(not value.strip() for value in institutions):
        raise ResearchContractError("federated institutions must be unique and non-empty")
    attestation_map: dict[str, PeerDecisionAttestation] = {}
    for peer in attestations:
        if peer.institution_id in attestation_map:
            raise ResearchContractError("federated attestations must be unique")
        attestation_map[peer.institution_id] = peer
    qualified: set[str] = set(); stale: set[str] = set(); blocked: set[str] = set(); unknown: set[str] = set(); aggregate: set[str] = set(); omissions: set[str] = set(); uncertainty: set[str] = set(); negative: set[str] = set()
    for institution_id in institutions:
        peer = attestation_map.get(institution_id)
        if peer is None:
            unknown.add(institution_id); omissions.add(f"institution:{institution_id}:missing-attestation"); continue
        if not policy_allow or not protected_closure or not signed_approval or not raw_data_local or not aggregate_only or not peer.policy_allow or not peer.protected_closure or not peer.raw_data_local or not peer.aggregate_only or peer.boundary != PRECLINICAL_BOUNDARY:
            blocked.add(institution_id); omissions.add(f"institution:{institution_id}:federation-gate-blocked"); continue
        if peer.replay_identity != replay_identity:
            unknown.add(institution_id); uncertainty.add(f"institution:{institution_id}:replay-mismatch"); continue
        if peer.epoch > current_epoch or current_epoch - peer.epoch > max_epoch_lag:
            stale.add(institution_id); omissions.add(f"institution:{institution_id}:stale-epoch"); continue
        if peer.evidence_state in {"proven", "supported"}:
            qualified.add(institution_id)
            aggregate.add(research_artifact_digest({"institution_id": institution_id, "epoch": peer.epoch, "context_digest": peer.context_digest, "section_digest": peer.section_digest, "replay_identity": peer.replay_identity}))
        elif peer.evidence_state in {"speculative", "unknown"}:
            unknown.add(institution_id); uncertainty.add(f"institution:{institution_id}:evidence-uncertain")
        else:
            blocked.add(institution_id); negative.add(f"institution:{institution_id}:contradicted-attestation")
    quorum = len(qualified)
    gates_open = policy_allow and protected_closure and signed_approval and raw_data_local and aggregate_only
    disposition = "blocked" if not gates_open else ("admitted" if quorum >= minimum_quorum else "refinement_required")
    if disposition != "admitted" and not stale and not unknown and not blocked:
        omissions.add("federation:quorum-not-reached")
    context_digest = research_artifact_digest({"institution_order": list(institutions), "qualified_order": sorted(qualified), "stale_order": sorted(stale), "blocked_order": sorted(blocked), "unknown_order": sorted(unknown), "replay_identity": replay_identity})
    section_digest = research_artifact_digest({"query_id": query_id, "goal": goal, "semantic_profile": semantic_profile, "aggregate_order": sorted(aggregate), "context_digest": context_digest, "quorum": quorum, "minimum_quorum": minimum_quorum})
    federation_envelope_digest = research_artifact_digest({"federation_id": federation_id, "purpose": goal, "aggregate_order": sorted(aggregate), "section_digest": section_digest, "raw_data_local": True, "aggregate_only": True})
    effects = (f"project:federated-decision-section:{federation_id}",) if disposition == "admitted" else ("block:unsafe-release",)
    artifact = {"content_hash": research_artifact_digest({"request_id": request_id, "section_digest": section_digest}), "media_type": "application/vnd.aurora.federated-decision-projection+json"}
    receipt = FederatedDecisionProjectionReceipt(request_id=request_id, federation_id=federation_id, query_id=query_id, goal=goal, semantic_profile=semantic_profile, disposition=disposition, institution_order=institutions, qualified_institution_order=tuple(sorted(qualified)), stale_institution_order=tuple(sorted(stale)), blocked_institution_order=tuple(sorted(blocked)), unknown_institution_order=tuple(sorted(unknown)), aggregate_order=tuple(sorted(aggregate)), quorum=quorum, minimum_quorum=minimum_quorum, current_epoch=current_epoch, context_digest=context_digest, section_digest=section_digest, federation_envelope_digest=federation_envelope_digest, replay_identity=replay_identity, omissions=tuple(sorted(omissions)), uncertainty=tuple(sorted(uncertainty)), negative_evidence=tuple(sorted(negative)), effect_receipts=effects, artifact=artifact)
    receipt.validate(); return receipt
