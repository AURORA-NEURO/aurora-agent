"""Multimodal multi-study evidence-surveillance copilot parity surface."""
from __future__ import annotations

from dataclasses import dataclass
import re
from typing import Any, Mapping, Sequence

from .research_contracts import (
    WORLDGEN_MULTIMODAL_EVIDENCE_SURVEILLANCE_RESEARCH_COPILOT_CONTRACT_VERSION,
    WORLDGEN_MULTIMODAL_EVIDENCE_SURVEILLANCE_RESEARCH_COPILOT_FEATURE_ID,
    PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
    ResearchContractError,
    research_artifact_digest,
)


@dataclass(frozen=True)
class MultimodalCopilotEvidenceObservation:
    source_id: str
    study_id: str
    modality: str
    semantic_profile: str
    source_type: str
    locator: str
    digest: str | None
    availability: str = "available"
    evidence_state: str = "supported"
    relevance_score: int = 0
    negative_result: bool = False


@dataclass(frozen=True)
class MultimodalEvidenceSurveillanceResearchCopilotReceipt:
    request_id: str
    agent_id: str
    semantic_profile: str
    dry_run: bool
    approval_granted: bool
    requested_tool: str
    disposition: str
    study_order: tuple[str, ...]
    modality_order: tuple[str, ...]
    candidate_order: tuple[str, ...]
    selected_order: tuple[str, ...]
    unresolved_order: tuple[str, ...]
    denied_order: tuple[str, ...]
    incomparable_order: tuple[str, ...]
    missing_cell_order: tuple[str, ...]
    replay_identity: str
    capability_digest: str
    comparability_digest: str
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
    feature_id: str = WORLDGEN_MULTIMODAL_EVIDENCE_SURVEILLANCE_RESEARCH_COPILOT_FEATURE_ID
    contract_version: str = WORLDGEN_MULTIMODAL_EVIDENCE_SURVEILLANCE_RESEARCH_COPILOT_CONTRACT_VERSION
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    raw_data_local: bool = True
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION or self.contract_version != WORLDGEN_MULTIMODAL_EVIDENCE_SURVEILLANCE_RESEARCH_COPILOT_CONTRACT_VERSION or self.feature_id != WORLDGEN_MULTIMODAL_EVIDENCE_SURVEILLANCE_RESEARCH_COPILOT_FEATURE_ID:
            raise ResearchContractError("multimodal copilot schema, feature, or version mismatch")
        if self.boundary != PRECLINICAL_BOUNDARY or not self.raw_data_local or not self.request_id.strip() or not self.agent_id.strip() or not self.semantic_profile.strip() or not self.requested_tool.strip() or len(self.study_order) < 2 or len(self.modality_order) < 2 or not self.candidate_order or not self.effect_receipts or self.qualified_set.get("semantic_profile") != self.semantic_profile:
            raise ResearchContractError("multimodal copilot identity, closure, locality, candidates, effects, or qualified-set linkage is incomplete")
        for values in (self.study_order, self.modality_order, self.candidate_order, self.selected_order, self.unresolved_order, self.denied_order, self.incomparable_order, self.missing_cell_order, self.omissions, self.uncertainty, self.negative_evidence, self.tool_receipts, self.effect_receipts, tuple(self.qualified_set.get("study_order", ())), tuple(self.qualified_set.get("modality_order", ())), tuple(self.qualified_set.get("selected_order", ())), tuple(self.qualified_set.get("incomparable_order", ())), tuple(self.qualified_set.get("missing_cell_order", ())), tuple(self.qualified_set.get("negative_order", ())), tuple(self.qualified_set.get("omissions", ())), tuple(self.qualified_set.get("uncertainty", ()))):
            if tuple(sorted(set(values))) != values:
                raise ResearchContractError("multimodal copilot ordering is not canonical")
        if set(self.selected_order) | set(self.unresolved_order) | set(self.denied_order) | set(self.missing_cell_order) != set(self.candidate_order) or any(value not in self.denied_order for value in self.incomparable_order) or tuple(self.qualified_set.get("selected_order", ())) != self.selected_order:
            raise ResearchContractError("multimodal copilot states do not partition candidates")
        for value in (self.replay_identity, self.capability_digest, self.comparability_digest, self.evidence_digest, self.provenance_digest, self.run_digest, self.artifact.get("content_hash")):
            if not isinstance(value, str) or not re.fullmatch(r"[0-9a-f]{64}", value):
                raise ResearchContractError("multimodal copilot digest is invalid")
        if any(not effect.startswith("dry-run:bounded-tool:") and not effect.startswith("invoke:declared-tool:") and effect != "block:unsafe-release" for effect in self.effect_receipts):
            raise ResearchContractError("multimodal copilot effect is outside declared-tool gate")
        if self.disposition == "blocked" and self.effect_receipts != ("block:unsafe-release",):
            raise ResearchContractError("blocked multimodal copilot must be explicitly blocked")
        if self.dry_run and any(effect.startswith("invoke:") for effect in self.effect_receipts):
            raise ResearchContractError("dry-run multimodal copilot cannot invoke tools")

    def digest(self) -> str:
        self.validate()
        return research_artifact_digest({"schema_version": self.schema_version, "contract_version": self.contract_version, "feature_id": self.feature_id, "request_id": self.request_id, "agent_id": self.agent_id, "semantic_profile": self.semantic_profile, "dry_run": self.dry_run, "approval_granted": self.approval_granted, "requested_tool": self.requested_tool, "disposition": self.disposition, "study_order": list(self.study_order), "modality_order": list(self.modality_order), "candidate_order": list(self.candidate_order), "selected_order": list(self.selected_order), "unresolved_order": list(self.unresolved_order), "denied_order": list(self.denied_order), "incomparable_order": list(self.incomparable_order), "missing_cell_order": list(self.missing_cell_order), "replay_identity": self.replay_identity, "capability_digest": self.capability_digest, "comparability_digest": self.comparability_digest, "evidence_digest": self.evidence_digest, "provenance_digest": self.provenance_digest, "run_digest": self.run_digest, "omissions": list(self.omissions), "uncertainty": list(self.uncertainty), "negative_evidence": list(self.negative_evidence), "tool_receipts": list(self.tool_receipts), "effect_receipts": list(self.effect_receipts), "qualified_set": dict(self.qualified_set), "artifact": dict(self.artifact), "raw_data_local": self.raw_data_local, "boundary": self.boundary})


def run_multimodal_evidence_surveillance_research_copilot(*, request_id: str, agent_id: str, semantic_profile: str, required_studies: Sequence[str], required_modalities: Sequence[str], declared_tools: Sequence[str], requested_tool: str, max_tool_calls: int, dry_run: bool, approval_reference: str | None, approval_granted: bool, observations: Sequence[MultimodalCopilotEvidenceObservation], min_relevance_score: int = 0, policy_allow: bool = True, protected_closure: bool = True, raw_data_local: bool = True, replay_identity: str) -> MultimodalEvidenceSurveillanceResearchCopilotReceipt:
    if not request_id.strip() or not agent_id.strip() or not semantic_profile.strip() or len(set(required_studies)) < 2 or len(set(required_modalities)) < 2 or not declared_tools or not requested_tool.strip() or max_tool_calls <= 0 or not observations or not re.fullmatch(r"[0-9a-f]{64}", replay_identity) or not raw_data_local:
        raise ResearchContractError("multimodal copilot identity, closure, tools, observations, replay, locality, or boundary is invalid")
    if len(set(declared_tools)) != len(declared_tools) or any(not tool.strip() for tool in declared_tools) or requested_tool not in declared_tools:
        raise ResearchContractError("requested tool must be declared exactly once")
    studies = tuple(sorted(set(required_studies))); modalities = tuple(sorted(set(required_modalities))); ordered = tuple(sorted(observations, key=lambda item: (-item.relevance_score, item.source_id))); observation_ids = tuple(item.source_id for item in ordered)
    if len(set(observation_ids)) != len(observation_ids) or any(not item.source_id.strip() or not item.study_id.strip() or not item.modality.strip() for item in ordered):
        raise ResearchContractError("multimodal observation identities must be unique and non-empty")
    required_cells = {f"{study}::{modality}::required" for study in studies for modality in modalities}; selected: set[str] = set(); digest_map: dict[str, str] = {}; unresolved: set[str] = set(); denied: set[str] = set(); incomparable: set[str] = set(); missing_cells = set(required_cells); omissions: set[str] = set(); uncertainty: set[str] = set(); negative: set[str] = set()
    for item in ordered:
        key = item.source_id
        if item.study_id not in studies or item.modality not in modalities or not item.locator.strip() or not item.source_type.strip() or not policy_allow or not protected_closure:
            denied.add(key); omissions.add(f"source:{key}:scope-policy-closure")
        elif item.semantic_profile != semantic_profile:
            denied.add(key); incomparable.add(key); omissions.add(f"source:{key}:semantic-profile-mismatch")
        elif item.availability != "available":
            unresolved.add(key); omissions.add(f"source:{key}:availability-{item.availability}")
        elif item.relevance_score < min_relevance_score:
            unresolved.add(key); uncertainty.add(f"source:{key}:relevance-below-threshold")
        elif item.digest is None:
            unresolved.add(key); omissions.add(f"source:{key}:content-digest-missing")
        elif item.evidence_state in {"unknown", "speculative"}:
            unresolved.add(key); uncertainty.add(f"source:{key}:unknown-not-asserted")
        elif item.evidence_state == "contradicted":
            denied.add(key); negative.add(f"source:{key}:contradicted")
        else:
            selected.add(key); digest_map[key] = item.digest; missing_cells.discard(f"{item.study_id}::{item.modality}::required")
            if item.negative_result: negative.add(f"source:{key}:negative-result")
    for cell in sorted(missing_cells):
        unresolved.add(cell); omissions.add(f"cell:{cell}:required-not-qualified"); uncertainty.add(f"cell:{cell}:missing-modality")
    # Satisfied cells are represented by their selected source; retain only
    # unresolved cells in the candidate partition.
    candidate = tuple(sorted(set(observation_ids) | missing_cells))
    if not policy_allow: omissions.add("control:policy-denied")
    if not protected_closure: omissions.add("control:protected-closure-incomplete")
    approval_missing = not dry_run and (not approval_granted or not (approval_reference or "").strip())
    if approval_missing: omissions.add("control:signed-approval-required")
    disposition = "blocked" if not policy_allow or not protected_closure or not raw_data_local or approval_missing else "unknown" if not selected else "partial" if unresolved or denied or missing_cells else "completed"
    study_order = studies; modality_order = modalities; selected_order = tuple(sorted(selected)); unresolved_order = tuple(sorted(unresolved)); denied_order = tuple(sorted(denied)); incomparable_order = tuple(sorted(incomparable)); missing_order = tuple(sorted(missing_cells)); omissions_order = tuple(sorted(omissions)); uncertainty_order = tuple(sorted(uncertainty)); negative_order = tuple(sorted(negative)); tool_receipts = (f"tool:{requested_tool}:denied",) if disposition == "blocked" else (f"tool:{requested_tool}:dry-run",) if dry_run else (f"tool:{requested_tool}:bounded-call:1/{max_tool_calls}",)
    capability_digest = research_artifact_digest({"agent_id": agent_id, "declared_tools": list(declared_tools), "requested_tool": requested_tool, "max_tool_calls": max_tool_calls, "dry_run": dry_run}); comparability_digest = research_artifact_digest({"semantic_profile": semantic_profile, "study_order": list(study_order), "modality_order": list(modality_order), "incomparable_order": list(incomparable_order)}); evidence_digest = research_artifact_digest({"candidate_order": list(candidate), "selected_order": list(selected_order), "unresolved_order": list(unresolved_order), "denied_order": list(denied_order), "missing_cell_order": list(missing_order)}); provenance_digest = research_artifact_digest({"request_id": request_id, "agent_id": agent_id, "replay_identity": replay_identity, "capability_digest": capability_digest, "comparability_digest": comparability_digest, "evidence_digest": evidence_digest}); run_digest = research_artifact_digest({"request_id": request_id, "dry_run": dry_run, "approval_reference": approval_reference, "tool_receipts": list(tool_receipts), "provenance_digest": provenance_digest})
    qualified_set = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "set_id": f"worldgen-qualified-evidence-multimodal-copilot:{request_id}", "semantic_profile": semantic_profile, "study_order": list(study_order), "modality_order": list(modality_order), "selected_order": list(selected_order), "selected_digests": [digest_map[source] for source in selected_order], "incomparable_order": list(incomparable_order), "missing_cell_order": list(missing_order), "negative_order": list(negative_order), "omissions": list(omissions_order), "uncertainty": list(uncertainty_order), "evidence_state": "supported" if disposition == "completed" else "unknown", "tool_mode": "dry_run" if dry_run else "bounded_invocation", "boundary": PRECLINICAL_BOUNDARY}; artifact = {"content_hash": research_artifact_digest(qualified_set), "media_type": "application/vnd.aurora.worldgen.qualified-evidence-set3+json"}
    receipt = MultimodalEvidenceSurveillanceResearchCopilotReceipt(request_id=request_id, agent_id=agent_id, semantic_profile=semantic_profile, dry_run=dry_run, approval_granted=approval_granted, requested_tool=requested_tool, disposition=disposition, study_order=study_order, modality_order=modality_order, candidate_order=candidate, selected_order=selected_order, unresolved_order=unresolved_order, denied_order=denied_order, incomparable_order=incomparable_order, missing_cell_order=missing_order, replay_identity=replay_identity, capability_digest=capability_digest, comparability_digest=comparability_digest, evidence_digest=evidence_digest, provenance_digest=provenance_digest, run_digest=run_digest, omissions=omissions_order, uncertainty=uncertainty_order, negative_evidence=negative_order, tool_receipts=tool_receipts, effect_receipts=("block:unsafe-release",) if disposition == "blocked" else (f"dry-run:bounded-tool:{agent_id}",) if dry_run else (f"invoke:declared-tool:{agent_id}",), qualified_set=qualified_set, artifact=artifact, raw_data_local=raw_data_local); receipt.validate(); return receipt
