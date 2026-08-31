"""Federated continual evidence-surveillance contract parity surface."""
from __future__ import annotations

from dataclasses import dataclass
import re
from typing import Any, Mapping, Sequence

from .research_contracts import (
    WORLDGEN_FEDERATED_CONTINUAL_EVIDENCE_SURVEILLANCE_CONTRACT_MODEL_CONTRACT_VERSION,
    WORLDGEN_FEDERATED_CONTINUAL_EVIDENCE_SURVEILLANCE_CONTRACT_MODEL_FEATURE_ID,
    PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
    ResearchContractError,
    research_artifact_digest,
)


@dataclass(frozen=True)
class FederatedContinualContractClaim:
    claim_id: str
    peer_id: str
    institution_id: str
    artifact_kind: str
    semantic_profile: str
    value_digest: str
    evidence_state: str = "supported"
    signed: bool = True
    permitted_artifact: bool = True
    aggregate_only: bool = True
    omitted: bool = False
    negative_result: bool = False


@dataclass(frozen=True)
class FederatedContinualEvidenceSurveillanceContractReceipt:
    request_id: str
    input_schema: str
    output_schema: str
    federation_id: str
    purpose: str
    endpoint: str
    semantic_profile: str
    compatibility: str
    disposition: str
    peer_order: tuple[str, ...]
    candidate_order: tuple[str, ...]
    retained_order: tuple[str, ...]
    unknown_order: tuple[str, ...]
    denied_order: tuple[str, ...]
    aggregate_order: tuple[str, ...]
    migration_order: tuple[str, ...]
    semantic_loss: tuple[str, ...]
    federation_digest: str
    envelope_digest: str
    contract_digest: str
    canonical_digest: str
    provenance_digest: str
    replay_identity: str
    effect_receipts: tuple[str, ...]
    artifact: Mapping[str, Any]
    feature_id: str = WORLDGEN_FEDERATED_CONTINUAL_EVIDENCE_SURVEILLANCE_CONTRACT_MODEL_FEATURE_ID
    contract_version: str = WORLDGEN_FEDERATED_CONTINUAL_EVIDENCE_SURVEILLANCE_CONTRACT_MODEL_CONTRACT_VERSION
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    raw_data_local: bool = True
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION or self.contract_version != WORLDGEN_FEDERATED_CONTINUAL_EVIDENCE_SURVEILLANCE_CONTRACT_MODEL_CONTRACT_VERSION or self.feature_id != WORLDGEN_FEDERATED_CONTINUAL_EVIDENCE_SURVEILLANCE_CONTRACT_MODEL_FEATURE_ID:
            raise ResearchContractError("federated continual contract schema, feature, or version mismatch")
        if self.boundary != PRECLINICAL_BOUNDARY or not self.raw_data_local or not self.request_id.strip() or self.input_schema != "EvidenceFeed4@1" or self.output_schema != "QualifiedEvidenceSet2@1" or not self.federation_id.strip() or not self.purpose.strip() or not self.endpoint.strip() or not self.semantic_profile.strip() or not self.candidate_order or not self.effect_receipts:
            raise ResearchContractError("federated contract identity, schema, locality, candidates, or effects are incomplete")
        for values in (self.peer_order, self.candidate_order, self.retained_order, self.unknown_order, self.denied_order, self.aggregate_order, self.migration_order, self.semantic_loss, self.effect_receipts):
            if tuple(sorted(set(values))) != values:
                raise ResearchContractError("federated contract ordering is not canonical")
        if set(self.retained_order) | set(self.unknown_order) | set(self.denied_order) != set(self.candidate_order) or any(value not in self.retained_order for value in self.aggregate_order):
            raise ResearchContractError("federated contract states do not partition candidates")
        for value in (self.federation_digest, self.envelope_digest, self.contract_digest, self.canonical_digest, self.provenance_digest, self.replay_identity, self.artifact.get("content_hash")):
            if not isinstance(value, str) or not re.fullmatch(r"[0-9a-f]{64}", value):
                raise ResearchContractError("federated contract digest is invalid")
        if any(not effect.startswith("exchange:aggregate-evidence-contract:") and effect != "block:unsafe-release" for effect in self.effect_receipts):
            raise ResearchContractError("federated contract effect is outside aggregate exchange gate")
        if self.disposition == "blocked" and self.effect_receipts != ("block:unsafe-release",):
            raise ResearchContractError("blocked federated contract must be explicitly blocked")

    def digest(self) -> str:
        self.validate()
        return research_artifact_digest({"schema_version": self.schema_version, "contract_version": self.contract_version, "feature_id": self.feature_id, "request_id": self.request_id, "input_schema": self.input_schema, "output_schema": self.output_schema, "federation_id": self.federation_id, "purpose": self.purpose, "endpoint": self.endpoint, "semantic_profile": self.semantic_profile, "compatibility": self.compatibility, "disposition": self.disposition, "peer_order": list(self.peer_order), "candidate_order": list(self.candidate_order), "retained_order": list(self.retained_order), "unknown_order": list(self.unknown_order), "denied_order": list(self.denied_order), "aggregate_order": list(self.aggregate_order), "migration_order": list(self.migration_order), "semantic_loss": list(self.semantic_loss), "federation_digest": self.federation_digest, "envelope_digest": self.envelope_digest, "contract_digest": self.contract_digest, "canonical_digest": self.canonical_digest, "provenance_digest": self.provenance_digest, "replay_identity": self.replay_identity, "effect_receipts": list(self.effect_receipts), "artifact": dict(self.artifact), "raw_data_local": self.raw_data_local, "boundary": self.boundary})


def model_federated_continual_evidence_surveillance_contract(*, request_id: str, input_schema: str, output_schema: str, federation_id: str, purpose: str, endpoint: str, semantic_profile: str, allowed_artifacts: Sequence[str], min_peer_quorum: int, claims: Sequence[FederatedContinualContractClaim], policy_allow: bool = True, protected_closure: bool = True, raw_data_local: bool = True, replay_identity: str) -> FederatedContinualEvidenceSurveillanceContractReceipt:
    if not request_id.strip() or not input_schema.strip() or not output_schema.strip() or not federation_id.strip() or not purpose.strip() or not endpoint.strip() or not semantic_profile.strip() or not allowed_artifacts or min_peer_quorum <= 0 or not claims or not re.fullmatch(r"[0-9a-f]{64}", replay_identity) or not raw_data_local:
        raise ResearchContractError("federated contract identity, schema, purpose, allow-list, quorum, claims, replay, or locality is invalid")
    ordered = tuple(sorted(claims, key=lambda claim: claim.claim_id)); candidate = tuple(claim.claim_id for claim in ordered)
    if len(set(candidate)) != len(candidate) or any(not claim.claim_id.strip() or not claim.peer_id.strip() or not claim.institution_id.strip() for claim in ordered):
        raise ResearchContractError("federated claim identities must be unique and non-empty")
    compatibility = "additive_migration" if input_schema == "EvidenceFeed4@1" and output_schema == "QualifiedEvidenceSet2@1" else "compatible" if input_schema == output_schema else "breaking"
    retained: set[str] = set(); unknown: set[str] = set(); denied: set[str] = set(); peers: set[str] = set(); aggregate: set[str] = set(); migration: set[str] = set(); loss: set[str] = set()
    for claim in ordered:
        if not policy_allow or not protected_closure or not raw_data_local:
            denied.add(claim.claim_id); loss.add(f"claim:{claim.claim_id}:policy-closure-locality")
        elif not claim.signed:
            denied.add(claim.claim_id); loss.add(f"claim:{claim.claim_id}:signature-missing")
        elif not claim.permitted_artifact or claim.artifact_kind not in allowed_artifacts:
            denied.add(claim.claim_id); loss.add(f"claim:{claim.claim_id}:artifact-not-permitted")
        elif not claim.aggregate_only:
            denied.add(claim.claim_id); loss.add(f"claim:{claim.claim_id}:raw-observation-export-denied")
        elif claim.semantic_profile != semantic_profile:
            denied.add(claim.claim_id); loss.add(f"claim:{claim.claim_id}:semantic-profile-mismatch")
        elif compatibility == "breaking":
            denied.add(claim.claim_id); loss.add(f"claim:{claim.claim_id}:breaking-schema")
        elif claim.omitted or claim.evidence_state in {"unknown", "speculative"}:
            unknown.add(claim.claim_id); loss.add(f"claim:{claim.claim_id}:unknown-not-asserted")
        elif claim.evidence_state == "contradicted":
            denied.add(claim.claim_id); loss.add(f"claim:{claim.claim_id}:contradicted")
        else:
            retained.add(claim.claim_id); peers.add(claim.peer_id); aggregate.add(claim.claim_id)
            if compatibility == "additive_migration": migration.add(f"claim:{claim.claim_id}:aggregate-only-migration")
            if claim.negative_result: loss.add(f"claim:{claim.claim_id}:negative-result-retained")
    if len(peers) < min_peer_quorum: loss.add(f"federation:quorum-incomplete:{len(peers)}<{min_peer_quorum}")
    if not policy_allow: loss.add("control:policy-denied")
    if not protected_closure: loss.add("control:protected-closure-incomplete")
    disposition = "blocked" if not policy_allow or not protected_closure or not raw_data_local else "unknown" if not retained else "partial" if unknown or denied or len(peers) < min_peer_quorum else "compatible"
    peer_order = tuple(sorted(peers)); retained_order = tuple(sorted(retained)); unknown_order = tuple(sorted(unknown)); denied_order = tuple(sorted(denied)); aggregate_order = tuple(sorted(aggregate)); migration_order = tuple(sorted(migration)); loss_order = tuple(sorted(loss))
    federation_digest = research_artifact_digest({"federation_id": federation_id, "purpose": purpose, "endpoint": endpoint, "peer_order": list(peer_order), "semantic_profile": semantic_profile})
    envelope_digest = research_artifact_digest({"aggregate_order": list(aggregate_order), "allowed_artifacts": sorted(allowed_artifacts), "aggregate_only": True, "federation_digest": federation_digest})
    contract_digest = research_artifact_digest({"input_schema": input_schema, "output_schema": output_schema, "compatibility": compatibility, "candidate_order": list(candidate)})
    canonical_digest = research_artifact_digest({"retained_order": list(retained_order), "unknown_order": list(unknown_order), "denied_order": list(denied_order), "aggregate_order": list(aggregate_order), "migration_order": list(migration_order), "semantic_loss": list(loss_order)})
    provenance_digest = research_artifact_digest({"request_id": request_id, "replay_identity": replay_identity, "envelope_digest": envelope_digest, "contract_digest": contract_digest})
    payload = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": WORLDGEN_FEDERATED_CONTINUAL_EVIDENCE_SURVEILLANCE_CONTRACT_MODEL_CONTRACT_VERSION, "feature_id": WORLDGEN_FEDERATED_CONTINUAL_EVIDENCE_SURVEILLANCE_CONTRACT_MODEL_FEATURE_ID, "request_id": request_id, "input_schema": input_schema, "output_schema": output_schema, "federation_id": federation_id, "purpose": purpose, "endpoint": endpoint, "semantic_profile": semantic_profile, "compatibility": compatibility, "disposition": disposition, "peer_order": list(peer_order), "candidate_order": list(candidate), "retained_order": list(retained_order), "unknown_order": list(unknown_order), "denied_order": list(denied_order), "aggregate_order": list(aggregate_order), "migration_order": list(migration_order), "semantic_loss": list(loss_order), "federation_digest": federation_digest, "envelope_digest": envelope_digest, "contract_digest": contract_digest, "canonical_digest": canonical_digest, "provenance_digest": provenance_digest, "replay_identity": replay_identity, "raw_data_local": raw_data_local, "boundary": PRECLINICAL_BOUNDARY}
    artifact = {"content_hash": research_artifact_digest(payload), "content_type": "application/vnd.aurora.worldgen.qualified-federated-evidence-set+json", "artifact_id": f"worldgen-federated-continual-contract:{request_id}", "semantic_loss": list(loss_order), "boundary": PRECLINICAL_BOUNDARY}
    receipt = FederatedContinualEvidenceSurveillanceContractReceipt(request_id=request_id, input_schema="EvidenceFeed4@1", output_schema="QualifiedEvidenceSet2@1", federation_id=federation_id, purpose=purpose, endpoint=endpoint, semantic_profile=semantic_profile, compatibility=compatibility, disposition=disposition, peer_order=peer_order, candidate_order=candidate, retained_order=retained_order, unknown_order=unknown_order, denied_order=denied_order, aggregate_order=aggregate_order, migration_order=migration_order, semantic_loss=loss_order, federation_digest=federation_digest, envelope_digest=envelope_digest, contract_digest=contract_digest, canonical_digest=canonical_digest, provenance_digest=provenance_digest, replay_identity=replay_identity, effect_receipts=(f"exchange:aggregate-evidence-contract:{federation_id}",) if disposition != "blocked" else ("block:unsafe-release",), artifact=artifact, raw_data_local=raw_data_local)
    receipt.validate(); return receipt

__all__ = ["FederatedContinualContractClaim", "FederatedContinualEvidenceSurveillanceContractReceipt", "model_federated_continual_evidence_surveillance_contract"]
