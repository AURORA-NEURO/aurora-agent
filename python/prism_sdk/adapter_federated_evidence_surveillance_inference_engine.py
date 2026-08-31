"""Federated continual evidence-surveillance inference parity surface."""
from __future__ import annotations

from dataclasses import dataclass
import re
from typing import Any, Mapping, Sequence

from .research_contracts import (
    ADAPTER_FEDERATED_EVIDENCE_SURVEILLANCE_INFERENCE_ENGINE_CONTRACT_VERSION,
    ADAPTER_FEDERATED_EVIDENCE_SURVEILLANCE_INFERENCE_ENGINE_FEATURE_ID,
    PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
    ResearchContractError,
    research_artifact_digest,
)


@dataclass(frozen=True)
class FederatedEvidenceObservation:
    peer_id: str
    institution_id: str
    source_id: str
    semantic_profile: str
    artifact_kind: str
    digest: str | None
    availability: str = "available"
    evidence_state: str = "supported"
    relevance_score: int = 0
    signed: bool = True
    permitted_artifact: bool = True
    aggregate_only: bool = True
    raw_data_local: bool = True
    negative_result: bool = False


@dataclass(frozen=True)
class FederatedEvidenceSurveillanceReceipt:
    request_id: str
    federation_id: str
    purpose: str
    endpoint: str
    disposition: str
    peer_order: tuple[str, ...]
    candidate_order: tuple[str, ...]
    ranked_order: tuple[str, ...]
    selected_order: tuple[str, ...]
    unresolved_order: tuple[str, ...]
    denied_order: tuple[str, ...]
    aggregate_order: tuple[str, ...]
    federation_digest: str
    envelope_digest: str
    evidence_digest: str
    provenance_digest: str
    replay_identity: str
    omissions: tuple[str, ...]
    uncertainty: tuple[str, ...]
    negative_evidence: tuple[str, ...]
    effect_receipts: tuple[str, ...]
    qualified_set: Mapping[str, Any]
    artifact: Mapping[str, Any]
    feature_id: str = ADAPTER_FEDERATED_EVIDENCE_SURVEILLANCE_INFERENCE_ENGINE_FEATURE_ID
    contract_version: str = ADAPTER_FEDERATED_EVIDENCE_SURVEILLANCE_INFERENCE_ENGINE_CONTRACT_VERSION
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    raw_data_local: bool = True
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION or self.feature_id != ADAPTER_FEDERATED_EVIDENCE_SURVEILLANCE_INFERENCE_ENGINE_FEATURE_ID or self.contract_version != ADAPTER_FEDERATED_EVIDENCE_SURVEILLANCE_INFERENCE_ENGINE_CONTRACT_VERSION:
            raise ResearchContractError("federated evidence schema, feature, or version mismatch")
        if self.boundary != PRECLINICAL_BOUNDARY or not self.raw_data_local or not self.request_id.strip() or not self.federation_id.strip() or not self.purpose.strip() or not self.endpoint.strip() or not self.candidate_order or not self.effect_receipts or self.qualified_set.get("federation_id") != self.federation_id:
            raise ResearchContractError("federated identity, locality, candidates, effects, or qualified-set linkage is incomplete")
        for values in (self.peer_order, self.candidate_order, self.selected_order, self.unresolved_order, self.denied_order, self.aggregate_order, self.omissions, self.uncertainty, self.negative_evidence, self.effect_receipts, tuple(self.qualified_set.get("peer_order", ())), tuple(self.qualified_set.get("selected_order", ())), tuple(self.qualified_set.get("aggregate_order", ())), tuple(self.qualified_set.get("omissions", ())), tuple(self.qualified_set.get("uncertainty", ())), tuple(self.qualified_set.get("negative_order", ()))):
            if tuple(sorted(set(values))) != values:
                raise ResearchContractError("federated ordering is not canonical")
        if len(self.ranked_order) != len(self.candidate_order) or set(self.ranked_order) != set(self.candidate_order):
            raise ResearchContractError("federated ranking must cover candidates exactly")
        if set(self.selected_order) | set(self.unresolved_order) | set(self.denied_order) != set(self.candidate_order) or tuple(self.qualified_set.get("selected_order", ())) != self.selected_order:
            raise ResearchContractError("federated states do not partition candidates")
        for value in (self.federation_digest, self.envelope_digest, self.evidence_digest, self.provenance_digest, self.replay_identity, self.artifact.get("content_hash")):
            if not isinstance(value, str) or not re.fullmatch(r"[0-9a-f]{64}", value):
                raise ResearchContractError("federated digest is invalid")
        if any(not effect.startswith("exchange:aggregate-evidence:") and effect != "block:unsafe-release" for effect in self.effect_receipts):
            raise ResearchContractError("federated effect is outside aggregate exchange gate")
        if self.disposition == "blocked" and self.effect_receipts != ("block:unsafe-release",):
            raise ResearchContractError("blocked federated surveillance must be explicitly blocked")

    def digest(self) -> str:
        self.validate()
        return research_artifact_digest({"schema_version": self.schema_version, "contract_version": self.contract_version, "feature_id": self.feature_id, "request_id": self.request_id, "federation_id": self.federation_id, "purpose": self.purpose, "endpoint": self.endpoint, "disposition": self.disposition, "peer_order": list(self.peer_order), "candidate_order": list(self.candidate_order), "ranked_order": list(self.ranked_order), "selected_order": list(self.selected_order), "unresolved_order": list(self.unresolved_order), "denied_order": list(self.denied_order), "aggregate_order": list(self.aggregate_order), "federation_digest": self.federation_digest, "envelope_digest": self.envelope_digest, "evidence_digest": self.evidence_digest, "provenance_digest": self.provenance_digest, "replay_identity": self.replay_identity, "omissions": list(self.omissions), "uncertainty": list(self.uncertainty), "negative_evidence": list(self.negative_evidence), "effect_receipts": list(self.effect_receipts), "qualified_set": dict(self.qualified_set), "artifact": dict(self.artifact), "raw_data_local": self.raw_data_local, "boundary": self.boundary})


def run_federated_evidence_surveillance(*, request_id: str, federation_id: str, purpose: str, endpoint: str, semantic_profile: str, allowed_artifacts: Sequence[str], min_peer_quorum: int, observations: Sequence[FederatedEvidenceObservation], policy_allow: bool = True, protected_closure: bool = True, raw_data_local: bool = True, replay_identity: str) -> FederatedEvidenceSurveillanceReceipt:
    if not request_id.strip() or not federation_id.strip() or not purpose.strip() or not endpoint.strip() or not semantic_profile.strip() or not allowed_artifacts or min_peer_quorum <= 0 or not observations or not re.fullmatch(r"[0-9a-f]{64}", replay_identity) or not raw_data_local:
        raise ResearchContractError("federated identity, purpose, endpoint, quorum, allow-list, observations, replay, or locality is invalid")
    ordered = tuple(sorted(observations, key=lambda item: (-item.relevance_score, item.peer_id, item.source_id))); key = lambda item: f"{item.peer_id}::{item.institution_id}::{item.source_id}"; ranked = tuple(key(item) for item in ordered); candidate = tuple(sorted(ranked))
    if len(set(candidate)) != len(candidate) or any(not item.peer_id.strip() or not item.institution_id.strip() for item in ordered):
        raise ResearchContractError("federated observation identities must be unique and non-empty")
    selected: set[str] = set(); unresolved: set[str] = set(); denied: set[str] = set(); peers: set[str] = set(); aggregate: set[str] = set(); digest_map: dict[str, str] = {}; omissions: set[str] = set(); uncertainty: set[str] = set(); negative: set[str] = set()
    for item in ordered:
        item_key = key(item)
        if not policy_allow or not protected_closure or not raw_data_local or not item.raw_data_local:
            denied.add(item_key); omissions.add(f"evidence:{item_key}:policy-closure-locality")
        elif not item.signed:
            denied.add(item_key); omissions.add(f"evidence:{item_key}:signature-missing")
        elif not item.permitted_artifact or item.artifact_kind not in allowed_artifacts:
            denied.add(item_key); omissions.add(f"evidence:{item_key}:artifact-not-permitted")
        elif not item.aggregate_only:
            denied.add(item_key); negative.add(f"evidence:{item_key}:raw-observation-export-denied")
        elif item.semantic_profile != semantic_profile:
            denied.add(item_key); omissions.add(f"evidence:{item_key}:semantic-profile-mismatch"); negative.add(f"evidence:{item_key}:incomparable")
        elif item.availability != "available":
            unresolved.add(item_key); omissions.add(f"evidence:{item_key}:availability-{item.availability}")
        elif item.digest is None:
            unresolved.add(item_key); omissions.add(f"evidence:{item_key}:content-digest-missing")
        elif item.evidence_state in {"unknown", "speculative"}:
            unresolved.add(item_key); uncertainty.add(f"evidence:{item_key}:unknown-not-asserted")
        elif item.evidence_state == "contradicted":
            denied.add(item_key); negative.add(f"evidence:{item_key}:contradicted")
        else:
            selected.add(item_key); peers.add(item.peer_id); aggregate.add(item_key); digest_map[item_key] = item.digest
            if item.negative_result: negative.add(f"evidence:{item_key}:negative-result")
    if len(peers) < min_peer_quorum:
        omissions.add(f"federation:quorum-incomplete:{len(peers)}<{min_peer_quorum}"); uncertainty.add("federation:quorum-unresolved")
    if not policy_allow: omissions.add("control:policy-denied")
    if not protected_closure: omissions.add("control:protected-closure-incomplete")
    if not raw_data_local: omissions.add("control:raw-data-locality-failed")
    disposition = "blocked" if not policy_allow or not protected_closure or not raw_data_local else "unknown" if not selected else "partial" if unresolved or denied or len(peers) < min_peer_quorum else "completed"
    peer_order = tuple(sorted(peers)); selected_order = tuple(sorted(selected)); unresolved_order = tuple(sorted(unresolved)); denied_order = tuple(sorted(denied)); aggregate_order = tuple(sorted(aggregate)); omissions_order = tuple(sorted(omissions)); uncertainty_order = tuple(sorted(uncertainty)); negative_order = tuple(sorted(negative))
    federation_digest = research_artifact_digest({"federation_id": federation_id, "purpose": purpose, "endpoint": endpoint, "peer_order": list(peer_order), "semantic_profile": semantic_profile})
    envelope_digest = research_artifact_digest({"aggregate_order": list(aggregate_order), "allowed_artifacts": sorted(allowed_artifacts), "raw_data_local": raw_data_local, "aggregate_only": True, "federation_digest": federation_digest})
    evidence_digest = research_artifact_digest({"candidate_order": list(candidate), "selected_order": list(selected_order), "unresolved_order": list(unresolved_order), "denied_order": list(denied_order)})
    provenance_digest = research_artifact_digest({"request_id": request_id, "replay_identity": replay_identity, "envelope_digest": envelope_digest, "evidence_digest": evidence_digest})
    qualified_set = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "set_id": f"qualified-federated-evidence:{federation_id}", "federation_id": federation_id, "purpose": purpose, "peer_order": list(peer_order), "selected_order": list(selected_order), "selected_digests": [digest_map[item] for item in selected_order], "aggregate_order": list(aggregate_order), "omissions": list(omissions_order), "uncertainty": list(uncertainty_order), "negative_order": list(negative_order), "boundary": PRECLINICAL_BOUNDARY}
    artifact = {"content_hash": research_artifact_digest(qualified_set), "media_type": "application/vnd.aurora.qualified-federated-evidence-set+json"}
    receipt = FederatedEvidenceSurveillanceReceipt(request_id=request_id, federation_id=federation_id, purpose=purpose, endpoint=endpoint, disposition=disposition, peer_order=peer_order, candidate_order=candidate, ranked_order=ranked, selected_order=selected_order, unresolved_order=unresolved_order, denied_order=denied_order, aggregate_order=aggregate_order, federation_digest=federation_digest, envelope_digest=envelope_digest, evidence_digest=evidence_digest, provenance_digest=provenance_digest, replay_identity=replay_identity, omissions=omissions_order, uncertainty=uncertainty_order, negative_evidence=negative_order, effect_receipts=(f"exchange:aggregate-evidence:{federation_id}",) if disposition != "blocked" else ("block:unsafe-release",), qualified_set=qualified_set, artifact=artifact, raw_data_local=raw_data_local)
    receipt.validate(); return receipt
