"""Restart and reconciliation journal for metadata-only goal workers.

The goal ledger knows that an objective is ``running`` but cannot tell whether a worker died
before or after crossing the executor/provider boundary.  This journal records that distinction
without retaining task text, prompts, parameters, credentials, or executor output.  Recovery is
deliberately conservative: a pre-dispatch interruption is paused for a safe retry; a post-dispatch
interruption is blocked until the caller reconciles the uncertain external outcome.
"""

from __future__ import annotations

from dataclasses import dataclass
import json
import time
from collections.abc import Mapping, Sequence
from typing import Any, Callable, Literal, Protocol

from .authoring import canonical_json, content_digest
from .goals import AutonomousGoalError, AutonomousGoalLedger, goal_task_digest


GOAL_WORKER_JOURNAL_SCHEMA = "bioprism-autonomous-goal-worker-journal/0.1"
GOAL_WORKER_JOURNAL_EVENT_SCHEMA = "bioprism-autonomous-goal-worker-event/0.1"
GOAL_WORKER_JOURNAL_SNAPSHOT_SCHEMA = "bioprism-autonomous-goal-worker-snapshot/0.1"
GOAL_WORKER_JOURNAL_RETENTION = "metadata_only_worker_boundary;tasks_prompts_parameters_credentials_and_results_not_retained"
MAX_GOAL_WORKER_JOURNAL_EVENTS = 16_384
MAX_GOAL_WORKER_JOURNAL_SNAPSHOT_BYTES = 2_000_000

WorkerJournalPhase = Literal[
    "prepared",
    "claimed",
    "dispatch_started",
    "settled",
    "failed",
    "reconciled",
]
_ACTIVE_PHASES = frozenset({"claimed", "dispatch_started"})
_ALL_PHASES = frozenset({"prepared", "claimed", "dispatch_started", "settled", "failed", "reconciled"})


def _fail(message: str) -> None:
    raise AutonomousGoalError(f"autonomous goal worker journal {message}")


def _identifier(value: Any, *, name: str) -> str:
    if not isinstance(value, str) or not value.strip() or "\x00" in value or len(value.encode("utf-8")) > 256:
        _fail(f"{name} is outside its bounded identifier contract")
    return value.strip()


def _digest(value: Any, *, name: str, allow_none: bool = False) -> str | None:
    if value is None and allow_none:
        return None
    if not isinstance(value, str) or len(value) != 64 or any(char not in "0123456789abcdef" for char in value):
        _fail(f"{name} must be a lowercase SHA-256 digest")
    return value


def _integer(value: Any, *, name: str, minimum: int = 0, maximum: int = 2**63 - 1) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or value < minimum or value > maximum:
        _fail(f"{name} is outside its integer bounds")
    return value


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
    created_ns: int
    previous_digest: str
    event_digest: str

    def _body(self) -> dict[str, Any]:
        return {
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

    def to_dict(self) -> dict[str, Any]:
        return {**self._body(), "event_digest": self.event_digest}

    @classmethod
    def from_mapping(cls, value: Mapping[str, Any]) -> "AutonomousGoalWorkerEvent":
        if not isinstance(value, Mapping):
            _fail("event must be a mapping")
        allowed = set(AutonomousGoalWorkerEvent._field_names())
        if set(value).difference(allowed):
            _fail("event contains unsupported fields")
        if value.get("schema") != GOAL_WORKER_JOURNAL_EVENT_SCHEMA or value.get("retention") != GOAL_WORKER_JOURNAL_RETENTION or value.get("secret_material") != "never_returned":
            _fail("event retention markers are invalid")
        phase = value.get("phase")
        if phase not in _ALL_PHASES:
            _fail("event phase is invalid")
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
            created_ns=_integer(value.get("created_ns"), name="event.created_ns"),
            previous_digest=(value.get("previous_digest") if isinstance(value.get("previous_digest"), str) else ""),
            event_digest=_digest(value.get("event_digest"), name="event.event_digest") or "",
        )
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
            "previous_digest", "event_digest", "retention", "secret_material",
        )


@dataclass(frozen=True, slots=True)
class AutonomousGoalWorkerJournalSnapshot:
    sequence: int
    head_digest: str
    events: tuple[AutonomousGoalWorkerEvent, ...]
    snapshot_digest: str

    def to_dict(self) -> dict[str, Any]:
        body = {
            "schema": GOAL_WORKER_JOURNAL_SNAPSHOT_SCHEMA,
            "sequence": self.sequence,
            "head_digest": self.head_digest,
            "events": [event.to_dict() for event in self.events],
            "retention": GOAL_WORKER_JOURNAL_RETENTION,
            "secret_material": "never_returned",
        }
        return {**body, "snapshot_digest": self.snapshot_digest}


class AutonomousGoalWorkerJournal:
    """Hash-chained metadata events that fence pre- and post-dispatch recovery."""

    def __init__(self, *, max_events: int = MAX_GOAL_WORKER_JOURNAL_EVENTS, clock: Callable[[], int] | None = None) -> None:
        if isinstance(max_events, bool) or not isinstance(max_events, int) or not 1 <= max_events <= MAX_GOAL_WORKER_JOURNAL_EVENTS:
            _fail(f"max_events must be between 1 and {MAX_GOAL_WORKER_JOURNAL_EVENTS}")
        self.max_events = max_events
        self._clock = clock or time.time_ns
        self._events: list[AutonomousGoalWorkerEvent] = []

    @property
    def head_digest(self) -> str:
        return self._events[-1].event_digest if self._events else ""

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
        created_ns: int | None = None,
    ) -> AutonomousGoalWorkerEvent:
        if len(self._events) >= self.max_events:
            _fail("event capacity is exhausted")
        if phase not in _ALL_PHASES:
            _fail("event phase is invalid")
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
            "created_ns": _integer(self._clock() if created_ns is None else created_ns, name="created_ns"),
            "previous_digest": self.head_digest,
            "retention": GOAL_WORKER_JOURNAL_RETENTION,
            "secret_material": "never_returned",
        }
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
        )
        self._events.append(event)
        return event

    def events(self, *, batch_id: str | None = None, goal_id: str | None = None) -> tuple[AutonomousGoalWorkerEvent, ...]:
        if batch_id is not None:
            batch_id = _identifier(batch_id, name="batch_id")
        if goal_id is not None:
            goal_id = _identifier(goal_id, name="goal_id")
        return tuple(event for event in self._events if (batch_id is None or event.batch_id == batch_id) and (goal_id is None or event.goal_id == goal_id))

    def active(self) -> tuple[AutonomousGoalWorkerEvent, ...]:
        latest: dict[str, AutonomousGoalWorkerEvent] = {}
        for event in self._events:
            latest[event.goal_id] = event
        return tuple(sorted((event for event in latest.values() if event.phase in _ACTIVE_PHASES), key=lambda event: event.sequence))

    def snapshot(self) -> dict[str, Any]:
        body = {
            "schema": GOAL_WORKER_JOURNAL_SNAPSHOT_SCHEMA,
            "sequence": len(self._events),
            "head_digest": self.head_digest,
            "events": [event.to_dict() for event in self._events],
            "retention": GOAL_WORKER_JOURNAL_RETENTION,
            "secret_material": "never_returned",
        }
        if len(canonical_json(body).encode("utf-8")) > MAX_GOAL_WORKER_JOURNAL_SNAPSHOT_BYTES:
            _fail("snapshot exceeds its byte bound")
        return {**body, "snapshot_digest": content_digest(body)}

    @staticmethod
    def validate_snapshot(value: Mapping[str, Any]) -> dict[str, Any]:
        if not isinstance(value, Mapping) or value.get("schema") != GOAL_WORKER_JOURNAL_SNAPSHOT_SCHEMA:
            _fail("snapshot schema is invalid")
        allowed = {"schema", "sequence", "head_digest", "events", "snapshot_digest", "retention", "secret_material"}
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
        for index, event in enumerate(events, start=1):
            if event.sequence != index or event.previous_digest != previous:
                _fail(f"snapshot event chain breaks at sequence {index}")
            previous = event.event_digest
        if previous != head:
            _fail("snapshot head digest does not match its event chain")
        body = {key: value[key] for key in ("schema", "sequence", "head_digest", "events", "retention", "secret_material")}
        supplied = _digest(value.get("snapshot_digest"), name="snapshot.snapshot_digest")
        if content_digest(body) != supplied:
            _fail("snapshot digest does not match its content")
        if len(canonical_json(dict(value)).encode("utf-8")) > MAX_GOAL_WORKER_JOURNAL_SNAPSHOT_BYTES:
            _fail("snapshot exceeds its byte bound")
        return {**body, "snapshot_digest": supplied}

    def restore(self, value: Mapping[str, Any]) -> dict[str, Any]:
        normalized = self.validate_snapshot(value)
        events = [AutonomousGoalWorkerEvent.from_mapping(raw) for raw in normalized["events"]]
        self._events = events
        return {"schema": GOAL_WORKER_JOURNAL_SCHEMA, "sequence": len(events), "head_digest": self.head_digest, "retention": GOAL_WORKER_JOURNAL_RETENTION, "secret_material": "never_returned"}

    def recover(self, ledger: AutonomousGoalLedger, *, now_ns: int | None = None) -> dict[str, Any]:
        if not isinstance(ledger, AutonomousGoalLedger):
            _fail("recover requires an AutonomousGoalLedger")
        if now_ns is not None:
            _integer(now_ns, name="recover.now_ns")
        recovered: list[dict[str, Any]] = []
        for event in self.active():
            current = ledger.get(event.goal_id)
            if current is None or current.status != "running" or current.revision != event.revision or current.attempt != event.attempt:
                _fail(f"active event for goal {event.goal_id} no longer matches the ledger")
            before_dispatch = event.phase == "claimed"
            result_status = "worker_restart_before_dispatch" if before_dispatch else "worker_restart_after_dispatch"
            target = "paused" if before_dispatch else "blocked"
            blocker = "worker_restart_before_dispatch" if before_dispatch else "worker_restart_after_dispatch_requires_reconciliation"
            next_action = "goal-retry" if before_dispatch else "goal-reconciliation-review"
            outcome_digest = content_digest({"goal_id": event.goal_id, "attempt": event.attempt, "result_status": result_status})
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
            )
            recovered.append({"goal_id": event.goal_id, "from_phase": event.phase, "goal_status": updated.status, "outcome_digest": outcome_digest})
        return {"schema": GOAL_WORKER_JOURNAL_SCHEMA, "recovered": recovered, "recovery_digest": content_digest(recovered), "retention": GOAL_WORKER_JOURNAL_RETENTION, "secret_material": "never_returned"}


class GoalWorkerJournalTextStore(Protocol):
    def read(self) -> str | None: ...
    def write(self, value: str) -> None: ...


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
        if not isinstance(raw, str) or len(raw.encode("utf-8")) > MAX_GOAL_WORKER_JOURNAL_SNAPSHOT_BYTES:
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
        write_if_unchanged = getattr(self.persistence.store, "write_if_unchanged", None)
        if callable(write_if_unchanged):
            if not write_if_unchanged(self._expected_snapshot_digest, canonical_json(snapshot)):
                _fail("journal persistence compare-and-swap conflict")
        else:
            self.persistence.write(snapshot)
        self._expected_snapshot_digest = snapshot["snapshot_digest"]
        return snapshot


__all__ = [
    "GOAL_WORKER_JOURNAL_EVENT_SCHEMA",
    "GOAL_WORKER_JOURNAL_RETENTION",
    "GOAL_WORKER_JOURNAL_SCHEMA",
    "GOAL_WORKER_JOURNAL_SNAPSHOT_SCHEMA",
    "MAX_GOAL_WORKER_JOURNAL_EVENTS",
    "MAX_GOAL_WORKER_JOURNAL_SNAPSHOT_BYTES",
    "AutonomousGoalWorkerEvent",
    "AutonomousGoalWorkerJournal",
    "AutonomousGoalWorkerJournalPersistenceCoordinator",
    "AutonomousGoalWorkerJournalSnapshot",
    "GoalWorkerJournalTextStore",
    "JsonAutonomousGoalWorkerJournalPersistence",
]
