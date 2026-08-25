"""Python parity contract for the prospective throughput researcher workbench."""

from __future__ import annotations

from dataclasses import dataclass
import re
from typing import Any, Mapping

from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError, research_artifact_digest

THROUGHPUT_RESEARCH_WORKBENCH_FEATURE_ID = "AFA-brain-P01-F19"
THROUGHPUT_RESEARCH_WORKBENCH_CONTRACT_VERSION = "brain-throughput-research-workbench/1.0"


@dataclass(frozen=True)
class BrainThroughputResearchWorkbenchReceipt:
    request_id: str
    workspace_id: str
    batch_id: str
    partition: str
    disposition: str
    view_order: tuple[str, ...]
    panel_order: tuple[str, ...]
    action_receipts: tuple[str, ...]
    candidate_order: tuple[str, ...]
    admitted_order: tuple[str, ...]
    blocked_order: tuple[str, ...]
    unknown_order: tuple[str, ...]
    checkpoint_seq: int
    queue_digest: str
    evidence_digest: str
    workbench_digest: str
    replay_identity: str
    budget_units: int
    omissions: tuple[str, ...]
    uncertainty: tuple[str, ...]
    negative_evidence: tuple[str, ...]
    effect_receipts: tuple[str, ...]
    artifact: Mapping[str, Any]
    feature_id: str = THROUGHPUT_RESEARCH_WORKBENCH_FEATURE_ID
    contract_version: str = THROUGHPUT_RESEARCH_WORKBENCH_CONTRACT_VERSION
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    raw_data_local: bool = True
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION or self.feature_id != THROUGHPUT_RESEARCH_WORKBENCH_FEATURE_ID or self.contract_version != THROUGHPUT_RESEARCH_WORKBENCH_CONTRACT_VERSION:
            raise ResearchContractError("throughput workbench schema, feature, or version mismatch")
        if self.boundary != PRECLINICAL_BOUNDARY or not self.raw_data_local or not self.request_id.strip() or not self.workspace_id.strip() or not self.batch_id.strip() or not self.partition.strip() or not self.view_order or not self.panel_order or not self.action_receipts or not self.candidate_order or not self.effect_receipts or self.budget_units <= 0:
            raise ResearchContractError("throughput workbench identity, queue views, evidence, locality, budget, or effects are incomplete")
        if any(value not in self.candidate_order for value in (*self.admitted_order, *self.blocked_order, *self.unknown_order)):
            raise ResearchContractError("throughput workbench state is not covered by candidates")
        for values in (self.view_order, self.panel_order, self.action_receipts, self.candidate_order, self.admitted_order, self.blocked_order, self.unknown_order, self.omissions, self.uncertainty, self.negative_evidence, self.effect_receipts):
            if tuple(sorted(set(values))) != values:
                raise ResearchContractError("throughput workbench ordering is invalid")
        for value in (self.queue_digest, self.evidence_digest, self.workbench_digest, self.replay_identity, self.artifact.get("content_hash")):
            if not isinstance(value, str) or not re.fullmatch(r"[0-9a-f]{64}", value):
                raise ResearchContractError("throughput workbench digest is invalid")
        if any(not effect.startswith("view:local-throughput-artifacts:") and effect != "block:unsafe-release" for effect in self.effect_receipts):
            raise ResearchContractError("throughput workbench effect is not read-only")

    def digest(self) -> str:
        self.validate()
        return research_artifact_digest({"schema_version": self.schema_version, "contract_version": self.contract_version, "feature_id": self.feature_id, "request_id": self.request_id, "workspace_id": self.workspace_id, "batch_id": self.batch_id, "partition": self.partition, "disposition": self.disposition, "view_order": list(self.view_order), "panel_order": list(self.panel_order), "action_receipts": list(self.action_receipts), "candidate_order": list(self.candidate_order), "admitted_order": list(self.admitted_order), "blocked_order": list(self.blocked_order), "unknown_order": list(self.unknown_order), "checkpoint_seq": self.checkpoint_seq, "queue_digest": self.queue_digest, "evidence_digest": self.evidence_digest, "workbench_digest": self.workbench_digest, "replay_identity": self.replay_identity, "budget_units": self.budget_units, "omissions": list(self.omissions), "uncertainty": list(self.uncertainty), "negative_evidence": list(self.negative_evidence), "effect_receipts": list(self.effect_receipts), "artifact": dict(self.artifact), "raw_data_local": self.raw_data_local, "boundary": self.boundary})
