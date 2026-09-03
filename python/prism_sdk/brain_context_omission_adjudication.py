"""Python parity contract for typed context omission/conflict adjudication."""
from __future__ import annotations

from dataclasses import dataclass
import re
from typing import Any, Mapping, Sequence

from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError, research_artifact_digest

CONTEXT_OMISSION_ADJUDICATION_FEATURE_ID = "AFA-brain-P03-F05"
CONTEXT_OMISSION_ADJUDICATION_CONTRACT_VERSION = "brain-context-omission-adjudication/1.0"


@dataclass(frozen=True)
class BrainContextOmissionAdjudicationReceipt:
    request_id: str
    objective: str
    disposition: str
    required_evidence_order: tuple[str, ...]
    admitted_order: tuple[str, ...]
    contested_order: tuple[str, ...]
    missing_order: tuple[str, ...]
    blocked_order: tuple[str, ...]
    unknown_order: tuple[str, ...]
    omission_certificate_order: tuple[str, ...]
    adjudication_digest: str
    context_digest: str
    replay_identity: str
    omissions: tuple[str, ...]
    uncertainty: tuple[str, ...]
    negative_evidence: tuple[str, ...]
    effect_receipts: tuple[str, ...]
    artifact: Mapping[str, Any]
    feature_id: str = CONTEXT_OMISSION_ADJUDICATION_FEATURE_ID
    contract_version: str = CONTEXT_OMISSION_ADJUDICATION_CONTRACT_VERSION
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    raw_data_local: bool = True
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION or self.feature_id != CONTEXT_OMISSION_ADJUDICATION_FEATURE_ID or self.contract_version != CONTEXT_OMISSION_ADJUDICATION_CONTRACT_VERSION:
            raise ResearchContractError("omission adjudication schema, feature, or version mismatch")
        if self.boundary != PRECLINICAL_BOUNDARY or not self.raw_data_local or not self.request_id.strip() or not self.objective.strip() or not self.required_evidence_order or not self.omission_certificate_order or not self.effect_receipts or self.disposition not in {"qualified", "partial", "unknown", "blocked"}:
            raise ResearchContractError("omission adjudication identity, evidence, certificates, locality, disposition, or effects are incomplete")
        for values in (self.required_evidence_order, self.admitted_order, self.contested_order, self.missing_order, self.blocked_order, self.unknown_order, self.omission_certificate_order, self.omissions, self.uncertainty, self.negative_evidence, self.effect_receipts):
            if tuple(sorted(set(values))) != values:
                raise ResearchContractError("omission adjudication vectors are not canonical")
        required = set(self.required_evidence_order); classified = set(self.admitted_order) | set(self.contested_order) | set(self.missing_order) | set(self.blocked_order) | set(self.unknown_order)
        if classified != required:
            raise ResearchContractError("omission adjudication states do not partition required evidence")
        for value in (self.adjudication_digest, self.context_digest, self.replay_identity, self.artifact.get("content_hash")):
            if not isinstance(value, str) or not re.fullmatch(r"[0-9a-f]{64}", value):
                raise ResearchContractError("omission adjudication digest is invalid")
        if any(not effect.startswith("compile:local-omission-adjudication:") and effect != "block:unsafe-release" for effect in self.effect_receipts):
            raise ResearchContractError("omission adjudication effect is outside local compilation gate")

    def digest(self) -> str:
        self.validate()
        return research_artifact_digest({"schema_version": self.schema_version, "contract_version": self.contract_version, "feature_id": self.feature_id, "request_id": self.request_id, "objective": self.objective, "disposition": self.disposition, "required_evidence_order": list(self.required_evidence_order), "admitted_order": list(self.admitted_order), "contested_order": list(self.contested_order), "missing_order": list(self.missing_order), "blocked_order": list(self.blocked_order), "unknown_order": list(self.unknown_order), "omission_certificate_order": list(self.omission_certificate_order), "adjudication_digest": self.adjudication_digest, "context_digest": self.context_digest, "replay_identity": self.replay_identity, "omissions": list(self.omissions), "uncertainty": list(self.uncertainty), "negative_evidence": list(self.negative_evidence), "effect_receipts": list(self.effect_receipts), "artifact": dict(self.artifact), "raw_data_local": self.raw_data_local, "boundary": self.boundary})


def adjudicate_context_omissions(*, request_id: str, objective: str, required_evidence_ids: Sequence[str], evidence: Sequence[Mapping[str, Any]], minimum_support_milli: int, replay_identity: str, policy_allow: bool = True, protected_closure: bool = True, raw_data_local: bool = True) -> BrainContextOmissionAdjudicationReceipt:
    if not request_id.strip() or not objective.strip() or not required_evidence_ids or not re.fullmatch(r"[0-9a-f]{64}", replay_identity):
        raise ResearchContractError("omission adjudication identity, required evidence, or replay is invalid")
    required = tuple(sorted(set(required_evidence_ids)))
    if len(required) != len(required_evidence_ids) or any(not value.strip() for value in required):
        raise ResearchContractError("required evidence identifiers must be unique and non-empty")
    evidence_map = {str(item["evidence_id"]): item for item in evidence}
    admitted: list[str] = []; contested: list[str] = []; missing: list[str] = []; blocked: list[str] = []; unknown: list[str] = []; omissions: list[str] = []; uncertainty: list[str] = []; negative: list[str] = []
    for evidence_id in required:
        item = evidence_map.get(evidence_id)
        if item is None:
            missing.append(evidence_id); omissions.append(f"evidence:{evidence_id}:missing")
        elif not policy_allow or not protected_closure or not raw_data_local or not bool(item.get("raw_data_local", True)) or not bool(item.get("provenance_complete", False)) or str(item.get("boundary", PRECLINICAL_BOUNDARY)) != PRECLINICAL_BOUNDARY:
            blocked.append(evidence_id); omissions.append(f"evidence:{evidence_id}:policy-provenance-locality-blocked")
        elif str(item.get("replay_identity")) != replay_identity:
            unknown.append(evidence_id); uncertainty.append(f"evidence:{evidence_id}:replay-mismatch")
        elif str(item.get("state")) == "contradicted":
            contested.append(evidence_id); negative.append(f"evidence:{evidence_id}:contradicted")
        elif str(item.get("state")) == "supported" and int(item.get("support_milli", 0)) >= minimum_support_milli:
            admitted.append(evidence_id)
        elif str(item.get("state")) in {"unknown", "speculative"}:
            unknown.append(evidence_id); uncertainty.append(f"evidence:{evidence_id}:unresolved")
        else:
            blocked.append(evidence_id); omissions.append(f"evidence:{evidence_id}:below-support-or-unproven")
    certificates = tuple(sorted({f"certificate:{value}" for value in omissions + uncertainty + negative} or {"certificate:none"}))
    disposition = "blocked" if not policy_allow or not protected_closure or not raw_data_local else ("qualified" if len(admitted) == len(required) and not omissions and not uncertainty and not negative else ("partial" if admitted else "unknown"))
    adjudication_digest = research_artifact_digest({"required": list(required), "admitted": admitted, "contested": contested, "missing": missing, "blocked": blocked, "unknown": unknown, "certificates": list(certificates), "replay_identity": replay_identity})
    context_digest = research_artifact_digest({"feature_id": CONTEXT_OMISSION_ADJUDICATION_FEATURE_ID, "request_id": request_id, "adjudication_digest": adjudication_digest, "negative": negative})
    effects = (f"compile:local-omission-adjudication:{request_id}",) if disposition in {"qualified", "partial"} else ("block:unsafe-release",)
    artifact = {"content_hash": research_artifact_digest({"request_id": request_id, "adjudication_digest": adjudication_digest}), "media_type": "application/vnd.aurora.context-omission-adjudication+json"}
    receipt = BrainContextOmissionAdjudicationReceipt(request_id=request_id, objective=objective, disposition=disposition, required_evidence_order=required, admitted_order=tuple(admitted), contested_order=tuple(contested), missing_order=tuple(missing), blocked_order=tuple(blocked), unknown_order=tuple(unknown), omission_certificate_order=certificates, adjudication_digest=adjudication_digest, context_digest=context_digest, replay_identity=replay_identity, omissions=tuple(sorted(omissions)), uncertainty=tuple(sorted(uncertainty)), negative_evidence=tuple(sorted(negative)), effect_receipts=effects, artifact=artifact)
    receipt.validate(); return receipt
