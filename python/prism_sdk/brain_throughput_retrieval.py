"""Python parity contract for bounded high-throughput retrieval synthesis."""
from __future__ import annotations

from dataclasses import dataclass
import re
from typing import Any, Mapping

from .research_contracts import (
    PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
    ResearchContractError,
    research_artifact_digest,
)

THROUGHPUT_RETRIEVAL_SYNTHESIS_FEATURE_ID = "AFA-brain-P02-F03"
THROUGHPUT_RETRIEVAL_SYNTHESIS_CONTRACT_VERSION = "brain-throughput-retrieval-synthesis/1.0"


@dataclass(frozen=True)
class BrainThroughputEvidenceSynthesis:
    request_id: str
    batch_id: str
    partition: str
    disposition: str
    candidate_order: tuple[str, ...]
    ranked_order: tuple[str, ...]
    qualified_order: tuple[str, ...]
    blocked_order: tuple[str, ...]
    unknown_order: tuple[str, ...]
    support_order: tuple[int, ...]
    checkpoint_seq: int
    queue_digest: str
    synthesis_digest: str
    omissions: tuple[str, ...]
    uncertainty: tuple[str, ...]
    negative_evidence: tuple[str, ...]
    replay_identity: str
    effect_receipts: tuple[str, ...]
    artifact: Mapping[str, Any]
    feature_id: str = THROUGHPUT_RETRIEVAL_SYNTHESIS_FEATURE_ID
    contract_version: str = THROUGHPUT_RETRIEVAL_SYNTHESIS_CONTRACT_VERSION
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    raw_data_local: bool = True
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if (
            self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            or self.feature_id != THROUGHPUT_RETRIEVAL_SYNTHESIS_FEATURE_ID
            or self.contract_version != THROUGHPUT_RETRIEVAL_SYNTHESIS_CONTRACT_VERSION
        ):
            raise ResearchContractError("throughput retrieval schema mismatch")
        if (
            self.boundary != PRECLINICAL_BOUNDARY
            or not self.raw_data_local
            or not self.request_id.strip()
            or not self.batch_id.strip()
            or not self.partition.strip()
            or self.disposition not in ("qualified", "partial", "unknown", "blocked")
            or not self.candidate_order
            or not self.ranked_order
            or len(self.ranked_order) != len(self.support_order)
            or not self.effect_receipts
            or self.checkpoint_seq < 0
        ):
            raise ResearchContractError("throughput retrieval identity incomplete")
        if any(
            value not in self.candidate_order
            for value in (
                *self.ranked_order,
                *self.qualified_order,
                *self.blocked_order,
                *self.unknown_order,
            )
        ):
            raise ResearchContractError("throughput retrieval state is not covered")
        for values in (
            self.candidate_order,
            self.qualified_order,
            self.blocked_order,
            self.unknown_order,
            self.omissions,
            self.uncertainty,
            self.negative_evidence,
            self.effect_receipts,
        ):
            if tuple(sorted(set(values))) != values:
                raise ResearchContractError("throughput retrieval ordering invalid")
        for value in (
            self.queue_digest,
            self.synthesis_digest,
            self.replay_identity,
            self.artifact.get("content_hash"),
        ):
            if not isinstance(value, str) or not re.fullmatch(r"[0-9a-f]{64}", value):
                raise ResearchContractError("throughput retrieval digest invalid")
        if any(
            not effect.startswith("read:local-throughput-artifacts:")
            and effect != "block:unsafe-release"
            for effect in self.effect_receipts
        ):
            raise ResearchContractError("throughput retrieval effect invalid")

    def digest(self) -> str:
        self.validate()
        return research_artifact_digest(
            {
                "schema_version": self.schema_version,
                "contract_version": self.contract_version,
                "feature_id": self.feature_id,
                "request_id": self.request_id,
                "batch_id": self.batch_id,
                "partition": self.partition,
                "disposition": self.disposition,
                "candidate_order": list(self.candidate_order),
                "ranked_order": list(self.ranked_order),
                "qualified_order": list(self.qualified_order),
                "blocked_order": list(self.blocked_order),
                "unknown_order": list(self.unknown_order),
                "support_order": list(self.support_order),
                "checkpoint_seq": self.checkpoint_seq,
                "queue_digest": self.queue_digest,
                "synthesis_digest": self.synthesis_digest,
                "omissions": list(self.omissions),
                "uncertainty": list(self.uncertainty),
                "negative_evidence": list(self.negative_evidence),
                "replay_identity": self.replay_identity,
                "effect_receipts": list(self.effect_receipts),
                "artifact": dict(self.artifact),
                "raw_data_local": self.raw_data_local,
                "boundary": self.boundary,
            }
        )
