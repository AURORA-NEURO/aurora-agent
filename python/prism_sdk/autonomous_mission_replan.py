"""Durable, evaluator-guided replanning for model-authored missions.

The ordinary Python mission learner already supports bounded evaluator feedback, but its complete
attempt loop is process-local.  This module adds the missing application boundary: a mission
attempt can be checkpointed before evaluator settlement, after settlement, or at a terminal stop,
then resumed with caller-owned mission/provider values.  Checkpoints contain only identities,
digests, counters, and bounded evaluator projections.  Mission arguments, prompts, provider
responses, credentials, evaluator instructions, and tool outputs never cross the persistence
boundary.

This is intentionally built on :class:`AutonomousBrain.run_adaptive_mission`; it does not create
a second provider selector or a second mission authorizer.  Replanning may change the transient
prompt context only.  The caller's mission policy, model candidates, opaque credential handles,
tool allow-list, dispatch approval, and effect boundary remain authoritative for every attempt.
"""

from __future__ import annotations

from dataclasses import dataclass
import hashlib
import json
import math
import re
import threading
from typing import Any, Callable, Mapping, Protocol, Sequence

from .authoring import canonical_json, content_digest
from .autonomous_prompt_learning import AutonomousPromptAdaptiveSelection
from .errors import ArgumentError
from .brain import (
    AutonomousBrain,
    BrainEvaluatorDecision,
    BrainLearningLedger,
    BrainMissionResult,
    BrainOutcomeEvaluator,
    BrainRunError,
)
from .llm_runtime import CredentialHandle, ModelCandidate, ProviderInvocationObserver
from .mission import MissionPolicy


AUTONOMOUS_MISSION_REPLAN_SCHEMA = "bioprism-python-autonomous-mission-replan/0.1"
AUTONOMOUS_MISSION_REPLAN_CHECKPOINT_SCHEMA = "bioprism-python-autonomous-mission-replan-checkpoint/0.1"
AUTONOMOUS_MISSION_REPLAN_STATE_SCHEMA = "bioprism-python-autonomous-mission-replan-state/0.1"
AUTONOMOUS_MISSION_REPLAN_SNAPSHOT_SCHEMA = "bioprism-python-autonomous-mission-replan-snapshot/0.1"
AUTONOMOUS_MISSION_REPLAN_MAX_REPLANS = 3
AUTONOMOUS_MISSION_REPLAN_MAX_ATTEMPTS = AUTONOMOUS_MISSION_REPLAN_MAX_REPLANS + 1
AUTONOMOUS_MISSION_REPLAN_MAX_INSTRUCTION_BYTES = 8_192
AUTONOMOUS_MISSION_REPLAN_MAX_ATTEMPTS_IN_STATE = 4
AUTONOMOUS_MISSION_REPLAN_MAX_EVALUATIONS_IN_STATE = 4
AUTONOMOUS_MISSION_REPLAN_MAX_STATE_BYTES = 256_000
AUTONOMOUS_MISSION_REPLAN_MAX_SNAPSHOT_BYTES = 8_000_000

_RETENTION = "metadata_only_no_arguments_outputs_credentials_provider_material_or_raw_instructions"
_RESULT_RETENTION = "provider_results_caller_owned;replan_instructions_transient;value_only_projection"
_SECRET_MATERIAL = "never_returned"
_PHASES = frozenset({"execution_pending", "evaluation_pending", "replan_handoff", "terminal"})
_CHECKPOINT_PHASES = frozenset({"execution_pending", "evaluation_recorded", "replan_scheduled", "terminal"})
_SECRET_MARKER = re.compile(
    r"(?:api[_-]?key|authorization|bearer|credential|password|private[_-]?key|"
    r"access[_-]?token|refresh[_-]?token|secret|transcript|provider[_-]?response|"
    r"tool[_-]?argument|raw[_-]?instruction)",
    re.IGNORECASE,
)
_MODEL_QUALITY_FIELDS = frozenset(
    {
        "status",
        "error_class",
        "provider",
        "model",
        "domain",
        "capability",
        "risk_class",
        "evaluator_id",
        "evaluator_version",
        "reward",
        "passed",
        "outcome_digest",
        "evidence_digest",
        "feedback_digest",
        "health_record_digest",
        "replayed",
        "retention",
        "secret_material",
        "prompt_learning",
    }
)
_MODEL_QUALITY_DIGEST_FIELDS = frozenset(
    {"outcome_digest", "evidence_digest", "feedback_digest", "health_record_digest"}
)


def _identifier(name: str, value: Any, *, maximum: int = 256) -> str:
    if not isinstance(value, str) or not value.strip() or "\x00" in value:
        raise BrainRunError(f"{name} must be a bounded non-empty identifier")
    if len(value.encode("utf-8")) > maximum or not re.fullmatch(r"[A-Za-z0-9_.:+-]+", value):
        raise BrainRunError(f"{name} must be a bounded identifier")
    return value


def _digest(name: str, value: Any, *, allow_none: bool = False) -> str | None:
    if allow_none and value is None:
        return None
    if not isinstance(value, str) or not re.fullmatch(r"[0-9a-f]{64}", value):
        raise BrainRunError(f"{name} must be a lowercase SHA-256 digest")
    return value


def _bounded_count(name: str, value: Any, maximum: int) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or not 0 <= value <= maximum:
        raise BrainRunError(f"{name} must be within [0, {maximum}]")
    return value


def _safe_metadata(value: Any, *, label: str, depth: int = 0) -> None:
    if depth > 10:
        raise BrainRunError(f"{label} is too deeply nested")
    if isinstance(value, Mapping):
        if len(value) > 512:
            raise BrainRunError(f"{label} contains too many metadata fields")
        for key, child in value.items():
            if not isinstance(key, str) or not key.strip():
                raise BrainRunError(f"{label} contains an invalid metadata key")
            _safe_metadata(child, label=label, depth=depth + 1)
        return
    if isinstance(value, (list, tuple)):
        if len(value) > 512:
            raise BrainRunError(f"{label} contains too many metadata items")
        for child in value:
            _safe_metadata(child, label=label, depth=depth + 1)
        return
    if isinstance(value, (bytes, bytearray)):
        raise BrainRunError(f"{label} cannot contain binary data")
    if isinstance(value, float) and not (-float("inf") < value < float("inf")):
        raise BrainRunError(f"{label} contains a non-finite number")
    if value is not None and not isinstance(value, (str, int, float, bool)):
        raise BrainRunError(f"{label} is not JSON-safe")


def _metadata_digest(value: Any, *, label: str) -> str:
    _safe_metadata(value, label=label)
    try:
        encoded = canonical_json(value).encode("utf-8")
    except (TypeError, ValueError) as error:
        raise BrainRunError(f"{label} is not canonical JSON") from error
    return hashlib.sha256(encoded).hexdigest()


def _private_shape_free(value: Mapping[str, Any], *, label: str) -> None:
    encoded = canonical_json(value)
    marker_free = (
        encoded
        .replace(_RETENTION, "")
        .replace(_RESULT_RETENTION, "")
        .replace('"secret_material":', "")
        .replace(f'"{_SECRET_MATERIAL}"', "")
    )
    if _SECRET_MARKER.search(marker_free):
        raise BrainRunError(f"{label} contains private or payload-shaped material")


def _prompt_learning_projection(value: Any) -> dict[str, Any]:
    """Normalize registry-bound prompt choices without retaining rendered prompt content."""

    if not isinstance(value, Mapping):
        raise BrainRunError("mission prompt learning projection must be a mapping")
    expected = {
        "selection_count",
        "selection_digests",
        "selections",
        "retention",
        "secret_material",
    }
    if set(value) != expected:
        raise BrainRunError("mission prompt learning projection fields are invalid")
    if value.get("retention") != "selection_metadata_only;rendered_messages_transient":
        raise BrainRunError("mission prompt learning projection retention is invalid")
    if value.get("secret_material") != _SECRET_MATERIAL:
        raise BrainRunError("mission prompt learning projection secret marker is invalid")
    selection_count = value.get("selection_count")
    if isinstance(selection_count, bool) or not isinstance(selection_count, int) or not 0 <= selection_count <= 128:
        raise BrainRunError("mission prompt learning selection_count is outside its bound")
    raw_digests = value.get("selection_digests")
    raw_selections = value.get("selections")
    if (
        not isinstance(raw_digests, Sequence)
        or isinstance(raw_digests, (str, bytes))
        or not isinstance(raw_selections, Sequence)
        or isinstance(raw_selections, (str, bytes))
        or len(raw_digests) != selection_count
        or len(raw_selections) != selection_count
    ):
        raise BrainRunError("mission prompt learning projection selections are malformed")
    selections: list[dict[str, Any]] = []
    digests: list[str] = []
    for index, raw in enumerate(raw_selections):
        if not isinstance(raw, Mapping):
            raise BrainRunError(f"mission prompt learning selection {index} is malformed")
        try:
            selection = AutonomousPromptAdaptiveSelection.from_dict(raw)
        except (ArgumentError, BrainRunError, TypeError, ValueError) as error:
            raise BrainRunError(f"mission prompt learning selection {index} is invalid") from error
        normalized = selection.to_dict()
        selections.append(normalized)
        digests.append(selection.selection_digest)
    if list(raw_digests) != digests:
        raise BrainRunError("mission prompt learning selection digests do not match selections")
    return {
        "selection_count": selection_count,
        "selection_digests": digests,
        "selections": selections,
        "retention": value["retention"],
        "secret_material": value["secret_material"],
    }


def _model_quality_projection(value: Mapping[str, Any]) -> dict[str, Any]:
    """Allow only the bounded fields that a health settlement can persist."""

    if not isinstance(value, Mapping):
        raise BrainRunError("mission model quality callback must return a mapping or None")
    unknown = [key for key in value if not isinstance(key, str) or key not in _MODEL_QUALITY_FIELDS]
    if unknown:
        raise BrainRunError("mission model quality projection contains unsupported fields")
    _safe_metadata(value, label="mission model quality projection")
    _private_shape_free(value, label="mission model quality projection")
    normalized = dict(value)
    for field, item in value.items():
        if field == "prompt_learning":
            normalized[field] = _prompt_learning_projection(item)
        elif field in _MODEL_QUALITY_DIGEST_FIELDS:
            if item is not None and not isinstance(item, str):
                raise BrainRunError(f"mission model quality {field} must be a digest or None")
            if item is not None and not re.fullmatch(r"[0-9a-f]{64}", item):
                raise BrainRunError(f"mission model quality {field} must be a lowercase SHA-256 digest")
        elif field in {"reward"}:
            if isinstance(item, bool) or not isinstance(item, (int, float)) or not math.isfinite(float(item)):
                raise BrainRunError("mission model quality reward must be finite")
            if not 0.0 <= float(item) <= 1.0:
                raise BrainRunError("mission model quality reward must be within [0, 1]")
        elif field in {"passed", "replayed"}:
            if not isinstance(item, bool):
                raise BrainRunError(f"mission model quality {field} must be boolean")
        elif not isinstance(item, str) or not item.strip() or "\x00" in item or len(item.encode("utf-8")) > 512:
            raise BrainRunError(f"mission model quality {field} must be bounded text")
    return normalized


def _screen_instruction(value: Any) -> str:
    if not isinstance(value, str) or not value.strip() or "\x00" in value:
        raise BrainRunError("mission replan instruction must be bounded text")
    if len(value.encode("utf-8")) > AUTONOMOUS_MISSION_REPLAN_MAX_INSTRUCTION_BYTES:
        raise BrainRunError("mission replan instruction exceeds its byte budget")
    if re.search(
        r"(?:api[_-]?key|authorization|bearer|credential|password|private[_-]?key|"
        r"access[_-]?token|refresh[_-]?token|secret|gsk_|sk-)",
        value,
        re.IGNORECASE,
    ):
        raise BrainRunError("mission replan instruction contains credential-shaped material")
    return value


def _result_outcome_digest(result: BrainMissionResult) -> str:
    if not isinstance(result, BrainMissionResult):
        raise BrainRunError("mission replan result must be a BrainMissionResult")
    mission = result.mission if isinstance(result.mission, Mapping) else None
    execution = result.execution if isinstance(result.execution, Mapping) else None
    return content_digest(
        {
            "run_id": result.brain_run.run_id,
            "brain_outcome_digest": result.brain_run.outcome_digest,
            "status": result.status,
            "mission_digest": None if mission is None else _metadata_digest(mission, label="mission result"),
            "execution_status": None if execution is None else execution.get("mission_status", execution.get("status")),
        }
    )


def _mission_identifier(result: BrainMissionResult, outcome_digest: str) -> str:
    mission = result.mission if isinstance(result.mission, Mapping) else None
    value = None if mission is None else mission.get("mission_id")
    if isinstance(value, str) and re.fullmatch(r"[A-Za-z0-9_.:+-]{1,256}", value):
        return value
    return "mission-" + outcome_digest[:24]


def _selection_digest(result: BrainMissionResult) -> str:
    return _metadata_digest(dict(result.brain_run.selection), label="mission selection")


def _route_digest(result: BrainMissionResult) -> str | None:
    route = result.route
    if not isinstance(route, Mapping):
        return None
    value = route.get("route_digest")
    return value if isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) else _metadata_digest(route, label="mission route")


def _counts(result: BrainMissionResult) -> tuple[int, int, int]:
    execution = result.execution if isinstance(result.execution, Mapping) else result.preflight
    if not isinstance(execution, Mapping):
        return (0, 0, 0)
    raw_results = execution.get("results", [])
    if not isinstance(raw_results, Sequence) or isinstance(raw_results, (str, bytes)):
        raw_results = []
    completed = sum(1 for item in raw_results if isinstance(item, Mapping) and item.get("status") in {"succeeded", "completed"})
    succeeded = sum(1 for item in raw_results if isinstance(item, Mapping) and item.get("status") == "succeeded")
    failed = sum(1 for item in raw_results if isinstance(item, Mapping) and item.get("status") in {"failed", "refused", "blocked", "cancelled"})
    return completed, succeeded, failed


def _evaluation_projection(
    decision: BrainEvaluatorDecision,
    *,
    model_quality: Mapping[str, Any] | None = None,
) -> dict[str, Any]:
    instruction = decision.replan_instruction
    projection = {
        "evaluator_id": decision.evaluator_id,
        "evaluator_version": decision.evaluator_version,
        "reward": decision.reward,
        "passed": decision.passed,
        "failed": decision.failed,
        "feedback_digest": decision.feedback_digest,
        "failure_class": decision.failure_class,
        "evidence_digest": decision.evidence_digest,
        "replan_requested": decision.replan_requested,
        "replan_instruction_digest": None if instruction is None else content_digest(instruction),
        "retention": "evaluator_values_and_digests_only",
        "secret_material": _SECRET_MATERIAL,
    }
    if model_quality is not None:
        projection["model_quality"] = _model_quality_projection(model_quality)
    projection["evaluation_digest"] = content_digest(projection)
    _private_shape_free(projection, label="mission evaluator projection")
    return projection


@dataclass(frozen=True, slots=True)
class AutonomousMissionReplanAttempt:
    """Value-only metadata for one mission proposal/execution attempt."""

    attempt: int
    mission_id: str
    status: str
    completed_steps: int
    succeeded_steps: int
    failed_steps: int
    selection_digest: str
    outcome_digest: str
    evaluation_digest: str | None
    replan_instruction_digest: str | None
    route_digest: str | None = None

    def __post_init__(self) -> None:
        _bounded_count("mission replan attempt", self.attempt, AUTONOMOUS_MISSION_REPLAN_MAX_ATTEMPTS)
        _identifier("mission replan attempt mission_id", self.mission_id)
        _identifier("mission replan attempt status", self.status)
        for name, value in (
            ("completed_steps", self.completed_steps),
            ("succeeded_steps", self.succeeded_steps),
            ("failed_steps", self.failed_steps),
        ):
            _bounded_count(f"mission replan attempt {name}", value, 1_000_000)
        _digest("mission replan attempt selection_digest", self.selection_digest)
        _digest("mission replan attempt outcome_digest", self.outcome_digest)
        _digest("mission replan attempt evaluation_digest", self.evaluation_digest, allow_none=True)
        _digest("mission replan attempt replan_instruction_digest", self.replan_instruction_digest, allow_none=True)
        _digest("mission replan attempt route_digest", self.route_digest, allow_none=True)

    def to_dict(self) -> dict[str, Any]:
        return {
            "attempt": self.attempt,
            "mission_id": self.mission_id,
            "status": self.status,
            "completed_steps": self.completed_steps,
            "succeeded_steps": self.succeeded_steps,
            "failed_steps": self.failed_steps,
            "selection_digest": self.selection_digest,
            "outcome_digest": self.outcome_digest,
            "evaluation_digest": self.evaluation_digest,
            "replan_instruction_digest": self.replan_instruction_digest,
            "route_digest": self.route_digest,
        }

    @classmethod
    def from_mapping(cls, value: Mapping[str, Any]) -> "AutonomousMissionReplanAttempt":
        expected = {
            "attempt", "mission_id", "status", "completed_steps", "succeeded_steps", "failed_steps",
            "selection_digest", "outcome_digest", "evaluation_digest", "replan_instruction_digest", "route_digest",
        }
        if set(value) != expected:
            raise BrainRunError("mission replan attempt contains unsupported or missing fields")
        return cls(**dict(value))


@dataclass(frozen=True, slots=True)
class AutonomousMissionReplanCheckpoint:
    """Metadata-only attempt-boundary checkpoint."""

    root_mission_id: str
    protected_contract_digest: str
    attempt: int
    phase: str
    mission_id: str
    outcome_digest: str | None
    evaluation_digest: str | None
    replan_instruction_digest: str | None
    bandit_state_digest: str
    checkpoint_digest: str = ""
    schema: str = AUTONOMOUS_MISSION_REPLAN_CHECKPOINT_SCHEMA
    retention: str = _RETENTION
    secret_material: str = _SECRET_MATERIAL

    def __post_init__(self) -> None:
        if self.schema != AUTONOMOUS_MISSION_REPLAN_CHECKPOINT_SCHEMA or self.retention != _RETENTION or self.secret_material != _SECRET_MATERIAL:
            raise BrainRunError("mission replan checkpoint retention markers are invalid")
        _identifier("mission replan checkpoint root_mission_id", self.root_mission_id)
        _digest("mission replan checkpoint protected_contract_digest", self.protected_contract_digest)
        _bounded_count("mission replan checkpoint attempt", self.attempt, AUTONOMOUS_MISSION_REPLAN_MAX_ATTEMPTS)
        if self.phase not in _CHECKPOINT_PHASES:
            raise BrainRunError("mission replan checkpoint phase is invalid")
        _identifier("mission replan checkpoint mission_id", self.mission_id)
        _digest("mission replan checkpoint outcome_digest", self.outcome_digest, allow_none=True)
        _digest("mission replan checkpoint evaluation_digest", self.evaluation_digest, allow_none=True)
        _digest("mission replan checkpoint replan_instruction_digest", self.replan_instruction_digest, allow_none=True)
        _digest("mission replan checkpoint bandit_state_digest", self.bandit_state_digest)
        supplied = None if not self.checkpoint_digest else _digest("mission replan checkpoint checkpoint_digest", self.checkpoint_digest)
        computed = content_digest(self._descriptor())
        if supplied is not None and supplied != computed:
            raise BrainRunError("mission replan checkpoint digest does not match its metadata")
        object.__setattr__(self, "checkpoint_digest", computed)
        _private_shape_free(self._descriptor(), label="mission replan checkpoint")

    def _descriptor(self) -> dict[str, Any]:
        return {
            "schema": self.schema,
            "root_mission_id": self.root_mission_id,
            "protected_contract_digest": self.protected_contract_digest,
            "attempt": self.attempt,
            "phase": self.phase,
            "mission_id": self.mission_id,
            "outcome_digest": self.outcome_digest,
            "evaluation_digest": self.evaluation_digest,
            "replan_instruction_digest": self.replan_instruction_digest,
            "bandit_state_digest": self.bandit_state_digest,
            "retention": self.retention,
            "secret_material": self.secret_material,
        }

    def to_dict(self) -> dict[str, Any]:
        return {**self._descriptor(), "checkpoint_digest": self.checkpoint_digest}

    @classmethod
    def from_mapping(cls, value: Mapping[str, Any]) -> "AutonomousMissionReplanCheckpoint":
        expected = {
            "schema", "root_mission_id", "protected_contract_digest", "attempt", "phase", "mission_id",
            "outcome_digest", "evaluation_digest", "replan_instruction_digest", "bandit_state_digest",
            "checkpoint_digest", "retention", "secret_material",
        }
        if not isinstance(value, Mapping) or set(value) != expected:
            raise BrainRunError("mission replan checkpoint contains unsupported or missing fields")
        return cls(**dict(value))


@dataclass(frozen=True, slots=True)
class AutonomousMissionReplanState:
    """Hash-chained durable orchestration state; no mission payload is retained."""

    root_mission_id: str
    protected_contract_digest: str
    max_replans: int
    attempt: int
    phase: str
    current_mission_id: str
    bandit_state_digest: str
    outcome_digest: str | None
    evaluation_digest: str | None
    replan_instruction_digest: str | None
    attempts: tuple[AutonomousMissionReplanAttempt, ...]
    evaluations: tuple[Mapping[str, Any], ...]
    last_checkpoint_digest: str | None
    terminal_status: str | None
    generation: int
    previous_state_digest: str | None
    state_digest: str = ""
    schema: str = AUTONOMOUS_MISSION_REPLAN_STATE_SCHEMA
    retention: str = _RETENTION
    secret_material: str = _SECRET_MATERIAL

    def __post_init__(self) -> None:
        if self.schema != AUTONOMOUS_MISSION_REPLAN_STATE_SCHEMA or self.retention != _RETENTION or self.secret_material != _SECRET_MATERIAL:
            raise BrainRunError("mission replan state retention markers are invalid")
        _identifier("mission replan state root_mission_id", self.root_mission_id)
        _digest("mission replan state protected_contract_digest", self.protected_contract_digest)
        _bounded_count("mission replan state max_replans", self.max_replans, AUTONOMOUS_MISSION_REPLAN_MAX_REPLANS)
        _bounded_count("mission replan state attempt", self.attempt, AUTONOMOUS_MISSION_REPLAN_MAX_ATTEMPTS)
        if self.phase not in _PHASES:
            raise BrainRunError("mission replan state phase is invalid")
        _identifier("mission replan state current_mission_id", self.current_mission_id)
        _digest("mission replan state bandit_state_digest", self.bandit_state_digest)
        _digest("mission replan state outcome_digest", self.outcome_digest, allow_none=True)
        _digest("mission replan state evaluation_digest", self.evaluation_digest, allow_none=True)
        _digest("mission replan state replan_instruction_digest", self.replan_instruction_digest, allow_none=True)
        _digest("mission replan state last_checkpoint_digest", self.last_checkpoint_digest, allow_none=True)
        if self.terminal_status is not None:
            _identifier("mission replan state terminal_status", self.terminal_status)
        attempts = tuple(self.attempts)
        if len(attempts) > AUTONOMOUS_MISSION_REPLAN_MAX_ATTEMPTS_IN_STATE or any(not isinstance(item, AutonomousMissionReplanAttempt) for item in attempts):
            raise BrainRunError("mission replan state attempts exceed their bound")
        if tuple(item.attempt for item in attempts) != tuple(range(1, len(attempts) + 1)):
            raise BrainRunError("mission replan state attempts must be contiguous")
        evaluations = tuple(dict(item) for item in self.evaluations)
        if len(evaluations) > AUTONOMOUS_MISSION_REPLAN_MAX_EVALUATIONS_IN_STATE:
            raise BrainRunError("mission replan state evaluations exceed their bound")
        for evaluation in evaluations:
            _safe_metadata(evaluation, label="mission replan state evaluation")
            _private_shape_free(evaluation, label="mission replan state evaluation")
        if self.phase == "terminal" and self.terminal_status is None:
            raise BrainRunError("terminal mission replan state requires terminal_status")
        if self.phase != "terminal" and self.terminal_status is not None:
            raise BrainRunError("non-terminal mission replan state cannot contain terminal_status")
        if self.phase in {"evaluation_pending", "replan_handoff", "terminal"} and self.outcome_digest is None:
            raise BrainRunError("mission replan state phase requires an outcome digest")
        if self.phase in {"replan_handoff", "terminal"} and self.evaluation_digest is None:
            raise BrainRunError("mission replan state phase requires an evaluation digest")
        if self.phase == "replan_handoff" and self.replan_instruction_digest is None:
            raise BrainRunError("mission replan handoff requires an instruction digest")
        if self.phase == "execution_pending" and self.attempt == 0 and self.replan_instruction_digest is not None:
            raise BrainRunError("initial execution-pending mission replan state cannot retain retry instruction")
        if isinstance(self.generation, bool) or not isinstance(self.generation, int) or not 1 <= self.generation <= 9_007_199_254_740_991:
            raise BrainRunError("mission replan state generation is outside its bound")
        _digest("mission replan state previous_state_digest", self.previous_state_digest, allow_none=True)
        if (self.generation == 1) != (self.previous_state_digest is None):
            raise BrainRunError("mission replan state hash chain is malformed")
        object.__setattr__(self, "attempts", attempts)
        object.__setattr__(self, "evaluations", evaluations)
        descriptor = self._descriptor()
        encoded = canonical_json(descriptor).encode("utf-8")
        if len(encoded) > AUTONOMOUS_MISSION_REPLAN_MAX_STATE_BYTES:
            raise BrainRunError("mission replan state exceeds its metadata budget")
        _private_shape_free(descriptor, label="mission replan state")
        computed = content_digest(descriptor)
        if self.state_digest and self.state_digest != computed:
            raise BrainRunError("mission replan state digest does not match its metadata")
        object.__setattr__(self, "state_digest", computed)

    def _descriptor(self) -> dict[str, Any]:
        return {
            "schema": self.schema,
            "root_mission_id": self.root_mission_id,
            "protected_contract_digest": self.protected_contract_digest,
            "max_replans": self.max_replans,
            "attempt": self.attempt,
            "phase": self.phase,
            "current_mission_id": self.current_mission_id,
            "bandit_state_digest": self.bandit_state_digest,
            "outcome_digest": self.outcome_digest,
            "evaluation_digest": self.evaluation_digest,
            "replan_instruction_digest": self.replan_instruction_digest,
            "attempts": [item.to_dict() for item in self.attempts],
            "evaluations": [dict(item) for item in self.evaluations],
            "last_checkpoint_digest": self.last_checkpoint_digest,
            "terminal_status": self.terminal_status,
            "generation": self.generation,
            "previous_state_digest": self.previous_state_digest,
            "retention": self.retention,
            "secret_material": self.secret_material,
        }

    def to_dict(self) -> dict[str, Any]:
        return {**self._descriptor(), "state_digest": self.state_digest}

    @classmethod
    def from_mapping(cls, value: Mapping[str, Any]) -> "AutonomousMissionReplanState":
        expected = {
            "schema", "root_mission_id", "protected_contract_digest", "max_replans", "attempt", "phase",
            "current_mission_id", "bandit_state_digest", "outcome_digest", "evaluation_digest",
            "replan_instruction_digest", "attempts", "evaluations", "last_checkpoint_digest", "terminal_status",
            "generation", "previous_state_digest", "state_digest", "retention", "secret_material",
        }
        if not isinstance(value, Mapping) or set(value) != expected:
            raise BrainRunError("mission replan state contains unsupported or missing fields")
        raw_attempts = value.get("attempts")
        raw_evaluations = value.get("evaluations")
        if not isinstance(raw_attempts, Sequence) or isinstance(raw_attempts, (str, bytes)) or not isinstance(raw_evaluations, Sequence) or isinstance(raw_evaluations, (str, bytes)):
            raise BrainRunError("mission replan state attempts/evaluations must be sequences")
        if any(not isinstance(item, Mapping) for item in raw_evaluations):
            raise BrainRunError("mission replan state evaluations must contain mappings")
        return cls(
            root_mission_id=value.get("root_mission_id"),
            protected_contract_digest=value.get("protected_contract_digest"),
            max_replans=value.get("max_replans"),
            attempt=value.get("attempt"),
            phase=value.get("phase"),
            current_mission_id=value.get("current_mission_id"),
            bandit_state_digest=value.get("bandit_state_digest"),
            outcome_digest=value.get("outcome_digest"),
            evaluation_digest=value.get("evaluation_digest"),
            replan_instruction_digest=value.get("replan_instruction_digest"),
            attempts=tuple(AutonomousMissionReplanAttempt.from_mapping(item) for item in raw_attempts),
            evaluations=tuple(dict(item) for item in raw_evaluations),
            last_checkpoint_digest=value.get("last_checkpoint_digest"),
            terminal_status=value.get("terminal_status"),
            generation=value.get("generation"),
            previous_state_digest=value.get("previous_state_digest"),
            state_digest=value.get("state_digest"),
            schema=value.get("schema"),
            retention=value.get("retention"),
            secret_material=value.get("secret_material"),
        )


@dataclass(frozen=True, slots=True)
class AutonomousMissionReplanSnapshot:
    states: tuple[AutonomousMissionReplanState, ...]
    snapshot_digest: str = ""
    schema: str = AUTONOMOUS_MISSION_REPLAN_SNAPSHOT_SCHEMA
    retention: str = "metadata_only_hash_bound"
    secret_material: str = _SECRET_MATERIAL

    def __post_init__(self) -> None:
        if self.schema != AUTONOMOUS_MISSION_REPLAN_SNAPSHOT_SCHEMA or self.retention != "metadata_only_hash_bound" or self.secret_material != _SECRET_MATERIAL:
            raise BrainRunError("mission replan snapshot retention markers are invalid")
        states = tuple(self.states)
        if len(states) > 8_192 or any(not isinstance(state, AutonomousMissionReplanState) for state in states):
            raise BrainRunError("mission replan snapshot exceeds its state capacity")
        if len({state.root_mission_id for state in states}) != len(states):
            raise BrainRunError("mission replan snapshot contains duplicate root mission IDs")
        object.__setattr__(self, "states", states)
        descriptor = {
            "schema": self.schema,
            "states": [state.to_dict() for state in states],
            "retention": self.retention,
            "secret_material": self.secret_material,
        }
        computed = content_digest(descriptor)
        supplied = None if not self.snapshot_digest else _digest("mission replan snapshot snapshot_digest", self.snapshot_digest)
        if supplied is not None and supplied != computed:
            raise BrainRunError("mission replan snapshot digest does not match its metadata")
        object.__setattr__(self, "snapshot_digest", computed)

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": self.schema,
            "states": [state.to_dict() for state in self.states],
            "retention": self.retention,
            "secret_material": self.secret_material,
            "snapshot_digest": self.snapshot_digest,
        }

    @classmethod
    def from_mapping(cls, value: Mapping[str, Any]) -> "AutonomousMissionReplanSnapshot":
        expected = {"schema", "states", "retention", "secret_material", "snapshot_digest"}
        if not isinstance(value, Mapping) or set(value) != expected:
            raise BrainRunError("mission replan snapshot contains unsupported or missing fields")
        raw_states = value.get("states")
        if not isinstance(raw_states, Sequence) or isinstance(raw_states, (str, bytes)):
            raise BrainRunError("mission replan snapshot states must be a sequence")
        return cls(
            states=tuple(AutonomousMissionReplanState.from_mapping(item) for item in raw_states),
            snapshot_digest=value.get("snapshot_digest"),
            schema=value.get("schema"),
            retention=value.get("retention"),
            secret_material=value.get("secret_material"),
        )


class AutonomousMissionReplanStateStore(Protocol):
    def load(self, root_mission_id: str) -> AutonomousMissionReplanState | Mapping[str, Any] | None: ...
    def save(self, state: AutonomousMissionReplanState | Mapping[str, Any]) -> None: ...


class AutonomousMissionReplanSnapshotPersistence(Protocol):
    def read(self) -> AutonomousMissionReplanSnapshot | Mapping[str, Any] | None: ...
    def write(self, snapshot: AutonomousMissionReplanSnapshot | Mapping[str, Any]) -> None: ...


class AutonomousMissionReplanTextStore(Protocol):
    def read(self) -> str | None: ...
    def write(self, value: str) -> None: ...


class InMemoryAutonomousMissionReplanStateStore:
    """Thread-safe reference store for tests and embedders without a persistence adapter."""

    def __init__(self, *, max_states: int = 8_192) -> None:
        if isinstance(max_states, bool) or not isinstance(max_states, int) or not 1 <= max_states <= 8_192:
            raise BrainRunError("mission replan state store max_states is outside its bound")
        self.max_states = max_states
        self._states: dict[str, AutonomousMissionReplanState] = {}
        self._lock = threading.RLock()

    def load(self, root_mission_id: str) -> AutonomousMissionReplanState | None:
        root_mission_id = _identifier("mission replan root_mission_id", root_mission_id)
        with self._lock:
            return self._states.get(root_mission_id)

    def save(self, state: AutonomousMissionReplanState | Mapping[str, Any]) -> None:
        normalized = state if isinstance(state, AutonomousMissionReplanState) else AutonomousMissionReplanState.from_mapping(state)
        with self._lock:
            prior = self._states.get(normalized.root_mission_id)
            if prior is not None and prior.state_digest == normalized.state_digest:
                return
            if prior is None and (normalized.generation != 1 or normalized.previous_state_digest is not None):
                raise BrainRunError("initial mission replan state must start at generation one")
            if prior is not None and (normalized.generation != prior.generation + 1 or normalized.previous_state_digest != prior.state_digest):
                raise BrainRunError("mission replan state generation chain is not contiguous")
            if prior is None and len(self._states) >= self.max_states:
                raise BrainRunError("mission replan state store is full")
            self._states[normalized.root_mission_id] = normalized

    def snapshot(self) -> AutonomousMissionReplanSnapshot:
        with self._lock:
            states = tuple(self._states[key] for key in sorted(self._states))
        return AutonomousMissionReplanSnapshot(states=states)

    def restore(self, snapshot: AutonomousMissionReplanSnapshot | Mapping[str, Any]) -> None:
        normalized = snapshot if isinstance(snapshot, AutonomousMissionReplanSnapshot) else AutonomousMissionReplanSnapshot.from_mapping(snapshot)
        if len(normalized.states) > self.max_states:
            raise BrainRunError("mission replan snapshot exceeds max_states")
        with self._lock:
            self._states = {state.root_mission_id: state for state in normalized.states}


class JsonAutonomousMissionReplanSnapshotPersistence:
    """Canonical JSON persistence for a caller-owned mission replan snapshot store."""

    def __init__(self, store: AutonomousMissionReplanTextStore, *, max_bytes: int = AUTONOMOUS_MISSION_REPLAN_MAX_SNAPSHOT_BYTES) -> None:
        if not all(callable(getattr(store, name, None)) for name in ("read", "write")):
            raise BrainRunError("mission replan JSON persistence requires a text store")
        if isinstance(max_bytes, bool) or not isinstance(max_bytes, int) or not 1 <= max_bytes <= AUTONOMOUS_MISSION_REPLAN_MAX_SNAPSHOT_BYTES:
            raise BrainRunError("mission replan JSON max_bytes is outside its bound")
        self.store = store
        self.max_bytes = max_bytes

    def read(self) -> AutonomousMissionReplanSnapshot | None:
        encoded = self.store.read()
        if encoded is None:
            return None
        if not isinstance(encoded, str) or len(encoded.encode("utf-8")) > self.max_bytes:
            raise BrainRunError("mission replan JSON exceeds its byte bound")
        try:
            raw = json.loads(encoded)
        except json.JSONDecodeError as error:
            raise BrainRunError("mission replan JSON is invalid") from error
        if not isinstance(raw, Mapping):
            raise BrainRunError("mission replan JSON must be an object")
        snapshot = AutonomousMissionReplanSnapshot.from_mapping(raw)
        if encoded != canonical_json(snapshot.to_dict()):
            raise BrainRunError("mission replan JSON is not canonical")
        return snapshot

    def write(self, snapshot: AutonomousMissionReplanSnapshot | Mapping[str, Any]) -> None:
        normalized = snapshot if isinstance(snapshot, AutonomousMissionReplanSnapshot) else AutonomousMissionReplanSnapshot.from_mapping(snapshot)
        encoded = canonical_json(normalized.to_dict())
        if len(encoded.encode("utf-8")) > self.max_bytes:
            raise BrainRunError("mission replan JSON exceeds its byte bound")
        self.store.write(encoded)


class AutonomousMissionReplanPersistenceCoordinator:
    """Flush and restore mission replan states through a caller-owned snapshot adapter."""

    def __init__(self, store: InMemoryAutonomousMissionReplanStateStore | AutonomousMissionReplanStateStore, persistence: AutonomousMissionReplanSnapshotPersistence) -> None:
        if not all(callable(getattr(store, name, None)) for name in ("load", "save", "snapshot", "restore")):
            raise BrainRunError("mission replan persistence requires a complete state store")
        if not all(callable(getattr(persistence, name, None)) for name in ("read", "write")):
            raise BrainRunError("mission replan persistence adapter is malformed")
        self.store = store
        self.persistence = persistence

    def flush(self) -> AutonomousMissionReplanSnapshot:
        snapshot = self.store.snapshot()
        self.persistence.write(snapshot)
        return snapshot

    def restore(self) -> AutonomousMissionReplanSnapshot | None:
        snapshot = self.persistence.read()
        if snapshot is None:
            return None
        normalized = snapshot if isinstance(snapshot, AutonomousMissionReplanSnapshot) else AutonomousMissionReplanSnapshot.from_mapping(snapshot)
        self.store.restore(normalized)
        return normalized


@dataclass(frozen=True, slots=True)
class AutonomousMissionReplanResult:
    """Safe public result; ``final_result`` remains caller-transient and is never serialized."""

    status: str
    root_mission_id: str
    protected_contract_digest: str
    attempts: tuple[AutonomousMissionReplanAttempt, ...]
    evaluations: tuple[Mapping[str, Any], ...]
    replan_count: int
    final_result: BrainMissionResult | None
    checkpoint: AutonomousMissionReplanCheckpoint | None
    schema: str = AUTONOMOUS_MISSION_REPLAN_SCHEMA
    retention: str = _RESULT_RETENTION
    secret_material: str = _SECRET_MATERIAL

    def __post_init__(self) -> None:
        if self.schema != AUTONOMOUS_MISSION_REPLAN_SCHEMA or self.retention != _RESULT_RETENTION or self.secret_material != _SECRET_MATERIAL:
            raise BrainRunError("mission replan result retention markers are invalid")
        _identifier("mission replan result root_mission_id", self.root_mission_id)
        _digest("mission replan result protected_contract_digest", self.protected_contract_digest)
        _bounded_count("mission replan result replan_count", self.replan_count, AUTONOMOUS_MISSION_REPLAN_MAX_REPLANS)
        attempts = tuple(self.attempts)
        if len(attempts) > AUTONOMOUS_MISSION_REPLAN_MAX_ATTEMPTS:
            raise BrainRunError("mission replan result attempts exceed their bound")
        if self.replan_count != max(0, len(attempts) - 1):
            raise BrainRunError("mission replan result replan_count does not match attempts")
        evaluations = tuple(dict(item) for item in self.evaluations)
        for evaluation in evaluations:
            _private_shape_free(evaluation, label="mission replan result evaluation")
        object.__setattr__(self, "attempts", attempts)
        object.__setattr__(self, "evaluations", evaluations)

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": self.schema,
            "status": self.status,
            "root_mission_id": self.root_mission_id,
            "protected_contract_digest": self.protected_contract_digest,
            "attempts": [attempt.to_dict() for attempt in self.attempts],
            "evaluations": [dict(evaluation) for evaluation in self.evaluations],
            "replan_count": self.replan_count,
            "checkpoint": None if self.checkpoint is None else self.checkpoint.to_dict(),
            "retention": self.retention,
            "secret_material": self.secret_material,
        }


@dataclass(frozen=True, slots=True)
class AutonomousMissionReplanRehydrationContext:
    root_mission_id: str
    protected_contract_digest: str
    attempt: int
    phase: str
    outcome_digest: str | None
    evaluation_digest: str | None
    replan_instruction_digest: str | None
    state_digest: str

    def to_dict(self) -> dict[str, Any]:
        return {
            "root_mission_id": self.root_mission_id,
            "protected_contract_digest": self.protected_contract_digest,
            "attempt": self.attempt,
            "phase": self.phase,
            "outcome_digest": self.outcome_digest,
            "evaluation_digest": self.evaluation_digest,
            "replan_instruction_digest": self.replan_instruction_digest,
            "state_digest": self.state_digest,
            "retention": _RETENTION,
            "secret_material": _SECRET_MATERIAL,
        }


def _insert_replan_chunk_into_override(
    prompt: Mapping[str, Any],
    chunk: Mapping[str, Any],
) -> dict[str, Any]:
    """Keep a rendered prompt override aligned with the metadata context on retries."""

    result = dict(prompt)
    override = result.get("_provider_messages_override")
    if override is None:
        return result
    if not isinstance(override, Mapping):
        raise BrainRunError("mission replan provider prompt override must be a mapping")
    chunk_id = chunk.get("id")
    content = chunk.get("content")
    if not isinstance(chunk_id, str) or not isinstance(content, str):
        raise BrainRunError("mission replan retry context chunk is malformed")
    raw_messages = override.get("messages")
    if (
        not isinstance(raw_messages, Sequence)
        or isinstance(raw_messages, (str, bytes))
        or not raw_messages
        or any(not isinstance(message, Mapping) for message in raw_messages)
    ):
        raise BrainRunError("mission replan provider prompt override messages are malformed")
    retry_message = {
        "role": "developer",
        "content": f"Context {chunk_id}:\n{content}",
    }
    rendered_messages = [dict(message) for message in raw_messages]
    last_user_index = max(
        (index for index, message in enumerate(rendered_messages) if message.get("role") == "user"),
        default=-1,
    )
    insertion_index = len(rendered_messages) if last_user_index < 0 else last_user_index
    rendered_messages.insert(insertion_index, retry_message)
    updated_override = dict(override)
    updated_override["messages"] = rendered_messages
    result["_provider_messages_override"] = updated_override
    return result


def _append_replan_context(prompt: Mapping[str, Any], *, attempt: int, result: BrainMissionResult, decision: BrainEvaluatorDecision) -> dict[str, Any]:
    if not isinstance(prompt, Mapping):
        raise BrainRunError("mission replan prompt must be a mapping")
    instruction = _screen_instruction(decision.replan_instruction)
    raw_context = prompt.get("context", [])
    if not isinstance(raw_context, Sequence) or isinstance(raw_context, (str, bytes)):
        raise BrainRunError("mission replan prompt context must be a sequence")
    chunks = [dict(chunk) for chunk in raw_context if isinstance(chunk, Mapping)]
    chunk_id = f"autonomy-mission-replan-{attempt}"
    if len(chunks) != len(raw_context) or any(chunk.get("id") == chunk_id for chunk in chunks):
        raise BrainRunError("mission replan prompt contains malformed or duplicate retry context")
    outcome_digest = _result_outcome_digest(result)
    chunks.append(
        {
            "id": chunk_id,
            "role": "developer",
            "content": json.dumps(
                {
                    "workflow": "bounded_autonomous_mission_replan",
                    "attempt": attempt,
                    "previous_outcome_digest": outcome_digest,
                    "evaluator_id": decision.evaluator_id,
                    "evaluator_version": decision.evaluator_version,
                    "reward": decision.reward,
                    "passed": decision.passed,
                    "failed": decision.failed,
                    "failure_class": decision.failure_class,
                    "instruction": instruction,
                    "guardrails": [
                        "This is bounded evaluator feedback, not authorization.",
                        "Preserve the caller mission policy, tools, credentials, budgets, and effect gates.",
                        "Do not claim an external effect from an unverified provider or tool result.",
                    ],
                },
                ensure_ascii=False,
                sort_keys=True,
                separators=(",", ":"),
            ),
            "required": True,
            "priority": 950,
        }
    )
    result = dict(prompt)
    result["context"] = chunks
    return _insert_replan_chunk_into_override(result, chunks[-1])


def _checkpoint_from_state(state: AutonomousMissionReplanState) -> AutonomousMissionReplanCheckpoint:
    phase = {
        "execution_pending": "execution_pending",
        "evaluation_pending": "evaluation_recorded",
        "replan_handoff": "replan_scheduled",
        "terminal": "terminal",
    }[state.phase]
    return AutonomousMissionReplanCheckpoint(
        root_mission_id=state.root_mission_id,
        protected_contract_digest=state.protected_contract_digest,
        attempt=state.attempt,
        phase=phase,
        mission_id=state.current_mission_id,
        outcome_digest=state.outcome_digest,
        evaluation_digest=state.evaluation_digest,
        replan_instruction_digest=state.replan_instruction_digest,
        bandit_state_digest=state.bandit_state_digest,
    )


def run_autonomous_mission_replan_cycle(
    brain: AutonomousBrain,
    *,
    task: str,
    model_candidates: Sequence[ModelCandidate | Mapping[str, Any]],
    prompt: Mapping[str, Any],
    plan: Mapping[str, Any],
    credentials: Mapping[str, CredentialHandle],
    mission_policy: MissionPolicy | Mapping[str, Any],
    evaluator: BrainOutcomeEvaluator,
    bandit_state: Mapping[str, Any],
    evidence: Mapping[str, Any] | None = None,
    ledger: BrainLearningLedger | None = None,
    mission_options: Mapping[str, Any] | None = None,
    max_replans: int = 1,
    root_mission_id: str | None = None,
    state_store: AutonomousMissionReplanStateStore | None = None,
    resume: bool = False,
    rehydrate_result: Callable[[AutonomousMissionReplanRehydrationContext], BrainMissionResult] | None = None,
    rehydrate_instruction: Callable[[AutonomousMissionReplanRehydrationContext], str] | None = None,
    checkpoint_sink: Callable[[AutonomousMissionReplanCheckpoint], Any] | None = None,
    execution_controller: Any | None = None,
    invocation_observer: ProviderInvocationObserver | None = None,
    trace_event_callback: Callable[..., Any] | None = None,
    model_quality_callback: Callable[
        [BrainMissionResult, BrainEvaluatorDecision], Mapping[str, Any] | None
    ]
    | None = None,
) -> AutonomousMissionReplanResult:
    """Run bounded mission attempts with restart-safe evaluator handoff.

    ``state_store`` is optional for local one-shot use.  When supplied, the method persists an
    ``execution_pending`` state before provider dispatch, an ``evaluation_pending`` state after a
    live result, and a ``replan_handoff``/``terminal`` state after evaluator settlement.  A
    restart must rehydrate only the private result or retry instruction whose digest is requested;
    provider calls and evaluator callbacks are never replayed silently.
    """

    if not isinstance(brain, AutonomousBrain):
        raise BrainRunError("brain must be an AutonomousBrain")
    if not isinstance(task, str) or not task.strip():
        raise BrainRunError("mission replan task must be non-empty text")
    if not isinstance(evaluator, BrainOutcomeEvaluator):
        raise BrainRunError("mission replan evaluator must be a BrainOutcomeEvaluator")
    if model_quality_callback is not None and not callable(model_quality_callback):
        raise BrainRunError("mission model_quality_callback must be callable or None")
    if not isinstance(bandit_state, Mapping):
        raise BrainRunError("mission replan bandit_state must be a mapping")
    BrainLearningLedger._assert_safe(bandit_state)
    if isinstance(max_replans, bool) or not isinstance(max_replans, int) or not 0 <= max_replans <= AUTONOMOUS_MISSION_REPLAN_MAX_REPLANS:
        raise BrainRunError(f"mission replan max_replans must be within [0, {AUTONOMOUS_MISSION_REPLAN_MAX_REPLANS}]")
    if not isinstance(prompt, Mapping) or not isinstance(plan, Mapping):
        raise BrainRunError("mission replan prompt and plan must be mappings")
    if not isinstance(credentials, Mapping):
        raise BrainRunError("mission replan credentials must be an opaque-handle mapping")
    if any(
        not isinstance(provider, str)
        or not isinstance(handle, CredentialHandle)
        or provider != handle.provider
        for provider, handle in credentials.items()
    ):
        raise BrainRunError("mission replan credentials must map provider names to matching opaque handles")
    if not isinstance(model_candidates, Sequence) or isinstance(model_candidates, (str, bytes)):
        raise BrainRunError("mission replan model_candidates must be a sequence")
    normalized_candidates = [
        candidate.to_dict()
        if isinstance(candidate, ModelCandidate)
        else ModelCandidate.from_mapping(candidate).to_dict()
        for candidate in model_candidates
        if isinstance(candidate, (ModelCandidate, Mapping))
    ]
    if len(normalized_candidates) != len(model_candidates):
        raise BrainRunError("mission replan model_candidates contain an invalid value")
    if not isinstance(mission_policy, (MissionPolicy, Mapping)):
        raise BrainRunError("mission replan mission_policy must be a MissionPolicy or mapping")
    policy_value = mission_policy.to_dict() if isinstance(mission_policy, MissionPolicy) else dict(mission_policy)
    if mission_options is not None and not isinstance(mission_options, Mapping):
        raise BrainRunError("mission replan mission_options must be a mapping or None")
    if checkpoint_sink is not None and state_store is None:
        raise BrainRunError("checkpoint_sink requires state_store")
    options = {} if mission_options is None else dict(mission_options)
    forbidden_options = {
        "task",
        "model_candidates",
        "prompt",
        "plan",
        "credentials",
        "mission_policy",
        "evaluator",
        "bandit_state",
        "max_replans",
        "ledger",
        "execution_controller",
        "invocation_observer",
        "trace_event_callback",
    }
    forbidden = sorted(forbidden_options.intersection(options))
    if forbidden:
        raise BrainRunError(
            "mission replan mission_options cannot override cycle controls or protected inputs: "
            + ", ".join(forbidden)
        )
    allowed_options = {
        "context",
        "content_parts",
        "contextual_observations",
        "required_capabilities",
        "input_tokens",
        "requested_output_tokens",
        "max_cost_per_million_tokens",
        "max_latency_ms",
        "min_quality",
        "selection_overrides",
        "approve_provider_call",
        "approve_mission_dispatch",
        "run_id",
        "max_output_tokens",
        "temperature",
        "response_schema",
        "idempotency_key",
        "claim_requests",
        "evaluator_review",
        "workflow_binding",
        "route_review",
        "operations_gate_acceptance",
        "route_request",
        "enforce_route_tools",
        "require_resolved_route",
        "provider_tools",
        "tool_choice",
        "max_provider_failovers",
    }
    unknown_options = sorted(set(options).difference(allowed_options))
    if unknown_options:
        raise BrainRunError(
            "mission replan mission_options contains unsupported fields: "
            + ", ".join(unknown_options)
        )
    BrainLearningLedger._assert_safe({"prompt": dict(prompt), "plan": dict(plan), "policy": policy_value, "evidence": evidence})

    task_digest = content_digest({"task": task})
    protected_contract_digest = content_digest(
        {
            "task_digest": task_digest,
            "prompt_digest": _metadata_digest(prompt, label="mission replan prompt"),
            "plan_digest": _metadata_digest(plan, label="mission replan plan"),
            "policy_digest": _metadata_digest(policy_value, label="mission replan mission policy"),
            "model_catalogue_digest": _metadata_digest(normalized_candidates, label="mission replan model catalogue"),
        }
    )
    resolved_root_id = root_mission_id or "mission-replan-" + task_digest[:24]
    _identifier("mission replan root_mission_id", resolved_root_id)
    state: AutonomousMissionReplanState | None = None
    if state_store is not None:
        if not all(callable(getattr(state_store, name, None)) for name in ("load", "save")):
            raise BrainRunError("mission replan state_store must implement load and save")
        loaded = state_store.load(resolved_root_id)
        if loaded is not None:
            state = loaded if isinstance(loaded, AutonomousMissionReplanState) else AutonomousMissionReplanState.from_mapping(loaded)
            if (
                state.protected_contract_digest != protected_contract_digest
                or state.max_replans != max_replans
                or state.root_mission_id != resolved_root_id
            ):
                raise BrainRunError("persisted mission replan state does not match the requested contract")
            if not resume:
                raise BrainRunError("persisted mission replan state requires resume=True")
            if _metadata_digest(bandit_state, label="mission replan bandit state") != state.bandit_state_digest:
                raise BrainRunError("rehydrated mission replan bandit state does not match its checkpoint")
            if state.phase == "terminal":
                return AutonomousMissionReplanResult(
                    status=state.terminal_status or "completed_without_replan",
                    root_mission_id=resolved_root_id,
                    protected_contract_digest=protected_contract_digest,
                    attempts=state.attempts,
                    evaluations=state.evaluations,
                    replan_count=max(0, len(state.attempts) - 1),
                    final_result=None,
                    checkpoint=_checkpoint_from_state(state),
                )
        else:
            state = None
    if state is None:
        state = AutonomousMissionReplanState(
            root_mission_id=resolved_root_id,
            protected_contract_digest=protected_contract_digest,
            max_replans=max_replans,
            attempt=0,
            phase="execution_pending",
            current_mission_id="mission-replan-root",
            bandit_state_digest=_metadata_digest(bandit_state, label="mission replan bandit state"),
            outcome_digest=None,
            evaluation_digest=None,
            replan_instruction_digest=None,
            attempts=(),
            evaluations=(),
            last_checkpoint_digest=None,
            terminal_status=None,
            generation=1,
            previous_state_digest=None,
        )
        if state_store is not None:
            state_store.save(state)

    def commit(
        *,
        phase: str,
        attempt: int,
        mission_id: str,
        bandit: Mapping[str, Any],
        outcome_digest: str | None,
        evaluation_digest: str | None,
        instruction_digest: str | None,
        attempts: Sequence[AutonomousMissionReplanAttempt],
        evaluations: Sequence[Mapping[str, Any]],
        terminal_status: str | None,
    ) -> AutonomousMissionReplanState:
        nonlocal state
        next_state = AutonomousMissionReplanState(
            root_mission_id=resolved_root_id,
            protected_contract_digest=protected_contract_digest,
            max_replans=max_replans,
            attempt=attempt,
            phase=phase,
            current_mission_id=mission_id,
            bandit_state_digest=_metadata_digest(bandit, label="mission replan bandit state"),
            outcome_digest=outcome_digest,
            evaluation_digest=evaluation_digest,
            replan_instruction_digest=instruction_digest,
            attempts=tuple(attempts),
            evaluations=tuple(evaluations),
            last_checkpoint_digest=state.last_checkpoint_digest,
            terminal_status=terminal_status,
            generation=state.generation + 1,
            previous_state_digest=state.state_digest,
        )
        if state_store is not None:
            state_store.save(next_state)
        state = next_state
        return next_state

    attempts = list(state.attempts)
    evaluations = [dict(item) for item in state.evaluations]
    current_bandit_state: Mapping[str, Any] = dict(bandit_state)
    current_prompt: Mapping[str, Any] = dict(prompt)
    start_attempt = state.attempt
    if state.phase in {"replan_handoff", "execution_pending"} and state.replan_instruction_digest is not None:
        if rehydrate_instruction is None:
            raise BrainRunError("mission replan restart requires rehydrate_instruction")
        context = AutonomousMissionReplanRehydrationContext(
            resolved_root_id, protected_contract_digest, state.attempt, state.phase,
            state.outcome_digest, state.evaluation_digest, state.replan_instruction_digest, state.state_digest,
        )
        instruction = _screen_instruction(rehydrate_instruction(context))
        if content_digest(instruction) != state.replan_instruction_digest:
            raise BrainRunError("rehydrated mission replan instruction does not match its digest")
        if not attempts or not evaluations:
            raise BrainRunError("mission replan handoff is missing its prior attempt")
        # The raw prior result is not retained.  The next prompt is rebuilt from caller-owned
        # base context and the verified instruction; its prior outcome is bound by the state.
        current_prompt = dict(prompt)
        raw_context = current_prompt.get("context", [])
        if not isinstance(raw_context, Sequence) or isinstance(raw_context, (str, bytes)):
            raise BrainRunError("mission replan prompt context must be a sequence")
        chunks = [dict(chunk) for chunk in raw_context if isinstance(chunk, Mapping)]
        if len(chunks) != len(raw_context):
            raise BrainRunError("mission replan prompt context must contain mappings")
        chunks.append(
            {
                "id": f"autonomy-mission-replan-{state.attempt + (1 if state.phase == 'replan_handoff' else 0)}",
                "role": "developer",
                "content": json.dumps(
                    {"workflow": "bounded_autonomous_mission_replan", "attempt": state.attempt + 1, "previous_outcome_digest": state.outcome_digest, "instruction": instruction, "does_not_authorize": ["new tools", "new credentials", "external effects"]},
                    ensure_ascii=False,
                    sort_keys=True,
                    separators=(",", ":"),
                ),
                "required": True,
                "priority": 950,
            }
        )
        current_prompt["context"] = chunks
        current_prompt = _insert_replan_chunk_into_override(current_prompt, chunks[-1])
        start_attempt = state.attempt if state.phase == "replan_handoff" else state.attempt - 1
    elif state.phase == "evaluation_pending":
        if rehydrate_result is None:
            raise BrainRunError("mission replan restart requires rehydrate_result for pending evaluation")
        # The callback is consumed in the loop below; this marker avoids provider replay.
        start_attempt = state.attempt - 1
    elif state.phase == "execution_pending" and state.attempt > 0:
        raise BrainRunError(
            "mission replan restart reached an execution boundary without a rehydratable result; "
            "reconcile the provider boundary before retrying"
        )

    for attempt_index in range(start_attempt, max_replans + 1):
        attempt_number = attempt_index + 1
        restored_result: BrainMissionResult | None = None
        if state_store is not None and state.phase == "replan_handoff" and state.attempt == attempt_number - 1:
            state = commit(
                phase="execution_pending",
                attempt=attempt_number,
                mission_id=state.current_mission_id,
                bandit=current_bandit_state,
                outcome_digest=None,
                evaluation_digest=None,
                instruction_digest=state.replan_instruction_digest,
                attempts=attempts,
                evaluations=evaluations,
                terminal_status=None,
            )
        if state.phase == "evaluation_pending" and state.attempt == attempt_number:
            if rehydrate_result is None:
                raise BrainRunError("mission replan restart requires rehydrate_result")
            context = AutonomousMissionReplanRehydrationContext(
                resolved_root_id, protected_contract_digest, state.attempt, state.phase,
                state.outcome_digest, state.evaluation_digest, state.replan_instruction_digest, state.state_digest,
            )
            restored_result = rehydrate_result(context)
            if not isinstance(restored_result, BrainMissionResult) or _result_outcome_digest(restored_result) != state.outcome_digest:
                raise BrainRunError("rehydrated mission result does not match its checkpoint")
        if restored_result is None:
            state = commit(
                phase="execution_pending",
                attempt=attempt_number,
                mission_id=state.current_mission_id,
                bandit=current_bandit_state,
                outcome_digest=None,
                evaluation_digest=None,
                instruction_digest=state.replan_instruction_digest,
                attempts=attempts,
                evaluations=evaluations,
                terminal_status=None,
            ) if state_store is not None else state
            result = brain.run_adaptive_mission(
                task=task,
                model_candidates=normalized_candidates,
                prompt=current_prompt,
                plan=plan,
                credentials=dict(credentials),
                mission_policy=mission_policy,
                bandit_state=current_bandit_state,
                ledger=ledger,
                execution_controller=execution_controller,
                invocation_observer=invocation_observer,
                trace_event_callback=trace_event_callback,
                **options,
            )
        else:
            result = restored_result
        outcome_digest = _result_outcome_digest(result)
        mission_id = _mission_identifier(result, outcome_digest)
        selection_digest = _selection_digest(result)
        state = commit(
            phase="evaluation_pending",
            attempt=attempt_number,
            mission_id=mission_id,
            bandit=current_bandit_state,
            outcome_digest=outcome_digest,
            evaluation_digest=None,
            instruction_digest=None,
            attempts=attempts,
            evaluations=evaluations,
            terminal_status=None,
        ) if state_store is not None else state
        decision, report = evaluator.evaluate_and_record_with_decision(
            brain,
            result,
            bandit_state=current_bandit_state,
            evidence=evidence,
            ledger=ledger,
        )
        next_state = report.get("next_state")
        if isinstance(next_state, Mapping):
            current_bandit_state = dict(next_state)
        model_quality = None
        if model_quality_callback is not None:
            try:
                model_quality = model_quality_callback(result, decision)
            except Exception as error:
                raise BrainRunError("mission model quality callback failed") from error
        projection = _evaluation_projection(decision, model_quality=model_quality)
        evaluation_digest = projection["evaluation_digest"]
        completed_steps, succeeded_steps, failed_steps = _counts(result)
        attempt_record = AutonomousMissionReplanAttempt(
            attempt=attempt_number,
            mission_id=mission_id,
            status=result.status,
            completed_steps=completed_steps,
            succeeded_steps=succeeded_steps,
            failed_steps=failed_steps,
            selection_digest=selection_digest,
            outcome_digest=outcome_digest,
            evaluation_digest=evaluation_digest,
            replan_instruction_digest=projection["replan_instruction_digest"],
            route_digest=_route_digest(result),
        )
        if len(attempts) >= attempt_number:
            attempts[attempt_number - 1] = attempt_record
        else:
            attempts.append(attempt_record)
        evaluations.append(projection)

        if result.status == "mission_dispatched" or result.execution is not None:
            final_status = "replan_blocked_after_dispatch"
        elif not decision.replan_requested:
            final_status = "completed" if decision.passed else "completed_without_replan"
        elif attempt_index >= max_replans:
            final_status = "replan_limit_reached"
        else:
            final_status = None

        if final_status is not None:
            if state_store is not None:
                state = commit(
                    phase="terminal",
                    attempt=attempt_number,
                    mission_id=mission_id,
                    bandit=current_bandit_state,
                    outcome_digest=outcome_digest,
                    evaluation_digest=evaluation_digest,
                    instruction_digest=projection["replan_instruction_digest"],
                    attempts=attempts,
                    evaluations=evaluations,
                    terminal_status=final_status,
                )
            checkpoint = _checkpoint_from_state(state) if state_store is not None else None
            if checkpoint_sink is not None and checkpoint is not None:
                checkpoint_sink(checkpoint)
            return AutonomousMissionReplanResult(
                status=final_status,
                root_mission_id=resolved_root_id,
                protected_contract_digest=protected_contract_digest,
                attempts=tuple(attempts),
                evaluations=tuple(evaluations),
                replan_count=max(0, len(attempts) - 1),
                final_result=result,
                checkpoint=checkpoint,
            )

        instruction = _screen_instruction(decision.replan_instruction)
        if state_store is not None:
            state = commit(
                phase="replan_handoff",
                attempt=attempt_number,
                mission_id=mission_id,
                bandit=current_bandit_state,
                outcome_digest=outcome_digest,
                evaluation_digest=evaluation_digest,
                instruction_digest=content_digest(instruction),
                attempts=attempts,
                evaluations=evaluations,
                terminal_status=None,
            )
        checkpoint = _checkpoint_from_state(state) if state_store is not None else None
        if checkpoint_sink is not None and checkpoint is not None:
            checkpoint_sink(checkpoint)
        current_prompt = _append_replan_context(
            current_prompt,
            attempt=attempt_number + 1,
            result=result,
            decision=decision,
        )
    raise BrainRunError("mission replan cycle exhausted without a terminal result")


__all__ = [
    "AUTONOMOUS_MISSION_REPLAN_SCHEMA",
    "AUTONOMOUS_MISSION_REPLAN_CHECKPOINT_SCHEMA",
    "AUTONOMOUS_MISSION_REPLAN_STATE_SCHEMA",
    "AUTONOMOUS_MISSION_REPLAN_SNAPSHOT_SCHEMA",
    "AUTONOMOUS_MISSION_REPLAN_MAX_REPLANS",
    "AUTONOMOUS_MISSION_REPLAN_MAX_ATTEMPTS",
    "AUTONOMOUS_MISSION_REPLAN_MAX_INSTRUCTION_BYTES",
    "AutonomousMissionReplanAttempt",
    "AutonomousMissionReplanCheckpoint",
    "AutonomousMissionReplanState",
    "AutonomousMissionReplanSnapshot",
    "AutonomousMissionReplanStateStore",
    "AutonomousMissionReplanSnapshotPersistence",
    "AutonomousMissionReplanTextStore",
    "InMemoryAutonomousMissionReplanStateStore",
    "JsonAutonomousMissionReplanSnapshotPersistence",
    "AutonomousMissionReplanPersistenceCoordinator",
    "AutonomousMissionReplanResult",
    "AutonomousMissionReplanRehydrationContext",
    "run_autonomous_mission_replan_cycle",
]
