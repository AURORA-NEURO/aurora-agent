"""Multimodal quality-control workflow fabric for ``AFA-cli-P07-F14``."""
from __future__ import annotations

from dataclasses import asdict, dataclass
import hashlib
import json
import re
from typing import Any, Mapping

from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError

FEATURE_ID = "AFA-cli-P07-F14"
CONTRACT_VERSION = "cli-multimodal-quality-control-workflow-fabric/1.0"
INPUT_SCHEMA = "QualityWorkflowDraft5@1"
OUTPUT_SCHEMA = "QualityWorkflowRun8@1"
STAGES = ("admit", "measure", "checkpoint", "quarantine", "retain-receipt")


def _hash(value: Any) -> str:
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()).hexdigest()


def _digest(value: Any) -> bool:
    return isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) is not None


def _canonical(values: list[str] | tuple[str, ...]) -> bool:
    return list(values) == sorted(set(values))


@dataclass(frozen=True)
class QualityControlWorkflowRun:
    schema_version: str; contract_version: str; feature_id: str; request_id: str; run_id: str; scope: str; semantic_profile: str; disposition: str; stage_order: tuple[str, ...]; required_observation_order: tuple[str, ...]; required_modality_order: tuple[str, ...]; passed_observation_order: tuple[str, ...]; pending_observation_order: tuple[str, ...]; quarantined_observation_order: tuple[str, ...]; blocked_observation_order: tuple[str, ...]; missing_modality_order: tuple[str, ...]; omission_order: tuple[str, ...]; uncertainty_order: tuple[str, ...]; negative_evidence_order: tuple[str, ...]; checkpoint_digest: str; replay_identity: str; workflow_digest: str; artifact: Mapping[str, Any]; effect_receipts: tuple[str, ...]; raw_data_local: bool; aggregate_only: bool; boundary: str

    def validate(self) -> None:
        if (self.schema_version, self.contract_version, self.feature_id) != (RESEARCH_CONTRACT_SCHEMA_VERSION, CONTRACT_VERSION, FEATURE_ID) or self.boundary != PRECLINICAL_BOUNDARY or self.raw_data_local is not True or self.aggregate_only is not True or tuple(self.stage_order) != STAGES or not all(isinstance(value, str) and value.strip() for value in (self.request_id, self.run_id, self.scope, self.semantic_profile)) or not self.required_observation_order or not self.required_modality_order or not self.effect_receipts:
            raise ResearchContractError("quality workflow identity, stages, locality, modalities, observations, or effects are incomplete")
        for values in (self.required_observation_order, self.required_modality_order, self.passed_observation_order, self.pending_observation_order, self.quarantined_observation_order, self.blocked_observation_order, self.missing_modality_order, self.omission_order, self.uncertainty_order, self.negative_evidence_order, self.effect_receipts):
            if not _canonical(values): raise ResearchContractError("quality workflow ordering is not canonical")
        required = set(self.required_observation_order); states = list(self.passed_observation_order) + list(self.pending_observation_order) + list(self.quarantined_observation_order) + list(self.blocked_observation_order)
        if len(required) != len(self.required_observation_order) or len(states) != len(required) or set(states) != required or any(item not in self.required_modality_order for item in self.missing_modality_order): raise ResearchContractError("quality observation or modality states do not partition the plan")
        for digest in (self.checkpoint_digest, self.replay_identity, self.workflow_digest, self.artifact.get("content_hash")):
            if not _digest(digest): raise ResearchContractError("quality workflow digest is invalid")
        if self.artifact.get("content_type") != "application/vnd.aurora.quality-workflow-run+json": raise ResearchContractError("quality workflow artifact type is invalid")
        if any(not effect.startswith("retain:quality-workflow:") and not effect.startswith("quarantine:quality-workflow:") and effect != "block:unsafe-release" for effect in self.effect_receipts): raise ResearchContractError("quality workflow effect is outside retention gate")

    def to_dict(self) -> dict[str, Any]:
        value = asdict(self)
        for key, item in value.items():
            if isinstance(item, tuple): value[key] = list(item)
        return value


def assure_quality_control_workflow(*, draft: Mapping[str, Any]) -> QualityControlWorkflowRun:
    required = ("request_id", "run_id", "scope", "semantic_profile")
    if any(not str(draft.get(field, "")).strip() for field in required) or not draft.get("required_observation_order") or not _canonical([str(item) for item in draft["required_observation_order"]]) or not draft.get("required_modality_order") or not _canonical([str(item) for item in draft["required_modality_order"]]) or tuple(draft.get("stage_order", ())) != STAGES or draft.get("raw_data_local") is not True or draft.get("aggregate_only") is not True or int(draft.get("budget_units", 0)) <= 0 or int(draft.get("budget_units", 0)) > int(draft.get("max_budget_units", 0)) or draft.get("boundary") != PRECLINICAL_BOUNDARY or not _digest(draft.get("replay_identity")):
        raise ResearchContractError("quality workflow identity, stages, modalities, locality, aggregate boundary, budget, replay, or boundary is invalid")
    required_order = [str(item) for item in draft["required_observation_order"]]; modality_order = [str(item) for item in draft["required_modality_order"]]
    if len(set(required_order)) != len(required_order) or len(set(modality_order)) != len(modality_order): raise ResearchContractError("required quality observations or modalities are duplicated")
    seen: set[str] = set(); passed: set[str] = set(); pending: set[str] = set(); quarantined: set[str] = set(); blocked: set[str] = set(); observed_modalities: set[str] = set(); omissions: set[str] = set(); uncertainty: set[str] = set(); negative: set[str] = set()
    for observation in draft.get("observations", []):
        oid = str(observation.get("observation_id", "")); modality = str(observation.get("modality", "")); observation_omissions = [str(item) for item in observation.get("omissions", [])]; observation_uncertainty = [str(item) for item in observation.get("uncertainty", [])]
        if not oid.strip() or oid not in set(required_order) or oid in seen or modality not in set(modality_order) or not str(observation.get("study_id", "")).strip() or not str(observation.get("metric", "")).strip() or observation.get("baseline_milli") is None or not _digest(observation.get("provenance_digest")) or observation.get("semantic_profile") != draft["semantic_profile"] or observation.get("replay_identity") != draft["replay_identity"] or observation.get("local_data") is not True or not _canonical(observation_omissions) or not _canonical(observation_uncertainty): raise ResearchContractError("quality observation identity, modality, baseline, provenance, profile, replay, locality, or annotations are invalid")
        seen.add(oid); observed_modalities.add(modality); omissions.update(f"observation:{oid}:{item}" for item in observation_omissions); uncertainty.update(f"observation:{oid}:{item}" for item in observation_uncertainty); negative.update({f"observation:{oid}:negative-result"} if observation.get("negative_result") else set()); threshold_ok = int(observation.get("observed_milli", 0)) >= int(observation.get("threshold_milli", 0)); state = str(observation.get("evidence_state", "unknown"))
        if state == "contradicted": quarantined.add(oid); negative.add(f"observation:{oid}:contradicted")
        elif state in {"unknown", "speculative"}: pending.add(oid); uncertainty.add(f"observation:{oid}:evidence-state")
        elif state in {"proven", "supported"} and threshold_ok and not observation_omissions and not observation_uncertainty: passed.add(oid)
        else: quarantined.add(oid); omissions.add(f"observation:{oid}:threshold-or-closure")
    for oid in set(required_order) - seen: pending.add(oid); omissions.add(f"missing-observation:{oid}"); uncertainty.add(f"missing-observation:{oid}")
    missing_modalities = set(modality_order) - observed_modalities
    for modality in missing_modalities: omissions.add(f"missing-modality:{modality}"); uncertainty.add(f"missing-modality:{modality}")
    violations: set[str] = set()
    for name, field in (("policy", "policy_allow"), ("protected-closure", "protected_closure"), ("signed-approval", "signed_approval")):
        if draft.get(field) is not True: violations.add(name)
    for event in draft.get("adversarial_events", []): violations.add(f"adversarial:{event}"); omissions.add(f"workflow:adversarial:{event}")
    disposition = "blocked" if violations or draft.get("adversarial_events") else "quarantine" if pending or quarantined or missing_modalities or uncertainty else "qualified"
    if disposition == "blocked": blocked = set(required_order); passed.clear(); pending.clear(); quarantined.clear()
    passed_order = sorted(passed); pending_order = sorted(pending); quarantined_order = sorted(quarantined); blocked_order = sorted(blocked); checkpoint = {"request_id": draft["request_id"], "run_id": draft["run_id"], "stage_order": list(STAGES), "required_observation_order": required_order, "passed_observation_order": passed_order, "pending_observation_order": pending_order, "quarantined_observation_order": quarantined_order, "blocked_observation_order": blocked_order, "replay_identity": draft["replay_identity"]}; checkpoint_digest = _hash(checkpoint); workflow_digest = _hash({"checkpoint_digest": checkpoint_digest, "disposition": disposition, "semantic_profile": draft["semantic_profile"]}); artifact = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "artifact_id": f"cli-quality-workflow:{draft['request_id']}", "content_type": "application/vnd.aurora.quality-workflow-run+json", "content_hash": checkpoint_digest, "semantic_loss": [], "provenance": [{"source_id": draft["run_id"], "relation": "quality-workflow-checkpoint", "digest": checkpoint_digest}], "boundary": PRECLINICAL_BOUNDARY}; receipt = QualityControlWorkflowRun(RESEARCH_CONTRACT_SCHEMA_VERSION, CONTRACT_VERSION, FEATURE_ID, str(draft["request_id"]), str(draft["run_id"]), str(draft["scope"]), str(draft["semantic_profile"]), disposition, STAGES, tuple(required_order), tuple(modality_order), tuple(passed_order), tuple(pending_order), tuple(quarantined_order), tuple(blocked_order), tuple(sorted(missing_modalities)), tuple(sorted(omissions)), tuple(sorted(uncertainty)), tuple(sorted(negative)), checkpoint_digest, str(draft["replay_identity"]), workflow_digest, artifact, (f"retain:quality-workflow:{draft['request_id']}",) if disposition == "qualified" else (f"quarantine:quality-workflow:{draft['request_id']}",) if disposition == "quarantine" else ("block:unsafe-release",), True, True, PRECLINICAL_BOUNDARY); receipt.validate(); return receipt


__all__ = ["FEATURE_ID", "CONTRACT_VERSION", "INPUT_SCHEMA", "OUTPUT_SCHEMA", "QualityControlWorkflowRun", "assure_quality_control_workflow"]
