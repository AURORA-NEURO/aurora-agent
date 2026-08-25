"""Ordered, metadata-only recovery for long-horizon autonomous goals.

The worker journal and control-loop checkpoint are useful independently, but restoring them in
the wrong order can admit a new cycle while an older provider/effect invocation is uncertain.
This module composes the two caller-owned persistence coordinators: journal restoration happens
first, active boundaries are reconciled and flushed, and only then is the loop checkpoint exposed
for resume. Task text, prompts, parameters, credentials, provider values, evaluator payloads,
and results never enter the report.
"""

from __future__ import annotations

from collections.abc import Mapping
import copy
import re
from typing import Any, Literal

from .authoring import canonical_json, content_digest
from .autonomous_goal_control_loop import AutonomousGoalControlLoop, AutonomousGoalControlLoopResult
from .autonomous_goal_control_persistence import (
    AutonomousGoalControlLoopPersistenceCoordinator,
    validate_autonomous_goal_control_loop_snapshot,
)
from .autonomous_goal_worker_journal import (
    AutonomousGoalWorkerJournalPersistenceCoordinator,
)
from .goals import AutonomousGoalError, AutonomousGoalLedger


GOAL_RECOVERY_SCHEMA = "bioprism-autonomous-goal-recovery/0.1"
GOAL_RECOVERY_RETENTION = "metadata_only_goal_recovery;tasks_prompts_parameters_credentials_provider_values_and_results_not_retained"
MAX_GOAL_RECOVERY_GOALS = 16_384
MAX_GOAL_RECOVERY_REPORT_BYTES = 2_000_000
_DIGEST = re.compile(r"^[0-9a-f]{64}$")

RecoveryStatus = Literal["fresh", "restored", "recovered"]


def _fail(message: str) -> None:
    raise AutonomousGoalError(f"autonomous goal recovery {message}")


def _digest(value: Any, *, name: str, allow_none: bool = False) -> str | None:
    if value is None and allow_none:
        return None
    if not isinstance(value, str) or _DIGEST.fullmatch(value) is None:
        _fail(f"{name} must be a lowercase SHA-256 digest")
    return value


def _integer(value: Any, *, name: str, minimum: int = 0, maximum: int = 2**63 - 1) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or value < minimum or value > maximum:
        _fail(f"{name} is outside its integer bounds")
    return value


def _identifier(value: Any, *, name: str, maximum: int = 256) -> str:
    if not isinstance(value, str) or not value.strip() or "\x00" in value or len(value.encode("utf-8")) > maximum:
        _fail(f"{name} is outside its bounded identifier contract")
    return value.strip()


def _entry(value: Any, *, index: int) -> dict[str, Any]:
    if not isinstance(value, Mapping):
        _fail(f"recovered entry {index} is malformed")
    expected = {"goal_id", "from_phase", "goal_status", "outcome_digest"}
    if set(value) != expected:
        _fail(f"recovered entry {index} contains unsupported or missing fields")
    if value["from_phase"] not in {"claimed", "dispatch_started"}:
        _fail(f"recovered entry {index} phase is invalid")
    if value["goal_status"] not in {"paused", "blocked"}:
        _fail(f"recovered entry {index} status is invalid")
    return {
        "goal_id": _identifier(value["goal_id"], name=f"recovered entry {index} goal_id"),
        "from_phase": value["from_phase"],
        "goal_status": value["goal_status"],
        "outcome_digest": _digest(value["outcome_digest"], name=f"recovered entry {index} outcome_digest"),
    }


def _report_body(value: Any, *, require_digest: bool) -> dict[str, Any]:
    if not isinstance(value, Mapping):
        _fail("report must be a mapping")
    required = {
        "schema", "status", "active_count_before_recovery", "recovered", "recovery_digest", "journal_snapshot_digest",
        "journal_head_digest", "control_loop_snapshot_digest", "control_loop_generation", "resume_snapshot", "ready_to_resume",
        "requires_external_reconciliation", "retention", "secret_material",
    }
    allowed = required | {"report_digest"}
    if set(value).difference(allowed) or not required.issubset(value) or (require_digest and "report_digest" not in value):
        _fail("report contains unsupported or missing fields")
    if value["schema"] != GOAL_RECOVERY_SCHEMA or value["retention"] != GOAL_RECOVERY_RETENTION or value["secret_material"] != "never_returned":
        _fail("report markers are invalid")
    if value["status"] not in {"fresh", "restored", "recovered"}:
        _fail("report status is invalid")
    active_count = _integer(value["active_count_before_recovery"], name="active_count_before_recovery", maximum=MAX_GOAL_RECOVERY_GOALS)
    raw_recovered = value["recovered"]
    if not isinstance(raw_recovered, list) or len(raw_recovered) > MAX_GOAL_RECOVERY_GOALS:
        _fail("recovered entries are outside their bounds")
    recovered = [_entry(raw, index=index) for index, raw in enumerate(raw_recovered)]
    if len(recovered) != active_count:
        _fail("recovered entries do not account for every active journal boundary")
    goal_ids = [row["goal_id"] for row in recovered]
    if len(set(goal_ids)) != len(goal_ids):
        _fail("recovered entries contain duplicate goals")
    recovery_digest = _digest(value["recovery_digest"], name="recovery_digest")
    if recovery_digest != content_digest(recovered):
        _fail("recovery digest does not match recovered entries")
    journal_snapshot_digest = _digest(value["journal_snapshot_digest"], name="journal_snapshot_digest", allow_none=True)
    journal_head_digest = value["journal_head_digest"] if value["journal_head_digest"] == "" else _digest(value["journal_head_digest"], name="journal_head_digest")
    control_snapshot_digest = _digest(value["control_loop_snapshot_digest"], name="control_loop_snapshot_digest", allow_none=True)
    control_generation = _integer(value["control_loop_generation"], name="control_loop_generation", maximum=2**31 - 1)
    resume_snapshot = None if value["resume_snapshot"] is None else validate_autonomous_goal_control_loop_snapshot(value["resume_snapshot"])
    if (None if resume_snapshot is None else resume_snapshot["snapshot_digest"]) != control_snapshot_digest or (0 if resume_snapshot is None else resume_snapshot["generation"]) != control_generation:
        _fail("control-loop snapshot metadata is inconsistent")
    if value["ready_to_resume"] is not True:
        _fail("report is not ready to resume")
    if not isinstance(value["requires_external_reconciliation"], bool):
        _fail("external reconciliation marker is invalid")
    requires_reconciliation = any(row["from_phase"] == "dispatch_started" for row in recovered)
    if value["requires_external_reconciliation"] != requires_reconciliation:
        _fail("external reconciliation marker is inconsistent")
    body = {
        "schema": value["schema"],
        "status": value["status"],
        "active_count_before_recovery": active_count,
        "recovered": recovered,
        "recovery_digest": recovery_digest,
        "journal_snapshot_digest": journal_snapshot_digest,
        "journal_head_digest": journal_head_digest,
        "control_loop_snapshot_digest": control_snapshot_digest,
        "control_loop_generation": control_generation,
        "resume_snapshot": resume_snapshot,
        "ready_to_resume": value["ready_to_resume"],
        "requires_external_reconciliation": requires_reconciliation,
        "retention": value["retention"],
        "secret_material": value["secret_material"],
    }
    if require_digest and _digest(value.get("report_digest"), name="report_digest") != content_digest(body):
        _fail("report digest does not match its content")
    return {**body, "report_digest": content_digest(body)}


def validate_autonomous_goal_recovery_report(value: Mapping[str, Any]) -> dict[str, Any]:
    """Validate a sealed recovery report before it is stored or handed to a resume caller."""

    normalized = _report_body(value, require_digest=True)
    if len(canonical_json(normalized).encode("utf-8")) > MAX_GOAL_RECOVERY_REPORT_BYTES:
        _fail("report exceeds its bounded size")
    return copy.deepcopy(normalized)


class AutonomousGoalRecoveryCoordinator:
    """Compose journal recovery and loop resume with an explicit fail-closed ordering."""

    def __init__(
        self,
        ledger: AutonomousGoalLedger,
        journal: AutonomousGoalWorkerJournalPersistenceCoordinator,
        control_loop: AutonomousGoalControlLoopPersistenceCoordinator,
    ) -> None:
        if not isinstance(ledger, AutonomousGoalLedger):
            _fail("ledger must be an AutonomousGoalLedger")
        if not isinstance(journal, AutonomousGoalWorkerJournalPersistenceCoordinator):
            _fail("journal coordinator is invalid")
        if not isinstance(control_loop, AutonomousGoalControlLoopPersistenceCoordinator):
            _fail("control-loop coordinator is invalid")
        self.ledger = ledger
        self.journal = journal
        self.control_loop = control_loop
        self._report: dict[str, Any] | None = None

    @property
    def report(self) -> dict[str, Any] | None:
        return None if self._report is None else copy.deepcopy(self._report)

    def restore(self, *, now_ns: int | None = None) -> dict[str, Any]:
        if now_ns is not None:
            _integer(now_ns, name="now_ns")
        # This order is the safety boundary: reconcile and durably flush uncertain dispatches
        # before a stale control-loop snapshot can be used to select new work.
        journal_snapshot_before = self.journal.restore()
        active_before = self.journal.journal.active()
        recovered: list[dict[str, Any]] = []
        journal_snapshot = journal_snapshot_before
        if active_before:
            recovery = self.journal.journal.recover(self.ledger, now_ns=now_ns)
            raw_recovered = recovery.get("recovered")
            if not isinstance(raw_recovered, list):
                _fail("journal recovery returned malformed entries")
            recovered = [_entry(row, index=index) for index, row in enumerate(raw_recovered)]
            # A crash after this flush is safe: the next process sees only reconciled events.
            journal_snapshot = self.journal.flush()
        control_snapshot = self.control_loop.restore()
        status: RecoveryStatus = "recovered" if recovered else ("restored" if journal_snapshot_before is not None or control_snapshot is not None else "fresh")
        body = {
            "schema": GOAL_RECOVERY_SCHEMA,
            "status": status,
            "active_count_before_recovery": len(active_before),
            "recovered": recovered,
            "recovery_digest": content_digest(recovered),
            "journal_snapshot_digest": None if journal_snapshot is None else journal_snapshot["snapshot_digest"],
            "journal_head_digest": self.journal.journal.head_digest if journal_snapshot is None else journal_snapshot["head_digest"],
            "control_loop_snapshot_digest": None if control_snapshot is None else control_snapshot["snapshot_digest"],
            "control_loop_generation": 0 if control_snapshot is None else control_snapshot["generation"],
            "resume_snapshot": control_snapshot,
            "ready_to_resume": not bool(self.journal.journal.active()),
            "requires_external_reconciliation": any(row["from_phase"] == "dispatch_started" for row in recovered),
            "retention": GOAL_RECOVERY_RETENTION,
            "secret_material": "never_returned",
        }
        self._report = _report_body({**body, "report_digest": content_digest(body)}, require_digest=True)
        return self.report

    def assert_ready_for_resume(self) -> dict[str, Any]:
        if self._report is None:
            _fail("restore must complete before resume")
        if self.journal.journal.active():
            _fail("journal still contains active boundaries")
        return self.report

    def resume(self, loop: AutonomousGoalControlLoop, options: Mapping[str, Any] | None = None) -> AutonomousGoalControlLoopResult:
        if not isinstance(loop, AutonomousGoalControlLoop):
            _fail("resume requires an AutonomousGoalControlLoop")
        report = self.assert_ready_for_resume()
        safe_options = {} if options is None else dict(options)
        if "resume_snapshot" in safe_options:
            _fail("resume_snapshot is owned by the recovery coordinator")
        return loop.run(**safe_options, resume_snapshot=report["resume_snapshot"])

    def checkpoint(self, snapshot: Mapping[str, Any]) -> dict[str, Any]:
        """Persist one loop checkpoint with the worker journal durably ahead of it."""

        prior = self.assert_ready_for_resume()
        journal_snapshot = self.journal.flush()
        control_snapshot = self.control_loop.flush(snapshot)
        prior_body = {key: value for key, value in prior.items() if key != "report_digest"}
        body = {
            **prior_body,
            "journal_snapshot_digest": journal_snapshot["snapshot_digest"],
            "journal_head_digest": journal_snapshot["head_digest"],
            "control_loop_snapshot_digest": control_snapshot["snapshot_digest"],
            "control_loop_generation": control_snapshot["generation"],
            "resume_snapshot": control_snapshot,
        }
        self._report = _report_body({**body, "report_digest": content_digest(body)}, require_digest=True)
        return copy.deepcopy(control_snapshot)


__all__ = [
    "GOAL_RECOVERY_SCHEMA",
    "GOAL_RECOVERY_RETENTION",
    "MAX_GOAL_RECOVERY_GOALS",
    "MAX_GOAL_RECOVERY_REPORT_BYTES",
    "RecoveryStatus",
    "AutonomousGoalRecoveryCoordinator",
    "validate_autonomous_goal_recovery_report",
]
