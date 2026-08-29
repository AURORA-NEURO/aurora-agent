"""Application lifecycle for the metadata-only autonomous run trace registry.

The registry itself is a deterministic projection over a validated trace journal.  This module
adds the process boundary around it: restore-before-read, serialized mutations, CAS-aware
persistence, and bounded outcomes that keep observability failures separate from task execution.
It never stores prompts, responses, credentials, tool arguments, evidence bodies, or effect
values, and it has no provider or replay authority.
"""

from __future__ import annotations

from contextlib import contextmanager
from dataclasses import dataclass
from threading import RLock
from typing import Any, Iterator, Mapping

from .autonomous_run_trace_registry import (
    AUTONOMOUS_RUN_TRACE_REGISTRY_AUTHORITY,
    AUTONOMOUS_RUN_TRACE_REGISTRY_RETENTION,
    AUTONOMOUS_RUN_TRACE_REGISTRY_SECRET_MATERIAL,
    AutonomousRunTraceRegistry,
    AutonomousRunTraceRegistryImportReport,
    AutonomousRunTraceRegistryPage,
    AutonomousRunTraceRegistryPersistenceCoordinator,
    AutonomousRunTraceRegistryPublication,
    AutonomousRunTraceRegistryRecord,
    AutonomousRunTraceRegistrySnapshot,
    JsonAutonomousRunTraceRegistryPersistence,
    publish_autonomous_run_trace_registry_snapshot,
)
from .autonomous_run_trace import AutonomousRunTraceStore
from .errors import ArgumentError


AUTONOMOUS_BRAIN_TRACE_REGISTRY_CONTROLLER_SCHEMA = "bioprism-python-autonomous-brain-trace-registry-controller/0.1"
AUTONOMOUS_BRAIN_TRACE_REGISTRY_CONTROLLER_STATUSES = (
    "empty",
    "restored",
    "flushed",
    "published",
    "compacted",
    "publication_failed",
    "persistence_failed",
)


def _error_projection(error: BaseException) -> dict[str, str]:
    code = getattr(error, "code", None)
    return {
        "error_class": type(error).__name__ if type(error).__name__ else "AutonomousBrainError",
        "failure_code": code if isinstance(code, str) and code else "error",
    }


@dataclass(frozen=True, slots=True)
class AutonomousBrainTraceRegistryControllerProjection:
    schema: str
    status: str
    snapshot_generation: int | None
    snapshot_digest: str | None
    runs: int
    events: int
    retained_event_count: int
    policy: Mapping[str, Any] | None
    persisted: bool
    retention: str
    authority: str
    secret_material: str

    def __post_init__(self) -> None:
        if self.schema != AUTONOMOUS_BRAIN_TRACE_REGISTRY_CONTROLLER_SCHEMA:
            raise ArgumentError("autonomous brain trace registry controller schema is invalid")
        if self.status not in AUTONOMOUS_BRAIN_TRACE_REGISTRY_CONTROLLER_STATUSES:
            raise ArgumentError("autonomous brain trace registry controller status is invalid")
        if self.snapshot_generation is not None and (isinstance(self.snapshot_generation, bool) or self.snapshot_generation < 1):
            raise ArgumentError("autonomous brain trace registry controller generation is invalid")
        if self.retention != AUTONOMOUS_RUN_TRACE_REGISTRY_RETENTION or self.authority != AUTONOMOUS_RUN_TRACE_REGISTRY_AUTHORITY or self.secret_material != AUTONOMOUS_RUN_TRACE_REGISTRY_SECRET_MATERIAL:
            raise ArgumentError("autonomous brain trace registry controller authority markers are invalid")

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": self.schema,
            "status": self.status,
            "snapshot_generation": self.snapshot_generation,
            "snapshot_digest": self.snapshot_digest,
            "runs": self.runs,
            "events": self.events,
            "retained_event_count": self.retained_event_count,
            "policy": None if self.policy is None else dict(self.policy),
            "persisted": self.persisted,
            "retention": self.retention,
            "authority": self.authority,
            "secret_material": self.secret_material,
        }


@dataclass(frozen=True, slots=True)
class AutonomousBrainTraceRegistryPublicationRun:
    controller: AutonomousBrainTraceRegistryControllerProjection
    publication: AutonomousRunTraceRegistryPublication
    persisted: bool
    persistence_error: Mapping[str, str] | None

    def to_dict(self) -> dict[str, Any]:
        return {
            "controller": self.controller.to_dict(),
            "publication": self.publication.to_dict(),
            "persisted": self.persisted,
            "persistence_error": None if self.persistence_error is None else dict(self.persistence_error),
        }


@dataclass(frozen=True, slots=True)
class AutonomousBrainTraceRegistryImportRun:
    controller: AutonomousBrainTraceRegistryControllerProjection
    report: AutonomousRunTraceRegistryImportReport
    persisted: bool
    persistence_error: Mapping[str, str] | None

    def to_dict(self) -> dict[str, Any]:
        return {
            "controller": self.controller.to_dict(),
            "report": self.report.to_dict(),
            "persisted": self.persisted,
            "persistence_error": None if self.persistence_error is None else dict(self.persistence_error),
        }


@dataclass(frozen=True, slots=True)
class AutonomousBrainTraceRegistryCompactRun:
    controller: AutonomousBrainTraceRegistryControllerProjection
    evicted_run_ids: tuple[str, ...]
    persisted: bool
    persistence_error: Mapping[str, str] | None

    def to_dict(self) -> dict[str, Any]:
        return {
            "controller": self.controller.to_dict(),
            "evicted_run_ids": list(self.evicted_run_ids),
            "persisted": self.persisted,
            "persistence_error": None if self.persistence_error is None else dict(self.persistence_error),
        }


@dataclass(frozen=True, slots=True)
class AutonomousBrainTraceRegistryIntegrity:
    verified: bool
    runs: int
    events: int
    retained_event_count: int
    snapshot_digest: str

    def __post_init__(self) -> None:
        if self.verified is not True:
            raise ArgumentError("autonomous brain trace registry integrity must be verified")

    def to_dict(self) -> dict[str, Any]:
        return {
            "verified": self.verified,
            "runs": self.runs,
            "events": self.events,
            "retained_event_count": self.retained_event_count,
            "snapshot_digest": self.snapshot_digest,
            "retention": AUTONOMOUS_RUN_TRACE_REGISTRY_RETENTION,
            "authority": AUTONOMOUS_RUN_TRACE_REGISTRY_AUTHORITY,
            "secret_material": AUTONOMOUS_RUN_TRACE_REGISTRY_SECRET_MATERIAL,
        }


class _SnapshotSource:
    def __init__(self, snapshot: Any) -> None:
        self._snapshot = snapshot

    def snapshot(self) -> Any:
        return self._snapshot


class AutonomousRunTraceRegistryController:
    """Restore-safe, serialized application boundary for the trace registry."""

    def __init__(self, agent: Any, registry: AutonomousRunTraceRegistry, persistence: JsonAutonomousRunTraceRegistryPersistence) -> None:
        if agent is None:
            raise ArgumentError("autonomous trace registry controller requires an AutonomousAgent")
        if not isinstance(registry, AutonomousRunTraceRegistry):
            raise ArgumentError("autonomous trace registry controller requires an AutonomousRunTraceRegistry")
        if not isinstance(persistence, JsonAutonomousRunTraceRegistryPersistence):
            raise ArgumentError("autonomous trace registry controller requires JSON registry persistence")
        self.agent = agent
        self.registry = registry
        self.persistence = persistence
        self._coordinator = AutonomousRunTraceRegistryPersistenceCoordinator(registry, persistence)
        self._lock = RLock()
        self._busy = False
        self._restored = False
        self._persisted = False

    @contextmanager
    def _operation(self, *, require_restored: bool = True) -> Iterator[None]:
        with self._lock:
            if self._busy:
                raise ArgumentError("autonomous trace registry controller already has an operation in progress")
            if require_restored and not self._restored:
                raise ArgumentError("autonomous trace registry controller must restore before use")
            self._busy = True
            try:
                yield
            finally:
                self._busy = False

    def _projection(self, status: str) -> AutonomousBrainTraceRegistryControllerProjection:
        snapshot = self.registry.snapshot()
        return AutonomousBrainTraceRegistryControllerProjection(
            schema=AUTONOMOUS_BRAIN_TRACE_REGISTRY_CONTROLLER_SCHEMA,
            status=status,
            snapshot_generation=snapshot.snapshot_generation,
            snapshot_digest=snapshot.snapshot_digest,
            runs=snapshot.record_count,
            events=snapshot.event_count,
            retained_event_count=snapshot.retained_event_count,
            policy=snapshot.policy.to_dict(),
            persisted=self._persisted,
            retention=AUTONOMOUS_RUN_TRACE_REGISTRY_RETENTION,
            authority=AUTONOMOUS_RUN_TRACE_REGISTRY_AUTHORITY,
            secret_material=AUTONOMOUS_RUN_TRACE_REGISTRY_SECRET_MATERIAL,
        )

    def restore(self) -> AutonomousBrainTraceRegistryControllerProjection:
        with self._operation(require_restored=False):
            snapshot = self._coordinator.restore()
            self._restored = True
            self._persisted = snapshot is not None
            return self._projection("restored" if snapshot is not None else "empty")

    def flush(self) -> AutonomousBrainTraceRegistryControllerProjection:
        with self._operation():
            self._coordinator.flush()
            self._persisted = True
            return self._projection("flushed")

    def _publish(self, source: Any, run_id: str) -> AutonomousBrainTraceRegistryPublicationRun:
        publication = publish_autonomous_run_trace_registry_snapshot(self.registry, source, run_id)
        if publication.status == "failed":
            return AutonomousBrainTraceRegistryPublicationRun(self._projection("publication_failed"), publication, self._persisted, None)
        self._persisted = False
        try:
            self._coordinator.flush()
        except Exception as error:
            return AutonomousBrainTraceRegistryPublicationRun(self._projection("persistence_failed"), publication, False, _error_projection(error))
        self._persisted = True
        return AutonomousBrainTraceRegistryPublicationRun(self._projection("published"), publication, True, None)

    def publish(self, trace_store: AutonomousRunTraceStore, run_id: str) -> AutonomousBrainTraceRegistryPublicationRun:
        with self._operation():
            if not callable(getattr(trace_store, "snapshot", None)):
                raise ArgumentError("autonomous trace registry publication requires a trace store")
            return self._publish(trace_store, run_id)

    def publish_snapshot(self, snapshot: Any, run_id: str) -> AutonomousBrainTraceRegistryPublicationRun:
        """Publish one already-captured snapshot without a second source-journal read."""

        with self._operation():
            return self._publish(_SnapshotSource(snapshot), run_id)

    def import_snapshot(self, raw: Mapping[str, Any]) -> AutonomousBrainTraceRegistryImportRun:
        with self._operation():
            report = self.registry.import_snapshot(raw)
            self._persisted = False
            try:
                self._coordinator.flush()
            except Exception as error:
                return AutonomousBrainTraceRegistryImportRun(self._projection("persistence_failed"), report, False, _error_projection(error))
            self._persisted = True
            return AutonomousBrainTraceRegistryImportRun(self._projection("published"), report, True, None)

    def compact(self) -> AutonomousBrainTraceRegistryCompactRun:
        with self._operation():
            evicted, _snapshot = self.registry.compact()
            self._persisted = False
            try:
                self._coordinator.flush()
            except Exception as error:
                return AutonomousBrainTraceRegistryCompactRun(self._projection("persistence_failed"), evicted, False, _error_projection(error))
            self._persisted = True
            return AutonomousBrainTraceRegistryCompactRun(self._projection("compacted"), evicted, True, None)

    def get(self, run_id: str) -> AutonomousRunTraceRegistryRecord | None:
        with self._operation():
            return self.registry.get(run_id)

    def query(self, query: Mapping[str, Any] | None = None) -> AutonomousRunTraceRegistryPage:
        with self._operation():
            return self.registry.query(query)

    def events(self, query: Mapping[str, Any] | None = None) -> tuple[Any, ...]:
        with self._operation():
            return self.registry.events(query)

    def snapshot(self) -> AutonomousRunTraceRegistrySnapshot:
        with self._operation():
            return self.registry.snapshot()

    def verify_integrity(self) -> AutonomousBrainTraceRegistryIntegrity:
        with self._operation():
            value = self.registry.verify_integrity()
            return AutonomousBrainTraceRegistryIntegrity(
                verified=value["verified"],
                runs=value["runs"],
                events=value["events"],
                retained_event_count=value["retained_event_count"],
                snapshot_digest=value["snapshot_digest"],
            )


__all__ = [
    "AUTONOMOUS_BRAIN_TRACE_REGISTRY_CONTROLLER_SCHEMA",
    "AUTONOMOUS_BRAIN_TRACE_REGISTRY_CONTROLLER_STATUSES",
    "AutonomousBrainTraceRegistryControllerProjection",
    "AutonomousBrainTraceRegistryPublicationRun",
    "AutonomousBrainTraceRegistryImportRun",
    "AutonomousBrainTraceRegistryCompactRun",
    "AutonomousBrainTraceRegistryIntegrity",
    "AutonomousRunTraceRegistryController",
]
