"""Federated continual knowledge-representation contract parity surface."""
from __future__ import annotations

from dataclasses import dataclass
import re
from typing import Any, Mapping, Sequence

from .research_contracts import (
    FEDERATED_CONTINUAL_KNOWLEDGE_REPRESENTATION_CONTRACT_MODEL_CONTRACT_VERSION,
    FEDERATED_CONTINUAL_KNOWLEDGE_REPRESENTATION_CONTRACT_MODEL_FEATURE_ID,
    PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
    ResearchContractError,
    research_artifact_digest,
)

@dataclass(frozen=True)
class FederatedKnowledgeContractPeer:
    peer_id: str
    institution_id: str
    endpoint: str
    purpose: str
    semantic_profile: str
    freshness_epoch: int
    signed_approval: bool
    permitted_artifact_digest: str | None
    evidence_digest: str | None
    provenance_digest: str | None
    state: str = "supported"
    boundary: str = PRECLINICAL_BOUNDARY

@dataclass(frozen=True)
class FederatedKnowledgeContractReceipt:
    request_id: str
    federation_id: str
    purpose: str
    semantic_profile: str
    disposition: str
    input_schema: str
    output_schema: str
    source_revision: int
    target_revision: int
    min_freshness_epoch: int
    quorum_required: int
    quorum_met: bool
    candidate_order: tuple[str, ...]
    qualified_order: tuple[str, ...]
    unresolved_order: tuple[str, ...]
    denied_order: tuple[str, ...]
    stale_order: tuple[str, ...]
    semantic_mismatch_order: tuple[str, ...]
    signer_order: tuple[str, ...]
    exchange_order: tuple[str, ...]
    contract_digest: str
    federation_digest: str
    migration_digest: str
    replay_identity: str
    omissions: tuple[str, ...]
    uncertainty: tuple[str, ...]
    negative_evidence: tuple[str, ...]
    effect_receipts: tuple[str, ...]
    artifact: Mapping[str, Any]
    feature_id: str = FEDERATED_CONTINUAL_KNOWLEDGE_REPRESENTATION_CONTRACT_MODEL_FEATURE_ID
    contract_version: str = FEDERATED_CONTINUAL_KNOWLEDGE_REPRESENTATION_CONTRACT_MODEL_CONTRACT_VERSION
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    raw_data_local: bool = True
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION or self.feature_id != FEDERATED_CONTINUAL_KNOWLEDGE_REPRESENTATION_CONTRACT_MODEL_FEATURE_ID or self.contract_version != FEDERATED_CONTINUAL_KNOWLEDGE_REPRESENTATION_CONTRACT_MODEL_CONTRACT_VERSION:
            raise ResearchContractError("federated knowledge contract schema, feature, or version mismatch")
        if self.boundary != PRECLINICAL_BOUNDARY or not self.raw_data_local or not self.request_id.strip() or not self.federation_id.strip() or not self.purpose.strip() or not self.semantic_profile.strip() or self.input_schema != "ScopedResearchClaims1@1" or self.output_schema != "TypedKnowledgeWorld1@1" or self.source_revision <= 0 or self.target_revision < self.source_revision or self.quorum_required <= 0 or not self.candidate_order or not self.effect_receipts:
            raise ResearchContractError("federated contract identity, schema, purpose, quorum, locality, or effects are incomplete")
        for values in (self.candidate_order, self.qualified_order, self.unresolved_order, self.denied_order, self.stale_order, self.semantic_mismatch_order, self.signer_order, self.exchange_order, self.omissions, self.uncertainty, self.negative_evidence, self.effect_receipts):
            if tuple(sorted(set(values))) != values:
                raise ResearchContractError("federated contract ordering is not canonical")
        classified = set(self.qualified_order) | set(self.unresolved_order) | set(self.denied_order)
        if classified != set(self.candidate_order) or any(value not in self.qualified_order for value in self.exchange_order):
            raise ResearchContractError("federated contract states do not partition peers")
        if self.quorum_met and len(self.qualified_order) < self.quorum_required:
            raise ResearchContractError("federated quorum witness is inconsistent")
        for value in (self.contract_digest, self.federation_digest, self.migration_digest, self.replay_identity, self.artifact.get("content_hash")):
            if not isinstance(value, str) or not re.fullmatch(r"[0-9a-f]{64}", value):
                raise ResearchContractError("federated contract digest is invalid")
        if any(not effect.startswith("exchange:permitted-federated-knowledge:") and effect != "block:unsafe-release" for effect in self.effect_receipts):
            raise ResearchContractError("federated contract effect is outside aggregate-only gate")

    def digest(self) -> str:
        self.validate()
        return research_artifact_digest({"schema_version": self.schema_version, "contract_version": self.contract_version, "feature_id": self.feature_id, "request_id": self.request_id, "federation_id": self.federation_id, "purpose": self.purpose, "semantic_profile": self.semantic_profile, "disposition": self.disposition, "input_schema": self.input_schema, "output_schema": self.output_schema, "source_revision": self.source_revision, "target_revision": self.target_revision, "min_freshness_epoch": self.min_freshness_epoch, "quorum_required": self.quorum_required, "quorum_met": self.quorum_met, "candidate_order": list(self.candidate_order), "qualified_order": list(self.qualified_order), "unresolved_order": list(self.unresolved_order), "denied_order": list(self.denied_order), "stale_order": list(self.stale_order), "semantic_mismatch_order": list(self.semantic_mismatch_order), "signer_order": list(self.signer_order), "exchange_order": list(self.exchange_order), "contract_digest": self.contract_digest, "federation_digest": self.federation_digest, "migration_digest": self.migration_digest, "replay_identity": self.replay_identity, "omissions": list(self.omissions), "uncertainty": list(self.uncertainty), "negative_evidence": list(self.negative_evidence), "effect_receipts": list(self.effect_receipts), "artifact": dict(self.artifact), "raw_data_local": self.raw_data_local, "boundary": self.boundary})

def model_federated_continual_knowledge_representation_contract(*, request_id: str, federation_id: str, purpose: str, semantic_profile: str, min_freshness_epoch: int, quorum_required: int, peers: Sequence[FederatedKnowledgeContractPeer], input_schema: str = "ScopedResearchClaims1@1", output_schema: str = "TypedKnowledgeWorld1@1", source_revision: int = 1, target_revision: int = 1, migration_requested: bool = False, aggregate_only: bool = True, policy_allow: bool = True, protected_closure: bool = True, raw_data_local: bool = True, replay_identity: str) -> FederatedKnowledgeContractReceipt:
    if not request_id.strip() or not federation_id.strip() or not purpose.strip() or not semantic_profile.strip() or min_freshness_epoch < 0 or quorum_required <= 0 or not peers or input_schema != "ScopedResearchClaims1@1" or output_schema != "TypedKnowledgeWorld1@1" or source_revision <= 0 or target_revision < source_revision or (target_revision > source_revision and not migration_requested) or not aggregate_only or not re.fullmatch(r"[0-9a-f]{64}", replay_identity) or not raw_data_local:
        raise ResearchContractError("federated contract identity, schemas, purpose, quorum, migration, aggregate-only, replay, locality, or boundary is invalid")
    ordered = tuple(sorted(peer.peer_id for peer in peers))
    if len(set(ordered)) != len(peers) or any(not value.strip() for value in ordered):
        raise ResearchContractError("peer identifiers must be unique and non-empty")
    peer_map = {peer.peer_id: peer for peer in peers}; qualified: set[str] = set(); unresolved: set[str] = set(); denied: set[str] = set(); stale: set[str] = set(); mismatch: set[str] = set(); signers: set[str] = set(); omissions: set[str] = set(); uncertainty = {"gate:purpose-bound-exchange", "gate:aggregate-only", "gate:raw-data-locality", "gate:unknown-is-not-asserted"}; negative: set[str] = set()
    for peer_id in ordered:
        peer = peer_map[peer_id]
        if not policy_allow or not protected_closure or peer.boundary != PRECLINICAL_BOUNDARY or peer.purpose != purpose or not peer.institution_id.strip() or not peer.endpoint.strip():
            denied.add(peer_id); negative.add(f"peer:{peer_id}:scope-policy-purpose")
        elif peer.freshness_epoch < min_freshness_epoch:
            unresolved.add(peer_id); stale.add(peer_id); omissions.add(f"peer:{peer_id}:stale")
        elif peer.semantic_profile != semantic_profile:
            unresolved.add(peer_id); mismatch.add(peer_id); uncertainty.add(f"peer:{peer_id}:semantic-profile-mismatch")
        elif not peer.signed_approval or peer.permitted_artifact_digest is None:
            unresolved.add(peer_id); omissions.add(f"peer:{peer_id}:signed-permitted-artifact-missing")
            if not peer.signed_approval: signers.add(peer_id)
        elif peer.evidence_digest is None or peer.provenance_digest is None:
            unresolved.add(peer_id); omissions.add(f"peer:{peer_id}:evidence-or-provenance-missing")
        elif peer.state in {"unknown", "speculative"}:
            unresolved.add(peer_id); uncertainty.add(f"peer:{peer_id}:unknown-not-asserted")
        elif peer.state == "contradicted":
            denied.add(peer_id); negative.add(f"peer:{peer_id}:contradicted")
        else:
            qualified.add(peer_id)
    if not policy_allow: omissions.add("control:policy-denied")
    if not protected_closure: omissions.add("control:protected-closure-incomplete")
    quorum_met = len(qualified) >= quorum_required
    if not quorum_met: omissions.add(f"control:quorum-not-met:{len(qualified)}-of-{quorum_required}")
    if target_revision > source_revision: uncertainty.add(f"migration:{source_revision}-to-{target_revision}")
    exchange = sorted(qualified) if quorum_met else []
    federation_digest = research_artifact_digest({"federation_id": federation_id, "purpose": purpose, "semantic_profile": semantic_profile, "qualified_order": sorted(qualified), "quorum_required": quorum_required, "min_freshness_epoch": min_freshness_epoch})
    contract_digest = research_artifact_digest({"candidate_order": list(ordered), "qualified_order": sorted(qualified), "unresolved_order": sorted(unresolved), "denied_order": sorted(denied), "stale_order": sorted(stale), "semantic_mismatch_order": sorted(mismatch), "signer_order": sorted(signers), "federation_digest": federation_digest})
    migration_digest = research_artifact_digest({"source_revision": source_revision, "target_revision": target_revision, "migration_requested": migration_requested})
    disposition = "blocked" if not policy_allow or not protected_closure or not aggregate_only or not raw_data_local else "partial" if not quorum_met or unresolved or denied else "migrated" if target_revision > source_revision else "completed"
    artifact = {"content_hash": research_artifact_digest({"request_id": request_id, "contract_digest": contract_digest}), "media_type": "application/vnd.aurora.typed-knowledge-world+json"}
    receipt = FederatedKnowledgeContractReceipt(request_id=request_id, federation_id=federation_id, purpose=purpose, semantic_profile=semantic_profile, disposition=disposition, input_schema=input_schema, output_schema=output_schema, source_revision=source_revision, target_revision=target_revision, min_freshness_epoch=min_freshness_epoch, quorum_required=quorum_required, quorum_met=quorum_met, candidate_order=ordered, qualified_order=tuple(sorted(qualified)), unresolved_order=tuple(sorted(unresolved)), denied_order=tuple(sorted(denied)), stale_order=tuple(sorted(stale)), semantic_mismatch_order=tuple(sorted(mismatch)), signer_order=tuple(sorted(signers)), exchange_order=tuple(exchange), contract_digest=contract_digest, federation_digest=federation_digest, migration_digest=migration_digest, replay_identity=replay_identity, omissions=tuple(sorted(omissions)), uncertainty=tuple(sorted(uncertainty)), negative_evidence=tuple(sorted(negative)), effect_receipts=(f"exchange:permitted-federated-knowledge:{request_id}",) if quorum_met and disposition != "blocked" else ("block:unsafe-release",), artifact=artifact, raw_data_local=raw_data_local)
    receipt.validate(); return receipt
