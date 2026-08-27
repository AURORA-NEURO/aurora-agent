"""Python parity surface for ``AFA-ids-P01-F07``.

This ids-owned contract model is deliberately a typed admission primitive.  It
accounts for queue overflow and schema incompatibility while preserving unknown,
contradicted, omitted, and negative evidence; it does not retrieve or infer
evidence and never exports raw observations.
"""
from __future__ import annotations
from dataclasses import dataclass
import hashlib, json, re
from typing import Any, Mapping, Sequence
from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError

FEATURE_ID = "AFA-ids-P01-F07"
CONTRACT_VERSION = "ids-prospective-throughput-evidence-surveillance-contract-model/1.0"
INPUT_SCHEMA = "EvidenceFeed3@1"
OUTPUT_SCHEMA = "QualifiedEvidenceSet2@1"
CONTENT_TYPE = "application/vnd.aurora.qualified-throughput-evidence-set+json"

def _digest(value: Any) -> bool: return isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) is not None
def _canonical(values: Sequence[str]) -> bool: return tuple(values) == tuple(sorted(set(values)))
def _hash(value: Any) -> str: return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()).hexdigest()

@dataclass(frozen=True)
class IdsEvidenceSurveillanceContractReceipt:
    request_id: str; input_schema: str; output_schema: str; batch_id: str; checkpoint_seq: int; disposition: str
    candidate_order: tuple[str, ...]; retained_order: tuple[str, ...]; unknown_order: tuple[str, ...]; denied_order: tuple[str, ...]; overflow_order: tuple[str, ...]
    omission_order: tuple[str, ...]; semantic_loss: tuple[str, ...]; queue_digest: str; checkpoint_digest: str; contract_digest: str; canonical_digest: str; provenance_digest: str; replay_identity: str
    effect_receipts: tuple[str, ...]; artifact: dict[str, Any]; raw_data_local: bool = True; boundary: str = PRECLINICAL_BOUNDARY
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION; contract_version: str = CONTRACT_VERSION; feature_id: str = FEATURE_ID
    def to_dict(self) -> dict[str, Any]:
        return {"schema_version": self.schema_version, "contract_version": self.contract_version, "feature_id": self.feature_id, "request_id": self.request_id, "input_schema": self.input_schema, "output_schema": self.output_schema, "batch_id": self.batch_id, "checkpoint_seq": self.checkpoint_seq, "disposition": self.disposition, "candidate_order": list(self.candidate_order), "retained_order": list(self.retained_order), "unknown_order": list(self.unknown_order), "denied_order": list(self.denied_order), "overflow_order": list(self.overflow_order), "omission_order": list(self.omission_order), "semantic_loss": list(self.semantic_loss), "queue_digest": self.queue_digest, "checkpoint_digest": self.checkpoint_digest, "contract_digest": self.contract_digest, "canonical_digest": self.canonical_digest, "provenance_digest": self.provenance_digest, "replay_identity": self.replay_identity, "effect_receipts": list(self.effect_receipts), "artifact": self.artifact, "raw_data_local": self.raw_data_local, "boundary": self.boundary}
    def validate(self) -> None:
        if (self.schema_version, self.contract_version, self.feature_id) != (RESEARCH_CONTRACT_SCHEMA_VERSION, CONTRACT_VERSION, FEATURE_ID) or self.boundary != PRECLINICAL_BOUNDARY or not self.raw_data_local or not all(v.strip() for v in (self.request_id, self.input_schema, self.output_schema, self.batch_id)) or self.checkpoint_seq == 0 or not self.candidate_order or not self.effect_receipts or self.disposition not in {"compatible", "partial", "unknown", "blocked"}: raise ResearchContractError("ids evidence contract identity, schemas, checkpoint, locality, candidates, or effects are incomplete")
        for values in (self.candidate_order, self.retained_order, self.unknown_order, self.denied_order, self.overflow_order, self.omission_order, self.semantic_loss, self.effect_receipts):
            if not _canonical(values): raise ResearchContractError("ids evidence contract ordering is not canonical")
        classified = [*self.retained_order, *self.unknown_order, *self.denied_order, *self.overflow_order]
        if set(classified) != set(self.candidate_order) or len(classified) != len(set(classified)) or len(self.candidate_order) != len(set(self.candidate_order)): raise ResearchContractError("ids evidence contract states do not partition candidates")
        if not all(_digest(value) for value in (self.queue_digest, self.checkpoint_digest, self.contract_digest, self.canonical_digest, self.provenance_digest, self.replay_identity, self.artifact.get("content_hash"))): raise ResearchContractError("ids evidence contract digest is invalid")
        if self.artifact.get("content_type") != CONTENT_TYPE or self.artifact.get("boundary") != PRECLINICAL_BOUNDARY: raise ResearchContractError("ids evidence artifact metadata is invalid")
        if any(not effect.startswith("read:local-evidence-contract:") and effect != "block:unsafe-release" for effect in self.effect_receipts): raise ResearchContractError("ids evidence contract effect is outside local-read gate")
        if self.disposition == "blocked" and self.effect_receipts != ("block:unsafe-release",): raise ResearchContractError("blocked ids evidence contract must explicitly block release")

def throughput_evidence_surveillance_contract_model_manifest() -> dict[str, Any]:
    return {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "capability_id": FEATURE_ID, "version": CONTRACT_VERSION, "owner_crate": "ids", "consumers": ["context compiler engineer", "evidence schema steward"], "behavior": "models EvidenceFeed3 into QualifiedEvidenceSet2 with bounded admission, checkpoint, migration, and overflow witnesses", "value": "makes high-throughput evidence capacity loss and replay identity part of the typed scientific data contract", "input_schema": INPUT_SCHEMA, "output_schema": OUTPUT_SCHEMA, "effects": ["read:local-research-artifacts", "write:local-artifact"], "autonomy_tier": "A1", "boundary": PRECLINICAL_BOUNDARY}

def model_throughput_evidence_surveillance_contract(*, request_id: str, input_schema: str, output_schema: str, batch_id: str, checkpoint_seq: int, previous_checkpoint: str | None, max_claims: int, budget_units: int, claims: Sequence[Mapping[str, Any]], policy_allow: bool, protected_closure: bool, raw_data_local: bool, replay_identity: str, boundary: str = PRECLINICAL_BOUNDARY) -> IdsEvidenceSurveillanceContractReceipt:
    if not all(v.strip() for v in (request_id, input_schema, output_schema, batch_id)) or checkpoint_seq == 0 or max_claims <= 0 or budget_units <= 0 or not claims or not _digest(replay_identity) or boundary != PRECLINICAL_BOUNDARY or not raw_data_local: raise ResearchContractError("ids evidence contract identity, schemas, batch/checkpoint, capacity, budget, claims, replay, locality, or boundary is invalid")
    rows = [dict(row) for row in claims]; rows.sort(key=lambda row: (int(row.get("sequence", 0)), str(row.get("claim_id", ""))))
    ids = [str(row.get("claim_id", "")) for row in rows]
    if any(not value.strip() or not str(row.get("semantic_type", "")).strip() or not _digest(row.get("value_digest")) for row, value in zip(rows, ids)) or len(set(ids)) != len(ids): raise ResearchContractError("ids evidence claim identities or value digests are malformed or duplicated")
    compatible = input_schema == INPUT_SCHEMA and output_schema == OUTPUT_SCHEMA; admission = min(max_claims, budget_units, len(rows)); admitted, overflow = rows[:admission], rows[admission:]; candidate_order = tuple(sorted(ids)); overflow_order = tuple(sorted(str(row["claim_id"]) for row in overflow)); retained: set[str] = set(); unknown: set[str] = set(); denied: set[str] = set(); omission: set[str] = set(); loss: set[str] = set()
    if len(rows) > max_claims: loss.add(f"queue:capacity-overflow:{len(rows)-max_claims}")
    if budget_units < max_claims: loss.add(f"queue:budget-bounded:{max_claims-budget_units}")
    for row in admitted:
        cid = str(row["claim_id"]); state = str(row.get("evidence_state", ""))
        if not compatible: denied.add(cid); loss.add(f"claim:{cid}:breaking-schema")
        elif row.get("omitted") or state in {"unknown", "speculative", "unmeasured"}: unknown.add(cid); loss.add(f"claim:{cid}:evidence-unresolved")
        elif state == "contradicted": denied.add(cid); loss.add(f"claim:{cid}:contradicted-retained")
        else: retained.add(cid); loss.update({f"claim:{cid}:negative-result-retained"} if row.get("negative_result") else set())
    if not policy_allow: omission.add("control:policy-denied")
    if not protected_closure: omission.add("control:protected-closure-incomplete")
    disposition = "blocked" if not policy_allow or not protected_closure or not raw_data_local else "unknown" if not retained else "partial" if unknown or denied or overflow_order else "compatible"; retained_order, unknown_order, denied_order = tuple(sorted(retained)), tuple(sorted(unknown)), tuple(sorted(denied)); omission_order, semantic_loss = tuple(sorted(omission)), tuple(sorted(loss)); effects = ("block:unsafe-release",) if disposition == "blocked" else (f"read:local-evidence-contract:{request_id}",)
    queue_digest = _hash({"batch_id": batch_id, "candidate_order": list(candidate_order), "overflow_order": list(overflow_order)}); checkpoint_digest = _hash({"batch_id": batch_id, "checkpoint_seq": checkpoint_seq, "previous_checkpoint": previous_checkpoint, "queue_digest": queue_digest}); contract_digest = _hash({"input_schema": input_schema, "output_schema": output_schema, "compatible": compatible, "candidate_order": list(candidate_order)}); canonical_digest = _hash({"retained_order": list(retained_order), "unknown_order": list(unknown_order), "denied_order": list(denied_order), "overflow_order": list(overflow_order), "omission_order": list(omission_order), "semantic_loss": list(semantic_loss)}); provenance_digest = _hash({"request_id": request_id, "replay_identity": replay_identity, "checkpoint_digest": checkpoint_digest, "contract_digest": contract_digest}); payload = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request_id, "input_schema": input_schema, "output_schema": output_schema, "batch_id": batch_id, "checkpoint_seq": checkpoint_seq, "disposition": disposition, "candidate_order": list(candidate_order), "retained_order": list(retained_order), "unknown_order": list(unknown_order), "denied_order": list(denied_order), "overflow_order": list(overflow_order), "omission_order": list(omission_order), "semantic_loss": list(semantic_loss), "queue_digest": queue_digest, "checkpoint_digest": checkpoint_digest, "contract_digest": contract_digest, "canonical_digest": canonical_digest, "provenance_digest": provenance_digest, "replay_identity": replay_identity, "raw_data_local": raw_data_local, "boundary": PRECLINICAL_BOUNDARY}; artifact = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "artifact_id": f"ids-throughput-evidence-contract:{request_id}", "content_type": CONTENT_TYPE, "content_hash": _hash(payload), "semantic_loss": [], "provenance": [provenance_digest], "boundary": PRECLINICAL_BOUNDARY}; result = IdsEvidenceSurveillanceContractReceipt(request_id, INPUT_SCHEMA, OUTPUT_SCHEMA, batch_id, checkpoint_seq, disposition, candidate_order, retained_order, unknown_order, denied_order, overflow_order, omission_order, semantic_loss, queue_digest, checkpoint_digest, contract_digest, canonical_digest, provenance_digest, replay_identity, effects, artifact, raw_data_local, boundary); result.validate(); return result

def ids_throughput_evidence_surveillance_digest(result: IdsEvidenceSurveillanceContractReceipt) -> str: result.validate(); return _hash(result.to_dict())
__all__ = ["FEATURE_ID", "CONTRACT_VERSION", "INPUT_SCHEMA", "OUTPUT_SCHEMA", "IdsEvidenceSurveillanceContractReceipt", "throughput_evidence_surveillance_contract_model_manifest", "model_throughput_evidence_surveillance_contract", "ids_throughput_evidence_surveillance_digest"]
