"""Prospective high-throughput evidence contract model parity surface."""
from __future__ import annotations

from dataclasses import dataclass
import re
from typing import Any, Mapping, Sequence

from .research_contracts import (
    ADAPTER_THROUGHPUT_EVIDENCE_SURVEILLANCE_CONTRACT_MODEL_CONTRACT_VERSION,
    ADAPTER_THROUGHPUT_EVIDENCE_SURVEILLANCE_CONTRACT_MODEL_FEATURE_ID,
    PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
    ResearchContractError,
    research_artifact_digest,
)


@dataclass(frozen=True)
class ThroughputContractClaim:
    claim_id: str
    sequence: int
    semantic_type: str
    value_digest: str
    evidence_state: str = "supported"
    omitted: bool = False
    negative_result: bool = False


@dataclass(frozen=True)
class ThroughputEvidenceSurveillanceContractReceipt:
    request_id: str
    input_schema: str
    output_schema: str
    batch_id: str
    checkpoint_seq: int
    compatibility: str
    disposition: str
    candidate_order: tuple[str, ...]
    retained_order: tuple[str, ...]
    unknown_order: tuple[str, ...]
    denied_order: tuple[str, ...]
    overflow_order: tuple[str, ...]
    migration_order: tuple[str, ...]
    semantic_loss: tuple[str, ...]
    queue_digest: str
    checkpoint_digest: str
    contract_digest: str
    canonical_digest: str
    provenance_digest: str
    replay_identity: str
    effect_receipts: tuple[str, ...]
    artifact: Mapping[str, Any]
    feature_id: str = ADAPTER_THROUGHPUT_EVIDENCE_SURVEILLANCE_CONTRACT_MODEL_FEATURE_ID
    contract_version: str = ADAPTER_THROUGHPUT_EVIDENCE_SURVEILLANCE_CONTRACT_MODEL_CONTRACT_VERSION
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    raw_data_local: bool = True
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION or self.contract_version != ADAPTER_THROUGHPUT_EVIDENCE_SURVEILLANCE_CONTRACT_MODEL_CONTRACT_VERSION or self.feature_id != ADAPTER_THROUGHPUT_EVIDENCE_SURVEILLANCE_CONTRACT_MODEL_FEATURE_ID:
            raise ResearchContractError("throughput contract schema, feature, or version mismatch")
        if self.boundary != PRECLINICAL_BOUNDARY or not self.raw_data_local or not self.request_id.strip() or self.input_schema != "EvidenceFeed3@1" or self.output_schema != "QualifiedEvidenceSet2@1" or not self.batch_id.strip() or self.checkpoint_seq <= 0 or not self.candidate_order or not self.effect_receipts:
            raise ResearchContractError("throughput contract identity, schemas, checkpoint, locality, candidates, or effects are incomplete")
        for values in (self.candidate_order, self.retained_order, self.unknown_order, self.denied_order, self.overflow_order, self.migration_order, self.semantic_loss, self.effect_receipts):
            if tuple(sorted(set(values))) != values:
                raise ResearchContractError("throughput contract ordering is not canonical")
        if set(self.retained_order) | set(self.unknown_order) | set(self.denied_order) | set(self.overflow_order) != set(self.candidate_order):
            raise ResearchContractError("throughput contract states do not partition candidates")
        for value in (self.queue_digest, self.checkpoint_digest, self.contract_digest, self.canonical_digest, self.provenance_digest, self.replay_identity, self.artifact.get("content_hash")):
            if not isinstance(value, str) or not re.fullmatch(r"[0-9a-f]{64}", value):
                raise ResearchContractError("throughput contract digest is invalid")
        if any(not effect.startswith("read:local-throughput-contract:") and effect != "block:unsafe-release" for effect in self.effect_receipts):
            raise ResearchContractError("throughput contract effect is outside local-read gate")
        if self.disposition == "blocked" and self.effect_receipts != ("block:unsafe-release",):
            raise ResearchContractError("blocked throughput contract must be explicitly blocked")

    def digest(self) -> str:
        self.validate()
        return research_artifact_digest({"schema_version": self.schema_version, "contract_version": self.contract_version, "feature_id": self.feature_id, "request_id": self.request_id, "input_schema": self.input_schema, "output_schema": self.output_schema, "batch_id": self.batch_id, "checkpoint_seq": self.checkpoint_seq, "compatibility": self.compatibility, "disposition": self.disposition, "candidate_order": list(self.candidate_order), "retained_order": list(self.retained_order), "unknown_order": list(self.unknown_order), "denied_order": list(self.denied_order), "overflow_order": list(self.overflow_order), "migration_order": list(self.migration_order), "semantic_loss": list(self.semantic_loss), "queue_digest": self.queue_digest, "checkpoint_digest": self.checkpoint_digest, "contract_digest": self.contract_digest, "canonical_digest": self.canonical_digest, "provenance_digest": self.provenance_digest, "replay_identity": self.replay_identity, "effect_receipts": list(self.effect_receipts), "artifact": dict(self.artifact), "raw_data_local": self.raw_data_local, "boundary": self.boundary})


def model_throughput_evidence_surveillance_contract(*, request_id: str, input_schema: str, output_schema: str, batch_id: str, checkpoint_seq: int, previous_checkpoint: str | None, max_claims: int, budget_units: int, claims: Sequence[ThroughputContractClaim], policy_allow: bool = True, protected_closure: bool = True, raw_data_local: bool = True, replay_identity: str) -> ThroughputEvidenceSurveillanceContractReceipt:
    if not request_id.strip() or not input_schema.strip() or not output_schema.strip() or not batch_id.strip() or checkpoint_seq <= 0 or max_claims <= 0 or budget_units <= 0 or not claims or not re.fullmatch(r"[0-9a-f]{64}", replay_identity) or not raw_data_local:
        raise ResearchContractError("throughput contract identity, schemas, checkpoint, capacity, budget, claims, replay, or locality is invalid")
    ordered = tuple(sorted(claims, key=lambda claim: (claim.sequence, claim.claim_id))); claim_ids = tuple(claim.claim_id for claim in ordered)
    if len(set(claim_ids)) != len(claim_ids) or any(not value.strip() for value in claim_ids):
        raise ResearchContractError("throughput claim identities must be unique and non-empty")
    compatibility = "additive_migration" if input_schema == "EvidenceFeed3@1" and output_schema == "QualifiedEvidenceSet2@1" else "compatible" if input_schema == output_schema else "breaking"
    admission = min(max_claims, budget_units, len(ordered)); admitted, overflow = ordered[:admission], ordered[admission:]; candidate = tuple(sorted(claim_ids)); overflow_order = tuple(sorted(claim.claim_id for claim in overflow)); retained: set[str] = set(); unknown: set[str] = set(); denied: set[str] = set(); migration: set[str] = set(); loss: set[str] = set()
    if len(ordered) > max_claims: loss.add(f"queue:capacity-overflow:{len(ordered)-max_claims}")
    if budget_units < max_claims: loss.add(f"queue:budget-bounded:{max_claims-budget_units}")
    for claim in admitted:
        if compatibility == "breaking":
            denied.add(claim.claim_id); loss.add(f"claim:{claim.claim_id}:breaking-schema")
        elif claim.omitted or claim.evidence_state in {"unknown", "speculative"}:
            unknown.add(claim.claim_id); loss.add(f"claim:{claim.claim_id}:unknown-not-asserted")
        elif claim.evidence_state == "contradicted":
            denied.add(claim.claim_id); loss.add(f"claim:{claim.claim_id}:contradicted-retained")
        else:
            retained.add(claim.claim_id)
            if compatibility == "additive_migration": migration.add(f"claim:{claim.claim_id}:sequence-preserved")
            if claim.negative_result: loss.add(f"claim:{claim.claim_id}:negative-result-retained")
    if not policy_allow: loss.add("control:policy-denied")
    if not protected_closure: loss.add("control:protected-closure-incomplete")
    disposition = "blocked" if not policy_allow or not protected_closure or not raw_data_local else "unknown" if not retained else "partial" if unknown or denied or overflow_order else "compatible"
    retained_order = tuple(sorted(retained)); unknown_order = tuple(sorted(unknown)); denied_order = tuple(sorted(denied)); migration_order = tuple(sorted(migration)); loss_order = tuple(sorted(loss))
    queue_digest = research_artifact_digest({"batch_id": batch_id, "candidate_order": list(candidate), "overflow_order": list(overflow_order)})
    checkpoint_digest = research_artifact_digest({"batch_id": batch_id, "checkpoint_seq": checkpoint_seq, "previous_checkpoint": previous_checkpoint, "queue_digest": queue_digest})
    contract_digest = research_artifact_digest({"input_schema": input_schema, "output_schema": output_schema, "compatibility": compatibility, "candidate_order": list(candidate)})
    canonical_digest = research_artifact_digest({"retained_order": list(retained_order), "unknown_order": list(unknown_order), "denied_order": list(denied_order), "overflow_order": list(overflow_order), "migration_order": list(migration_order), "semantic_loss": list(loss_order)})
    provenance_digest = research_artifact_digest({"request_id": request_id, "replay_identity": replay_identity, "checkpoint_digest": checkpoint_digest, "contract_digest": contract_digest})
    payload = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": ADAPTER_THROUGHPUT_EVIDENCE_SURVEILLANCE_CONTRACT_MODEL_CONTRACT_VERSION, "feature_id": ADAPTER_THROUGHPUT_EVIDENCE_SURVEILLANCE_CONTRACT_MODEL_FEATURE_ID, "request_id": request_id, "input_schema": input_schema, "output_schema": output_schema, "batch_id": batch_id, "checkpoint_seq": checkpoint_seq, "compatibility": compatibility, "disposition": disposition, "candidate_order": list(candidate), "retained_order": list(retained_order), "unknown_order": list(unknown_order), "denied_order": list(denied_order), "overflow_order": list(overflow_order), "migration_order": list(migration_order), "semantic_loss": list(loss_order), "queue_digest": queue_digest, "checkpoint_digest": checkpoint_digest, "contract_digest": contract_digest, "canonical_digest": canonical_digest, "provenance_digest": provenance_digest, "replay_identity": replay_identity, "raw_data_local": raw_data_local, "boundary": PRECLINICAL_BOUNDARY}
    artifact = {"content_hash": research_artifact_digest(payload), "media_type": "application/vnd.aurora.qualified-throughput-evidence-set+json"}
    receipt = ThroughputEvidenceSurveillanceContractReceipt(request_id=request_id, input_schema="EvidenceFeed3@1", output_schema="QualifiedEvidenceSet2@1", batch_id=batch_id, checkpoint_seq=checkpoint_seq, compatibility=compatibility, disposition=disposition, candidate_order=candidate, retained_order=retained_order, unknown_order=unknown_order, denied_order=denied_order, overflow_order=overflow_order, migration_order=migration_order, semantic_loss=loss_order, queue_digest=queue_digest, checkpoint_digest=checkpoint_digest, contract_digest=contract_digest, canonical_digest=canonical_digest, provenance_digest=provenance_digest, replay_identity=replay_identity, effect_receipts=(f"read:local-throughput-contract:{request_id}",) if disposition != "blocked" else ("block:unsafe-release",), artifact=artifact, raw_data_local=raw_data_local)
    receipt.validate(); return receipt
