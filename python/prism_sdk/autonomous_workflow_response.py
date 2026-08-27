"""Value-only integrity scoring for autonomous workflow stage responses.

Workflow stages use a narrower contract than the top-level domain response: every stage reports
its identity, declared status, evidence, uncertainty, notes, and next actions.  The normal domain
evaluator still owns task-specific correctness.  This module adds an independent composition
signal so the online learner can improve stage reporting without treating a well-formed stage as
proof that the underlying task or external effect is correct.
"""

from __future__ import annotations

from dataclasses import dataclass
import math
import re
from typing import Any, Mapping, Sequence

from .authoring import content_digest
from .errors import ArgumentError


AUTONOMOUS_WORKFLOW_STAGE_RESPONSE_EVALUATION_SCHEMA = (
    "bioprism-python-autonomous-workflow-stage-response-evaluation/0.1"
)
AUTONOMOUS_WORKFLOW_STAGE_RESPONSE_EVALUATOR_VERSION = "1"
AUTONOMOUS_WORKFLOW_STAGE_RESPONSE_STATUSES = ("completed", "proposed", "blocked", "not_attempted")
AUTONOMOUS_WORKFLOW_STAGE_RESPONSE_PASS_THRESHOLD = 0.8
MAX_AUTONOMOUS_WORKFLOW_STAGE_RESPONSE_ITEMS = 32
MAX_AUTONOMOUS_WORKFLOW_STAGE_RESPONSE_ITEM_BYTES = 4_096
MAX_AUTONOMOUS_WORKFLOW_STAGE_RESPONSE_NOTES_BYTES = 32_000
_IDENTIFIER = re.compile(r"^[A-Za-z0-9_.:-]+$")
_DIGEST = re.compile(r"^[0-9a-f]{64}$")
_SECRET_KEYS = {
    "apikey", "authorization", "bearer", "credential", "credentials", "password", "secret",
    "secretkey", "token", "accesstoken", "refreshtoken", "privatekey",
}
_CREDENTIAL_SHAPES = re.compile(r"\b(?:gsk_|sk-proj-|sk-[A-Za-z0-9]{16,})", re.IGNORECASE)
_STAGE_FIELDS = ("stage_id", "status", "evidence", "uncertainty", "notes", "next_actions")
_SIGNAL_WEIGHTS = {
    "schema_valid": 2.0,
    "stage_identity": 1.5,
    "status_declared": 1.0,
    "evidence_present": 2.0,
    "uncertainty_reported": 1.5,
    "notes_present": 1.0,
    "next_actions_present": 1.0,
    "response_digest_bound": 1.0,
}


def _text(name: str, value: Any, maximum: int) -> str:
    if not isinstance(value, str) or not value.strip() or "\x00" in value or len(value.encode("utf-8")) > maximum:
        raise ArgumentError(f"{name} is outside its bounded text contract")
    return value


def _bounded_notes(value: Any) -> str:
    if not isinstance(value, str) or "\x00" in value or len(value.encode("utf-8")) > MAX_AUTONOMOUS_WORKFLOW_STAGE_RESPONSE_NOTES_BYTES:
        raise ArgumentError("workflow stage response notes are outside their bounded text contract")
    return value


def _identifier(name: str, value: Any) -> str:
    value = _text(name, value, 256)
    if not _IDENTIFIER.fullmatch(value):
        raise ArgumentError(f"{name} must be a bounded identifier")
    return value


def _digest(name: str, value: Any) -> str:
    if not isinstance(value, str) or not _DIGEST.fullmatch(value):
        raise ArgumentError(f"{name} must be a lowercase SHA-256 digest")
    return value


def _safe_value(value: Any, depth: int = 0) -> None:
    if depth > 16:
        raise ArgumentError("workflow stage response is too deeply nested")
    if isinstance(value, str):
        if _CREDENTIAL_SHAPES.search(value):
            raise ArgumentError("workflow stage response contains credential-shaped material")
        return
    if isinstance(value, Mapping):
        for key, child in value.items():
            normalized = "".join(character for character in key.lower() if character.isalnum()) if isinstance(key, str) else ""
            if normalized in _SECRET_KEYS or normalized.startswith(("gsk", "skproj")):
                raise ArgumentError("workflow stage response contains credential-shaped fields")
            _safe_value(child, depth + 1)
    elif isinstance(value, (list, tuple)):
        for child in value:
            _safe_value(child, depth + 1)


def _bounded_list(name: str, value: Any) -> tuple[str, ...]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes, bytearray)) or len(value) > MAX_AUTONOMOUS_WORKFLOW_STAGE_RESPONSE_ITEMS:
        raise ArgumentError(f"{name} must contain at most {MAX_AUTONOMOUS_WORKFLOW_STAGE_RESPONSE_ITEMS} entries")
    return tuple(_text(f"{name} entry", item, MAX_AUTONOMOUS_WORKFLOW_STAGE_RESPONSE_ITEM_BYTES) for item in value)


def _normalize_stage_response(value: Any, *, stage_id: str) -> dict[str, Any]:
    if not isinstance(value, Mapping):
        raise ArgumentError("workflow stage response must be a mapping")
    _safe_value(value)
    if set(value) != set(_STAGE_FIELDS):
        raise ArgumentError("workflow stage response contains unsupported or missing fields")
    expected_stage_id = _identifier("workflow stage response stage_id", stage_id)
    actual_stage_id = _identifier("workflow stage response stage_id", value.get("stage_id"))
    if actual_stage_id != expected_stage_id:
        raise ArgumentError("workflow stage response stage_id does not match the scheduled stage")
    status = value.get("status")
    if status not in AUTONOMOUS_WORKFLOW_STAGE_RESPONSE_STATUSES:
        raise ArgumentError("workflow stage response status is invalid")
    normalized = {
        "stage_id": actual_stage_id,
        "status": status,
        "evidence": list(_bounded_list("workflow stage response evidence", value.get("evidence"))),
        "uncertainty": list(_bounded_list("workflow stage response uncertainty", value.get("uncertainty"))),
        "notes": _bounded_notes(value.get("notes")),
        "next_actions": list(_bounded_list("workflow stage response next_actions", value.get("next_actions"))),
    }
    return normalized


@dataclass(frozen=True, slots=True)
class AutonomousWorkflowStageResponseEvaluation:
    """Digest-bound structural evaluation for one workflow stage response."""

    schema: str
    evaluator_id: str
    evaluator_version: str
    domain: str
    workflow_id: str
    workflow_digest: str
    stage_id: str
    response_digest: str
    signals: Mapping[str, float]
    missing_signals: tuple[str, ...]
    reward: float
    passed: bool
    failed: bool
    failure_class: str | None
    feedback_digest: str
    evidence_digest: str
    replan_requested: bool
    replan_instruction: str | None
    reward_input: Mapping[str, Any]
    evaluator_authority: str
    retention: str
    secret_material: str
    evaluation_digest: str

    def to_dict(self) -> dict[str, Any]:
        descriptor = {
            "schema": self.schema,
            "evaluator_id": self.evaluator_id,
            "evaluator_version": self.evaluator_version,
            "domain": self.domain,
            "workflow_id": self.workflow_id,
            "workflow_digest": self.workflow_digest,
            "stage_id": self.stage_id,
            "response_digest": self.response_digest,
            "signals": dict(self.signals),
            "missing_signals": list(self.missing_signals),
            "reward": self.reward,
            "passed": self.passed,
            "failed": self.failed,
            "failure_class": self.failure_class,
            "feedback_digest": self.feedback_digest,
            "evidence_digest": self.evidence_digest,
            "replan_requested": self.replan_requested,
            "replan_instruction": self.replan_instruction,
            "reward_input": dict(self.reward_input),
            "evaluator_authority": self.evaluator_authority,
            "retention": self.retention,
            "secret_material": self.secret_material,
        }
        return {**descriptor, "evaluation_digest": self.evaluation_digest}


def evaluate_autonomous_workflow_stage_response(
    value: Any,
    *,
    domain: str,
    workflow_id: str,
    workflow_digest: str,
    stage_id: str,
) -> AutonomousWorkflowStageResponseEvaluation:
    """Return a deterministic composition reward for a validated workflow stage response."""

    domain = _identifier("workflow stage evaluation domain", domain)
    workflow_id = _identifier("workflow stage evaluation workflow_id", workflow_id)
    workflow_digest = _digest("workflow stage evaluation workflow_digest", workflow_digest)
    normalized = _normalize_stage_response(value, stage_id=stage_id)
    response_digest = content_digest(normalized)
    # A completed stage may legitimately have no unresolved uncertainty or follow-up action.
    # The required notes field is the explicit bounded declaration that the stage has no such
    # disclosure. Non-completed stages must still report both fields themselves.
    completed_without_disclosure = normalized["status"] == "completed" and bool(normalized["notes"].strip())
    signals = {
        "schema_valid": 1.0,
        "stage_identity": 1.0,
        "status_declared": 1.0,
        "evidence_present": float(bool(normalized["evidence"])),
        "uncertainty_reported": float(bool(normalized["uncertainty"]) or completed_without_disclosure),
        "notes_present": float(bool(normalized["notes"])),
        "next_actions_present": float(bool(normalized["next_actions"]) or completed_without_disclosure),
        "response_digest_bound": 1.0,
    }
    total_weight = sum(_SIGNAL_WEIGHTS.values())
    reward = round(sum(signals[name] * weight for name, weight in _SIGNAL_WEIGHTS.items()) / total_weight, 12)
    missing = tuple(name for name, score in signals.items() if score < 1.0)
    # The aggregate reward is useful for learning, but it must never hide a missing integrity
    # signal at a continuation boundary. A stage is therefore passable only when every signal
    # is satisfied and the score clears the documented floor.
    passed = reward >= AUTONOMOUS_WORKFLOW_STAGE_RESPONSE_PASS_THRESHOLD and not missing
    evaluator_id = f"autonomous-{domain}-workflow-stage-integrity"
    feedback_digest = content_digest({"workflow_digest": workflow_digest, "stage_id": stage_id, "response_digest": response_digest, "signals": signals})
    failure_class = None if passed else "workflow_stage_response_integrity_gate"
    instruction = None if passed else f"Improve bounded {domain} workflow stage composition: {', '.join(missing) or 'the stage integrity score'}."
    reward_input = {
        "evaluator_id": evaluator_id,
        "evaluator_version": AUTONOMOUS_WORKFLOW_STAGE_RESPONSE_EVALUATOR_VERSION,
        "reward": reward,
        "passed": passed,
        "failed": not passed,
        "feedback_digest": feedback_digest,
        "failure_class": failure_class,
        "evidence_digest": response_digest,
    }
    descriptor = {
        "schema": AUTONOMOUS_WORKFLOW_STAGE_RESPONSE_EVALUATION_SCHEMA,
        "evaluator_id": evaluator_id,
        "evaluator_version": AUTONOMOUS_WORKFLOW_STAGE_RESPONSE_EVALUATOR_VERSION,
        "domain": domain,
        "workflow_id": workflow_id,
        "workflow_digest": workflow_digest,
        "stage_id": stage_id,
        "response_digest": response_digest,
        "signals": signals,
        "missing_signals": list(missing),
        "reward": reward,
        "passed": passed,
        "failed": not passed,
        "failure_class": failure_class,
        "feedback_digest": feedback_digest,
        "evidence_digest": response_digest,
        "replan_requested": not passed,
        "replan_instruction": instruction,
        "reward_input": reward_input,
        "evaluator_authority": "workflow_stage_response_contract_only;not_external_truth",
        "retention": "value_only;stage_response_and_credentials_not_retained",
        "secret_material": "never_returned",
    }
    return AutonomousWorkflowStageResponseEvaluation(
        evaluation_digest=content_digest(descriptor),
        **descriptor,
    )


def validate_autonomous_workflow_stage_response_evaluation(value: Any) -> AutonomousWorkflowStageResponseEvaluation:
    """Strictly validate a persisted workflow-stage structural evaluation projection."""

    if not isinstance(value, Mapping):
        raise ArgumentError("workflow stage response evaluation must be a mapping")
    _safe_value(value)
    allowed = {
        "schema", "evaluator_id", "evaluator_version", "domain", "workflow_id", "workflow_digest", "stage_id",
        "response_digest", "signals", "missing_signals", "reward", "passed", "failed", "failure_class",
        "feedback_digest", "evidence_digest", "replan_requested", "replan_instruction", "reward_input",
        "evaluator_authority", "retention", "secret_material", "evaluation_digest",
    }
    if set(value) != allowed or value.get("schema") != AUTONOMOUS_WORKFLOW_STAGE_RESPONSE_EVALUATION_SCHEMA:
        raise ArgumentError("workflow stage response evaluation has an invalid schema or field set")
    evaluator_id = _identifier("workflow stage evaluation evaluator_id", value.get("evaluator_id"))
    evaluator_version = _identifier("workflow stage evaluation evaluator_version", value.get("evaluator_version"))
    domain = _identifier("workflow stage evaluation domain", value.get("domain"))
    workflow_id = _identifier("workflow stage evaluation workflow_id", value.get("workflow_id"))
    stage_id = _identifier("workflow stage evaluation stage_id", value.get("stage_id"))
    workflow_digest = _digest("workflow stage evaluation workflow_digest", value.get("workflow_digest"))
    response_digest = _digest("workflow stage evaluation response_digest", value.get("response_digest"))
    feedback_digest = _digest("workflow stage evaluation feedback_digest", value.get("feedback_digest"))
    evidence_digest = _digest("workflow stage evaluation evidence_digest", value.get("evidence_digest"))
    if evidence_digest != response_digest:
        raise ArgumentError("workflow stage evaluation evidence_digest must match response_digest")
    if value.get("evaluator_authority") != "workflow_stage_response_contract_only;not_external_truth":
        raise ArgumentError("workflow stage evaluation authority marker is invalid")
    if value.get("retention") != "value_only;stage_response_and_credentials_not_retained" or value.get("secret_material") != "never_returned":
        raise ArgumentError("workflow stage evaluation retention markers are invalid")
    raw_signals = value.get("signals")
    if not isinstance(raw_signals, Mapping) or set(raw_signals) != set(_SIGNAL_WEIGHTS):
        raise ArgumentError("workflow stage evaluation signals are incomplete")
    signals: dict[str, float] = {}
    for name in _SIGNAL_WEIGHTS:
        score = raw_signals[name]
        if isinstance(score, bool) or not isinstance(score, (int, float)) or not math.isfinite(float(score)) or not 0.0 <= float(score) <= 1.0:
            raise ArgumentError("workflow stage evaluation signal scores must be finite values within [0, 1]")
        signals[name] = float(score)
    raw_missing = value.get("missing_signals")
    if not isinstance(raw_missing, Sequence) or isinstance(raw_missing, (str, bytes, bytearray)):
        raise ArgumentError("workflow stage evaluation missing_signals must be a sequence")
    missing = tuple(_identifier("workflow stage evaluation missing signal", item) for item in raw_missing)
    if len(set(missing)) != len(missing) or any(name not in signals or signals[name] >= 1.0 for name in missing):
        raise ArgumentError("workflow stage evaluation missing_signals do not match signals")
    reward = value.get("reward")
    if isinstance(reward, bool) or not isinstance(reward, (int, float)) or not math.isfinite(float(reward)) or not 0.0 <= float(reward) <= 1.0:
        raise ArgumentError("workflow stage evaluation reward must be finite and within [0, 1]")
    passed = value.get("passed")
    failed = value.get("failed")
    replan_requested = value.get("replan_requested")
    if not isinstance(passed, bool) or not isinstance(failed, bool) or failed == passed or not isinstance(replan_requested, bool) or replan_requested != failed:
        raise ArgumentError("workflow stage evaluation status flags are inconsistent")
    failure_class = value.get("failure_class")
    if failure_class is not None:
        failure_class = _identifier("workflow stage evaluation failure_class", failure_class)
    elif not passed:
        raise ArgumentError("failed workflow stage evaluations require a failure_class")
    instruction = value.get("replan_instruction")
    if instruction is not None:
        instruction = _text("workflow stage evaluation replan_instruction", instruction, MAX_AUTONOMOUS_WORKFLOW_STAGE_RESPONSE_ITEM_BYTES)
    elif not passed:
        raise ArgumentError("failed workflow stage evaluations require a replan_instruction")
    raw_reward = value.get("reward_input")
    if not isinstance(raw_reward, Mapping) or set(raw_reward) != {
        "evaluator_id", "evaluator_version", "reward", "passed", "failed", "feedback_digest", "failure_class", "evidence_digest",
    }:
        raise ArgumentError("workflow stage evaluation reward_input is malformed")
    if (
        raw_reward.get("evaluator_id") != evaluator_id
        or raw_reward.get("evaluator_version") != evaluator_version
        or raw_reward.get("reward") != reward
        or raw_reward.get("passed") != passed
        or raw_reward.get("failed") != failed
        or raw_reward.get("feedback_digest") != feedback_digest
        or raw_reward.get("failure_class") != failure_class
        or raw_reward.get("evidence_digest") != evidence_digest
    ):
        raise ArgumentError("workflow stage evaluation reward_input does not match its projection")
    descriptor = dict(value)
    evaluation_digest = descriptor.pop("evaluation_digest")
    if not isinstance(evaluation_digest, str) or _digest("workflow stage evaluation evaluation_digest", evaluation_digest) != content_digest(descriptor):
        raise ArgumentError("workflow stage evaluation digest does not match its projection")
    return AutonomousWorkflowStageResponseEvaluation(
        schema=value["schema"], evaluator_id=evaluator_id, evaluator_version=evaluator_version, domain=domain,
        workflow_id=workflow_id, workflow_digest=workflow_digest, stage_id=stage_id, response_digest=response_digest,
        signals=signals, missing_signals=missing, reward=float(reward), passed=passed, failed=failed,
        failure_class=failure_class, feedback_digest=feedback_digest, evidence_digest=evidence_digest,
        replan_requested=replan_requested, replan_instruction=instruction, reward_input=dict(raw_reward),
        evaluator_authority=value["evaluator_authority"], retention=value["retention"], secret_material=value["secret_material"],
        evaluation_digest=evaluation_digest,
    )


def replay_autonomous_workflow_stage_response_evaluation(
    value: Any,
    expected: AutonomousWorkflowStageResponseEvaluation | Mapping[str, Any],
) -> AutonomousWorkflowStageResponseEvaluation:
    """Re-run stage composition scoring and reject evaluation drift."""

    projection = expected.to_dict() if isinstance(expected, AutonomousWorkflowStageResponseEvaluation) else expected
    if not isinstance(projection, Mapping):
        raise ArgumentError("workflow stage replay requires an evaluation projection")
    validated = validate_autonomous_workflow_stage_response_evaluation(projection)
    replayed = evaluate_autonomous_workflow_stage_response(
        value,
        domain=validated.domain,
        workflow_id=validated.workflow_id,
        workflow_digest=validated.workflow_digest,
        stage_id=validated.stage_id,
    )
    if replayed.evaluation_digest != validated.evaluation_digest:
        raise ArgumentError("workflow stage evaluator replay drifted from the recorded evaluation")
    return replayed


__all__ = [
    "AUTONOMOUS_WORKFLOW_STAGE_RESPONSE_EVALUATION_SCHEMA",
    "AUTONOMOUS_WORKFLOW_STAGE_RESPONSE_EVALUATOR_VERSION",
    "AUTONOMOUS_WORKFLOW_STAGE_RESPONSE_STATUSES",
    "AUTONOMOUS_WORKFLOW_STAGE_RESPONSE_PASS_THRESHOLD",
    "MAX_AUTONOMOUS_WORKFLOW_STAGE_RESPONSE_ITEMS",
    "MAX_AUTONOMOUS_WORKFLOW_STAGE_RESPONSE_ITEM_BYTES",
    "MAX_AUTONOMOUS_WORKFLOW_STAGE_RESPONSE_NOTES_BYTES",
    "AutonomousWorkflowStageResponseEvaluation",
    "evaluate_autonomous_workflow_stage_response",
    "validate_autonomous_workflow_stage_response_evaluation",
    "replay_autonomous_workflow_stage_response_evaluation",
]
