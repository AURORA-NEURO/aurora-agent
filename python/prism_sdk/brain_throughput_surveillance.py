"""Python mirror of the bounded high-throughput evidence receipt."""

from __future__ import annotations

from dataclasses import dataclass
import re
from typing import Any, Mapping

from .research_contracts import (
    HIGH_THROUGHPUT_EVIDENCE_SURVEILLANCE_CONTRACT_VERSION,
    HIGH_THROUGHPUT_EVIDENCE_SURVEILLANCE_FEATURE_ID,
    PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
    ResearchContractError,
    research_artifact_digest,
)


@dataclass(frozen=True)
class BrainHighThroughputEvidenceReceipt:
    request_id: str
    batch_id: str
    partition: str
    disposition: str
    candidate_order: tuple[str, ...]
    admitted_order: tuple[str, ...]
    blocked_order: tuple[str, ...]
    unknown_order: tuple[str, ...]
    relevance_order: tuple[int, ...]
    omissions: tuple[str, ...]
    uncertainty: tuple[str, ...]
    negative_evidence: tuple[str, ...]
    checkpoint_seq: int
    queue_digest: str
    replay_identity: str
    effect_receipts: tuple[str, ...]
    artifact: Mapping[str, Any]
    feature_id: str = HIGH_THROUGHPUT_EVIDENCE_SURVEILLANCE_FEATURE_ID
    contract_version: str = HIGH_THROUGHPUT_EVIDENCE_SURVEILLANCE_CONTRACT_VERSION
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    raw_data_local: bool = True
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION or self.feature_id != HIGH_THROUGHPUT_EVIDENCE_SURVEILLANCE_FEATURE_ID or self.contract_version != HIGH_THROUGHPUT_EVIDENCE_SURVEILLANCE_CONTRACT_VERSION:
            raise ResearchContractError("throughput evidence schema, feature, or version mismatch")
        if self.boundary != PRECLINICAL_BOUNDARY or not self.raw_data_local or not self.request_id.strip() or not self.batch_id.strip() or not self.partition.strip() or not self.candidate_order or len(self.relevance_order) != len(self.candidate_order) or self.checkpoint_seq < 1 or not self.effect_receipts:
            raise ResearchContractError("throughput identity, checkpoint, ranking, locality, or effects are incomplete")
        if self.disposition not in {"qualified", "partial", "unknown", "blocked"}:
            raise ResearchContractError("throughput disposition is unknown")
        if any(value not in self.candidate_order for value in (*self.admitted_order, *self.blocked_order, *self.unknown_order)):
            raise ResearchContractError("throughput state is not covered by candidate order")
        for values in (self.candidate_order, self.admitted_order, self.blocked_order, self.unknown_order, self.omissions, self.uncertainty, self.negative_evidence, self.effect_receipts):
            if tuple(sorted(set(values))) != values:
                raise ResearchContractError("throughput ordering is invalid")
        for value in (self.queue_digest, self.replay_identity, self.artifact.get("content_hash")):
            if not isinstance(value, str) or not re.fullmatch(r"[0-9a-f]{64}", value):
                raise ResearchContractError("throughput digest is invalid")
        if self.admitted_order and any(not effect.startswith("read:local-research-artifacts:") for effect in self.effect_receipts):
            raise ResearchContractError("admitted batch requires a local-read receipt")
        if not self.admitted_order and self.effect_receipts != ("block:unsafe-release",):
            raise ResearchContractError("empty batch must be explicitly blocked")

    def digest(self) -> str:
        self.validate()
        return research_artifact_digest({
            "schema_version": self.schema_version, "contract_version": self.contract_version, "feature_id": self.feature_id,
            "request_id": self.request_id, "batch_id": self.batch_id, "partition": self.partition, "disposition": self.disposition,
            "candidate_order": list(self.candidate_order), "admitted_order": list(self.admitted_order), "blocked_order": list(self.blocked_order),
            "unknown_order": list(self.unknown_order), "relevance_order": list(self.relevance_order), "omissions": list(self.omissions),
            "uncertainty": list(self.uncertainty), "negative_evidence": list(self.negative_evidence), "checkpoint_seq": self.checkpoint_seq,
            "queue_digest": self.queue_digest, "replay_identity": self.replay_identity, "effect_receipts": list(self.effect_receipts),
            "artifact": dict(self.artifact), "raw_data_local": self.raw_data_local, "boundary": self.boundary,
        })
