"""Calibration-gated, restart-safe learning settlement for the autonomous brain.

The low-level brain already knows how to evaluate an episode, assign discounted trajectory
credit, and record a value-only bandit update.  This module supplies the missing operational
boundary around those primitives:

* one controller enforces the optional all-domain evaluator-calibration gate before mutation;
* single and trajectory settlements can be queued as metadata-only commands;
* leases, bounded retries, idempotent command identities, and stale-worker fencing are explicit;
* in-memory, canonical JSON, CAS JSON, and SQLite snapshots are available for local services;
* workers never receive a provider response, credential, prompt, raw evidence value, or tool
  payload from this boundary.

The caller still owns evaluator truth, evidence retention, durable multi-host authorization, and
the backing transaction implementation.  A command may contain a learning episode projection,
an evaluator decision, and a bandit state because those are already value-only contracts; a
non-null evidence body or payload-shaped secret is rejected before enqueue.
"""

from __future__ import annotations

from copy import deepcopy
from dataclasses import dataclass
import json
from pathlib import Path
import sqlite3
import threading
import time
from typing import Any, Mapping, NoReturn, Protocol, Sequence

from .authoring import canonical_json, content_digest
from .autonomous_evaluator_calibration import (
    admit_autonomous_evaluator_calibration,
    validate_autonomous_evaluator_calibration_report,
)
from .domain_tools import AUTONOMOUS_DOMAIN_NAMES
from .errors import ArgumentError
from .brain import (
    AutonomousBrain,
    BrainEvaluatorDecision,
    BrainLearningEpisode,
    BrainLearningLedger,
    BrainLearningTrajectory,
    BrainLearningTrajectoryResult,
    BrainOutcomeEvaluator,
    BrainRunError,
)


AUTONOMOUS_LEARNING_CONTROLLER_SCHEMA = "bioprism-python-autonomous-learning-controller/0.1"
AUTONOMOUS_LEARNING_FEEDBACK_OUTBOX_SCHEMA = "bioprism-python-autonomous-learning-feedback-outbox/0.1"
AUTONOMOUS_LEARNING_FEEDBACK_OUTBOX_SQLITE_SCHEMA = "bioprism-python-autonomous-learning-feedback-outbox-sqlite/0.1"
MAX_AUTONOMOUS_LEARNING_FEEDBACK_COMMANDS = 8_192
MAX_AUTONOMOUS_LEARNING_FEEDBACK_LEASE_MS = 300_000
MAX_AUTONOMOUS_LEARNING_FEEDBACK_ATTEMPTS = 8
MAX_AUTONOMOUS_LEARNING_FEEDBACK_WORKER_ROWS = 256
MAX_AUTONOMOUS_LEARNING_FEEDBACK_SNAPSHOT_BYTES = 4_000_000
MAX_AUTONOMOUS_LEARNING_FEEDBACK_ERROR_BYTES = 256

_DOMAINS = tuple(AUTONOMOUS_DOMAIN_NAMES)
_RETENTION = "value_only;task_prompt_response_credentials_and_evidence_not_retained"
_SECRET_MATERIAL = "never_returned"
_OPERATIONS = {"single", "trajectory"}
_STATUSES = {"pending", "leased", "applied", "failed", "cancelled", "reconciliation_required"}
_SECRET_KEYS = {
    "apikey",
    "authorization",
    "bearer",
    "credential",
    "password",
    "privatekey",
    "rawresponse",
    "refreshtoken",
    "secret",
    "toolarguments",
}


def _fail(message: str) -> NoReturn:
    raise ArgumentError(f"autonomous learning controller {message}")


def _bounded_text(name: str, value: Any, maximum: int) -> str:
    if not isinstance(value, str) or not value or "\x00" in value:
        _fail(f"{name} must be a non-empty string")
    if len(value.encode("utf-8")) > maximum:
        _fail(f"{name} exceeds {maximum} bytes")
    return value


def _identifier(name: str, value: Any, maximum: int = 512) -> str:
    text = _bounded_text(name, value, maximum)
    allowed = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_.:/-"
    if any(character not in allowed for character in text):
        _fail(f"{name} contains an unsafe identifier character")
    return text


def _digest(name: str, value: Any, *, allow_none: bool = False) -> str | None:
    if value is None and allow_none:
        return None
    if not isinstance(value, str) or len(value) != 64 or any(
        character not in "0123456789abcdef" for character in value
    ):
        _fail(f"{name} must be a lowercase SHA-256 digest")
    return value


def _bounded_integer(name: str, value: Any, minimum: int, maximum: int) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or not minimum <= value <= maximum:
        _fail(f"{name} must be between {minimum} and {maximum}")
    return value


def _now_ms(value: int | None = None) -> int:
    if value is None:
        return int(time.time() * 1000)
    return _bounded_integer("clock value", value, 0, 9_223_372_036_854_775_807)


def _safe_value_only(value: Any, *, path: str = "$") -> None:
    """Reject payload keys that could turn the outbox into a secret/value store."""

    if isinstance(value, Mapping):
        for key, child in value.items():
            if not isinstance(key, str):
                _fail(f"{path} contains a non-string key")
            normalized = "".join(character for character in key.lower() if character.isalnum())
            if normalized in _SECRET_KEYS:
                _fail(f"{path}.{key} is not permitted in a learning command")
            if normalized == "evidence" and child is not None:
                _fail(f"{path}.{key} must be null; evidence remains caller-owned")
            if normalized in {"prompt", "outputtext", "content"}:
                _fail(f"{path}.{key} is not permitted in a learning command")
            _safe_value_only(child, path=f"{path}.{key}")
    elif isinstance(value, Sequence) and not isinstance(value, (str, bytes, bytearray)):
        for index, child in enumerate(value):
            _safe_value_only(child, path=f"{path}[{index}]")
    else:
        try:
            json.dumps(value, ensure_ascii=False, allow_nan=False)
        except (TypeError, ValueError) as error:
            raise ArgumentError(f"autonomous learning controller {path} is not JSON-safe") from error


def _decision(value: BrainEvaluatorDecision | Mapping[str, Any]) -> BrainEvaluatorDecision:
    if isinstance(value, BrainEvaluatorDecision):
        return value
    if not isinstance(value, Mapping):
        _fail("evaluator decision must be a BrainEvaluatorDecision or mapping")
    allowed = {
        "evaluator_id", "evaluator_version", "reward", "passed", "failed", "feedback_digest",
        "failure_class", "evidence_digest", "replan_requested", "replan_instruction",
    }
    if set(value).difference(allowed):
        _fail("evaluator decision contains unsupported fields")
    try:
        return BrainEvaluatorDecision(
            evaluator_id=value.get("evaluator_id"),
            evaluator_version=value.get("evaluator_version"),
            reward=value.get("reward"),
            passed=value.get("passed"),
            failed=value.get("failed", False),
            feedback_digest=value.get("feedback_digest"),
            failure_class=value.get("failure_class"),
            evidence_digest=value.get("evidence_digest"),
            replan_requested=value.get("replan_requested", False),
            replan_instruction=value.get("replan_instruction"),
        )
    except (BrainRunError, TypeError, ValueError) as error:
        raise ArgumentError("autonomous learning controller evaluator decision is invalid") from error


def _domain_from_episode(episode: BrainLearningEpisode) -> str | None:
    context = episode.evaluation_input.get("context")
    domain = context.get("domain") if isinstance(context, Mapping) else None
    if not isinstance(domain, str):
        route = episode.evaluation_input.get("route")
        domain = route.get("domain") if isinstance(route, Mapping) else None
    if isinstance(domain, str) and domain.startswith("cross_domain"):
        return "cross_domain"
    return domain if domain in _DOMAINS else None


def _normalize_payload(operation: str, value: Any) -> dict[str, Any]:
    if not isinstance(value, Mapping):
        _fail("feedback command payload must be an object")
    if operation == "single":
        expected = {"episode", "decision", "bandit_state"}
        if set(value) != expected:
            _fail("single feedback payload must contain episode, decision, and bandit_state")
        try:
            episode = BrainLearningEpisode.from_mapping(value["episode"])
        except (BrainRunError, TypeError, ValueError) as error:
            raise ArgumentError("single feedback payload episode is invalid") from error
        if episode.status != "pending":
            _fail("single feedback payload episode must be pending")
        decision = _decision(value["decision"])
        bandit_state = value["bandit_state"]
        if not isinstance(bandit_state, Mapping):
            _fail("single feedback payload bandit_state must be an object")
        normalized = {
            "episode": episode.to_dict(),
            "decision": decision.to_dict(),
            "bandit_state": json.loads(canonical_json(dict(bandit_state))),
        }
    elif operation == "trajectory":
        expected = {"trajectory", "decisions", "bandit_state"}
        if set(value) != expected:
            _fail("trajectory feedback payload must contain trajectory, decisions, and bandit_state")
        try:
            trajectory = BrainLearningTrajectory.from_mapping(value["trajectory"])
        except (BrainRunError, TypeError, ValueError) as error:
            raise ArgumentError("trajectory feedback payload trajectory is invalid") from error
        decisions_raw = value["decisions"]
        if isinstance(decisions_raw, (str, bytes)) or not isinstance(decisions_raw, Sequence) or len(decisions_raw) != len(trajectory.episodes):
            _fail("trajectory feedback payload decisions must match the trajectory")
        decisions = [_decision(raw).to_dict() for raw in decisions_raw]
        bandit_state = value["bandit_state"]
        if not isinstance(bandit_state, Mapping):
            _fail("trajectory feedback payload bandit_state must be an object")
        normalized = {
            "trajectory": trajectory.to_dict(),
            "decisions": decisions,
            "bandit_state": json.loads(canonical_json(dict(bandit_state))),
        }
    else:
        _fail("feedback command operation is invalid")
    _safe_value_only(normalized)
    return normalized


@dataclass(frozen=True, slots=True)
class AutonomousLearningFeedbackCommand:
    """One leased, idempotent, value-only evaluator settlement command."""

    command_id: str
    operation: str
    target_id: str
    target_digest: str
    request_digest: str
    payload: Mapping[str, Any]
    status: str = "pending"
    attempts: int = 0
    available_at: int = 0
    lease_owner: str | None = None
    lease_until: int | None = None
    last_error_class: str | None = None
    result_digest: str | None = None
    created_at: int = 0
    updated_at: int = 0
    command_digest: str | None = None

    def __post_init__(self) -> None:
        _identifier("feedback command_id", self.command_id)
        if self.operation not in _OPERATIONS:
            raise BrainRunError("feedback command operation is invalid")
        _identifier("feedback target_id", self.target_id)
        _digest("feedback target_digest", self.target_digest)
        _digest("feedback request_digest", self.request_digest)
        payload = _normalize_payload(self.operation, self.payload)
        if self.status not in _STATUSES:
            raise BrainRunError("feedback command status is invalid")
        _bounded_integer("feedback command attempts", self.attempts, 0, MAX_AUTONOMOUS_LEARNING_FEEDBACK_ATTEMPTS)
        _now_ms(self.available_at)
        _now_ms(self.created_at)
        _now_ms(self.updated_at)
        if self.lease_owner is not None:
            _identifier("feedback lease_owner", self.lease_owner, 256)
        if self.lease_until is not None:
            _now_ms(self.lease_until)
        if self.last_error_class is not None:
            _bounded_text("feedback last_error_class", self.last_error_class, MAX_AUTONOMOUS_LEARNING_FEEDBACK_ERROR_BYTES)
        _digest("feedback result_digest", self.result_digest, allow_none=True)
        if self.status == "leased" and (self.lease_owner is None or self.lease_until is None):
            raise BrainRunError("leased feedback command must have a lease")
        if self.status != "leased" and (self.lease_owner is not None or self.lease_until is not None):
            raise BrainRunError("non-leased feedback command cannot retain a lease")
        if self.status == "applied" and self.result_digest is None:
            raise BrainRunError("applied feedback command must have a result digest")
        if self.status != "applied" and self.result_digest is not None:
            raise BrainRunError("non-applied feedback command cannot retain a result digest")
        body = {
            "schema": AUTONOMOUS_LEARNING_FEEDBACK_OUTBOX_SCHEMA,
            "command_id": self.command_id,
            "operation": self.operation,
            "target_id": self.target_id,
            "target_digest": self.target_digest,
            "request_digest": self.request_digest,
            "payload": payload,
            "status": self.status,
            "attempts": self.attempts,
            "available_at": self.available_at,
            "lease_owner": self.lease_owner,
            "lease_until": self.lease_until,
            "last_error_class": self.last_error_class,
            "result_digest": self.result_digest,
            "created_at": self.created_at,
            "updated_at": self.updated_at,
        }
        expected = content_digest(body)
        if self.command_digest is not None and self.command_digest != expected:
            raise BrainRunError("feedback command digest does not match its content")
        object.__setattr__(self, "payload", payload)
        object.__setattr__(self, "command_digest", expected)

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_LEARNING_FEEDBACK_OUTBOX_SCHEMA,
            "command_id": self.command_id,
            "operation": self.operation,
            "target_id": self.target_id,
            "target_digest": self.target_digest,
            "request_digest": self.request_digest,
            "payload": deepcopy(dict(self.payload)),
            "status": self.status,
            "attempts": self.attempts,
            "available_at": self.available_at,
            "lease_owner": self.lease_owner,
            "lease_until": self.lease_until,
            "last_error_class": self.last_error_class,
            "result_digest": self.result_digest,
            "created_at": self.created_at,
            "updated_at": self.updated_at,
            "command_digest": self.command_digest,
            "retention": _RETENTION,
            "secret_material": _SECRET_MATERIAL,
        }

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> "AutonomousLearningFeedbackCommand":
        if not isinstance(value, Mapping) or value.get("schema") != AUTONOMOUS_LEARNING_FEEDBACK_OUTBOX_SCHEMA:
            raise BrainRunError("feedback command schema is invalid")
        if value.get("retention") != _RETENTION or value.get("secret_material") != _SECRET_MATERIAL:
            raise BrainRunError("feedback command retention markers are invalid")
        return cls(
            command_id=value.get("command_id"),
            operation=value.get("operation"),
            target_id=value.get("target_id"),
            target_digest=value.get("target_digest"),
            request_digest=value.get("request_digest"),
            payload=value.get("payload"),
            status=value.get("status", "pending"),
            attempts=value.get("attempts", 0),
            available_at=value.get("available_at", 0),
            lease_owner=value.get("lease_owner"),
            lease_until=value.get("lease_until"),
            last_error_class=value.get("last_error_class"),
            result_digest=value.get("result_digest"),
            created_at=value.get("created_at", 0),
            updated_at=value.get("updated_at", 0),
            command_digest=value.get("command_digest"),
        )


def validate_autonomous_learning_feedback_command(value: Mapping[str, Any]) -> AutonomousLearningFeedbackCommand:
    return AutonomousLearningFeedbackCommand.from_dict(value)


class InMemoryAutonomousLearningFeedbackOutbox:
    """Thread-safe outbox with lease ownership, retry bounds, and command CAS semantics."""

    def __init__(self, *, max_commands: int = MAX_AUTONOMOUS_LEARNING_FEEDBACK_COMMANDS, clock: Any = None) -> None:
        self.max_commands = _bounded_integer("feedback outbox max_commands", max_commands, 1, MAX_AUTONOMOUS_LEARNING_FEEDBACK_COMMANDS)
        if clock is not None and not callable(clock):
            _fail("feedback outbox clock must be callable")
        self.clock = clock or (lambda: int(time.time() * 1000))
        self._commands: dict[str, AutonomousLearningFeedbackCommand] = {}
        self._lock = threading.RLock()

    def _time(self, now: int | None) -> int:
        return _now_ms(self.clock() if now is None else now)

    def _replace(self, command: AutonomousLearningFeedbackCommand, **changes: Any) -> AutonomousLearningFeedbackCommand:
        values = command.to_dict()
        values.update(changes)
        values.pop("schema", None)
        values.pop("retention", None)
        values.pop("secret_material", None)
        values.pop("command_digest", None)
        return AutonomousLearningFeedbackCommand(**values)

    def enqueue(self, command: AutonomousLearningFeedbackCommand) -> AutonomousLearningFeedbackCommand:
        if not isinstance(command, AutonomousLearningFeedbackCommand):
            _fail("feedback outbox enqueue requires a typed command")
        with self._lock:
            prior = self._commands.get(command.command_id)
            if prior is not None:
                if prior.request_digest != command.request_digest or prior.target_digest != command.target_digest:
                    _fail("feedback command id is already bound to different content")
                return deepcopy(prior)
            if len(self._commands) >= self.max_commands:
                _fail("feedback outbox capacity is exhausted")
            self._commands[command.command_id] = deepcopy(command)
            return deepcopy(command)

    def get(self, command_id: str) -> AutonomousLearningFeedbackCommand | None:
        _identifier("feedback command_id", command_id)
        with self._lock:
            command = self._commands.get(command_id)
            return None if command is None else deepcopy(command)

    def commands(self) -> list[AutonomousLearningFeedbackCommand]:
        with self._lock:
            return [deepcopy(self._commands[key]) for key in sorted(self._commands)]

    def reconcile_expired(self, *, now: int | None = None) -> int:
        observed = self._time(now)
        changed = 0
        with self._lock:
            for command_id, command in list(self._commands.items()):
                if command.status != "leased" or command.lease_until is None or command.lease_until > observed:
                    continue
                replacement = self._replace(
                    command,
                    status="reconciliation_required",
                    lease_owner=None,
                    lease_until=None,
                    last_error_class="lease_expired",
                    updated_at=observed,
                )
                self._commands[command_id] = replacement
                changed += 1
        return changed

    def requeue(self, command_id: str, *, now: int | None = None) -> AutonomousLearningFeedbackCommand:
        observed = self._time(now)
        with self._lock:
            command = self._commands.get(command_id)
            if command is None:
                _fail("cannot requeue an unknown feedback command")
            if command.status not in {"reconciliation_required", "failed"}:
                _fail("feedback command is not awaiting explicit requeue")
            if command.attempts >= MAX_AUTONOMOUS_LEARNING_FEEDBACK_ATTEMPTS:
                _fail("feedback command retry bound is exhausted")
            replacement = self._replace(
                command,
                status="pending",
                available_at=observed,
                lease_owner=None,
                lease_until=None,
                last_error_class=None,
                result_digest=None,
                updated_at=observed,
            )
            self._commands[command_id] = replacement
            return deepcopy(replacement)

    def claim(self, worker_id: str, *, lease_ms: int = 30_000, now: int | None = None) -> AutonomousLearningFeedbackCommand | None:
        owner = _identifier("feedback worker_id", worker_id, 256)
        duration = _bounded_integer("feedback lease_ms", lease_ms, 1, MAX_AUTONOMOUS_LEARNING_FEEDBACK_LEASE_MS)
        observed = self._time(now)
        self.reconcile_expired(now=observed)
        with self._lock:
            eligible = [
                command for command in self._commands.values()
                if command.status == "pending" and command.available_at <= observed and command.attempts < MAX_AUTONOMOUS_LEARNING_FEEDBACK_ATTEMPTS
            ]
            if not eligible:
                return None
            command = sorted(eligible, key=lambda row: (row.available_at, row.created_at, row.command_id))[0]
            replacement = self._replace(
                command,
                status="leased",
                attempts=command.attempts + 1,
                lease_owner=owner,
                lease_until=observed + duration,
                updated_at=observed,
            )
            self._commands[command.command_id] = replacement
            return deepcopy(replacement)

    def renew(self, command_id: str, worker_id: str, *, lease_ms: int = 30_000, now: int | None = None) -> AutonomousLearningFeedbackCommand:
        owner = _identifier("feedback worker_id", worker_id, 256)
        duration = _bounded_integer("feedback lease_ms", lease_ms, 1, MAX_AUTONOMOUS_LEARNING_FEEDBACK_LEASE_MS)
        observed = self._time(now)
        with self._lock:
            command = self._commands.get(command_id)
            if command is None or command.status != "leased" or command.lease_owner != owner or command.lease_until is None or command.lease_until < observed:
                _fail("feedback command lease is not owned by this worker")
            replacement = self._replace(command, lease_until=observed + duration, updated_at=observed)
            self._commands[command_id] = replacement
            return deepcopy(replacement)

    def mark_applied(self, command_id: str, worker_id: str, result_digest: str, *, now: int | None = None) -> AutonomousLearningFeedbackCommand:
        owner = _identifier("feedback worker_id", worker_id, 256)
        _digest("feedback result_digest", result_digest)
        observed = self._time(now)
        with self._lock:
            command = self._commands.get(command_id)
            if command is None:
                _fail("cannot apply an unknown feedback command")
            if command.status == "applied":
                if command.result_digest != result_digest:
                    _fail("applied feedback command has a conflicting result digest")
                return deepcopy(command)
            if command.status != "leased" or command.lease_owner != owner or command.lease_until is None or command.lease_until < observed:
                _fail("feedback command lease is not owned by this worker")
            replacement = self._replace(command, status="applied", lease_owner=None, lease_until=None, result_digest=result_digest, updated_at=observed)
            self._commands[command_id] = replacement
            return deepcopy(replacement)

    def mark_failed(self, command_id: str, worker_id: str, error_class: str, *, retryable: bool = True, now: int | None = None) -> AutonomousLearningFeedbackCommand:
        owner = _identifier("feedback worker_id", worker_id, 256)
        error_name = _identifier("feedback error_class", error_class, MAX_AUTONOMOUS_LEARNING_FEEDBACK_ERROR_BYTES)
        if not isinstance(retryable, bool):
            _fail("feedback retryable must be boolean")
        observed = self._time(now)
        with self._lock:
            command = self._commands.get(command_id)
            if command is None or command.status != "leased" or command.lease_owner != owner or command.lease_until is None or command.lease_until < observed:
                _fail("feedback command lease is not owned by this worker")
            can_retry = retryable and command.attempts < MAX_AUTONOMOUS_LEARNING_FEEDBACK_ATTEMPTS
            next_status = "pending" if can_retry else "failed"
            delay = min(60_000, 250 * (2 ** min(command.attempts, 8)))
            replacement = self._replace(
                command,
                status=next_status,
                available_at=observed + delay if can_retry else command.available_at,
                lease_owner=None,
                lease_until=None,
                last_error_class=error_name,
                updated_at=observed,
            )
            self._commands[command_id] = replacement
            return deepcopy(replacement)

    def cancel(self, command_id: str, *, now: int | None = None) -> AutonomousLearningFeedbackCommand:
        observed = self._time(now)
        with self._lock:
            command = self._commands.get(command_id)
            if command is None:
                _fail("cannot cancel an unknown feedback command")
            if command.status in {"applied", "cancelled"}:
                return deepcopy(command)
            replacement = self._replace(command, status="cancelled", lease_owner=None, lease_until=None, updated_at=observed)
            self._commands[command_id] = replacement
            return deepcopy(replacement)

    def snapshot(self) -> dict[str, Any]:
        with self._lock:
            body = {
                "schema": AUTONOMOUS_LEARNING_FEEDBACK_OUTBOX_SCHEMA,
                "commands": [command.to_dict() for command in self.commands()],
                "retention": "value_only_feedback_commands;source_evidence_not_retained",
                "secret_material": _SECRET_MATERIAL,
            }
        snapshot = {**body, "snapshot_digest": content_digest(body)}
        if len(canonical_json(snapshot).encode("utf-8")) > MAX_AUTONOMOUS_LEARNING_FEEDBACK_SNAPSHOT_BYTES:
            _fail("feedback outbox snapshot exceeds its byte bound")
        return snapshot

    def restore(self, snapshot: Mapping[str, Any]) -> None:
        normalized = validate_autonomous_learning_feedback_snapshot(snapshot, max_commands=self.max_commands)
        commands = [AutonomousLearningFeedbackCommand.from_dict(raw) for raw in normalized["commands"]]
        with self._lock:
            self._commands = {command.command_id: command for command in commands}


def validate_autonomous_learning_feedback_snapshot(value: Mapping[str, Any], *, max_commands: int = MAX_AUTONOMOUS_LEARNING_FEEDBACK_COMMANDS) -> dict[str, Any]:
    if not isinstance(value, Mapping) or value.get("schema") != AUTONOMOUS_LEARNING_FEEDBACK_OUTBOX_SCHEMA:
        _fail("feedback outbox snapshot schema is invalid")
    _bounded_integer("feedback outbox max_commands", max_commands, 1, MAX_AUTONOMOUS_LEARNING_FEEDBACK_COMMANDS)
    if value.get("retention") != "value_only_feedback_commands;source_evidence_not_retained" or value.get("secret_material") != _SECRET_MATERIAL:
        _fail("feedback outbox snapshot retention markers are invalid")
    raw_commands = value.get("commands")
    if isinstance(raw_commands, (str, bytes)) or not isinstance(raw_commands, Sequence) or len(raw_commands) > max_commands:
        _fail("feedback outbox commands are outside their bound")
    commands = [AutonomousLearningFeedbackCommand.from_dict(raw) for raw in raw_commands]
    if len({command.command_id for command in commands}) != len(commands):
        _fail("feedback outbox contains duplicate command ids")
    if [command.command_id for command in commands] != sorted(command.command_id for command in commands):
        _fail("feedback outbox commands are not in canonical order")
    body = {key: item for key, item in value.items() if key != "snapshot_digest"}
    _digest("feedback outbox snapshot_digest", value.get("snapshot_digest"))
    if content_digest(body) != value["snapshot_digest"]:
        _fail("feedback outbox snapshot digest is invalid")
    return deepcopy(dict(value))


class AutonomousLearningFeedbackSnapshotTextStore(Protocol):
    def read(self) -> str | None: ...

    def write(self, value: str) -> None: ...


class TransactionalAutonomousLearningFeedbackSnapshotTextStore(AutonomousLearningFeedbackSnapshotTextStore, Protocol):
    def write_if_unchanged(self, expected_snapshot_digest: str | None, value: str) -> bool: ...


class InMemoryAutonomousLearningFeedbackPersistence:
    def __init__(self, initial: Mapping[str, Any] | None = None) -> None:
        self._snapshot: dict[str, Any] | None = None
        self._lock = threading.RLock()
        if initial is not None:
            self.write(initial)

    def read(self) -> dict[str, Any] | None:
        with self._lock:
            return None if self._snapshot is None else json.loads(canonical_json(self._snapshot))

    def write(self, snapshot: Mapping[str, Any]) -> None:
        normalized = validate_autonomous_learning_feedback_snapshot(snapshot)
        with self._lock:
            self._snapshot = normalized

    def write_if_unchanged(self, expected_snapshot_digest: str | None, snapshot: Mapping[str, Any]) -> bool:
        _digest("feedback expected snapshot digest", expected_snapshot_digest, allow_none=True)
        normalized = validate_autonomous_learning_feedback_snapshot(snapshot)
        with self._lock:
            observed = None if self._snapshot is None else self._snapshot["snapshot_digest"]
            if observed != expected_snapshot_digest:
                return False
            self._snapshot = normalized
            return True


class JsonAutonomousLearningFeedbackPersistence:
    def __init__(self, store: AutonomousLearningFeedbackSnapshotTextStore, *, max_commands: int = MAX_AUTONOMOUS_LEARNING_FEEDBACK_COMMANDS, max_bytes: int = MAX_AUTONOMOUS_LEARNING_FEEDBACK_SNAPSHOT_BYTES) -> None:
        if not callable(getattr(store, "read", None)) or not callable(getattr(store, "write", None)):
            _fail("feedback JSON persistence requires a text store")
        self.store = store
        self.max_commands = _bounded_integer("feedback JSON max_commands", max_commands, 1, MAX_AUTONOMOUS_LEARNING_FEEDBACK_COMMANDS)
        self.max_bytes = _bounded_integer("feedback JSON max_bytes", max_bytes, 1, MAX_AUTONOMOUS_LEARNING_FEEDBACK_SNAPSHOT_BYTES)

    def _encode(self, snapshot: Mapping[str, Any]) -> str:
        normalized = validate_autonomous_learning_feedback_snapshot(snapshot, max_commands=self.max_commands)
        encoded = canonical_json(normalized)
        if len(encoded.encode("utf-8")) > self.max_bytes:
            _fail("feedback JSON snapshot exceeds its byte bound")
        return encoded

    def read(self) -> dict[str, Any] | None:
        encoded = self.store.read()
        if encoded is None:
            return None
        if not isinstance(encoded, str) or len(encoded.encode("utf-8")) > self.max_bytes:
            _fail("feedback JSON snapshot exceeds its byte bound")
        try:
            value = json.loads(encoded)
        except (TypeError, ValueError) as error:
            raise ArgumentError("autonomous learning feedback JSON snapshot is invalid") from error
        if encoded != canonical_json(value):
            _fail("feedback JSON snapshot is not canonical")
        validate_autonomous_learning_feedback_snapshot(value, max_commands=self.max_commands)
        return value

    def write(self, snapshot: Mapping[str, Any]) -> None:
        self.store.write(self._encode(snapshot))


class TransactionalJsonAutonomousLearningFeedbackPersistence(JsonAutonomousLearningFeedbackPersistence):
    def __init__(self, store: TransactionalAutonomousLearningFeedbackSnapshotTextStore, **kwargs: Any) -> None:
        super().__init__(store, **kwargs)
        if not callable(getattr(store, "write_if_unchanged", None)):
            _fail("transactional feedback JSON persistence requires write_if_unchanged")
        self.store = store

    def write_if_unchanged(self, expected_snapshot_digest: str | None, snapshot: Mapping[str, Any]) -> bool:
        _digest("feedback expected snapshot digest", expected_snapshot_digest, allow_none=True)
        return bool(self.store.write_if_unchanged(expected_snapshot_digest, self._encode(snapshot)))


class SQLiteAutonomousLearningFeedbackPersistence:
    """Transactional SQLite snapshot persistence for evaluator feedback workers."""

    def __init__(self, path: str | Path, *, max_commands: int = MAX_AUTONOMOUS_LEARNING_FEEDBACK_COMMANDS, busy_timeout_ms: int = 5_000) -> None:
        if not isinstance(path, (str, Path)) or not str(path):
            _fail("feedback SQLite path must be non-empty")
        self.path = str(path)
        self.max_commands = _bounded_integer("feedback SQLite max_commands", max_commands, 1, MAX_AUTONOMOUS_LEARNING_FEEDBACK_COMMANDS)
        self.busy_timeout_ms = _bounded_integer("feedback SQLite busy_timeout_ms", busy_timeout_ms, 1, 120_000)
        self._lock = threading.RLock()
        if self.path != ":memory:":
            Path(self.path).parent.mkdir(parents=True, exist_ok=True)
        try:
            self._connection = sqlite3.connect(self.path, isolation_level=None, check_same_thread=False)
            self._connection.row_factory = sqlite3.Row
            self._connection.execute("PRAGMA synchronous=FULL")
            self._connection.execute(f"PRAGMA busy_timeout={self.busy_timeout_ms}")
            self._connection.execute("CREATE TABLE IF NOT EXISTS autonomous_learning_feedback_snapshots (singleton INTEGER PRIMARY KEY CHECK(singleton=1), persistence_schema TEXT NOT NULL, schema TEXT NOT NULL, snapshot_json TEXT NOT NULL, snapshot_digest TEXT NOT NULL)")
        except sqlite3.Error as error:
            raise ArgumentError("could not initialize feedback SQLite persistence") from error

    def close(self) -> None:
        with self._lock:
            self._connection.close()

    def __enter__(self) -> "SQLiteAutonomousLearningFeedbackPersistence":
        return self

    def __exit__(self, *_: Any) -> None:
        self.close()

    def read(self) -> dict[str, Any] | None:
        with self._lock:
            try:
                row = self._connection.execute("SELECT persistence_schema, schema, snapshot_json, snapshot_digest FROM autonomous_learning_feedback_snapshots WHERE singleton=1").fetchone()
            except sqlite3.Error as error:
                raise ArgumentError("could not read feedback SQLite persistence") from error
        if row is None:
            return None
        if row["persistence_schema"] != AUTONOMOUS_LEARNING_FEEDBACK_OUTBOX_SQLITE_SCHEMA or row["schema"] != AUTONOMOUS_LEARNING_FEEDBACK_OUTBOX_SCHEMA:
            _fail("feedback SQLite snapshot schema is invalid")
        try:
            value = json.loads(row["snapshot_json"])
        except (TypeError, ValueError) as error:
            raise ArgumentError("feedback SQLite snapshot is invalid") from error
        if value.get("snapshot_digest") != row["snapshot_digest"]:
            _fail("feedback SQLite snapshot digest is invalid")
        return validate_autonomous_learning_feedback_snapshot(value, max_commands=self.max_commands)

    def _normalized(self, snapshot: Mapping[str, Any]) -> tuple[dict[str, Any], str]:
        value = validate_autonomous_learning_feedback_snapshot(snapshot, max_commands=self.max_commands)
        return value, canonical_json(value)

    def write(self, snapshot: Mapping[str, Any]) -> None:
        value, encoded = self._normalized(snapshot)
        with self._lock:
            try:
                self._connection.execute("BEGIN IMMEDIATE")
                self._connection.execute("INSERT INTO autonomous_learning_feedback_snapshots(singleton,persistence_schema,schema,snapshot_json,snapshot_digest) VALUES(1,?,?,?,?) ON CONFLICT(singleton) DO UPDATE SET persistence_schema=excluded.persistence_schema,schema=excluded.schema,snapshot_json=excluded.snapshot_json,snapshot_digest=excluded.snapshot_digest", (AUTONOMOUS_LEARNING_FEEDBACK_OUTBOX_SQLITE_SCHEMA, AUTONOMOUS_LEARNING_FEEDBACK_OUTBOX_SCHEMA, encoded, value["snapshot_digest"]))
                self._connection.execute("COMMIT")
            except sqlite3.Error as error:
                try:
                    self._connection.execute("ROLLBACK")
                except sqlite3.Error:
                    pass
                raise ArgumentError("could not write feedback SQLite persistence") from error

    def write_if_unchanged(self, expected_snapshot_digest: str | None, snapshot: Mapping[str, Any]) -> bool:
        _digest("feedback expected snapshot digest", expected_snapshot_digest, allow_none=True)
        value, encoded = self._normalized(snapshot)
        with self._lock:
            try:
                self._connection.execute("BEGIN IMMEDIATE")
                row = self._connection.execute("SELECT snapshot_digest FROM autonomous_learning_feedback_snapshots WHERE singleton=1").fetchone()
                observed = None if row is None else row["snapshot_digest"]
                if observed != expected_snapshot_digest:
                    self._connection.execute("ROLLBACK")
                    return False
                self._connection.execute("INSERT INTO autonomous_learning_feedback_snapshots(singleton,persistence_schema,schema,snapshot_json,snapshot_digest) VALUES(1,?,?,?,?) ON CONFLICT(singleton) DO UPDATE SET persistence_schema=excluded.persistence_schema,schema=excluded.schema,snapshot_json=excluded.snapshot_json,snapshot_digest=excluded.snapshot_digest", (AUTONOMOUS_LEARNING_FEEDBACK_OUTBOX_SQLITE_SCHEMA, AUTONOMOUS_LEARNING_FEEDBACK_OUTBOX_SCHEMA, encoded, value["snapshot_digest"]))
                self._connection.execute("COMMIT")
                return True
            except sqlite3.Error as error:
                try:
                    self._connection.execute("ROLLBACK")
                except sqlite3.Error:
                    pass
                raise ArgumentError("could not compare-and-swap feedback SQLite persistence") from error


class AutonomousLearningFeedbackPersistenceCoordinator:
    """Restore and flush the outbox with CAS fencing when available."""

    def __init__(self, outbox: InMemoryAutonomousLearningFeedbackOutbox, persistence: Any) -> None:
        if not isinstance(outbox, InMemoryAutonomousLearningFeedbackOutbox) or not callable(getattr(persistence, "read", None)) or not callable(getattr(persistence, "write", None)):
            _fail("feedback persistence coordinator arguments are malformed")
        self.outbox = outbox
        self.persistence = persistence
        self._expected_snapshot_digest: str | None = None
        self._lock = threading.RLock()

    def restore(self) -> dict[str, Any]:
        with self._lock:
            snapshot = self.persistence.read()
            if snapshot is None:
                self._expected_snapshot_digest = None
                return {"status": "empty", "snapshot_digest": None, "commands": 0}
            self.outbox.restore(snapshot)
            self._expected_snapshot_digest = snapshot["snapshot_digest"]
            return {"status": "restored", "snapshot_digest": self._expected_snapshot_digest, "commands": len(snapshot["commands"])}

    def flush(self) -> dict[str, Any]:
        with self._lock:
            snapshot = self.outbox.snapshot()
            cas = getattr(self.persistence, "write_if_unchanged", None)
            if callable(cas) and not cas(self._expected_snapshot_digest, snapshot):
                _fail("feedback persistence compare-and-swap conflict")
            if not callable(cas):
                self.persistence.write(snapshot)
            self._expected_snapshot_digest = snapshot["snapshot_digest"]
            return snapshot


class AutonomousLearningController:
    """One calibration-aware controller for single, trajectory, and queued learning settlement."""

    def __init__(
        self,
        brain: AutonomousBrain,
        *,
        ledger: BrainLearningLedger | None = None,
        calibration_report: Mapping[str, Any] | None = None,
        require_calibrated_learning: bool = False,
    ) -> None:
        if not isinstance(brain, AutonomousBrain):
            raise BrainRunError("learning controller requires an AutonomousBrain")
        if ledger is not None and not isinstance(ledger, BrainLearningLedger):
            raise BrainRunError("learning controller ledger must be a BrainLearningLedger or None")
        if not isinstance(require_calibrated_learning, bool):
            raise BrainRunError("learning controller require_calibrated_learning must be boolean")
        if require_calibrated_learning and calibration_report is None:
            raise BrainRunError("learning controller requires calibration_report when calibrated learning is required")
        self.brain = brain
        self.ledger = ledger
        self.calibration_report = None if calibration_report is None else validate_autonomous_evaluator_calibration_report(calibration_report)
        self.require_calibrated_learning = require_calibrated_learning

    def to_dict(self) -> dict[str, Any]:
        report = self.calibration_report
        return {
            "schema": AUTONOMOUS_LEARNING_CONTROLLER_SCHEMA,
            "require_calibrated_learning": self.require_calibrated_learning,
            "calibration_configured": report is not None,
            "calibration_report_digest": None if report is None else report["report_digest"],
            "calibration_status": "unconfigured" if report is None else report["status"],
            "calibration_decision": "hold_learning" if report is None else report["gate"]["decision"],
            "calibration_ready_domain_count": 0 if report is None else sum(row["status"] == "ready" for row in report["domains"]),
            "calibration_held_domain_count": len(_DOMAINS) if report is None else sum(row["status"] != "ready" for row in report["domains"]),
            "execution": "value_only_learning_settlement;provider_and_evidence_replay_forbidden",
            "retention": "controller_configuration_and_calibration_digest_only",
            "secret_material": _SECRET_MATERIAL,
        }

    def assert_learning_admission(self, domain: str | None) -> dict[str, Any]:
        if not self.require_calibrated_learning:
            return {"decision": "admit_learning", "domain": domain, "reason": "calibration_gate_not_required"}
        if domain not in _DOMAINS:
            raise BrainRunError("learning admission requires a supported autonomous domain identity")
        if self.calibration_report is None:
            raise BrainRunError("learning admission is missing its calibration report")
        try:
            admission = admit_autonomous_evaluator_calibration(self.calibration_report, domain)
        except ArgumentError as error:
            raise BrainRunError("learning calibration admission could not be evaluated") from error
        if admission["decision"] != "admit_learning":
            raise BrainRunError("learning calibration admission is holding " + domain + ": " + ", ".join(admission["reasons"]))
        return admission

    @staticmethod
    def _episode(value: BrainLearningEpisode | Mapping[str, Any]) -> BrainLearningEpisode:
        try:
            return value if isinstance(value, BrainLearningEpisode) else BrainLearningEpisode.from_mapping(value)
        except (BrainRunError, TypeError, ValueError) as error:
            raise BrainRunError("learning controller episode is invalid") from error

    @staticmethod
    def _trajectory(value: BrainLearningTrajectory | Mapping[str, Any]) -> BrainLearningTrajectory:
        try:
            return value if isinstance(value, BrainLearningTrajectory) else BrainLearningTrajectory.from_mapping(value)
        except (BrainRunError, TypeError, ValueError) as error:
            raise BrainRunError("learning controller trajectory is invalid") from error

    def prepare_episode(self, result: Any, *, evidence: Mapping[str, Any] | None = None, arm_id: str | None = None, episode_id: str | None = None) -> BrainLearningEpisode:
        return self.brain.prepare_learning_episode(result, evidence=evidence, arm_id=arm_id, episode_id=episode_id, ledger=self.ledger)

    def prepare_trajectory(self, results: Sequence[Any], *, evidence_by_step: Sequence[Mapping[str, Any] | None] | None = None, arm_ids: Sequence[str | None] | None = None, trajectory_id: str | None = None, discount: float = 0.90, terminal_reward: float | None = None) -> BrainLearningTrajectory:
        return self.brain.prepare_learning_trajectory(results, evidence_by_step=evidence_by_step, arm_ids=arm_ids, trajectory_id=trajectory_id, discount=discount, terminal_reward=terminal_reward, ledger=self.ledger)

    def settle_episode(
        self,
        episode: BrainLearningEpisode | Mapping[str, Any],
        *,
        evaluator: BrainOutcomeEvaluator,
        decision: BrainEvaluatorDecision | Mapping[str, Any],
        bandit_state: Mapping[str, Any],
    ) -> tuple[BrainEvaluatorDecision, dict[str, Any]]:
        normalized_episode = self._episode(episode)
        self.assert_learning_admission(_domain_from_episode(normalized_episode))
        if not isinstance(evaluator, BrainOutcomeEvaluator):
            raise BrainRunError("learning controller evaluator must be a BrainOutcomeEvaluator")
        return evaluator.settle_episode(
            self.brain,
            normalized_episode,
            decision=_decision(decision),
            bandit_state=bandit_state,
            ledger=self.ledger,
        )

    def evaluate_episode(
        self,
        episode: BrainLearningEpisode | Mapping[str, Any],
        *,
        evaluator: BrainOutcomeEvaluator,
        bandit_state: Mapping[str, Any],
        evidence: Mapping[str, Any] | None = None,
    ) -> tuple[BrainEvaluatorDecision, dict[str, Any]]:
        normalized_episode = self._episode(episode)
        self.assert_learning_admission(_domain_from_episode(normalized_episode))
        if not isinstance(evaluator, BrainOutcomeEvaluator):
            raise BrainRunError("learning controller evaluator must be a BrainOutcomeEvaluator")
        return evaluator.evaluate_episode(
            self.brain,
            normalized_episode,
            bandit_state=bandit_state,
            evidence=evidence,
            ledger=self.ledger,
        )

    def evaluate_and_record(
        self,
        result: Any,
        *,
        evaluator: BrainOutcomeEvaluator,
        bandit_state: Mapping[str, Any],
        evidence: Mapping[str, Any] | None = None,
        arm_id: str | None = None,
    ) -> tuple[BrainEvaluatorDecision, dict[str, Any]]:
        from .brain import build_brain_evaluation_input

        evaluation_input = build_brain_evaluation_input(result, evidence=evidence)
        context = evaluation_input.get("context")
        domain = context.get("domain") if isinstance(context, Mapping) else None
        if isinstance(domain, str) and domain.startswith("cross_domain"):
            domain = "cross_domain"
        self.assert_learning_admission(domain)
        if not isinstance(evaluator, BrainOutcomeEvaluator):
            raise BrainRunError("learning controller evaluator must be a BrainOutcomeEvaluator")
        return evaluator.evaluate_and_record_with_decision(
            self.brain,
            result,
            bandit_state=bandit_state,
            evidence=evidence,
            arm_id=arm_id,
            ledger=self.ledger,
        )

    def settle_trajectory(
        self,
        trajectory: BrainLearningTrajectory | Mapping[str, Any],
        *,
        evaluator: BrainOutcomeEvaluator,
        decisions: Sequence[BrainEvaluatorDecision | Mapping[str, Any]],
        bandit_state: Mapping[str, Any],
        evidence_by_step: Sequence[Mapping[str, Any] | None] | None = None,
    ) -> BrainLearningTrajectoryResult:
        normalized = self._trajectory(trajectory)
        for episode in normalized.episodes:
            self.assert_learning_admission(_domain_from_episode(episode))
        if not isinstance(evaluator, BrainOutcomeEvaluator):
            raise BrainRunError("learning controller evaluator must be a BrainOutcomeEvaluator")
        normalized_decisions = tuple(_decision(decision) for decision in decisions)
        return evaluator.settle_trajectory(
            self.brain,
            normalized,
            decisions=normalized_decisions,
            bandit_state=bandit_state,
            evidence_by_step=evidence_by_step,
            ledger=self.ledger,
        )

    def evaluate_trajectory(
        self,
        trajectory: BrainLearningTrajectory | Mapping[str, Any],
        *,
        evaluator: BrainOutcomeEvaluator,
        bandit_state: Mapping[str, Any],
        evidence_by_step: Sequence[Mapping[str, Any] | None] | None = None,
    ) -> BrainLearningTrajectoryResult:
        normalized = self._trajectory(trajectory)
        for episode in normalized.episodes:
            self.assert_learning_admission(_domain_from_episode(episode))
        if not isinstance(evaluator, BrainOutcomeEvaluator):
            raise BrainRunError("learning controller evaluator must be a BrainOutcomeEvaluator")
        return evaluator.evaluate_trajectory(
            self.brain,
            normalized,
            bandit_state=bandit_state,
            evidence_by_step=evidence_by_step,
            ledger=self.ledger,
        )

    def enqueue_episode_settlement(
        self,
        outbox: InMemoryAutonomousLearningFeedbackOutbox,
        episode: BrainLearningEpisode | Mapping[str, Any],
        *,
        decision: BrainEvaluatorDecision | Mapping[str, Any],
        bandit_state: Mapping[str, Any],
        command_id: str | None = None,
        now: int | None = None,
    ) -> AutonomousLearningFeedbackCommand:
        normalized_episode = self._episode(episode)
        self.assert_learning_admission(_domain_from_episode(normalized_episode))
        normalized_decision = _decision(decision)
        payload = _normalize_payload("single", {"episode": normalized_episode.to_dict(), "decision": normalized_decision.to_dict(), "bandit_state": bandit_state})
        target_digest = content_digest(normalized_episode.to_dict())
        request_digest = content_digest(payload)
        resolved_id = command_id or f"learning:{request_digest}"
        timestamp = _now_ms(now)
        command = AutonomousLearningFeedbackCommand(
            command_id=resolved_id,
            operation="single",
            target_id=normalized_episode.episode_id,
            target_digest=target_digest,
            request_digest=request_digest,
            payload=payload,
            created_at=timestamp,
            updated_at=timestamp,
        )
        return outbox.enqueue(command)

    def enqueue_trajectory_settlement(
        self,
        outbox: InMemoryAutonomousLearningFeedbackOutbox,
        trajectory: BrainLearningTrajectory | Mapping[str, Any],
        *,
        decisions: Sequence[BrainEvaluatorDecision | Mapping[str, Any]],
        bandit_state: Mapping[str, Any],
        command_id: str | None = None,
        now: int | None = None,
    ) -> AutonomousLearningFeedbackCommand:
        normalized_trajectory = self._trajectory(trajectory)
        for episode in normalized_trajectory.episodes:
            self.assert_learning_admission(_domain_from_episode(episode))
        normalized_decisions = [_decision(decision) for decision in decisions]
        payload = _normalize_payload("trajectory", {"trajectory": normalized_trajectory.to_dict(), "decisions": [decision.to_dict() for decision in normalized_decisions], "bandit_state": bandit_state})
        target_digest = content_digest(normalized_trajectory.to_dict())
        request_digest = content_digest(payload)
        resolved_id = command_id or f"trajectory:{request_digest}"
        timestamp = _now_ms(now)
        command = AutonomousLearningFeedbackCommand(
            command_id=resolved_id,
            operation="trajectory",
            target_id=normalized_trajectory.trajectory_id,
            target_digest=target_digest,
            request_digest=request_digest,
            payload=payload,
            created_at=timestamp,
            updated_at=timestamp,
        )
        return outbox.enqueue(command)

    def apply_feedback_command(self, command: AutonomousLearningFeedbackCommand | Mapping[str, Any], *, evaluator: BrainOutcomeEvaluator) -> dict[str, Any]:
        normalized = command if isinstance(command, AutonomousLearningFeedbackCommand) else AutonomousLearningFeedbackCommand.from_dict(command)
        payload = normalized.payload
        if normalized.operation == "single":
            decision, report = self.settle_episode(payload["episode"], evaluator=evaluator, decision=payload["decision"], bandit_state=payload["bandit_state"])
            return {"operation": "single", "target_id": normalized.target_id, "decision": decision.to_dict(), "report": dict(report), "retention": "value_only_learning_settlement", "secret_material": _SECRET_MATERIAL}
        result = self.settle_trajectory(payload["trajectory"], evaluator=evaluator, decisions=payload["decisions"], bandit_state=payload["bandit_state"])
        return {"operation": "trajectory", "target_id": normalized.target_id, "result": result.to_dict(), "retention": "value_only_learning_settlement", "secret_material": _SECRET_MATERIAL}


class AutonomousLearningFeedbackWorker:
    """Lease and apply queued value-only decisions through one controller."""

    def __init__(self, outbox: InMemoryAutonomousLearningFeedbackOutbox, controller: AutonomousLearningController, evaluator: BrainOutcomeEvaluator) -> None:
        if not isinstance(outbox, InMemoryAutonomousLearningFeedbackOutbox):
            raise BrainRunError("feedback worker outbox is malformed")
        if not isinstance(controller, AutonomousLearningController):
            raise BrainRunError("feedback worker controller is malformed")
        if not isinstance(evaluator, BrainOutcomeEvaluator):
            raise BrainRunError("feedback worker evaluator is malformed")
        self.outbox = outbox
        self.controller = controller
        self.evaluator = evaluator

    def run(self, *, worker_id: str, limit: int = 16, lease_ms: int = 30_000, now: int | None = None) -> dict[str, Any]:
        worker = _identifier("feedback worker_id", worker_id, 256)
        max_rows = _bounded_integer("feedback worker limit", limit, 1, MAX_AUTONOMOUS_LEARNING_FEEDBACK_WORKER_ROWS)
        rows: list[dict[str, Any]] = []
        inspected = applied = failed = 0
        for _ in range(max_rows):
            command = self.outbox.claim(worker, lease_ms=lease_ms, now=now)
            if command is None:
                break
            inspected += 1
            try:
                result = self.controller.apply_feedback_command(command, evaluator=self.evaluator)
                result_digest = content_digest(result)
                settled = self.outbox.mark_applied(command.command_id, worker, result_digest, now=now)
                rows.append({"command_id": command.command_id, "status": settled.status, "attempts": settled.attempts, "result_digest": result_digest, "error_class": None})
                applied += 1
            except Exception as error:
                retryable = isinstance(error, (TimeoutError, ConnectionError))
                try:
                    settled = self.outbox.mark_failed(command.command_id, worker, type(error).__name__, retryable=retryable, now=now)
                except ArgumentError as fencing_error:
                    self.outbox.reconcile_expired(now=now)
                    observed = self.outbox.get(command.command_id)
                    if observed is None or observed.status == "leased":
                        raise fencing_error from error
                    settled = observed
                rows.append({"command_id": command.command_id, "status": settled.status, "attempts": settled.attempts, "result_digest": None, "error_class": type(error).__name__})
                failed += 1
        return {
            "schema": AUTONOMOUS_LEARNING_FEEDBACK_OUTBOX_SCHEMA,
            "worker_id": worker,
            "inspected": inspected,
            "applied": applied,
            "failed": failed,
            "rows": rows,
            "retention": "value_only_feedback_dispatch_metadata",
            "secret_material": _SECRET_MATERIAL,
        }


__all__ = [
    "AUTONOMOUS_LEARNING_CONTROLLER_SCHEMA",
    "AUTONOMOUS_LEARNING_FEEDBACK_OUTBOX_SCHEMA",
    "AUTONOMOUS_LEARNING_FEEDBACK_OUTBOX_SQLITE_SCHEMA",
    "MAX_AUTONOMOUS_LEARNING_FEEDBACK_COMMANDS",
    "MAX_AUTONOMOUS_LEARNING_FEEDBACK_LEASE_MS",
    "MAX_AUTONOMOUS_LEARNING_FEEDBACK_ATTEMPTS",
    "MAX_AUTONOMOUS_LEARNING_FEEDBACK_WORKER_ROWS",
    "MAX_AUTONOMOUS_LEARNING_FEEDBACK_SNAPSHOT_BYTES",
    "AutonomousLearningFeedbackCommand",
    "validate_autonomous_learning_feedback_command",
    "InMemoryAutonomousLearningFeedbackOutbox",
    "validate_autonomous_learning_feedback_snapshot",
    "AutonomousLearningFeedbackSnapshotTextStore",
    "TransactionalAutonomousLearningFeedbackSnapshotTextStore",
    "InMemoryAutonomousLearningFeedbackPersistence",
    "JsonAutonomousLearningFeedbackPersistence",
    "TransactionalJsonAutonomousLearningFeedbackPersistence",
    "SQLiteAutonomousLearningFeedbackPersistence",
    "AutonomousLearningFeedbackPersistenceCoordinator",
    "AutonomousLearningController",
    "AutonomousLearningFeedbackWorker",
]
