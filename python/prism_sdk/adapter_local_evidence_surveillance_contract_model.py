"""Typed local evidence-surveillance contract model parity surface."""
from __future__ import annotations

from dataclasses import dataclass
import re
from typing import Any, Mapping, Sequence

from .research_contracts import (
    ADAPTER_LOCAL_EVIDENCE_SURVEILLANCE_CONTRACT_MODEL_CONTRACT_VERSION,
    ADAPTER_LOCAL_EVIDENCE_SURVEILLANCE_CONTRACT_MODEL_FEATURE_ID,
    PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
    ResearchContractError,
    research_artifact_digest,
)


@dataclass(frozen=True)
class ContractModelClaim:
    claim_id: str
    semantic_type: str
    value_digest: str
    evidence_state: str = "supported"
    omitted: bool = False
    negative_result: bool = False


@dataclass(frozen=True)
class LocalEvidenceSurveillanceContractReceipt:
    request_id: str
    input_schema: str
    output_schema: str
    compatibility: str
    disposition: str
    candidate_order: tuple[str, ...]
    retained_order: tuple[str, ...]
    unknown_order: tuple[str, ...]
    denied_order: tuple[str, ...]
    migration_order: tuple[str, ...]
    semantic_loss: tuple[str, ...]
    required_order: tuple[str, ...]
    contract_digest: str
    canonical_digest: str
    provenance_digest: str
    replay_identity: str
    effect_receipts: tuple[str, ...]
    artifact: Mapping[str, Any]
    feature_id: str = ADAPTER_LOCAL_EVIDENCE_SURVEILLANCE_CONTRACT_MODEL_FEATURE_ID
    contract_version: str = ADAPTER_LOCAL_EVIDENCE_SURVEILLANCE_CONTRACT_MODEL_CONTRACT_VERSION
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    raw_data_local: bool = True
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION or self.contract_version != ADAPTER_LOCAL_EVIDENCE_SURVEILLANCE_CONTRACT_MODEL_CONTRACT_VERSION or self.feature_id != ADAPTER_LOCAL_EVIDENCE_SURVEILLANCE_CONTRACT_MODEL_FEATURE_ID:
            raise ResearchContractError("contract model schema, feature, or version mismatch")
        if self.boundary != PRECLINICAL_BOUNDARY or not self.raw_data_local or not self.request_id.strip() or self.input_schema != "EvidenceFeed1@1" or self.output_schema != "QualifiedEvidenceSet2@1" or not self.candidate_order or not self.effect_receipts:
            raise ResearchContractError("contract identity, schemas, locality, candidates, or effects are incomplete")
        for values in (self.candidate_order, self.retained_order, self.unknown_order, self.denied_order, self.migration_order, self.semantic_loss, self.required_order, self.effect_receipts):
            if tuple(sorted(set(values))) != values:
                raise ResearchContractError("contract ordering is not canonical")
        if set(self.retained_order) | set(self.unknown_order) | set(self.denied_order) != set(self.candidate_order):
            raise ResearchContractError("contract states do not partition candidates")
        for value in (self.contract_digest, self.canonical_digest, self.provenance_digest, self.replay_identity, self.artifact.get("content_hash")):
            if not isinstance(value, str) or not re.fullmatch(r"[0-9a-f]{64}", value):
                raise ResearchContractError("contract digest is invalid")
        if any(not effect.startswith("read:local-contract:") and effect != "block:unsafe-release" for effect in self.effect_receipts):
            raise ResearchContractError("contract effect is outside local-read gate")
        if self.disposition == "blocked" and self.effect_receipts != ("block:unsafe-release",):
            raise ResearchContractError("blocked contract must be explicitly blocked")

    def digest(self) -> str:
        self.validate()
        return research_artifact_digest({"schema_version": self.schema_version, "contract_version": self.contract_version, "feature_id": self.feature_id, "request_id": self.request_id, "input_schema": self.input_schema, "output_schema": self.output_schema, "compatibility": self.compatibility, "disposition": self.disposition, "candidate_order": list(self.candidate_order), "retained_order": list(self.retained_order), "unknown_order": list(self.unknown_order), "denied_order": list(self.denied_order), "migration_order": list(self.migration_order), "semantic_loss": list(self.semantic_loss), "required_order": list(self.required_order), "contract_digest": self.contract_digest, "canonical_digest": self.canonical_digest, "provenance_digest": self.provenance_digest, "replay_identity": self.replay_identity, "effect_receipts": list(self.effect_receipts), "artifact": dict(self.artifact), "raw_data_local": self.raw_data_local, "boundary": self.boundary})


def model_local_evidence_surveillance_contract(*, request_id: str, input_schema: str, output_schema: str, claims: Sequence[ContractModelClaim], required_claim_ids: Sequence[str], policy_allow: bool = True, protected_closure: bool = True, raw_data_local: bool = True, replay_identity: str) -> LocalEvidenceSurveillanceContractReceipt:
    if not request_id.strip() or not input_schema.strip() or not output_schema.strip() or not claims or not re.fullmatch(r"[0-9a-f]{64}", replay_identity) or not raw_data_local:
        raise ResearchContractError("contract identity, schemas, claims, replay, or locality is invalid")
    ordered = tuple(sorted(claims, key=lambda claim: claim.claim_id)); claim_ids = tuple(claim.claim_id for claim in ordered)
    if len(set(claim_ids)) != len(claim_ids) or any(not value.strip() for value in claim_ids):
        raise ResearchContractError("claim identities must be unique and non-empty")
    candidate = tuple(sorted(set(claim_ids) | set(required_claim_ids)))
    compatibility = "additive_migration" if input_schema == "EvidenceFeed1@1" and output_schema == "QualifiedEvidenceSet2@1" else "compatible" if input_schema == output_schema else "breaking"
    retained: set[str] = set(); unknown: set[str] = set(); denied: set[str] = set(); migration: set[str] = set(); semantic_loss: set[str] = set(); required = set(required_claim_ids)
    for claim in ordered:
        if compatibility == "breaking":
            denied.add(claim.claim_id); semantic_loss.add(f"claim:{claim.claim_id}:breaking-schema")
        elif claim.omitted:
            unknown.add(claim.claim_id); semantic_loss.add(f"claim:{claim.claim_id}:omitted-preserved")
        elif claim.evidence_state in {"unknown", "speculative"}:
            unknown.add(claim.claim_id); semantic_loss.add(f"claim:{claim.claim_id}:unknown-not-asserted")
        elif claim.evidence_state == "contradicted":
            denied.add(claim.claim_id); semantic_loss.add(f"claim:{claim.claim_id}:contradicted-retained")
        else:
            retained.add(claim.claim_id)
            if compatibility == "additive_migration": migration.add(f"claim:{claim.claim_id}:evidence-state-preserved")
            if claim.negative_result: semantic_loss.add(f"claim:{claim.claim_id}:negative-result-retained")
    for claim_id in required:
        if claim_id not in candidate or claim_id not in retained:
            unknown.add(claim_id); semantic_loss.add(f"claim:{claim_id}:required-unresolved")
    if not policy_allow: semantic_loss.add("control:policy-denied")
    if not protected_closure: semantic_loss.add("control:protected-closure-incomplete")
    disposition = "blocked" if not policy_allow or not protected_closure or not raw_data_local else "unknown" if not retained else "partial" if compatibility == "breaking" or unknown or denied else "compatible"
    retained_order = tuple(sorted(retained)); unknown_order = tuple(sorted(unknown)); denied_order = tuple(sorted(denied)); migration_order = tuple(sorted(migration)); loss_order = tuple(sorted(semantic_loss)); required_order = tuple(sorted(required))
    contract_digest = research_artifact_digest({"input_schema": input_schema, "output_schema": output_schema, "compatibility": compatibility, "candidate_order": list(candidate), "required_order": list(required_order)})
    canonical_digest = research_artifact_digest({"retained_order": list(retained_order), "unknown_order": list(unknown_order), "denied_order": list(denied_order), "migration_order": list(migration_order), "semantic_loss": list(loss_order)})
    provenance_digest = research_artifact_digest({"request_id": request_id, "replay_identity": replay_identity, "contract_digest": contract_digest, "canonical_digest": canonical_digest})
    payload = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": ADAPTER_LOCAL_EVIDENCE_SURVEILLANCE_CONTRACT_MODEL_CONTRACT_VERSION, "feature_id": ADAPTER_LOCAL_EVIDENCE_SURVEILLANCE_CONTRACT_MODEL_FEATURE_ID, "request_id": request_id, "input_schema": input_schema, "output_schema": output_schema, "compatibility": compatibility, "disposition": disposition, "candidate_order": list(candidate), "retained_order": list(retained_order), "unknown_order": list(unknown_order), "denied_order": list(denied_order), "migration_order": list(migration_order), "semantic_loss": list(loss_order), "required_order": list(required_order), "contract_digest": contract_digest, "canonical_digest": canonical_digest, "provenance_digest": provenance_digest, "replay_identity": replay_identity, "raw_data_local": raw_data_local, "boundary": PRECLINICAL_BOUNDARY}
    artifact = {"content_hash": research_artifact_digest(payload), "media_type": "application/vnd.aurora.qualified-evidence-set+json"}
    receipt = LocalEvidenceSurveillanceContractReceipt(request_id=request_id, input_schema="EvidenceFeed1@1", output_schema="QualifiedEvidenceSet2@1", compatibility=compatibility, disposition=disposition, candidate_order=candidate, retained_order=retained_order, unknown_order=unknown_order, denied_order=denied_order, migration_order=migration_order, semantic_loss=loss_order, required_order=required_order, contract_digest=contract_digest, canonical_digest=canonical_digest, provenance_digest=provenance_digest, replay_identity=replay_identity, effect_receipts=(f"read:local-contract:{request_id}",) if disposition != "blocked" else ("block:unsafe-release",), artifact=artifact, raw_data_local=raw_data_local)
    receipt.validate(); return receipt
