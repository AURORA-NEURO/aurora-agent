"""High-level, digest-bound autonomous connector operation orchestration.

The lower-level connector runtime is intentionally explicit: callers construct a typed
dispatch request, select a connector, and provide approval.  That is the right primitive for
durable workers, but it makes ordinary applications repeat the same identity and replay
plumbing for every operation.  This module supplies the application-facing composition layer
that already exists in the TypeScript SDK.

The facade is deliberately provider-neutral.  It accepts only JSON-safe transient metadata,
uses the reviewed operation catalogue and connector registry to build a request-free plan, and
dispatches through the existing approval/replay runtime.  Plans, batch summaries, and errors
contain digests and identities only; request values and connector observations remain transient
caller values.
"""

from __future__ import annotations

from concurrent.futures import ThreadPoolExecutor
from dataclasses import dataclass, field
import threading
from typing import Any, Mapping, Sequence

from .authoring import content_digest
from .autonomous_connector_worker import (
    AutonomousConnectorOperationContract,
    AutonomousConnectorOperationRegistry,
    AutonomousConnectorWorkItem,
    AutonomousConnectorWorkQueuePersistenceCoordinator,
    AutonomousConnectorWorker,
    InMemoryAutonomousConnectorWorkQueue,
)
from .autonomous_connectors import (
    AUTONOMOUS_CONNECTOR_SELECTION_STRATEGIES,
    AutonomousConnectorDispatchRequest,
    AutonomousConnectorDispatchResult,
    AutonomousConnectorRegistry,
    AutonomousConnectorRuntime,
    AutonomousConnectorSelectionPlan,
    _capability_identifier,
    _digest,
    _identifier,
    _json_safe,
    _reject_secret_fields,
)
from .domain_tools import AUTONOMOUS_DOMAIN_NAMES
from .errors import ArgumentError
from .brain import BrainRunError


AUTONOMOUS_CONNECTOR_OPERATION_FACADE_SCHEMA = (
    "bioprism-python-autonomous-connector-operation-facade/0.1"
)
AUTONOMOUS_CONNECTOR_OPERATION_BATCH_SCHEMA = (
    "bioprism-python-autonomous-connector-operation-batch/0.1"
)
MAX_AUTONOMOUS_CONNECTOR_FACADE_BATCH = 64
MAX_AUTONOMOUS_CONNECTOR_FACADE_PARALLELISM = 8
MAX_AUTONOMOUS_CONNECTOR_FACADE_PARENT_DIGESTS = 128
MAX_AUTONOMOUS_CONNECTOR_FACADE_REQUEST_BYTES = 2_000_000
AUTONOMOUS_CONNECTOR_INTENT_SCHEMA = "bioprism-python-autonomous-connector-intent/0.1"
MAX_AUTONOMOUS_CONNECTOR_INTENT_TASK_BYTES = 128_000
MAX_AUTONOMOUS_CONNECTOR_INTENT_HINTS = 32
AUTONOMOUS_CONNECTOR_INTENT_JOB_SCHEMA = "bioprism-python-autonomous-connector-intent-job/0.1"
MAX_AUTONOMOUS_CONNECTOR_INTENT_JOB_ITEMS = 32
AUTONOMOUS_CONNECTOR_INTENT_CONTROLLER_SCHEMA = (
    "bioprism-python-autonomous-connector-intent-controller/0.1"
)


def _parent_digests(value: Sequence[str] | None) -> tuple[str, ...]:
    if value is None:
        return ()
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)):
        raise ArgumentError("connector operation parent_digests must be a sequence")
    if len(value) > MAX_AUTONOMOUS_CONNECTOR_FACADE_PARENT_DIGESTS:
        raise ArgumentError("connector operation parent_digests exceeds its bound")
    normalized = tuple(_digest("connector operation parent_digest", item) for item in value)
    if len(set(normalized)) != len(normalized):
        raise ArgumentError("connector operation parent_digests contains duplicates")
    return normalized


def _error_projection(error: BaseException) -> dict[str, str]:
    name = type(error).__name__
    return {
        "error_class": name if name and all(character.isalnum() or character in "_.:-" for character in name) else "ConnectorOperationError",
        "failure_code": "argument" if isinstance(error, ArgumentError) else "error",
    }


@dataclass(frozen=True, slots=True)
class AutonomousConnectorOperationInput:
    """Transient operation metadata accepted by the high-level facade."""

    domain: str
    capability: str
    operation_id: str
    subject_digest: str | None = None
    request: Mapping[str, Any] = field(default_factory=dict)
    execution_id: str | None = None
    call_id: str | None = None
    attempt_id: str | None = None
    parent_digests: tuple[str, ...] = ()
    approved: bool = False
    selection_strategy: str = "lexicographic_connector_id"
    selection_signals: Mapping[str, Mapping[str, Any]] | None = None

    def __post_init__(self) -> None:
        _identifier("connector operation domain", self.domain)
        if self.domain not in AUTONOMOUS_DOMAIN_NAMES:
            raise ArgumentError("connector operation domain is unsupported")
        _capability_identifier("connector operation capability", self.capability)
        _identifier("connector operation operation_id", self.operation_id)
        if self.subject_digest is not None:
            _digest("connector operation subject_digest", self.subject_digest)
        if not isinstance(self.request, Mapping):
            raise ArgumentError("connector operation request must be an object")
        safe_request = _json_safe(
            "connector operation request",
            dict(self.request),
            maximum=MAX_AUTONOMOUS_CONNECTOR_FACADE_REQUEST_BYTES,
        )
        _reject_secret_fields(safe_request)
        if self.execution_id is not None:
            _identifier("connector operation execution_id", self.execution_id)
        if self.call_id is not None:
            _identifier("connector operation call_id", self.call_id)
        if self.attempt_id is not None:
            _identifier("connector operation attempt_id", self.attempt_id)
        if not isinstance(self.approved, bool):
            raise ArgumentError("connector operation approved must be boolean")
        if self.selection_strategy not in AUTONOMOUS_CONNECTOR_SELECTION_STRATEGIES:
            raise ArgumentError("connector operation selection_strategy is invalid")
        if self.selection_signals is not None and not isinstance(self.selection_signals, Mapping):
            raise ArgumentError("connector operation selection_signals must be an object")
        object.__setattr__(self, "request", safe_request)
        object.__setattr__(self, "parent_digests", _parent_digests(self.parent_digests))

    @classmethod
    def from_mapping(cls, value: Mapping[str, Any]) -> "AutonomousConnectorOperationInput":
        if not isinstance(value, Mapping):
            raise ArgumentError("connector operation input must be an object")
        allowed = {
            "domain", "capability", "operation_id", "subject_digest", "request", "execution_id",
            "call_id", "attempt_id", "parent_digests", "approved", "selection_strategy",
            "selection_signals",
        }
        unknown = sorted(set(value).difference(allowed))
        if unknown:
            raise ArgumentError("connector operation input contains unknown fields: " + ", ".join(unknown))
        return cls(
            domain=value.get("domain"),
            capability=value.get("capability"),
            operation_id=value.get("operation_id"),
            subject_digest=value.get("subject_digest"),
            request=value.get("request", {}),
            execution_id=value.get("execution_id"),
            call_id=value.get("call_id"),
            attempt_id=value.get("attempt_id"),
            parent_digests=tuple(value.get("parent_digests", ())),
            approved=value.get("approved", False),
            selection_strategy=value.get("selection_strategy", "lexicographic_connector_id"),
            selection_signals=value.get("selection_signals"),
        )


@dataclass(frozen=True, slots=True)
class AutonomousConnectorOperationPlan:
    """Request-free, digest-bound operation plan safe to persist or review."""

    domain: str
    capability: str
    operation_id: str
    operation_digest: str
    subject_digest: str
    execution_id: str
    call_id: str
    attempt_id: str | None
    parent_digests: tuple[str, ...]
    request_digest: str
    selection_plan: AutonomousConnectorSelectionPlan
    selected_connector_id: str | None
    status: str
    approved: bool
    plan_digest: str = field(init=False)

    def __post_init__(self) -> None:
        if self.domain not in AUTONOMOUS_DOMAIN_NAMES:
            raise ArgumentError("connector operation plan domain is unsupported")
        _capability_identifier("connector operation plan capability", self.capability)
        _identifier("connector operation plan operation_id", self.operation_id)
        _digest("connector operation plan operation_digest", self.operation_digest)
        _digest("connector operation plan subject_digest", self.subject_digest)
        _identifier("connector operation plan execution_id", self.execution_id)
        _identifier("connector operation plan call_id", self.call_id)
        if self.attempt_id is not None:
            _identifier("connector operation plan attempt_id", self.attempt_id)
        _parent_digests(self.parent_digests)
        _digest("connector operation plan request_digest", self.request_digest)
        if not isinstance(self.selection_plan, AutonomousConnectorSelectionPlan):
            raise ArgumentError("connector operation plan selection_plan is invalid")
        if (
            self.selection_plan.domains != (self.domain,)
            or self.selection_plan.capability != self.capability
        ):
            raise ArgumentError("connector operation plan selection does not match the operation")
        if self.status not in {"ready", "connector_missing"}:
            raise ArgumentError("connector operation plan status is invalid")
        if self.status == "ready" and self.selected_connector_id is None:
            raise ArgumentError("ready connector operation plan requires a connector")
        if self.status == "connector_missing" and self.selected_connector_id is not None:
            raise ArgumentError("missing connector operation plan cannot select a connector")
        if self.selected_connector_id is not None:
            _identifier("connector operation plan selected_connector_id", self.selected_connector_id)
        if not isinstance(self.approved, bool):
            raise ArgumentError("connector operation plan approved must be boolean")
        object.__setattr__(self, "parent_digests", _parent_digests(self.parent_digests))
        object.__setattr__(self, "plan_digest", content_digest(self._descriptor()))

    def _descriptor(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_CONNECTOR_OPERATION_FACADE_SCHEMA,
            "domain": self.domain,
            "capability": self.capability,
            "operation_id": self.operation_id,
            "operation_digest": self.operation_digest,
            "subject_digest": self.subject_digest,
            "execution_id": self.execution_id,
            "call_id": self.call_id,
            "attempt_id": self.attempt_id,
            "parent_digests": list(self.parent_digests),
            "request_digest": self.request_digest,
            "selection_plan": self.selection_plan.to_dict(),
            "selected_connector_id": self.selected_connector_id,
            "status": self.status,
            "approved": self.approved,
            "retention": "metadata_only_no_request_values",
            "secret_material": "never_returned",
        }

    def to_dict(self) -> dict[str, Any]:
        return {**self._descriptor(), "plan_digest": self.plan_digest}

    @classmethod
    def from_mapping(cls, value: Mapping[str, Any]) -> "AutonomousConnectorOperationPlan":
        expected = {
            "schema", "domain", "capability", "operation_id", "operation_digest", "subject_digest",
            "execution_id", "call_id", "attempt_id", "parent_digests", "request_digest",
            "selection_plan", "selected_connector_id", "status", "approved", "retention",
            "secret_material", "plan_digest",
        }
        if not isinstance(value, Mapping) or set(value) != expected:
            raise ArgumentError("connector operation plan is malformed")
        if value.get("schema") != AUTONOMOUS_CONNECTOR_OPERATION_FACADE_SCHEMA:
            raise ArgumentError("connector operation plan schema is invalid")
        if value.get("retention") != "metadata_only_no_request_values" or value.get("secret_material") != "never_returned":
            raise ArgumentError("connector operation plan retention is invalid")
        parent = value.get("parent_digests")
        if not isinstance(parent, Sequence) or isinstance(parent, (str, bytes)):
            raise ArgumentError("connector operation plan parent_digests are invalid")
        selection = value.get("selection_plan")
        if not isinstance(selection, Mapping):
            raise ArgumentError("connector operation plan selection_plan is invalid")
        plan = cls(
            domain=value.get("domain"),
            capability=value.get("capability"),
            operation_id=value.get("operation_id"),
            operation_digest=value.get("operation_digest"),
            subject_digest=value.get("subject_digest"),
            execution_id=value.get("execution_id"),
            call_id=value.get("call_id"),
            attempt_id=value.get("attempt_id"),
            parent_digests=tuple(parent),
            request_digest=value.get("request_digest"),
            selection_plan=AutonomousConnectorSelectionPlan.from_mapping(selection),
            selected_connector_id=value.get("selected_connector_id"),
            status=value.get("status"),
            approved=value.get("approved"),
        )
        if value.get("plan_digest") != plan.plan_digest:
            raise ArgumentError("connector operation plan digest is invalid")
        return plan


@dataclass(frozen=True, slots=True)
class AutonomousConnectorOperationExecution:
    """Transient dispatch value paired with a request-free operation plan."""

    status: str
    operation_plan: AutonomousConnectorOperationPlan
    dispatch: AutonomousConnectorDispatchResult
    replay: str

    def __post_init__(self) -> None:
        if not isinstance(self.operation_plan, AutonomousConnectorOperationPlan):
            raise ArgumentError("connector operation execution plan is invalid")
        if not isinstance(self.dispatch, AutonomousConnectorDispatchResult):
            raise ArgumentError("connector operation execution dispatch is invalid")
        if self.status not in {"observed", "partial", "refused", "error", "unknown"}:
            raise ArgumentError("connector operation execution status is invalid")
        if self.replay not in {"fresh", "replayed"}:
            raise ArgumentError("connector operation execution replay is invalid")

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_CONNECTOR_OPERATION_FACADE_SCHEMA,
            "status": self.status,
            "operation_plan": self.operation_plan.to_dict(),
            "dispatch": self.dispatch.to_dict(),
            "replay": self.replay,
            "retention": "operation_plan_metadata_only;dispatch_value_transient",
            "secret_material": "never_returned",
        }

@dataclass(frozen=True, slots=True)
class AutonomousConnectorOperationBatchResult:
    """Ordered, bounded batch projection for independent connector operations."""

    status: str
    items: tuple[Mapping[str, Any], ...]
    completed_count: int
    failed_count: int
    omitted_count: int
    max_parallelism: int
    stop_on_error: bool
    batch_digest: str

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_CONNECTOR_OPERATION_BATCH_SCHEMA,
            "status": self.status,
            "items": [dict(item) for item in self.items],
            "completed_count": self.completed_count,
            "failed_count": self.failed_count,
            "omitted_count": self.omitted_count,
            "max_parallelism": self.max_parallelism,
            "stop_on_error": self.stop_on_error,
            "batch_digest": self.batch_digest,
            "retention": "operation_plans_metadata_only;dispatch_values_transient",
            "secret_material": "never_returned",
        }


@dataclass(frozen=True, slots=True)
class _PreparedOperation:
    operation: AutonomousConnectorOperationContract
    request: Mapping[str, Any]
    dispatch: AutonomousConnectorDispatchRequest | None
    plan: AutonomousConnectorOperationPlan


class AutonomousConnectorOperationFacade:
    """Compose operation validation, connector selection, approval, and replay."""

    def __init__(
        self,
        registry: AutonomousConnectorRegistry,
        runtime: AutonomousConnectorRuntime,
        operation_registry: AutonomousConnectorOperationRegistry | None = None,
    ) -> None:
        if not isinstance(registry, AutonomousConnectorRegistry):
            raise ArgumentError("connector operation facade requires an AutonomousConnectorRegistry")
        if not isinstance(runtime, AutonomousConnectorRuntime) or runtime.registry is not registry:
            raise ArgumentError("connector operation facade runtime must use the same registry")
        self.registry = registry
        self.runtime = runtime
        self.operation_registry = operation_registry or AutonomousConnectorOperationRegistry()
        if not isinstance(self.operation_registry, AutonomousConnectorOperationRegistry):
            raise ArgumentError("connector operation facade operation_registry is invalid")

    def plan(
        self,
        value: AutonomousConnectorOperationInput | Mapping[str, Any],
    ) -> AutonomousConnectorOperationPlan:
        """Build a reviewed plan without invoking a connector."""

        return self._prepare(self._coerce(value)).plan

    def execute(
        self,
        value: AutonomousConnectorOperationInput | Mapping[str, Any],
    ) -> AutonomousConnectorOperationExecution:
        prepared = self._prepare(self._coerce(value))
        if prepared.dispatch is None or prepared.plan.status != "ready":
            raise BrainRunError("connector operation has no eligible connector")
        return self._dispatch(prepared)

    def execute_planned(
        self,
        plan: AutonomousConnectorOperationPlan,
        value: AutonomousConnectorOperationInput | Mapping[str, Any],
    ) -> AutonomousConnectorOperationExecution:
        if not isinstance(plan, AutonomousConnectorOperationPlan):
            raise ArgumentError("connector operation execute_planned requires a typed plan")
        prepared = self._prepare(self._coerce(value))
        if prepared.plan.plan_digest != plan.plan_digest:
            raise ArgumentError("connector operation plan does not match the supplied transient request")
        if prepared.dispatch is None:
            raise BrainRunError("connector operation plan has no eligible connector")
        return self._dispatch(prepared)

    def prepare_dispatch(
        self,
        plan: AutonomousConnectorOperationPlan,
        value: AutonomousConnectorOperationInput | Mapping[str, Any],
    ) -> tuple[AutonomousConnectorSelectionPlan, AutonomousConnectorDispatchRequest]:
        """Rehydrate a reviewed plan into a transient worker request without dispatching it."""

        if not isinstance(plan, AutonomousConnectorOperationPlan):
            raise ArgumentError("connector operation prepare_dispatch requires a typed plan")
        prepared = self._prepare(self._coerce(value))
        if prepared.plan.plan_digest != plan.plan_digest:
            raise ArgumentError("connector operation plan does not match the supplied transient request")
        if prepared.dispatch is None:
            raise BrainRunError("connector operation plan has no eligible connector")
        return prepared.plan.selection_plan, prepared.dispatch

    def execute_batch(
        self,
        values: Sequence[AutonomousConnectorOperationInput | Mapping[str, Any]],
        *,
        max_parallelism: int = 4,
        stop_on_error: bool = False,
    ) -> AutonomousConnectorOperationBatchResult:
        if not isinstance(values, Sequence) or isinstance(values, (str, bytes)):
            raise ArgumentError("connector operation batch must be a sequence")
        if not 1 <= len(values) <= MAX_AUTONOMOUS_CONNECTOR_FACADE_BATCH:
            raise ArgumentError(
                f"connector operation batch must contain 1..={MAX_AUTONOMOUS_CONNECTOR_FACADE_BATCH} entries"
            )
        if isinstance(max_parallelism, bool) or not 1 <= max_parallelism <= MAX_AUTONOMOUS_CONNECTOR_FACADE_PARALLELISM:
            raise ArgumentError("connector operation batch max_parallelism is outside its bound")
        if not isinstance(stop_on_error, bool):
            raise ArgumentError("connector operation batch stop_on_error must be boolean")
        items: list[dict[str, Any] | None] = [None] * len(values)
        cursor = 0
        halted = False
        cursor_lock = threading.Lock()
        halt_lock = threading.Lock()

        def worker() -> None:
            nonlocal cursor, halted
            while True:
                with cursor_lock:
                    index = cursor
                    cursor += 1
                if index >= len(values):
                    return
                with halt_lock:
                    is_halted = halted
                if is_halted:
                    items[index] = {"index": index, "status": "omitted", "plan_digest": None}
                    continue
                try:
                    execution = self.execute(values[index])
                    succeeded = execution.status in {"observed", "partial"}
                    items[index] = {
                        "index": index,
                        "status": "succeeded" if succeeded else "refused",
                        "plan_digest": execution.operation_plan.plan_digest,
                        "execution": execution.to_dict(),
                    }
                    if stop_on_error and not succeeded:
                        with halt_lock:
                            halted = True
                except BaseException as error:
                    items[index] = {
                        "index": index,
                        "status": "failed" if stop_on_error else "refused",
                        "plan_digest": None,
                        **_error_projection(error),
                    }
                    if stop_on_error:
                        with halt_lock:
                            halted = True

        with ThreadPoolExecutor(max_workers=min(max_parallelism, len(values))) as pool:
            futures = [pool.submit(worker) for _ in range(min(max_parallelism, len(values)))]
            for future in futures:
                future.result()
        normalized = tuple(
            item
            if item is not None
            else {
                "index": index,
                "status": "failed",
                "plan_digest": None,
                "error_class": "ConnectorOperationError",
                "failure_code": "missing_batch_result",
            }
            for index, item in enumerate(items)
        )
        completed = sum(item["status"] == "succeeded" for item in normalized)
        failed = sum(item["status"] in {"refused", "failed"} for item in normalized)
        omitted = sum(item["status"] == "omitted" for item in normalized)
        status = "completed" if failed == 0 and omitted == 0 else "partial" if completed else "failed"
        batch_digest = content_digest(
            [
                {
                    "index": item["index"],
                    "status": item["status"],
                    "plan_digest": item["plan_digest"],
                    "error_class": item.get("error_class"),
                    "failure_code": item.get("failure_code"),
                    "dispatch": (
                        item.get("execution", {}).get("dispatch", {}).get("receipt")
                        if isinstance(item.get("execution"), Mapping)
                        else None
                    ),
                }
                for item in normalized
            ]
        )
        return AutonomousConnectorOperationBatchResult(
            status=status,
            items=normalized,
            completed_count=completed,
            failed_count=failed,
            omitted_count=omitted,
            max_parallelism=max_parallelism,
            stop_on_error=stop_on_error,
            batch_digest=batch_digest,
        )

    @staticmethod
    def _coerce(value: AutonomousConnectorOperationInput | Mapping[str, Any]) -> AutonomousConnectorOperationInput:
        if isinstance(value, AutonomousConnectorOperationInput):
            return value
        return AutonomousConnectorOperationInput.from_mapping(value)

    def _prepare(self, value: AutonomousConnectorOperationInput) -> _PreparedOperation:
        operation = self.operation_registry.resolve(value.operation_id)
        if operation.domain != value.domain:
            raise ArgumentError("connector operation domain does not match its operation contract")
        if not operation.supports(value.capability):
            raise ArgumentError("connector operation capability is outside its operation contract")
        supplied = dict(value.request)
        if "operation_id" in supplied and supplied["operation_id"] != value.operation_id:
            raise ArgumentError("connector operation request operation_id does not match the operation")
        if "subject_digest" in supplied and value.subject_digest is not None and supplied["subject_digest"] != value.subject_digest:
            raise ArgumentError("connector operation request subject_digest does not match the operation input")
        without_identity = {
            key: child for key, child in supplied.items() if key not in {"operation_id", "subject_digest"}
        }
        subject_digest = value.subject_digest or content_digest(
            {
                "schema": "bioprism-python-autonomous-connector-subject/0.1",
                "domain": value.domain,
                "operation_id": value.operation_id,
                "metadata": without_identity,
            }
        )
        request = {
            **without_identity,
            "operation_id": value.operation_id,
            "subject_digest": subject_digest,
        }
        if value.selection_strategy == "weighted_evidence":
            if value.selection_signals is None:
                raise ArgumentError("weighted connector selection requires selection_signals")
            selection = self.registry.select_adaptive_for_domains(
                (value.domain,),
                capability=value.capability,
                selection_signals=value.selection_signals,
            )
        else:
            if value.selection_signals is not None:
                raise ArgumentError("lexicographic connector selection cannot consume selection_signals")
            selection = self.registry.select_for_domains(
                (value.domain,), capability=value.capability
            )
        row = selection.rows[0]
        if row is None:
            raise BrainRunError("connector operation selection returned no domain row")
        identity = content_digest(
            {
                "schema": AUTONOMOUS_CONNECTOR_OPERATION_FACADE_SCHEMA,
                "domain": value.domain,
                "capability": value.capability,
                "operation_id": value.operation_id,
                "subject_digest": subject_digest,
                "request": request,
                "parent_digests": list(value.parent_digests),
                "attempt_id": value.attempt_id,
                "selection_plan_digest": selection.plan_digest,
                "approved": value.approved,
            }
        )
        execution_id = value.execution_id or f"connector-execution-{identity[:48]}"
        call_id = value.call_id or f"connector-call-{identity[:48]}"
        request_digest = content_digest(
            {
                "schema": AUTONOMOUS_CONNECTOR_OPERATION_FACADE_SCHEMA,
                "domain": value.domain,
                "capability": value.capability,
                "operation_id": value.operation_id,
                "subject_digest": subject_digest,
                "request": request,
            }
        )
        selected = row.connector_id if row.status == "selected" else None
        dispatch = None
        if selected is not None:
            dispatch = AutonomousConnectorDispatchRequest(
                dispatch_id=f"connector-dispatch-{identity[:48]}",
                execution_id=execution_id,
                call_id=call_id,
                connector_id=selected,
                domains=(value.domain,),
                capability=value.capability,
                request=request,
                parent_digests=value.parent_digests,
                attempt_id=value.attempt_id,
                selection_plan_digest=selection.plan_digest,
                approved=value.approved,
            )
            operation.assert_request(dispatch)
            request_digest = dispatch.request_digest
        plan = AutonomousConnectorOperationPlan(
            domain=value.domain,
            capability=value.capability,
            operation_id=value.operation_id,
            operation_digest=operation.operation_digest,
            subject_digest=subject_digest,
            execution_id=execution_id,
            call_id=call_id,
            attempt_id=value.attempt_id,
            parent_digests=value.parent_digests,
            request_digest=request_digest,
            selection_plan=selection,
            selected_connector_id=selected,
            status="ready" if selected is not None else "connector_missing",
            approved=value.approved,
        )
        return _PreparedOperation(operation, request, dispatch, plan)

    def _dispatch(self, prepared: _PreparedOperation) -> AutonomousConnectorOperationExecution:
        if prepared.dispatch is None:
            raise BrainRunError("connector operation has no dispatch request")
        result = self.runtime.dispatch_from_plan(prepared.plan.selection_plan, prepared.dispatch)
        return AutonomousConnectorOperationExecution(
            status=result.receipt.status,
            operation_plan=prepared.plan,
            dispatch=result,
            replay=result.replay,
        )


def _intent_tokens(value: str) -> frozenset[str]:
    normalized = "".join(character.lower() if character.isalnum() else " " for character in value)
    return frozenset(token for token in normalized.split() if len(token) >= 2)


def _operation_intent(
    operation: AutonomousConnectorOperationContract,
    text: str,
    *,
    capability: str | None,
) -> tuple[str, float, tuple[str, ...], str]:
    """Choose an exact reviewed capability using deterministic lexical evidence.

    This is deliberately not an LLM substitute.  It only maps words already present in the
    reviewed operation catalogue; an unmatched task falls back to the contract's first
    capability and remains visible in the plan as a default selection.
    """

    if capability is not None:
        if not operation.supports(capability):
            raise ArgumentError(
                f"connector intent capability {capability!r} is outside {operation.operation_id!r}"
            )
        return capability, 1.0, (capability,), "caller_capability"
    task_tokens = _intent_tokens(text)
    scored: list[tuple[float, str, tuple[str, ...]]] = []
    for candidate in operation.capabilities:
        candidate_tokens = _intent_tokens(candidate.replace("+", " "))
        matched = tuple(sorted(candidate_tokens.intersection(task_tokens)))
        exact = 1.0 if candidate.lower() in text.lower() else 0.0
        score = min(1.0, 0.25 * len(matched) + 0.65 * exact)
        scored.append((score, candidate, matched))
    scored.sort(key=lambda row: (-row[0], row[1]))
    score, selected, matched = scored[0]
    if score <= 0.0:
        return selected, 0.0, (), "domain_default_capability"
    return selected, score, matched, "exact_catalogue_terms"


@dataclass(frozen=True, slots=True)
class AutonomousConnectorIntentSelection:
    """One domain operation selected from a task without retaining task text."""

    domain: str
    operation_id: str
    operation_digest: str
    capability: str
    score: float
    matched_terms: tuple[str, ...]
    selection_reason: str
    operation_plan: AutonomousConnectorOperationPlan

    def __post_init__(self) -> None:
        if self.domain not in AUTONOMOUS_DOMAIN_NAMES:
            raise ArgumentError("connector intent selection domain is unsupported")
        _identifier("connector intent selection operation_id", self.operation_id)
        _digest("connector intent selection operation_digest", self.operation_digest)
        _capability_identifier("connector intent selection capability", self.capability)
        if isinstance(self.score, bool) or not isinstance(self.score, (int, float)) or not 0.0 <= float(self.score) <= 1.0:
            raise ArgumentError("connector intent selection score must be between 0 and 1")
        if not isinstance(self.matched_terms, Sequence) or any(not isinstance(term, str) for term in self.matched_terms):
            raise ArgumentError("connector intent selection matched_terms must be strings")
        _identifier("connector intent selection reason", self.selection_reason)
        if not isinstance(self.operation_plan, AutonomousConnectorOperationPlan):
            raise ArgumentError("connector intent selection operation_plan is invalid")

    def to_dict(self) -> dict[str, Any]:
        return {
            "domain": self.domain,
            "operation_id": self.operation_id,
            "operation_digest": self.operation_digest,
            "capability": self.capability,
            "score": float(self.score),
            "matched_terms": list(self.matched_terms),
            "selection_reason": self.selection_reason,
            "operation_plan": self.operation_plan.to_dict(),
            "retention": "metadata_only_task_and_request_values_not_retained",
            "secret_material": "never_returned",
        }


@dataclass(frozen=True, slots=True)
class AutonomousConnectorIntentPlan:
    """Request-free plan for one automatically routed, possibly cross-domain task."""

    task_digest: str
    route: Mapping[str, Any]
    selected_domains: tuple[str, ...]
    cross_domain: bool
    status: str
    selections: tuple[AutonomousConnectorIntentSelection, ...]
    plan_digest: str = field(init=False)

    def __post_init__(self) -> None:
        _digest("connector intent task_digest", self.task_digest)
        if not isinstance(self.route, Mapping):
            raise ArgumentError("connector intent route must be an object")
        safe_route = _json_safe(
            "connector intent route",
            dict(self.route),
            maximum=MAX_AUTONOMOUS_CONNECTOR_FACADE_REQUEST_BYTES,
        )
        _reject_secret_fields(safe_route)
        if not isinstance(self.selected_domains, Sequence):
            raise ArgumentError("connector intent selected_domains must be a sequence")
        domains = tuple(_identifier("connector intent selected domain", item) for item in self.selected_domains)
        if any(domain not in AUTONOMOUS_DOMAIN_NAMES for domain in domains) or len(set(domains)) != len(domains):
            raise ArgumentError("connector intent selected_domains are invalid")
        if not domains and self.status != "route_review_required":
            raise ArgumentError("connector intent selected_domains must be non-empty outside route review")
        if self.cross_domain != (len(domains) > 1):
            raise ArgumentError("connector intent cross_domain does not match selected domains")
        if self.status not in {"ready", "route_review_required", "connector_review_required"}:
            raise ArgumentError("connector intent plan status is invalid")
        if not isinstance(self.selections, Sequence) or tuple(item.domain for item in self.selections) != domains:
            raise ArgumentError("connector intent selections must align with selected domains")
        object.__setattr__(self, "route", safe_route)
        object.__setattr__(self, "selected_domains", domains)
        object.__setattr__(self, "selections", tuple(self.selections))
        object.__setattr__(self, "plan_digest", content_digest(self._descriptor()))

    def _descriptor(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_CONNECTOR_INTENT_SCHEMA,
            "task_digest": self.task_digest,
            "route": dict(self.route),
            "selected_domains": list(self.selected_domains),
            "cross_domain": self.cross_domain,
            "status": self.status,
            "selections": [selection.to_dict() for selection in self.selections],
            "retention": "metadata_only_task_and_request_values_not_retained",
            "secret_material": "never_returned",
        }

    def to_dict(self) -> dict[str, Any]:
        return {**self._descriptor(), "plan_digest": self.plan_digest}

    @classmethod
    def from_mapping(cls, value: Mapping[str, Any]) -> "AutonomousConnectorIntentPlan":
        expected = {
            "schema", "task_digest", "route", "selected_domains", "cross_domain", "status",
            "selections", "retention", "secret_material", "plan_digest",
        }
        if not isinstance(value, Mapping) or set(value) != expected:
            raise ArgumentError("connector intent plan is malformed")
        if value.get("schema") != AUTONOMOUS_CONNECTOR_INTENT_SCHEMA:
            raise ArgumentError("connector intent plan schema is invalid")
        raw_route = value.get("route")
        raw_domains = value.get("selected_domains")
        raw_selections = value.get("selections")
        if (
            not isinstance(raw_route, Mapping)
            or not isinstance(raw_domains, Sequence)
            or isinstance(raw_domains, (str, bytes))
            or not isinstance(raw_selections, Sequence)
            or isinstance(raw_selections, (str, bytes))
        ):
            raise ArgumentError("connector intent plan collections are invalid")
        selections: list[AutonomousConnectorIntentSelection] = []
        for raw in raw_selections:
            if not isinstance(raw, Mapping):
                raise ArgumentError("connector intent plan selection is invalid")
            expected_selection = {
                "domain", "operation_id", "operation_digest", "capability", "score", "matched_terms",
                "selection_reason", "operation_plan", "retention", "secret_material",
            }
            if (
                set(raw) != expected_selection
                or raw.get("retention") != "metadata_only_task_and_request_values_not_retained"
                or raw.get("secret_material") != "never_returned"
            ):
                raise ArgumentError("connector intent plan selection metadata is invalid")
            terms = raw.get("matched_terms")
            operation_plan = raw.get("operation_plan")
            if (
                not isinstance(terms, Sequence)
                or isinstance(terms, (str, bytes))
                or not isinstance(operation_plan, Mapping)
            ):
                raise ArgumentError("connector intent plan selection fields are invalid")
            selections.append(
                AutonomousConnectorIntentSelection(
                    domain=raw.get("domain"),
                    operation_id=raw.get("operation_id"),
                    operation_digest=raw.get("operation_digest"),
                    capability=raw.get("capability"),
                    score=raw.get("score"),
                    matched_terms=tuple(terms),
                    selection_reason=raw.get("selection_reason"),
                    operation_plan=AutonomousConnectorOperationPlan.from_mapping(operation_plan),
                )
            )
        plan = cls(
            task_digest=value.get("task_digest"),
            route=raw_route,
            selected_domains=tuple(raw_domains),
            cross_domain=value.get("cross_domain"),
            status=value.get("status"),
            selections=tuple(selections),
        )
        if (
            value.get("retention") != "metadata_only_task_and_request_values_not_retained"
            or value.get("secret_material") != "never_returned"
        ):
            raise ArgumentError("connector intent plan retention is invalid")
        if value.get("plan_digest") != plan.plan_digest:
            raise ArgumentError("connector intent plan digest is invalid")
        return plan


@dataclass(frozen=True, slots=True)
class AutonomousConnectorIntentExecution:
    """Transient values from an intent plan with a redacted durable projection."""

    status: str
    plan: AutonomousConnectorIntentPlan
    items: tuple[Mapping[str, Any], ...]
    executions: tuple[AutonomousConnectorOperationExecution, ...]

    def __post_init__(self) -> None:
        if self.status not in {"completed", "partial", "failed", "route_review_required", "connector_review_required"}:
            raise ArgumentError("connector intent execution status is invalid")
        if not isinstance(self.plan, AutonomousConnectorIntentPlan):
            raise ArgumentError("connector intent execution plan is invalid")

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_CONNECTOR_INTENT_SCHEMA,
            "status": self.status,
            "plan": self.plan.to_dict(),
            "items": [dict(item) for item in self.items],
            "execution_count": len(self.executions),
            "retention": "metadata_only_task_and_request_values;dispatch_values_transient",
            "secret_material": "never_returned",
        }


@dataclass(frozen=True, slots=True)
class AutonomousConnectorIntentJob:
    """Metadata-only queue submission for a reviewed intent plan.

    Queue rows contain only operation, request, selection, and replay digests.  The caller must
    re-supply transient request metadata when a worker resumes the job.
    """

    job_id: str
    plan_digest: str
    status: str
    items: tuple[Mapping[str, Any], ...]
    enqueued_count: int
    omitted_count: int
    job_digest: str = field(init=False)

    def __post_init__(self) -> None:
        _identifier("connector intent job_id", self.job_id)
        _digest("connector intent job plan_digest", self.plan_digest)
        if self.status not in {"queued", "route_review_required", "connector_review_required", "partial"}:
            raise ArgumentError("connector intent job status is invalid")
        if not isinstance(self.items, Sequence) or isinstance(self.items, (str, bytes)):
            raise ArgumentError("connector intent job items must be a sequence")
        if len(self.items) > MAX_AUTONOMOUS_CONNECTOR_INTENT_JOB_ITEMS:
            raise ArgumentError("connector intent job contains too many items")
        for item in self.items:
            if not isinstance(item, Mapping):
                raise ArgumentError("connector intent job item must be an object")
        if isinstance(self.enqueued_count, bool) or not isinstance(self.enqueued_count, int) or self.enqueued_count < 0:
            raise ArgumentError("connector intent job enqueued_count is invalid")
        if isinstance(self.omitted_count, bool) or not isinstance(self.omitted_count, int) or self.omitted_count < 0:
            raise ArgumentError("connector intent job omitted_count is invalid")
        object.__setattr__(self, "job_digest", content_digest(self._descriptor()))

    def _descriptor(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_CONNECTOR_INTENT_JOB_SCHEMA,
            "job_id": self.job_id,
            "plan_digest": self.plan_digest,
            "status": self.status,
            "items": [dict(item) for item in self.items],
            "enqueued_count": self.enqueued_count,
            "omitted_count": self.omitted_count,
            "retention": "metadata_only_task_request_plan_and_connector_values_not_retained",
            "secret_material": "never_returned",
        }

    def to_dict(self) -> dict[str, Any]:
        return {**self._descriptor(), "job_digest": self.job_digest}


class AutonomousConnectorIntentFacade:
    """Route a user task to exact reviewed connector operations across all domains.

    ``route`` is a caller-owned deterministic or provider-assisted route function.  It is
    called only during planning.  The facade never lets a lexical match authorize a connector;
    selection, approval, replay, and executor boundaries remain owned by
    :class:`AutonomousConnectorOperationFacade`.
    """

    def __init__(
        self,
        operation_facade: AutonomousConnectorOperationFacade,
        route: Any,
    ) -> None:
        if not isinstance(operation_facade, AutonomousConnectorOperationFacade):
            raise ArgumentError("connector intent facade requires an operation facade")
        if not callable(route):
            raise ArgumentError("connector intent facade route must be callable")
        self.operation_facade = operation_facade
        self.route = route

    def plan(
        self,
        *,
        task: str,
        hints: Sequence[str] = (),
        request_by_domain: Mapping[str, Mapping[str, Any]] | None = None,
        capability: str | None = None,
        max_domains: int = 3,
        allow_cross_domain: bool = True,
        min_confidence: float = 0.25,
        min_margin: float = 0.10,
        approved: bool = False,
        selection_strategy: str = "lexicographic_connector_id",
        selection_signals: Mapping[str, Mapping[str, Any]] | None = None,
    ) -> AutonomousConnectorIntentPlan:
        if not isinstance(task, str) or not task.strip() or len(task.encode("utf-8")) > MAX_AUTONOMOUS_CONNECTOR_INTENT_TASK_BYTES:
            raise ArgumentError("connector intent task is outside its bound")
        if not isinstance(hints, Sequence) or isinstance(hints, (str, bytes)) or len(hints) > MAX_AUTONOMOUS_CONNECTOR_INTENT_HINTS:
            raise ArgumentError("connector intent hints are outside their bound")
        if any(not isinstance(hint, str) or not hint.strip() for hint in hints):
            raise ArgumentError("connector intent hints must be non-empty strings")
        if request_by_domain is not None and not isinstance(request_by_domain, Mapping):
            raise ArgumentError("connector intent request_by_domain must be an object")
        route = self.route(
            task=task,
            hints=tuple(hints),
            min_confidence=min_confidence,
            min_margin=min_margin,
            max_domains=max_domains,
            allow_cross_domain=allow_cross_domain,
        )
        route_dict = route.to_dict() if hasattr(route, "to_dict") else dict(route)
        selected_domains = tuple(getattr(route, "selected_domains", route_dict.get("selected_domains", ())))
        abstained = bool(getattr(route, "abstained", route_dict.get("abstained", False)))
        if abstained or not selected_domains:
            # The route itself is the authoritative refusal; no operation is prepared or
            # dispatched until a caller supplies a reviewed route.
            return AutonomousConnectorIntentPlan(
                task_digest=content_digest({"schema": AUTONOMOUS_CONNECTOR_INTENT_SCHEMA, "task": task}),
                route=route_dict,
                selected_domains=(),
                cross_domain=False,
                status="route_review_required",
                selections=(),
            )
        text = " ".join((task, *hints))
        selections: list[AutonomousConnectorIntentSelection] = []
        for domain in selected_domains:
            contracts = self.operation_facade.operation_registry.for_domain(domain)
            if not contracts:
                raise BrainRunError(f"no connector operation contract is registered for {domain!r}")
            scored = []
            for operation in contracts:
                selected_capability, score, matched, reason = _operation_intent(
                    operation, text, capability=capability
                )
                scored.append((score, operation.operation_id, operation, selected_capability, matched, reason))
            scored.sort(key=lambda row: (-row[0], row[1]))
            score, _operation_id, operation, selected_capability, matched, reason = scored[0]
            raw_request = {} if request_by_domain is None else request_by_domain.get(domain, {})
            operation_input = AutonomousConnectorOperationInput(
                domain=domain,
                capability=selected_capability,
                operation_id=operation.operation_id,
                request=raw_request,
                approved=approved,
                selection_strategy=selection_strategy,
                selection_signals=selection_signals,
            )
            operation_plan = self.operation_facade.plan(operation_input)
            selections.append(
                AutonomousConnectorIntentSelection(
                    domain=domain,
                    operation_id=operation.operation_id,
                    operation_digest=operation.operation_digest,
                    capability=selected_capability,
                    score=score,
                    matched_terms=matched,
                    selection_reason=reason,
                    operation_plan=operation_plan,
                )
            )
        status = (
            "route_review_required"
            if abstained
            else "connector_review_required"
            if any(item.operation_plan.status != "ready" for item in selections)
            else "ready"
        )
        return AutonomousConnectorIntentPlan(
            task_digest=content_digest({"schema": AUTONOMOUS_CONNECTOR_INTENT_SCHEMA, "task": task}),
            route=route_dict,
            selected_domains=tuple(selected_domains),
            cross_domain=len(selected_domains) > 1,
            status=status,
            selections=tuple(selections),
        )

    def execute(
        self,
        plan: AutonomousConnectorIntentPlan,
        *,
        task: str,
        hints: Sequence[str] = (),
        request_by_domain: Mapping[str, Mapping[str, Any]] | None = None,
        capability: str | None = None,
        max_domains: int = 3,
        allow_cross_domain: bool = True,
        min_confidence: float = 0.25,
        min_margin: float = 0.10,
        approved: bool = False,
        selection_strategy: str = "lexicographic_connector_id",
        selection_signals: Mapping[str, Mapping[str, Any]] | None = None,
        stop_on_error: bool = True,
    ) -> AutonomousConnectorIntentExecution:
        if not isinstance(plan, AutonomousConnectorIntentPlan):
            raise ArgumentError("connector intent execute requires a typed plan")
        current = self.plan(
            task=task,
            hints=hints,
            request_by_domain=request_by_domain,
            capability=capability,
            max_domains=max_domains,
            allow_cross_domain=allow_cross_domain,
            min_confidence=min_confidence,
            min_margin=min_margin,
            approved=approved,
            selection_strategy=selection_strategy,
            selection_signals=selection_signals,
        )
        if current.plan_digest != plan.plan_digest:
            raise ArgumentError("connector intent plan does not match the supplied transient task metadata")
        if current.status != "ready":
            return AutonomousConnectorIntentExecution(
                status=current.status,
                plan=current,
                items=tuple(
                    {"index": index, "domain": selection.domain, "status": "omitted", "plan_digest": selection.operation_plan.plan_digest}
                    for index, selection in enumerate(current.selections)
                ),
                executions=(),
            )
        items: list[dict[str, Any] | None] = [None] * len(current.selections)
        executions: list[AutonomousConnectorOperationExecution] = []
        for index, selection in enumerate(current.selections):
            if stop_on_error and any(item.get("status") in {"failed", "refused"} for item in items if item is not None):
                items[index] = {
                    "index": index,
                    "domain": selection.domain,
                    "status": "omitted",
                    "plan_digest": selection.operation_plan.plan_digest,
                }
                continue
            raw_request = {} if request_by_domain is None else request_by_domain.get(selection.domain, {})
            operation_input = AutonomousConnectorOperationInput(
                domain=selection.domain,
                capability=selection.capability,
                operation_id=selection.operation_id,
                request=raw_request,
                approved=approved,
                selection_strategy=selection_strategy,
                selection_signals=selection_signals,
            )
            try:
                execution = self.operation_facade.execute_planned(selection.operation_plan, operation_input)
                succeeded = execution.status in {"observed", "partial"}
                executions.append(execution)
                items[index] = {
                    "index": index,
                    "domain": selection.domain,
                    "status": "succeeded" if succeeded else "refused",
                    "plan_digest": selection.operation_plan.plan_digest,
                    "execution": execution.to_dict(),
                }
            except BaseException as error:
                items[index] = {
                    "index": index,
                    "domain": selection.domain,
                    "status": "failed",
                    "plan_digest": selection.operation_plan.plan_digest,
                    **_error_projection(error),
                }
        normalized = tuple(item for item in items if item is not None)
        failed = sum(item["status"] in {"failed", "refused"} for item in normalized)
        omitted = sum(item["status"] == "omitted" for item in normalized)
        status = "completed" if failed == 0 and omitted == 0 else "partial" if executions else "failed"
        return AutonomousConnectorIntentExecution(status, current, normalized, tuple(executions))

    def enqueue(
        self,
        plan: AutonomousConnectorIntentPlan,
        *,
        job_id: str,
        queue: InMemoryAutonomousConnectorWorkQueue,
        task: str,
        hints: Sequence[str] = (),
        request_by_domain: Mapping[str, Mapping[str, Any]] | None = None,
        capability: str | None = None,
        max_domains: int = 3,
        allow_cross_domain: bool = True,
        min_confidence: float = 0.25,
        min_margin: float = 0.10,
        approved: bool = False,
        selection_strategy: str = "lexicographic_connector_id",
        selection_signals: Mapping[str, Mapping[str, Any]] | None = None,
        max_attempts: int = 3,
        now: int | None = None,
    ) -> AutonomousConnectorIntentJob:
        """Submit a reviewed intent as bounded queue work.

        The task and request metadata are used only to recompute and bind the reviewed plan. The
        queue receives typed dispatch requests whose durable projection is digest-only.
        """

        if not isinstance(plan, AutonomousConnectorIntentPlan):
            raise ArgumentError("connector intent enqueue requires a typed plan")
        if not isinstance(queue, InMemoryAutonomousConnectorWorkQueue):
            raise ArgumentError("connector intent enqueue requires a typed work queue")
        _identifier("connector intent job_id", job_id)
        current = self.plan(
            task=task,
            hints=hints,
            request_by_domain=request_by_domain,
            capability=capability,
            max_domains=max_domains,
            allow_cross_domain=allow_cross_domain,
            min_confidence=min_confidence,
            min_margin=min_margin,
            approved=approved,
            selection_strategy=selection_strategy,
            selection_signals=selection_signals,
        )
        if current.plan_digest != plan.plan_digest:
            raise ArgumentError("connector intent job plan does not match the supplied transient task metadata")
        if len(current.selections) > MAX_AUTONOMOUS_CONNECTOR_INTENT_JOB_ITEMS:
            raise ArgumentError("connector intent job contains too many selections")
        if current.status != "ready":
            items = tuple(
                {
                    "index": index,
                    "domain": selection.domain,
                    "status": "omitted",
                    "work_id": None,
                    "operation_plan_digest": selection.operation_plan.plan_digest,
                    "queue_item_digest": None,
                }
                for index, selection in enumerate(current.selections)
            )
            return AutonomousConnectorIntentJob(
                job_id=job_id,
                plan_digest=current.plan_digest,
                status=current.status,
                items=items,
                enqueued_count=0,
                omitted_count=len(items),
            )
        items: list[dict[str, Any]] = []
        for index, selection in enumerate(current.selections):
            raw_request = {} if request_by_domain is None else request_by_domain.get(selection.domain, {})
            operation_input = AutonomousConnectorOperationInput(
                domain=selection.domain,
                capability=selection.capability,
                operation_id=selection.operation_id,
                request=raw_request,
                approved=approved,
                selection_strategy=selection_strategy,
                selection_signals=selection_signals,
            )
            selection_plan, dispatch = self.operation_facade.prepare_dispatch(
                selection.operation_plan,
                operation_input,
            )
            work_id = f"{job_id}-{index}"
            queued = queue.enqueue(
                work_id=work_id,
                operation_id=selection.operation_id,
                request=dispatch,
                selection_plan_digest=selection_plan.plan_digest,
                max_attempts=max_attempts,
                now=now,
            )
            items.append(
                {
                    "index": index,
                    "domain": selection.domain,
                    "status": "queued",
                    "work_id": queued.work_id,
                    "operation_plan_digest": selection.operation_plan.plan_digest,
                    "queue_item_digest": queued.item_digest,
                }
            )
        return AutonomousConnectorIntentJob(
            job_id=job_id,
            plan_digest=current.plan_digest,
            status="queued",
            items=tuple(items),
            enqueued_count=len(items),
            omitted_count=0,
        )

    def run_queued(
        self,
        plan: AutonomousConnectorIntentPlan,
        *,
        job_id: str,
        queue: InMemoryAutonomousConnectorWorkQueue,
        task: str,
        hints: Sequence[str] = (),
        request_by_domain: Mapping[str, Mapping[str, Any]] | None = None,
        capability: str | None = None,
        max_domains: int = 3,
        allow_cross_domain: bool = True,
        min_confidence: float = 0.25,
        min_margin: float = 0.10,
        approved: bool = False,
        selection_strategy: str = "lexicographic_connector_id",
        selection_signals: Mapping[str, Mapping[str, Any]] | None = None,
        worker_id: str = "connector-intent-worker",
        limit: int = 64,
        lease_ms: int = 30_000,
        now: int | None = None,
    ) -> dict[str, Any]:
        """Recover and execute queued intent work using caller-resupplied transient metadata."""

        if not isinstance(plan, AutonomousConnectorIntentPlan):
            raise ArgumentError("connector intent run_queued requires a typed plan")
        if not isinstance(queue, InMemoryAutonomousConnectorWorkQueue):
            raise ArgumentError("connector intent run_queued requires a typed work queue")
        _identifier("connector intent job_id", job_id)
        current = self.plan(
            task=task,
            hints=hints,
            request_by_domain=request_by_domain,
            capability=capability,
            max_domains=max_domains,
            allow_cross_domain=allow_cross_domain,
            min_confidence=min_confidence,
            min_margin=min_margin,
            approved=approved,
            selection_strategy=selection_strategy,
            selection_signals=selection_signals,
        )
        if current.plan_digest != plan.plan_digest:
            raise ArgumentError("connector intent worker plan does not match the supplied transient task metadata")

        selections = {
            f"{job_id}-{index}": selection
            for index, selection in enumerate(current.selections)
        }

        def rehydrate(item: AutonomousConnectorWorkItem) -> dict[str, Any]:
            selection = selections.get(item.work_id)
            if selection is None or selection.domain != item.domain or selection.operation_id != item.operation_id:
                raise ArgumentError("connector intent worker item is outside the reviewed plan")
            raw_request = {} if request_by_domain is None else request_by_domain.get(selection.domain, {})
            operation_input = AutonomousConnectorOperationInput(
                domain=selection.domain,
                capability=selection.capability,
                operation_id=selection.operation_id,
                request=raw_request,
                approved=approved,
                selection_strategy=selection_strategy,
                selection_signals=selection_signals,
            )
            selection_plan, dispatch = self.operation_facade.prepare_dispatch(
                selection.operation_plan,
                operation_input,
            )
            if dispatch.request_digest != item.request_digest or selection_plan.plan_digest != item.selection_plan_digest:
                raise ArgumentError("connector intent worker item identity does not match the reviewed plan")
            return {"plan": selection_plan, "request": dispatch}

        return AutonomousConnectorWorker(
            self.operation_facade.runtime,
            queue,
            rehydrate,
        ).run(
            worker_id=worker_id,
            limit=limit,
            lease_ms=lease_ms,
            now=now,
            work_ids=tuple(selections),
        )


class AutonomousConnectorIntentJobController:
    """Own the restart-safe lifecycle around a metadata-only intent job.

    The lower-level facade intentionally leaves persistence orchestration to the caller so it
    remains usable with databases, queues, and service-owned stores. This controller is the
    application-facing process boundary: startup must explicitly restore the queue, submission
    flushes one verified snapshot, execution flushes the post-worker state, and a partially
    enqueued job is rolled back to its pre-submit snapshot. It never persists the task, request
    values, plans, connector payloads, or credentials.
    """

    def __init__(
        self,
        intent: AutonomousConnectorIntentFacade,
        queue: InMemoryAutonomousConnectorWorkQueue,
        persistence: Any,
    ) -> None:
        if not isinstance(intent, AutonomousConnectorIntentFacade):
            raise ArgumentError("connector intent job controller requires an intent facade")
        if not isinstance(queue, InMemoryAutonomousConnectorWorkQueue):
            raise ArgumentError("connector intent job controller requires a typed work queue")
        self.intent = intent
        self.queue = queue
        self.persistence = AutonomousConnectorWorkQueuePersistenceCoordinator(queue, persistence)
        self._restored = False

    def _require_restored(self) -> None:
        if not self._restored:
            raise ArgumentError(
                "connector intent job controller must restore before enqueue or execution"
            )

    @staticmethod
    def _projection(*, status: str, snapshot_digest: str | None, items: int) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_CONNECTOR_INTENT_CONTROLLER_SCHEMA,
            "status": status,
            "snapshot_digest": snapshot_digest,
            "items": items,
            "persisted": True,
            "retention": "metadata_only_task_request_plan_connector_values_not_retained",
            "secret_material": "never_returned",
        }

    def restore(self) -> dict[str, Any]:
        """Restore the queue exactly once at process startup and verify its registry binding."""

        result = self.persistence.restore()
        self._restored = True
        return self._projection(
            status=result["status"],
            snapshot_digest=result["snapshot_digest"],
            items=result["items"],
        )

    def flush(self) -> dict[str, Any]:
        """Persist the current verified queue snapshot without accepting transient values."""

        self._require_restored()
        snapshot = self.persistence.flush()
        return self._projection(
            status="flushed",
            snapshot_digest=snapshot["snapshot_digest"],
            items=len(snapshot["items"]),
        )

    def enqueue(
        self,
        plan: AutonomousConnectorIntentPlan,
        intent_input: Mapping[str, Any],
    ) -> dict[str, Any]:
        """Submit and atomically persist one reviewed intent job.

        ``intent_input`` is transient and must contain the same ``job_id`` and task metadata
        used to build ``plan``. The returned projection is safe to store or send to a control
        plane; it contains the metadata-only job and the resulting snapshot digest only.
        """

        self._require_restored()
        if not isinstance(intent_input, Mapping):
            raise ArgumentError("connector intent controller input must be an object")
        if "job_id" not in intent_input:
            raise ArgumentError("connector intent controller input requires job_id")
        before = self.queue.snapshot()
        try:
            job = self.intent.enqueue(plan, queue=self.queue, **dict(intent_input))
            snapshot = self.persistence.flush()
        except BaseException:
            self.queue.restore(before)
            try:
                self.persistence.persistence.write(before)
            except BaseException:
                # Preserve the original failure. The queue remains restored in-process; a
                # caller-owned persistence adapter can surface its own I/O failure on retry.
                pass
            raise
        return {
            **self._projection(
                status="submitted",
                snapshot_digest=snapshot["snapshot_digest"],
                items=len(snapshot["items"]),
            ),
            "job": job.to_dict(),
        }

    def run_queued(
        self,
        plan: AutonomousConnectorIntentPlan,
        intent_input: Mapping[str, Any],
    ) -> dict[str, Any]:
        """Rehydrate transient input, execute the reviewed job, and persist worker state."""

        self._require_restored()
        if not isinstance(intent_input, Mapping):
            raise ArgumentError("connector intent controller input must be an object")
        if "job_id" not in intent_input:
            raise ArgumentError("connector intent controller input requires job_id")
        worker: dict[str, Any] | None = None
        try:
            worker = self.intent.run_queued(plan, queue=self.queue, **dict(intent_input))
        finally:
            # Persist leases, retry backoff, completions, and reconciliation states even when a
            # caller-owned rehydrator raises outside the worker's typed error projection.
            snapshot = self.persistence.flush()
        return {
            **self._projection(
                status="executed",
                snapshot_digest=snapshot["snapshot_digest"],
                items=len(snapshot["items"]),
            ),
            "worker": worker,
        }


__all__ = [
    "AUTONOMOUS_CONNECTOR_OPERATION_FACADE_SCHEMA",
    "AUTONOMOUS_CONNECTOR_OPERATION_BATCH_SCHEMA",
    "MAX_AUTONOMOUS_CONNECTOR_FACADE_BATCH",
    "MAX_AUTONOMOUS_CONNECTOR_FACADE_PARALLELISM",
    "MAX_AUTONOMOUS_CONNECTOR_FACADE_PARENT_DIGESTS",
    "MAX_AUTONOMOUS_CONNECTOR_FACADE_REQUEST_BYTES",
    "AUTONOMOUS_CONNECTOR_INTENT_SCHEMA",
    "MAX_AUTONOMOUS_CONNECTOR_INTENT_TASK_BYTES",
    "MAX_AUTONOMOUS_CONNECTOR_INTENT_HINTS",
    "AUTONOMOUS_CONNECTOR_INTENT_JOB_SCHEMA",
    "MAX_AUTONOMOUS_CONNECTOR_INTENT_JOB_ITEMS",
    "AUTONOMOUS_CONNECTOR_INTENT_CONTROLLER_SCHEMA",
    "AutonomousConnectorOperationInput",
    "AutonomousConnectorOperationPlan",
    "AutonomousConnectorOperationExecution",
    "AutonomousConnectorOperationBatchResult",
    "AutonomousConnectorOperationFacade",
    "AutonomousConnectorIntentSelection",
    "AutonomousConnectorIntentPlan",
    "AutonomousConnectorIntentExecution",
    "AutonomousConnectorIntentJob",
    "AutonomousConnectorIntentFacade",
    "AutonomousConnectorIntentJobController",
]
