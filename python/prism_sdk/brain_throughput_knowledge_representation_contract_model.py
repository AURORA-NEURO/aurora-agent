"""Prospective high-throughput knowledge-representation contract parity surface."""
from __future__ import annotations

from dataclasses import dataclass
import re
from typing import Any, Mapping, Sequence

from .research_contracts import (
    PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
    ResearchContractError,
    THROUGHPUT_KNOWLEDGE_REPRESENTATION_CONTRACT_MODEL_CONTRACT_VERSION,
    THROUGHPUT_KNOWLEDGE_REPRESENTATION_CONTRACT_MODEL_FEATURE_ID,
    research_artifact_digest,
)

@dataclass(frozen=True)
class ThroughputKnowledgeContractJob:
    job_id: str
    study_id: str
    claim_count: int
    evidence_digest: str | None
    provenance_digest: str | None
    state: str = "supported"
    boundary: str = PRECLINICAL_BOUNDARY

@dataclass(frozen=True)
class ThroughputKnowledgeContractReceipt:
    request_id: str
    batch_id: str
    partition: str
    disposition: str
    input_schema: str
    output_schema: str
    source_revision: int
    target_revision: int
    max_concurrency: int
    checkpoint_seq: int
    budget_units: int
    candidate_order: tuple[str, ...]
    admitted_order: tuple[str, ...]
    unresolved_order: tuple[str, ...]
    denied_order: tuple[str, ...]
    capacity_exceeded_order: tuple[str, ...]
    semantic_loss_order: tuple[str, ...]
    contract_digest: str
    queue_digest: str
    migration_digest: str
    replay_identity: str
    omissions: tuple[str, ...]
    uncertainty: tuple[str, ...]
    negative_evidence: tuple[str, ...]
    effect_receipts: tuple[str, ...]
    artifact: Mapping[str, Any]
    feature_id: str = THROUGHPUT_KNOWLEDGE_REPRESENTATION_CONTRACT_MODEL_FEATURE_ID
    contract_version: str = THROUGHPUT_KNOWLEDGE_REPRESENTATION_CONTRACT_MODEL_CONTRACT_VERSION
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    raw_data_local: bool = True
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION or self.feature_id != THROUGHPUT_KNOWLEDGE_REPRESENTATION_CONTRACT_MODEL_FEATURE_ID or self.contract_version != THROUGHPUT_KNOWLEDGE_REPRESENTATION_CONTRACT_MODEL_CONTRACT_VERSION:
            raise ResearchContractError("throughput knowledge contract schema, feature, or version mismatch")
        if self.boundary != PRECLINICAL_BOUNDARY or not self.raw_data_local or not self.request_id.strip() or not self.batch_id.strip() or not self.partition.strip() or self.input_schema != "ScopedResearchClaims1@1" or self.output_schema != "TypedKnowledgeWorld1@1" or self.source_revision <= 0 or self.target_revision < self.source_revision or self.max_concurrency <= 0 or self.checkpoint_seq <= 0 or self.budget_units <= 0 or not self.candidate_order or not self.effect_receipts:
            raise ResearchContractError("throughput contract identity, schema, queue, capacity, checkpoint, budget, locality, or effects are incomplete")
        for values in (self.candidate_order, self.admitted_order, self.unresolved_order, self.denied_order, self.capacity_exceeded_order, self.semantic_loss_order, self.omissions, self.uncertainty, self.negative_evidence, self.effect_receipts):
            if tuple(sorted(set(values))) != values:
                raise ResearchContractError("throughput contract ordering is not canonical")
        classified = set(self.admitted_order) | set(self.unresolved_order) | set(self.denied_order)
        if classified != set(self.candidate_order) or any(value not in self.candidate_order for value in self.capacity_exceeded_order):
            raise ResearchContractError("throughput contract states do not partition jobs")
        for value in (self.contract_digest, self.queue_digest, self.migration_digest, self.replay_identity, self.artifact.get("content_hash")):
            if not isinstance(value, str) or not re.fullmatch(r"[0-9a-f]{64}", value):
                raise ResearchContractError("throughput contract digest is invalid")
        if any(not effect.startswith("read:local-throughput-knowledge-contract:") and effect != "block:unsafe-release" for effect in self.effect_receipts):
            raise ResearchContractError("throughput contract effect is outside local read gate")

    def digest(self) -> str:
        self.validate()
        return research_artifact_digest({"schema_version": self.schema_version, "contract_version": self.contract_version, "feature_id": self.feature_id, "request_id": self.request_id, "batch_id": self.batch_id, "partition": self.partition, "disposition": self.disposition, "input_schema": self.input_schema, "output_schema": self.output_schema, "source_revision": self.source_revision, "target_revision": self.target_revision, "max_concurrency": self.max_concurrency, "checkpoint_seq": self.checkpoint_seq, "budget_units": self.budget_units, "candidate_order": list(self.candidate_order), "admitted_order": list(self.admitted_order), "unresolved_order": list(self.unresolved_order), "denied_order": list(self.denied_order), "capacity_exceeded_order": list(self.capacity_exceeded_order), "semantic_loss_order": list(self.semantic_loss_order), "contract_digest": self.contract_digest, "queue_digest": self.queue_digest, "migration_digest": self.migration_digest, "replay_identity": self.replay_identity, "omissions": list(self.omissions), "uncertainty": list(self.uncertainty), "negative_evidence": list(self.negative_evidence), "effect_receipts": list(self.effect_receipts), "artifact": dict(self.artifact), "raw_data_local": self.raw_data_local, "boundary": self.boundary})

def model_throughput_knowledge_representation_contract(*, request_id: str, batch_id: str, partition: str, jobs: Sequence[ThroughputKnowledgeContractJob], input_schema: str = "ScopedResearchClaims1@1", output_schema: str = "TypedKnowledgeWorld1@1", source_revision: int = 1, target_revision: int = 1, migration_requested: bool = False, max_concurrency: int = 1, checkpoint_seq: int = 1, budget_units: int = 1, replay_identity: str, policy_allow: bool = True, protected_closure: bool = True, raw_data_local: bool = True) -> ThroughputKnowledgeContractReceipt:
    if not request_id.strip() or not batch_id.strip() or not partition.strip() or not jobs or input_schema != "ScopedResearchClaims1@1" or output_schema != "TypedKnowledgeWorld1@1" or source_revision <= 0 or target_revision < source_revision or (target_revision > source_revision and not migration_requested) or max_concurrency <= 0 or checkpoint_seq <= 0 or budget_units <= 0 or not re.fullmatch(r"[0-9a-f]{64}", replay_identity) or not raw_data_local:
        raise ResearchContractError("throughput contract identity, schemas, queue, revisions, capacity, checkpoint, budget, replay, locality, or boundary is invalid")
    ordered = tuple(sorted(job.job_id for job in jobs))
    if len(set(ordered)) != len(jobs) or any(not value.strip() for value in ordered):
        raise ResearchContractError("throughput job identifiers must be unique and non-empty")
    job_map = {job.job_id: job for job in jobs}; admitted: set[str] = set(); unresolved: set[str] = set(); denied: set[str] = set(); capacity: set[str] = set(); semantic_loss: set[str] = set(); omissions: set[str] = set(); uncertainty = {"gate:queue-order", "gate:checkpoint", "gate:budget", "gate:unknown-is-not-asserted", "gate:locality"}; negative: set[str] = set()
    for index, job_id in enumerate(ordered):
        job = job_map[job_id]
        if not policy_allow or not protected_closure or job.boundary != PRECLINICAL_BOUNDARY:
            denied.add(job_id); negative.add(f"job:{job_id}:scope-policy-closure")
        elif job.evidence_digest is None or job.provenance_digest is None:
            unresolved.add(job_id); omissions.add(f"job:{job_id}:evidence-or-provenance-missing")
        elif job.state in {"unknown", "speculative"}:
            unresolved.add(job_id); uncertainty.add(f"job:{job_id}:unknown-not-asserted")
        elif job.state == "contradicted":
            denied.add(job_id); negative.add(f"job:{job_id}:contradicted")
        elif index >= max_concurrency or job.claim_count > budget_units:
            unresolved.add(job_id); capacity.add(job_id); omissions.add(f"job:{job_id}:capacity-or-budget")
        else:
            admitted.add(job_id)
            if target_revision > source_revision: semantic_loss.add(job_id)
    if target_revision > source_revision: uncertainty.add(f"migration:{source_revision}-to-{target_revision}")
    if not policy_allow: omissions.add("control:policy-denied")
    if not protected_closure: omissions.add("control:protected-closure-incomplete")
    queue_digest = research_artifact_digest({"batch_id": batch_id, "partition": partition, "candidate_order": list(ordered), "max_concurrency": max_concurrency, "checkpoint_seq": checkpoint_seq, "budget_units": budget_units})
    contract_digest = research_artifact_digest({"queue_digest": queue_digest, "admitted_order": sorted(admitted), "unresolved_order": sorted(unresolved), "denied_order": sorted(denied), "capacity_exceeded_order": sorted(capacity), "semantic_loss_order": sorted(semantic_loss)})
    migration_digest = research_artifact_digest({"source_revision": source_revision, "target_revision": target_revision, "migration_requested": migration_requested, "semantic_loss_order": sorted(semantic_loss)})
    disposition = "blocked" if not policy_allow or not protected_closure or not raw_data_local else "partial" if not admitted or unresolved or denied else "migrated" if target_revision > source_revision else "completed"
    artifact = {"content_hash": research_artifact_digest({"request_id": request_id, "contract_digest": contract_digest}), "media_type": "application/vnd.aurora.typed-knowledge-world+json"}
    receipt = ThroughputKnowledgeContractReceipt(request_id=request_id, batch_id=batch_id, partition=partition, disposition=disposition, input_schema=input_schema, output_schema=output_schema, source_revision=source_revision, target_revision=target_revision, max_concurrency=max_concurrency, checkpoint_seq=checkpoint_seq, budget_units=budget_units, candidate_order=ordered, admitted_order=tuple(sorted(admitted)), unresolved_order=tuple(sorted(unresolved)), denied_order=tuple(sorted(denied)), capacity_exceeded_order=tuple(sorted(capacity)), semantic_loss_order=tuple(sorted(semantic_loss)), contract_digest=contract_digest, queue_digest=queue_digest, migration_digest=migration_digest, replay_identity=replay_identity, omissions=tuple(sorted(omissions)), uncertainty=tuple(sorted(uncertainty)), negative_evidence=tuple(sorted(negative)), effect_receipts=(f"read:local-throughput-knowledge-contract:{request_id}",) if disposition != "blocked" else ("block:unsafe-release",), artifact=artifact, raw_data_local=raw_data_local)
    receipt.validate(); return receipt
