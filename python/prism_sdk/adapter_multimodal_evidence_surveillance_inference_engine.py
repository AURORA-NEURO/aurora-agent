"""Multimodal multi-study evidence-surveillance inference engine parity surface."""
from __future__ import annotations

from dataclasses import dataclass
import re
from typing import Any, Mapping, Sequence

from .research_contracts import (
    ADAPTER_MULTIMODAL_EVIDENCE_SURVEILLANCE_INFERENCE_ENGINE_CONTRACT_VERSION,
    ADAPTER_MULTIMODAL_EVIDENCE_SURVEILLANCE_INFERENCE_ENGINE_FEATURE_ID,
    PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
    ResearchContractError,
    research_artifact_digest,
)


@dataclass(frozen=True)
class MultimodalEvidenceObservation:
    source_id: str
    study_id: str
    modality: str
    source_type: str
    locator: str
    semantic_profile: str
    digest: str | None
    availability: str = "available"
    evidence_state: str = "supported"
    relevance_score: int = 0
    negative_result: bool = False


@dataclass(frozen=True)
class MultimodalEvidenceSurveillanceReceipt:
    request_id: str
    intent: str
    disposition: str
    study_order: tuple[str, ...]
    modality_order: tuple[str, ...]
    candidate_order: tuple[str, ...]
    ranked_order: tuple[str, ...]
    selected_order: tuple[str, ...]
    unresolved_order: tuple[str, ...]
    denied_order: tuple[str, ...]
    comparability_digest: str
    evidence_digest: str
    provenance_digest: str
    replay_identity: str
    omissions: tuple[str, ...]
    uncertainty: tuple[str, ...]
    negative_evidence: tuple[str, ...]
    effect_receipts: tuple[str, ...]
    qualified_set: Mapping[str, Any]
    artifact: Mapping[str, Any]
    semantic_profile: str
    feature_id: str = ADAPTER_MULTIMODAL_EVIDENCE_SURVEILLANCE_INFERENCE_ENGINE_FEATURE_ID
    contract_version: str = ADAPTER_MULTIMODAL_EVIDENCE_SURVEILLANCE_INFERENCE_ENGINE_CONTRACT_VERSION
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    raw_data_local: bool = True
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION or self.feature_id != ADAPTER_MULTIMODAL_EVIDENCE_SURVEILLANCE_INFERENCE_ENGINE_FEATURE_ID or self.contract_version != ADAPTER_MULTIMODAL_EVIDENCE_SURVEILLANCE_INFERENCE_ENGINE_CONTRACT_VERSION:
            raise ResearchContractError("multimodal evidence schema, feature, or version mismatch")
        if self.boundary != PRECLINICAL_BOUNDARY or not self.raw_data_local or not self.request_id.strip() or not self.intent.strip() or len(self.study_order) < 2 or len(self.modality_order) < 2 or not self.candidate_order or not self.effect_receipts or self.qualified_set.get("intent") != self.intent:
            raise ResearchContractError("multimodal identity, closure, locality, candidates, effects, or qualified-set linkage is incomplete")
        for values in (self.study_order, self.modality_order, self.candidate_order, self.selected_order, self.unresolved_order, self.denied_order, self.omissions, self.uncertainty, self.negative_evidence, self.effect_receipts, tuple(self.qualified_set.get("study_order", ())), tuple(self.qualified_set.get("modality_order", ())), tuple(self.qualified_set.get("selected_order", ())), tuple(self.qualified_set.get("coverage_order", ())), tuple(self.qualified_set.get("omissions", ())), tuple(self.qualified_set.get("uncertainty", ())), tuple(self.qualified_set.get("negative_order", ()))):
            if tuple(sorted(set(values))) != values:
                raise ResearchContractError("multimodal ordering is not canonical")
        if len(self.ranked_order) != len(self.candidate_order) or set(self.ranked_order) != set(self.candidate_order):
            raise ResearchContractError("multimodal ranking must cover candidates exactly")
        classified = set(self.selected_order) | set(self.unresolved_order) | set(self.denied_order)
        if classified != set(self.candidate_order) or tuple(self.qualified_set.get("selected_order", ())) != self.selected_order:
            raise ResearchContractError("multimodal states do not partition candidates")
        for value in (self.comparability_digest, self.evidence_digest, self.provenance_digest, self.replay_identity, self.artifact.get("content_hash")):
            if not isinstance(value, str) or not re.fullmatch(r"[0-9a-f]{64}", value):
                raise ResearchContractError("multimodal digest is invalid")
        if any(not effect.startswith("read:local-multimodal-evidence:") and effect != "block:unsafe-release" for effect in self.effect_receipts):
            raise ResearchContractError("multimodal effect is outside local-read gate")
        if self.disposition == "blocked" and self.effect_receipts != ("block:unsafe-release",):
            raise ResearchContractError("blocked multimodal surveillance must be explicitly blocked")

    def digest(self) -> str:
        self.validate()
        return research_artifact_digest({
            "schema_version": self.schema_version, "contract_version": self.contract_version, "feature_id": self.feature_id,
            "request_id": self.request_id, "intent": self.intent, "disposition": self.disposition,
            "study_order": list(self.study_order), "modality_order": list(self.modality_order), "candidate_order": list(self.candidate_order),
            "ranked_order": list(self.ranked_order), "selected_order": list(self.selected_order), "unresolved_order": list(self.unresolved_order), "denied_order": list(self.denied_order),
            "comparability_digest": self.comparability_digest, "evidence_digest": self.evidence_digest, "provenance_digest": self.provenance_digest, "replay_identity": self.replay_identity,
            "omissions": list(self.omissions), "uncertainty": list(self.uncertainty), "negative_evidence": list(self.negative_evidence), "effect_receipts": list(self.effect_receipts),
            "qualified_set": dict(self.qualified_set), "artifact": dict(self.artifact), "semantic_profile": self.semantic_profile, "raw_data_local": self.raw_data_local, "boundary": self.boundary,
        })


def run_multimodal_evidence_surveillance(*, request_id: str, intent: str, required_studies: Sequence[str], required_modalities: Sequence[str], semantic_profile: str, observations: Sequence[MultimodalEvidenceObservation], min_relevance_score: int = 0, policy_allow: bool = True, protected_closure: bool = True, raw_data_local: bool = True, replay_identity: str) -> MultimodalEvidenceSurveillanceReceipt:
    if not request_id.strip() or not intent.strip() or not semantic_profile.strip() or len(set(required_studies)) < 2 or len(set(required_modalities)) < 2 or not observations or not re.fullmatch(r"[0-9a-f]{64}", replay_identity) or not raw_data_local:
        raise ResearchContractError("multimodal identity, closure, observations, replay, or locality is invalid")
    studies = tuple(sorted(set(required_studies))); modalities = tuple(sorted(set(required_modalities)))
    if "imaging" not in modalities or "omics" not in modalities:
        raise ResearchContractError("multimodal request must declare imaging and omics modalities")
    ordered = tuple(sorted(observations, key=lambda item: (-item.relevance_score, item.study_id, item.modality, item.source_id)))
    make_key = lambda item: f"{item.study_id}::{item.modality}::{item.source_id}"
    ranked = tuple(make_key(item) for item in ordered); candidate = tuple(sorted(ranked))
    if len(set(candidate)) != len(candidate) or any(not value.strip() for value in candidate):
        raise ResearchContractError("multimodal observation keys must be unique and non-empty")
    required_cells = {f"{study}::{modality}" for study in studies for modality in modalities}
    selected: set[str] = set(); unresolved: set[str] = set(); denied: set[str] = set(); digest_map: dict[str, str] = {}; omissions: set[str] = set(); uncertainty: set[str] = set(); negative: set[str] = set(); selected_cells: set[str] = set()
    for item in ordered:
        item_key = make_key(item); cell = f"{item.study_id}::{item.modality}"
        if item.study_id not in studies or item.modality not in modalities or not item.source_type.strip() or not item.locator.strip() or not policy_allow or not protected_closure or not raw_data_local:
            denied.add(item_key); omissions.add(f"evidence:{item_key}:scope-policy-locality")
        elif item.semantic_profile != semantic_profile:
            denied.add(item_key); omissions.add(f"evidence:{item_key}:semantic-profile-mismatch"); negative.add(f"evidence:{item_key}:incomparable")
        elif item.availability != "available":
            unresolved.add(item_key); omissions.add(f"evidence:{item_key}:availability-{item.availability}")
        elif item.relevance_score < min_relevance_score:
            unresolved.add(item_key); uncertainty.add(f"evidence:{item_key}:relevance-below-threshold")
        elif item.digest is None:
            unresolved.add(item_key); omissions.add(f"evidence:{item_key}:content-digest-missing")
        elif item.evidence_state in {"unknown", "speculative"}:
            unresolved.add(item_key); uncertainty.add(f"evidence:{item_key}:unknown-not-asserted")
        elif item.evidence_state == "contradicted":
            denied.add(item_key); negative.add(f"evidence:{item_key}:contradicted")
        else:
            selected.add(item_key); selected_cells.add(cell); digest_map[item_key] = item.digest
            if item.negative_result: negative.add(f"evidence:{item_key}:negative-result")
    for cell in required_cells:
        if cell not in selected_cells:
            omissions.add(f"cell:{cell}:required-modality-study-missing"); uncertainty.add(f"cell:{cell}:comparability-incomplete")
    if not policy_allow: omissions.add("control:policy-denied")
    if not protected_closure: omissions.add("control:protected-closure-incomplete")
    if not raw_data_local: omissions.add("control:raw-data-locality-failed")
    disposition = "blocked" if not policy_allow or not protected_closure or not raw_data_local else "unknown" if not selected else "partial" if unresolved or denied or selected_cells != required_cells else "completed"
    selected_order = tuple(sorted(selected)); unresolved_order = tuple(sorted(unresolved)); denied_order = tuple(sorted(denied)); coverage = tuple(sorted(selected_cells)); omissions_order = tuple(sorted(omissions)); uncertainty_order = tuple(sorted(uncertainty)); negative_order = tuple(sorted(negative))
    comparability_digest = research_artifact_digest({"study_order": list(studies), "modality_order": list(modalities), "semantic_profile": semantic_profile, "coverage_order": list(coverage)})
    evidence_digest = research_artifact_digest({"candidate_order": list(candidate), "ranked_order": list(ranked), "selected_order": list(selected_order), "unresolved_order": list(unresolved_order), "denied_order": list(denied_order)})
    provenance_digest = research_artifact_digest({"request_id": request_id, "replay_identity": replay_identity, "comparability_digest": comparability_digest, "evidence_digest": evidence_digest})
    qualified_set = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "set_id": f"qualified-multimodal-evidence:{request_id}", "intent": intent, "study_order": list(studies), "modality_order": list(modalities), "selected_order": list(selected_order), "selected_digests": [digest_map[item] for item in selected_order], "coverage_order": list(coverage), "omissions": list(omissions_order), "uncertainty": list(uncertainty_order), "negative_order": list(negative_order), "semantic_profile": semantic_profile, "evidence_state": "supported" if disposition == "completed" else "unknown", "boundary": PRECLINICAL_BOUNDARY}
    artifact = {"content_hash": research_artifact_digest(qualified_set), "media_type": "application/vnd.aurora.qualified-multimodal-evidence-set+json"}
    receipt = MultimodalEvidenceSurveillanceReceipt(request_id=request_id, intent=intent, disposition=disposition, study_order=studies, modality_order=modalities, candidate_order=candidate, ranked_order=ranked, selected_order=selected_order, unresolved_order=unresolved_order, denied_order=denied_order, comparability_digest=comparability_digest, evidence_digest=evidence_digest, provenance_digest=provenance_digest, replay_identity=replay_identity, omissions=omissions_order, uncertainty=uncertainty_order, negative_evidence=negative_order, effect_receipts=(f"read:local-multimodal-evidence:{request_id}",) if disposition != "blocked" else ("block:unsafe-release",), qualified_set=qualified_set, artifact=artifact, semantic_profile=semantic_profile, raw_data_local=raw_data_local)
    receipt.validate(); return receipt
