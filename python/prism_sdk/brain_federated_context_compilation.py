"""Python parity contract for aggregate-only federated context compilation."""
from __future__ import annotations

from dataclasses import dataclass
import re
from typing import Any, Mapping, Sequence

from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError, research_artifact_digest

FEDERATED_CONTEXT_COMPILATION_FEATURE_ID = "AFA-brain-P03-F04"
FEDERATED_CONTEXT_COMPILATION_CONTRACT_VERSION = "brain-federated-context-compilation/1.0"


@dataclass(frozen=True)
class BrainFederatedContextCompilationReceipt:
    request_id: str
    federation_id: str
    institution_id: str
    purpose: str
    semantic_profile: str
    endpoint: str
    study_order: tuple[str, ...]
    modality_order: tuple[str, ...]
    disposition: str
    candidate_order: tuple[str, ...]
    qualified_order: tuple[str, ...]
    blocked_order: tuple[str, ...]
    unknown_order: tuple[str, ...]
    aggregate_order: tuple[str, ...]
    comparability_digest: str
    envelope_digest: str
    context_digest: str
    replay_identity: str
    omissions: tuple[str, ...]
    uncertainty: tuple[str, ...]
    negative_evidence: tuple[str, ...]
    effect_receipts: tuple[str, ...]
    artifact: Mapping[str, Any]
    feature_id: str = FEDERATED_CONTEXT_COMPILATION_FEATURE_ID
    contract_version: str = FEDERATED_CONTEXT_COMPILATION_CONTRACT_VERSION
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    raw_data_local: bool = True
    aggregate_only: bool = True
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION or self.feature_id != FEDERATED_CONTEXT_COMPILATION_FEATURE_ID or self.contract_version != FEDERATED_CONTEXT_COMPILATION_CONTRACT_VERSION:
            raise ResearchContractError("federated context compilation schema, feature, or version mismatch")
        if self.boundary != PRECLINICAL_BOUNDARY or not self.raw_data_local or not self.aggregate_only or not self.request_id.strip() or not self.federation_id.strip() or not self.institution_id.strip() or not self.purpose.strip() or not self.semantic_profile.strip() or not self.endpoint.strip() or len(self.study_order) < 2 or len(self.modality_order) < 2 or not self.candidate_order or not self.effect_receipts or self.disposition not in {"qualified", "partial", "unknown", "blocked"}:
            raise ResearchContractError("federated context identity, closure, aggregate-only locality, disposition, or effects are incomplete")
        for values in (self.study_order, self.modality_order, self.candidate_order, self.qualified_order, self.blocked_order, self.unknown_order, self.aggregate_order, self.omissions, self.uncertainty, self.negative_evidence, self.effect_receipts):
            if tuple(sorted(set(values))) != values:
                raise ResearchContractError("federated context vectors are not canonical")
        candidates = set(self.candidate_order)
        classified = set(self.qualified_order) | set(self.blocked_order) | set(self.unknown_order)
        if classified != candidates or any(not re.fullmatch(r"[0-9a-f]{64}", value) for value in self.aggregate_order):
            raise ResearchContractError("federated context candidate states or aggregate order are invalid")
        for value in (self.comparability_digest, self.envelope_digest, self.context_digest, self.replay_identity, self.artifact.get("content_hash")):
            if not isinstance(value, str) or not re.fullmatch(r"[0-9a-f]{64}", value):
                raise ResearchContractError("federated context digest is invalid")
        if any(not effect.startswith("manage:local-federated-context:") and effect != "block:unsafe-release" for effect in self.effect_receipts):
            raise ResearchContractError("federated context effect is outside local management gate")

    def digest(self) -> str:
        self.validate()
        return research_artifact_digest({"schema_version": self.schema_version, "contract_version": self.contract_version, "feature_id": self.feature_id, "request_id": self.request_id, "federation_id": self.federation_id, "institution_id": self.institution_id, "purpose": self.purpose, "semantic_profile": self.semantic_profile, "endpoint": self.endpoint, "study_order": list(self.study_order), "modality_order": list(self.modality_order), "disposition": self.disposition, "candidate_order": list(self.candidate_order), "qualified_order": list(self.qualified_order), "blocked_order": list(self.blocked_order), "unknown_order": list(self.unknown_order), "aggregate_order": list(self.aggregate_order), "comparability_digest": self.comparability_digest, "envelope_digest": self.envelope_digest, "context_digest": self.context_digest, "replay_identity": self.replay_identity, "omissions": list(self.omissions), "uncertainty": list(self.uncertainty), "negative_evidence": list(self.negative_evidence), "effect_receipts": list(self.effect_receipts), "artifact": dict(self.artifact), "raw_data_local": self.raw_data_local, "aggregate_only": self.aggregate_only, "boundary": self.boundary})


def compile_federated_context(*, request_id: str, federation_id: str, institution_id: str, purpose: str, semantic_profile: str, endpoint: str, study_ids: Sequence[str], required_modalities: Sequence[str], required_context_ids: Sequence[str], candidates: Sequence[Mapping[str, Any]], minimum_support_milli: int, replay_identity: str, policy_allow: bool = True, protected_closure: bool = True, signed_approval: bool = True, raw_data_local: bool = True, aggregate_only: bool = True) -> BrainFederatedContextCompilationReceipt:
    if not all(value.strip() for value in (request_id, federation_id, institution_id, purpose, semantic_profile, endpoint)) or len(study_ids) < 2 or len(required_modalities) < 2 or not required_context_ids or not re.fullmatch(r"[0-9a-f]{64}", replay_identity):
        raise ResearchContractError("federated context identity, closure, or replay is invalid")
    studies = tuple(sorted(set(study_ids))); modalities = tuple(sorted(set(required_modalities))); required = tuple(sorted(set(required_context_ids)))
    if len(studies) != len(study_ids) or len(modalities) != len(required_modalities) or len(required) != len(required_context_ids) or any(not value.strip() for value in required):
        raise ResearchContractError("federated context identities must be unique and non-empty")
    candidate_map = {str(item["context_id"]): item for item in candidates}
    if len(candidate_map) != len(candidates):
        raise ResearchContractError("federated context candidates must be unique")
    qualified: list[str] = []; blocked: list[str] = []; unknown: list[str] = []; omissions: list[str] = []; uncertainty: list[str] = []; aggregates: list[str] = []
    for context_id in required:
        item = candidate_map.get(context_id)
        if item is None:
            unknown.append(context_id); omissions.append(f"context:{context_id}:missing-at-institution")
        elif not policy_allow or not protected_closure or not signed_approval or not raw_data_local or not aggregate_only or not bool(item.get("raw_data_local", True)) or str(item.get("boundary", PRECLINICAL_BOUNDARY)) != PRECLINICAL_BOUNDARY or str(item.get("study_id")) not in studies or str(item.get("modality")) not in modalities:
            blocked.append(context_id); omissions.append(f"context:{context_id}:federation-gate-blocked")
        elif str(item.get("replay_identity")) != replay_identity:
            unknown.append(context_id); uncertainty.append(f"context:{context_id}:replay-mismatch")
        elif str(item.get("state")) == "supported" and int(item.get("support_milli", 0)) >= minimum_support_milli:
            qualified.append(context_id); aggregates.append(research_artifact_digest({"context_id": context_id, "study_id": item["study_id"], "modality": item["modality"], "support_milli": item["support_milli"], "evidence_digest": item["evidence_digest"], "provenance_digest": item["provenance_digest"]}))
        elif str(item.get("state")) in {"unknown", "speculative"}:
            unknown.append(context_id); uncertainty.append(f"context:{context_id}:evidence-state-unknown")
        else:
            blocked.append(context_id); omissions.append(f"context:{context_id}:unsupported-or-below-threshold")
    disposition = "blocked" if not policy_allow or not protected_closure or not signed_approval or not raw_data_local or not aggregate_only else ("qualified" if len(qualified) == len(required) and not omissions and not uncertainty else ("partial" if qualified else "unknown"))
    comparability_digest = research_artifact_digest({"study_order": list(studies), "modality_order": list(modalities), "semantic_profile": semantic_profile})
    context_digest = research_artifact_digest({"candidate_order": list(required), "qualified_order": qualified, "blocked_order": blocked, "unknown_order": unknown, "replay_identity": replay_identity})
    envelope_digest = research_artifact_digest({"federation_id": federation_id, "institution_id": institution_id, "purpose": purpose, "aggregate_order": aggregates, "comparability_digest": comparability_digest, "context_digest": context_digest})
    effects = (f"manage:local-federated-context:{federation_id}",) if disposition in {"qualified", "partial"} else ("block:unsafe-release",)
    artifact = {"content_hash": research_artifact_digest({"request_id": request_id, "envelope_digest": envelope_digest}), "media_type": "application/vnd.aurora.federated-context+json"}
    receipt = BrainFederatedContextCompilationReceipt(request_id=request_id, federation_id=federation_id, institution_id=institution_id, purpose=purpose, semantic_profile=semantic_profile, endpoint=endpoint, study_order=studies, modality_order=modalities, disposition=disposition, candidate_order=required, qualified_order=tuple(sorted(qualified)), blocked_order=tuple(sorted(blocked)), unknown_order=tuple(sorted(unknown)), aggregate_order=tuple(sorted(aggregates)), comparability_digest=comparability_digest, envelope_digest=envelope_digest, context_digest=context_digest, replay_identity=replay_identity, omissions=tuple(sorted(omissions)), uncertainty=tuple(sorted(uncertainty)), negative_evidence=(), effect_receipts=effects, artifact=artifact)
    receipt.validate()
    return receipt
