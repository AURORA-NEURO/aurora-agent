"""Federated-continual context compilation control-plane parity contract."""
from __future__ import annotations

from dataclasses import dataclass
import re
from typing import Any, Mapping, Sequence

from .research_contracts import (
    FEDERATED_CONTINUAL_CONTEXT_COMPILATION_FEDERATED_CONTROL_PLANE_CONTRACT_VERSION,
    FEDERATED_CONTINUAL_CONTEXT_COMPILATION_FEDERATED_CONTROL_PLANE_FEATURE_ID,
    PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
    ResearchContractError,
    research_artifact_digest,
)


@dataclass(frozen=True)
class FederatedContinualContextControlPeer:
    peer_id: str
    institution_id: str
    context_digest: str
    section_digest: str
    evidence_digest: str | None
    provenance_digest: str | None
    replay_identity: str
    semantic_profile: str = "preclinical-v1"
    fresh: bool = True
    comparable: bool = True
    permitted_summary: bool = True
    signed_approval: bool = True
    policy_allow: bool = True
    ready: bool = True
    state: str = "supported"
    raw_data_local: bool = True
    boundary: str = PRECLINICAL_BOUNDARY


@dataclass(frozen=True)
class FederatedContinualContextControlReceipt:
    request_id: str
    federation_id: str
    round_id: str
    disposition: str
    candidate_order: tuple[str, ...]
    qualified_order: tuple[str, ...]
    degraded_order: tuple[str, ...]
    unresolved_order: tuple[str, ...]
    denied_order: tuple[str, ...]
    exchange_order: tuple[str, ...]
    semantic_profile_order: tuple[str, ...]
    freshness_order: tuple[str, ...]
    checkpoint_seq: int
    quorum_required: int
    quorum_met: bool
    run_digest: str
    telemetry_digest: str
    federation_digest: str
    replay_identity: str
    witness_order: tuple[str, ...]
    counterexample_order: tuple[str, ...]
    omissions: tuple[str, ...]
    uncertainty: tuple[str, ...]
    negative_evidence: tuple[str, ...]
    effect_receipts: tuple[str, ...]
    artifact: Mapping[str, Any]
    feature_id: str = FEDERATED_CONTINUAL_CONTEXT_COMPILATION_FEDERATED_CONTROL_PLANE_FEATURE_ID
    contract_version: str = FEDERATED_CONTINUAL_CONTEXT_COMPILATION_FEDERATED_CONTROL_PLANE_CONTRACT_VERSION
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    raw_data_local: bool = True
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION or self.feature_id != FEDERATED_CONTINUAL_CONTEXT_COMPILATION_FEDERATED_CONTROL_PLANE_FEATURE_ID or self.contract_version != FEDERATED_CONTINUAL_CONTEXT_COMPILATION_FEDERATED_CONTROL_PLANE_CONTRACT_VERSION:
            raise ResearchContractError("federated continual control schema, feature, or version mismatch")
        if self.boundary != PRECLINICAL_BOUNDARY or not self.raw_data_local or not self.request_id.strip() or not self.federation_id.strip() or not self.round_id.strip() or not self.candidate_order or self.checkpoint_seq != len(self.candidate_order) or self.quorum_required <= 0 or not self.effect_receipts or self.disposition not in {"completed", "degraded", "unresolved", "denied"}:
            raise ResearchContractError("federated continual identity, checkpoint, quorum, locality, disposition, or effects are incomplete")
        for values in (self.candidate_order, self.qualified_order, self.degraded_order, self.unresolved_order, self.denied_order, self.semantic_profile_order, self.freshness_order, self.witness_order, self.counterexample_order, self.omissions, self.uncertainty, self.negative_evidence, self.effect_receipts):
            if tuple(sorted(set(values))) != values:
                raise ResearchContractError("federated continual ordering is not canonical")
        if tuple(sorted(set(self.exchange_order))) != self.exchange_order:
            raise ResearchContractError("federated continual exchange ordering is not canonical")
        classified = set(self.qualified_order) | set(self.degraded_order) | set(self.unresolved_order) | set(self.denied_order)
        if classified != set(self.candidate_order):
            raise ResearchContractError("federated continual dispositions do not partition peers")
        if len(self.exchange_order) != len(self.qualified_order):
            raise ResearchContractError("federated continual exchange does not match qualified peers")
        for value in (*self.exchange_order, self.run_digest, self.telemetry_digest, self.federation_digest, self.replay_identity, self.artifact.get("content_hash")):
            if not isinstance(value, str) or not re.fullmatch(r"[0-9a-f]{64}", value):
                raise ResearchContractError("federated continual digest is invalid")
        if any(not effect.startswith("exchange:permitted-federated-context-summary:") and not effect.startswith("manage:federated-context:") and effect != "block:unsafe-release" for effect in self.effect_receipts):
            raise ResearchContractError("federated continual effect is outside the governed operations gate")

    def digest(self) -> str:
        self.validate()
        return research_artifact_digest({"schema_version": self.schema_version, "contract_version": self.contract_version, "feature_id": self.feature_id, "request_id": self.request_id, "federation_id": self.federation_id, "round_id": self.round_id, "disposition": self.disposition, "candidate_order": list(self.candidate_order), "qualified_order": list(self.qualified_order), "degraded_order": list(self.degraded_order), "unresolved_order": list(self.unresolved_order), "denied_order": list(self.denied_order), "exchange_order": list(self.exchange_order), "semantic_profile_order": list(self.semantic_profile_order), "freshness_order": list(self.freshness_order), "checkpoint_seq": self.checkpoint_seq, "quorum_required": self.quorum_required, "quorum_met": self.quorum_met, "run_digest": self.run_digest, "telemetry_digest": self.telemetry_digest, "federation_digest": self.federation_digest, "replay_identity": self.replay_identity, "witness_order": list(self.witness_order), "counterexample_order": list(self.counterexample_order), "omissions": list(self.omissions), "uncertainty": list(self.uncertainty), "negative_evidence": list(self.negative_evidence), "effect_receipts": list(self.effect_receipts), "artifact": dict(self.artifact), "raw_data_local": self.raw_data_local, "boundary": self.boundary})


def operate_federated_continual_context_compilation(*, request_id: str, federation_id: str, round_id: str, peers: Sequence[FederatedContinualContextControlPeer], min_quorum: int, replay_identity: str, policy_allow: bool = True, protected_closure: bool = True, raw_data_local: bool = True, signed_approval: bool = True) -> FederatedContinualContextControlReceipt:
    if not request_id.strip() or not federation_id.strip() or not round_id.strip() or not peers or min_quorum <= 0 or min_quorum > len(peers) or not re.fullmatch(r"[0-9a-f]{64}", replay_identity):
        raise ResearchContractError("federated continual identity, peer set, quorum, or replay is invalid")
    ordered = tuple(sorted(peer.peer_id for peer in peers))
    if len(set(ordered)) != len(peers) or any(not value.strip() for value in ordered):
        raise ResearchContractError("federated continual peer identifiers must be unique and non-empty")
    peer_map = {peer.peer_id: peer for peer in peers}; qualified: set[str] = set(); degraded: set[str] = set(); unresolved: set[str] = set(); denied: set[str] = set(); exchanges: list[str] = []
    semantic_profiles: set[str] = set(); fresh: set[str] = set(); witnesses = {"gate:typed-federated-context-contract", "gate:freshness", "gate:semantic-comparability", "gate:quorum", "gate:replay-identity", "gate:aggregate-only", "gate:locality"}; counter: set[str] = set(); omissions: set[str] = set(); uncertainty: set[str] = set(); negative: set[str] = set(); open_gate = policy_allow and protected_closure and raw_data_local and signed_approval
    for peer_id in ordered:
        peer = peer_map[peer_id]
        if not open_gate or not peer.policy_allow or not peer.signed_approval or not peer.permitted_summary or not peer.raw_data_local or peer.boundary != PRECLINICAL_BOUNDARY:
            denied.add(peer_id); counter.add(f"counterexample:{peer_id}:policy-approval-locality-or-purpose")
        elif not peer.fresh:
            unresolved.add(peer_id); omissions.add(f"peer:{peer_id}:stale-context")
        elif not peer.comparable or not peer.semantic_profile.strip():
            denied.add(peer_id); counter.add(f"counterexample:{peer_id}:semantic-profile-mismatch")
        elif not peer.ready:
            unresolved.add(peer_id); uncertainty.add(f"peer:{peer_id}:not-ready")
        elif peer.replay_identity != replay_identity:
            unresolved.add(peer_id); uncertainty.add(f"peer:{peer_id}:replay-mismatch")
        elif peer.evidence_digest is None or peer.provenance_digest is None:
            unresolved.add(peer_id); omissions.add(f"peer:{peer_id}:evidence-or-provenance-missing")
        elif peer.state in {"unknown", "speculative"}:
            unresolved.add(peer_id); uncertainty.add(f"peer:{peer_id}:evidence-uncertain")
        elif peer.state == "contradicted":
            denied.add(peer_id); negative.add(f"peer:{peer_id}:contradicted")
        else:
            qualified.add(peer_id); semantic_profiles.add(peer.semantic_profile); fresh.add(peer_id); exchanges.append(research_artifact_digest({"peer_id": peer.peer_id, "institution_id": peer.institution_id, "context_digest": peer.context_digest, "section_digest": peer.section_digest, "evidence_digest": peer.evidence_digest, "provenance_digest": peer.provenance_digest, "semantic_profile": peer.semantic_profile, "replay_identity": peer.replay_identity}))
    quorum_met = len(qualified) >= min_quorum
    if not quorum_met:
        uncertainty.add(f"quorum:required-{min_quorum}:observed-{len(qualified)}"); omissions.add("federation:quorum-incomplete")
    if not policy_allow: counter.add("counterexample:policy-denied"); omissions.add("control:policy-denied")
    if not protected_closure: counter.add("counterexample:protected-closure-incomplete"); omissions.add("control:protected-closure-incomplete")
    if not signed_approval: counter.add("counterexample:signed-approval-missing"); omissions.add("control:signed-approval-missing")
    if unresolved or degraded: witnesses.add("gate:partial-peer-results-retained")
    exchange_order = tuple(sorted(exchanges)); disposition = "denied" if not open_gate or denied else "unresolved" if not quorum_met else "degraded" if unresolved else "completed"; telemetry = research_artifact_digest({"feature_id": FEDERATED_CONTINUAL_CONTEXT_COMPILATION_FEDERATED_CONTROL_PLANE_FEATURE_ID, "federation_id": federation_id, "round_id": round_id, "candidate_order": list(ordered), "qualified_order": sorted(qualified)}); federation = research_artifact_digest({"federation_id": federation_id, "round_id": round_id, "exchange_order": list(exchange_order), "quorum_required": min_quorum, "quorum_met": quorum_met, "raw_data_local": raw_data_local}); run = research_artifact_digest({"feature_id": FEDERATED_CONTINUAL_CONTEXT_COMPILATION_FEDERATED_CONTROL_PLANE_FEATURE_ID, "request_id": request_id, "disposition": disposition, "qualified_order": sorted(qualified), "unresolved_order": sorted(unresolved), "denied_order": sorted(denied), "quorum_met": quorum_met, "telemetry_digest": telemetry, "federation_digest": federation, "replay_identity": replay_identity}); artifact = {"content_hash": research_artifact_digest({"request_id": request_id, "run_digest": run}), "media_type": "application/vnd.aurora.federated-continual-context-control+json"}
    receipt = FederatedContinualContextControlReceipt(request_id=request_id, federation_id=federation_id, round_id=round_id, disposition=disposition, candidate_order=ordered, qualified_order=tuple(sorted(qualified)), degraded_order=tuple(sorted(degraded)), unresolved_order=tuple(sorted(unresolved)), denied_order=tuple(sorted(denied)), exchange_order=exchange_order, semantic_profile_order=tuple(sorted(semantic_profiles)), freshness_order=tuple(sorted(fresh)), checkpoint_seq=len(ordered), quorum_required=min_quorum, quorum_met=quorum_met, run_digest=run, telemetry_digest=telemetry, federation_digest=federation, replay_identity=replay_identity, witness_order=tuple(sorted(witnesses)), counterexample_order=tuple(sorted(counter)), omissions=tuple(sorted(omissions)), uncertainty=tuple(sorted(uncertainty)), negative_evidence=tuple(sorted(negative)), effect_receipts=(f"exchange:permitted-federated-context-summary:{request_id}", f"manage:federated-context:{request_id}") if disposition == "completed" else ("block:unsafe-release",), artifact=artifact, raw_data_local=raw_data_local); receipt.validate(); return receipt
