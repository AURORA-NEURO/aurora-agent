"""Strict startup/shutdown composition for the autonomous brain.

Each autonomous subsystem owns its own persistence contract and compare-and-swap fence.  That
separation is intentional, but applications still need one safe lifecycle boundary: restoring
model inventory before a run, restoring evaluator and learning state before admitting feedback,
and flushing the same metadata without pretending that several independent stores are one
transaction.  This module composes those existing coordinators and keeps the cross-store
non-atomicity explicit.

The lifecycle report is metadata-only.  It retains component identifiers, schema/digest
projections, generation counters, bounded failure classes, and next-action guidance; it never
copies a snapshot, task, prompt, provider response, credential, tool argument, evidence value, or
raw exception message.
"""

from __future__ import annotations

from dataclasses import dataclass
import threading
from typing import Any, Mapping, Sequence

from .authoring import content_digest
from .errors import ArgumentError


AUTONOMOUS_AGENT_LIFECYCLE_SCHEMA = "bioprism-python-autonomous-agent-persistence-lifecycle/0.1"
AUTONOMOUS_AGENT_LIFECYCLE_COMPONENTS = (
    "model_inventory",
    "runtime_health",
    "health",
    "activation",
    "selection_promotion",
    "evaluator_calibration",
    "memory",
    "learning",
    "prompt_learning",
    "capability_journal",
    "decision_cycle",
    "execution",
)
AUTONOMOUS_AGENT_LIFECYCLE_RESTORE_ORDER = AUTONOMOUS_AGENT_LIFECYCLE_COMPONENTS
AUTONOMOUS_AGENT_LIFECYCLE_FLUSH_ORDER = tuple(reversed(AUTONOMOUS_AGENT_LIFECYCLE_COMPONENTS))
AUTONOMOUS_AGENT_LIFECYCLE_OPERATIONS = ("restore", "flush")
AUTONOMOUS_AGENT_LIFECYCLE_STATUSES = ("completed", "partial", "failed", "empty", "unconfigured")
AUTONOMOUS_AGENT_LIFECYCLE_COMPONENT_STATUSES = (
    "restored",
    "flushed",
    "empty",
    "unconfigured",
    "not_attempted",
    "failed",
)


class AutonomousAgentPersistenceLifecycleError(ArgumentError):
    """A strict lifecycle operation stopped after a component failed."""

    def __init__(self, operation: str, report: "AutonomousAgentPersistenceLifecycleReport") -> None:
        self.operation = operation
        self.report = report
        super().__init__(f"autonomous agent persistence {operation} did not complete")


def _bounded_identifier(name: str, value: Any) -> str:
    if not isinstance(value, str) or not value or len(value.encode("utf-8")) > 128:
        raise ArgumentError(f"{name} must be a bounded identifier")
    if any(character not in "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_.-" for character in value):
        raise ArgumentError(f"{name} must be a bounded identifier")
    return value


def _bounded_digest(name: str, value: Any) -> str | None:
    if value is None:
        return None
    if not isinstance(value, str) or len(value) != 64 or any(character not in "0123456789abcdef" for character in value):
        raise ArgumentError(f"{name} must be a lowercase SHA-256 digest or null")
    return value


def _error_class(value: BaseException) -> str:
    candidate = type(value).__name__
    return candidate if candidate and len(candidate) <= 128 and all(
        character in "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_.-"
        for character in candidate
    ) else "UnknownError"


def _snapshot_projection(value: Any) -> tuple[str | None, str | None, str | None, int | None]:
    """Extract only safe scalar metadata from one coordinator result."""

    if value is None:
        return None, None, None, None
    if hasattr(value, "to_dict") and callable(value.to_dict):
        value = value.to_dict()
    if not isinstance(value, Mapping):
        return None, None, None, None
    schema = value.get("schema") if isinstance(value.get("schema"), str) else None
    snapshot_digest = None
    for key in (
        "snapshot_digest", "inventory_digest", "report_digest", "state_digest", "memory_digest",
        "prompt_learning_digest", "digest",
    ):
        candidate = value.get(key)
        if candidate is not None:
            snapshot_digest = _bounded_digest(f"lifecycle {key}", candidate)
            break
    state_digest = _bounded_digest("lifecycle state_digest", value.get("state_digest"))
    generation = value.get("generation", value.get("snapshot_generation"))
    if generation is not None and (isinstance(generation, bool) or not isinstance(generation, int) or generation < 0):
        generation = None
    return schema, snapshot_digest, state_digest, generation


@dataclass(frozen=True, slots=True)
class AutonomousAgentPersistenceComponentResult:
    """Metadata-only result for one lifecycle component."""

    component_id: str
    operation: str
    status: str
    snapshot_schema: str | None = None
    snapshot_digest: str | None = None
    state_digest: str | None = None
    generation: int | None = None
    error_class: str | None = None
    component_digest: str | None = None

    def __post_init__(self) -> None:
        _bounded_identifier("lifecycle component_id", self.component_id)
        if self.operation not in AUTONOMOUS_AGENT_LIFECYCLE_OPERATIONS:
            raise ArgumentError("lifecycle component operation is unsupported")
        if self.status not in AUTONOMOUS_AGENT_LIFECYCLE_COMPONENT_STATUSES:
            raise ArgumentError("lifecycle component status is unsupported")
        if self.snapshot_schema is not None:
            _bounded_identifier("lifecycle snapshot_schema", self.snapshot_schema.replace("/", "-"))
        _bounded_digest("lifecycle snapshot_digest", self.snapshot_digest)
        _bounded_digest("lifecycle state_digest", self.state_digest)
        if self.generation is not None and (isinstance(self.generation, bool) or not isinstance(self.generation, int) or self.generation < 0):
            raise ArgumentError("lifecycle component generation is invalid")
        if self.error_class is not None:
            _bounded_identifier("lifecycle error_class", self.error_class)
        payload = self._payload()
        expected = content_digest(payload)
        if self.component_digest is not None and self.component_digest != expected:
            raise ArgumentError("lifecycle component digest does not match its fields")
        object.__setattr__(self, "component_digest", expected)

    def _payload(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_AGENT_LIFECYCLE_SCHEMA,
            "component_id": self.component_id,
            "operation": self.operation,
            "status": self.status,
            "snapshot_schema": self.snapshot_schema,
            "snapshot_digest": self.snapshot_digest,
            "state_digest": self.state_digest,
            "generation": self.generation,
            "error_class": self.error_class,
        }

    def to_dict(self) -> dict[str, Any]:
        return {
            **self._payload(),
            "component_digest": self.component_digest,
            "retention": "component_metadata_only;cross_store_payloads_caller_owned",
            "secret_material": "never_returned",
        }


@dataclass(frozen=True, slots=True)
class AutonomousAgentPersistenceLifecycleReport:
    """Digest-bound report for one ordered restore or flush pass."""

    operation: str
    status: str
    ordered_component_ids: tuple[str, ...]
    completed_component_ids: tuple[str, ...]
    unconfigured_component_ids: tuple[str, ...]
    failed_component_id: str | None
    components: tuple[AutonomousAgentPersistenceComponentResult, ...]
    next_action: str
    lifecycle_digest: str | None = None

    def __post_init__(self) -> None:
        if self.operation not in AUTONOMOUS_AGENT_LIFECYCLE_OPERATIONS:
            raise ArgumentError("lifecycle operation is unsupported")
        if self.status not in AUTONOMOUS_AGENT_LIFECYCLE_STATUSES:
            raise ArgumentError("lifecycle status is unsupported")
        ordered = tuple(_bounded_identifier("lifecycle ordered component", value) for value in self.ordered_component_ids)
        completed = tuple(_bounded_identifier("lifecycle completed component", value) for value in self.completed_component_ids)
        unconfigured = tuple(_bounded_identifier("lifecycle unconfigured component", value) for value in self.unconfigured_component_ids)
        if len(set(ordered)) != len(ordered) or len(set(completed)) != len(completed) or len(set(unconfigured)) != len(unconfigured):
            raise ArgumentError("lifecycle component ids must be unique")
        if any(value not in ordered for value in (*completed, *unconfigured)):
            raise ArgumentError("lifecycle component result is outside its order")
        if self.failed_component_id is not None:
            _bounded_identifier("lifecycle failed component", self.failed_component_id)
            if self.failed_component_id not in ordered:
                raise ArgumentError("lifecycle failed component is outside its order")
        if len(self.components) != len(ordered):
            raise ArgumentError("lifecycle report must contain one result per ordered component")
        if tuple(item.component_id for item in self.components) != ordered:
            raise ArgumentError("lifecycle component results are out of order")
        _bounded_identifier("lifecycle next_action", self.next_action)
        expected = content_digest(self._payload())
        if self.lifecycle_digest is not None and self.lifecycle_digest != expected:
            raise ArgumentError("lifecycle digest does not match its fields")
        object.__setattr__(self, "ordered_component_ids", ordered)
        object.__setattr__(self, "completed_component_ids", completed)
        object.__setattr__(self, "unconfigured_component_ids", unconfigured)
        object.__setattr__(self, "lifecycle_digest", expected)

    def _payload(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_AGENT_LIFECYCLE_SCHEMA,
            "operation": self.operation,
            "status": self.status,
            "ordered_component_ids": list(self.ordered_component_ids),
            "completed_component_ids": list(self.completed_component_ids),
            "unconfigured_component_ids": list(self.unconfigured_component_ids),
            "failed_component_id": self.failed_component_id,
            "components": [component.to_dict() for component in self.components],
            "next_action": self.next_action,
            "atomicity": "per_component_cas_only;cross_store_atomicity_caller_owned",
        }

    def to_dict(self) -> dict[str, Any]:
        return {
            **self._payload(),
            "lifecycle_digest": self.lifecycle_digest,
            "retention": "component_metadata_only;cross_store_payloads_caller_owned",
            "secret_material": "never_returned",
        }


class AutonomousAgentPersistenceLifecycleCoordinator:
    """Compose the configured agent persistence coordinators in a strict lifecycle order."""

    def __init__(
        self,
        agent: Any,
        *,
        model_inventory_store: Any | None = None,
        activation_store: Any | None = None,
        selection_promotion_store: Any | None = None,
        capability_journal_persistence: Any | None = None,
        decision_cycle_persistence: Any | None = None,
        execution_persistence: Any | None = None,
        require_all: bool = False,
        continue_on_error: bool = False,
    ) -> None:
        if agent is None:
            raise ArgumentError("agent persistence lifecycle requires an agent")
        if not callable(getattr(agent, "restore_model_inventory", None)) or not callable(getattr(agent, "flush_model_inventory", None)):
            raise ArgumentError("agent persistence lifecycle requires model inventory lifecycle methods")
        if not isinstance(require_all, bool) or not isinstance(continue_on_error, bool):
            raise ArgumentError("agent persistence lifecycle options must be boolean")
        self.agent = agent
        self.model_inventory_store = model_inventory_store
        self.activation_store = activation_store
        self.selection_promotion_store = selection_promotion_store
        self.capability_journal_persistence = capability_journal_persistence
        self.decision_cycle_persistence = decision_cycle_persistence
        self.execution_persistence = execution_persistence
        if selection_promotion_store is not None and getattr(agent, "selection_promotion", None) is None:
            raise ArgumentError("selection promotion persistence requires a configured selection lifecycle")
        self.require_all = require_all
        self.continue_on_error = continue_on_error
        self._lock = threading.RLock()
        self._last_report: AutonomousAgentPersistenceLifecycleReport | None = None

    @property
    def last_report(self) -> AutonomousAgentPersistenceLifecycleReport | None:
        return self._last_report

    def _coordinator_for(self, component_id: str) -> Any | None:
        if component_id == "model_inventory":
            return self.model_inventory_store
        if component_id == "activation":
            return self.activation_store
        if component_id == "selection_promotion":
            return self.selection_promotion_store
        if component_id == "capability_journal":
            return self.capability_journal_persistence
        if component_id == "decision_cycle":
            return self.decision_cycle_persistence
        if component_id == "execution":
            return self.execution_persistence
        return getattr(self.agent, f"{component_id}_persistence", None)

    def _invoke(self, component_id: str, operation: str) -> Any:
        if component_id == "model_inventory":
            if operation == "restore":
                return self.agent.restore_model_inventory(self.model_inventory_store)
            return self.agent.flush_model_inventory(self.model_inventory_store)
        if component_id == "activation":
            if operation == "restore":
                return self.agent.restore_activation(self.activation_store)
            return self.agent.save_activation(self.activation_store)
        if component_id == "selection_promotion":
            if operation == "restore":
                return self.agent.restore_selection_promotion(self.selection_promotion_store)
            return self.agent.save_selection_promotion(self.selection_promotion_store)
        if component_id == "capability_journal":
            if operation == "restore":
                return self.agent.restore_capability_journal_persistence()
            return self.agent.flush_capability_journal_persistence()
        if component_id == "decision_cycle":
            if operation == "restore":
                return self.agent.restore_decision_cycle_persistence()
            return self.agent.flush_decision_cycle_persistence()
        if component_id == "execution":
            if operation == "restore":
                return self.agent.restore_execution_persistence()
            return self.agent.flush_execution_persistence()
        method = getattr(self.agent, f"{operation}_{component_id}", None)
        if not callable(method):
            raise ArgumentError(f"agent does not expose {operation}_{component_id}")
        return method()

    def _order(self, operation: str) -> tuple[str, ...]:
        return AUTONOMOUS_AGENT_LIFECYCLE_RESTORE_ORDER if operation == "restore" else AUTONOMOUS_AGENT_LIFECYCLE_FLUSH_ORDER

    def _report(
        self,
        operation: str,
        ordered: tuple[str, ...],
        results: list[AutonomousAgentPersistenceComponentResult],
        failed_component_id: str | None,
    ) -> AutonomousAgentPersistenceLifecycleReport:
        completed = tuple(item.component_id for item in results if item.status in ("restored", "flushed", "empty"))
        unconfigured = tuple(item.component_id for item in results if item.status == "unconfigured")
        failed = failed_component_id is not None or any(item.status == "failed" for item in results)
        configured = len(ordered) - len(unconfigured)
        if failed:
            status = "failed" if not completed else "partial"
            next_action = "recover_failed_components_before_execution"
        elif configured == 0:
            status = "unconfigured"
            next_action = "bind_persistence_coordinators_before_execution"
        elif unconfigured:
            status = "partial"
            next_action = "bind_unconfigured_persistence_or_accept_partial_lifecycle"
        elif results and all(item.status == "empty" for item in results):
            status = "empty"
            next_action = "safe_to_begin_execution_with_empty_persistence"
        else:
            status = "completed"
            next_action = "safe_to_begin_execution" if operation == "restore" else "safe_to_finalize_process"
        while len(results) < len(ordered):
            component_id = ordered[len(results)]
            results.append(AutonomousAgentPersistenceComponentResult(component_id, operation, "not_attempted"))
        return AutonomousAgentPersistenceLifecycleReport(
            operation=operation,
            status=status,
            ordered_component_ids=ordered,
            completed_component_ids=completed,
            unconfigured_component_ids=unconfigured,
            failed_component_id=failed_component_id,
            components=tuple(results),
            next_action=next_action,
        )

    def _run(self, operation: str, *, strict: bool | None = None, continue_on_error: bool | None = None) -> AutonomousAgentPersistenceLifecycleReport:
        if operation not in AUTONOMOUS_AGENT_LIFECYCLE_OPERATIONS:
            raise ArgumentError("unsupported agent persistence lifecycle operation")
        resolved_strict = True if strict is None else strict
        resolved_continue = self.continue_on_error if continue_on_error is None else continue_on_error
        if not isinstance(resolved_strict, bool) or not isinstance(resolved_continue, bool):
            raise ArgumentError("lifecycle strict and continue_on_error options must be boolean")
        ordered = self._order(operation)
        results: list[AutonomousAgentPersistenceComponentResult] = []
        failed_component_id: str | None = None
        for component_id in ordered:
            if self._coordinator_for(component_id) is None:
                result = AutonomousAgentPersistenceComponentResult(component_id, operation, "unconfigured")
                results.append(result)
                if self.require_all and failed_component_id is None:
                    failed_component_id = component_id
                if self.require_all and not resolved_continue:
                    break
                continue
            try:
                value = self._invoke(component_id, operation)
                schema, snapshot_digest, state_digest, generation = _snapshot_projection(value)
                result = AutonomousAgentPersistenceComponentResult(
                    component_id,
                    operation,
                    "restored" if operation == "restore" and value is not None else "flushed" if operation == "flush" and value is not None else "empty",
                    snapshot_schema=schema,
                    snapshot_digest=snapshot_digest,
                    state_digest=state_digest,
                    generation=generation,
                )
                results.append(result)
            except Exception as error:
                results.append(AutonomousAgentPersistenceComponentResult(component_id, operation, "failed", error_class=_error_class(error)))
                failed_component_id = failed_component_id or component_id
                if not resolved_continue:
                    break
        report = self._report(operation, ordered, results, failed_component_id)
        self._last_report = report
        if resolved_strict and (report.failed_component_id is not None or report.status == "failed"):
            raise AutonomousAgentPersistenceLifecycleError(operation, report)
        return report

    def restore(self, *, strict: bool | None = None, continue_on_error: bool | None = None) -> AutonomousAgentPersistenceLifecycleReport:
        """Restore all configured components before new autonomous work is admitted."""

        with self._lock:
            return self._run("restore", strict=strict, continue_on_error=continue_on_error)

    def flush(self, *, strict: bool | None = None, continue_on_error: bool | None = None) -> AutonomousAgentPersistenceLifecycleReport:
        """Flush all configured components without claiming cross-store atomicity."""

        with self._lock:
            return self._run("flush", strict=strict, continue_on_error=continue_on_error)


__all__ = [
    "AUTONOMOUS_AGENT_LIFECYCLE_SCHEMA",
    "AUTONOMOUS_AGENT_LIFECYCLE_COMPONENTS",
    "AUTONOMOUS_AGENT_LIFECYCLE_RESTORE_ORDER",
    "AUTONOMOUS_AGENT_LIFECYCLE_FLUSH_ORDER",
    "AUTONOMOUS_AGENT_LIFECYCLE_OPERATIONS",
    "AUTONOMOUS_AGENT_LIFECYCLE_STATUSES",
    "AUTONOMOUS_AGENT_LIFECYCLE_COMPONENT_STATUSES",
    "AutonomousAgentPersistenceLifecycleError",
    "AutonomousAgentPersistenceComponentResult",
    "AutonomousAgentPersistenceLifecycleReport",
    "AutonomousAgentPersistenceLifecycleCoordinator",
]
