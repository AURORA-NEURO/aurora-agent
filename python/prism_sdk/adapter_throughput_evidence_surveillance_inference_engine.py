"""Prospective high-throughput evidence-surveillance admission parity surface."""
from __future__ import annotations

from dataclasses import dataclass
import re
from typing import Any, Mapping, Sequence

from .research_contracts import (
    ADAPTER_THROUGHPUT_EVIDENCE_SURVEILLANCE_INFERENCE_ENGINE_CONTRACT_VERSION,
    ADAPTER_THROUGHPUT_EVIDENCE_SURVEILLANCE_INFERENCE_ENGINE_FEATURE_ID,
    PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
    ResearchContractError,
    research_artifact_digest,
)


@dataclass(frozen=True)
class ThroughputEvidenceObservation:
    observation_id: str
    batch_id: str
    sequence: int
    digest: str | None
    availability: str = "available"
    evidence_state: str = "supported"
    relevance_score: int = 0
    negative_result: bool = False


@dataclass(frozen=True)
class ThroughputEvidenceSurveillanceReceipt:
    request_id: str
    batch_id: str
    checkpoint_seq: int
    disposition: str
    candidate_order: tuple[str, ...]
    ranked_order: tuple[str, ...]
    selected_order: tuple[str, ...]
    unresolved_order: tuple[str, ...]
    denied_order: tuple[str, ...]
    overflow_order: tuple[str, ...]
    queue_digest: str
    checkpoint_digest: str
    evidence_digest: str
    provenance_digest: str
    replay_identity: str
    omissions: tuple[str, ...]
    uncertainty: tuple[str, ...]
    negative_evidence: tuple[str, ...]
    effect_receipts: tuple[str, ...]
    qualified_set: Mapping[str, Any]
    artifact: Mapping[str, Any]
    feature_id: str = ADAPTER_THROUGHPUT_EVIDENCE_SURVEILLANCE_INFERENCE_ENGINE_FEATURE_ID
    contract_version: str = ADAPTER_THROUGHPUT_EVIDENCE_SURVEILLANCE_INFERENCE_ENGINE_CONTRACT_VERSION
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    raw_data_local: bool = True
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION or self.feature_id != ADAPTER_THROUGHPUT_EVIDENCE_SURVEILLANCE_INFERENCE_ENGINE_FEATURE_ID or self.contract_version != ADAPTER_THROUGHPUT_EVIDENCE_SURVEILLANCE_INFERENCE_ENGINE_CONTRACT_VERSION:
            raise ResearchContractError("throughput evidence schema, feature, or version mismatch")
        if self.boundary != PRECLINICAL_BOUNDARY or not self.raw_data_local or not self.request_id.strip() or not self.batch_id.strip() or self.checkpoint_seq <= 0 or not self.candidate_order or not self.effect_receipts or self.qualified_set.get("batch_id") != self.batch_id:
            raise ResearchContractError("throughput identity, checkpoint, locality, candidates, effects, or qualified-set linkage is incomplete")
        for values in (self.candidate_order, self.selected_order, self.unresolved_order, self.denied_order, self.overflow_order, self.omissions, self.uncertainty, self.negative_evidence, self.effect_receipts, tuple(self.qualified_set.get("selected_order", ())), tuple(self.qualified_set.get("omissions", ())), tuple(self.qualified_set.get("uncertainty", ())), tuple(self.qualified_set.get("negative_order", ()) )):
            if tuple(sorted(set(values))) != values:
                raise ResearchContractError("throughput ordering is not canonical")
        if len(self.ranked_order) != len(self.candidate_order) or set(self.ranked_order) != set(self.candidate_order):
            raise ResearchContractError("throughput ranking must cover candidates exactly")
        if set(self.selected_order) | set(self.unresolved_order) | set(self.denied_order) | set(self.overflow_order) != set(self.candidate_order):
            raise ResearchContractError("throughput states do not partition candidates")
        for value in (self.queue_digest, self.checkpoint_digest, self.evidence_digest, self.provenance_digest, self.replay_identity, self.artifact.get("content_hash")):
            if not isinstance(value, str) or not re.fullmatch(r"[0-9a-f]{64}", value):
                raise ResearchContractError("throughput digest is invalid")
        if any(not effect.startswith("read:local-throughput-evidence:") and effect != "block:unsafe-release" for effect in self.effect_receipts):
            raise ResearchContractError("throughput effect is outside local-read gate")
        if self.disposition == "blocked" and self.effect_receipts != ("block:unsafe-release",):
            raise ResearchContractError("blocked throughput surveillance must be explicitly blocked")

    def digest(self) -> str:
        self.validate()
        return research_artifact_digest({"schema_version": self.schema_version, "contract_version": self.contract_version, "feature_id": self.feature_id, "request_id": self.request_id, "batch_id": self.batch_id, "checkpoint_seq": self.checkpoint_seq, "disposition": self.disposition, "candidate_order": list(self.candidate_order), "ranked_order": list(self.ranked_order), "selected_order": list(self.selected_order), "unresolved_order": list(self.unresolved_order), "denied_order": list(self.denied_order), "overflow_order": list(self.overflow_order), "queue_digest": self.queue_digest, "checkpoint_digest": self.checkpoint_digest, "evidence_digest": self.evidence_digest, "provenance_digest": self.provenance_digest, "replay_identity": self.replay_identity, "omissions": list(self.omissions), "uncertainty": list(self.uncertainty), "negative_evidence": list(self.negative_evidence), "effect_receipts": list(self.effect_receipts), "qualified_set": dict(self.qualified_set), "artifact": dict(self.artifact), "raw_data_local": self.raw_data_local, "boundary": self.boundary})


def run_throughput_evidence_surveillance(*, request_id: str, batch_id: str, checkpoint_seq: int, observations: Sequence[ThroughputEvidenceObservation], max_items: int, budget_units: int, min_relevance_score: int = 0, policy_allow: bool = True, protected_closure: bool = True, raw_data_local: bool = True, replay_identity: str, previous_checkpoint: str | None = None) -> ThroughputEvidenceSurveillanceReceipt:
    if not request_id.strip() or not batch_id.strip() or checkpoint_seq <= 0 or not observations or max_items <= 0 or budget_units <= 0 or not re.fullmatch(r"[0-9a-f]{64}", replay_identity) or not raw_data_local or any(item.batch_id != batch_id or not item.observation_id.strip() for item in observations):
        raise ResearchContractError("throughput identity, checkpoint, observations, capacity, budget, replay, or locality is invalid")
    ordered = tuple(sorted(observations, key=lambda item: (item.sequence, item.observation_id))); ranked = tuple(item.observation_id for item in ordered); candidate = tuple(sorted(ranked))
    if len(set(candidate)) != len(candidate):
        raise ResearchContractError("throughput observation identities must be unique")
    admission_limit = min(max_items, budget_units); admitted, overflow = ordered[:admission_limit], ordered[admission_limit:]; overflow_order = tuple(sorted(item.observation_id for item in overflow))
    selected: set[str] = set(); unresolved: set[str] = set(); denied: set[str] = set(); digest_map: dict[str, str] = {}; omissions: set[str] = set(); uncertainty: set[str] = set(); negative: set[str] = set()
    if len(ordered) > max_items: omissions.add(f"queue:capacity-exceeded:{len(ordered) - max_items}")
    if budget_units < max_items: omissions.add(f"queue:budget-bounded:{max_items - budget_units}")
    for item in admitted:
        if not policy_allow or not protected_closure or not raw_data_local:
            denied.add(item.observation_id); omissions.add(f"evidence:{item.observation_id}:policy-closure-locality")
        elif item.availability != "available":
            unresolved.add(item.observation_id); omissions.add(f"evidence:{item.observation_id}:availability-{item.availability}")
        elif item.relevance_score < min_relevance_score:
            unresolved.add(item.observation_id); uncertainty.add(f"evidence:{item.observation_id}:relevance-below-threshold")
        elif item.digest is None:
            unresolved.add(item.observation_id); omissions.add(f"evidence:{item.observation_id}:content-digest-missing")
        elif item.evidence_state in {"unknown", "speculative"}:
            unresolved.add(item.observation_id); uncertainty.add(f"evidence:{item.observation_id}:unknown-not-asserted")
        elif item.evidence_state == "contradicted":
            denied.add(item.observation_id); negative.add(f"evidence:{item.observation_id}:contradicted")
        else:
            selected.add(item.observation_id); digest_map[item.observation_id] = item.digest
            if item.negative_result: negative.add(f"evidence:{item.observation_id}:negative-result")
    if not policy_allow: omissions.add("control:policy-denied")
    if not protected_closure: omissions.add("control:protected-closure-incomplete")
    if not raw_data_local: omissions.add("control:raw-data-locality-failed")
    disposition = "blocked" if not policy_allow or not protected_closure or not raw_data_local else "unknown" if not selected else "partial" if unresolved or denied or overflow_order else "completed"
    selected_order = tuple(sorted(selected)); unresolved_order = tuple(sorted(unresolved)); denied_order = tuple(sorted(denied)); omissions_order = tuple(sorted(omissions)); uncertainty_order = tuple(sorted(uncertainty)); negative_order = tuple(sorted(negative))
    queue_digest = research_artifact_digest({"batch_id": batch_id, "candidate_order": list(candidate), "ranked_order": list(ranked), "overflow_order": list(overflow_order)})
    checkpoint_digest = research_artifact_digest({"batch_id": batch_id, "checkpoint_seq": checkpoint_seq, "previous_checkpoint": previous_checkpoint, "queue_digest": queue_digest})
    evidence_digest = research_artifact_digest({"selected_order": list(selected_order), "unresolved_order": list(unresolved_order), "denied_order": list(denied_order)})
    provenance_digest = research_artifact_digest({"request_id": request_id, "replay_identity": replay_identity, "checkpoint_digest": checkpoint_digest, "evidence_digest": evidence_digest})
    qualified_set = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "set_id": f"qualified-throughput-evidence:{batch_id}", "batch_id": batch_id, "checkpoint_seq": checkpoint_seq, "selected_order": list(selected_order), "selected_digests": [digest_map[item] for item in selected_order], "omissions": list(omissions_order), "uncertainty": list(uncertainty_order), "negative_order": list(negative_order), "boundary": PRECLINICAL_BOUNDARY}
    artifact = {"content_hash": research_artifact_digest(qualified_set), "media_type": "application/vnd.aurora.qualified-throughput-evidence-set+json"}
    receipt = ThroughputEvidenceSurveillanceReceipt(request_id=request_id, batch_id=batch_id, checkpoint_seq=checkpoint_seq, disposition=disposition, candidate_order=candidate, ranked_order=ranked, selected_order=selected_order, unresolved_order=unresolved_order, denied_order=denied_order, overflow_order=overflow_order, queue_digest=queue_digest, checkpoint_digest=checkpoint_digest, evidence_digest=evidence_digest, provenance_digest=provenance_digest, replay_identity=replay_identity, omissions=omissions_order, uncertainty=uncertainty_order, negative_evidence=negative_order, effect_receipts=(f"read:local-throughput-evidence:{batch_id}",) if disposition != "blocked" else ("block:unsafe-release",), qualified_set=qualified_set, artifact=artifact, raw_data_local=raw_data_local)
    receipt.validate(); return receipt
