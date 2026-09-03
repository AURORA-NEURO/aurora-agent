"""Federated interpretation-surface assurance parity contract.

This module classifies typed evidence for a read-only research workbench.  It never renders
figures, infers biological truth, moves raw bytes, or makes clinical decisions.
"""
from __future__ import annotations

from dataclasses import dataclass
import re
from typing import Any, Mapping, Sequence

from .research_contracts import (
    PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
    RUNTIME_INTERPRETATION_ASSURANCE_CONTRACT_VERSION,
    RUNTIME_INTERPRETATION_ASSURANCE_FEATURE_ID,
    ResearchContractError,
    research_artifact_digest,
)

INPUT_SCHEMA = "EvidenceBackedResult4@1"
OUTPUT_SCHEMA = "InteractiveInterpretation7@1"
CONTENT_TYPE = "application/vnd.aurora.runtime-interactive-interpretation-7+json"


@dataclass(frozen=True)
class InteractiveInterpretation7:
    value: Mapping[str, Any]

    def to_dict(self) -> dict[str, Any]:
        return dict(self.value)

    def validate(self) -> None:
        value = self.value
        if (
            value.get("schema_version") != OUTPUT_SCHEMA
            or value.get("feature_id") != RUNTIME_INTERPRETATION_ASSURANCE_FEATURE_ID
            or value.get("contract_version") != RUNTIME_INTERPRETATION_ASSURANCE_CONTRACT_VERSION
            or value.get("boundary") != PRECLINICAL_BOUNDARY
            or value.get("locality") != "raw-data-local; aggregate-only federation"
            or value.get("effect_receipts") != ["block:unsafe-release"]
            or not value.get("candidate_order")
        ):
            raise ResearchContractError("runtime interpretation identity, locality, boundary, or release gate is incomplete")
        artifact = value.get("artifact")
        if not isinstance(artifact, Mapping) or not re.fullmatch(r"[0-9a-f]{64}", str(artifact.get("content_hash", ""))):
            raise ResearchContractError("runtime interpretation artifact digest is invalid")
        candidates = set(value.get("candidate_order", ()))
        classified = set(value.get("qualified", ())) | set(value.get("unresolved", ())) | set(value.get("blocked", ())) | set(value.get("incomparable", ()))
        if classified != candidates:
            raise ResearchContractError("runtime interpretation outcomes do not partition candidates")

    def digest(self) -> str:
        self.validate()
        return research_artifact_digest(self.value)


def _digest(value: Any) -> str:
    return research_artifact_digest(value)


def assure_interpretation(request: Mapping[str, Any]) -> InteractiveInterpretation7:
    """Classify a typed evidence result into a deterministic interpretation receipt."""
    if (
        request.get("schema_version") != INPUT_SCHEMA
        or request.get("boundary") != PRECLINICAL_BOUNDARY
        or not all(str(request.get(field, "")).strip() for field in ("request_id", "researcher", "purpose", "semantic_profile", "replay_identity"))
        or not isinstance(request.get("candidates"), Sequence)
        or not request.get("candidates")
    ):
        raise ResearchContractError("typed evidence result is incomplete or outside the preclinical boundary")
    candidates = sorted(
        request["candidates"],
        key=lambda item: (-int(item.get("interpretation_score_milli", 0)), int(item.get("study_order", 0)), int(item.get("modality_order", 0)), str(item.get("candidate_id", ""))),
    )
    order = [str(item.get("candidate_id", "")) for item in candidates]
    if len(set(order)) != len(order) or any(not item for item in order):
        raise ResearchContractError("candidate identifiers must be unique and non-empty")
    global_block = not all(bool(request.get(field, False)) for field in ("policy_allowed", "protected_closure", "signed_approval", "federation_allowed", "raw_data_local", "aggregate_only")) or bool(request.get("adversarial_event_order"))
    qualified: list[str] = []; unresolved: list[str] = []; blocked: list[str] = []; incomparable: list[str] = []
    missing_study: set[int] = set(); missing_modality: set[int] = set(); omissions: set[str] = set(request.get("adversarial_event_order", ())); uncertainty: set[str] = set(); negative: set[str] = set()
    required_profile = str(request["semantic_profile"]); required_digest = str(request.get("comparability_digest", ""))
    for item in candidates:
        candidate_id = str(item["candidate_id"]); omissions.update(map(str, item.get("omission_order", ()))); uncertainty.update(map(str, item.get("uncertainty_order", ())))
        if bool(item.get("negative_result")) or str(item.get("evidence_state")) == "negative": negative.add(candidate_id)
        if int(item.get("study_order", 0)) != int(request.get("required_study_order", 0)): missing_study.add(int(item.get("study_order", 0)))
        if int(item.get("modality_order", 0)) != int(request.get("required_modality_order", 0)): missing_modality.add(int(item.get("modality_order", 0)))
        comparable = str(item.get("semantic_profile", "")) == required_profile and str(item.get("comparability_digest", "")) == required_digest
        if global_block or not bool(item.get("policy_allowed")) or not bool(item.get("local")) or not bool(item.get("aggregate_only")): blocked.append(candidate_id)
        elif not comparable: incomparable.append(candidate_id)
        elif str(item.get("evidence_state")) in {"proven", "supported"}: qualified.append(candidate_id)
        else: unresolved.append(candidate_id)
    artifact_payload = {"request_id": request["request_id"], "candidate_order": order, "qualified": qualified, "unresolved": unresolved, "blocked": blocked, "incomparable": incomparable, "missing_study_order": sorted(missing_study), "missing_modality_order": sorted(missing_modality), "omissions": sorted(omissions), "uncertainty": sorted(uncertainty), "negative_results": sorted(negative), "replay_identity": request["replay_identity"]}
    interpretation_digest = _digest(artifact_payload)
    artifact = {"artifact_id": f"runtime-interpretation:{request['request_id']}", "content_type": CONTENT_TYPE, "content_hash": _digest({"interpretation": artifact_payload, "interpretation_digest": interpretation_digest}), "semantic_loss": "raw data and unresolved evidence remain local", "provenance": [str(request.get("comparability_digest", ""))]}
    receipt = InteractiveInterpretation7({"schema_version": OUTPUT_SCHEMA, "feature_id": RUNTIME_INTERPRETATION_ASSURANCE_FEATURE_ID, "contract_version": RUNTIME_INTERPRETATION_ASSURANCE_CONTRACT_VERSION, **artifact_payload, "interpretation_digest": interpretation_digest, "artifact": artifact, "effect_receipts": ["block:unsafe-release"], "locality": "raw-data-local; aggregate-only federation", "boundary": PRECLINICAL_BOUNDARY})
    receipt.validate()
    return receipt


def interpretation_assurance_manifest() -> dict[str, Any]:
    return {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "capability_id": RUNTIME_INTERPRETATION_ASSURANCE_FEATURE_ID, "version": RUNTIME_INTERPRETATION_ASSURANCE_CONTRACT_VERSION, "owner_crate": "runtime", "consumer": "laboratory automation engineer", "inputs": [INPUT_SCHEMA], "outputs": [OUTPUT_SCHEMA], "effects": ["block:unsafe-release"], "autonomy_tier": "a1", "boundary": PRECLINICAL_BOUNDARY}
