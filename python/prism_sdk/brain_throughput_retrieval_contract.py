"""Python parity contract for the high-throughput retrieval contract model."""
from __future__ import annotations

from dataclasses import dataclass
import re
from typing import Any, Mapping

from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError, research_artifact_digest

THROUGHPUT_RETRIEVAL_CONTRACT_MODEL_FEATURE_ID = "AFA-brain-P02-F07"
THROUGHPUT_RETRIEVAL_CONTRACT_MODEL_CONTRACT_VERSION = "brain-throughput-retrieval-contract-model/1.0"
THROUGHPUT_RETRIEVAL_INPUT_SCHEMA = "ScopedRetrievalQuery3@1"
THROUGHPUT_RETRIEVAL_OUTPUT_SCHEMA = "EvidenceSynthesis2@1"


@dataclass(frozen=True)
class BrainThroughputRetrievalContractModelReceipt:
    request_id: str
    batch_id: str
    partition: str
    max_items: int
    checkpoint_seq: int
    disposition: str
    compatibility: str
    input_schema: str
    output_schema: str
    required_order: tuple[str, ...]
    provided_order: tuple[str, ...]
    missing_order: tuple[str, ...]
    semantic_loss_order: tuple[str, ...]
    queue_digest: str
    semantic_digest: str
    artifact_digest: str
    provenance_digest: str
    contract_digest: str
    replay_identity: str
    omissions: tuple[str, ...]
    uncertainty: tuple[str, ...]
    negative_evidence: tuple[str, ...]
    effect_receipts: tuple[str, ...]
    artifact: Mapping[str, Any]
    feature_id: str = THROUGHPUT_RETRIEVAL_CONTRACT_MODEL_FEATURE_ID
    contract_version: str = THROUGHPUT_RETRIEVAL_CONTRACT_MODEL_CONTRACT_VERSION
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    raw_data_local: bool = True
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION or self.feature_id != THROUGHPUT_RETRIEVAL_CONTRACT_MODEL_FEATURE_ID or self.contract_version != THROUGHPUT_RETRIEVAL_CONTRACT_MODEL_CONTRACT_VERSION or self.input_schema != THROUGHPUT_RETRIEVAL_INPUT_SCHEMA or self.output_schema != THROUGHPUT_RETRIEVAL_OUTPUT_SCHEMA:
            raise ResearchContractError("throughput retrieval contract schema mismatch")
        if self.boundary != PRECLINICAL_BOUNDARY or not self.raw_data_local or not self.request_id.strip() or not self.batch_id.strip() or not self.partition.strip() or self.max_items <= 0 or self.checkpoint_seq <= 0 or self.disposition not in ("qualified", "partial", "unknown", "blocked") or self.compatibility not in ("additive", "migration_required", "breaking", "unknown") or not self.required_order or not self.provided_order or not self.effect_receipts:
            raise ResearchContractError("throughput retrieval contract identity incomplete")
        if any(value not in self.required_order for value in self.missing_order) or any(value not in self.provided_order for value in self.semantic_loss_order):
            raise ResearchContractError("throughput retrieval contract loss state is not covered")
        for values in (self.required_order, self.provided_order, self.missing_order, self.semantic_loss_order, self.omissions, self.uncertainty, self.negative_evidence, self.effect_receipts):
            if tuple(sorted(set(values))) != values:
                raise ResearchContractError("throughput retrieval contract ordering invalid")
        for value in (self.queue_digest, self.semantic_digest, self.artifact_digest, self.provenance_digest, self.contract_digest, self.replay_identity, self.artifact.get("content_hash")):
            if not isinstance(value, str) or not re.fullmatch(r"[0-9a-f]{64}", value):
                raise ResearchContractError("throughput retrieval contract digest invalid")
        if any(not effect.startswith("read:local-throughput-artifacts:") and effect != "block:unsafe-release" for effect in self.effect_receipts):
            raise ResearchContractError("throughput retrieval contract effect invalid")

    def digest(self) -> str:
        self.validate()
        return research_artifact_digest({"schema_version": self.schema_version, "contract_version": self.contract_version, "feature_id": self.feature_id, "request_id": self.request_id, "batch_id": self.batch_id, "partition": self.partition, "max_items": self.max_items, "checkpoint_seq": self.checkpoint_seq, "disposition": self.disposition, "compatibility": self.compatibility, "input_schema": self.input_schema, "output_schema": self.output_schema, "required_order": list(self.required_order), "provided_order": list(self.provided_order), "missing_order": list(self.missing_order), "semantic_loss_order": list(self.semantic_loss_order), "queue_digest": self.queue_digest, "semantic_digest": self.semantic_digest, "artifact_digest": self.artifact_digest, "provenance_digest": self.provenance_digest, "contract_digest": self.contract_digest, "replay_identity": self.replay_identity, "omissions": list(self.omissions), "uncertainty": list(self.uncertainty), "negative_evidence": list(self.negative_evidence), "effect_receipts": list(self.effect_receipts), "artifact": dict(self.artifact), "raw_data_local": self.raw_data_local, "boundary": self.boundary})
