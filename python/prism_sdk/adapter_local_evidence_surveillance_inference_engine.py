"""Local single-study evidence-surveillance inference engine parity surface."""
from __future__ import annotations

from dataclasses import dataclass
import re
from typing import Any, Mapping, Sequence

from .research_contracts import (
    ADAPTER_LOCAL_EVIDENCE_SURVEILLANCE_INFERENCE_ENGINE_CONTRACT_VERSION,
    ADAPTER_LOCAL_EVIDENCE_SURVEILLANCE_INFERENCE_ENGINE_FEATURE_ID,
    PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
    ResearchContractError,
    research_artifact_digest,
)

@dataclass(frozen=True)
class LocalEvidenceObservation:
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
class LocalEvidenceSurveillanceReceipt:
    request_id: str
    study_id: str
    intent: str
    disposition: str
    candidate_order: tuple[str, ...]
    selected_order: tuple[str, ...]
    unresolved_order: tuple[str, ...]
    denied_order: tuple[str, ...]
    replay_identity: str
    evidence_digest: str
    provenance_digest: str
    omissions: tuple[str, ...]
    uncertainty: tuple[str, ...]
    negative_evidence: tuple[str, ...]
    effect_receipts: tuple[str, ...]
    qualified_set: Mapping[str, Any]
    artifact: Mapping[str, Any]
    feature_id: str = ADAPTER_LOCAL_EVIDENCE_SURVEILLANCE_INFERENCE_ENGINE_FEATURE_ID
    contract_version: str = ADAPTER_LOCAL_EVIDENCE_SURVEILLANCE_INFERENCE_ENGINE_CONTRACT_VERSION
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    raw_data_local: bool = True
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION or self.feature_id != ADAPTER_LOCAL_EVIDENCE_SURVEILLANCE_INFERENCE_ENGINE_FEATURE_ID or self.contract_version != ADAPTER_LOCAL_EVIDENCE_SURVEILLANCE_INFERENCE_ENGINE_CONTRACT_VERSION:
            raise ResearchContractError("local evidence surveillance schema, feature, or version mismatch")
        if self.boundary != PRECLINICAL_BOUNDARY or not self.raw_data_local or not self.request_id.strip() or not self.study_id.strip() or not self.intent.strip() or not self.candidate_order or not self.effect_receipts or self.qualified_set.get("study_id") != self.study_id or self.qualified_set.get("intent") != self.intent:
            raise ResearchContractError("local surveillance identity, locality, candidates, effects, or qualified-set linkage is incomplete")
        for values in (self.candidate_order, self.selected_order, self.unresolved_order, self.denied_order, self.omissions, self.uncertainty, self.negative_evidence, self.effect_receipts, tuple(self.qualified_set.get("selected_order", ())), tuple(self.qualified_set.get("negative_order", ())), tuple(self.qualified_set.get("omissions", ())), tuple(self.qualified_set.get("uncertainty", ()) )):
            if tuple(sorted(set(values))) != values:
                raise ResearchContractError("local surveillance ordering is not canonical")
        classified = set(self.selected_order) | set(self.unresolved_order) | set(self.denied_order)
        if classified != set(self.candidate_order) or tuple(self.qualified_set.get("selected_order", ())) != self.selected_order:
            raise ResearchContractError("local surveillance states do not partition candidates")
        for value in (self.replay_identity, self.evidence_digest, self.provenance_digest, self.artifact.get("content_hash")):
            if not isinstance(value, str) or not re.fullmatch(r"[0-9a-f]{64}", value):
                raise ResearchContractError("local surveillance digest is invalid")
        if any(not effect.startswith("read:local-evidence-surveillance:") and effect != "block:unsafe-release" for effect in self.effect_receipts):
            raise ResearchContractError("local surveillance effect is outside read-only gate")

    def digest(self) -> str:
        self.validate()
        return research_artifact_digest({"schema_version": self.schema_version, "contract_version": self.contract_version, "feature_id": self.feature_id, "request_id": self.request_id, "study_id": self.study_id, "intent": self.intent, "disposition": self.disposition, "candidate_order": list(self.candidate_order), "selected_order": list(self.selected_order), "unresolved_order": list(self.unresolved_order), "denied_order": list(self.denied_order), "replay_identity": self.replay_identity, "evidence_digest": self.evidence_digest, "provenance_digest": self.provenance_digest, "omissions": list(self.omissions), "uncertainty": list(self.uncertainty), "negative_evidence": list(self.negative_evidence), "effect_receipts": list(self.effect_receipts), "qualified_set": dict(self.qualified_set), "artifact": dict(self.artifact), "raw_data_local": self.raw_data_local, "boundary": self.boundary})

def run_local_evidence_surveillance(*, request_id: str, study_id: str, intent: str, required_source_ids: Sequence[str], observations: Sequence[LocalEvidenceObservation], min_relevance_score: int = 0, policy_allow: bool = True, protected_closure: bool = True, raw_data_local: bool = True, replay_identity: str) -> LocalEvidenceSurveillanceReceipt:
    if not request_id.strip() or not study_id.strip() or not intent.strip() or not observations or not re.fullmatch(r"[0-9a-f]{64}", replay_identity) or not raw_data_local:
        raise ResearchContractError("local surveillance identity, observations, replay, locality, or boundary is invalid")
    ordered_observations = tuple(sorted(observations, key=lambda item: (-item.relevance_score, item.source_id)))
    candidate = tuple(item.source_id for item in ordered_observations)
    if len(set(candidate)) != len(candidate) or any(not value.strip() for value in candidate):
        raise ResearchContractError("observation source identities must be unique and non-empty")
    selected: set[str] = set(); digest_map: dict[str, str] = {}; unresolved: set[str] = set(); denied: set[str] = set(); omissions: set[str] = set(); uncertainty: set[str] = set(); negative: set[str] = set()
    for item in ordered_observations:
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
        if required not in selected:
            omissions.add(f"source:{required}:required-not-qualified"); uncertainty.add(f"source:{required}:required-unresolved")
    if not policy_allow: omissions.add("control:policy-denied")
    if not protected_closure: omissions.add("control:protected-closure-incomplete")
    selected_order = tuple(sorted(selected)); disposition = "blocked" if not policy_allow or not protected_closure or not raw_data_local else "unknown" if not selected else "partial" if unresolved or denied or any(required not in selected for required in required_source_ids) else "completed"
    evidence_digest = research_artifact_digest({"candidate_order": list(candidate), "selected_order": list(selected_order), "unresolved_order": sorted(unresolved), "denied_order": sorted(denied)})
    provenance_digest = research_artifact_digest({"request_id": request_id, "study_id": study_id, "replay_identity": replay_identity, "evidence_digest": evidence_digest})
    qualified_set = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "set_id": f"qualified-evidence-local:{request_id}", "study_id": study_id, "intent": intent, "selected_order": list(selected_order), "selected_digests": [digest_map[source] for source in selected_order], "negative_order": sorted(negative), "omissions": sorted(omissions), "uncertainty": sorted(uncertainty), "evidence_state": "supported" if disposition == "completed" else "unknown", "ordering_rule": "relevance_score descending, source_id ascending; artifact digests ascending", "boundary": PRECLINICAL_BOUNDARY}
    artifact = {"content_hash": research_artifact_digest(qualified_set), "media_type": "application/vnd.aurora.qualified-evidence-set+json"}
    receipt = LocalEvidenceSurveillanceReceipt(request_id=request_id, study_id=study_id, intent=intent, disposition=disposition, candidate_order=candidate, selected_order=selected_order, unresolved_order=tuple(sorted(unresolved)), denied_order=tuple(sorted(denied)), replay_identity=replay_identity, evidence_digest=evidence_digest, provenance_digest=provenance_digest, omissions=tuple(sorted(omissions)), uncertainty=tuple(sorted(uncertainty)), negative_evidence=tuple(sorted(negative)), effect_receipts=(f"read:local-evidence-surveillance:{request_id}",) if disposition != "blocked" else ("block:unsafe-release",), qualified_set=qualified_set, artifact=artifact, raw_data_local=raw_data_local)
    receipt.validate(); return receipt
