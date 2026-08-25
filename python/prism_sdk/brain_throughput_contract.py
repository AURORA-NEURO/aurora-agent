"""Python mirror of the high-throughput evidence contract receipt."""

from __future__ import annotations

from dataclasses import dataclass
import re
from typing import Any, Mapping

from .research_contracts import (
    THROUGHPUT_CONTRACT_MODEL_CONTRACT_VERSION,
    THROUGHPUT_CONTRACT_MODEL_FEATURE_ID,
    PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
    ResearchContractError,
    research_artifact_digest,
)


@dataclass(frozen=True)
class BrainThroughputContractModelReceipt:
    request_id: str
    batch_id: str
    partition: str
    disposition: str
    compatibility: str
    input_schema: str
    output_schema: str
    required_order: tuple[str, ...]
    provided_order: tuple[str, ...]
    missing_order: tuple[str, ...]
    semantic_loss_order: tuple[str, ...]
    max_items: int
    observed_items: int
    admitted_items: int
    checkpoint_seq: int
    queue_digest: str
    contract_digest: str
    replay_identity: str
    omissions: tuple[str, ...]
    uncertainty: tuple[str, ...]
    negative_evidence: tuple[str, ...]
    effect_receipts: tuple[str, ...]
    artifact: Mapping[str, Any]
    feature_id: str = THROUGHPUT_CONTRACT_MODEL_FEATURE_ID
    contract_version: str = THROUGHPUT_CONTRACT_MODEL_CONTRACT_VERSION
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    raw_data_local: bool = True
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION or self.feature_id != THROUGHPUT_CONTRACT_MODEL_FEATURE_ID or self.contract_version != THROUGHPUT_CONTRACT_MODEL_CONTRACT_VERSION:
            raise ResearchContractError("throughput contract schema, feature, or version mismatch")
        if self.boundary != PRECLINICAL_BOUNDARY or not self.raw_data_local or not self.request_id.strip() or not self.batch_id.strip() or not self.partition.strip() or self.input_schema != "EvidenceFeed3@1" or self.output_schema != "QualifiedEvidenceSet2@1" or not self.required_order or not self.provided_order or self.max_items < 1 or self.checkpoint_seq < 1 or not self.effect_receipts:
            raise ResearchContractError("throughput identity, schemas, fields, capacity, checkpoint, locality, or effects are incomplete")
        if self.admitted_items > self.max_items or self.admitted_items > self.observed_items:
            raise ResearchContractError("admitted item count exceeds declared capacity or observations")
        if any(value not in self.required_order for value in self.missing_order) or any(value not in self.provided_order for value in self.semantic_loss_order):
            raise ResearchContractError("throughput loss state is outside declared fields")
        for values in (self.required_order, self.provided_order, self.missing_order, self.semantic_loss_order, self.omissions, self.uncertainty, self.negative_evidence, self.effect_receipts):
            if tuple(sorted(set(values))) != values:
                raise ResearchContractError("throughput contract ordering is invalid")
        for value in (self.queue_digest, self.contract_digest, self.replay_identity, self.artifact.get("content_hash")):
            if not isinstance(value, str) or not re.fullmatch(r"[0-9a-f]{64}", value):
                raise ResearchContractError("throughput contract digest is invalid")
        if self.disposition == "qualified" and any(not effect.startswith("read:local-research-artifacts:") for effect in self.effect_receipts):
            raise ResearchContractError("qualified throughput contract requires a local-read receipt")
        if self.disposition != "qualified" and self.effect_receipts != ("block:unsafe-release",):
            raise ResearchContractError("non-qualified throughput contract must be explicitly blocked")

    def digest(self) -> str:
        self.validate()
        return research_artifact_digest({
            "schema_version": self.schema_version, "contract_version": self.contract_version, "feature_id": self.feature_id,
            "request_id": self.request_id, "batch_id": self.batch_id, "partition": self.partition, "disposition": self.disposition,
            "compatibility": self.compatibility, "input_schema": self.input_schema, "output_schema": self.output_schema,
            "required_order": list(self.required_order), "provided_order": list(self.provided_order), "missing_order": list(self.missing_order),
            "semantic_loss_order": list(self.semantic_loss_order), "max_items": self.max_items, "observed_items": self.observed_items,
            "admitted_items": self.admitted_items, "checkpoint_seq": self.checkpoint_seq, "queue_digest": self.queue_digest,
            "contract_digest": self.contract_digest, "replay_identity": self.replay_identity, "omissions": list(self.omissions),
            "uncertainty": list(self.uncertainty), "negative_evidence": list(self.negative_evidence), "effect_receipts": list(self.effect_receipts),
            "artifact": dict(self.artifact), "raw_data_local": self.raw_data_local, "boundary": self.boundary,
        })
