"""Python parity contract for bounded throughput context compilation."""
from __future__ import annotations

from dataclasses import dataclass
import re
from typing import Any, Mapping, Sequence

from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError, research_artifact_digest

THROUGHPUT_CONTEXT_COMPILATION_FEATURE_ID = "AFA-brain-P03-F03"
THROUGHPUT_CONTEXT_COMPILATION_CONTRACT_VERSION = "brain-throughput-context-compilation/1.0"


@dataclass(frozen=True)
class BrainThroughputContextCompilationReceipt:
    request_id: str
    batch_id: str
    objective: str
    disposition: str
    batch_order: tuple[str, ...]
    accepted_order: tuple[str, ...]
    deferred_order: tuple[str, ...]
    blocked_order: tuple[str, ...]
    unknown_order: tuple[str, ...]
    queue_digest: str
    throughput_digest: str
    replay_identity: str
    omissions: tuple[str, ...]
    uncertainty: tuple[str, ...]
    negative_evidence: tuple[str, ...]
    effect_receipts: tuple[str, ...]
    artifact: Mapping[str, Any]
    feature_id: str = THROUGHPUT_CONTEXT_COMPILATION_FEATURE_ID
    contract_version: str = THROUGHPUT_CONTEXT_COMPILATION_CONTRACT_VERSION
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    raw_data_local: bool = True
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION or self.feature_id != THROUGHPUT_CONTEXT_COMPILATION_FEATURE_ID or self.contract_version != THROUGHPUT_CONTEXT_COMPILATION_CONTRACT_VERSION:
            raise ResearchContractError("throughput context compilation schema, feature, or version mismatch")
        if self.boundary != PRECLINICAL_BOUNDARY or not self.raw_data_local or not self.request_id.strip() or not self.batch_id.strip() or not self.objective.strip() or not self.batch_order or not self.effect_receipts or self.disposition not in {"qualified", "partial", "unknown", "blocked"}:
            raise ResearchContractError("throughput context identity, batch, locality, disposition, or effects are incomplete")
        for values in (self.batch_order, self.accepted_order, self.deferred_order, self.blocked_order, self.unknown_order, self.omissions, self.uncertainty, self.negative_evidence, self.effect_receipts):
            if tuple(sorted(set(values))) != values:
                raise ResearchContractError("throughput context vectors are not canonical")
        batch = set(self.batch_order)
        classified = set(self.accepted_order) | set(self.deferred_order) | set(self.blocked_order) | set(self.unknown_order)
        if classified != batch or sum(len(set(values)) for values in (self.accepted_order, self.deferred_order, self.blocked_order, self.unknown_order)) != len(batch):
            raise ResearchContractError("throughput context queue states do not partition the batch")
        for value in (self.queue_digest, self.throughput_digest, self.replay_identity, self.artifact.get("content_hash")):
            if not isinstance(value, str) or not re.fullmatch(r"[0-9a-f]{64}", value):
                raise ResearchContractError("throughput context digest is invalid")
        if any(not effect.startswith("compile:local-throughput-context:") and effect != "block:unsafe-release" for effect in self.effect_receipts):
            raise ResearchContractError("throughput context effect is outside local compilation gate")

    def digest(self) -> str:
        self.validate()
        return research_artifact_digest({"schema_version": self.schema_version, "contract_version": self.contract_version, "feature_id": self.feature_id, "request_id": self.request_id, "batch_id": self.batch_id, "objective": self.objective, "disposition": self.disposition, "batch_order": list(self.batch_order), "accepted_order": list(self.accepted_order), "deferred_order": list(self.deferred_order), "blocked_order": list(self.blocked_order), "unknown_order": list(self.unknown_order), "queue_digest": self.queue_digest, "throughput_digest": self.throughput_digest, "replay_identity": self.replay_identity, "omissions": list(self.omissions), "uncertainty": list(self.uncertainty), "negative_evidence": list(self.negative_evidence), "effect_receipts": list(self.effect_receipts), "artifact": dict(self.artifact), "raw_data_local": self.raw_data_local, "boundary": self.boundary})


def compile_throughput_context(*, request_id: str, batch_id: str, objective: str, items: Sequence[Mapping[str, Any]], max_items: int, replay_identity: str, policy_allow: bool = True, raw_data_local: bool = True) -> BrainThroughputContextCompilationReceipt:
    if not request_id.strip() or not batch_id.strip() or not objective.strip() or not items or max_items <= 0 or not re.fullmatch(r"[0-9a-f]{64}", replay_identity):
        raise ResearchContractError("throughput context request identity, capacity, or replay is invalid")
    ordered = sorted(items, key=lambda item: str(item["context_id"]))
    ids = tuple(str(item["context_id"]) for item in ordered)
    if len(set(ids)) != len(ids) or any(not value.strip() for value in ids):
        raise ResearchContractError("throughput context identifiers must be unique and non-empty")
    accepted: list[str] = []; deferred: list[str] = []; blocked: list[str] = []; unknown: list[str] = []; omissions: list[str] = []; uncertainty: list[str] = []; capacity = max_items
    for item in ordered:
        context_id = str(item["context_id"])
        if not policy_allow or not raw_data_local or not bool(item.get("policy_allow", True)) or not bool(item.get("raw_data_local", True)):
            blocked.append(context_id); omissions.append(f"context:{context_id}:scope-or-policy-blocked")
        elif str(item.get("replay_identity")) != replay_identity:
            unknown.append(context_id); uncertainty.append(f"context:{context_id}:replay-mismatch")
        elif int(item.get("required_fact_count", 0)) == 0 or int(item.get("supported_fact_count", 0)) == 0:
            unknown.append(context_id); uncertainty.append(f"context:{context_id}:no-qualified-facts")
        elif int(item["supported_fact_count"]) < int(item["required_fact_count"]):
            deferred.append(context_id); omissions.append(f"context:{context_id}:incomplete-fact-closure")
        elif capacity == 0:
            deferred.append(context_id); omissions.append(f"context:{context_id}:capacity-deferred")
        else:
            accepted.append(context_id); capacity -= 1
    disposition = "blocked" if not policy_allow or not raw_data_local else ("qualified" if accepted and not deferred and not blocked and not unknown else ("partial" if accepted or deferred else "unknown"))
    queue_digest = research_artifact_digest({"batch_id": batch_id, "batch_order": list(ids), "replay_identity": replay_identity})
    throughput_digest = research_artifact_digest({"accepted": accepted, "deferred": deferred, "blocked": blocked, "unknown": unknown, "max_items": max_items})
    effects = (f"compile:local-throughput-context:{batch_id}",) if disposition in {"qualified", "partial"} else ("block:unsafe-release",)
    artifact = {"content_hash": research_artifact_digest({"batch_id": batch_id, "queue_digest": queue_digest, "throughput_digest": throughput_digest}), "media_type": "application/vnd.aurora.throughput-context+json"}
    receipt = BrainThroughputContextCompilationReceipt(request_id=request_id, batch_id=batch_id, objective=objective, disposition=disposition, batch_order=ids, accepted_order=tuple(accepted), deferred_order=tuple(deferred), blocked_order=tuple(blocked), unknown_order=tuple(unknown), queue_digest=queue_digest, throughput_digest=throughput_digest, replay_identity=replay_identity, omissions=tuple(sorted(omissions)), uncertainty=tuple(sorted(uncertainty)), negative_evidence=(), effect_receipts=effects, artifact=artifact)
    receipt.validate()
    return receipt
