"""Parity implementation for AFA-adapter-P01-F11."""
from __future__ import annotations

from dataclasses import dataclass
import re
from typing import Any, Sequence

from .research_contracts import (
    ADAPTER_THROUGHPUT_EVIDENCE_SURVEILLANCE_RESEARCH_COPILOT_CONTRACT_VERSION,
    ADAPTER_THROUGHPUT_EVIDENCE_SURVEILLANCE_RESEARCH_COPILOT_FEATURE_ID,
    PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError,
    research_artifact_digest,
)


@dataclass(frozen=True)
class ThroughputCopilotEvidenceObservation:
    source_id: str
    sequence: int
    digest: str | None
    availability: str = "available"
    evidence_state: str = "supported"
    relevance_score: int = 0
    negative_result: bool = False


@dataclass(frozen=True)
class ThroughputEvidenceSurveillanceResearchCopilotReceipt:
    request_id: str; agent_id: str; batch_id: str; checkpoint_seq: int; capacity: int; disposition: str
    candidate_order: tuple[str, ...]; selected_order: tuple[str, ...]; unresolved_order: tuple[str, ...]; denied_order: tuple[str, ...]; overflow_order: tuple[str, ...]
    replay_identity: str; queue_digest: str; checkpoint_digest: str; evidence_digest: str; provenance_digest: str; run_digest: str
    omissions: tuple[str, ...]; uncertainty: tuple[str, ...]; negative_evidence: tuple[str, ...]; tool_receipts: tuple[str, ...]; effect_receipts: tuple[str, ...]
    qualified_set: dict[str, Any]; artifact: dict[str, Any]
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    contract_version: str = ADAPTER_THROUGHPUT_EVIDENCE_SURVEILLANCE_RESEARCH_COPILOT_CONTRACT_VERSION
    feature_id: str = ADAPTER_THROUGHPUT_EVIDENCE_SURVEILLANCE_RESEARCH_COPILOT_FEATURE_ID
    raw_data_local: bool = True; boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION or self.contract_version != ADAPTER_THROUGHPUT_EVIDENCE_SURVEILLANCE_RESEARCH_COPILOT_CONTRACT_VERSION or self.feature_id != ADAPTER_THROUGHPUT_EVIDENCE_SURVEILLANCE_RESEARCH_COPILOT_FEATURE_ID or self.boundary != PRECLINICAL_BOUNDARY or not self.raw_data_local or not self.request_id.strip() or not self.agent_id.strip() or not self.batch_id.strip() or self.checkpoint_seq <= 0 or self.capacity <= 0 or not self.candidate_order or not self.effect_receipts or self.qualified_set.get("batch_id") != self.batch_id or self.qualified_set.get("checkpoint_seq") != self.checkpoint_seq:
            raise ResearchContractError("throughput identity, checkpoint, locality, candidates, effects, or qualified-set linkage is incomplete")
        for values in (self.candidate_order, self.selected_order, self.unresolved_order, self.denied_order, self.overflow_order, self.omissions, self.uncertainty, self.negative_evidence, self.tool_receipts, self.effect_receipts, tuple(self.qualified_set.get("selected_order", ())), tuple(self.qualified_set.get("overflow_order", ())), tuple(self.qualified_set.get("omissions", ())), tuple(self.qualified_set.get("uncertainty", ())), tuple(self.qualified_set.get("negative_order", ()) )):
            if tuple(sorted(set(values))) != values: raise ResearchContractError("throughput ordering is not canonical")
        if set(self.selected_order) | set(self.unresolved_order) | set(self.denied_order) | set(self.overflow_order) != set(self.candidate_order) or tuple(self.qualified_set.get("selected_order", ())) != self.selected_order or tuple(self.qualified_set.get("overflow_order", ())) != self.overflow_order:
            raise ResearchContractError("throughput states do not partition candidates")
        for value in (self.replay_identity, self.queue_digest, self.checkpoint_digest, self.evidence_digest, self.provenance_digest, self.run_digest, self.artifact.get("content_hash")):
            if not isinstance(value, str) or not re.fullmatch(r"[0-9a-f]{64}", value): raise ResearchContractError("throughput digest is invalid")
        if any(not effect.startswith("dry-run:bounded-tool:") and not effect.startswith("invoke:declared-tool:") and effect != "block:unsafe-release" for effect in self.effect_receipts): raise ResearchContractError("throughput effect is outside declared-tool gate")
        if self.disposition == "blocked" and self.effect_receipts != ("block:unsafe-release",): raise ResearchContractError("blocked throughput copilot must be explicitly blocked")


def run_throughput_evidence_surveillance_research_copilot(*, request_id: str, agent_id: str, batch_id: str, checkpoint_seq: int, capacity: int, declared_tools: Sequence[str], requested_tool: str, max_tool_calls: int, dry_run: bool, approval_reference: str | None, approval_granted: bool, observations: Sequence[ThroughputCopilotEvidenceObservation], min_relevance_score: int = 0, policy_allow: bool = True, protected_closure: bool = True, raw_data_local: bool = True, replay_identity: str) -> ThroughputEvidenceSurveillanceResearchCopilotReceipt:
    if not request_id.strip() or not agent_id.strip() or not batch_id.strip() or checkpoint_seq <= 0 or capacity <= 0 or max_tool_calls <= 0 or not declared_tools or requested_tool not in declared_tools or not observations or not raw_data_local or not re.fullmatch(r"[0-9a-f]{64}", replay_identity): raise ResearchContractError("identity, checkpoint, capacity, tools, observations, locality, or replay is invalid")
    ordered = tuple(sorted(observations, key=lambda item: (item.sequence, item.source_id))); ids = tuple(item.source_id for item in ordered)
    if len(set(ids)) != len(ids) or any(not item.source_id.strip() for item in ordered): raise ResearchContractError("observation ids must be unique and non-empty")
    selected: set[str] = set(); unresolved: set[str] = set(); denied: set[str] = set(); overflow: set[str] = set(); omissions: set[str] = set(); uncertainty: set[str] = set(); negative: set[str] = set(); digest_map: dict[str, str] = {}
    for index, item in enumerate(ordered):
        if index >= capacity: overflow.add(item.source_id); omissions.add(f"source:{item.source_id}:capacity-overflow"); continue
        if not policy_allow or not protected_closure: denied.add(item.source_id); omissions.add(f"source:{item.source_id}:policy-or-closure")
        elif item.availability != "available": unresolved.add(item.source_id); omissions.add(f"source:{item.source_id}:availability-{item.availability}")
        elif item.relevance_score < min_relevance_score: unresolved.add(item.source_id); uncertainty.add(f"source:{item.source_id}:relevance-below-threshold")
        elif item.digest is None: unresolved.add(item.source_id); omissions.add(f"source:{item.source_id}:content-digest-missing")
        elif item.evidence_state in {"unknown", "speculative"}: unresolved.add(item.source_id); uncertainty.add(f"source:{item.source_id}:unknown-not-asserted")
        elif item.evidence_state == "contradicted": denied.add(item.source_id); negative.add(f"source:{item.source_id}:contradicted")
        else: selected.add(item.source_id); digest_map[item.source_id] = item.digest; negative.add(f"source:{item.source_id}:negative-result") if item.negative_result else None
    if not policy_allow: omissions.add("control:policy-denied")
    if not protected_closure: omissions.add("control:protected-closure-incomplete")
    approval_missing = not dry_run and (not approval_granted or not (approval_reference or "").strip())
    if approval_missing: omissions.add("control:signed-approval-required")
    disposition = "blocked" if not policy_allow or not protected_closure or not raw_data_local or approval_missing else "unknown" if not selected else "partial" if unresolved or denied or overflow else "completed"
    candidate = ids; selected_order = tuple(sorted(selected)); unresolved_order = tuple(sorted(unresolved)); denied_order = tuple(sorted(denied)); overflow_order = tuple(sorted(overflow)); omissions_order = tuple(sorted(omissions)); uncertainty_order = tuple(sorted(uncertainty)); negative_order = tuple(sorted(negative)); tool_receipts = (f"tool:{requested_tool}:denied",) if disposition == "blocked" else (f"tool:{requested_tool}:dry-run",) if dry_run else (f"tool:{requested_tool}:bounded-call:1/{max_tool_calls}",)
    queue_digest = research_artifact_digest({"batch_id": batch_id, "capacity": capacity, "candidate_order": list(candidate), "checkpoint_seq": checkpoint_seq}); checkpoint_digest = research_artifact_digest({"batch_id": batch_id, "checkpoint_seq": checkpoint_seq, "replay_identity": replay_identity}); evidence_digest = research_artifact_digest({"selected_order": list(selected_order), "unresolved_order": list(unresolved_order), "denied_order": list(denied_order), "overflow_order": list(overflow_order)}); provenance_digest = research_artifact_digest({"request_id": request_id, "agent_id": agent_id, "queue_digest": queue_digest, "checkpoint_digest": checkpoint_digest, "evidence_digest": evidence_digest}); run_digest = research_artifact_digest({"request_id": request_id, "dry_run": dry_run, "tool_receipts": list(tool_receipts), "provenance_digest": provenance_digest})
    qualified = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "set_id": f"qualified-evidence-throughput-copilot:{request_id}", "batch_id": batch_id, "checkpoint_seq": checkpoint_seq, "selected_order": list(selected_order), "selected_digests": [digest_map[item] for item in selected_order], "overflow_order": list(overflow_order), "omissions": list(omissions_order), "uncertainty": list(uncertainty_order), "negative_order": list(negative_order), "evidence_state": "supported" if disposition == "completed" else "unknown", "tool_mode": "dry_run" if dry_run else "bounded_invocation", "boundary": PRECLINICAL_BOUNDARY}; artifact = {"content_hash": research_artifact_digest(qualified), "media_type": "application/vnd.aurora.qualified-evidence-set3+json"}
    receipt = ThroughputEvidenceSurveillanceResearchCopilotReceipt(request_id, agent_id, batch_id, checkpoint_seq, capacity, disposition, candidate, selected_order, unresolved_order, denied_order, overflow_order, replay_identity, queue_digest, checkpoint_digest, evidence_digest, provenance_digest, run_digest, omissions_order, uncertainty_order, negative_order, tool_receipts, ("block:unsafe-release",) if disposition == "blocked" else (f"dry-run:bounded-tool:{agent_id}",) if dry_run else (f"invoke:declared-tool:{agent_id}",), qualified, artifact); receipt.validate(); return receipt
