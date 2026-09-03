"""Python parity contract for the high-throughput retrieval workflow fabric."""
from __future__ import annotations

from dataclasses import dataclass
import re
from typing import Any, Mapping

from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError, research_artifact_digest

THROUGHPUT_RETRIEVAL_WORKFLOW_FABRIC_FEATURE_ID = "AFA-brain-P02-F15"
THROUGHPUT_RETRIEVAL_WORKFLOW_FABRIC_CONTRACT_VERSION = "brain-throughput-retrieval-workflow-fabric/1.0"


@dataclass(frozen=True)
class BrainThroughputRetrievalWorkflowFabricReceipt:
    request_id: str
    workflow_id: str
    batch_id: str
    partition: str
    disposition: str
    stage_order: tuple[str, ...]
    plan_order: tuple[str, ...]
    completed_order: tuple[str, ...]
    blocked_order: tuple[str, ...]
    compensation_order: tuple[str, ...]
    candidate_order: tuple[str, ...]
    ranked_order: tuple[str, ...]
    qualified_order: tuple[str, ...]
    unknown_order: tuple[str, ...]
    checkpoint_seq: int
    queue_digest: str
    synthesis_digest: str
    checkpoint_digest: str
    workflow_digest: str
    replay_identity: str
    budget_units: int
    omissions: tuple[str, ...]
    uncertainty: tuple[str, ...]
    negative_evidence: tuple[str, ...]
    effect_receipts: tuple[str, ...]
    artifact: Mapping[str, Any]
    feature_id: str = THROUGHPUT_RETRIEVAL_WORKFLOW_FABRIC_FEATURE_ID
    contract_version: str = THROUGHPUT_RETRIEVAL_WORKFLOW_FABRIC_CONTRACT_VERSION
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    raw_data_local: bool = True
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION or self.feature_id != THROUGHPUT_RETRIEVAL_WORKFLOW_FABRIC_FEATURE_ID or self.contract_version != THROUGHPUT_RETRIEVAL_WORKFLOW_FABRIC_CONTRACT_VERSION:
            raise ResearchContractError("throughput retrieval workflow schema, feature, or version mismatch")
        if self.boundary != PRECLINICAL_BOUNDARY or not self.raw_data_local or not self.request_id.strip() or not self.workflow_id.strip() or not self.batch_id.strip() or not self.partition.strip() or not self.stage_order or not self.plan_order or not self.completed_order or not self.effect_receipts or self.checkpoint_seq <= 0 or self.budget_units <= 0:
            raise ResearchContractError("throughput workflow identity, queue, checkpoint, stages, plan, locality, budget, or effects are incomplete")
        if any(value not in self.candidate_order for value in (*self.ranked_order, *self.qualified_order, *self.blocked_order, *self.unknown_order)):
            raise ResearchContractError("throughput workflow state is not covered by candidates")
        for values in (self.stage_order, self.plan_order, self.completed_order, self.blocked_order, self.compensation_order, self.candidate_order, self.ranked_order, self.qualified_order, self.unknown_order, self.omissions, self.uncertainty, self.negative_evidence, self.effect_receipts):
            if tuple(sorted(set(values))) != values:
                raise ResearchContractError("throughput workflow ordering is invalid")
        for value in (self.queue_digest, self.synthesis_digest, self.checkpoint_digest, self.workflow_digest, self.replay_identity, self.artifact.get("content_hash")):
            if not isinstance(value, str) or not re.fullmatch(r"[0-9a-f]{64}", value):
                raise ResearchContractError("throughput workflow digest is invalid")
        if any(not effect.startswith("schedule:throughput-retrieval-work:") and not effect.startswith("compensate:throughput-retrieval-work:") and effect != "block:unsafe-release" for effect in self.effect_receipts):
            raise ResearchContractError("throughput workflow effect is outside schedule/compensation gate")
        if self.disposition == "qualified" and not any(effect.startswith("schedule:throughput-retrieval-work:") for effect in self.effect_receipts):
            raise ResearchContractError("qualified throughput workflow requires schedule receipt")

    def digest(self) -> str:
        self.validate()
        return research_artifact_digest({
            "schema_version": self.schema_version, "contract_version": self.contract_version, "feature_id": self.feature_id,
            "request_id": self.request_id, "workflow_id": self.workflow_id, "batch_id": self.batch_id, "partition": self.partition, "disposition": self.disposition,
            "stage_order": list(self.stage_order), "plan_order": list(self.plan_order), "completed_order": list(self.completed_order), "blocked_order": list(self.blocked_order), "compensation_order": list(self.compensation_order),
            "candidate_order": list(self.candidate_order), "ranked_order": list(self.ranked_order), "qualified_order": list(self.qualified_order), "unknown_order": list(self.unknown_order), "checkpoint_seq": self.checkpoint_seq,
            "queue_digest": self.queue_digest, "synthesis_digest": self.synthesis_digest, "checkpoint_digest": self.checkpoint_digest, "workflow_digest": self.workflow_digest, "replay_identity": self.replay_identity, "budget_units": self.budget_units,
            "omissions": list(self.omissions), "uncertainty": list(self.uncertainty), "negative_evidence": list(self.negative_evidence), "effect_receipts": list(self.effect_receipts), "artifact": dict(self.artifact), "raw_data_local": self.raw_data_local, "boundary": self.boundary,
        })
