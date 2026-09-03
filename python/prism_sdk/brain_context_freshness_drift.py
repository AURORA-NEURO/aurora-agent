"""Python parity contract for context freshness and semantic-drift evaluation."""
from __future__ import annotations

from dataclasses import dataclass
import re
from typing import Any, Mapping

from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError, research_artifact_digest

CONTEXT_FRESHNESS_DRIFT_FEATURE_ID = "AFA-brain-P03-F07"
CONTEXT_FRESHNESS_DRIFT_CONTRACT_VERSION = "brain-context-freshness-drift/1.0"


@dataclass(frozen=True)
class BrainContextFreshnessDriftReceipt:
    request_id: str
    objective: str
    disposition: str
    changed_dimension_order: tuple[str, ...]
    freshness_age_seconds: int
    baseline_digest: str
    candidate_digest: str
    drift_digest: str
    context_digest: str
    replay_identity: str
    omissions: tuple[str, ...]
    uncertainty: tuple[str, ...]
    negative_evidence: tuple[str, ...]
    effect_receipts: tuple[str, ...]
    artifact: Mapping[str, Any]
    feature_id: str = CONTEXT_FRESHNESS_DRIFT_FEATURE_ID
    contract_version: str = CONTEXT_FRESHNESS_DRIFT_CONTRACT_VERSION
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    raw_data_local: bool = True
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION or self.feature_id != CONTEXT_FRESHNESS_DRIFT_FEATURE_ID or self.contract_version != CONTEXT_FRESHNESS_DRIFT_CONTRACT_VERSION:
            raise ResearchContractError("freshness/drift schema, feature, or version mismatch")
        if self.boundary != PRECLINICAL_BOUNDARY or not self.raw_data_local or not self.request_id.strip() or not self.objective.strip() or not self.effect_receipts or self.disposition not in {"fresh", "drifted", "stale", "unknown", "blocked"}:
            raise ResearchContractError("freshness/drift identity, locality, disposition, or effects are incomplete")
        for values in (self.changed_dimension_order, self.omissions, self.uncertainty, self.negative_evidence, self.effect_receipts):
            if tuple(sorted(set(values))) != values:
                raise ResearchContractError("freshness/drift vectors are not canonical")
        for value in (self.baseline_digest, self.candidate_digest, self.drift_digest, self.context_digest, self.replay_identity, self.artifact.get("content_hash")):
            if not isinstance(value, str) or not re.fullmatch(r"[0-9a-f]{64}", value):
                raise ResearchContractError("freshness/drift digest is invalid")
        if any(not effect.startswith("evaluate:local-context-freshness:") and effect != "block:unsafe-release" for effect in self.effect_receipts):
            raise ResearchContractError("freshness/drift effect is outside local evaluation gate")

    def digest(self) -> str:
        self.validate()
        return research_artifact_digest({"schema_version": self.schema_version, "contract_version": self.contract_version, "feature_id": self.feature_id, "request_id": self.request_id, "objective": self.objective, "disposition": self.disposition, "changed_dimension_order": list(self.changed_dimension_order), "freshness_age_seconds": self.freshness_age_seconds, "baseline_digest": self.baseline_digest, "candidate_digest": self.candidate_digest, "drift_digest": self.drift_digest, "context_digest": self.context_digest, "replay_identity": self.replay_identity, "omissions": list(self.omissions), "uncertainty": list(self.uncertainty), "negative_evidence": list(self.negative_evidence), "effect_receipts": list(self.effect_receipts), "artifact": dict(self.artifact), "raw_data_local": self.raw_data_local, "boundary": self.boundary})


def evaluate_context_freshness_drift(*, request_id: str, objective: str, baseline: Mapping[str, Any], candidate: Mapping[str, Any], now_epoch: int, max_age_seconds: int, policy_allow: bool = True, protected_closure: bool = True, raw_data_local: bool = True) -> BrainContextFreshnessDriftReceipt:
    if not request_id.strip() or not objective.strip() or max_age_seconds <= 0 or not raw_data_local:
        raise ResearchContractError("freshness/drift identity, age limit, or locality is invalid")
    age = max(0, now_epoch - int(candidate["observed_at_epoch"]))
    changed = tuple(sorted(key for key in ("source_digest", "schema_digest", "semantics_digest", "provenance_digest") if baseline[key] != candidate[key]))
    omissions = tuple(sorted(("context:policy-or-protected-closure-blocked",) if not policy_allow or not protected_closure else ((f"context:stale:{age}",) if age > max_age_seconds else ())))
    uncertainty = tuple(() if baseline["replay_identity"] == candidate["replay_identity"] else ("context:replay-identity-mismatch",))
    negative: tuple[str, ...] = ()
    disposition = "blocked" if not policy_allow or not protected_closure else ("unknown" if uncertainty else ("stale" if age > max_age_seconds else ("drifted" if changed else "fresh")))
    def snap(value: Mapping[str, Any]) -> Mapping[str, Any]:
        return {key: value[key] for key in ("snapshot_id", "source_digest", "schema_digest", "semantics_digest", "provenance_digest")}
    baseline_digest = research_artifact_digest(snap(baseline)); candidate_digest = research_artifact_digest(snap(candidate)); drift_digest = research_artifact_digest({"changed_dimension_order": list(changed), "freshness_age_seconds": age, "disposition": disposition}); context_digest = research_artifact_digest({"feature_id": CONTEXT_FRESHNESS_DRIFT_FEATURE_ID, "baseline_digest": baseline_digest, "candidate_digest": candidate_digest, "drift_digest": drift_digest, "replay_identity": candidate["replay_identity"]})
    effects = (f"evaluate:local-context-freshness:{request_id}",) if disposition == "fresh" else ("block:unsafe-release",)
    artifact = {"content_hash": research_artifact_digest({"request_id": request_id, "context_digest": context_digest}), "media_type": "application/vnd.aurora.context-freshness-drift+json"}
    receipt = BrainContextFreshnessDriftReceipt(request_id=request_id, objective=objective, disposition=disposition, changed_dimension_order=changed, freshness_age_seconds=age, baseline_digest=baseline_digest, candidate_digest=candidate_digest, drift_digest=drift_digest, context_digest=context_digest, replay_identity=str(candidate["replay_identity"]), omissions=omissions, uncertainty=uncertainty, negative_evidence=negative, effect_receipts=effects, artifact=artifact)
    receipt.validate(); return receipt
