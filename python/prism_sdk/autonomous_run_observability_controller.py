"""Coordinated trace publication and longitudinal analytics for application workers.

This controller is intentionally a composition boundary rather than another execution engine.
It reads a source trace once, gives that exact snapshot to the registry and analytics controllers,
and returns independent, redacted outcomes. A persistence failure is never converted into a
provider retry or a replay of a task that may already have caused an external effect.
"""

from __future__ import annotations

from contextlib import contextmanager
from dataclasses import dataclass
from threading import RLock
from typing import Any, Iterator, Mapping

from .authoring import content_digest
from .autonomous_run_analytics import (
    AUTONOMOUS_RUN_TRACE_ANALYTICS_RETENTION,
    AutonomousRunTraceAnalyticsPolicy,
)
from .autonomous_run_analytics_controller import (
    AutonomousBrainRunAnalyticsAnalysisRun,
    AutonomousBrainRunAnalyticsControllerProjection,
    AutonomousRunAnalyticsController,
)
from .autonomous_run_trace_registry_controller import (
    AutonomousBrainTraceRegistryControllerProjection,
    AutonomousBrainTraceRegistryPublicationRun,
    AutonomousRunTraceRegistryController,
)
from .errors import ArgumentError


AUTONOMOUS_BRAIN_RUN_OBSERVABILITY_CONTROLLER_SCHEMA = "bioprism-python-autonomous-brain-run-observability-controller/0.1"
AUTONOMOUS_BRAIN_RUN_OBSERVABILITY_ALERT_SCHEMA = "bioprism-python-autonomous-brain-run-observability-alert/0.1"
AUTONOMOUS_BRAIN_RUN_OBSERVABILITY_CONTROLLER_STATUSES = (
    "empty",
    "restored",
    "flushed",
    "published_and_analyzed",
    "source_snapshot_failed",
    "trace_publication_failed",
    "analytics_failed",
    "alert_delivery_failed",
    "persistence_partial",
)


def _error_projection(error: BaseException) -> dict[str, str]:
    code = getattr(error, "code", None)
    return {
        "error_class": type(error).__name__ if type(error).__name__ else "AutonomousBrainError",
        "failure_code": code if isinstance(code, str) and code else "error",
    }


@dataclass(frozen=True, slots=True)
class AutonomousBrainRunObservabilityControllerProjection:
    schema: str
    status: str
    ready: bool
    persisted: bool
    trace_registry: AutonomousBrainTraceRegistryControllerProjection | None
    run_analytics: AutonomousBrainRunAnalyticsControllerProjection | None
    last_run_id: str | None
    last_source_snapshot_digest: str | None

    def __post_init__(self) -> None:
        if self.schema != AUTONOMOUS_BRAIN_RUN_OBSERVABILITY_CONTROLLER_SCHEMA:
            raise ArgumentError("autonomous brain run observability controller schema is invalid")
        if self.status not in AUTONOMOUS_BRAIN_RUN_OBSERVABILITY_CONTROLLER_STATUSES:
            raise ArgumentError("autonomous brain run observability controller status is invalid")

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": self.schema,
            "status": self.status,
            "ready": self.ready,
            "persisted": self.persisted,
            "trace_registry": None if self.trace_registry is None else self.trace_registry.to_dict(),
            "run_analytics": None if self.run_analytics is None else self.run_analytics.to_dict(),
            "last_run_id": self.last_run_id,
            "last_source_snapshot_digest": self.last_source_snapshot_digest,
        }


@dataclass(frozen=True, slots=True)
class AutonomousBrainRunObservabilityAlert:
    schema: str
    alert_id: str
    source_snapshot_digest: str
    report_digest: str
    code: str
    severity: str
    scope: str
    identity: str
    detail: str
    observed_value: float | int | None
    threshold: float | int | None
    retention: str
    secret_material: str

    def __post_init__(self) -> None:
        if self.schema != AUTONOMOUS_BRAIN_RUN_OBSERVABILITY_ALERT_SCHEMA:
            raise ArgumentError("autonomous brain run observability alert schema is invalid")
        for name, value in (("alert_id", self.alert_id), ("source_snapshot_digest", self.source_snapshot_digest), ("report_digest", self.report_digest)):
            if not isinstance(value, str) or len(value) != 64 or any(character not in "0123456789abcdef" for character in value):
                raise ArgumentError(f"autonomous brain run observability alert {name} is invalid")
        if self.retention != AUTONOMOUS_RUN_TRACE_ANALYTICS_RETENTION or self.secret_material != "never_returned":
            raise ArgumentError("autonomous brain run observability alert retention markers are invalid")

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": self.schema,
            "alert_id": self.alert_id,
            "source_snapshot_digest": self.source_snapshot_digest,
            "report_digest": self.report_digest,
            "code": self.code,
            "severity": self.severity,
            "scope": self.scope,
            "identity": self.identity,
            "detail": self.detail,
            "observed_value": self.observed_value,
            "threshold": self.threshold,
            "retention": self.retention,
            "secret_material": self.secret_material,
        }


@dataclass(frozen=True, slots=True)
class AutonomousBrainRunObservabilityAlertDelivery:
    status: str
    attempted: int
    delivered: int
    error: Mapping[str, str] | None

    def __post_init__(self) -> None:
        if self.status not in {"not_configured", "not_needed", "delivered", "failed"}:
            raise ArgumentError("autonomous brain run observability alert delivery status is invalid")
        if isinstance(self.attempted, bool) or not isinstance(self.attempted, int) or self.attempted < 0 or isinstance(self.delivered, bool) or not isinstance(self.delivered, int) or not 0 <= self.delivered <= self.attempted:
            raise ArgumentError("autonomous brain run observability alert delivery counts are invalid")

    def to_dict(self) -> dict[str, Any]:
        return {
            "status": self.status,
            "attempted": self.attempted,
            "delivered": self.delivered,
            "error": None if self.error is None else dict(self.error),
        }


@dataclass(frozen=True, slots=True)
class AutonomousBrainRunObservabilityRestoreRun:
    controller: AutonomousBrainRunObservabilityControllerProjection

    def to_dict(self) -> dict[str, Any]:
        return {"controller": self.controller.to_dict()}


@dataclass(frozen=True, slots=True)
class AutonomousBrainRunObservabilityFlushRun:
    controller: AutonomousBrainRunObservabilityControllerProjection
    persisted: bool
    persistence_errors: tuple[Mapping[str, str], ...]

    def to_dict(self) -> dict[str, Any]:
        return {
            "controller": self.controller.to_dict(),
            "persisted": self.persisted,
            "persistence_errors": [dict(error) for error in self.persistence_errors],
        }


@dataclass(frozen=True, slots=True)
class AutonomousBrainRunObservabilityRun:
    controller: AutonomousBrainRunObservabilityControllerProjection
    run_id: str
    source_snapshot_digest: str | None
    trace_registry: AutonomousBrainTraceRegistryPublicationRun | None
    run_analytics: AutonomousBrainRunAnalyticsAnalysisRun | None
    alert_delivery: AutonomousBrainRunObservabilityAlertDelivery
    errors: tuple[Mapping[str, str], ...]

    def to_dict(self) -> dict[str, Any]:
        return {
            "controller": self.controller.to_dict(),
            "run_id": self.run_id,
            "source_snapshot_digest": self.source_snapshot_digest,
            "trace_registry": None if self.trace_registry is None else self.trace_registry.to_dict(),
            "run_analytics": None if self.run_analytics is None else self.run_analytics.to_dict(),
            "alert_delivery": self.alert_delivery.to_dict(),
            "errors": [dict(error) for error in self.errors],
        }


class AutonomousRunObservabilityController:
    """Restore-safe coordinator for the two metadata-only run projections."""

    def __init__(
        self,
        agent: Any,
        trace_registry: AutonomousRunTraceRegistryController,
        run_analytics: AutonomousRunAnalyticsController,
        alert_sink: Any | None = None,
    ) -> None:
        if agent is None:
            raise ArgumentError("autonomous run observability controller requires an AutonomousAgent")
        if not isinstance(trace_registry, AutonomousRunTraceRegistryController) or not isinstance(run_analytics, AutonomousRunAnalyticsController):
            raise ArgumentError("autonomous run observability controller requires metadata controllers")
        if trace_registry.agent is not agent or run_analytics.agent is not agent:
            raise ArgumentError("autonomous run observability controllers must belong to the same agent")
        if alert_sink is not None and not callable(getattr(alert_sink, "publish", None)):
            raise ArgumentError("autonomous run observability alert sink is malformed")
        self.agent = agent
        self.trace_registry = trace_registry
        self.run_analytics = run_analytics
        self.alert_sink = alert_sink
        self._lock = RLock()
        self._busy = False
        self._restored = False
        self._persisted = False
        self._last_run_id: str | None = None
        self._last_source_snapshot_digest: str | None = None
        self._last_trace_projection: AutonomousBrainTraceRegistryControllerProjection | None = None
        self._last_analytics_projection: AutonomousBrainRunAnalyticsControllerProjection | None = None

    @contextmanager
    def _operation(self, *, require_restored: bool = True) -> Iterator[None]:
        with self._lock:
            if self._busy:
                raise ArgumentError("autonomous run observability controller already has an operation in progress")
            if require_restored and not self._restored:
                raise ArgumentError("autonomous run observability controller must restore before use")
            self._busy = True
            try:
                yield
            finally:
                self._busy = False

    def _projection(
        self,
        status: str,
        trace_registry: AutonomousBrainTraceRegistryControllerProjection | None = None,
        run_analytics: AutonomousBrainRunAnalyticsControllerProjection | None = None,
    ) -> AutonomousBrainRunObservabilityControllerProjection:
        return AutonomousBrainRunObservabilityControllerProjection(
            schema=AUTONOMOUS_BRAIN_RUN_OBSERVABILITY_CONTROLLER_SCHEMA,
            status=status,
            ready=self._restored,
            persisted=self._persisted,
            trace_registry=self._last_trace_projection if trace_registry is None else trace_registry,
            run_analytics=self._last_analytics_projection if run_analytics is None else run_analytics,
            last_run_id=self._last_run_id,
            last_source_snapshot_digest=self._last_source_snapshot_digest,
        )

    def restore(self) -> AutonomousBrainRunObservabilityRestoreRun:
        with self._operation(require_restored=False):
            self._restored = False
            trace_registry = self.trace_registry.restore()
            run_analytics = self.run_analytics.restore()
            self._last_trace_projection = trace_registry
            self._last_analytics_projection = run_analytics
            self._persisted = trace_registry.persisted and run_analytics.persisted
            self._restored = True
            status = "empty" if trace_registry.status == "empty" and run_analytics.status == "empty" else "restored"
            return AutonomousBrainRunObservabilityRestoreRun(self._projection(status, trace_registry, run_analytics))

    def flush(self) -> AutonomousBrainRunObservabilityFlushRun:
        with self._operation():
            errors: list[Mapping[str, str]] = []
            trace_registry = self._last_trace_projection
            run_analytics = self._last_analytics_projection
            try:
                trace_registry = self.trace_registry.flush()
                self._last_trace_projection = trace_registry
            except Exception as error:
                errors.append({"scope": "trace_registry", **_error_projection(error)})
            try:
                run_analytics = self.run_analytics.flush()
                self._last_analytics_projection = run_analytics
            except Exception as error:
                errors.append({"scope": "run_analytics", **_error_projection(error)})
            self._persisted = not errors
            return AutonomousBrainRunObservabilityFlushRun(
                self._projection("flushed" if not errors else "persistence_partial", trace_registry, run_analytics),
                self._persisted,
                tuple(errors),
            )

    def _deliver_alerts(self, report: Any | None) -> AutonomousBrainRunObservabilityAlertDelivery:
        if self.alert_sink is None:
            return AutonomousBrainRunObservabilityAlertDelivery("not_configured", 0, 0, None)
        if report is None or not report.alerts:
            return AutonomousBrainRunObservabilityAlertDelivery("not_needed", 0, 0, None)
        events: list[AutonomousBrainRunObservabilityAlert] = []
        for alert in report.alerts:
            identity = {
                "source_snapshot_digest": report.source_snapshot_digest,
                "report_digest": report.report_digest,
                "code": alert.code,
                "severity": alert.severity,
                "scope": alert.scope,
                "identity": alert.identity,
            }
            events.append(AutonomousBrainRunObservabilityAlert(
                schema=AUTONOMOUS_BRAIN_RUN_OBSERVABILITY_ALERT_SCHEMA,
                alert_id=content_digest(identity),
                source_snapshot_digest=report.source_snapshot_digest,
                report_digest=report.report_digest,
                code=alert.code,
                severity=alert.severity,
                scope=alert.scope,
                identity=alert.identity,
                detail=alert.detail,
                observed_value=alert.observed_value,
                threshold=alert.threshold,
                retention=AUTONOMOUS_RUN_TRACE_ANALYTICS_RETENTION,
                secret_material="never_returned",
            ))
        try:
            self.alert_sink.publish(tuple(events))
        except Exception as error:
            return AutonomousBrainRunObservabilityAlertDelivery("failed", len(events), 0, _error_projection(error))
        return AutonomousBrainRunObservabilityAlertDelivery("delivered", len(events), len(events), None)

    def publish_and_analyze(
        self,
        trace_store: Any,
        run_id: str,
        policy: AutonomousRunTraceAnalyticsPolicy | Mapping[str, Any] | None = None,
        *,
        ingested_at: int | None = None,
    ) -> AutonomousBrainRunObservabilityRun:
        with self._operation():
            if not callable(getattr(trace_store, "snapshot", None)):
                raise ArgumentError("autonomous run observability publication requires a trace store")
            if not isinstance(run_id, str) or not 1 <= len(run_id) <= 256 or any(character not in "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_.:-" for character in run_id):
                raise ArgumentError("autonomous run observability run_id must be a bounded identifier")
            errors: list[Mapping[str, str]] = []
            source_snapshot_digest: str | None = None
            trace_outcome: AutonomousBrainTraceRegistryPublicationRun | None = None
            analytics_outcome: AutonomousBrainRunAnalyticsAnalysisRun | None = None
            try:
                source = trace_store.snapshot()
                candidate = getattr(source, "snapshot_digest", None)
                if candidate is None and isinstance(source, Mapping):
                    candidate = source.get("snapshot_digest")
                if isinstance(candidate, str) and len(candidate) == 64 and all(character in "0123456789abcdef" for character in candidate):
                    source_snapshot_digest = candidate
            except Exception as error:
                errors.append({"scope": "source_snapshot", **_error_projection(error)})
                return AutonomousBrainRunObservabilityRun(
                    self._projection("source_snapshot_failed"), run_id, source_snapshot_digest, None, None,
                    AutonomousBrainRunObservabilityAlertDelivery("not_needed", 0, 0, None), tuple(errors)
                )

            trace_outcome = self.trace_registry.publish_snapshot(source, run_id)
            self._last_trace_projection = trace_outcome.controller
            if trace_outcome.publication.status == "failed":
                errors.append({
                    "scope": "trace_publication",
                    "error_class": trace_outcome.publication.error_class or "AutonomousRunTraceRegistryPublicationError",
                    "failure_code": trace_outcome.publication.failure_code or "trace_registry_publication_failed",
                })
            elif trace_outcome.persistence_error is not None:
                errors.append({"scope": "trace_persistence", **dict(trace_outcome.persistence_error)})

            try:
                analytics_outcome = self.run_analytics.analyze_and_ingest(source, policy, ingested_at=ingested_at)
                self._last_analytics_projection = analytics_outcome.controller
                if analytics_outcome.persistence_error is not None:
                    errors.append({"scope": "analytics_persistence", **dict(analytics_outcome.persistence_error)})
            except Exception as error:
                errors.append({"scope": "analytics", **_error_projection(error)})

            alert_delivery = self._deliver_alerts(analytics_outcome.report if analytics_outcome is not None else None)
            if alert_delivery.error is not None:
                errors.append({"scope": "alert_delivery", **dict(alert_delivery.error)})
            self._last_run_id = run_id
            self._last_source_snapshot_digest = source_snapshot_digest
            self._persisted = trace_outcome.persisted and analytics_outcome is not None and analytics_outcome.persisted
            if trace_outcome.publication.status == "failed":
                status = "trace_publication_failed"
            elif analytics_outcome is None:
                status = "analytics_failed"
            elif alert_delivery.status == "failed":
                status = "alert_delivery_failed"
            elif errors and (not trace_outcome.persisted or not analytics_outcome.persisted):
                status = "persistence_partial"
            elif errors:
                status = "analytics_failed"
            else:
                status = "published_and_analyzed"
            return AutonomousBrainRunObservabilityRun(
                self._projection(status, trace_outcome.controller, analytics_outcome.controller if analytics_outcome is not None else None),
                run_id,
                source_snapshot_digest,
                trace_outcome,
                analytics_outcome,
                alert_delivery,
                tuple(errors),
            )

    def verify_integrity(self) -> dict[str, Any]:
        with self._operation():
            return {
                "verified": True,
                "trace_registry": self.trace_registry.verify_integrity().to_dict(),
                "run_analytics": self.run_analytics.verify_integrity().to_dict(),
            }


__all__ = [
    "AUTONOMOUS_BRAIN_RUN_OBSERVABILITY_CONTROLLER_SCHEMA",
    "AUTONOMOUS_BRAIN_RUN_OBSERVABILITY_ALERT_SCHEMA",
    "AUTONOMOUS_BRAIN_RUN_OBSERVABILITY_CONTROLLER_STATUSES",
    "AutonomousBrainRunObservabilityControllerProjection",
    "AutonomousBrainRunObservabilityAlert",
    "AutonomousBrainRunObservabilityAlertDelivery",
    "AutonomousBrainRunObservabilityRestoreRun",
    "AutonomousBrainRunObservabilityFlushRun",
    "AutonomousBrainRunObservabilityRun",
    "AutonomousRunObservabilityController",
]
