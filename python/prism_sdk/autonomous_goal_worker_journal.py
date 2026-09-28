"""Restart and reconciliation journal for metadata-only goal workers.

The goal ledger knows that an objective is ``running`` but cannot tell whether a worker died
before or after crossing the executor/provider boundary.  This journal records that distinction
without retaining task text, prompts, parameters, credentials, or executor output.  Recovery is
deliberately conservative: a pre-dispatch interruption is paused for a safe retry; a post-dispatch
interruption is blocked until the caller reconciles the uncertain external outcome.
"""

from __future__ import annotations

from dataclasses import dataclass
import hashlib
import hmac
import json
import threading
import time
from collections.abc import Mapping, Sequence
from types import MappingProxyType
from typing import Any, Callable, Literal, Protocol

from .authoring import MAX_SAFE_JSON_INTEGER, canonical_json, content_digest, utf8_scalar_byte_length
from .goals import AutonomousGoalError, AutonomousGoalLedger, goal_task_digest
from .goal_time import (
    AutonomousGoalLegacyTimestampUnit,
    AutonomousGoalTimeError,
    migrate_legacy_autonomous_goal_timestamp_ns,
    normalize_autonomous_goal_timestamp_ns,
    require_autonomous_goal_timestamp_ns_wire,
)


GOAL_WORKER_JOURNAL_SCHEMA_V01 = "bioprism-autonomous-goal-worker-journal/0.1"
GOAL_WORKER_JOURNAL_EVENT_SCHEMA_V01 = "bioprism-autonomous-goal-worker-event/0.1"
GOAL_WORKER_JOURNAL_SNAPSHOT_SCHEMA_V01 = "bioprism-autonomous-goal-worker-snapshot/0.1"
GOAL_WORKER_JOURNAL_SCHEMA = "bioprism-autonomous-goal-worker-journal/0.2"
GOAL_WORKER_JOURNAL_EVENT_SCHEMA = "bioprism-autonomous-goal-worker-event/0.2"
GOAL_WORKER_JOURNAL_SNAPSHOT_SCHEMA = "bioprism-autonomous-goal-worker-snapshot/0.2"
GOAL_WORKER_JOURNAL_RETENTION = "metadata_only_worker_boundary;tasks_prompts_parameters_credentials_and_results_not_retained"
MAX_GOAL_WORKER_JOURNAL_EVENTS = 16_384
MAX_GOAL_WORKER_JOURNAL_SNAPSHOT_BYTES = 2_000_000
MAX_AUTHENTICATED_GOAL_WORKER_JOURNAL_BYTES = MAX_GOAL_WORKER_JOURNAL_SNAPSHOT_BYTES + 2_048
AUTHENTICATED_GOAL_WORKER_JOURNAL_SCHEMA_V01 = "bioprism-autonomous-goal-worker-journal-auth/0.1"
AUTHENTICATED_GOAL_WORKER_JOURNAL_SCHEMA = "bioprism-autonomous-goal-worker-journal-auth/0.2"
GOAL_DISPATCH_RESOLUTION_SCHEMA_V01 = "bioprism-autonomous-goal-dispatch-resolution/0.1"
GOAL_DISPATCH_RESOLUTION_SCHEMA = "bioprism-autonomous-goal-dispatch-resolution/0.2"
GOAL_DISPATCH_RESOLUTION_RETENTION = "metadata_only_dispatch_status;external_evidence_payloads_not_retained"

WorkerJournalPhase = Literal[
    "prepared",
    "claimed",
    "dispatch_started",
    "settled",
    "failed",
    "reconciled",
]
_ACTIVE_PHASES = frozenset({"prepared", "claimed", "dispatch_started"})
_ALL_PHASES = frozenset({"prepared", "claimed", "dispatch_started", "settled", "failed", "reconciled"})
GoalDispatchResolutionStatus = Literal["completed", "not_applied", "failed", "pending", "unknown"]


class AutonomousGoalDispatchResolutionVerifier(Protocol):
    """Deployment-owned verifier for an externally obtained dispatch status receipt."""

    verifier_id: str

    def verify(self, resolution: Mapping[str, Any], /) -> bool:
        """Authenticate the exact metadata receipt using deployment-owned trust material."""


def _fail(message: str) -> None:
    raise AutonomousGoalError(f"autonomous goal worker journal {message}")


def _identifier(value: Any, *, name: str) -> str:
    if not isinstance(value, str) or not value.strip() or "\x00" in value:
        _fail(f"{name} is outside its bounded identifier contract")
    byte_length = utf8_scalar_byte_length(value)
    if byte_length is None or byte_length > 256:
        _fail(f"{name} is outside its bounded identifier contract")
    return value.strip()


def _digest(value: Any, *, name: str, allow_none: bool = False) -> str | None:
    if value is None and allow_none:
        return None
    if not isinstance(value, str) or len(value) != 64 or any(char not in "0123456789abcdef" for char in value):
        _fail(f"{name} must be a lowercase SHA-256 digest")
    return value


def _integer(value: Any, *, name: str, minimum: int = 0, maximum: int = MAX_SAFE_JSON_INTEGER) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or value < minimum or value > maximum:
        _fail(f"{name} is outside its integer bounds")
    return value


def _timestamp(value: Any, *, name: str) -> str:
    try:
        return normalize_autonomous_goal_timestamp_ns(value, name=name)
    except AutonomousGoalTimeError as error:
        raise AutonomousGoalError(str(error)) from error


def _timestamp_wire(value: Any, *, name: str) -> str:
    try:
        return require_autonomous_goal_timestamp_ns_wire(value, name=name)
    except AutonomousGoalTimeError as error:
        raise AutonomousGoalError(str(error)) from error


def _migrate_timestamp(value: Any, source_unit: AutonomousGoalLegacyTimestampUnit, *, name: str) -> str:
    try:
        return migrate_legacy_autonomous_goal_timestamp_ns(value, source_unit, name=name)
    except AutonomousGoalTimeError as error:
        raise AutonomousGoalError(str(error)) from error


def _normalize_journal_migration(value: Any) -> dict[str, Any] | None:
    if value is None:
        return None
    keys = {"source_schema", "source_timestamp_unit", "source_snapshot_digest", "source_head_digest"}
    if not isinstance(value, Mapping) or set(value) != keys:
        _fail("snapshot migration provenance is malformed")
    if value.get("source_schema") != GOAL_WORKER_JOURNAL_SNAPSHOT_SCHEMA_V01:
        _fail("snapshot migration source schema is unsupported")
    if value.get("source_timestamp_unit") not in {"milliseconds", "nanoseconds"}:
        _fail("snapshot migration timestamp unit is invalid")
    if not isinstance(value.get("source_snapshot_digest"), str) or len(value["source_snapshot_digest"]) != 64:
        _fail("snapshot migration source digest is invalid")
    _digest(value["source_snapshot_digest"], name="snapshot migration source digest")
    source_head = value.get("source_head_digest")
    if source_head != "":
        _digest(source_head, name="snapshot migration source head")
    return {
        "source_schema": GOAL_WORKER_JOURNAL_SNAPSHOT_SCHEMA_V01,
        "source_timestamp_unit": value["source_timestamp_unit"],
        "source_snapshot_digest": value["source_snapshot_digest"],
        "source_head_digest": source_head,
    }


def _restart_outcome_digest(goal_id: str, attempt: int, *, before_dispatch: bool) -> str:
    return content_digest({
        "goal_id": goal_id,
        "attempt": attempt,
        "result_status": "worker_restart_before_dispatch" if before_dispatch else "worker_restart_after_dispatch",
    })


def _needs_external_reconciliation(event: "AutonomousGoalWorkerEvent") -> bool:
    if event.phase != "reconciled":
        return False
    # A post-dispatch restart receipt records that the goal is blocked, but does not
    # settle the external call. Keep it active across later process restarts until a
    # caller supplies a status receipt. A pre-dispatch restart is safe to retry.
    return event.outcome_digest != _restart_outcome_digest(event.goal_id, event.attempt, before_dispatch=True)


def _future_event_reserve(event: "AutonomousGoalWorkerEvent") -> int:
    if event.phase == "prepared":
        return 1
    if event.phase == "claimed":
        return 1
    if event.phase == "dispatch_started":
        return 3
    if event.phase == "reconciled" and _needs_external_reconciliation(event):
        # A pending/unknown receipt needs a later receipt and settlement. A staged
        # terminal receipt needs only the final settled marker.
        return 2 if event.resolution_goal_status in {None, "blocked"} else 1
    return 0


def validate_goal_dispatch_resolution(value: Mapping[str, Any]) -> dict[str, Any]:
    """Validate value-only status evidence without retaining the external response body."""

    if not isinstance(value, Mapping):
        _fail("dispatch resolution must be a mapping")
    expected = {
        "schema", "goal_id", "attempt", "dispatch_event_digest", "execution_binding_digest",
        "status", "evidence_digest", "verifier_id", "observed_ns", "retention", "secret_material",
    }
    if set(value) != expected:
        _fail("dispatch resolution contains unsupported or missing fields")
    if value["schema"] == GOAL_DISPATCH_RESOLUTION_SCHEMA_V01:
        _fail("legacy dispatch resolution requires external re-verification and re-issuance under schema 0.2")
    if value["schema"] != GOAL_DISPATCH_RESOLUTION_SCHEMA or value["retention"] != GOAL_DISPATCH_RESOLUTION_RETENTION or value["secret_material"] != "never_returned":
        _fail("dispatch resolution markers are invalid")
    if not isinstance(value["status"], str) or value["status"] not in {"completed", "not_applied", "failed", "pending", "unknown"}:
        _fail("dispatch resolution status is invalid")
    return {
        "schema": GOAL_DISPATCH_RESOLUTION_SCHEMA,
        "goal_id": _identifier(value["goal_id"], name="dispatch resolution goal_id"),
        "attempt": _integer(value["attempt"], name="dispatch resolution attempt", minimum=1, maximum=128),
        "dispatch_event_digest": _digest(value["dispatch_event_digest"], name="dispatch resolution dispatch_event_digest"),
        "execution_binding_digest": _digest(value["execution_binding_digest"], name="dispatch resolution execution_binding_digest"),
        "status": value["status"],
        "evidence_digest": _digest(value["evidence_digest"], name="dispatch resolution evidence_digest"),
        "verifier_id": _identifier(value["verifier_id"], name="dispatch resolution verifier_id"),
        "observed_ns": _timestamp_wire(value["observed_ns"], name="dispatch resolution observed_ns"),
        "retention": GOAL_DISPATCH_RESOLUTION_RETENTION,
        "secret_material": "never_returned",
    }


def _authenticated_goal_dispatch_resolution(
    raw: Mapping[str, Any], verifier: AutonomousGoalDispatchResolutionVerifier | None,
) -> dict[str, Any]:
    resolution = validate_goal_dispatch_resolution(raw)
    if verifier is None or not callable(getattr(verifier, "verify", None)):
        _fail("external status reconciliation requires a deployment-owned verifier")
    verifier_id = _identifier(getattr(verifier, "verifier_id", None), name="configured verifier_id")
    if resolution["verifier_id"] != verifier_id:
        _fail("dispatch resolution verifier_id does not match the configured status authority")
    try:
        verified = verifier.verify(MappingProxyType(resolution))
    except Exception as error:
        raise AutonomousGoalError("autonomous goal worker journal deployment verifier failed") from error
    if verified is not True:
        _fail("deployment verifier rejected the dispatch resolution")
    return resolution


@dataclass(frozen=True, slots=True)
class AutonomousGoalWorkerEvent:
    sequence: int
    batch_id: str
    goal_id: str
    phase: WorkerJournalPhase
    attempt: int
    revision: int
    schedule_digest: str
    claim_digest: str | None
    outcome_digest: str | None
    error_digest: str | None
    created_ns: str
    previous_digest: str
    event_digest: str
    task_digest: str | None = None
    execution_binding_digest: str | None = None
    resolution_goal_status: Literal["completed", "failed", "ready", "blocked", "paused"] | None = None

    def _body(self) -> dict[str, Any]:
        body = {
            "schema": GOAL_WORKER_JOURNAL_EVENT_SCHEMA,
            "sequence": self.sequence,
            "batch_id": self.batch_id,
            "goal_id": self.goal_id,
            "phase": self.phase,
            "attempt": self.attempt,
            "revision": self.revision,
            "schedule_digest": self.schedule_digest,
            "claim_digest": self.claim_digest,
            "outcome_digest": self.outcome_digest,
            "error_digest": self.error_digest,
            "created_ns": self.created_ns,
            "previous_digest": self.previous_digest,
            "retention": GOAL_WORKER_JOURNAL_RETENTION,
            "secret_material": "never_returned",
        }
        if self.task_digest is not None:
            body["task_digest"] = self.task_digest
        if self.execution_binding_digest is not None:
            body["execution_binding_digest"] = self.execution_binding_digest
        if self.resolution_goal_status is not None:
            body["resolution_goal_status"] = self.resolution_goal_status
        return body

    def to_dict(self) -> dict[str, Any]:
        return {**self._body(), "event_digest": self.event_digest}

    @classmethod
    def from_mapping(cls, value: Mapping[str, Any]) -> "AutonomousGoalWorkerEvent":
        if not isinstance(value, Mapping):
            _fail("event must be a mapping")
        allowed = set(AutonomousGoalWorkerEvent._field_names())
        if set(value).difference(allowed):
            _fail("event contains unsupported fields")
        optional = {"task_digest", "execution_binding_digest", "resolution_goal_status"}
        if set(allowed).difference(optional).difference(value):
            _fail("event is missing required fields")
        if value.get("schema") != GOAL_WORKER_JOURNAL_EVENT_SCHEMA or value.get("retention") != GOAL_WORKER_JOURNAL_RETENTION or value.get("secret_material") != "never_returned":
            _fail("event retention markers are invalid")
        phase = value.get("phase")
        if phase not in _ALL_PHASES:
            _fail("event phase is invalid")
        previous_digest = value.get("previous_digest")
        if not isinstance(previous_digest, str):
            _fail("event.previous_digest must be a string")
        task_digest = value.get("task_digest")
        if "task_digest" in value and task_digest is None:
            _fail("event.task_digest must be omitted or a digest")
        execution_binding_digest = value.get("execution_binding_digest")
        if "execution_binding_digest" in value and execution_binding_digest is None:
            _fail("event.execution_binding_digest must be omitted or a digest")
        resolution_goal_status = value.get("resolution_goal_status")
        if "resolution_goal_status" in value and (not isinstance(resolution_goal_status, str) or resolution_goal_status not in {"completed", "failed", "ready", "blocked", "paused"}):
            _fail("event resolution goal status is invalid")
        event = cls(
            sequence=_integer(value.get("sequence"), name="event.sequence", minimum=1, maximum=MAX_GOAL_WORKER_JOURNAL_EVENTS),
            batch_id=_identifier(value.get("batch_id"), name="event.batch_id"),
            goal_id=_identifier(value.get("goal_id"), name="event.goal_id"),
            phase=phase,
            attempt=_integer(value.get("attempt"), name="event.attempt", minimum=0, maximum=128),
            revision=_integer(value.get("revision"), name="event.revision"),
            schedule_digest=_digest(value.get("schedule_digest"), name="event.schedule_digest") or "",
            claim_digest=_digest(value.get("claim_digest"), name="event.claim_digest", allow_none=True),
            outcome_digest=_digest(value.get("outcome_digest"), name="event.outcome_digest", allow_none=True),
            error_digest=_digest(value.get("error_digest"), name="event.error_digest", allow_none=True),
            created_ns=_timestamp_wire(value.get("created_ns"), name="event.created_ns"),
            previous_digest=previous_digest,
            event_digest=_digest(value.get("event_digest"), name="event.event_digest") or "",
            task_digest=_digest(task_digest, name="event.task_digest") if "task_digest" in value else None,
            execution_binding_digest=_digest(execution_binding_digest, name="event.execution_binding_digest") if "execution_binding_digest" in value else None,
            resolution_goal_status=resolution_goal_status if "resolution_goal_status" in value else None,
        )
        if value.get("resolution_goal_status") is not None and event.resolution_goal_status is None:
            _fail("event resolution goal status is invalid")
        if event.resolution_goal_status is not None and (event.phase not in {"reconciled", "settled"} or event.outcome_digest is None):
            _fail("event resolution goal status is inconsistent with its phase")
        if event.sequence == 1 and event.previous_digest != "":
            _fail("first event must have an empty previous digest")
        if event.sequence > 1:
            _digest(event.previous_digest, name="event.previous_digest")
        if content_digest(event._body()) != event.event_digest:
            _fail(f"event {event.sequence} digest does not match its content")
        return event

    @staticmethod
    def _field_names() -> tuple[str, ...]:
        return (
            "schema", "sequence", "batch_id", "goal_id", "phase", "attempt", "revision",
            "schedule_digest", "claim_digest", "outcome_digest", "error_digest", "created_ns",
            "task_digest", "execution_binding_digest", "resolution_goal_status", "previous_digest", "event_digest", "retention", "secret_material",
        )


@dataclass(frozen=True, slots=True)
class AutonomousGoalWorkerJournalSnapshot:
    sequence: int
    head_digest: str
    events: tuple[AutonomousGoalWorkerEvent, ...]
    snapshot_digest: str
    migration: Mapping[str, Any] | None = None

    def to_dict(self) -> dict[str, Any]:
        body = {
            "schema": GOAL_WORKER_JOURNAL_SNAPSHOT_SCHEMA,
            "sequence": self.sequence,
            "head_digest": self.head_digest,
            "events": [event.to_dict() for event in self.events],
            "retention": GOAL_WORKER_JOURNAL_RETENTION,
            "secret_material": "never_returned",
        }
        if self.migration is not None:
            body["migration"] = dict(self.migration)
        return {**body, "snapshot_digest": self.snapshot_digest}


class AutonomousGoalWorkerJournal:
    """Hash-chained metadata events that fence pre- and post-dispatch recovery."""

    def __init__(self, *, max_events: int = MAX_GOAL_WORKER_JOURNAL_EVENTS, clock: Callable[[], int | str] | None = None) -> None:
        if isinstance(max_events, bool) or not isinstance(max_events, int) or not 1 <= max_events <= MAX_GOAL_WORKER_JOURNAL_EVENTS:
            _fail(f"max_events must be between 1 and {MAX_GOAL_WORKER_JOURNAL_EVENTS}")
        self.max_events = max_events
        self._clock = clock or time.time_ns
        self._events: list[AutonomousGoalWorkerEvent] = []
        self._latest_by_goal: dict[str, AutonomousGoalWorkerEvent] = {}
        self._future_event_reserve_count = 0
        self._worker_run_lock = threading.Lock()
        self._migration: dict[str, Any] | None = None

    @property
    def head_digest(self) -> str:
        return self._events[-1].event_digest if self._events else ""

    def require_capacity(self, additional_events: int) -> None:
        """Check new work while preserving capacity for every active journal boundary."""

        if isinstance(additional_events, bool) or not isinstance(additional_events, int) or additional_events < 0:
            _fail("additional event capacity must be a non-negative integer")
        if len(self._events) + self._future_event_reserve_count + additional_events > self.max_events:
            _fail("journal event capacity cannot preserve active recovery and requested transitions")

    def begin_worker_run(self) -> Callable[[], None]:
        """Admit one local worker against this journal's shared event budget."""

        if not self._worker_run_lock.acquire(blocking=False):
            _fail("another worker run is already active for this journal")
        released = False

        def release() -> None:
            nonlocal released
            if not released:
                released = True
                self._worker_run_lock.release()

        return release

    def record(
        self,
        *,
        batch_id: str,
        goal_id: str,
        phase: WorkerJournalPhase,
        attempt: int,
        revision: int,
        schedule_digest: str,
        claim_digest: str | None = None,
        outcome_digest: str | None = None,
        error_digest: str | None = None,
        task_digest: str | None = None,
        execution_binding_digest: str | None = None,
        resolution_goal_status: Literal["completed", "failed", "ready", "blocked", "paused"] | None = None,
        created_ns: int | str | None = None,
    ) -> AutonomousGoalWorkerEvent:
        if len(self._events) >= self.max_events:
            _fail("event capacity is exhausted")
        if phase not in _ALL_PHASES:
            _fail("event phase is invalid")
        normalized_task_digest = _digest(task_digest, name="task_digest") if task_digest is not None else None
        normalized_execution_binding_digest = _digest(execution_binding_digest, name="execution_binding_digest") if execution_binding_digest is not None else None
        if resolution_goal_status is not None:
            if resolution_goal_status not in {"completed", "failed", "ready", "blocked", "paused"} or phase not in {"reconciled", "settled"} or outcome_digest is None:
                _fail("resolution goal status is inconsistent with its phase")
        event_body = {
            "schema": GOAL_WORKER_JOURNAL_EVENT_SCHEMA,
            "sequence": len(self._events) + 1,
            "batch_id": _identifier(batch_id, name="batch_id"),
            "goal_id": _identifier(goal_id, name="goal_id"),
            "phase": phase,
            "attempt": _integer(attempt, name="attempt", minimum=0, maximum=128),
            "revision": _integer(revision, name="revision"),
            "schedule_digest": _digest(schedule_digest, name="schedule_digest"),
            "claim_digest": _digest(claim_digest, name="claim_digest", allow_none=True),
            "outcome_digest": _digest(outcome_digest, name="outcome_digest", allow_none=True),
            "error_digest": _digest(error_digest, name="error_digest", allow_none=True),
            "created_ns": _timestamp(self._clock() if created_ns is None else created_ns, name="created_ns"),
            "previous_digest": self.head_digest,
            "retention": GOAL_WORKER_JOURNAL_RETENTION,
            "secret_material": "never_returned",
        }
        if normalized_task_digest is not None:
            event_body["task_digest"] = normalized_task_digest
        if normalized_execution_binding_digest is not None:
            event_body["execution_binding_digest"] = normalized_execution_binding_digest
        if resolution_goal_status is not None:
            event_body["resolution_goal_status"] = resolution_goal_status
        event = AutonomousGoalWorkerEvent(
            sequence=event_body["sequence"],
            batch_id=event_body["batch_id"],
            goal_id=event_body["goal_id"],
            phase=event_body["phase"],
            attempt=event_body["attempt"],
            revision=event_body["revision"],
            schedule_digest=event_body["schedule_digest"],
            claim_digest=event_body["claim_digest"],
            outcome_digest=event_body["outcome_digest"],
            error_digest=event_body["error_digest"],
            created_ns=event_body["created_ns"],
            previous_digest=event_body["previous_digest"],
            event_digest=content_digest(event_body),
            task_digest=normalized_task_digest,
            execution_binding_digest=normalized_execution_binding_digest,
            resolution_goal_status=resolution_goal_status,
        )
        prior = self._latest_by_goal.get(event.goal_id)
        if phase == "prepared" and prior is not None and (
            prior.phase in _ACTIVE_PHASES or _needs_external_reconciliation(prior)
        ):
            _fail(f"goal {event.goal_id} already has an active journal boundary")
        next_reserve = (
            self._future_event_reserve_count
            - (0 if prior is None else _future_event_reserve(prior))
            + _future_event_reserve(event)
        )
        if len(self._events) + 1 + next_reserve > self.max_events:
            _fail("event capacity cannot preserve active recovery transitions")
        self._events.append(event)
        self._latest_by_goal[event.goal_id] = event
        self._future_event_reserve_count = next_reserve
        return event

    def events(self, *, batch_id: str | None = None, goal_id: str | None = None) -> tuple[AutonomousGoalWorkerEvent, ...]:
        if batch_id is not None:
            batch_id = _identifier(batch_id, name="batch_id")
        if goal_id is not None:
            goal_id = _identifier(goal_id, name="goal_id")
        return tuple(event for event in self._events if (batch_id is None or event.batch_id == batch_id) and (goal_id is None or event.goal_id == goal_id))

    def active(self) -> tuple[AutonomousGoalWorkerEvent, ...]:
        return tuple(sorted((event for event in self._latest_by_goal.values() if event.phase in _ACTIVE_PHASES or _needs_external_reconciliation(event)), key=lambda event: event.sequence))

    def active_for(self, goal_id: str) -> AutonomousGoalWorkerEvent | None:
        normalized_goal_id = _identifier(goal_id, name="goal_id")
        return next((event for event in self.active() if event.goal_id == normalized_goal_id), None)

    def replay_settled_external_outcomes(self, ledger: AutonomousGoalLedger) -> int:
        """Replay a journaled settlement if the caller restores an older ledger snapshot."""
        latest_by_goal: dict[str, AutonomousGoalWorkerEvent] = {}
        for event in self._events:
            latest_by_goal[event.goal_id] = event
        replayed = 0
        for event in latest_by_goal.values():
            if event.phase != "settled" or event.resolution_goal_status is None:
                continue
            current = ledger.get(event.goal_id)
            if current is None:
                _fail(f"settled dispatch resolution for goal {event.goal_id} has no ledger record")
            if current.attempt > event.attempt or current.revision > event.revision:
                continue
            if current.attempt == event.attempt and current.revision == event.revision and current.status == event.resolution_goal_status and current.outcome_digest == event.outcome_digest:
                continue
            history = self.events(goal_id=event.goal_id)
            event_index = next((index for index, candidate in enumerate(history) if candidate.event_digest == event.event_digest), -1)
            previous_index = event_index - 1
            if previous_index > 0 and history[previous_index].phase == "reconciled" and history[previous_index].outcome_digest == event.outcome_digest and history[previous_index].resolution_goal_status == event.resolution_goal_status:
                previous_index -= 1
            previous_digest = history[previous_index].outcome_digest if previous_index >= 0 else None
            if current.attempt != event.attempt or current.revision != event.revision - 1 or current.status != "blocked" or current.outcome_digest != previous_digest or "worker_restart_after_dispatch_requires_reconciliation" not in current.blockers:
                _fail(f"settled dispatch resolution for goal {event.goal_id} conflicts with the restored ledger")
            target_status = event.resolution_goal_status
            next_action = goal_task_digest("goal-reconciliation-review") if target_status in {"failed", "paused"} else goal_task_digest("goal-retry") if target_status == "ready" else None
            ledger.transition(
                event.goal_id, target_status, expected_revision=current.revision,
                blockers=("required_goal_criteria_review",) if target_status == "paused" else (), next_action_digest=next_action, outcome_digest=event.outcome_digest,
                now_ns=event.created_ns,
            )
            replayed += 1
        return replayed

    def assert_no_active(self, goal_id: str) -> None:
        event = self.active_for(goal_id)
        if event is not None:
            _fail(f"goal {goal_id} has an unreconciled {event.phase} event; recover or reconcile the worker journal before retrying")

    def prepare_external_outcome(
        self,
        ledger: AutonomousGoalLedger,
        raw: Mapping[str, Any],
        verifier: AutonomousGoalDispatchResolutionVerifier | None,
    ) -> dict[str, Any]:
        """Validate a status receipt and produce an in-memory, revision-fenced commit plan."""

        if not isinstance(ledger, AutonomousGoalLedger):
            _fail("external outcome reconciliation requires an AutonomousGoalLedger")
        resolution = _authenticated_goal_dispatch_resolution(raw, verifier)
        resolution_digest = content_digest(resolution)
        history = self.events(goal_id=resolution["goal_id"])
        dispatch_index = next((index for index, event in enumerate(history) if event.event_digest == resolution["dispatch_event_digest"]), None)
        if dispatch_index is None:
            _fail("dispatch resolution references an unknown journal event")
        dispatch = history[dispatch_index]
        if dispatch.phase != "dispatch_started" or dispatch.attempt != resolution["attempt"]:
            _fail("dispatch resolution does not match a dispatched attempt")
        if dispatch.execution_binding_digest is None or dispatch.execution_binding_digest != resolution["execution_binding_digest"]:
            _fail("dispatch resolution does not match the execution binding")
        recovery_digest = _restart_outcome_digest(dispatch.goal_id, dispatch.attempt, before_dispatch=False)
        after_dispatch = history[dispatch_index + 1 :]
        if not any(event.phase == "reconciled" and event.attempt == dispatch.attempt and event.batch_id == dispatch.batch_id and event.outcome_digest == recovery_digest for event in after_dispatch):
            _fail("dispatch resolution requires completed journal recovery first")
        if any(event.phase in {"prepared", "claimed", "dispatch_started"} and event.attempt != dispatch.attempt for event in after_dispatch):
            _fail("dispatch resolution is stale after a later attempt")

        current = ledger.get(resolution["goal_id"])
        if current is None or current.attempt != resolution["attempt"]:
            _fail("dispatch resolution does not match the current goal attempt")
        target_status = (
            ("completed" if current.required_criteria_complete else "paused") if resolution["status"] == "completed" else
            "failed" if resolution["status"] == "failed" else
            ("ready" if current.attempt < current.max_attempts else "failed") if resolution["status"] == "not_applied" else
            "blocked"
        )
        latest = history[-1] if history else None
        if latest is not None and latest.phase in {"reconciled", "settled"} and latest.attempt == resolution["attempt"] and latest.execution_binding_digest == resolution["execution_binding_digest"] and latest.outcome_digest == resolution_digest and latest.resolution_goal_status == target_status:
            if current.status == target_status and current.revision == latest.revision and current.outcome_digest == resolution_digest:
                return {"resolution": resolution, "dispatch": dispatch, "target_status": target_status, "resolution_digest": resolution_digest, "expected_revision": current.revision, "expected_head_digest": self.head_digest, "idempotent": True, "goal_revision": current.revision}
            prior_digest = history[-2].outcome_digest if len(history) > 1 else None
            if latest.phase == "reconciled" and current.status == "blocked" and current.revision + 1 == latest.revision and current.outcome_digest == prior_digest:
                return {"resolution": resolution, "dispatch": dispatch, "target_status": target_status, "resolution_digest": resolution_digest, "expected_revision": current.revision, "expected_outcome_digest": current.outcome_digest, "expected_head_digest": self.head_digest, "idempotent": False, "already_staged": True}
            _fail("idempotent dispatch resolution conflicts with the current goal state")
        if current.status != "blocked" or "worker_restart_after_dispatch_requires_reconciliation" not in current.blockers:
            _fail("goal is not awaiting post-dispatch outcome reconciliation")
        if latest is None or latest.revision != current.revision or latest.attempt != current.attempt:
            _fail("goal revision has drifted from its recovered dispatch journal")
        if int(resolution["observed_ns"]) <= current.updated_ns:
            _fail("dispatch status evidence is stale relative to the recovered goal state")
        return {"resolution": resolution, "dispatch": dispatch, "target_status": target_status, "resolution_digest": resolution_digest, "expected_revision": current.revision, "expected_outcome_digest": current.outcome_digest, "expected_head_digest": self.head_digest, "idempotent": False, "already_staged": False}

    def stage_external_outcome(self, prepared: Mapping[str, Any]) -> AutonomousGoalWorkerEvent:
        if prepared.get("idempotent") is True:
            _fail("idempotent external outcome does not need staging")
        if prepared.get("already_staged") is True:
            event = self.events(goal_id=prepared["resolution"]["goal_id"])[-1]
            return event
        if self.head_digest != prepared.get("expected_head_digest"):
            _fail("worker journal changed while dispatch resolution was being prepared")
        resolution = prepared["resolution"]
        dispatch = prepared["dispatch"]
        return self.record(
            batch_id=dispatch.batch_id,
            goal_id=dispatch.goal_id,
            phase="reconciled",
            attempt=dispatch.attempt,
            revision=prepared["expected_revision"] + 1,
            schedule_digest=dispatch.schedule_digest,
            claim_digest=dispatch.claim_digest,
            outcome_digest=prepared["resolution_digest"],
            task_digest=dispatch.task_digest,
            execution_binding_digest=dispatch.execution_binding_digest,
            resolution_goal_status=prepared["target_status"],
            created_ns=resolution["observed_ns"],
        )

    def commit_external_outcome(self, ledger: AutonomousGoalLedger, prepared: Mapping[str, Any]) -> dict[str, Any]:
        resolution = prepared["resolution"]
        target_status = prepared["target_status"]
        resolution_digest = prepared["resolution_digest"]
        dispatch = prepared["dispatch"]
        if prepared.get("idempotent") is True:
            return self._dispatch_resolution_result(resolution, target_status, prepared["goal_revision"], idempotent=True)
        latest = self.events(goal_id=resolution["goal_id"])[-1]
        if latest.phase != "reconciled" or latest.resolution_goal_status != target_status or latest.outcome_digest != resolution_digest:
            _fail("dispatch resolution was not durably staged before commit")
        current = ledger.get(resolution["goal_id"])
        if current is None or current.attempt != resolution["attempt"]:
            _fail("dispatch resolution does not match the current goal attempt")
        if current.revision == prepared["expected_revision"]:
            if current.status != "blocked" or current.outcome_digest != prepared.get("expected_outcome_digest") or "worker_restart_after_dispatch_requires_reconciliation" not in current.blockers or int(resolution["observed_ns"]) <= current.updated_ns:
                _fail("goal changed after dispatch resolution was staged")
            next_action = (goal_task_digest("goal-reconciliation-review") if target_status in {"blocked", "failed", "paused"} else goal_task_digest("goal-retry") if target_status == "ready" else None)
            updated = ledger.transition(
                resolution["goal_id"], target_status, expected_revision=current.revision,
                blockers=current.blockers if target_status == "blocked" else ("required_goal_criteria_review",) if target_status == "paused" else (), next_action_digest=next_action,
                outcome_digest=resolution_digest, now_ns=resolution["observed_ns"],
            )
        elif current.revision == latest.revision and current.status == target_status and current.outcome_digest == resolution_digest:
            updated = current
        else:
            _fail("goal revision has drifted from its staged dispatch resolution")
        if target_status != "blocked" and latest.phase == "reconciled":
            self.record(
                batch_id=dispatch.batch_id, goal_id=dispatch.goal_id, phase="settled", attempt=dispatch.attempt,
                revision=updated.revision, schedule_digest=dispatch.schedule_digest, claim_digest=dispatch.claim_digest,
                outcome_digest=resolution_digest, task_digest=dispatch.task_digest,
                execution_binding_digest=dispatch.execution_binding_digest, resolution_goal_status=target_status,
                created_ns=resolution["observed_ns"],
            )
        return self._dispatch_resolution_result(resolution, updated.status, updated.revision, idempotent=False)

    @staticmethod
    def _dispatch_resolution_result(resolution: Mapping[str, Any], goal_status: str, goal_revision: int, *, idempotent: bool) -> dict[str, Any]:
        return {
            "schema": GOAL_DISPATCH_RESOLUTION_SCHEMA, "goal_id": resolution["goal_id"],
            "attempt": resolution["attempt"], "dispatch_event_digest": resolution["dispatch_event_digest"],
            "resolution_digest": content_digest(resolution), "status": resolution["status"],
            "goal_status": goal_status, "goal_revision": goal_revision, "idempotent": idempotent,
            "retention": GOAL_DISPATCH_RESOLUTION_RETENTION, "secret_material": "never_returned",
        }

    def reconcile_external_outcome(
        self,
        ledger: AutonomousGoalLedger,
        raw: Mapping[str, Any],
        verifier: AutonomousGoalDispatchResolutionVerifier | None,
    ) -> dict[str, Any]:
        """Authenticate a status receipt and stage it; persistence coordinators stage it durably."""
        prepared = self.prepare_external_outcome(ledger, raw, verifier)
        if prepared["idempotent"] is True:
            return self._dispatch_resolution_result(prepared["resolution"], prepared["target_status"], prepared["goal_revision"], idempotent=True)
        self.stage_external_outcome(prepared)
        return self.commit_external_outcome(ledger, prepared)

    def snapshot(self) -> dict[str, Any]:
        body = {
            "schema": GOAL_WORKER_JOURNAL_SNAPSHOT_SCHEMA,
            "sequence": len(self._events),
            "head_digest": self.head_digest,
            "events": [event.to_dict() for event in self._events],
            "retention": GOAL_WORKER_JOURNAL_RETENTION,
            "secret_material": "never_returned",
        }
        if self._migration is not None:
            body["migration"] = dict(self._migration)
        if len(canonical_json(body).encode("utf-8")) > MAX_GOAL_WORKER_JOURNAL_SNAPSHOT_BYTES:
            _fail("snapshot exceeds its byte bound")
        return {**body, "snapshot_digest": content_digest(body)}

    @staticmethod
    def validate_snapshot(value: Mapping[str, Any]) -> dict[str, Any]:
        if not isinstance(value, Mapping) or value.get("schema") != GOAL_WORKER_JOURNAL_SNAPSHOT_SCHEMA:
            _fail("snapshot schema is invalid")
        allowed = {"schema", "sequence", "head_digest", "events", "snapshot_digest", "retention", "secret_material", "migration"}
        if set(value).difference(allowed) or value.get("retention") != GOAL_WORKER_JOURNAL_RETENTION or value.get("secret_material") != "never_returned":
            _fail("snapshot contains unsupported or unsafe fields")
        raw_events = value.get("events")
        if not isinstance(raw_events, Sequence) or isinstance(raw_events, (str, bytes, bytearray)) or len(raw_events) > MAX_GOAL_WORKER_JOURNAL_EVENTS:
            _fail("snapshot events are outside their bounds")
        sequence = _integer(value.get("sequence"), name="snapshot.sequence", maximum=MAX_GOAL_WORKER_JOURNAL_EVENTS)
        if sequence != len(raw_events):
            _fail("snapshot sequence does not match its events")
        head = value.get("head_digest")
        if sequence == 0:
            if head != "":
                _fail("empty snapshot must have an empty head digest")
        else:
            _digest(head, name="snapshot.head_digest")
        events = tuple(AutonomousGoalWorkerEvent.from_mapping(raw) for raw in raw_events)
        previous = ""
        latest_by_goal: dict[str, AutonomousGoalWorkerEvent] = {}
        for index, event in enumerate(events, start=1):
            if event.sequence != index or event.previous_digest != previous:
                _fail(f"snapshot event chain breaks at sequence {index}")
            prior = latest_by_goal.get(event.goal_id)
            if event.phase == "prepared" and prior is not None and (
                prior.phase in _ACTIVE_PHASES or _needs_external_reconciliation(prior)
            ):
                _fail(f"snapshot preparation for goal {event.goal_id} overwrites an active journal boundary")
            latest_by_goal[event.goal_id] = event
            previous = event.event_digest
        if previous != head:
            _fail("snapshot head digest does not match its event chain")
        body = {key: value[key] for key in ("schema", "sequence", "head_digest", "events", "retention", "secret_material")}
        migration = _normalize_journal_migration(value.get("migration")) if "migration" in value else None
        if migration is not None:
            body["migration"] = migration
        supplied = _digest(value.get("snapshot_digest"), name="snapshot.snapshot_digest")
        if content_digest(body) != supplied:
            _fail("snapshot digest does not match its content")
        if len(canonical_json(dict(value)).encode("utf-8")) > MAX_GOAL_WORKER_JOURNAL_SNAPSHOT_BYTES:
            _fail("snapshot exceeds its byte bound")
        return {**body, "snapshot_digest": supplied}

    def restore(self, value: Mapping[str, Any]) -> dict[str, Any]:
        normalized = self.validate_snapshot(value)
        events = [AutonomousGoalWorkerEvent.from_mapping(raw) for raw in normalized["events"]]
        latest: dict[str, AutonomousGoalWorkerEvent] = {}
        for event in events:
            latest[event.goal_id] = event
        reserve = sum(_future_event_reserve(event) for event in latest.values())
        if len(events) + reserve > self.max_events:
            _fail("snapshot capacity cannot preserve active recovery transitions")
        self._events = events
        self._latest_by_goal = latest
        self._future_event_reserve_count = reserve
        self._migration = normalized.get("migration")
        return {"schema": GOAL_WORKER_JOURNAL_SCHEMA, "sequence": len(events), "head_digest": self.head_digest, "retention": GOAL_WORKER_JOURNAL_RETENTION, "secret_material": "never_returned"}

    def recover(
        self,
        ledger: AutonomousGoalLedger,
        *,
        now_ns: int | str | None = None,
        goal_id: str | None = None,
    ) -> dict[str, Any]:
        if not isinstance(ledger, AutonomousGoalLedger):
            _fail("recover requires an AutonomousGoalLedger")
        if now_ns is not None:
            now_ns = _timestamp(now_ns, name="recover.now_ns")
        normalized_goal_id = None if goal_id is None else _identifier(goal_id, name="recover.goal_id")
        recovered: list[dict[str, Any]] = []
        for event in self.active():
            if normalized_goal_id is not None and event.goal_id != normalized_goal_id:
                continue
            current = ledger.get(event.goal_id)
            if event.phase == "prepared":
                if current is None:
                    _fail(f"prepared event for goal {event.goal_id} has no ledger record")
                if current.attempt == event.attempt and current.revision == event.revision and current.status in {"ready", "paused", "failed"}:
                    recovery_attempt = event.attempt
                    updated = current
                elif (
                    current.status == "running"
                    and current.attempt == event.attempt + 1
                    and current.revision - event.revision in {1, 2}
                ):
                    recovery_attempt = current.attempt
                    outcome_digest = _restart_outcome_digest(event.goal_id, recovery_attempt, before_dispatch=True)
                    updated = ledger.transition(
                        event.goal_id,
                        "paused",
                        expected_revision=current.revision,
                        blockers=("worker_restart_before_dispatch",),
                        next_action_digest=goal_task_digest("goal-retry"),
                        outcome_digest=outcome_digest,
                        now_ns=now_ns,
                    )
                elif (
                    current.status == "paused"
                    and current.attempt == event.attempt + 1
                    and current.revision - event.revision in {2, 3}
                    and current.outcome_digest == _restart_outcome_digest(event.goal_id, current.attempt, before_dispatch=True)
                    and "worker_restart_before_dispatch" in current.blockers
                ):
                    recovery_attempt = current.attempt
                    updated = current
                else:
                    _fail(f"prepared event for goal {event.goal_id} no longer matches the ledger")
                outcome_digest = _restart_outcome_digest(event.goal_id, recovery_attempt, before_dispatch=True)
                self.record(
                    batch_id=event.batch_id,
                    goal_id=event.goal_id,
                    phase="reconciled",
                    attempt=recovery_attempt,
                    revision=updated.revision,
                    schedule_digest=event.schedule_digest,
                    claim_digest=event.claim_digest,
                    outcome_digest=outcome_digest,
                    task_digest=event.task_digest,
                    execution_binding_digest=event.execution_binding_digest,
                )
                recovered.append({"goal_id": event.goal_id, "from_phase": "prepared", "goal_status": updated.status, "outcome_digest": outcome_digest})
                continue
            if event.resolution_goal_status is not None:
                if current is None or current.attempt != event.attempt:
                    _fail(f"staged dispatch resolution for goal {event.goal_id} no longer matches the ledger")
                target_status = event.resolution_goal_status
                if current.revision == event.revision - 1:
                    history = self.events(goal_id=event.goal_id)
                    event_index = next((index for index, candidate in enumerate(history) if candidate.event_digest == event.event_digest), -1)
                    prior_digest = history[event_index - 1].outcome_digest if event_index > 0 else None
                    if current.status != "blocked" or current.outcome_digest != prior_digest or "worker_restart_after_dispatch_requires_reconciliation" not in current.blockers:
                        _fail(f"staged dispatch resolution for goal {event.goal_id} cannot be applied to the current ledger state")
                    next_action = (goal_task_digest("goal-reconciliation-review") if target_status in {"blocked", "failed", "paused"} else goal_task_digest("goal-retry") if target_status == "ready" else None)
                    current = ledger.transition(
                        event.goal_id, target_status, expected_revision=current.revision,
                        blockers=current.blockers if target_status == "blocked" else ("required_goal_criteria_review",) if target_status == "paused" else (), next_action_digest=next_action,
                        outcome_digest=event.outcome_digest, now_ns=event.created_ns,
                    )
                elif current.revision != event.revision or current.status != target_status or current.outcome_digest != event.outcome_digest:
                    _fail(f"staged dispatch resolution for goal {event.goal_id} conflicts with the current ledger state")
                if target_status != "blocked" and event.phase == "reconciled":
                    self.record(
                        batch_id=event.batch_id, goal_id=event.goal_id, phase="settled", attempt=event.attempt,
                        revision=current.revision, schedule_digest=event.schedule_digest, claim_digest=event.claim_digest,
                        outcome_digest=event.outcome_digest, task_digest=event.task_digest,
                        execution_binding_digest=event.execution_binding_digest,
                        resolution_goal_status=target_status, created_ns=event.created_ns,
                    )
                recovered.append({"goal_id": event.goal_id, "from_phase": "dispatch_started", "goal_status": current.status, "outcome_digest": event.outcome_digest})
                continue
            if _needs_external_reconciliation(event):
                if current is None or current.status != "blocked" or current.attempt != event.attempt or current.revision != event.revision or "worker_restart_after_dispatch_requires_reconciliation" not in current.blockers:
                    _fail(f"pending external resolution for goal {event.goal_id} no longer matches the ledger")
                recovered.append({"goal_id": event.goal_id, "from_phase": "dispatch_started", "goal_status": current.status, "outcome_digest": event.outcome_digest})
                continue
            if (
                event.phase == "dispatch_started"
                and current is not None
                and current.attempt == event.attempt
                and current.revision == event.revision + 1
                and current.status in {"completed", "failed", "paused", "blocked"}
                and current.outcome_digest is not None
                and "worker_restart_after_dispatch_requires_reconciliation" not in current.blockers
            ):
                # The ledger transition is atomic and precedes the journal's terminal event.
                # Seal that already-committed result instead of treating it as an interrupted
                # provider call that needs another external status receipt.
                self.record(
                    batch_id=event.batch_id,
                    goal_id=event.goal_id,
                    phase="settled",
                    attempt=event.attempt,
                    revision=current.revision,
                    schedule_digest=event.schedule_digest,
                    claim_digest=event.claim_digest,
                    outcome_digest=current.outcome_digest,
                    task_digest=event.task_digest,
                    execution_binding_digest=event.execution_binding_digest,
                )
                recovered.append({"goal_id": event.goal_id, "from_phase": "dispatch_started", "goal_status": current.status, "outcome_digest": current.outcome_digest})
                continue
            before_dispatch = event.phase == "claimed"
            target = "paused" if before_dispatch else "blocked"
            blocker = "worker_restart_before_dispatch" if before_dispatch else "worker_restart_after_dispatch_requires_reconciliation"
            next_action = "goal-retry" if before_dispatch else "goal-reconciliation-review"
            outcome_digest = _restart_outcome_digest(event.goal_id, event.attempt, before_dispatch=before_dispatch)
            if current is not None and current.attempt == event.attempt and current.revision == event.revision + 1 and current.status == target and current.outcome_digest == outcome_digest and blocker in current.blockers:
                updated = current
                self.record(
                    batch_id=event.batch_id, goal_id=event.goal_id, phase="reconciled", attempt=event.attempt,
                    revision=updated.revision, schedule_digest=event.schedule_digest, claim_digest=event.claim_digest,
                    outcome_digest=outcome_digest, task_digest=event.task_digest,
                    execution_binding_digest=event.execution_binding_digest,
                )
                recovered.append({"goal_id": event.goal_id, "from_phase": "claimed" if before_dispatch else "dispatch_started", "goal_status": updated.status, "outcome_digest": outcome_digest})
                continue
            if current is None or current.status != "running" or current.revision != event.revision or current.attempt != event.attempt:
                _fail(f"active event for goal {event.goal_id} no longer matches the ledger")
            updated = ledger.transition(
                event.goal_id,
                target,
                expected_revision=current.revision,
                blockers=(blocker,),
                next_action_digest=goal_task_digest(next_action),
                outcome_digest=outcome_digest,
                now_ns=now_ns,
            )
            self.record(
                batch_id=event.batch_id,
                goal_id=event.goal_id,
                phase="reconciled",
                attempt=event.attempt,
                revision=updated.revision,
                schedule_digest=event.schedule_digest,
                claim_digest=event.claim_digest,
                outcome_digest=outcome_digest,
                task_digest=event.task_digest,
                execution_binding_digest=event.execution_binding_digest,
            )
            recovered.append({"goal_id": event.goal_id, "from_phase": "claimed" if before_dispatch else "dispatch_started", "goal_status": updated.status, "outcome_digest": outcome_digest})
        return {"schema": GOAL_WORKER_JOURNAL_SCHEMA, "recovered": recovered, "recovery_digest": content_digest(recovered), "retention": GOAL_WORKER_JOURNAL_RETENTION, "secret_material": "never_returned"}


def migrate_legacy_autonomous_goal_worker_journal_snapshot(
    value: Mapping[str, Any], source_unit: AutonomousGoalLegacyTimestampUnit,
) -> dict[str, Any]:
    """Verify a v0.1 worker journal and explicitly convert numeric timestamps to v0.2 ns."""

    if source_unit not in {"milliseconds", "nanoseconds"}:
        _fail("legacy snapshot migration requires milliseconds or nanoseconds as the source unit")
    if not isinstance(value, Mapping) or value.get("schema") != GOAL_WORKER_JOURNAL_SNAPSHOT_SCHEMA_V01:
        _fail("legacy snapshot schema is unsupported")
    snapshot_keys = {"schema", "sequence", "head_digest", "events", "snapshot_digest", "retention", "secret_material"}
    if set(value) != snapshot_keys or value.get("retention") != GOAL_WORKER_JOURNAL_RETENTION or value.get("secret_material") != "never_returned":
        _fail("legacy snapshot contains unsupported or unsafe metadata")
    raw_events = value.get("events")
    if not isinstance(raw_events, Sequence) or isinstance(raw_events, (str, bytes, bytearray)) or len(raw_events) > MAX_GOAL_WORKER_JOURNAL_EVENTS:
        _fail("legacy snapshot events are outside their bounds")
    sequence = _integer(value.get("sequence"), name="legacy snapshot.sequence", maximum=MAX_GOAL_WORKER_JOURNAL_EVENTS)
    if sequence != len(raw_events):
        _fail("legacy snapshot sequence does not match its events")
    try:
        encoded_size = len(canonical_json(dict(value)).encode("utf-8"))
    except (TypeError, ValueError) as error:
        raise AutonomousGoalError("autonomous goal worker journal legacy snapshot is not canonical JSON") from error
    if encoded_size > MAX_GOAL_WORKER_JOURNAL_SNAPSHOT_BYTES:
        _fail("legacy snapshot exceeds its byte bound")
    legacy_body = {key: item for key, item in value.items() if key != "snapshot_digest"}
    source_snapshot_digest = _digest(value.get("snapshot_digest"), name="legacy snapshot.snapshot_digest")
    if content_digest(legacy_body) != source_snapshot_digest:
        _fail("legacy snapshot digest does not match its content")
    source_head = value.get("head_digest")
    if sequence == 0:
        if source_head != "":
            _fail("empty legacy snapshot must have an empty head digest")
    else:
        _digest(source_head, name="legacy snapshot.head_digest")

    required_event_keys = {
        "schema", "sequence", "batch_id", "goal_id", "phase", "attempt", "revision", "schedule_digest",
        "claim_digest", "outcome_digest", "error_digest", "created_ns", "previous_digest", "event_digest",
        "retention", "secret_material",
    }
    optional_event_keys = {"task_digest", "execution_binding_digest", "resolution_goal_status"}
    migrated_events: list[dict[str, Any]] = []
    source_previous = ""
    migrated_previous = ""
    for index, raw in enumerate(raw_events, start=1):
        if not isinstance(raw, Mapping) or not required_event_keys.issubset(raw) or set(raw).difference(required_event_keys | optional_event_keys):
            _fail(f"legacy event {index} is malformed")
        if raw.get("schema") != GOAL_WORKER_JOURNAL_EVENT_SCHEMA_V01 or raw.get("retention") != GOAL_WORKER_JOURNAL_RETENTION or raw.get("secret_material") != "never_returned":
            _fail(f"legacy event {index} markers are invalid")
        if _integer(raw.get("sequence"), name=f"legacy event {index}.sequence", minimum=1, maximum=MAX_GOAL_WORKER_JOURNAL_EVENTS) != index or raw.get("previous_digest") != source_previous:
            _fail(f"legacy event chain breaks at sequence {index}")
        legacy_event_body = {key: item for key, item in raw.items() if key != "event_digest"}
        source_event_digest = _digest(raw.get("event_digest"), name=f"legacy event {index}.event_digest")
        if content_digest(legacy_event_body) != source_event_digest:
            _fail(f"legacy event {index} digest does not match its content")
        migrated_body = {
            **legacy_event_body,
            "schema": GOAL_WORKER_JOURNAL_EVENT_SCHEMA,
            "created_ns": _migrate_timestamp(raw.get("created_ns"), source_unit, name=f"legacy event {index}.created_ns"),
            "previous_digest": migrated_previous,
        }
        migrated_event = {**migrated_body, "event_digest": content_digest(migrated_body)}
        parsed_event = AutonomousGoalWorkerEvent.from_mapping(migrated_event).to_dict()
        migrated_events.append(parsed_event)
        source_previous = source_event_digest
        migrated_previous = parsed_event["event_digest"]
    if source_previous != source_head:
        _fail("legacy snapshot head digest does not match its event chain")
    migration = {
        "source_schema": GOAL_WORKER_JOURNAL_SNAPSHOT_SCHEMA_V01,
        "source_timestamp_unit": source_unit,
        "source_snapshot_digest": source_snapshot_digest,
        "source_head_digest": source_head,
    }
    migrated_body = {
        "schema": GOAL_WORKER_JOURNAL_SNAPSHOT_SCHEMA,
        "sequence": len(migrated_events),
        "head_digest": migrated_previous,
        "events": migrated_events,
        "retention": GOAL_WORKER_JOURNAL_RETENTION,
        "secret_material": "never_returned",
        "migration": migration,
    }
    return AutonomousGoalWorkerJournal.validate_snapshot({**migrated_body, "snapshot_digest": content_digest(migrated_body)})


def migrate_legacy_authenticated_autonomous_goal_worker_journal_envelope(
    value: Mapping[str, Any],
    source_unit: AutonomousGoalLegacyTimestampUnit,
    *,
    keys: Mapping[str, bytes],
    active_key_id: str,
) -> dict[str, Any]:
    """Authenticate a v0.1 shared-store envelope, migrate, and reseal under a current key."""

    if not isinstance(value, Mapping) or not isinstance(value.get("authentication"), Mapping):
        _fail("legacy authenticated journal envelope is malformed")
    try:
        if len(canonical_json(dict(value)).encode("utf-8")) > MAX_AUTHENTICATED_GOAL_WORKER_JOURNAL_BYTES:
            _fail("legacy authenticated journal envelope exceeds its byte bound")
    except (TypeError, ValueError) as error:
        raise AutonomousGoalError("autonomous goal worker legacy authenticated envelope is not canonical JSON") from error
    authentication = value["authentication"]
    if set(authentication) != {"schema", "key_id", "tag"} or authentication.get("schema") != AUTHENTICATED_GOAL_WORKER_JOURNAL_SCHEMA_V01:
        _fail("legacy authenticated journal envelope markers are invalid")
    normalized_keys: dict[str, bytes] = {}
    if not isinstance(keys, Mapping) or not 1 <= len(keys) <= 16:
        _fail("legacy authenticated journal keyring must contain 1..16 keys")
    for raw_id, raw_key in keys.items():
        key_id = _identifier(raw_id, name="legacy authentication key id")
        if key_id in normalized_keys or not isinstance(raw_key, bytes) or not 32 <= len(raw_key) <= 4096:
            _fail("legacy authenticated journal keyring contains a duplicate id or an invalid key")
        normalized_keys[key_id] = bytes(raw_key)
    source_key_id = _identifier(authentication.get("key_id"), name="legacy authentication key id")
    source_tag = _digest(authentication.get("tag"), name="legacy authentication tag")
    source_key = normalized_keys.get(source_key_id)
    if source_key is None:
        _fail("legacy authenticated journal key id is not trusted by this keyring")
    source_snapshot = {name: item for name, item in value.items() if name != "authentication"}
    source_body = {"schema": AUTHENTICATED_GOAL_WORKER_JOURNAL_SCHEMA_V01, "key_id": source_key_id, "snapshot": source_snapshot}
    expected_source_tag = hmac.new(source_key, canonical_json(source_body).encode("utf-8"), hashlib.sha256).hexdigest()
    if not hmac.compare_digest(expected_source_tag, source_tag):
        _fail("legacy authenticated journal tag does not match its snapshot")

    active_id = _identifier(active_key_id, name="active authentication key id")
    active_key = normalized_keys.get(active_id)
    if active_key is None:
        _fail("active authentication key id is not present in the keyring")
    snapshot = migrate_legacy_autonomous_goal_worker_journal_snapshot(source_snapshot, source_unit)
    body = {"schema": AUTHENTICATED_GOAL_WORKER_JOURNAL_SCHEMA, "key_id": active_id, "snapshot": snapshot}
    tag = hmac.new(active_key, canonical_json(body).encode("utf-8"), hashlib.sha256).hexdigest()
    envelope = {**snapshot, "authentication": {"schema": AUTHENTICATED_GOAL_WORKER_JOURNAL_SCHEMA, "key_id": active_id, "tag": tag}}
    if len(canonical_json(envelope).encode("utf-8")) > MAX_AUTHENTICATED_GOAL_WORKER_JOURNAL_BYTES:
        _fail("migrated authenticated journal envelope exceeds its byte bound")
    return envelope


class GoalWorkerJournalTextStore(Protocol):
    def read(self) -> str | None: ...
    def write(self, value: str) -> None: ...


class TransactionalGoalWorkerJournalTextStore(GoalWorkerJournalTextStore, Protocol):
    def write_if_unchanged(self, expected_snapshot_digest: str | None, value: str) -> bool: ...


class JsonAutonomousGoalWorkerJournalPersistence:
    """Canonical JSON adapter; the caller owns encryption, authorization, and durability."""

    def __init__(self, store: GoalWorkerJournalTextStore) -> None:
        if not callable(getattr(store, "read", None)) or not callable(getattr(store, "write", None)):
            _fail("journal text store must implement read and write")
        self.store = store

    def read(self) -> dict[str, Any] | None:
        raw = self.store.read()
        if raw is None:
            return None
        raw_bytes = utf8_scalar_byte_length(raw) if isinstance(raw, str) else None
        if raw_bytes is None or raw_bytes > MAX_GOAL_WORKER_JOURNAL_SNAPSHOT_BYTES:
            _fail("journal JSON is outside its byte bound")
        try:
            value = json.loads(raw)
        except (TypeError, ValueError) as error:
            raise AutonomousGoalError("journal JSON is invalid") from error
        if canonical_json(value) != raw:
            _fail("journal JSON is not canonical")
        return AutonomousGoalWorkerJournal.validate_snapshot(value)

    def write(self, value: Mapping[str, Any]) -> None:
        normalized = AutonomousGoalWorkerJournal.validate_snapshot(value)
        self.store.write(canonical_json(normalized))


class AuthenticatedTransactionalJsonAutonomousGoalWorkerJournalPersistence(JsonAutonomousGoalWorkerJournalPersistence):
    """HMAC-authenticated metadata snapshots with required shared-store CAS.

    The keyring is supplied by deployment configuration. Readers may retain previous keys during
    rotation; every write uses the active key. This authenticates integrity and origin only. Store
    authorization, encryption, durability, and anti-rollback policy remain deployment-owned.
    """

    def __init__(
        self,
        store: TransactionalGoalWorkerJournalTextStore,
        *,
        keys: Mapping[str, bytes],
        active_key_id: str,
    ) -> None:
        super().__init__(store)
        if not callable(getattr(store, "write_if_unchanged", None)):
            _fail("authenticated shared journal persistence requires write_if_unchanged")
        if not isinstance(keys, Mapping) or not 1 <= len(keys) <= 16:
            _fail("authenticated journal keyring must contain 1..16 keys")
        normalized_keys: dict[str, bytes] = {}
        for raw_id, raw_key in keys.items():
            key_id = _identifier(raw_id, name="authentication key id")
            if key_id in normalized_keys or not isinstance(raw_key, bytes) or not 32 <= len(raw_key) <= 4096:
                _fail("authenticated journal keyring contains a duplicate id or an invalid key")
            normalized_keys[key_id] = bytes(raw_key)
        active_id = _identifier(active_key_id, name="active authentication key id")
        if active_id not in normalized_keys:
            _fail("active authentication key id is not present in the keyring")
        self._keys = normalized_keys
        self.active_key_id = active_id

    @staticmethod
    def _authentication_body(snapshot: Mapping[str, Any], key_id: str) -> dict[str, Any]:
        return {"schema": AUTHENTICATED_GOAL_WORKER_JOURNAL_SCHEMA, "key_id": key_id, "snapshot": snapshot}

    def _envelope(self, value: Mapping[str, Any]) -> dict[str, Any]:
        snapshot = AutonomousGoalWorkerJournal.validate_snapshot(value)
        body = self._authentication_body(snapshot, self.active_key_id)
        tag = hmac.new(self._keys[self.active_key_id], canonical_json(body).encode("utf-8"), hashlib.sha256).hexdigest()
        return {
            **snapshot,
            "authentication": {"schema": AUTHENTICATED_GOAL_WORKER_JOURNAL_SCHEMA, "key_id": self.active_key_id, "tag": tag},
        }

    def read(self) -> dict[str, Any] | None:
        encoded = self.store.read()
        if encoded is None:
            return None
        encoded_bytes = utf8_scalar_byte_length(encoded) if isinstance(encoded, str) else None
        if encoded_bytes is None or encoded_bytes > MAX_AUTHENTICATED_GOAL_WORKER_JOURNAL_BYTES:
            _fail("authenticated journal JSON is outside its byte bound")
        try:
            raw = json.loads(encoded)
        except (TypeError, ValueError, json.JSONDecodeError) as error:
            raise AutonomousGoalError("autonomous goal worker journal JSON is invalid") from error
        if not isinstance(raw, Mapping) or not isinstance(raw.get("authentication"), Mapping):
            _fail("authenticated journal envelope is malformed")
        authentication = raw["authentication"]
        if set(authentication) != {"schema", "key_id", "tag"} or authentication["schema"] != AUTHENTICATED_GOAL_WORKER_JOURNAL_SCHEMA:
            _fail("authenticated journal envelope markers are invalid")
        key_id = _identifier(authentication["key_id"], name="authentication key id")
        supplied_tag = _digest(authentication["tag"], name="authentication tag")
        key = self._keys.get(key_id)
        if key is None:
            _fail("authenticated journal key id is not trusted by this keyring")
        snapshot_raw = {name: item for name, item in raw.items() if name != "authentication"}
        snapshot = AutonomousGoalWorkerJournal.validate_snapshot(snapshot_raw)
        expected = hmac.new(key, canonical_json(self._authentication_body(snapshot, key_id)).encode("utf-8"), hashlib.sha256).hexdigest()
        if not hmac.compare_digest(expected, supplied_tag):
            _fail("authenticated journal tag does not match its snapshot")
        normalized_envelope = {
            **snapshot,
            "authentication": {"schema": AUTHENTICATED_GOAL_WORKER_JOURNAL_SCHEMA, "key_id": key_id, "tag": supplied_tag},
        }
        if canonical_json(normalized_envelope) != encoded:
            _fail("authenticated journal JSON is not canonical")
        return snapshot

    def write(self, value: Mapping[str, Any]) -> None:
        current = self.read()
        expected = None if current is None else current["snapshot_digest"]
        if not self.write_if_unchanged(expected, value):
            _fail("authenticated journal persistence compare-and-swap conflict")

    def write_if_unchanged(self, expected_snapshot_digest: str | None, value: Mapping[str, Any]) -> bool:
        if expected_snapshot_digest is not None:
            _digest(expected_snapshot_digest, name="expected_snapshot_digest")
        envelope = self._envelope(value)
        if expected_snapshot_digest == envelope["snapshot_digest"]:
            current = self.read()
            return current is not None and current["snapshot_digest"] == expected_snapshot_digest
        encoded = canonical_json(envelope)
        if len(encoded.encode("utf-8")) > MAX_AUTHENTICATED_GOAL_WORKER_JOURNAL_BYTES:
            _fail("authenticated journal snapshot exceeds its byte bound")
        return self.store.write_if_unchanged(expected_snapshot_digest, encoded)


class AutonomousGoalWorkerJournalPersistenceCoordinator:
    """Restore/flush coordinator that keeps a caller-owned journal snapshot CAS-safe."""

    def __init__(self, journal: AutonomousGoalWorkerJournal, persistence: JsonAutonomousGoalWorkerJournalPersistence) -> None:
        if not isinstance(journal, AutonomousGoalWorkerJournal):
            _fail("coordinator journal is invalid")
        if not isinstance(persistence, JsonAutonomousGoalWorkerJournalPersistence):
            _fail("coordinator persistence is invalid")
        self.journal = journal
        self.persistence = persistence
        self._expected_snapshot_digest: str | None = None

    def restore(self) -> dict[str, Any] | None:
        value = self.persistence.read()
        if value is None:
            self._expected_snapshot_digest = None
            return None
        self.journal.restore(value)
        self._expected_snapshot_digest = value["snapshot_digest"]
        return value

    def flush(self) -> dict[str, Any]:
        snapshot = self.journal.snapshot()
        adapter_write_if_unchanged = getattr(self.persistence, "write_if_unchanged", None)
        if callable(adapter_write_if_unchanged):
            if not adapter_write_if_unchanged(self._expected_snapshot_digest, snapshot):
                _fail("journal persistence compare-and-swap conflict")
        else:
            write_if_unchanged = getattr(self.persistence.store, "write_if_unchanged", None)
            if callable(write_if_unchanged):
                if not write_if_unchanged(self._expected_snapshot_digest, canonical_json(snapshot)):
                    _fail("journal persistence compare-and-swap conflict")
            else:
                self.persistence.write(snapshot)
        self._expected_snapshot_digest = snapshot["snapshot_digest"]
        return snapshot

    def reconcile_external_outcome(
        self,
        ledger: AutonomousGoalLedger,
        raw: Mapping[str, Any],
        verifier: AutonomousGoalDispatchResolutionVerifier | None,
    ) -> dict[str, Any]:
        """Authenticate then durably stage status before changing the goal ledger."""
        prepared = self.journal.prepare_external_outcome(ledger, raw, verifier)
        if prepared["idempotent"] is True:
            return self.journal.commit_external_outcome(ledger, prepared)
        self.journal.stage_external_outcome(prepared)
        self.flush()
        result = self.journal.commit_external_outcome(ledger, prepared)
        if result["goal_status"] != "blocked":
            self.flush()
        return result


__all__ = [
    "GOAL_DISPATCH_RESOLUTION_RETENTION",
    "GOAL_DISPATCH_RESOLUTION_SCHEMA",
    "AUTHENTICATED_GOAL_WORKER_JOURNAL_SCHEMA",
    "GOAL_WORKER_JOURNAL_EVENT_SCHEMA",
    "GOAL_WORKER_JOURNAL_RETENTION",
    "GOAL_WORKER_JOURNAL_SCHEMA",
    "GOAL_WORKER_JOURNAL_SNAPSHOT_SCHEMA",
    "MAX_GOAL_WORKER_JOURNAL_EVENTS",
    "MAX_GOAL_WORKER_JOURNAL_SNAPSHOT_BYTES",
    "AutonomousGoalWorkerEvent",
    "AutonomousGoalDispatchResolutionVerifier",
    "AutonomousGoalWorkerJournal",
    "AutonomousGoalWorkerJournalPersistenceCoordinator",
    "AuthenticatedTransactionalJsonAutonomousGoalWorkerJournalPersistence",
    "AutonomousGoalWorkerJournalSnapshot",
    "GoalDispatchResolutionStatus",
    "GoalWorkerJournalTextStore",
    "TransactionalGoalWorkerJournalTextStore",
    "JsonAutonomousGoalWorkerJournalPersistence",
    "validate_goal_dispatch_resolution",
]
