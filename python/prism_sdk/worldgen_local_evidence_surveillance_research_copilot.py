"""Bounded local evidence-surveillance research copilot parity surface."""
from __future__ import annotations

from dataclasses import dataclass
import re
from typing import Any, Mapping, Sequence

from .research_contracts import (
    WORLDGEN_LOCAL_EVIDENCE_SURVEILLANCE_RESEARCH_COPILOT_CONTRACT_VERSION,
    WORLDGEN_LOCAL_EVIDENCE_SURVEILLANCE_RESEARCH_COPILOT_FEATURE_ID,
    PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
    ResearchContractError,
    research_artifact_digest,
)


@dataclass(frozen=True)
class CopilotEvidenceObservation:
    source_id: str
    study_id: str
    source_type: str
    locator: str
    digest: str | None
    availability: str = "available"
    evidence_state: str = "supported"
    relevance_score: int = 0
    negative_result: bool = False


@dataclass(frozen=True)
class LocalEvidenceSurveillanceResearchCopilotReceipt:
    request_id: str
    agent_id: str
    study_id: str
    intent: str
    dry_run: bool
    requested_tool: str
    disposition: str
    candidate_order: tuple[str, ...]
    selected_order: tuple[str, ...]
    unresolved_order: tuple[str, ...]
    denied_order: tuple[str, ...]
    replay_identity: str
    capability_digest: str
    evidence_digest: str
    provenance_digest: str
    run_digest: str
    omissions: tuple[str, ...]
    uncertainty: tuple[str, ...]
    negative_evidence: tuple[str, ...]
    tool_receipts: tuple[str, ...]
    effect_receipts: tuple[str, ...]
    qualified_set: Mapping[str, Any]
    artifact: Mapping[str, Any]
    feature_id: str = WORLDGEN_LOCAL_EVIDENCE_SURVEILLANCE_RESEARCH_COPILOT_FEATURE_ID
    contract_version: str = WORLDGEN_LOCAL_EVIDENCE_SURVEILLANCE_RESEARCH_COPILOT_CONTRACT_VERSION
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    raw_data_local: bool = True
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION or self.contract_version != WORLDGEN_LOCAL_EVIDENCE_SURVEILLANCE_RESEARCH_COPILOT_CONTRACT_VERSION or self.feature_id != WORLDGEN_LOCAL_EVIDENCE_SURVEILLANCE_RESEARCH_COPILOT_FEATURE_ID:
            raise ResearchContractError("copilot schema, feature, or version mismatch")
        if self.boundary != PRECLINICAL_BOUNDARY or not self.raw_data_local or not self.request_id.strip() or not self.agent_id.strip() or not self.study_id.strip() or not self.intent.strip() or not self.requested_tool.strip() or not self.candidate_order or not self.effect_receipts or self.qualified_set.get("study_id") != self.study_id or self.qualified_set.get("intent") != self.intent:
            raise ResearchContractError("copilot identity, locality, candidates, effects, or qualified-set linkage is incomplete")
        for values in (self.candidate_order, self.selected_order, self.unresolved_order, self.denied_order, self.omissions, self.uncertainty, self.negative_evidence, self.tool_receipts, self.effect_receipts, tuple(self.qualified_set.get("selected_order", ())), tuple(self.qualified_set.get("negative_order", ())), tuple(self.qualified_set.get("omissions", ())), tuple(self.qualified_set.get("uncertainty", ()) )):
            if tuple(sorted(set(values))) != values:
                raise ResearchContractError("copilot ordering is not canonical")
        if set(self.selected_order) | set(self.unresolved_order) | set(self.denied_order) != set(self.candidate_order) or tuple(self.qualified_set.get("selected_order", ())) != self.selected_order:
            raise ResearchContractError("copilot states do not partition candidates")
        for value in (self.replay_identity, self.capability_digest, self.evidence_digest, self.provenance_digest, self.run_digest, self.artifact.get("content_hash")):
            if not isinstance(value, str) or not re.fullmatch(r"[0-9a-f]{64}", value):
                raise ResearchContractError("copilot digest is invalid")
        if any(not effect.startswith("dry-run:bounded-tool:") and not effect.startswith("invoke:declared-tool:") and effect != "block:unsafe-release" for effect in self.effect_receipts):
            raise ResearchContractError("copilot effect is outside declared-tool gate")
        if self.disposition == "blocked" and self.effect_receipts != ("block:unsafe-release",):
            raise ResearchContractError("blocked copilot must be explicitly blocked")
        if self.dry_run and any(effect.startswith("invoke:") for effect in self.effect_receipts):
            raise ResearchContractError("dry-run copilot cannot invoke tools")

    def digest(self) -> str:
        self.validate()
        return research_artifact_digest({"schema_version": self.schema_version, "contract_version": self.contract_version, "feature_id": self.feature_id, "request_id": self.request_id, "agent_id": self.agent_id, "study_id": self.study_id, "intent": self.intent, "dry_run": self.dry_run, "requested_tool": self.requested_tool, "disposition": self.disposition, "candidate_order": list(self.candidate_order), "selected_order": list(self.selected_order), "unresolved_order": list(self.unresolved_order), "denied_order": list(self.denied_order), "replay_identity": self.replay_identity, "capability_digest": self.capability_digest, "evidence_digest": self.evidence_digest, "provenance_digest": self.provenance_digest, "run_digest": self.run_digest, "omissions": list(self.omissions), "uncertainty": list(self.uncertainty), "negative_evidence": list(self.negative_evidence), "tool_receipts": list(self.tool_receipts), "effect_receipts": list(self.effect_receipts), "qualified_set": dict(self.qualified_set), "artifact": dict(self.artifact), "raw_data_local": self.raw_data_local, "boundary": self.boundary})


def run_local_evidence_surveillance_research_copilot(*, request_id: str, agent_id: str, study_id: str, intent: str, declared_tools: Sequence[str], requested_tool: str, max_tool_calls: int, dry_run: bool, required_source_ids: Sequence[str], observations: Sequence[CopilotEvidenceObservation], min_relevance_score: int = 0, policy_allow: bool = True, protected_closure: bool = True, raw_data_local: bool = True, replay_identity: str) -> LocalEvidenceSurveillanceResearchCopilotReceipt:
    if not request_id.strip() or not agent_id.strip() or not study_id.strip() or not intent.strip() or not declared_tools or not requested_tool.strip() or max_tool_calls <= 0 or not observations or not re.fullmatch(r"[0-9a-f]{64}", replay_identity) or not raw_data_local:
        raise ResearchContractError("copilot identity, tools, observations, replay, locality, or boundary is invalid")
    if len(set(declared_tools)) != len(declared_tools) or any(not tool.strip() for tool in declared_tools) or requested_tool not in declared_tools:
        raise ResearchContractError("requested tool must be declared exactly once")
    ordered = tuple(sorted(observations, key=lambda item: (-item.relevance_score, item.source_id))); candidate = tuple(item.source_id for item in ordered)
    if len(set(candidate)) != len(candidate) or any(not value.strip() for value in candidate):
        raise ResearchContractError("observation source identities must be unique and non-empty")
    selected: set[str] = set(); digest_map: dict[str, str] = {}; unresolved: set[str] = set(); denied: set[str] = set(); omissions: set[str] = set(); uncertainty: set[str] = set(); negative: set[str] = set()
    for item in ordered:
        if item.study_id != study_id or not item.locator.strip() or not item.source_type.strip() or not policy_allow or not protected_closure:
            denied.add(item.source_id); omissions.add(f"source:{item.source_id}:scope-policy-closure")
        elif item.availability != "available":
            unresolved.add(item.source_id); omissions.add(f"source:{item.source_id}:availability-{item.availability}")
        elif item.relevance_score < min_relevance_score:
            unresolved.add(item.source_id); uncertainty.add(f"source:{item.source_id}:relevance-below-threshold")
        elif item.digest is None:
            unresolved.add(item.source_id); omissions.add(f"source:{item.source_id}:content-digest-missing")
        elif item.evidence_state in {"unknown", "speculative"}:
            unresolved.add(item.source_id); uncertainty.add(f"source:{item.source_id}:unknown-not-asserted")
        elif item.evidence_state == "contradicted":
            denied.add(item.source_id); negative.add(f"source:{item.source_id}:contradicted")
        else:
            selected.add(item.source_id); digest_map[item.source_id] = item.digest
            if item.negative_result: negative.add(f"source:{item.source_id}:negative-result")
    for required in sorted(set(required_source_ids)):
        if required not in selected: omissions.add(f"source:{required}:required-not-qualified"); uncertainty.add(f"source:{required}:required-unresolved")
    if not policy_allow: omissions.add("control:policy-denied")
    if not protected_closure: omissions.add("control:protected-closure-incomplete")
    disposition = "blocked" if not policy_allow or not protected_closure or not raw_data_local else "unknown" if not selected else "partial" if unresolved or denied or any(required not in selected for required in required_source_ids) else "completed"
    selected_order = tuple(sorted(selected)); unresolved_order = tuple(sorted(unresolved)); denied_order = tuple(sorted(denied)); omissions_order = tuple(sorted(omissions)); uncertainty_order = tuple(sorted(uncertainty)); negative_order = tuple(sorted(negative)); tool_receipts = (f"tool:{requested_tool}:denied",) if disposition == "blocked" else (f"tool:{requested_tool}:dry-run",) if dry_run else (f"tool:{requested_tool}:bounded-call:1/{max_tool_calls}",)
    capability_digest = research_artifact_digest({"agent_id": agent_id, "declared_tools": list(declared_tools), "requested_tool": requested_tool, "max_tool_calls": max_tool_calls, "dry_run": dry_run}); evidence_digest = research_artifact_digest({"candidate_order": list(candidate), "selected_order": list(selected_order), "unresolved_order": list(unresolved_order), "denied_order": list(denied_order)}); provenance_digest = research_artifact_digest({"request_id": request_id, "agent_id": agent_id, "replay_identity": replay_identity, "capability_digest": capability_digest, "evidence_digest": evidence_digest}); run_digest = research_artifact_digest({"request_id": request_id, "dry_run": dry_run, "tool_receipts": list(tool_receipts), "provenance_digest": provenance_digest})
    qualified_set = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "set_id": f"worldgen-qualified-evidence-copilot:{request_id}", "study_id": study_id, "intent": intent, "selected_order": list(selected_order), "selected_digests": [digest_map[source] for source in selected_order], "negative_order": list(negative_order), "omissions": list(omissions_order), "uncertainty": list(uncertainty_order), "evidence_state": "supported" if disposition == "completed" else "unknown", "ordering_rule": "relevance_score descending, source_id ascending; artifact digests ascending", "tool_mode": "dry_run" if dry_run else "bounded_invocation", "boundary": PRECLINICAL_BOUNDARY}; artifact = {"content_hash": research_artifact_digest(qualified_set), "media_type": "application/vnd.aurora.worldgen.qualified-evidence-set3+json"}
    receipt = LocalEvidenceSurveillanceResearchCopilotReceipt(request_id=request_id, agent_id=agent_id, study_id=study_id, intent=intent, dry_run=dry_run, requested_tool=requested_tool, disposition=disposition, candidate_order=candidate, selected_order=selected_order, unresolved_order=unresolved_order, denied_order=denied_order, replay_identity=replay_identity, capability_digest=capability_digest, evidence_digest=evidence_digest, provenance_digest=provenance_digest, run_digest=run_digest, omissions=omissions_order, uncertainty=uncertainty_order, negative_evidence=negative_order, tool_receipts=tool_receipts, effect_receipts=("block:unsafe-release",) if disposition == "blocked" else (f"dry-run:bounded-tool:{agent_id}",) if dry_run else (f"invoke:declared-tool:{agent_id}",), qualified_set=qualified_set, artifact=artifact, raw_data_local=raw_data_local); receipt.validate(); return receipt
