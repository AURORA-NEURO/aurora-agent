"""Python parity contract for the throughput retrieval research copilot."""
from __future__ import annotations

from dataclasses import dataclass
import re
from typing import Any, Mapping

from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError, research_artifact_digest

THROUGHPUT_RETRIEVAL_COPILOT_FEATURE_ID = "AFA-brain-P02-F11"
THROUGHPUT_RETRIEVAL_COPILOT_CONTRACT_VERSION = "brain-throughput-retrieval-research-copilot/1.0"


@dataclass(frozen=True)
class BrainThroughputRetrievalCopilotReceipt:
    request_id: str
    operator_id: str
    batch_id: str
    partition: str
    disposition: str
    plan_order: tuple[str, ...]
    action_order: tuple[str, ...]
    tool_order: tuple[str, ...]
    candidate_order: tuple[str, ...]
    qualified_order: tuple[str, ...]
    blocked_order: tuple[str, ...]
    unknown_order: tuple[str, ...]
    checkpoint_seq: int
    queue_digest: str
    synthesis_digest: str
    plan_digest: str
    approval_reference: str
    replay_identity: str
    budget_units: int
    omissions: tuple[str, ...]
    uncertainty: tuple[str, ...]
    negative_evidence: tuple[str, ...]
    effect_receipts: tuple[str, ...]
    artifact: Mapping[str, Any]
    feature_id: str = THROUGHPUT_RETRIEVAL_COPILOT_FEATURE_ID
    contract_version: str = THROUGHPUT_RETRIEVAL_COPILOT_CONTRACT_VERSION
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    raw_data_local: bool = True
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION or self.feature_id != THROUGHPUT_RETRIEVAL_COPILOT_FEATURE_ID or self.contract_version != THROUGHPUT_RETRIEVAL_COPILOT_CONTRACT_VERSION:
            raise ResearchContractError("throughput retrieval copilot schema mismatch")
        if self.boundary != PRECLINICAL_BOUNDARY or not self.raw_data_local or not self.request_id.strip() or not self.operator_id.strip() or not self.batch_id.strip() or not self.partition.strip() or self.disposition not in ("qualified", "partial", "unknown", "blocked") or not self.plan_order or len(self.plan_order) != len(self.action_order) or not self.tool_order or self.checkpoint_seq <= 0 or self.budget_units <= 0 or not self.effect_receipts:
            raise ResearchContractError("throughput retrieval copilot identity incomplete")
        if any(value not in self.candidate_order for value in (*self.qualified_order, *self.blocked_order, *self.unknown_order)):
            raise ResearchContractError("throughput retrieval copilot state is not covered")
        for values in (self.plan_order, self.action_order, self.tool_order, self.candidate_order, self.qualified_order, self.blocked_order, self.unknown_order, self.omissions, self.uncertainty, self.negative_evidence, self.effect_receipts):
            if tuple(sorted(set(values))) != values:
                raise ResearchContractError("throughput retrieval copilot ordering invalid")
        for value in (self.queue_digest, self.synthesis_digest, self.plan_digest, self.approval_reference, self.replay_identity, self.artifact.get("content_hash")):
            if not isinstance(value, str) or not re.fullmatch(r"[0-9a-f]{64}", value):
                raise ResearchContractError("throughput retrieval copilot digest invalid")
        if any(not effect.startswith("invoke:declared-tool:") and effect != "block:unsafe-release" for effect in self.effect_receipts):
            raise ResearchContractError("throughput retrieval copilot effect invalid")

    def digest(self) -> str:
        self.validate()
        return research_artifact_digest({"schema_version": self.schema_version, "contract_version": self.contract_version, "feature_id": self.feature_id, "request_id": self.request_id, "operator_id": self.operator_id, "batch_id": self.batch_id, "partition": self.partition, "disposition": self.disposition, "plan_order": list(self.plan_order), "action_order": list(self.action_order), "tool_order": list(self.tool_order), "candidate_order": list(self.candidate_order), "qualified_order": list(self.qualified_order), "blocked_order": list(self.blocked_order), "unknown_order": list(self.unknown_order), "checkpoint_seq": self.checkpoint_seq, "queue_digest": self.queue_digest, "synthesis_digest": self.synthesis_digest, "plan_digest": self.plan_digest, "approval_reference": self.approval_reference, "replay_identity": self.replay_identity, "budget_units": self.budget_units, "omissions": list(self.omissions), "uncertainty": list(self.uncertainty), "negative_evidence": list(self.negative_evidence), "effect_receipts": list(self.effect_receipts), "artifact": dict(self.artifact), "raw_data_local": self.raw_data_local, "boundary": self.boundary})
