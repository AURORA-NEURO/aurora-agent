"""Restart-safe, value-only objective state for long-horizon autonomous work.

The provider runtime, mission executor, episodic memory, and learning ledger each retain a
different slice of an autonomous run.  This module supplies the missing objective boundary: a
caller can keep one bounded goal alive across attempts, route changes, evaluator handoffs, and
process restarts without writing the original task, prompt, provider response, tool arguments, or
credentials to the goal store.

Goal text is accepted only transiently by :func:`goal_task_digest`.  All durable fields are
identifiers, SHA-256 digests, bounded criterion state, and explicit lifecycle metadata, including
separate outcome, evaluator, learning-state, and cross-domain progress identities. The ledger is
deliberately domain-neutral; domain packs and evaluators remain authoritative for meaning,
authorization, and completion evidence.
"""

from __future__ import annotations

from dataclasses import dataclass, field
import hashlib
import json
import math
import sqlite3
import threading
import time
from collections.abc import Mapping, Sequence
from typing import Any, Callable, Literal, Protocol


GOAL_SCHEMA = "bioprism-autonomous-goal/0.1"
GOAL_EVENT_SCHEMA = "bioprism-autonomous-goal-event/0.1"
GOAL_SNAPSHOT_SCHEMA = "bioprism-autonomous-goal-snapshot/0.1"
GOAL_STEP_SCHEMA = "bioprism-autonomous-goal-step/0.1"
GOAL_RETENTION = "value_only_goal_state;task_prompt_response_tool_payloads_and_credentials_not_retained"
MAX_GOALS = 4_096
MAX_GOAL_EVENTS = 16_384
MAX_GOAL_SNAPSHOT_BYTES = 4_000_000
MAX_GOAL_CRITERIA = 64
MAX_GOAL_BLOCKERS = 32
MAX_GOAL_IDENTIFIER_BYTES = 256
MAX_GOAL_DIGEST_BYTES = 64

GoalStatus = Literal[
    "ready",
    "running",
    "paused",
    "blocked",
    "failed",
    "completed",
    "cancelled",
]
CriterionStatus = Literal["pending", "satisfied", "failed", "waived"]
_UNSET = object()

_GOAL_COMPLETED_RESULTS = frozenset({"completed", "completed_without_replan", "children_completed"})
_GOAL_PAUSED_RESULTS = frozenset(
    {
        "approval_required",
        "reconciliation_required",
        "turn_limit_reached",
        "paused",
        "stage_blocked",
        "children_partial",
        "child_incomplete",
    }
)
_GOAL_BLOCKED_RESULTS = frozenset({"route_review_required", "planning_review_required", "provider_disagreement"})

_ALLOWED_TRANSITIONS: dict[GoalStatus, frozenset[GoalStatus]] = {
    "ready": frozenset({"running", "blocked", "cancelled"}),
    "running": frozenset({"paused", "blocked", "failed", "completed", "cancelled"}),
    "paused": frozenset({"running", "blocked", "cancelled"}),
    "blocked": frozenset({"ready", "cancelled"}),
    "failed": frozenset({"ready", "cancelled"}),
    "completed": frozenset(),
    "cancelled": frozenset(),
}


class AutonomousGoalError(RuntimeError):
    """A goal request, transition, persistence operation, or integrity check was refused."""


class AutonomousGoalConflict(AutonomousGoalError):
    """A goal changed after the caller's optimistic revision was read."""


def goal_status_for_result(result_status: str, *, criteria_complete: bool) -> GoalStatus:
    """Map a bounded runtime result into a goal lifecycle state without trusting provider text."""

    if not isinstance(result_status, str) or not result_status.strip():
        return "failed"
    if result_status in _GOAL_COMPLETED_RESULTS:
        return "completed" if criteria_complete else "paused"
    if result_status in _GOAL_PAUSED_RESULTS:
        return "paused"
    if result_status in _GOAL_BLOCKED_RESULTS:
        return "blocked"
    return "failed"


def goal_task_digest(task: str) -> str:
    """Hash transient task text for durable goal identity without retaining the text."""

    if not isinstance(task, str) or not task.strip() or "\x00" in task:
        raise AutonomousGoalError("goal task must be a non-empty NUL-free string")
    encoded = task.encode("utf-8")
    if len(encoded) > 32_000:
        raise AutonomousGoalError("goal task exceeds the bounded input size")
    return hashlib.sha256(encoded).hexdigest()


def _digest(value: Any) -> str:
    try:
        encoded = json.dumps(
            value,
            ensure_ascii=False,
            sort_keys=True,
            separators=(",", ":"),
            allow_nan=False,
        ).encode("utf-8")
    except (TypeError, ValueError) as error:
        raise AutonomousGoalError("goal value is not canonical JSON") from error
    return hashlib.sha256(encoded).hexdigest()


def _identifier(value: Any, *, name: str) -> str:
    if not isinstance(value, str) or not value.strip() or "\x00" in value:
        raise AutonomousGoalError(f"{name} must be a non-empty NUL-free string")
    if len(value.encode("utf-8")) > MAX_GOAL_IDENTIFIER_BYTES:
        raise AutonomousGoalError(f"{name} exceeds its bounded size")
    return value.strip()


def _digest_value(value: Any, *, name: str, allow_none: bool = False) -> str | None:
    if value is None and allow_none:
        return None
    if not isinstance(value, str) or len(value) != MAX_GOAL_DIGEST_BYTES:
        raise AutonomousGoalError(f"{name} must be a lowercase SHA-256 digest")
    if any(character not in "0123456789abcdef" for character in value):
        raise AutonomousGoalError(f"{name} must be a lowercase SHA-256 digest")
    return value


def _valid_digest(value: Any) -> bool:
    return isinstance(value, str) and len(value) == MAX_GOAL_DIGEST_BYTES and all(
        character in "0123456789abcdef" for character in value
    )


def _bounded_sequence(value: Any, *, name: str, maximum: int) -> list[Any]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes, bytearray)):
        raise AutonomousGoalError(f"{name} must be a sequence")
    if len(value) > maximum:
        raise AutonomousGoalError(f"{name} exceeds its bound")
    return list(value)


@dataclass(frozen=True, slots=True)
class AutonomousGoalCriterion:
    """A caller/evaluator-owned completion claim represented only by digests."""

    criterion_id: str
    criterion_digest: str
    required: bool = True
    status: CriterionStatus = "pending"
    weight: float = 1.0
    evidence_digest: str | None = None

    def __post_init__(self) -> None:
        object.__setattr__(self, "criterion_id", _identifier(self.criterion_id, name="criterion_id"))
        object.__setattr__(
            self,
            "criterion_digest",
            _digest_value(self.criterion_digest, name="criterion_digest"),
        )
        if not isinstance(self.required, bool):
            raise AutonomousGoalError("criterion.required must be boolean")
        if self.status not in {"pending", "satisfied", "failed", "waived"}:
            raise AutonomousGoalError("criterion.status is unsupported")
        if (
            isinstance(self.weight, bool)
            or not isinstance(self.weight, (int, float))
            or not math.isfinite(float(self.weight))
            or not 0.0 < float(self.weight) <= 1_000.0
        ):
            raise AutonomousGoalError("criterion.weight must be within (0, 1000]")
        object.__setattr__(self, "weight", float(self.weight))
        object.__setattr__(
            self,
            "evidence_digest",
            _digest_value(self.evidence_digest, name="criterion.evidence_digest", allow_none=True),
        )

    @classmethod
    def from_mapping(cls, value: Mapping[str, Any]) -> "AutonomousGoalCriterion":
        if not isinstance(value, Mapping):
            raise AutonomousGoalError("goal criterion must be a mapping")
        return cls(
            criterion_id=value.get("criterion_id"),
            criterion_digest=value.get("criterion_digest"),
            required=value.get("required", True),
            status=value.get("status", "pending"),
            weight=value.get("weight", 1.0),
            evidence_digest=value.get("evidence_digest"),
        )

    def to_dict(self) -> dict[str, Any]:
        return {
            "criterion_id": self.criterion_id,
            "criterion_digest": self.criterion_digest,
            "required": self.required,
            "status": self.status,
            "weight": int(self.weight) if self.weight.is_integer() else self.weight,
            "evidence_digest": self.evidence_digest,
        }


def _normalize_criteria(value: Any) -> tuple[AutonomousGoalCriterion, ...]:
    rows = _bounded_sequence(value, name="goal.criteria", maximum=MAX_GOAL_CRITERIA)
    criteria = tuple(
        item if isinstance(item, AutonomousGoalCriterion) else AutonomousGoalCriterion.from_mapping(item)
        for item in rows
    )
    if len({criterion.criterion_id for criterion in criteria}) != len(criteria):
        raise AutonomousGoalError("goal criteria contain duplicate criterion_id values")
    return tuple(sorted(criteria, key=lambda criterion: criterion.criterion_id))


def _normalize_blockers(value: Any) -> tuple[str, ...]:
    rows = _bounded_sequence(value, name="goal.blockers", maximum=MAX_GOAL_BLOCKERS)
    blockers = tuple(_identifier(item, name="goal blocker") for item in rows)
    return tuple(sorted(set(blockers)))


def _goal_identity(record: "AutonomousGoalRecord") -> dict[str, Any]:
    """Return only immutable creation fields for idempotent goal creation."""

    return {
        "goal_id": record.goal_id,
        "task_digest": record.task_digest,
        "domain": record.domain,
        "capability": record.capability,
        "risk_class": record.risk_class,
        "max_attempts": record.max_attempts,
        "criteria": [
            {
                "criterion_id": criterion.criterion_id,
                "criterion_digest": criterion.criterion_digest,
                "required": criterion.required,
                "weight": int(criterion.weight) if criterion.weight.is_integer() else criterion.weight,
            }
            for criterion in record.criteria
        ],
    }


@dataclass(frozen=True, slots=True)
class AutonomousGoalRecord:
    """Content-addressed current state for one long-horizon objective."""

    goal_id: str
    task_digest: str
    domain: str
    capability: str | None
    risk_class: str | None
    status: GoalStatus
    attempt: int
    max_attempts: int
    revision: int
    criteria: tuple[AutonomousGoalCriterion, ...] = field(default_factory=tuple)
    blockers: tuple[str, ...] = field(default_factory=tuple)
    next_action_digest: str | None = None
    outcome_digest: str | None = None
    evaluator_digest: str | None = None
    learning_state_digest: str | None = None
    progress_digest: str | None = None
    created_ns: int = 0
    updated_ns: int = 0
    state_digest: str = field(init=False)

    def __post_init__(self) -> None:
        object.__setattr__(self, "goal_id", _identifier(self.goal_id, name="goal_id"))
        object.__setattr__(self, "task_digest", _digest_value(self.task_digest, name="task_digest"))
        object.__setattr__(self, "domain", _identifier(self.domain, name="goal.domain"))
        for name in ("capability", "risk_class"):
            value = getattr(self, name)
            object.__setattr__(self, name, None if value is None else _identifier(value, name=f"goal.{name}"))
        if self.status not in _ALLOWED_TRANSITIONS:
            raise AutonomousGoalError("goal.status is unsupported")
        for name in ("attempt", "max_attempts", "revision", "created_ns", "updated_ns"):
            value = getattr(self, name)
            if not isinstance(value, int) or isinstance(value, bool) or value < 0:
                raise AutonomousGoalError(f"goal.{name} must be a non-negative integer")
        if not 1 <= self.max_attempts <= 128:
            raise AutonomousGoalError("goal.max_attempts must be between 1 and 128")
        if self.attempt > self.max_attempts:
            raise AutonomousGoalError("goal.attempt cannot exceed max_attempts")
        normalized_criteria = _normalize_criteria(self.criteria)
        object.__setattr__(self, "criteria", normalized_criteria)
        object.__setattr__(self, "blockers", _normalize_blockers(self.blockers))
        object.__setattr__(
            self,
            "next_action_digest",
            _digest_value(self.next_action_digest, name="goal.next_action_digest", allow_none=True),
        )
        for name in ("outcome_digest", "evaluator_digest", "learning_state_digest", "progress_digest"):
            object.__setattr__(
                self,
                name,
                _digest_value(getattr(self, name), name=f"goal.{name}", allow_none=True),
            )
        if self.updated_ns < self.created_ns:
            raise AutonomousGoalError("goal.updated_ns cannot precede created_ns")
        object.__setattr__(self, "state_digest", _digest(self._state_payload()))

    def _state_payload(self) -> dict[str, Any]:
        return {
            "schema": GOAL_SCHEMA,
            "goal_id": self.goal_id,
            "task_digest": self.task_digest,
            "domain": self.domain,
            "capability": self.capability,
            "risk_class": self.risk_class,
            "status": self.status,
            "attempt": self.attempt,
            "max_attempts": self.max_attempts,
            "revision": self.revision,
            "criteria": [criterion.to_dict() for criterion in self.criteria],
            "blockers": list(self.blockers),
            "next_action_digest": self.next_action_digest,
            "outcome_digest": self.outcome_digest,
            "evaluator_digest": self.evaluator_digest,
            "learning_state_digest": self.learning_state_digest,
            "progress_digest": self.progress_digest,
            "created_ns": self.created_ns,
            "updated_ns": self.updated_ns,
        }

    def _legacy_state_payload(self) -> dict[str, Any]:
        """Reconstruct the pre-settlement state payload for SQLite restart migration."""

        payload = self._state_payload()
        for key in ("outcome_digest", "evaluator_digest", "learning_state_digest", "progress_digest"):
            payload.pop(key, None)
        return payload

    @classmethod
    def from_mapping(cls, value: Mapping[str, Any]) -> "AutonomousGoalRecord":
        if not isinstance(value, Mapping) or value.get("schema") != GOAL_SCHEMA:
            raise AutonomousGoalError("goal record has an invalid schema")
        if value.get("retention") != GOAL_RETENTION or value.get("secret_material") != "never_returned":
            raise AutonomousGoalError("goal record retention contract is invalid")
        record = cls(
            goal_id=value.get("goal_id"),
            task_digest=value.get("task_digest"),
            domain=value.get("domain"),
            capability=value.get("capability"),
            risk_class=value.get("risk_class"),
            status=value.get("status"),
            attempt=value.get("attempt"),
            max_attempts=value.get("max_attempts"),
            revision=value.get("revision"),
            criteria=value.get("criteria", ()),
            blockers=value.get("blockers", ()),
            next_action_digest=value.get("next_action_digest"),
            outcome_digest=value.get("outcome_digest"),
            evaluator_digest=value.get("evaluator_digest"),
            learning_state_digest=value.get("learning_state_digest"),
            progress_digest=value.get("progress_digest"),
            created_ns=value.get("created_ns"),
            updated_ns=value.get("updated_ns"),
        )
        supplied = value.get("state_digest")
        if supplied != record.state_digest:
            legacy_fields = ("outcome_digest", "evaluator_digest", "learning_state_digest", "progress_digest")
            if any(field_name in value for field_name in legacy_fields) or supplied != _digest(record._legacy_state_payload()):
                raise AutonomousGoalError("goal state_digest does not match its content")
        return record

    def to_dict(self) -> dict[str, Any]:
        return {
            **self._state_payload(),
            "state_digest": self.state_digest,
            "retention": GOAL_RETENTION,
            "secret_material": "never_returned",
        }

    @property
    def required_criteria_complete(self) -> bool:
        return all(
            not criterion.required or criterion.status in {"satisfied", "waived"}
            for criterion in self.criteria
        )


class AutonomousGoalLedger:
    """SQLite-backed objective lifecycle with optimistic transitions and hash-chain events."""

    def __init__(
        self,
        path: str = ":memory:",
        *,
        max_goals: int = MAX_GOALS,
        max_events: int = MAX_GOAL_EVENTS,
        clock: Callable[[], int] | None = None,
    ) -> None:
        if not isinstance(path, str) or not path.strip():
            raise AutonomousGoalError("goal ledger path must be a non-empty string")
        if not isinstance(max_goals, int) or isinstance(max_goals, bool) or not 1 <= max_goals <= MAX_GOALS:
            raise AutonomousGoalError(f"max_goals must be between 1 and {MAX_GOALS}")
        if not isinstance(max_events, int) or isinstance(max_events, bool) or not 1 <= max_events <= MAX_GOAL_EVENTS:
            raise AutonomousGoalError(f"max_events must be between 1 and {MAX_GOAL_EVENTS}")
        self.path = path
        self.max_goals = max_goals
        self.max_events = max_events
        self._clock = clock or time.time_ns
        self._lock = threading.RLock()
        self._connection = sqlite3.connect(path, isolation_level=None, check_same_thread=False)
        self._connection.row_factory = sqlite3.Row
        self._connection.execute("PRAGMA foreign_keys = ON")
        with self._lock:
            self._connection.executescript(
                """
                CREATE TABLE IF NOT EXISTS autonomous_goals (
                    goal_id TEXT PRIMARY KEY,
                    state_json TEXT NOT NULL,
                    state_digest TEXT NOT NULL,
                    status TEXT NOT NULL,
                    domain TEXT NOT NULL,
                    revision INTEGER NOT NULL,
                    created_ns INTEGER NOT NULL,
                    updated_ns INTEGER NOT NULL
                );
                CREATE TABLE IF NOT EXISTS autonomous_goal_events (
                    sequence INTEGER PRIMARY KEY AUTOINCREMENT,
                    goal_id TEXT NOT NULL,
                    event_type TEXT NOT NULL,
                    payload_json TEXT NOT NULL,
                    previous_digest TEXT NOT NULL,
                    event_digest TEXT NOT NULL UNIQUE,
                    created_ns INTEGER NOT NULL,
                    FOREIGN KEY(goal_id) REFERENCES autonomous_goals(goal_id)
                );
                """
            )
            columns = {
                str(row[1])
                for row in self._connection.execute("PRAGMA table_info(autonomous_goals)").fetchall()
            }
            if "status" not in columns:
                self._connection.execute("ALTER TABLE autonomous_goals ADD COLUMN status TEXT NOT NULL DEFAULT 'ready'")
            if "domain" not in columns:
                self._connection.execute("ALTER TABLE autonomous_goals ADD COLUMN domain TEXT NOT NULL DEFAULT 'unknown'")
            self._connection.execute("CREATE INDEX IF NOT EXISTS autonomous_goals_status_idx ON autonomous_goals(status)")
            self._connection.execute("CREATE INDEX IF NOT EXISTS autonomous_goals_domain_idx ON autonomous_goals(domain)")

    def close(self) -> None:
        with self._lock:
            self._connection.close()

    def __enter__(self) -> "AutonomousGoalLedger":
        return self

    def __exit__(self, _type: Any, _value: Any, _traceback: Any) -> None:
        self.close()

    def create(
        self,
        *,
        goal_id: str,
        task_digest: str,
        domain: str,
        capability: str | None = None,
        risk_class: str | None = None,
        criteria: Sequence[AutonomousGoalCriterion | Mapping[str, Any]] = (),
        max_attempts: int = 8,
        now_ns: int | None = None,
    ) -> AutonomousGoalRecord:
        now = self._now(now_ns)
        record = AutonomousGoalRecord(
            goal_id=goal_id,
            task_digest=task_digest,
            domain=domain,
            capability=capability,
            risk_class=risk_class,
            status="ready",
            attempt=0,
            max_attempts=max_attempts,
            revision=0,
            criteria=tuple(criteria),
            created_ns=now,
            updated_ns=now,
        )
        with self._lock:
            try:
                self._connection.execute("BEGIN IMMEDIATE")
                existing = self._connection.execute(
                    "SELECT state_json FROM autonomous_goals WHERE goal_id = ?",
                    (record.goal_id,),
                ).fetchone()
                if existing is not None:
                    prior = AutonomousGoalRecord.from_mapping(json.loads(existing["state_json"]))
                    if _goal_identity(prior) != _goal_identity(record):
                        raise AutonomousGoalConflict("goal_id already exists with a different identity")
                    self._connection.execute("COMMIT")
                    return prior
                count = int(self._connection.execute("SELECT COUNT(*) FROM autonomous_goals").fetchone()[0])
                if count >= self.max_goals:
                    raise AutonomousGoalError("goal ledger capacity is exhausted")
                self._insert_record(record)
                self._append_event("created", record, now)
                self._connection.execute("COMMIT")
                return record
            except Exception:
                self._connection.execute("ROLLBACK")
                raise

    def get(self, goal_id: str) -> AutonomousGoalRecord | None:
        identifier = _identifier(goal_id, name="goal_id")
        with self._lock:
            row = self._connection.execute(
                "SELECT state_json FROM autonomous_goals WHERE goal_id = ?",
                (identifier,),
            ).fetchone()
        return None if row is None else AutonomousGoalRecord.from_mapping(json.loads(row["state_json"]))

    def list(
        self,
        *,
        domain: str | None = None,
        statuses: Sequence[GoalStatus] = (),
        limit: int = 128,
    ) -> tuple[AutonomousGoalRecord, ...]:
        if not isinstance(limit, int) or isinstance(limit, bool) or not 1 <= limit <= 512:
            raise AutonomousGoalError("goal list limit must be between 1 and 512")
        normalized_domain = None if domain is None else _identifier(domain, name="goal.domain")
        normalized_statuses = tuple(statuses)
        if any(status not in _ALLOWED_TRANSITIONS for status in normalized_statuses):
            raise AutonomousGoalError("goal list contains an unsupported status")
        clauses: list[str] = []
        values: list[Any] = []
        if normalized_domain is not None:
            clauses.append("json_extract(state_json, '$.domain') = ?")
            values.append(normalized_domain)
        if normalized_statuses:
            clauses.append("json_extract(state_json, '$.status') IN (" + ",".join("?" for _ in normalized_statuses) + ")")
            values.extend(normalized_statuses)
        where = " WHERE " + " AND ".join(clauses) if clauses else ""
        with self._lock:
            rows = self._connection.execute(
                "SELECT state_json FROM autonomous_goals" + where + " ORDER BY updated_ns DESC, goal_id ASC LIMIT ?",
                (*values, limit),
            ).fetchall()
        return tuple(AutonomousGoalRecord.from_mapping(json.loads(row["state_json"])) for row in rows)

    def transition(
        self,
        goal_id: str,
        status: GoalStatus,
        *,
        expected_revision: int | None = None,
        criterion_updates: Sequence[Mapping[str, Any]] = (),
        blockers: Sequence[str] = (),
        next_action_digest: str | None = None,
        outcome_digest: str | None | object = _UNSET,
        evaluator_digest: str | None | object = _UNSET,
        learning_state_digest: str | None | object = _UNSET,
        progress_digest: str | None | object = _UNSET,
        now_ns: int | None = None,
    ) -> AutonomousGoalRecord:
        current = self.get(goal_id)
        if current is None:
            raise AutonomousGoalError(f"goal {goal_id!r} was not found")
        if status not in _ALLOWED_TRANSITIONS:
            raise AutonomousGoalError("goal.status is unsupported")
        if status != current.status and status not in _ALLOWED_TRANSITIONS[current.status]:
            raise AutonomousGoalError(f"goal cannot transition from {current.status} to {status}")
        if status == "ready" and current.status == "failed" and current.attempt >= current.max_attempts:
            raise AutonomousGoalError("goal attempt budget is exhausted")
        if expected_revision is not None:
            if not isinstance(expected_revision, int) or isinstance(expected_revision, bool) or expected_revision < 0:
                raise AutonomousGoalError("expected_revision must be a non-negative integer")
            if current.revision != expected_revision:
                raise AutonomousGoalConflict(
                    f"goal revision conflict: expected {expected_revision}, observed {current.revision}"
                )
        criteria = self._apply_criterion_updates(current.criteria, criterion_updates)
        if status == "completed" and not all(
            not criterion.required or criterion.status in {"satisfied", "waived"}
            for criterion in criteria
        ):
            raise AutonomousGoalError("goal cannot complete while a required criterion is unresolved")
        attempt = current.attempt
        if status == "running" and current.status != "running":
            if attempt >= current.max_attempts:
                raise AutonomousGoalError("goal attempt budget is exhausted")
            attempt += 1
        now = self._now(now_ns)
        updated = AutonomousGoalRecord(
            goal_id=current.goal_id,
            task_digest=current.task_digest,
            domain=current.domain,
            capability=current.capability,
            risk_class=current.risk_class,
            status=status,
            attempt=attempt,
            max_attempts=current.max_attempts,
            revision=current.revision + 1,
            criteria=criteria,
            blockers=tuple(blockers),
            next_action_digest=next_action_digest,
            outcome_digest=current.outcome_digest if outcome_digest is _UNSET else outcome_digest,
            evaluator_digest=current.evaluator_digest if evaluator_digest is _UNSET else evaluator_digest,
            learning_state_digest=current.learning_state_digest if learning_state_digest is _UNSET else learning_state_digest,
            progress_digest=current.progress_digest if progress_digest is _UNSET else progress_digest,
            created_ns=current.created_ns,
            updated_ns=now,
        )
        with self._lock:
            try:
                self._connection.execute("BEGIN IMMEDIATE")
                row = self._connection.execute(
                    "SELECT state_json FROM autonomous_goals WHERE goal_id = ?",
                    (current.goal_id,),
                ).fetchone()
                if row is None:
                    raise AutonomousGoalError(f"goal {current.goal_id!r} disappeared")
                observed = AutonomousGoalRecord.from_mapping(json.loads(row["state_json"]))
                if observed.state_digest != current.state_digest:
                    raise AutonomousGoalConflict("goal changed while transition was being prepared")
                self._insert_record(updated)
                self._append_event("transition", updated, now)
                self._connection.execute("COMMIT")
                return updated
            except Exception:
                self._connection.execute("ROLLBACK")
                raise

    def update_criteria(
        self,
        goal_id: str,
        criterion_updates: Sequence[Mapping[str, Any]],
        *,
        expected_revision: int | None = None,
        now_ns: int | None = None,
    ) -> AutonomousGoalRecord:
        current = self.get(goal_id)
        if current is None:
            raise AutonomousGoalError(f"goal {goal_id!r} was not found")
        return self.transition(
            goal_id,
            current.status,
            expected_revision=expected_revision,
            criterion_updates=criterion_updates,
            blockers=current.blockers,
            next_action_digest=current.next_action_digest,
            now_ns=now_ns,
        )

    def verify_integrity(self) -> dict[str, Any]:
        with self._lock:
            rows = self._connection.execute(
                "SELECT sequence, goal_id, event_type, payload_json, previous_digest, event_digest, created_ns "
                "FROM autonomous_goal_events ORDER BY sequence ASC"
            ).fetchall()
            goals = self._connection.execute("SELECT state_json FROM autonomous_goals").fetchall()
        previous = ""
        for row in rows:
            body = {
                "schema": GOAL_EVENT_SCHEMA,
                "sequence": row["sequence"],
                "goal_id": row["goal_id"],
                "event_type": row["event_type"],
                "payload": json.loads(row["payload_json"]),
                "previous_digest": row["previous_digest"],
                "created_ns": row["created_ns"],
                "retention": GOAL_RETENTION,
                "secret_material": "never_returned",
            }
            if row["previous_digest"] != previous or _digest(body) != row["event_digest"]:
                raise AutonomousGoalError(f"goal event hash chain breaks at sequence {row['sequence']}")
            previous = row["event_digest"]
        for row in goals:
            AutonomousGoalRecord.from_mapping(json.loads(row["state_json"]))
        return {
            "schema": GOAL_EVENT_SCHEMA,
            "ok": True,
            "goals": len(goals),
            "events": len(rows),
            "head_digest": previous,
            "retention": GOAL_RETENTION,
            "secret_material": "never_returned",
        }

    def snapshot(self) -> dict[str, Any]:
        """Return a strict, portable lifecycle image for cross-process goal handoff."""

        with self._lock:
            integrity = self.verify_integrity()
            event_rows = self._connection.execute(
                "SELECT sequence, goal_id, event_type, payload_json, previous_digest, event_digest, created_ns "
                "FROM autonomous_goal_events ORDER BY sequence ASC"
            ).fetchall()
            goal_rows = self._connection.execute(
                "SELECT state_json FROM autonomous_goals ORDER BY goal_id ASC"
            ).fetchall()
            events: list[dict[str, Any]] = []
            for row in event_rows:
                try:
                    payload = json.loads(row["payload_json"])
                except (TypeError, ValueError, json.JSONDecodeError) as error:
                    raise AutonomousGoalError("goal event contains invalid JSON") from error
                events.append(
                    {
                        "schema": GOAL_EVENT_SCHEMA,
                        "sequence": int(row["sequence"]),
                        "goal_id": str(row["goal_id"]),
                        "event_type": str(row["event_type"]),
                        "payload": payload,
                        "previous_digest": str(row["previous_digest"]),
                        "created_ns": int(row["created_ns"]),
                        "event_digest": str(row["event_digest"]),
                        "retention": GOAL_RETENTION,
                        "secret_material": "never_returned",
                    }
                )
            goals = [AutonomousGoalRecord.from_mapping(json.loads(row["state_json"])).to_dict() for row in goal_rows]
            return _build_goal_snapshot(
                goals,
                events,
                max_goals=self.max_goals,
                max_events=self.max_events,
                max_bytes=MAX_GOAL_SNAPSHOT_BYTES,
                expected_goal_count=int(integrity["goals"]),
            )

    def restore(self, snapshot: Mapping[str, Any]) -> None:
        """Atomically replace goals and rebuild their current-state index from a validated image."""

        normalized = _normalize_goal_snapshot(
            snapshot,
            max_goals=self.max_goals,
            max_events=self.max_events,
            max_bytes=MAX_GOAL_SNAPSHOT_BYTES,
        )
        with self._lock:
            try:
                self._connection.execute("BEGIN IMMEDIATE")
                self._connection.execute("DELETE FROM autonomous_goal_events")
                self._connection.execute("DELETE FROM autonomous_goals")
                self._connection.execute(
                    "DELETE FROM sqlite_sequence WHERE name = 'autonomous_goal_events'"
                )
                for goal in normalized["goals"]:
                    self._insert_record(AutonomousGoalRecord.from_mapping(goal))
                for event in normalized["events"]:
                    self._connection.execute(
                        "INSERT INTO autonomous_goal_events "
                        "(sequence, goal_id, event_type, payload_json, previous_digest, event_digest, created_ns) "
                        "VALUES (?, ?, ?, ?, ?, ?, ?)",
                        (
                            event["sequence"],
                            event["goal_id"],
                            event["event_type"],
                            json.dumps(event["payload"], ensure_ascii=False, sort_keys=True, separators=(",", ":")),
                            event["previous_digest"],
                            event["event_digest"],
                            event["created_ns"],
                        ),
                    )
                self._connection.execute("COMMIT")
            except Exception:
                self._connection.execute("ROLLBACK")
                raise

    def stats(self) -> dict[str, Any]:
        with self._lock:
            goals = self._connection.execute("SELECT status, COUNT(*) AS count FROM autonomous_goals GROUP BY status").fetchall()
            events = int(self._connection.execute("SELECT COUNT(*) FROM autonomous_goal_events").fetchone()[0])
        counts = {str(row["status"]): int(row["count"]) for row in goals}
        return {
            "schema": GOAL_SCHEMA,
            "total": sum(counts.values()),
            "statuses": counts,
            "events": events,
            "retention": GOAL_RETENTION,
            "secret_material": "never_returned",
        }

    def _apply_criterion_updates(
        self,
        current: Sequence[AutonomousGoalCriterion],
        updates: Sequence[Mapping[str, Any]],
    ) -> tuple[AutonomousGoalCriterion, ...]:
        rows = _bounded_sequence(updates, name="criterion_updates", maximum=MAX_GOAL_CRITERIA)
        by_id = {criterion.criterion_id: criterion for criterion in current}
        for raw in rows:
            if not isinstance(raw, Mapping):
                raise AutonomousGoalError("criterion update must be a mapping")
            criterion_id = _identifier(raw.get("criterion_id"), name="criterion update criterion_id")
            prior = by_id.get(criterion_id)
            if prior is None:
                raise AutonomousGoalError(f"criterion update references unknown criterion {criterion_id!r}")
            status = raw.get("status", prior.status)
            if status not in {"pending", "satisfied", "failed", "waived"}:
                raise AutonomousGoalError("criterion update status is unsupported")
            if prior.status in {"satisfied", "waived"} and status not in {prior.status}:
                raise AutonomousGoalError("satisfied or waived criteria cannot regress")
            evidence = raw.get("evidence_digest", prior.evidence_digest)
            by_id[criterion_id] = AutonomousGoalCriterion(
                criterion_id=prior.criterion_id,
                criterion_digest=prior.criterion_digest,
                required=prior.required,
                status=status,
                weight=prior.weight,
                evidence_digest=evidence,
            )
        return tuple(sorted(by_id.values(), key=lambda criterion: criterion.criterion_id))

    def _insert_record(self, record: AutonomousGoalRecord) -> None:
        self._connection.execute(
            "INSERT INTO autonomous_goals "
            "(goal_id, state_json, state_digest, status, domain, revision, created_ns, updated_ns) VALUES (?, ?, ?, ?, ?, ?, ?, ?) "
            "ON CONFLICT(goal_id) DO UPDATE SET state_json=excluded.state_json, state_digest=excluded.state_digest, "
            "status=excluded.status, domain=excluded.domain, revision=excluded.revision, "
            "created_ns=excluded.created_ns, updated_ns=excluded.updated_ns",
            (
                record.goal_id,
                json.dumps(record.to_dict(), ensure_ascii=False, sort_keys=True, separators=(",", ":")),
                record.state_digest,
                record.status,
                record.domain,
                record.revision,
                record.created_ns,
                record.updated_ns,
            ),
        )

    def _append_event(self, event_type: str, record: AutonomousGoalRecord, now_ns: int) -> None:
        if not isinstance(event_type, str) or not event_type.strip():
            raise AutonomousGoalError("goal event type must be non-empty")
        count = int(self._connection.execute("SELECT COUNT(*) FROM autonomous_goal_events").fetchone()[0])
        if count >= self.max_events:
            raise AutonomousGoalError("goal event capacity is exhausted")
        prior = self._connection.execute(
            "SELECT event_digest FROM autonomous_goal_events ORDER BY sequence DESC LIMIT 1"
        ).fetchone()
        previous = "" if prior is None else str(prior["event_digest"])
        sequence = count + 1
        payload = record.to_dict()
        body = {
            "schema": GOAL_EVENT_SCHEMA,
            "sequence": sequence,
            "goal_id": record.goal_id,
            "event_type": event_type,
            "payload": payload,
            "previous_digest": previous,
            "created_ns": now_ns,
            "retention": GOAL_RETENTION,
            "secret_material": "never_returned",
        }
        event_digest = _digest(body)
        self._connection.execute(
            "INSERT INTO autonomous_goal_events "
            "(goal_id, event_type, payload_json, previous_digest, event_digest, created_ns) "
            "VALUES (?, ?, ?, ?, ?, ?)",
            (
                record.goal_id,
                event_type,
                json.dumps(payload, ensure_ascii=False, sort_keys=True, separators=(",", ":")),
                previous,
                event_digest,
                now_ns,
            ),
        )

    def _now(self, now_ns: int | None) -> int:
        value = self._clock() if now_ns is None else now_ns
        if not isinstance(value, int) or isinstance(value, bool) or value < 0:
            raise AutonomousGoalError("goal clock must return a non-negative integer")
        return value


def _normalize_goal_event(
    value: Any,
    *,
    expected_sequence: int,
    previous_digest: str,
) -> dict[str, Any]:
    event_keys = {
        "schema",
        "sequence",
        "goal_id",
        "event_type",
        "payload",
        "previous_digest",
        "created_ns",
        "event_digest",
        "retention",
        "secret_material",
    }
    if not isinstance(value, Mapping) or set(value) != event_keys:
        raise AutonomousGoalError("goal snapshot event is malformed")
    if (
        value.get("schema") != GOAL_EVENT_SCHEMA
        or value.get("retention") != GOAL_RETENTION
        or value.get("secret_material") != "never_returned"
    ):
        raise AutonomousGoalError("goal snapshot event retention or schema is invalid")
    if value.get("sequence") != expected_sequence or value.get("previous_digest") != previous_digest:
        raise AutonomousGoalError("goal snapshot event sequence or chain is invalid")
    goal_id = _identifier(value.get("goal_id"), name="goal event goal_id")
    event_type = value.get("event_type")
    if event_type not in {"created", "transition"}:
        raise AutonomousGoalError("goal snapshot event type is unsupported")
    created_ns = value.get("created_ns")
    if not isinstance(created_ns, int) or isinstance(created_ns, bool) or created_ns < 0:
        raise AutonomousGoalError("goal snapshot event timestamp is invalid")
    event_digest = value.get("event_digest")
    if not _valid_digest(event_digest):
        raise AutonomousGoalError("goal snapshot event digest is invalid")
    payload = value.get("payload")
    if not isinstance(payload, Mapping):
        raise AutonomousGoalError("goal snapshot event payload is invalid")
    record = AutonomousGoalRecord.from_mapping(payload)
    normalized_payload = record.to_dict()
    if record.goal_id != goal_id or set(payload) != set(normalized_payload) or dict(payload) != normalized_payload:
        raise AutonomousGoalError("goal snapshot event payload is not a normalized current goal")
    body = {
        "schema": GOAL_EVENT_SCHEMA,
        "sequence": expected_sequence,
        "goal_id": goal_id,
        "event_type": event_type,
        "payload": normalized_payload,
        "previous_digest": previous_digest,
        "created_ns": created_ns,
        "retention": GOAL_RETENTION,
        "secret_material": "never_returned",
    }
    if _digest(body) != event_digest:
        raise AutonomousGoalError("goal snapshot event digest does not match its metadata")
    return {**body, "event_digest": event_digest}


def _build_goal_snapshot(
    goals: Sequence[Mapping[str, Any]],
    events: Sequence[Mapping[str, Any]],
    *,
    max_goals: int,
    max_events: int,
    max_bytes: int,
    expected_goal_count: int | None = None,
) -> dict[str, Any]:
    if len(goals) > max_goals or len(events) > max_events:
        raise AutonomousGoalError("goal snapshot exceeds its capacity")
    normalized_goals = [AutonomousGoalRecord.from_mapping(goal).to_dict() for goal in goals]
    if len({goal["goal_id"] for goal in normalized_goals}) != len(normalized_goals):
        raise AutonomousGoalError("goal snapshot contains duplicate goals")
    normalized_goals.sort(key=lambda goal: goal["goal_id"])
    normalized_events: list[dict[str, Any]] = []
    previous = ""
    latest: dict[str, str] = {}
    for expected_sequence, raw_event in enumerate(events, start=1):
        event = _normalize_goal_event(
            raw_event,
            expected_sequence=expected_sequence,
            previous_digest=previous,
        )
        if event["event_type"] == "created":
            if event["goal_id"] in latest:
                raise AutonomousGoalError("goal snapshot contains a duplicate created event")
        elif event["goal_id"] not in latest:
            raise AutonomousGoalError("goal snapshot transition references an unknown goal")
        latest[event["goal_id"]] = event["payload"]["state_digest"]
        normalized_events.append(event)
        previous = event["event_digest"]
    if expected_goal_count is not None and expected_goal_count != len(normalized_goals):
        raise AutonomousGoalError("goal snapshot goal count disagrees with the ledger")
    if set(latest) != {goal["goal_id"] for goal in normalized_goals}:
        raise AutonomousGoalError("goal snapshot current goals are not represented by events")
    by_goal = {goal["goal_id"]: goal for goal in normalized_goals}
    for goal_id, state_digest in latest.items():
        if by_goal[goal_id]["state_digest"] != state_digest:
            raise AutonomousGoalError(f"goal snapshot current state is not bound to its latest event for {goal_id}")
    body = {
        "schema": GOAL_SNAPSHOT_SCHEMA,
        "sequence": len(normalized_events),
        "head_digest": previous,
        "goals": normalized_goals,
        "events": normalized_events,
        "retention": GOAL_RETENTION,
        "secret_material": "never_returned",
    }
    snapshot = {**body, "snapshot_digest": _digest(body)}
    if len(_canonical_goal_json(snapshot).encode("utf-8")) > min(max_bytes, MAX_GOAL_SNAPSHOT_BYTES):
        raise AutonomousGoalError("goal snapshot exceeds its byte bound")
    return snapshot


def _canonical_goal_json(value: Any) -> str:
    try:
        return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False)
    except (TypeError, ValueError) as error:
        raise AutonomousGoalError("goal value is not canonical JSON") from error


def _normalize_goal_snapshot(
    value: Mapping[str, Any],
    *,
    max_goals: int,
    max_events: int,
    max_bytes: int,
) -> dict[str, Any]:
    expected_keys = {
        "schema",
        "sequence",
        "head_digest",
        "goals",
        "events",
        "snapshot_digest",
        "retention",
        "secret_material",
    }
    if not isinstance(value, Mapping) or set(value) != expected_keys:
        raise AutonomousGoalError("goal snapshot is malformed")
    if value.get("schema") != GOAL_SNAPSHOT_SCHEMA:
        raise AutonomousGoalError("goal snapshot schema is unsupported")
    if value.get("retention") != GOAL_RETENTION or value.get("secret_material") != "never_returned":
        raise AutonomousGoalError("goal snapshot contains unsupported or unsafe metadata")
    raw_goals = value.get("goals")
    raw_events = value.get("events")
    if not isinstance(raw_goals, Sequence) or isinstance(raw_goals, (str, bytes, bytearray)) or len(raw_goals) > max_goals:
        raise AutonomousGoalError("goal snapshot goal capacity is invalid")
    if not isinstance(raw_events, Sequence) or isinstance(raw_events, (str, bytes, bytearray)) or len(raw_events) > max_events:
        raise AutonomousGoalError("goal snapshot event capacity is invalid")
    sequence = value.get("sequence")
    if not isinstance(sequence, int) or isinstance(sequence, bool) or sequence < 0 or sequence != len(raw_events):
        raise AutonomousGoalError("goal snapshot sequence is invalid")
    goals = [AutonomousGoalRecord.from_mapping(goal).to_dict() for goal in raw_goals]
    if len({goal["goal_id"] for goal in goals}) != len(goals):
        raise AutonomousGoalError("goal snapshot contains duplicate goals")
    goals.sort(key=lambda goal: goal["goal_id"])
    events: list[dict[str, Any]] = []
    previous = ""
    latest: dict[str, str] = {}
    for expected_sequence, raw_event in enumerate(raw_events, start=1):
        event = _normalize_goal_event(
            raw_event,
            expected_sequence=expected_sequence,
            previous_digest=previous,
        )
        if event["event_type"] == "created":
            if event["goal_id"] in latest:
                raise AutonomousGoalError("goal snapshot contains a duplicate created event")
        elif event["goal_id"] not in latest:
            raise AutonomousGoalError("goal snapshot transition references an unknown goal")
        latest[event["goal_id"]] = event["payload"]["state_digest"]
        events.append(event)
        previous = event["event_digest"]
    if set(latest) != {goal["goal_id"] for goal in goals}:
        raise AutonomousGoalError("goal snapshot current goals are not represented by events")
    by_goal = {goal["goal_id"]: goal for goal in goals}
    for goal_id, state_digest in latest.items():
        if by_goal[goal_id]["state_digest"] != state_digest:
            raise AutonomousGoalError(f"goal snapshot current state is not bound to its latest event for {goal_id}")
    head_digest = value.get("head_digest")
    if not isinstance(head_digest, str) or (sequence > 0 and not _valid_digest(head_digest)) or (sequence == 0 and head_digest != "") or head_digest != previous:
        raise AutonomousGoalError("goal snapshot head digest is invalid")
    body = {
        "schema": GOAL_SNAPSHOT_SCHEMA,
        "sequence": sequence,
        "head_digest": head_digest,
        "goals": goals,
        "events": events,
        "retention": GOAL_RETENTION,
        "secret_material": "never_returned",
    }
    snapshot_digest = value.get("snapshot_digest")
    if not isinstance(snapshot_digest, str) or not _valid_digest(snapshot_digest) or _digest(body) != snapshot_digest:
        raise AutonomousGoalError("goal snapshot digest mismatch")
    normalized = {**body, "snapshot_digest": snapshot_digest}
    if len(_canonical_goal_json(normalized).encode("utf-8")) > min(max_bytes, MAX_GOAL_SNAPSHOT_BYTES):
        raise AutonomousGoalError("goal snapshot exceeds its byte bound")
    return normalized


def validate_goal_snapshot(value: Mapping[str, Any]) -> dict[str, Any]:
    """Public strict validator for value-only autonomous goal snapshots."""

    return _normalize_goal_snapshot(
        value,
        max_goals=MAX_GOALS,
        max_events=MAX_GOAL_EVENTS,
        max_bytes=MAX_GOAL_SNAPSHOT_BYTES,
    )


class AutonomousGoalSnapshotTextStore(Protocol):
    """Portable text persistence for objective lifecycle snapshots."""

    def read(self) -> str | None: ...

    def write(self, value: str) -> None: ...


class TransactionalAutonomousGoalSnapshotTextStore(AutonomousGoalSnapshotTextStore, Protocol):
    def write_if_unchanged(self, expected_snapshot_digest: str | None, value: str) -> bool: ...


class JsonAutonomousGoalSnapshotPersistence:
    """Canonical JSON goal persistence over a caller-owned text store."""

    def __init__(self, store: AutonomousGoalSnapshotTextStore, *, max_bytes: int = MAX_GOAL_SNAPSHOT_BYTES) -> None:
        if not all(callable(getattr(store, name, None)) for name in ("read", "write")):
            raise AutonomousGoalError("goal JSON persistence requires a text store")
        if isinstance(max_bytes, bool) or not isinstance(max_bytes, int) or not 1 <= max_bytes <= MAX_GOAL_SNAPSHOT_BYTES:
            raise AutonomousGoalError("goal JSON persistence max_bytes is outside its bound")
        self.store = store
        self.max_bytes = max_bytes

    def read(self) -> dict[str, Any] | None:
        encoded = self.store.read()
        if encoded is None:
            return None
        if not isinstance(encoded, str) or len(encoded.encode("utf-8")) > self.max_bytes:
            raise AutonomousGoalError("goal JSON exceeds its byte bound")
        try:
            raw = json.loads(encoded)
        except (UnicodeDecodeError, json.JSONDecodeError) as error:
            raise AutonomousGoalError("goal JSON is invalid") from error
        if not isinstance(raw, Mapping):
            raise AutonomousGoalError("goal JSON snapshot must be an object")
        return _normalize_goal_snapshot(
            raw,
            max_goals=MAX_GOALS,
            max_events=MAX_GOAL_EVENTS,
            max_bytes=self.max_bytes,
        )

    def write(self, snapshot: Mapping[str, Any]) -> None:
        normalized = _normalize_goal_snapshot(
            snapshot,
            max_goals=MAX_GOALS,
            max_events=MAX_GOAL_EVENTS,
            max_bytes=self.max_bytes,
        )
        encoded = _canonical_goal_json(normalized)
        if len(encoded.encode("utf-8")) > self.max_bytes:
            raise AutonomousGoalError("goal JSON exceeds its byte bound")
        self.store.write(encoded)


class TransactionalJsonAutonomousGoalSnapshotPersistence(JsonAutonomousGoalSnapshotPersistence):
    """Canonical JSON goal persistence with stale-writer fencing."""

    def __init__(self, store: TransactionalAutonomousGoalSnapshotTextStore, *, max_bytes: int = MAX_GOAL_SNAPSHOT_BYTES) -> None:
        super().__init__(store, max_bytes=max_bytes)
        if not callable(getattr(store, "write_if_unchanged", None)):
            raise AutonomousGoalError("transactional goal persistence requires write_if_unchanged")
        self.store = store

    def write_if_unchanged(self, expected_snapshot_digest: str | None, snapshot: Mapping[str, Any]) -> bool:
        if expected_snapshot_digest is not None and not _valid_digest(expected_snapshot_digest):
            raise AutonomousGoalError("goal expected snapshot digest is invalid")
        normalized = _normalize_goal_snapshot(
            snapshot,
            max_goals=MAX_GOALS,
            max_events=MAX_GOAL_EVENTS,
            max_bytes=self.max_bytes,
        )
        return self.store.write_if_unchanged(expected_snapshot_digest, _canonical_goal_json(normalized))


class AutonomousGoalPersistenceCoordinator:
    """Flush and restore the objective ledger through caller-owned storage."""

    def __init__(self, store: AutonomousGoalLedger, persistence: Any) -> None:
        if not isinstance(store, AutonomousGoalLedger):
            raise AutonomousGoalError("goal persistence requires an AutonomousGoalLedger")
        if not all(callable(getattr(persistence, name, None)) for name in ("read", "write")):
            raise AutonomousGoalError("goal persistence adapter is malformed")
        self.store = store
        self.persistence = persistence
        self._expected_snapshot_digest: str | None = None

    def restore(self) -> dict[str, Any] | None:
        raw = self.persistence.read()
        if raw is None:
            self._expected_snapshot_digest = None
            return None
        snapshot = _normalize_goal_snapshot(
            raw,
            max_goals=self.store.max_goals,
            max_events=self.store.max_events,
            max_bytes=MAX_GOAL_SNAPSHOT_BYTES,
        )
        self.store.restore(snapshot)
        self._expected_snapshot_digest = snapshot["snapshot_digest"]
        return snapshot

    def flush(self) -> dict[str, Any]:
        snapshot = self.store.snapshot()
        write_if_unchanged = getattr(self.persistence, "write_if_unchanged", None)
        if callable(write_if_unchanged):
            if not write_if_unchanged(self._expected_snapshot_digest, snapshot):
                raise AutonomousGoalError("goal persistence compare-and-swap conflict")
        else:
            self.persistence.write(snapshot)
        self._expected_snapshot_digest = snapshot["snapshot_digest"]
        return snapshot


__all__ = [
    "GOAL_EVENT_SCHEMA",
    "GOAL_SNAPSHOT_SCHEMA",
    "GOAL_RETENTION",
    "GOAL_SCHEMA",
    "GOAL_STEP_SCHEMA",
    "MAX_GOAL_BLOCKERS",
    "MAX_GOAL_CRITERIA",
    "MAX_GOAL_EVENTS",
    "MAX_GOAL_SNAPSHOT_BYTES",
    "MAX_GOALS",
    "AutonomousGoalConflict",
    "AutonomousGoalCriterion",
    "AutonomousGoalError",
    "AutonomousGoalLedger",
    "AutonomousGoalRecord",
    "AutonomousGoalPersistenceCoordinator",
    "AutonomousGoalSnapshotTextStore",
    "JsonAutonomousGoalSnapshotPersistence",
    "TransactionalAutonomousGoalSnapshotTextStore",
    "TransactionalJsonAutonomousGoalSnapshotPersistence",
    "goal_status_for_result",
    "goal_task_digest",
    "validate_goal_snapshot",
]
