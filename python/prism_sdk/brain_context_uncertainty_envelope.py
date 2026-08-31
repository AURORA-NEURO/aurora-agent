"""Python parity contract for confidence and interval uncertainty envelopes."""
from __future__ import annotations

from dataclasses import dataclass
import re
from typing import Any, Mapping, Sequence

from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError, research_artifact_digest

CONTEXT_UNCERTAINTY_ENVELOPE_FEATURE_ID = "AFA-brain-P03-F08"
CONTEXT_UNCERTAINTY_ENVELOPE_CONTRACT_VERSION = "brain-context-uncertainty-envelope/1.0"


@dataclass(frozen=True)
class BrainContextUncertaintyEnvelopeReceipt:
    request_id: str
    objective: str
    disposition: str
    required_evidence_order: tuple[str, ...]
    qualified_order: tuple[str, ...]
    uncertain_order: tuple[str, ...]
    missing_order: tuple[str, ...]
    blocked_order: tuple[str, ...]
    interval_width_order: tuple[str, ...]
    uncertainty_digest: str
    context_digest: str
    replay_identity: str
    omissions: tuple[str, ...]
    uncertainty: tuple[str, ...]
    negative_evidence: tuple[str, ...]
    effect_receipts: tuple[str, ...]
    artifact: Mapping[str, Any]
    feature_id: str = CONTEXT_UNCERTAINTY_ENVELOPE_FEATURE_ID
    contract_version: str = CONTEXT_UNCERTAINTY_ENVELOPE_CONTRACT_VERSION
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    raw_data_local: bool = True
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION or self.feature_id != CONTEXT_UNCERTAINTY_ENVELOPE_FEATURE_ID or self.contract_version != CONTEXT_UNCERTAINTY_ENVELOPE_CONTRACT_VERSION:
            raise ResearchContractError("uncertainty envelope schema, feature, or version mismatch")
        if self.boundary != PRECLINICAL_BOUNDARY or not self.raw_data_local or not self.request_id.strip() or not self.objective.strip() or not self.required_evidence_order or not self.effect_receipts or self.disposition not in {"qualified", "partial", "unknown", "blocked"}:
            raise ResearchContractError("uncertainty envelope identity, evidence, locality, disposition, or effects are incomplete")
        for values in (self.required_evidence_order, self.qualified_order, self.uncertain_order, self.missing_order, self.blocked_order, self.interval_width_order, self.omissions, self.uncertainty, self.negative_evidence, self.effect_receipts):
            if tuple(sorted(set(values))) != values:
                raise ResearchContractError("uncertainty envelope vectors are not canonical")
        required = set(self.required_evidence_order); classified = set(self.qualified_order) | set(self.uncertain_order) | set(self.missing_order) | set(self.blocked_order)
        if classified != required:
            raise ResearchContractError("uncertainty envelope states do not partition required evidence")
        for value in (self.uncertainty_digest, self.context_digest, self.replay_identity, self.artifact.get("content_hash")):
            if not isinstance(value, str) or not re.fullmatch(r"[0-9a-f]{64}", value):
                raise ResearchContractError("uncertainty envelope digest is invalid")
        if any(not effect.startswith("compile:local-uncertainty-envelope:") and effect != "block:unsafe-release" for effect in self.effect_receipts):
            raise ResearchContractError("uncertainty envelope effect is outside local compilation gate")

    def digest(self) -> str:
        self.validate()
        return research_artifact_digest({"schema_version": self.schema_version, "contract_version": self.contract_version, "feature_id": self.feature_id, "request_id": self.request_id, "objective": self.objective, "disposition": self.disposition, "required_evidence_order": list(self.required_evidence_order), "qualified_order": list(self.qualified_order), "uncertain_order": list(self.uncertain_order), "missing_order": list(self.missing_order), "blocked_order": list(self.blocked_order), "interval_width_order": list(self.interval_width_order), "uncertainty_digest": self.uncertainty_digest, "context_digest": self.context_digest, "replay_identity": self.replay_identity, "omissions": list(self.omissions), "uncertainty": list(self.uncertainty), "negative_evidence": list(self.negative_evidence), "effect_receipts": list(self.effect_receipts), "artifact": dict(self.artifact), "raw_data_local": self.raw_data_local, "boundary": self.boundary})


def compile_context_uncertainty_envelope(*, request_id: str, objective: str, required_evidence_ids: Sequence[str], observations: Sequence[Mapping[str, Any]], minimum_confidence_milli: int, maximum_interval_width_milli: int, replay_identity: str, policy_allow: bool = True, protected_closure: bool = True, raw_data_local: bool = True) -> BrainContextUncertaintyEnvelopeReceipt:
    if not request_id.strip() or not objective.strip() or not required_evidence_ids or maximum_interval_width_milli <= 0 or not re.fullmatch(r"[0-9a-f]{64}", replay_identity):
        raise ResearchContractError("uncertainty envelope identity, thresholds, or replay is invalid")
    required = tuple(sorted(set(required_evidence_ids))); items = {str(item["evidence_id"]): item for item in observations}
    if len(required) != len(required_evidence_ids) or any(not value.strip() for value in required) or len(items) != len(observations):
        raise ResearchContractError("uncertainty evidence identifiers must be unique and non-empty")
    qualified: list[str] = []; uncertain: list[str] = []; missing: list[str] = []; blocked: list[str] = []; widths: list[str] = []; omissions: list[str] = []; uncertainty: list[str] = []; negative: list[str] = []
    for evidence_id in required:
        item = items.get(evidence_id)
        if item is None:
            missing.append(evidence_id); omissions.append(f"evidence:{evidence_id}:missing")
        elif not policy_allow or not protected_closure or not raw_data_local or not bool(item.get("raw_data_local", True)) or not bool(item.get("provenance_complete", False)) or str(item.get("boundary", PRECLINICAL_BOUNDARY)) != PRECLINICAL_BOUNDARY:
            blocked.append(evidence_id); omissions.append(f"evidence:{evidence_id}:policy-provenance-locality-blocked")
        elif str(item.get("replay_identity")) != replay_identity:
            uncertain.append(evidence_id); uncertainty.append(f"evidence:{evidence_id}:replay-mismatch")
        else:
            width = int(item["upper_milli"]) - int(item["lower_milli"]); widths.append(f"{evidence_id}:{width}")
            if str(item.get("state")) == "contradicted":
                uncertain.append(evidence_id); negative.append(f"evidence:{evidence_id}:contradicted")
            elif str(item.get("state")) == "supported" and int(item.get("confidence_milli", 0)) >= minimum_confidence_milli and width <= maximum_interval_width_milli:
                qualified.append(evidence_id)
            elif str(item.get("state")) in {"unknown", "speculative"}:
                uncertain.append(evidence_id); uncertainty.append(f"evidence:{evidence_id}:unresolved")
            else:
                uncertain.append(evidence_id); uncertainty.append(f"evidence:{evidence_id}:confidence-or-interval-too-wide")
    disposition = "blocked" if not policy_allow or not protected_closure or not raw_data_local else ("unknown" if not qualified else ("qualified" if len(qualified) == len(required) and not uncertain and not missing and not blocked and not omissions and not uncertainty and not negative else "partial"))
    uncertainty_digest = research_artifact_digest({"required": list(required), "qualified": qualified, "uncertain": uncertain, "missing": missing, "blocked": blocked, "interval_width_order": sorted(widths), "replay_identity": replay_identity}); context_digest = research_artifact_digest({"feature_id": CONTEXT_UNCERTAINTY_ENVELOPE_FEATURE_ID, "request_id": request_id, "uncertainty_digest": uncertainty_digest, "negative": negative})
    effects = (f"compile:local-uncertainty-envelope:{request_id}",) if disposition in {"qualified", "partial"} else ("block:unsafe-release",); artifact = {"content_hash": research_artifact_digest({"request_id": request_id, "context_digest": context_digest}), "media_type": "application/vnd.aurora.context-uncertainty-envelope+json"}
    receipt = BrainContextUncertaintyEnvelopeReceipt(request_id=request_id, objective=objective, disposition=disposition, required_evidence_order=required, qualified_order=tuple(sorted(qualified)), uncertain_order=tuple(sorted(uncertain)), missing_order=tuple(sorted(missing)), blocked_order=tuple(sorted(blocked)), interval_width_order=tuple(sorted(widths)), uncertainty_digest=uncertainty_digest, context_digest=context_digest, replay_identity=replay_identity, omissions=tuple(sorted(omissions)), uncertainty=tuple(sorted(uncertainty)), negative_evidence=tuple(sorted(negative)), effect_receipts=effects, artifact=artifact)
    receipt.validate(); return receipt
