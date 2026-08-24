"""Dependency-aware evidence execution for reviewed workflow portfolios.

Provider execution and evidence acquisition are separate external boundaries.  A portfolio can
finish its provider work while still needing source observations, projection, or evaluator
settlement.  This module composes the existing AutonomousEvidenceRuntime across the portfolio's
dependency waves without turning a provider result into evidence truth.

The supervisor is intentionally caller-owned at the value boundary.  Applications provide
acquisition, projection, evaluation, value rehydration, and per-item journals.  The returned
artifact retains only digests, bounded counts, statuses, and failure classes; raw evidence values
remain transient on the result object and never appear in JSON, checkpoints, or persistence.
"""

from __future__ import annotations

from collections.abc import Callable, Mapping, Sequence
from concurrent.futures import ThreadPoolExecutor
from dataclasses import dataclass, field
import inspect
import json
from typing import Any, Protocol

from .authoring import canonical_json, content_digest
from .autonomy import AUTONOMOUS_DOMAINS
from .autonomous_evidence import AutonomousEvidencePlan
from .autonomous_evidence_runtime import (
    AutonomousEvidenceRuntime,
    AutonomousEvidenceRuntimeJournal,
    AutonomousEvidenceRuntimeResult,
)
from .autonomous_workflow_portfolio import (
    AutonomousWorkflowPortfolioExecutionItem,
    AutonomousWorkflowPortfolioExecutionResult,
    AutonomousWorkflowPortfolioPlan,
)
from .errors import ArgumentError


AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_SCHEMA = (
    "bioprism-python-autonomous-workflow-portfolio-evidence/0.1"
)
AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_CHECKPOINT_SCHEMA = (
    "bioprism-python-autonomous-workflow-portfolio-evidence-checkpoint/0.1"
)
AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_CONTROLLER_SCHEMA = (
    "bioprism-python-autonomous-workflow-portfolio-evidence-controller/0.1"
)
MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_ITEMS = 64
MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_REQUESTS = 128
MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_PARALLELISM = 8
MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_CHECKPOINT_BYTES = 512_000
_RETENTION = "metadata_only;raw_evidence_values_caller_owned"
_SECRET_MATERIAL = "never_returned"
_ITEM_STATUSES = {
    "completed",
    "partial",
    "awaiting_evaluation",
    "failed",
    "reconciliation_required",
    "not_requested",
    "omitted",
}
_OVERALL_STATUSES = {
    "completed",
    "partial",
    "awaiting_evaluation",
    "failed",
    "reconciliation_required",
}
_CHECKPOINTABLE_STATUSES = {"completed", "failed", "omitted", "not_requested"}
_PROVIDER_STATUSES = {
    "succeeded",
    "failed",
    "blocked",
    "approval_required",
    "reconciliation_required",
    "not_started",
    "omitted",
}
_IDENTIFIER_CHARS = frozenset(
    "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_.:/-"
)


def _identifier(label: str, value: Any, *, maximum: int = 256) -> str:
    if (
        not isinstance(value, str)
        or not value
        or len(value.encode("utf-8")) > maximum
        or any(character not in _IDENTIFIER_CHARS for character in value)
    ):
        raise ArgumentError(f"{label} is outside its identifier contract")
    return value


def _text(label: str, value: Any, *, maximum: int = 512) -> str:
    if (
        not isinstance(value, str)
        or not value
        or "\x00" in value
        or len(value.encode("utf-8")) > maximum
    ):
        raise ArgumentError(f"{label} is outside its text contract")
    return value


def _digest(label: str, value: Any, *, allow_none: bool = False) -> str | None:
    if value is None and allow_none:
        return None
    if (
        not isinstance(value, str)
        or len(value) != 64
        or any(character not in "0123456789abcdef" for character in value)
    ):
        raise ArgumentError(f"{label} must be a lowercase SHA-256 digest")
    return value


def _sequence(label: str, value: Any, *, maximum: int) -> tuple[Any, ...]:
    if (
        isinstance(value, (str, bytes, bytearray))
        or not isinstance(value, Sequence)
        or len(value) > maximum
    ):
        raise ArgumentError(f"{label} must contain at most {maximum} entries")
    return tuple(value)


def _string_sequence(
    label: str,
    value: Any,
    *,
    maximum: int,
    identifiers: bool = True,
) -> tuple[str, ...]:
    values = _sequence(label, value, maximum=maximum)
    result = tuple(
        (
            _identifier(f"{label}[{index}]", item)
            if identifiers
            else _text(f"{label}[{index}]", item)
        )
        for index, item in enumerate(values)
    )
    if identifiers and len(set(result)) != len(result):
        raise ArgumentError(f"{label} must not contain duplicates")
    return result


def _json_digest(label: str, value: Any) -> str:
    try:
        return content_digest(value)
    except (TypeError, ValueError, OverflowError) as error:
        raise ArgumentError(f"{label} must be JSON-safe") from error


def _error_class(error: BaseException) -> str:
    name = error.__class__.__name__.strip()
    return (
        name
        if name
        and len(name.encode("utf-8")) <= 128
        and all(character in _IDENTIFIER_CHARS for character in name)
        else "PortfolioEvidenceError"
    )


def _runtime_field(runtime: Any, name: str, default: Any = None) -> Any:
    if isinstance(runtime, Mapping):
        return runtime.get(name, default)
    return getattr(runtime, name, default)


def _runtime_options(runtime: Any) -> dict[str, Any]:
    if runtime is None:
        raise ArgumentError("portfolio evidence runtime options are required")
    acquirer = _runtime_field(runtime, "acquirer")
    if acquirer is None or (
        not callable(getattr(acquirer, "acquire", None)) and not callable(acquirer)
    ):
        raise ArgumentError("portfolio evidence runtime requires an acquirer")
    evaluator = _runtime_field(runtime, "evaluator")
    reevaluate_pending = _runtime_field(runtime, "reevaluate_pending", False)
    if not isinstance(reevaluate_pending, bool):
        raise ArgumentError("portfolio evidence runtime reevaluate_pending must be boolean")
    options = {
        "acquirer": acquirer,
        "projector": _runtime_field(runtime, "projector"),
        "evaluator": evaluator,
        "rehydrate_value": _runtime_field(runtime, "rehydrate_value"),
        "reevaluate_pending": reevaluate_pending,
    }
    rehydrate_value = options["rehydrate_value"]
    if rehydrate_value is not None and not callable(rehydrate_value):
        raise ArgumentError("portfolio evidence runtime rehydrate_value must be callable")
    return options


def _evaluator_identity(runtime_options: Mapping[str, Any]) -> tuple[str | None, str | None]:
    evaluator = runtime_options.get("evaluator")
    if evaluator is None:
        return None, None
    evaluator_id = _identifier(
        "portfolio evidence evaluator_id",
        getattr(evaluator, "evaluator_id", None),
    )
    evaluator_version = _identifier(
        "portfolio evidence evaluator_version",
        getattr(evaluator, "evaluator_version", None),
    )
    return evaluator_id, evaluator_version


def _call_context_callback(callback: Callable[..., Any], context: Mapping[str, Any]) -> Any:
    """Call a caller callback using the richest compatible shape."""

    try:
        signature = inspect.signature(callback)
    except (TypeError, ValueError):
        return callback(**context)
    try:
        signature.bind(**context)
    except TypeError:
        try:
            signature.bind(context)
        except TypeError:
            signature.bind(*context.values())
            return callback(*context.values())
        return callback(context)
    return callback(**context)


def _provider_execution_map(
    execution: AutonomousWorkflowPortfolioExecutionResult,
) -> dict[str, AutonomousWorkflowPortfolioExecutionItem]:
    result: dict[str, AutonomousWorkflowPortfolioExecutionItem] = {}
    for item in execution.items:
        if item.item_id in result:
            raise ArgumentError("portfolio provider execution item ids must be unique")
        result[item.item_id] = item
    if set(result) != {item.item_id for item in execution.plan.items}:
        raise ArgumentError("portfolio provider execution items do not match its plan")
    return result


def _runtime_status(status: str) -> str:
    if status == "completed":
        return "completed"
    if status == "awaiting_evaluation":
        return "awaiting_evaluation"
    if status == "reconciliation_required":
        return "reconciliation_required"
    if status == "failed":
        return "failed"
    return "partial"


def _overall_status(items: Sequence["AutonomousWorkflowPortfolioEvidenceItem"]) -> str:
    requested = [
        item
        for item in items
        if item.status not in {"not_requested", "omitted"}
    ]
    if requested and all(item.status == "completed" for item in requested):
        return "completed"
    if any(item.status == "reconciliation_required" for item in items):
        return "reconciliation_required"
    if any(item.status == "awaiting_evaluation" for item in items):
        return "awaiting_evaluation"
    if requested and all(item.status == "failed" for item in requested):
        return "failed"
    return "partial"


@dataclass(frozen=True, slots=True)
class AutonomousWorkflowPortfolioEvidenceItemRequest:
    """Transient acquisition requests assigned to one reviewed portfolio item."""

    item_id: str
    requests: tuple[Mapping[str, Any], ...]

    def __post_init__(self) -> None:
        _identifier("portfolio evidence item_id", self.item_id)
        values = _sequence(
            f"portfolio evidence requests for {self.item_id}",
            self.requests,
            maximum=MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_REQUESTS,
        )
        normalized: list[Mapping[str, Any]] = []
        for index, value in enumerate(values):
            if not isinstance(value, Mapping):
                raise ArgumentError(
                    f"portfolio evidence request {self.item_id}[{index}] must be a mapping"
                )
            normalized.append(dict(value))
        object.__setattr__(self, "requests", tuple(normalized))

    @classmethod
    def from_value(cls, value: Any) -> "AutonomousWorkflowPortfolioEvidenceItemRequest":
        if isinstance(value, cls):
            return value
        if not isinstance(value, Mapping):
            raise ArgumentError("portfolio evidence item request must be an object")
        return cls(
            item_id=value.get("item_id"),
            requests=tuple(value.get("requests", ())),
        )

    def request_digest(self) -> str:
        return _json_digest(
            "portfolio evidence item request",
            {
                "schema": AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_CHECKPOINT_SCHEMA,
                "item_id": self.item_id,
                "requests": [dict(request) for request in self.requests],
            },
        )

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_SCHEMA,
            "item_id": self.item_id,
            "request_count": len(self.requests),
            "request_digest": self.request_digest(),
            "retention": _RETENTION,
            "secret_material": _SECRET_MATERIAL,
        }


@dataclass(frozen=True, slots=True)
class AutonomousWorkflowPortfolioEvidenceItem:
    """Metadata projection for one item; runtime values remain transient."""

    item_id: str
    domain: str
    provider_status: str
    status: str
    request_count: int
    runtime: AutonomousEvidenceRuntimeResult | None = field(
        default=None,
        repr=False,
        compare=False,
    )
    error_class: str | None = None

    def __post_init__(self) -> None:
        _identifier("portfolio evidence item_id", self.item_id)
        if self.domain not in AUTONOMOUS_DOMAINS:
            raise ArgumentError("portfolio evidence item domain is unsupported")
        if self.provider_status not in _PROVIDER_STATUSES:
            raise ArgumentError("portfolio evidence provider status is invalid")
        if self.status not in _ITEM_STATUSES:
            raise ArgumentError("portfolio evidence item status is invalid")
        if (
            isinstance(self.request_count, bool)
            or not isinstance(self.request_count, int)
            or not 0 <= self.request_count <= MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_REQUESTS
        ):
            raise ArgumentError("portfolio evidence item request_count is outside its bound")
        if self.runtime is not None and not isinstance(
            self.runtime, AutonomousEvidenceRuntimeResult
        ):
            raise ArgumentError("portfolio evidence item runtime is malformed")
        if self.error_class is not None:
            _identifier("portfolio evidence item error_class", self.error_class)
        if self.status == "completed" and self.runtime is None:
            raise ArgumentError("completed portfolio evidence item requires a runtime result")
        if self.status in {"failed", "reconciliation_required", "omitted"} and self.error_class is None:
            raise ArgumentError("held portfolio evidence item requires an error class")

    @property
    def result_digest(self) -> str | None:
        return None if self.runtime is None else self.runtime.result_digest

    def to_dict(self) -> dict[str, Any]:
        runtime = self.runtime
        return {
            "schema": AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_SCHEMA,
            "item_id": self.item_id,
            "domain": self.domain,
            "provider_status": self.provider_status,
            "status": self.status,
            "request_count": self.request_count,
            "completed_requirement_count": (
                0 if runtime is None else len(runtime.completed_requirement_ids)
            ),
            "pending_evaluation_count": (
                0
                if runtime is None
                else len(runtime.pending_evaluation_requirement_ids)
            ),
            "missing_requirement_count": (
                0 if runtime is None else len(runtime.missing_requirement_ids)
            ),
            "result_digest": self.result_digest,
            "receipt_digests": (
                []
                if runtime is None
                else [receipt.receipt_digest for receipt in runtime.receipts]
            ),
            "assessment_digests": (
                []
                if runtime is None
                else [
                    assessment.assessment_digest
                    for assessment in runtime.assessments
                ]
            ),
            "error_class": self.error_class,
            "retention": _RETENTION,
            "secret_material": _SECRET_MATERIAL,
        }


@dataclass(frozen=True, slots=True)
class AutonomousWorkflowPortfolioEvidenceProgress:
    """Metadata-only wave progress sent to a caller-owned sink."""

    plan: AutonomousWorkflowPortfolioPlan
    evidence_plan: AutonomousEvidencePlan
    items: tuple[AutonomousWorkflowPortfolioEvidenceItem, ...]
    status: str
    result_digest: str

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_SCHEMA,
            "status": self.status,
            "portfolio_plan_digest": self.plan.portfolio_digest,
            "evidence_plan_digest": self.evidence_plan.plan_digest,
            "items": [item.to_dict() for item in self.items],
            "result_digest": self.result_digest,
            "retention": _RETENTION,
            "secret_material": _SECRET_MATERIAL,
        }


def _metadata_descriptor(
    plan: AutonomousWorkflowPortfolioPlan,
    evidence_plan: AutonomousEvidencePlan,
    items: Sequence[AutonomousWorkflowPortfolioEvidenceItem],
    status: str,
) -> dict[str, Any]:
    return {
        "schema": AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_SCHEMA,
        "status": status,
        "portfolio_plan_digest": plan.portfolio_digest,
        "evidence_plan_digest": evidence_plan.plan_digest,
        "items": [item.to_dict() for item in items],
        "retention": _RETENTION,
        "secret_material": _SECRET_MATERIAL,
    }


def _metadata_digest(
    plan: AutonomousWorkflowPortfolioPlan,
    evidence_plan: AutonomousEvidencePlan,
    items: Sequence[AutonomousWorkflowPortfolioEvidenceItem],
    status: str,
) -> str:
    return content_digest(_metadata_descriptor(plan, evidence_plan, items, status))


@dataclass(frozen=True, slots=True)
class AutonomousWorkflowPortfolioEvidenceExecutionResult:
    """Portfolio evidence result with raw runtime values available only transiently."""

    plan: AutonomousWorkflowPortfolioPlan
    evidence_plan: AutonomousEvidencePlan
    items: tuple[AutonomousWorkflowPortfolioEvidenceItem, ...]
    status: str
    result_digest: str

    def __post_init__(self) -> None:
        if self.status not in _OVERALL_STATUSES:
            raise ArgumentError("portfolio evidence result status is invalid")
        if not isinstance(self.plan, AutonomousWorkflowPortfolioPlan):
            raise ArgumentError("portfolio evidence result plan is malformed")
        if not isinstance(self.evidence_plan, AutonomousEvidencePlan):
            raise ArgumentError("portfolio evidence result evidence plan is malformed")
        expected_ids = tuple(item.item_id for item in self.plan.items)
        actual_ids = tuple(item.item_id for item in self.items)
        if actual_ids != expected_ids:
            raise ArgumentError("portfolio evidence result items do not match plan order")
        _digest("portfolio evidence result_digest", self.result_digest)
        if _metadata_digest(self.plan, self.evidence_plan, self.items, self.status) != self.result_digest:
            raise ArgumentError("portfolio evidence result digest does not match its contents")

    def runtime_for(self, item_id: str) -> AutonomousEvidenceRuntimeResult | None:
        _identifier("portfolio evidence runtime item_id", item_id)
        return next((item.runtime for item in self.items if item.item_id == item_id), None)

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_SCHEMA,
            "status": self.status,
            "portfolio_plan_digest": self.plan.portfolio_digest,
            "evidence_plan_digest": self.evidence_plan.plan_digest,
            "completed_count": sum(item.status == "completed" for item in self.items),
            "partial_count": sum(item.status == "partial" for item in self.items),
            "awaiting_evaluation_count": sum(item.status == "awaiting_evaluation" for item in self.items),
            "failed_count": sum(item.status in {"failed", "reconciliation_required"} for item in self.items),
            "omitted_count": sum(item.status == "omitted" for item in self.items),
            "not_requested_count": sum(item.status == "not_requested" for item in self.items),
            "items": [item.to_dict() for item in self.items],
            "result_digest": self.result_digest,
            "retention": _RETENTION,
            "secret_material": _SECRET_MATERIAL,
        }


def _inject_item_metadata(
    request: Mapping[str, Any],
    provider_item: AutonomousWorkflowPortfolioExecutionItem,
) -> dict[str, Any]:
    metadata = request.get("metadata", {})
    if metadata is None:
        metadata = {}
    if not isinstance(metadata, Mapping):
        raise ArgumentError("portfolio evidence request metadata must be a mapping")
    normalized = dict(metadata)
    reserved = {
        "portfolio_item_id": provider_item.item_id,
        "portfolio_item_domain": provider_item.domain,
        "portfolio_provider_status": provider_item.status,
        "portfolio_provider_result_digest": provider_item.result_digest,
    }
    for key in reserved:
        if key in normalized:
            raise ArgumentError(f"portfolio evidence request metadata reserves {key}")
    return {**dict(request), "metadata": {**normalized, **reserved}}


def _parent_evidence_digests(
    provider_item: AutonomousWorkflowPortfolioExecutionItem,
    evidence: Mapping[str, AutonomousWorkflowPortfolioEvidenceItem],
) -> tuple[str, ...]:
    values: list[str] = []
    for dependency in provider_item.depends_on:
        item = evidence.get(dependency)
        digest = None if item is None else item.result_digest
        if digest is not None:
            values.append(digest)
    return tuple(values)


def _scoped_evidence_plan(
    agent: Any,
    domain: str,
    evidence_plan: AutonomousEvidencePlan,
) -> AutonomousEvidencePlan:
    result = agent.evidence_plan(
        (domain,),
        available_evidence=evidence_plan.available_evidence,
    )
    if not isinstance(result, AutonomousEvidencePlan):
        raise ArgumentError("portfolio evidence scoped plan is malformed")
    return result


def _validate_requests(
    plan: AutonomousWorkflowPortfolioPlan,
    evidence_plan: AutonomousEvidencePlan,
    raw_items: Any,
) -> dict[str, AutonomousWorkflowPortfolioEvidenceItemRequest]:
    values = _sequence(
        "portfolio evidence items",
        raw_items,
        maximum=MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_ITEMS,
    )
    plan_by_id = {item.item_id: item for item in plan.items}
    result: dict[str, AutonomousWorkflowPortfolioEvidenceItemRequest] = {}
    for raw in values:
        entry = AutonomousWorkflowPortfolioEvidenceItemRequest.from_value(raw)
        if entry.item_id in result:
            raise ArgumentError(f"portfolio evidence item {entry.item_id} is duplicated")
        plan_item = plan_by_id.get(entry.item_id)
        if plan_item is None:
            raise ArgumentError(f"portfolio evidence item {entry.item_id} is not in the reviewed plan")
        for index, request in enumerate(entry.requests):
            requirement_id = request.get("requirement_id")
            if not isinstance(requirement_id, str):
                raise ArgumentError(
                    f"portfolio evidence request {entry.item_id}[{index}] requirement_id is invalid"
                )
            requirement = next(
                (candidate for candidate in evidence_plan.requirements if candidate.requirement_id == requirement_id),
                None,
            )
            if requirement is None:
                raise ArgumentError(
                    f"portfolio evidence request {requirement_id} is not in the evidence plan"
                )
            if requirement.domain != plan_item.domain:
                raise ArgumentError(
                    f"portfolio evidence request {requirement_id} crosses item domain {plan_item.domain}"
                )
        result[entry.item_id] = entry
    return result


def _snapshot_items(
    plan: AutonomousWorkflowPortfolioPlan,
    providers: Mapping[str, AutonomousWorkflowPortfolioExecutionItem],
    transient: Mapping[str, AutonomousWorkflowPortfolioEvidenceItem],
) -> tuple[AutonomousWorkflowPortfolioEvidenceItem, ...]:
    rows: list[AutonomousWorkflowPortfolioEvidenceItem] = []
    for item in plan.items:
        if item.item_id in transient:
            rows.append(transient[item.item_id])
            continue
        provider = providers.get(item.item_id)
        rows.append(
            AutonomousWorkflowPortfolioEvidenceItem(
                item_id=item.item_id,
                domain=item.domain,
                provider_status="omitted" if provider is None else provider.status,
                status="omitted",
                request_count=0,
                error_class="portfolio_evidence_not_scheduled",
            )
        )
    return tuple(rows)


def _run_parallel(
    item_ids: Sequence[str],
    maximum: int,
    callback: Callable[[str], AutonomousWorkflowPortfolioEvidenceItem],
) -> tuple[AutonomousWorkflowPortfolioEvidenceItem, ...]:
    if not item_ids:
        return ()
    with ThreadPoolExecutor(
        max_workers=min(maximum, len(item_ids)),
        thread_name_prefix="aurora-portfolio-evidence",
    ) as pool:
        futures = {item_id: pool.submit(callback, item_id) for item_id in item_ids}
        return tuple(futures[item_id].result() for item_id in item_ids)


def execute_autonomous_workflow_portfolio_evidence(
    agent: Any,
    execution: AutonomousWorkflowPortfolioExecutionResult,
    *,
    items: Sequence[Mapping[str, Any] | AutonomousWorkflowPortfolioEvidenceItemRequest],
    runtime: Any,
    plan: AutonomousWorkflowPortfolioPlan | None = None,
    evidence_plan: AutonomousEvidencePlan | None = None,
    journal_for: Callable[..., AutonomousEvidenceRuntimeJournal | None] | None = None,
    max_parallelism: int = 4,
    stop_on_failure: bool = False,
    progress_sink: Callable[[AutonomousWorkflowPortfolioEvidenceProgress], Any] | None = None,
) -> AutonomousWorkflowPortfolioEvidenceExecutionResult:
    """Acquire and evaluate portfolio evidence in provider dependency waves."""

    if agent is None or not callable(getattr(agent, "evidence_plan", None)):
        raise ArgumentError("portfolio evidence execution requires an AutonomousAgent")
    if not isinstance(execution, AutonomousWorkflowPortfolioExecutionResult):
        raise ArgumentError("portfolio evidence execution requires a typed provider execution result")
    if (
        isinstance(max_parallelism, bool)
        or not isinstance(max_parallelism, int)
        or not 1 <= max_parallelism <= MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_PARALLELISM
    ):
        raise ArgumentError("portfolio evidence max_parallelism is outside its bound")
    if not isinstance(stop_on_failure, bool):
        raise ArgumentError("portfolio evidence stop_on_failure must be boolean")
    if progress_sink is not None and not callable(progress_sink):
        raise ArgumentError("portfolio evidence progress_sink must be callable")
    if journal_for is not None and not callable(journal_for):
        raise ArgumentError("portfolio evidence journal_for must be callable")
    runtime_options = _runtime_options(runtime)
    reviewed_plan = execution.plan if plan is None else plan
    if not isinstance(reviewed_plan, AutonomousWorkflowPortfolioPlan):
        raise ArgumentError("portfolio evidence plan is malformed")
    if reviewed_plan.portfolio_digest != execution.plan.portfolio_digest:
        raise ArgumentError("portfolio evidence plan does not match provider execution")
    providers = _provider_execution_map(execution)
    domains = tuple(dict.fromkeys(item.domain for item in reviewed_plan.items))
    reviewed_evidence_plan = agent.evidence_plan(domains) if evidence_plan is None else evidence_plan
    if not isinstance(reviewed_evidence_plan, AutonomousEvidencePlan):
        raise ArgumentError("portfolio evidence plan is malformed")
    if not set(domains).issubset(set(reviewed_evidence_plan.domains)):
        raise ArgumentError("portfolio evidence plan does not cover every portfolio domain")
    requests_by_item = _validate_requests(reviewed_plan, reviewed_evidence_plan, items)
    transient: dict[str, AutonomousWorkflowPortfolioEvidenceItem] = {}
    for item in reviewed_plan.items:
        provider = providers[item.item_id]
        request_entry = requests_by_item.get(item.item_id)
        if provider.status != "succeeded":
            transient[item.item_id] = AutonomousWorkflowPortfolioEvidenceItem(
                item_id=item.item_id,
                domain=item.domain,
                provider_status=provider.status,
                status="omitted",
                request_count=0 if request_entry is None else len(request_entry.requests),
                error_class="provider_execution_not_succeeded",
            )
        elif request_entry is None or not request_entry.requests:
            transient[item.item_id] = AutonomousWorkflowPortfolioEvidenceItem(
                item_id=item.item_id,
                domain=item.domain,
                provider_status=provider.status,
                status="not_requested",
                request_count=0,
            )

    def report_progress() -> None:
        if progress_sink is None:
            return
        snapshot = _snapshot_items(reviewed_plan, providers, transient)
        status = _overall_status(snapshot)
        progress_sink(
            AutonomousWorkflowPortfolioEvidenceProgress(
                plan=reviewed_plan,
                evidence_plan=reviewed_evidence_plan,
                items=snapshot,
                status=status,
                result_digest=_metadata_digest(reviewed_plan, reviewed_evidence_plan, snapshot, status),
            )
        )

    stopped = False
    plan_by_id = {item.item_id: item for item in reviewed_plan.items}
    for wave in reviewed_plan.dependency_graph.waves:
        wave_ids = tuple(item_id for item_id in wave if item_id not in transient)
        if stopped:
            for item_id in wave_ids:
                provider = providers[item_id]
                transient[item_id] = AutonomousWorkflowPortfolioEvidenceItem(
                    item_id=item_id,
                    domain=plan_by_id[item_id].domain,
                    provider_status=provider.status,
                    status="omitted",
                    request_count=len(requests_by_item[item_id].requests),
                    error_class="portfolio_evidence_stopped_after_failure",
                )
            report_progress()
            continue
        runnable: list[str] = []
        for item_id in wave_ids:
            provider = providers[item_id]
            if all(
                dependency not in transient
                or transient[dependency].status in {"completed", "not_requested"}
                for dependency in provider.depends_on
            ):
                runnable.append(item_id)
            else:
                transient[item_id] = AutonomousWorkflowPortfolioEvidenceItem(
                    item_id=item_id,
                    domain=plan_by_id[item_id].domain,
                    provider_status=provider.status,
                    status="omitted",
                    request_count=len(requests_by_item[item_id].requests),
                    error_class="evidence_dependency_not_completed",
                )

        def run_item(item_id: str) -> AutonomousWorkflowPortfolioEvidenceItem:
            plan_item = plan_by_id[item_id]
            provider = providers[item_id]
            request_entry = requests_by_item[item_id]
            try:
                scoped = _scoped_evidence_plan(agent, plan_item.domain, reviewed_evidence_plan)
                journal = None
                if journal_for is not None:
                    journal = _call_context_callback(
                        journal_for,
                        {
                            "item_id": item_id,
                            "domain": plan_item.domain,
                            "evidence_plan_digest": scoped.plan_digest,
                        },
                    )
                if journal is not None and not all(
                    callable(getattr(journal, name, None)) for name in ("append", "records")
                ):
                    raise ArgumentError("portfolio evidence item journal is malformed")
                evidence_runtime = AutonomousEvidenceRuntime(scoped, journal=journal)
                evidence_runtime.rehydrate()
                item_requests = tuple(
                    _inject_item_metadata(request, provider)
                    for request in request_entry.requests
                )
                runtime_result = evidence_runtime.execute(
                    item_requests,
                    acquirer=runtime_options["acquirer"],
                    projector=runtime_options["projector"],
                    evaluator=runtime_options["evaluator"],
                    rehydrate_value=runtime_options["rehydrate_value"],
                    parent_evidence_digests=_parent_evidence_digests(provider, transient),
                    reevaluate_pending=runtime_options["reevaluate_pending"],
                )
                return AutonomousWorkflowPortfolioEvidenceItem(
                    item_id=item_id,
                    domain=plan_item.domain,
                    provider_status=provider.status,
                    status=_runtime_status(runtime_result.status),
                    request_count=len(request_entry.requests),
                    runtime=runtime_result,
                )
            except Exception as error:
                return AutonomousWorkflowPortfolioEvidenceItem(
                    item_id=item_id,
                    domain=plan_item.domain,
                    provider_status=provider.status,
                    status="failed",
                    request_count=len(request_entry.requests),
                    error_class=_error_class(error),
                )

        for result in _run_parallel(tuple(runnable), max_parallelism, run_item):
            transient[result.item_id] = result
        report_progress()
        if stop_on_failure and any(
            transient[item_id].status in {"failed", "reconciliation_required"}
            for item_id in runnable
        ):
            stopped = True

    snapshot = _snapshot_items(reviewed_plan, providers, transient)
    status = _overall_status(snapshot)
    return AutonomousWorkflowPortfolioEvidenceExecutionResult(
        plan=reviewed_plan,
        evidence_plan=reviewed_evidence_plan,
        items=snapshot,
        status=status,
        result_digest=_metadata_digest(reviewed_plan, reviewed_evidence_plan, snapshot, status),
    )


def _provider_execution_digest(execution: AutonomousWorkflowPortfolioExecutionResult) -> str:
    return content_digest(
        {
            "schema": AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_CHECKPOINT_SCHEMA,
            "plan_digest": execution.plan.portfolio_digest,
            "admission_digest": execution.admission_digest,
            "checkpoint_digest": execution.checkpoint.checkpoint_digest,
            "items": [item.to_dict() for item in execution.items],
        }
    )


def _input_binding(
    plan: AutonomousWorkflowPortfolioPlan,
    entries: Mapping[str, AutonomousWorkflowPortfolioEvidenceItemRequest],
) -> tuple[tuple[str, ...], tuple[str, ...], str]:
    item_ids = tuple(item.item_id for item in plan.items)
    request_digests = tuple(
        entries.get(item_id, AutonomousWorkflowPortfolioEvidenceItemRequest(item_id, ())).request_digest()
        for item_id in item_ids
    )
    evidence_input_digest = content_digest(
        {
            "schema": AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_CHECKPOINT_SCHEMA,
            "items": [
                {"item_id": item_id, "request_digest": request_digest}
                for item_id, request_digest in zip(item_ids, request_digests)
            ],
        }
    )
    return item_ids, request_digests, evidence_input_digest


def _checkpoint_item_digest(item: AutonomousWorkflowPortfolioEvidenceItem) -> str:
    return content_digest(item.to_dict())


def _checkpoint_payload(
    *,
    job_id: str,
    execution: AutonomousWorkflowPortfolioExecutionResult,
    evidence_plan_digest: str,
    evidence_input_digest: str,
    item_ids: Sequence[str],
    item_request_digests: Sequence[str],
    settled_items: Sequence[AutonomousWorkflowPortfolioEvidenceItem],
    max_parallelism: int,
    stop_on_failure: bool,
    reevaluate_pending: bool,
    evaluator_id: str | None,
    evaluator_version: str | None,
    runtime_policy_digest: str | None,
    status: str,
) -> dict[str, Any]:
    settled = [
        item
        for item in settled_items
        if item.status in _CHECKPOINTABLE_STATUSES
        and not (item.status == "omitted" and item.error_class == "portfolio_evidence_not_scheduled")
    ]
    item_by_id = {item.item_id: item for item in settled}
    ordered_ids = [item_id for item_id in item_ids if item_id in item_by_id]
    ordered = [item_by_id[item_id] for item_id in ordered_ids]
    return {
        "schema": AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_CHECKPOINT_SCHEMA,
        "job_id": job_id,
        "portfolio_plan_digest": execution.plan.portfolio_digest,
        "admission_digest": execution.admission_digest,
        "provider_execution_digest": _provider_execution_digest(execution),
        "evidence_plan_digest": evidence_plan_digest,
        "evidence_input_digest": evidence_input_digest,
        "item_ids": list(item_ids),
        "item_request_digests": list(item_request_digests),
        "settled_item_ids": ordered_ids,
        "settled_item_statuses": [item.status for item in ordered],
        "settled_result_digests": [_checkpoint_item_digest(item) for item in ordered],
        "max_parallelism": max_parallelism,
        "stop_on_failure": stop_on_failure,
        "reevaluate_pending": reevaluate_pending,
        "evaluator_id": evaluator_id,
        "evaluator_version": evaluator_version,
        "runtime_policy_digest": runtime_policy_digest,
        "status": status,
        "retention": "metadata_only;raw_evidence_values_caller_owned",
        "secret_material": _SECRET_MATERIAL,
    }


@dataclass(frozen=True, slots=True)
class AutonomousWorkflowPortfolioEvidenceCheckpoint:
    """Digest-bound metadata checkpoint for portfolio evidence waves."""

    job_id: str
    portfolio_plan_digest: str
    admission_digest: str | None
    provider_execution_digest: str
    evidence_plan_digest: str
    evidence_input_digest: str
    item_ids: tuple[str, ...]
    item_request_digests: tuple[str, ...]
    settled_item_ids: tuple[str, ...]
    settled_item_statuses: tuple[str, ...]
    settled_result_digests: tuple[str, ...]
    max_parallelism: int
    stop_on_failure: bool
    reevaluate_pending: bool
    evaluator_id: str | None
    evaluator_version: str | None
    runtime_policy_digest: str | None
    status: str
    checkpoint_digest: str

    def __post_init__(self) -> None:
        _identifier("portfolio evidence checkpoint job_id", self.job_id)
        for label, value in (
            ("portfolio_plan_digest", self.portfolio_plan_digest),
            ("provider_execution_digest", self.provider_execution_digest),
            ("evidence_plan_digest", self.evidence_plan_digest),
            ("evidence_input_digest", self.evidence_input_digest),
        ):
            _digest(f"portfolio evidence checkpoint {label}", value)
        _digest("portfolio evidence checkpoint admission_digest", self.admission_digest, allow_none=True)
        _digest("portfolio evidence checkpoint runtime_policy_digest", self.runtime_policy_digest, allow_none=True)
        ids = _string_sequence(
            "portfolio evidence checkpoint item_ids",
            self.item_ids,
            maximum=MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_ITEMS,
        )
        request_digests = tuple(self.item_request_digests)
        if len(request_digests) != len(ids) or any(
            _digest("portfolio evidence checkpoint item_request_digest", value) is None
            for value in request_digests
        ):
            raise ArgumentError("portfolio evidence checkpoint request digests are invalid")
        settled_ids = _string_sequence(
            "portfolio evidence checkpoint settled_item_ids",
            self.settled_item_ids,
            maximum=len(ids),
        )
        if any(item_id not in ids for item_id in settled_ids):
            raise ArgumentError("portfolio evidence checkpoint settles an unknown item")
        if tuple(ids.index(item_id) for item_id in settled_ids) != tuple(
            sorted(ids.index(item_id) for item_id in settled_ids)
        ):
            raise ArgumentError("portfolio evidence checkpoint settled ids are not plan ordered")
        statuses = _string_sequence(
            "portfolio evidence checkpoint settled_item_statuses",
            self.settled_item_statuses,
            maximum=len(ids),
            identifiers=False,
        )
        if len(statuses) != len(settled_ids) or any(
            value not in _CHECKPOINTABLE_STATUSES for value in statuses
        ):
            raise ArgumentError("portfolio evidence checkpoint settled statuses are invalid")
        result_digests = tuple(self.settled_result_digests)
        if len(result_digests) != len(settled_ids) or any(
            _digest("portfolio evidence checkpoint settled_result_digest", value) is None
            for value in result_digests
        ):
            raise ArgumentError("portfolio evidence checkpoint result digests are invalid")
        if (
            isinstance(self.max_parallelism, bool)
            or not isinstance(self.max_parallelism, int)
            or not 1 <= self.max_parallelism <= MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_PARALLELISM
        ):
            raise ArgumentError("portfolio evidence checkpoint max_parallelism is outside its bound")
        if not isinstance(self.stop_on_failure, bool) or not isinstance(self.reevaluate_pending, bool):
            raise ArgumentError("portfolio evidence checkpoint controls are invalid")
        for label, value in (("evaluator_id", self.evaluator_id), ("evaluator_version", self.evaluator_version)):
            if value is not None:
                _identifier(f"portfolio evidence checkpoint {label}", value)
        if (self.evaluator_id is None) != (self.evaluator_version is None):
            raise ArgumentError("portfolio evidence checkpoint evaluator identity is incomplete")
        if self.status not in _OVERALL_STATUSES:
            raise ArgumentError("portfolio evidence checkpoint status is invalid")
        _digest("portfolio evidence checkpoint checkpoint_digest", self.checkpoint_digest)
        object.__setattr__(self, "item_ids", ids)
        object.__setattr__(self, "item_request_digests", request_digests)
        object.__setattr__(self, "settled_item_ids", settled_ids)
        object.__setattr__(self, "settled_item_statuses", statuses)
        object.__setattr__(self, "settled_result_digests", result_digests)

    def _payload(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_CHECKPOINT_SCHEMA,
            "job_id": self.job_id,
            "portfolio_plan_digest": self.portfolio_plan_digest,
            "admission_digest": self.admission_digest,
            "provider_execution_digest": self.provider_execution_digest,
            "evidence_plan_digest": self.evidence_plan_digest,
            "evidence_input_digest": self.evidence_input_digest,
            "item_ids": list(self.item_ids),
            "item_request_digests": list(self.item_request_digests),
            "settled_item_ids": list(self.settled_item_ids),
            "settled_item_statuses": list(self.settled_item_statuses),
            "settled_result_digests": list(self.settled_result_digests),
            "max_parallelism": self.max_parallelism,
            "stop_on_failure": self.stop_on_failure,
            "reevaluate_pending": self.reevaluate_pending,
            "evaluator_id": self.evaluator_id,
            "evaluator_version": self.evaluator_version,
            "runtime_policy_digest": self.runtime_policy_digest,
            "status": self.status,
            "retention": _RETENTION,
            "secret_material": _SECRET_MATERIAL,
        }

    def to_dict(self) -> dict[str, Any]:
        return {**self._payload(), "checkpoint_digest": self.checkpoint_digest}

    @classmethod
    def create_from_progress(
        cls,
        *,
        job_id: str,
        execution: AutonomousWorkflowPortfolioExecutionResult,
        progress: AutonomousWorkflowPortfolioEvidenceProgress,
        evidence_input_digest: str,
        item_ids: Sequence[str],
        item_request_digests: Sequence[str],
        max_parallelism: int,
        stop_on_failure: bool,
        reevaluate_pending: bool,
        evaluator_id: str | None,
        evaluator_version: str | None,
        runtime_policy_digest: str | None,
    ) -> "AutonomousWorkflowPortfolioEvidenceCheckpoint":
        payload = _checkpoint_payload(
            job_id=job_id,
            execution=execution,
            evidence_plan_digest=progress.evidence_plan.plan_digest,
            evidence_input_digest=evidence_input_digest,
            item_ids=item_ids,
            item_request_digests=item_request_digests,
            settled_items=progress.items,
            max_parallelism=max_parallelism,
            stop_on_failure=stop_on_failure,
            reevaluate_pending=reevaluate_pending,
            evaluator_id=evaluator_id,
            evaluator_version=evaluator_version,
            runtime_policy_digest=runtime_policy_digest,
            status=progress.status,
        )
        return cls(
            job_id=payload["job_id"],
            portfolio_plan_digest=payload["portfolio_plan_digest"],
            admission_digest=payload["admission_digest"],
            provider_execution_digest=payload["provider_execution_digest"],
            evidence_plan_digest=payload["evidence_plan_digest"],
            evidence_input_digest=payload["evidence_input_digest"],
            item_ids=tuple(payload["item_ids"]),
            item_request_digests=tuple(payload["item_request_digests"]),
            settled_item_ids=tuple(payload["settled_item_ids"]),
            settled_item_statuses=tuple(payload["settled_item_statuses"]),
            settled_result_digests=tuple(payload["settled_result_digests"]),
            max_parallelism=payload["max_parallelism"],
            stop_on_failure=payload["stop_on_failure"],
            reevaluate_pending=payload["reevaluate_pending"],
            evaluator_id=payload["evaluator_id"],
            evaluator_version=payload["evaluator_version"],
            runtime_policy_digest=payload["runtime_policy_digest"],
            status=payload["status"],
            checkpoint_digest=content_digest(payload),
        )

    @classmethod
    def from_dict(cls, value: Any) -> "AutonomousWorkflowPortfolioEvidenceCheckpoint":
        if not isinstance(value, Mapping):
            raise ArgumentError("portfolio evidence checkpoint must be an object")
        allowed = {
            "schema",
            "job_id",
            "portfolio_plan_digest",
            "admission_digest",
            "provider_execution_digest",
            "evidence_plan_digest",
            "evidence_input_digest",
            "item_ids",
            "item_request_digests",
            "settled_item_ids",
            "settled_item_statuses",
            "settled_result_digests",
            "max_parallelism",
            "stop_on_failure",
            "reevaluate_pending",
            "evaluator_id",
            "evaluator_version",
            "runtime_policy_digest",
            "status",
            "checkpoint_digest",
            "retention",
            "secret_material",
        }
        if set(value) != allowed or value.get("schema") != AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_CHECKPOINT_SCHEMA:
            raise ArgumentError("portfolio evidence checkpoint schema is invalid")
        if value.get("retention") != _RETENTION or value.get("secret_material") != _SECRET_MATERIAL:
            raise ArgumentError("portfolio evidence checkpoint retention markers are invalid")
        checkpoint = cls(
            job_id=value.get("job_id"),
            portfolio_plan_digest=value.get("portfolio_plan_digest"),
            admission_digest=value.get("admission_digest"),
            provider_execution_digest=value.get("provider_execution_digest"),
            evidence_plan_digest=value.get("evidence_plan_digest"),
            evidence_input_digest=value.get("evidence_input_digest"),
            item_ids=tuple(_sequence("portfolio evidence checkpoint item_ids", value.get("item_ids"), maximum=MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_ITEMS)),
            item_request_digests=tuple(_sequence("portfolio evidence checkpoint item_request_digests", value.get("item_request_digests"), maximum=MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_ITEMS)),
            settled_item_ids=tuple(_sequence("portfolio evidence checkpoint settled_item_ids", value.get("settled_item_ids"), maximum=MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_ITEMS)),
            settled_item_statuses=tuple(_sequence("portfolio evidence checkpoint settled_item_statuses", value.get("settled_item_statuses"), maximum=MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_ITEMS)),
            settled_result_digests=tuple(_sequence("portfolio evidence checkpoint settled_result_digests", value.get("settled_result_digests"), maximum=MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_ITEMS)),
            max_parallelism=value.get("max_parallelism"),
            stop_on_failure=value.get("stop_on_failure"),
            reevaluate_pending=value.get("reevaluate_pending"),
            evaluator_id=value.get("evaluator_id"),
            evaluator_version=value.get("evaluator_version"),
            runtime_policy_digest=value.get("runtime_policy_digest"),
            status=value.get("status"),
            checkpoint_digest=value.get("checkpoint_digest"),
        )
        if content_digest(checkpoint._payload()) != checkpoint.checkpoint_digest:
            raise ArgumentError("portfolio evidence checkpoint digest does not match its contents")
        return checkpoint


def _checkpoint_binding(
    checkpoint: AutonomousWorkflowPortfolioEvidenceCheckpoint,
    *,
    job_id: str,
    execution: AutonomousWorkflowPortfolioExecutionResult,
    plan: AutonomousWorkflowPortfolioPlan,
    evidence_plan: AutonomousEvidencePlan,
    item_ids: Sequence[str],
    item_request_digests: Sequence[str],
    evidence_input_digest: str,
    max_parallelism: int,
    stop_on_failure: bool,
    reevaluate_pending: bool,
    evaluator_id: str | None,
    evaluator_version: str | None,
    runtime_policy_digest: str | None,
) -> None:
    expected = {
        "job_id": job_id,
        "portfolio_plan_digest": plan.portfolio_digest,
        "admission_digest": execution.admission_digest,
        "provider_execution_digest": _provider_execution_digest(execution),
        "evidence_plan_digest": evidence_plan.plan_digest,
        "evidence_input_digest": evidence_input_digest,
        "item_ids": tuple(item_ids),
        "item_request_digests": tuple(item_request_digests),
        "max_parallelism": max_parallelism,
        "stop_on_failure": stop_on_failure,
        "reevaluate_pending": reevaluate_pending,
        "evaluator_id": evaluator_id,
        "evaluator_version": evaluator_version,
        "runtime_policy_digest": runtime_policy_digest,
    }
    actual = {key: getattr(checkpoint, key) for key in expected}
    actual["item_ids"] = tuple(actual["item_ids"])
    actual["item_request_digests"] = tuple(actual["item_request_digests"])
    if actual != expected:
        raise ArgumentError("portfolio evidence checkpoint does not match reviewed execution or evidence input")
    if checkpoint.status == "completed" and (
        len(checkpoint.settled_item_ids) != len(plan.items)
        or any(status != "completed" for status in checkpoint.settled_item_statuses)
    ):
        raise ArgumentError("completed portfolio evidence checkpoint is incomplete")


def _require_replay_journals(
    plan: AutonomousWorkflowPortfolioPlan,
    checkpoint: AutonomousWorkflowPortfolioEvidenceCheckpoint,
    journal_for: Callable[..., AutonomousEvidenceRuntimeJournal | None] | None,
) -> None:
    if "completed" not in checkpoint.settled_item_statuses:
        return
    if journal_for is None:
        raise ArgumentError("portfolio evidence resume requires journal_for for completed items")
    plan_by_id = {item.item_id: item for item in plan.items}
    for item_id, status in zip(checkpoint.settled_item_ids, checkpoint.settled_item_statuses):
        if status != "completed":
            continue
        item = plan_by_id[item_id]
        journal = _call_context_callback(
            journal_for,
            {
                "item_id": item_id,
                "domain": item.domain,
                "evidence_plan_digest": checkpoint.evidence_plan_digest,
            },
        )
        if journal is None or not all(
            callable(getattr(journal, name, None)) for name in ("append", "records")
        ):
            raise ArgumentError(f"portfolio evidence resume requires a journal for completed item {item_id}")


def execute_autonomous_workflow_portfolio_evidence_resumable(
    agent: Any,
    execution: AutonomousWorkflowPortfolioExecutionResult,
    *,
    job_id: str,
    items: Sequence[Mapping[str, Any] | AutonomousWorkflowPortfolioEvidenceItemRequest],
    runtime: Any,
    checkpoint_sink: Callable[[AutonomousWorkflowPortfolioEvidenceCheckpoint], Any],
    checkpoint: AutonomousWorkflowPortfolioEvidenceCheckpoint | Mapping[str, Any] | None = None,
    plan: AutonomousWorkflowPortfolioPlan | None = None,
    evidence_plan: AutonomousEvidencePlan | None = None,
    require_admission: bool = False,
    runtime_policy_digest: str | None = None,
    journal_for: Callable[..., AutonomousEvidenceRuntimeJournal | None] | None = None,
    max_parallelism: int = 4,
    stop_on_failure: bool = False,
    progress_sink: Callable[[AutonomousWorkflowPortfolioEvidenceProgress], Any] | None = None,
) -> AutonomousWorkflowPortfolioEvidenceExecutionResult:
    """Run portfolio evidence with digest-bound wave checkpoints and journal replay."""

    _identifier("portfolio evidence resumable job_id", job_id)
    if not isinstance(execution, AutonomousWorkflowPortfolioExecutionResult):
        raise ArgumentError("portfolio evidence resumable execution requires a typed provider result")
    if not callable(checkpoint_sink):
        raise ArgumentError("portfolio evidence resumable checkpoint_sink is required")
    if not isinstance(require_admission, bool):
        raise ArgumentError("portfolio evidence resumable require_admission must be boolean")
    _digest("portfolio evidence execution admission_digest", execution.admission_digest, allow_none=True)
    if require_admission and execution.admission_digest is None:
        raise ArgumentError("portfolio evidence resumable execution requires a reviewed admission")
    _digest("portfolio evidence runtime_policy_digest", runtime_policy_digest, allow_none=True)
    runtime_options = _runtime_options(runtime)
    evaluator_id, evaluator_version = _evaluator_identity(runtime_options)
    if not isinstance(stop_on_failure, bool):
        raise ArgumentError("portfolio evidence stop_on_failure must be boolean")
    reviewed_plan = execution.plan if plan is None else plan
    if not isinstance(reviewed_plan, AutonomousWorkflowPortfolioPlan):
        raise ArgumentError("portfolio evidence resumable plan is malformed")
    if reviewed_plan.portfolio_digest != execution.plan.portfolio_digest:
        raise ArgumentError("portfolio evidence resumable plan does not match provider execution")
    domains = tuple(dict.fromkeys(item.domain for item in reviewed_plan.items))
    reviewed_evidence_plan = agent.evidence_plan(domains) if evidence_plan is None else evidence_plan
    if not isinstance(reviewed_evidence_plan, AutonomousEvidencePlan):
        raise ArgumentError("portfolio evidence resumable evidence plan is malformed")
    requests_by_item = _validate_requests(reviewed_plan, reviewed_evidence_plan, items)
    item_ids, item_request_digests, evidence_input_digest = _input_binding(reviewed_plan, requests_by_item)
    restored = (
        None
        if checkpoint is None
        else (
            checkpoint
            if isinstance(checkpoint, AutonomousWorkflowPortfolioEvidenceCheckpoint)
            else AutonomousWorkflowPortfolioEvidenceCheckpoint.from_dict(checkpoint)
        )
    )
    if restored is not None:
        _checkpoint_binding(
            restored,
            job_id=job_id,
            execution=execution,
            plan=reviewed_plan,
            evidence_plan=reviewed_evidence_plan,
            item_ids=item_ids,
            item_request_digests=item_request_digests,
            evidence_input_digest=evidence_input_digest,
            max_parallelism=max_parallelism,
            stop_on_failure=stop_on_failure,
            reevaluate_pending=runtime_options["reevaluate_pending"],
            evaluator_id=evaluator_id,
            evaluator_version=evaluator_version,
            runtime_policy_digest=runtime_policy_digest,
        )
        _require_replay_journals(reviewed_plan, restored, journal_for)
        if "completed" in restored.settled_item_statuses and runtime_options["rehydrate_value"] is None:
            raise ArgumentError("portfolio evidence resume requires rehydrate_value for completed items")

    def persist(progress: AutonomousWorkflowPortfolioEvidenceProgress) -> None:
        next_checkpoint = AutonomousWorkflowPortfolioEvidenceCheckpoint.create_from_progress(
            job_id=job_id,
            execution=execution,
            progress=progress,
            evidence_input_digest=evidence_input_digest,
            item_ids=item_ids,
            item_request_digests=item_request_digests,
            max_parallelism=max_parallelism,
            stop_on_failure=stop_on_failure,
            reevaluate_pending=runtime_options["reevaluate_pending"],
            evaluator_id=evaluator_id,
            evaluator_version=evaluator_version,
            runtime_policy_digest=runtime_policy_digest,
        )
        checkpoint_sink(next_checkpoint)
        if progress_sink is not None:
            progress_sink(progress)

    return execute_autonomous_workflow_portfolio_evidence(
        agent,
        execution,
        items=items,
        runtime=runtime,
        plan=reviewed_plan,
        evidence_plan=reviewed_evidence_plan,
        journal_for=journal_for,
        max_parallelism=max_parallelism,
        stop_on_failure=stop_on_failure,
        progress_sink=persist,
    )


class AutonomousWorkflowPortfolioEvidenceCheckpointStore(Protocol):
    def read(self) -> Mapping[str, Any] | AutonomousWorkflowPortfolioEvidenceCheckpoint | None: ...
    def write(self, checkpoint: Mapping[str, Any] | AutonomousWorkflowPortfolioEvidenceCheckpoint) -> None: ...


class TransactionalAutonomousWorkflowPortfolioEvidenceCheckpointStore(
    AutonomousWorkflowPortfolioEvidenceCheckpointStore,
    Protocol,
):
    def write_if_unchanged(
        self,
        expected_checkpoint_digest: str | None,
        checkpoint: Mapping[str, Any] | AutonomousWorkflowPortfolioEvidenceCheckpoint,
    ) -> bool: ...


class InMemoryAutonomousWorkflowPortfolioEvidenceCheckpointStore:
    """Validated single-process checkpoint storage for local workers and tests."""

    def __init__(
        self,
        initial: Mapping[str, Any] | AutonomousWorkflowPortfolioEvidenceCheckpoint | None = None,
    ) -> None:
        self._checkpoint = (
            None
            if initial is None
            else AutonomousWorkflowPortfolioEvidenceCheckpoint.from_dict(
                initial.to_dict() if isinstance(initial, AutonomousWorkflowPortfolioEvidenceCheckpoint) else initial
            )
        )

    def read(self) -> dict[str, Any] | None:
        return None if self._checkpoint is None else self._checkpoint.to_dict()

    def write(self, checkpoint: Mapping[str, Any] | AutonomousWorkflowPortfolioEvidenceCheckpoint) -> None:
        self._checkpoint = (
            checkpoint
            if isinstance(checkpoint, AutonomousWorkflowPortfolioEvidenceCheckpoint)
            else AutonomousWorkflowPortfolioEvidenceCheckpoint.from_dict(checkpoint)
        )

    def write_if_unchanged(
        self,
        expected_checkpoint_digest: str | None,
        checkpoint: Mapping[str, Any] | AutonomousWorkflowPortfolioEvidenceCheckpoint,
    ) -> bool:
        current = None if self._checkpoint is None else self._checkpoint.checkpoint_digest
        if current != expected_checkpoint_digest:
            return False
        self.write(checkpoint)
        return True


class JsonAutonomousWorkflowPortfolioEvidenceCheckpointPersistence:
    """Canonical JSON persistence for metadata-only portfolio evidence checkpoints."""

    def __init__(self, store: Any, *, max_bytes: int = MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_CHECKPOINT_BYTES) -> None:
        if not all(callable(getattr(store, name, None)) for name in ("read", "write")):
            raise ArgumentError("portfolio evidence checkpoint persistence requires a text store")
        if (
            isinstance(max_bytes, bool)
            or not isinstance(max_bytes, int)
            or not 1 <= max_bytes <= MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_CHECKPOINT_BYTES
        ):
            raise ArgumentError("portfolio evidence checkpoint max_bytes is outside its bound")
        self.store = store
        self.max_bytes = max_bytes

    def _encode(self, checkpoint: Mapping[str, Any] | AutonomousWorkflowPortfolioEvidenceCheckpoint) -> str:
        validated = checkpoint if isinstance(checkpoint, AutonomousWorkflowPortfolioEvidenceCheckpoint) else AutonomousWorkflowPortfolioEvidenceCheckpoint.from_dict(checkpoint)
        encoded = canonical_json(validated.to_dict())
        if len(encoded.encode("utf-8")) > self.max_bytes:
            raise ArgumentError("portfolio evidence checkpoint exceeds its byte bound")
        return encoded

    def read(self) -> dict[str, Any] | None:
        encoded = self.store.read()
        if encoded is None:
            return None
        if not isinstance(encoded, str) or len(encoded.encode("utf-8")) > self.max_bytes:
            raise ArgumentError("portfolio evidence checkpoint text exceeds its bound")
        try:
            raw = json.loads(encoded)
        except (TypeError, ValueError) as error:
            raise ArgumentError("portfolio evidence checkpoint text is invalid JSON") from error
        normalized = self._encode(raw)
        if encoded != normalized:
            raise ArgumentError("portfolio evidence checkpoint text is not canonical")
        return json.loads(normalized)

    def write(self, checkpoint: Mapping[str, Any] | AutonomousWorkflowPortfolioEvidenceCheckpoint) -> None:
        self.store.write(self._encode(checkpoint))


class TransactionalJsonAutonomousWorkflowPortfolioEvidenceCheckpointPersistence(
    JsonAutonomousWorkflowPortfolioEvidenceCheckpointPersistence,
):
    """Canonical checkpoint JSON with stale-writer compare-and-swap fencing."""

    def __init__(self, store: Any, *, max_bytes: int = MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_CHECKPOINT_BYTES) -> None:
        super().__init__(store, max_bytes=max_bytes)
        if not callable(getattr(store, "write_if_unchanged", None)):
            raise ArgumentError("transactional portfolio evidence checkpoint store requires write_if_unchanged")
        self.store = store

    def write_if_unchanged(
        self,
        expected_checkpoint_digest: str | None,
        checkpoint: Mapping[str, Any] | AutonomousWorkflowPortfolioEvidenceCheckpoint,
    ) -> bool:
        _digest("portfolio evidence expected checkpoint digest", expected_checkpoint_digest, allow_none=True)
        return self.store.write_if_unchanged(expected_checkpoint_digest, self._encode(checkpoint))


class AutonomousWorkflowPortfolioEvidenceController:
    """Serialize local resumable evidence runs and fence stale checkpoint writers."""

    def __init__(
        self,
        agent: Any,
        execution: AutonomousWorkflowPortfolioExecutionResult,
        *,
        job_id: str,
        persistence: AutonomousWorkflowPortfolioEvidenceCheckpointStore,
        require_admission: bool = True,
    ) -> None:
        _identifier("portfolio evidence controller job_id", job_id)
        if not isinstance(execution, AutonomousWorkflowPortfolioExecutionResult):
            raise ArgumentError("portfolio evidence controller execution is malformed")
        if not all(callable(getattr(persistence, name, None)) for name in ("read", "write")):
            raise ArgumentError("portfolio evidence controller persistence is malformed")
        self.agent = agent
        self.execution = execution
        self.job_id = job_id
        self.persistence = persistence
        self.require_admission = require_admission
        self._checkpoint: AutonomousWorkflowPortfolioEvidenceCheckpoint | None = None
        self._expected_digest: str | None = None

    def restore(self) -> AutonomousWorkflowPortfolioEvidenceCheckpoint | None:
        raw = self.persistence.read()
        self._checkpoint = (
            None
            if raw is None
            else (
                raw
                if isinstance(raw, AutonomousWorkflowPortfolioEvidenceCheckpoint)
                else AutonomousWorkflowPortfolioEvidenceCheckpoint.from_dict(raw)
            )
        )
        if self._checkpoint is not None:
            if self._checkpoint.job_id != self.job_id:
                raise ArgumentError("portfolio evidence controller checkpoint job_id does not match")
            self._expected_digest = self._checkpoint.checkpoint_digest
        else:
            self._expected_digest = None
        return self._checkpoint

    def run(
        self,
        *,
        items: Sequence[Mapping[str, Any] | AutonomousWorkflowPortfolioEvidenceItemRequest],
        runtime: Any,
        **options: Any,
    ) -> AutonomousWorkflowPortfolioEvidenceExecutionResult:
        if any(key in options for key in {"job_id", "checkpoint", "checkpoint_sink"}):
            raise ArgumentError("portfolio evidence controller owns job_id, checkpoint, and checkpoint_sink")
        if self._checkpoint is None and self._expected_digest is None:
            self.restore()

        def persist(checkpoint: AutonomousWorkflowPortfolioEvidenceCheckpoint) -> None:
            writer = getattr(self.persistence, "write_if_unchanged", None)
            if callable(writer):
                if not writer(self._expected_digest, checkpoint):
                    raise ArgumentError("portfolio evidence checkpoint compare-and-swap conflict")
            else:
                self.persistence.write(checkpoint)
            self._checkpoint = checkpoint
            self._expected_digest = checkpoint.checkpoint_digest

        return execute_autonomous_workflow_portfolio_evidence_resumable(
            self.agent,
            self.execution,
            job_id=self.job_id,
            items=items,
            runtime=runtime,
            checkpoint_sink=persist,
            checkpoint=self._checkpoint,
            require_admission=self.require_admission,
            **options,
        )

    def projection(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_CONTROLLER_SCHEMA,
            "job_id": self.job_id,
            "portfolio_plan_digest": self.execution.plan.portfolio_digest,
            "admission_digest": self.execution.admission_digest,
            "checkpoint_digest": None if self._checkpoint is None else self._checkpoint.checkpoint_digest,
            "status": "empty" if self._checkpoint is None else self._checkpoint.status,
            "retention": _RETENTION,
            "secret_material": _SECRET_MATERIAL,
        }


def validate_autonomous_workflow_portfolio_evidence_checkpoint(
    value: Any,
) -> AutonomousWorkflowPortfolioEvidenceCheckpoint:
    """Validate a caller-rehydrated metadata-only evidence checkpoint."""

    return AutonomousWorkflowPortfolioEvidenceCheckpoint.from_dict(value)


__all__ = [
    "AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_SCHEMA",
    "AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_CHECKPOINT_SCHEMA",
    "AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_CONTROLLER_SCHEMA",
    "MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_ITEMS",
    "MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_REQUESTS",
    "MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_PARALLELISM",
    "MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_EVIDENCE_CHECKPOINT_BYTES",
    "AutonomousWorkflowPortfolioEvidenceItemRequest",
    "AutonomousWorkflowPortfolioEvidenceItem",
    "AutonomousWorkflowPortfolioEvidenceProgress",
    "AutonomousWorkflowPortfolioEvidenceExecutionResult",
    "AutonomousWorkflowPortfolioEvidenceCheckpoint",
    "AutonomousWorkflowPortfolioEvidenceCheckpointStore",
    "TransactionalAutonomousWorkflowPortfolioEvidenceCheckpointStore",
    "InMemoryAutonomousWorkflowPortfolioEvidenceCheckpointStore",
    "JsonAutonomousWorkflowPortfolioEvidenceCheckpointPersistence",
    "TransactionalJsonAutonomousWorkflowPortfolioEvidenceCheckpointPersistence",
    "AutonomousWorkflowPortfolioEvidenceController",
    "execute_autonomous_workflow_portfolio_evidence",
    "execute_autonomous_workflow_portfolio_evidence_resumable",
    "validate_autonomous_workflow_portfolio_evidence_checkpoint",
]
