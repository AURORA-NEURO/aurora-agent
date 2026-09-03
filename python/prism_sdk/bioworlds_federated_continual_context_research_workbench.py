"""Federated continual context-compilation researcher workbench parity contract."""
from __future__ import annotations

from dataclasses import dataclass
import re
from typing import Any, Mapping, Sequence

from .research_contracts import (
    BIOWORLDS_FEDERATED_CONTEXT_RESEARCH_WORKBENCH_CONTRACT_VERSION,
    BIOWORLDS_FEDERATED_CONTEXT_RESEARCH_WORKBENCH_FEATURE_ID,
    PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
    ResearchContractError,
    research_artifact_digest,
)


@dataclass(frozen=True)
class BioworldsFederatedContextWorkbenchPeer:
    institution_id: str
    artifact_id: str
    context_digest: str
    section_digest: str
    evidence_digest: str | None
    provenance_digest: str | None
    replay_identity: str
    state: str = "supported"
    fresh: bool = True
    semantic_profile: str = "preclinical:context:v1"
    permitted_artifact: bool = True
    signed_approval: bool = True
    aggregate_only: bool = True
    raw_data_local: bool = True
    boundary: str = PRECLINICAL_BOUNDARY


@dataclass(frozen=True)
class BioworldsFederatedContextWorkbenchReceipt:
    request_id: str
    federation_id: str
    purpose: str
    scope: str
    goal: str
    semantic_profile: str
    verdict: str
    institution_order: tuple[str, ...]
    candidate_order: tuple[str, ...]
    qualified_order: tuple[str, ...]
    blocked_order: tuple[str, ...]
    unknown_order: tuple[str, ...]
    stale_order: tuple[str, ...]
    aggregate_order: tuple[str, ...]
    quorum: int
    minimum_quorum: int
    envelope_digest: str
    verification_digest: str
    replay_identity: str
    witness_order: tuple[str, ...]
    counterexample_order: tuple[str, ...]
    omissions: tuple[str, ...]
    uncertainty: tuple[str, ...]
    negative_evidence: tuple[str, ...]
    effect_receipts: tuple[str, ...]
    artifact: Mapping[str, Any]
    feature_id: str = BIOWORLDS_FEDERATED_CONTEXT_RESEARCH_WORKBENCH_FEATURE_ID
    contract_version: str = BIOWORLDS_FEDERATED_CONTEXT_RESEARCH_WORKBENCH_CONTRACT_VERSION
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    raw_data_local: bool = True
    aggregate_only: bool = True
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if (
            self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            or self.feature_id != BIOWORLDS_FEDERATED_CONTEXT_RESEARCH_WORKBENCH_FEATURE_ID
            or self.contract_version != BIOWORLDS_FEDERATED_CONTEXT_RESEARCH_WORKBENCH_CONTRACT_VERSION
        ):
            raise ResearchContractError("federated workbench schema, feature, or version mismatch")
        if (
            self.boundary != PRECLINICAL_BOUNDARY
            or not self.raw_data_local
            or not self.aggregate_only
            or not self.request_id.strip()
            or not self.federation_id.strip()
            or not self.purpose.strip()
            or not self.scope.strip()
            or not self.goal.strip()
            or not self.semantic_profile.strip()
            or len(self.institution_order) < 2
            or not self.candidate_order
            or not self.witness_order
            or not self.effect_receipts
            or self.minimum_quorum < 1
            or self.quorum != len(self.qualified_order)
            or self.quorum > len(self.candidate_order)
            or self.verdict not in {"qualified", "unresolved", "blocked"}
        ):
            raise ResearchContractError("federated workbench identity, quorum, locality, aggregate-only, or effects are incomplete")
        for values in (
            self.institution_order, self.candidate_order, self.qualified_order, self.blocked_order,
            self.unknown_order, self.stale_order, self.aggregate_order, self.witness_order,
            self.counterexample_order, self.omissions, self.uncertainty, self.negative_evidence,
            self.effect_receipts,
        ):
            if tuple(sorted(set(values))) != values:
                raise ResearchContractError("federated workbench ordering is not canonical")
        classified = set(self.qualified_order) | set(self.blocked_order) | set(self.unknown_order)
        if classified != set(self.candidate_order):
            raise ResearchContractError("federated workbench outcomes do not partition candidates")
        if len(self.aggregate_order) != len(self.qualified_order):
            raise ResearchContractError("federated aggregate order does not match qualified peers")
        for value in (*self.aggregate_order, self.envelope_digest, self.verification_digest, self.replay_identity, self.artifact.get("content_hash")):
            if not isinstance(value, str) or not re.fullmatch(r"[0-9a-f]{64}", value):
                raise ResearchContractError("federated workbench digest is invalid")
        if any(not effect.startswith("view:federated-context-workbench:") and effect != "block:unsafe-release" for effect in self.effect_receipts):
            raise ResearchContractError("federated workbench effect is outside the governed release gate")

    def digest(self) -> str:
        self.validate()
        return research_artifact_digest({"schema_version": self.schema_version, "contract_version": self.contract_version, "feature_id": self.feature_id, "request_id": self.request_id, "federation_id": self.federation_id, "purpose": self.purpose, "scope": self.scope, "goal": self.goal, "semantic_profile": self.semantic_profile, "verdict": self.verdict, "institution_order": list(self.institution_order), "candidate_order": list(self.candidate_order), "qualified_order": list(self.qualified_order), "blocked_order": list(self.blocked_order), "unknown_order": list(self.unknown_order), "stale_order": list(self.stale_order), "aggregate_order": list(self.aggregate_order), "quorum": self.quorum, "minimum_quorum": self.minimum_quorum, "envelope_digest": self.envelope_digest, "verification_digest": self.verification_digest, "replay_identity": self.replay_identity, "witness_order": list(self.witness_order), "counterexample_order": list(self.counterexample_order), "omissions": list(self.omissions), "uncertainty": list(self.uncertainty), "negative_evidence": list(self.negative_evidence), "effect_receipts": list(self.effect_receipts), "artifact": dict(self.artifact), "raw_data_local": self.raw_data_local, "aggregate_only": self.aggregate_only, "boundary": self.boundary})


def bioworlds_federated_context_research_workbench_manifest() -> dict[str, object]:
    return {
        "feature_id": BIOWORLDS_FEDERATED_CONTEXT_RESEARCH_WORKBENCH_FEATURE_ID,
        "version": BIOWORLDS_FEDERATED_CONTEXT_RESEARCH_WORKBENCH_CONTRACT_VERSION,
        "owner_crate": "bioworlds",
        "input_schema": "DecisionQuery4@1",
        "output_schema": "CertifiedDecisionSection5@1",
        "autonomy_tier": "A1",
        "determinism": "byte_stable",
        "boundary": PRECLINICAL_BOUNDARY,
    }


def compile_bioworlds_federated_context_workbench(*, request_id: str, federation_id: str, purpose: str, scope: str, goal: str, semantic_profile: str, institution_ids: Sequence[str], peers: Sequence[BioworldsFederatedContextWorkbenchPeer], minimum_quorum: int, replay_identity: str, policy_allow: bool = True, protected_closure: bool = True, raw_data_local: bool = True, aggregate_only: bool = True, signed_approval: bool = True) -> BioworldsFederatedContextWorkbenchReceipt:
    if not request_id.strip() or not federation_id.strip() or not purpose.strip() or not scope.strip() or not goal.strip() or not semantic_profile.strip() or len(institution_ids) < 2 or minimum_quorum < 1 or not re.fullmatch(r"[0-9a-f]{64}", replay_identity) or not raw_data_local or not aggregate_only:
        raise ResearchContractError("federated workbench identity, quorum, replay, locality, aggregate-only, or boundary is invalid")
    institutions = tuple(sorted(set(institution_ids)))
    if len(institutions) != len(institution_ids) or any(not value.strip() for value in institutions) or minimum_quorum > len(institutions):
        raise ResearchContractError("federated institution identifiers or quorum are invalid")
    peer_map: dict[str, BioworldsFederatedContextWorkbenchPeer] = {}
    for peer in peers:
        if peer.institution_id not in institutions or not peer.artifact_id.strip() or not peer.semantic_profile.strip():
            raise ResearchContractError("federated peer identity is invalid")
        if peer.institution_id in peer_map:
            raise ResearchContractError("federated peers must be unique per institution")
        peer_map[peer.institution_id] = peer
    qualified: set[str] = set(); blocked: set[str] = set(); unknown: set[str] = set(); stale: set[str] = set(); aggregates: list[str] = []
    witnesses: set[str] = {"gate:typed-federated-contract", "gate:institution-closure", "gate:semantic-profile", "gate:freshness", "gate:provenance", "gate:replay-identity", "gate:permitted-aggregate", "gate:quorum"}
    counter: set[str] = set(); omissions: set[str] = set(); uncertainty: set[str] = set(); negative: set[str] = set()
    open_gate = policy_allow and protected_closure and raw_data_local and aggregate_only and signed_approval
    for institution in institutions:
        peer = peer_map.get(institution)
        if peer is None:
            unknown.add(institution); omissions.add(f"institution:{institution}:missing-peer")
        elif not open_gate or not peer.permitted_artifact or not peer.signed_approval or not peer.aggregate_only or not peer.raw_data_local or peer.boundary != PRECLINICAL_BOUNDARY:
            blocked.add(institution); counter.add(f"counterexample:{institution}:permission-approval-aggregate-locality")
        elif peer.semantic_profile != semantic_profile:
            blocked.add(institution); negative.add(f"institution:{institution}:semantic-profile-mismatch")
        elif not peer.fresh:
            unknown.add(institution); stale.add(institution); uncertainty.add(f"institution:{institution}:stale")
        elif peer.replay_identity != replay_identity:
            unknown.add(institution); uncertainty.add(f"institution:{institution}:replay-mismatch")
        elif peer.evidence_digest is None or peer.provenance_digest is None:
            unknown.add(institution); omissions.add(f"institution:{institution}:evidence-or-provenance-missing")
        elif peer.state in {"unknown", "speculative"}:
            unknown.add(institution); uncertainty.add(f"institution:{institution}:evidence-uncertain")
        elif peer.state == "contradicted":
            blocked.add(institution); negative.add(f"institution:{institution}:contradicted")
        else:
            qualified.add(institution); aggregates.append(research_artifact_digest({"institution_id": peer.institution_id, "artifact_id": peer.artifact_id, "context_digest": peer.context_digest, "section_digest": peer.section_digest, "evidence_digest": peer.evidence_digest, "provenance_digest": peer.provenance_digest, "semantic_profile": peer.semantic_profile, "replay_identity": peer.replay_identity}))
    if not policy_allow: counter.add("counterexample:policy-denied"); omissions.add("assurance:policy-denied")
    if not protected_closure: counter.add("counterexample:protected-closure-incomplete"); omissions.add("assurance:protected-closure-incomplete")
    if not signed_approval: counter.add("counterexample:signed-approval-missing"); omissions.add("assurance:signed-approval-missing")
    if unknown or len(qualified) < minimum_quorum: witnesses.add("gate:incomplete-federated-closure-retained")
    aggregate_order = tuple(sorted(aggregates))
    verdict = "blocked" if not open_gate or blocked else "unresolved" if unknown or len(qualified) < minimum_quorum else "qualified"
    envelope = research_artifact_digest({"feature_id": BIOWORLDS_FEDERATED_CONTEXT_RESEARCH_WORKBENCH_FEATURE_ID, "federation_id": federation_id, "semantic_profile": semantic_profile, "aggregate_order": list(aggregate_order), "quorum": len(qualified), "minimum_quorum": minimum_quorum, "replay_identity": replay_identity})
    verification = research_artifact_digest({"feature_id": BIOWORLDS_FEDERATED_CONTEXT_RESEARCH_WORKBENCH_FEATURE_ID, "request_id": request_id, "candidate_order": list(institutions), "qualified_order": sorted(qualified), "blocked_order": sorted(blocked), "unknown_order": sorted(unknown), "stale_order": sorted(stale), "envelope_digest": envelope, "verdict": verdict, "replay_identity": replay_identity})
    artifact = {"content_hash": research_artifact_digest({"request_id": request_id, "verification_digest": verification}), "media_type": "application/vnd.aurora.bioworlds-federated-context-research-workbench+json"}
    receipt = BioworldsFederatedContextWorkbenchReceipt(request_id=request_id, federation_id=federation_id, purpose=purpose, scope=scope, goal=goal, semantic_profile=semantic_profile, verdict=verdict, institution_order=institutions, candidate_order=institutions, qualified_order=tuple(sorted(qualified)), blocked_order=tuple(sorted(blocked)), unknown_order=tuple(sorted(unknown)), stale_order=tuple(sorted(stale)), aggregate_order=aggregate_order, quorum=len(qualified), minimum_quorum=minimum_quorum, envelope_digest=envelope, verification_digest=verification, replay_identity=replay_identity, witness_order=tuple(sorted(witnesses)), counterexample_order=tuple(sorted(counter)), omissions=tuple(sorted(omissions)), uncertainty=tuple(sorted(uncertainty)), negative_evidence=tuple(sorted(negative)), effect_receipts=(f"view:federated-context-workbench:{request_id}",) if verdict == "qualified" else ("block:unsafe-release",), artifact=artifact, raw_data_local=raw_data_local, aggregate_only=aggregate_only)
    receipt.validate(); return receipt

