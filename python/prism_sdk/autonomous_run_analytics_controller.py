"""Application lifecycle around the metadata-only autonomous run analytics ledger.

The lower-level analytics function and ledger deliberately have no execution authority.  This
controller adds the deployment-facing lifecycle: restore-before-read, a re-entrancy fence,
canonical JSON/CAS persistence, and explicit separation between in-memory report acceptance and
durable persistence.  It never stores the source trace, task text, prompts, responses, evidence,
tool payloads, credentials, or cost claims.
"""

from __future__ import annotations

from contextlib import contextmanager
from dataclasses import dataclass
from threading import RLock
from typing import Any, Iterator, Mapping

from .authoring import content_digest
from .autonomous_run_analytics import (
    AutonomousRunTraceAnalyticsPolicy,
    AutonomousRunTraceAnalyticsReport,
    analyze_autonomous_run_trace,
    validate_autonomous_run_trace_analytics_report,
)
from .autonomous_authorization import AutonomousAuthorizationContext
from .autonomous_run_analytics_ledger import (
    AUTONOMOUS_RUN_ANALYTICS_LEDGER_AUTHORITY,
    AUTONOMOUS_RUN_ANALYTICS_LEDGER_RETENTION,
    AutonomousRunAnalyticsLedger,
    AutonomousRunAnalyticsLedgerEntry,
    AutonomousRunAnalyticsLedgerIngestResult,
    AutonomousRunAnalyticsLedgerPersistenceCoordinator,
    AutonomousRunAnalyticsLedgerPolicy,
    AutonomousRunAnalyticsLedgerSummary,
    JsonAutonomousRunAnalyticsLedgerPersistence,
    validate_autonomous_run_analytics_ledger_snapshot,
)
from .errors import ArgumentError


AUTONOMOUS_BRAIN_RUN_ANALYTICS_CONTROLLER_SCHEMA = "bioprism-python-autonomous-brain-run-analytics-controller/0.1"
AUTONOMOUS_BRAIN_RUN_ANALYTICS_CONTROLLER_STATUSES = (
    "empty",
    "restored",
    "flushed",
    "ingested",
    "persistence_failed",
)


def _error_projection(error: BaseException) -> dict[str, str]:
    code = getattr(error, "code", None)
    return {
        "error_class": type(error).__name__ if type(error).__name__ else "AutonomousBrainError",
        "failure_code": code if isinstance(code, str) and code else "error",
    }


@dataclass(frozen=True, slots=True)
class AutonomousBrainRunAnalyticsControllerProjection:
    schema: str
    status: str
    snapshot_generation: int
    snapshot_digest: str
    summary: AutonomousRunAnalyticsLedgerSummary
    policy: AutonomousRunAnalyticsLedgerPolicy
    persisted: bool
    retention: str
    authority: str
    secret_material: str

    def __post_init__(self) -> None:
        if self.schema != AUTONOMOUS_BRAIN_RUN_ANALYTICS_CONTROLLER_SCHEMA:
            raise ArgumentError("autonomous brain run analytics controller schema is invalid")
        if self.status not in AUTONOMOUS_BRAIN_RUN_ANALYTICS_CONTROLLER_STATUSES:
            raise ArgumentError("autonomous brain run analytics controller status is invalid")
        if not isinstance(self.snapshot_generation, int) or self.snapshot_generation < 1:
            raise ArgumentError("autonomous brain run analytics controller generation is invalid")
        if self.retention != AUTONOMOUS_RUN_ANALYTICS_LEDGER_RETENTION:
            raise ArgumentError("autonomous brain run analytics controller retention is invalid")
        if self.authority != AUTONOMOUS_RUN_ANALYTICS_LEDGER_AUTHORITY or self.secret_material != "never_returned":
            raise ArgumentError("autonomous brain run analytics controller authority markers are invalid")

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": self.schema,
            "status": self.status,
            "snapshot_generation": self.snapshot_generation,
            "snapshot_digest": self.snapshot_digest,
            "summary": self.summary.to_dict(),
            "policy": self.policy.to_dict(),
            "persisted": self.persisted,
            "retention": self.retention,
            "authority": self.authority,
            "secret_material": self.secret_material,
        }


@dataclass(frozen=True, slots=True)
class AutonomousBrainRunAnalyticsIngestRun:
    controller: AutonomousBrainRunAnalyticsControllerProjection
    ingest: AutonomousRunAnalyticsLedgerIngestResult
    persisted: bool
    persistence_error: Mapping[str, str] | None

    def to_dict(self) -> dict[str, Any]:
        return {
            "controller": self.controller.to_dict(),
            "ingest": self.ingest.to_dict(),
            "persisted": self.persisted,
            "persistence_error": None if self.persistence_error is None else dict(self.persistence_error),
        }


@dataclass(frozen=True, slots=True)
class AutonomousBrainRunAnalyticsAnalysisRun:
    controller: AutonomousBrainRunAnalyticsControllerProjection
    report: AutonomousRunTraceAnalyticsReport
    ingest: AutonomousRunAnalyticsLedgerIngestResult
    persisted: bool
    persistence_error: Mapping[str, str] | None

    def to_dict(self) -> dict[str, Any]:
        return {
            "controller": self.controller.to_dict(),
            "report": self.report.to_dict(),
            "ingest": self.ingest.to_dict(),
            "persisted": self.persisted,
            "persistence_error": None if self.persistence_error is None else dict(self.persistence_error),
        }


@dataclass(frozen=True, slots=True)
class AutonomousBrainRunAnalyticsIntegrity:
    verified: bool
    snapshot_generation: int
    snapshot_digest: str
    summary_digest: str
    report_count: int
    retention: str
    authority: str
    secret_material: str

    def __post_init__(self) -> None:
        if self.verified is not True:
            raise ArgumentError("autonomous brain run analytics integrity must be verified")
        if self.retention != AUTONOMOUS_RUN_ANALYTICS_LEDGER_RETENTION or self.authority != AUTONOMOUS_RUN_ANALYTICS_LEDGER_AUTHORITY or self.secret_material != "never_returned":
            raise ArgumentError("autonomous brain run analytics integrity markers are invalid")

    def to_dict(self) -> dict[str, Any]:
        return {
            "verified": self.verified,
            "snapshot_generation": self.snapshot_generation,
            "snapshot_digest": self.snapshot_digest,
            "summary_digest": self.summary_digest,
            "report_count": self.report_count,
            "retention": self.retention,
            "authority": self.authority,
            "secret_material": self.secret_material,
        }


class AutonomousRunAnalyticsController:
    """Restore-safe, serialized application boundary for longitudinal analytics."""

    def __init__(
        self,
        agent: Any,
        ledger: AutonomousRunAnalyticsLedger,
        persistence: JsonAutonomousRunAnalyticsLedgerPersistence,
        authorization_context: AutonomousAuthorizationContext | None = None,
    ) -> None:
        if agent is None or not callable(getattr(agent, "analyze_run_trace", None)):
            raise ArgumentError("autonomous run analytics controller requires an AutonomousAgent")
        if not isinstance(ledger, AutonomousRunAnalyticsLedger):
            raise ArgumentError("autonomous run analytics controller requires an AutonomousRunAnalyticsLedger")
        if not isinstance(persistence, JsonAutonomousRunAnalyticsLedgerPersistence):
            raise ArgumentError("autonomous run analytics controller requires JSON analytics ledger persistence")
        self.agent = agent
        self.ledger = ledger
        self.persistence = persistence
        self.authorization_context = authorization_context
        self._coordinator = AutonomousRunAnalyticsLedgerPersistenceCoordinator(ledger, persistence)
        self._lock = RLock()
        self._busy = False
        self._restored = False
        self._persisted = False

    @contextmanager
    def _operation(self, *, require_restored: bool = True) -> Iterator[None]:
        with self._lock:
            if self._busy:
                raise ArgumentError("autonomous run analytics controller already has an operation in progress")
            if require_restored and not self._restored:
                raise ArgumentError("autonomous run analytics controller must restore before use")
            self._busy = True
            try:
                yield
            finally:
                self._busy = False

    def _projection(self, status: str) -> AutonomousBrainRunAnalyticsControllerProjection:
        snapshot = self.ledger.snapshot()
        summary = self.ledger.summary()
        return AutonomousBrainRunAnalyticsControllerProjection(
            schema=AUTONOMOUS_BRAIN_RUN_ANALYTICS_CONTROLLER_SCHEMA,
            status=status,
            snapshot_generation=snapshot["generation"],
            snapshot_digest=snapshot["snapshot_digest"],
            summary=summary,
            policy=self.ledger.policy,
            persisted=self._persisted,
            retention=summary.retention,
            authority=summary.authority,
            secret_material=summary.secret_material,
        )

    def restore(self) -> AutonomousBrainRunAnalyticsControllerProjection:
        with self._operation(require_restored=False):
            snapshot = self._coordinator.restore()
            self._restored = True
            self._persisted = snapshot is not None
            return self._projection("restored" if snapshot is not None else "empty")

    def flush(self) -> AutonomousBrainRunAnalyticsControllerProjection:
        with self._operation():
            self._persisted = False
            self._coordinator.flush()
            self._persisted = True
            return self._projection("flushed")

    def _ingest_report(self, raw: Mapping[str, Any] | AutonomousRunTraceAnalyticsReport, *, ingested_at: int | None = None) -> AutonomousBrainRunAnalyticsIngestRun:
        report = validate_autonomous_run_trace_analytics_report(raw)
        if self.authorization_context is not None:
            report_domains = tuple(
                row.identity for row in report.domains if row.kind == "domain"
            )
            authorization_kwargs: dict[str, Any] = {
                "operation": "analytics_write",
                "resource_digest": content_digest({
                    "schema": "bioprism-autonomous-analytics-authorization-resource/0.1",
                    "source_snapshot_digest": report.source_snapshot_digest,
                    "policy_digest": report.policy_digest,
                    "report_digest": report.report_digest,
                }),
            }
            if len(report_domains) == 1:
                authorization_kwargs["domain"] = report_domains[0]
            else:
                authorization_kwargs["domains"] = report_domains
            self.authorization_context.authorize_operation(**authorization_kwargs)
        ingest = self.ledger.ingest(report, ingested_at=ingested_at)
        if ingest.status != "accepted":
            return AutonomousBrainRunAnalyticsIngestRun(self._projection("ingested"), ingest, self._persisted, None)
        self._persisted = False
        try:
            self._coordinator.flush()
        except Exception as error:
            return AutonomousBrainRunAnalyticsIngestRun(self._projection("persistence_failed"), ingest, False, _error_projection(error))
        self._persisted = True
        return AutonomousBrainRunAnalyticsIngestRun(self._projection("ingested"), ingest, True, None)

    def ingest(self, report: Mapping[str, Any] | AutonomousRunTraceAnalyticsReport, *, ingested_at: int | None = None) -> AutonomousBrainRunAnalyticsIngestRun:
        with self._operation():
            return self._ingest_report(report, ingested_at=ingested_at)

    def analyze_and_ingest(
        self,
        snapshot: Mapping[str, Any] | Any,
        policy: AutonomousRunTraceAnalyticsPolicy | Mapping[str, Any] | None = None,
        *,
        ingested_at: int | None = None,
    ) -> AutonomousBrainRunAnalyticsAnalysisRun:
        with self._operation():
            if isinstance(policy, Mapping):
                normalized_policy = AutonomousRunTraceAnalyticsPolicy().to_dict()
                normalized_policy.update(policy)
                policy = AutonomousRunTraceAnalyticsPolicy.from_dict(normalized_policy)
            report = analyze_autonomous_run_trace(snapshot, policy)
            outcome = self._ingest_report(report, ingested_at=ingested_at)
            return AutonomousBrainRunAnalyticsAnalysisRun(
                controller=outcome.controller,
                report=report,
                ingest=outcome.ingest,
                persisted=outcome.persisted,
                persistence_error=outcome.persistence_error,
            )

    def summary(self) -> AutonomousRunAnalyticsLedgerSummary:
        with self._operation():
            return self.ledger.summary()

    def entries(self) -> tuple[AutonomousRunAnalyticsLedgerEntry, ...]:
        with self._operation():
            return self.ledger.entries

    def history(self, *, limit: int = 100, status: str | None = None) -> tuple[AutonomousRunAnalyticsLedgerEntry, ...]:
        with self._operation():
            return self.ledger.history(limit=limit, status=status)

    def snapshot(self) -> dict[str, Any]:
        with self._operation():
            return self.ledger.snapshot()

    def verify_integrity(self) -> AutonomousBrainRunAnalyticsIntegrity:
        with self._operation():
            snapshot = validate_autonomous_run_analytics_ledger_snapshot(self.ledger.snapshot())
            summary = self.ledger.summary()
            return AutonomousBrainRunAnalyticsIntegrity(
                verified=True,
                snapshot_generation=snapshot["generation"],
                snapshot_digest=snapshot["snapshot_digest"],
                summary_digest=summary.summary_digest,
                report_count=summary.report_count,
                retention=summary.retention,
                authority=summary.authority,
                secret_material=summary.secret_material,
            )


__all__ = [
    "AUTONOMOUS_BRAIN_RUN_ANALYTICS_CONTROLLER_SCHEMA",
    "AUTONOMOUS_BRAIN_RUN_ANALYTICS_CONTROLLER_STATUSES",
    "AutonomousBrainRunAnalyticsControllerProjection",
    "AutonomousBrainRunAnalyticsIngestRun",
    "AutonomousBrainRunAnalyticsAnalysisRun",
    "AutonomousBrainRunAnalyticsIntegrity",
    "AutonomousRunAnalyticsController",
]
