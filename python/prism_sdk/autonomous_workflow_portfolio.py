"""Digest-bound composition and replay verification for multiple autonomous workflows.

The regular Python runtime already executes one prepared workflow or one reviewed cross-domain
fan-out.  This module adds the missing portfolio-level planning boundary: an embedding service can
submit a bounded set of explicit domain tasks, inspect dependency waves and coverage, persist only
digests/metadata, and replay the plan after restart before handing individual blueprints to the
existing execution kernel.

The planner never invokes a provider, discovers a model, reads a connector, executes a tool, or
accepts a credential.  Task text, hints, and context are used transiently to build each existing
``AutonomousTaskBlueprint`` and to compute request digests; they are deliberately absent from the
returned plan and verification projection.
"""

from __future__ import annotations

from dataclasses import dataclass, field
import json
from collections.abc import Mapping, Sequence
from concurrent.futures import ThreadPoolExecutor
from typing import Any, TYPE_CHECKING

from .authoring import content_digest
from .autonomy import (
    AUTONOMOUS_DOMAINS,
    AutonomousTaskBlueprint,
    BrainRunError,
)

if TYPE_CHECKING:
    from .autonomy import AutonomousAgent


AUTONOMOUS_WORKFLOW_PORTFOLIO_SCHEMA = "bioprism-python-autonomous-workflow-portfolio/0.1"
AUTONOMOUS_WORKFLOW_PORTFOLIO_VERIFICATION_SCHEMA = (
    "bioprism-python-autonomous-workflow-portfolio-verification/0.1"
)
MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_ITEMS = 64
MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_DEPENDENCIES = 16
MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_HINTS = 32
MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_CONTEXT_BYTES = 64_000
MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_STAGE_IDS = 64
MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_CAPABILITIES = 128
_PORTFOLIO_RETENTION = "metadata_only_task_and_blueprint_values_not_retained"
_PORTFOLIO_SECRET_MATERIAL = "never_returned"
_IDENTIFIER_CHARS = frozenset("abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_.:-")


def _identifier(label: str, value: Any, *, maximum: int = 128) -> str:
    if (
        not isinstance(value, str)
        or not value
        or len(value.encode("utf-8")) > maximum
        or any(character not in _IDENTIFIER_CHARS for character in value)
    ):
        raise BrainRunError(f"{label} is outside its identifier contract")
    return value


def _text(label: str, value: Any, *, maximum: int) -> str:
    if (
        not isinstance(value, str)
        or not value
        or len(value.encode("utf-8")) > maximum
        or any(ord(character) < 32 for character in value)
    ):
        raise BrainRunError(f"{label} is outside its bounded text contract")
    return value


def _string_list(label: str, value: Any, *, maximum: int, item_maximum: int = 2_048) -> tuple[str, ...]:
    if value is None:
        return ()
    if isinstance(value, (str, bytes)) or not isinstance(value, Sequence) or len(value) > maximum:
        raise BrainRunError(f"{label} must contain at most {maximum} entries")
    result = tuple(_text(label, item, maximum=item_maximum) for item in value)
    if len(set(result)) != len(result):
        raise BrainRunError(f"{label} must not contain duplicates")
    return result


def _sequence_value(label: str, value: Any) -> tuple[Any, ...]:
    """Normalize a JSON array while turning malformed input into the SDK error type."""

    if value is None:
        return ()
    if isinstance(value, (str, bytes)) or not isinstance(value, Sequence):
        raise BrainRunError(f"{label} must be an array")
    return tuple(value)


def _safe_context(value: Any) -> dict[str, Any]:
    if value is None:
        return {}
    if not isinstance(value, Mapping):
        raise BrainRunError("workflow portfolio item context must be a mapping")
    try:
        encoded = json.dumps(dict(value), ensure_ascii=False, allow_nan=False, separators=(",", ":"))
    except (TypeError, ValueError) as error:
        raise BrainRunError("workflow portfolio item context must be JSON-safe") from error
    if len(encoded.encode("utf-8")) > MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_CONTEXT_BYTES:
        raise BrainRunError("workflow portfolio item context exceeds its bounded size")
    return dict(value)


@dataclass(frozen=True, slots=True)
class AutonomousWorkflowPortfolioItemRequest:
    """Transient caller input for one explicit workflow portfolio item."""

    task: str
    domain: str
    id: str | None = None
    capability: str | None = None
    depends_on: tuple[str, ...] = ()
    hints: tuple[str, ...] = ()
    context: Mapping[str, Any] = field(default_factory=dict)

    def __post_init__(self) -> None:
        _text("workflow portfolio task", self.task, maximum=32_000)
        _identifier("workflow portfolio domain", self.domain)
        if self.domain not in AUTONOMOUS_DOMAINS:
            raise BrainRunError(f"workflow portfolio domain is unsupported: {self.domain!r}")
        if self.id is not None:
            _identifier("workflow portfolio item id", self.id)
        if self.capability is not None:
            _text("workflow portfolio capability", self.capability, maximum=256)
        dependencies = _string_list(
            "workflow portfolio depends_on",
            self.depends_on,
            maximum=MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_DEPENDENCIES,
        )
        hints = _string_list(
            "workflow portfolio hints",
            self.hints,
            maximum=MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_HINTS,
        )
        if self.id is not None and self.id in dependencies:
            raise BrainRunError("workflow portfolio item cannot depend on itself")
        object.__setattr__(self, "depends_on", dependencies)
        object.__setattr__(self, "hints", hints)
        object.__setattr__(self, "context", _safe_context(self.context))

    def normalized_id(self, index: int) -> str:
        return self.id or f"item-{index + 1}"

    def request_payload(self, item_id: str) -> dict[str, Any]:
        """Return the transient request identity used for the request digest."""

        return {
            "schema": "bioprism-python-autonomous-workflow-portfolio-request/0.1",
            "item_id": item_id,
            "task": self.task,
            "domain": self.domain,
            "capability": self.capability,
            "depends_on": list(self.depends_on),
            "hints": list(self.hints),
            "context": dict(self.context),
        }


@dataclass(frozen=True, slots=True)
class AutonomousWorkflowPortfolioItem:
    """Metadata-only projection of one compiled workflow item."""

    item_id: str
    domain: str
    capability: str | None
    depends_on: tuple[str, ...]
    task_digest: str
    request_digest: str
    route_digest: str | None
    workflow_id: str | None
    workflow_digest: str | None
    plan_digest: str | None
    evidence_plan_digest: str | None
    stage_ids: tuple[str, ...]
    required_capabilities: tuple[str, ...]
    status: str
    error_class: str | None = None

    def __post_init__(self) -> None:
        _identifier("workflow portfolio item id", self.item_id)
        _identifier("workflow portfolio item domain", self.domain)
        if self.domain not in AUTONOMOUS_DOMAINS:
            raise BrainRunError("workflow portfolio item domain is unsupported")
        if self.capability is not None:
            _text("workflow portfolio item capability", self.capability, maximum=256)
        dependencies = _string_list(
            "workflow portfolio item dependencies",
            self.depends_on,
            maximum=MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_DEPENDENCIES,
        )
        for name, value in (
            ("task_digest", self.task_digest),
            ("request_digest", self.request_digest),
        ):
            if not isinstance(value, str) or len(value) != 64 or any(character not in "0123456789abcdef" for character in value):
                raise BrainRunError(f"workflow portfolio item {name} is not a SHA-256 digest")
        for name, value in (
            ("route_digest", self.route_digest),
            ("workflow_digest", self.workflow_digest),
            ("plan_digest", self.plan_digest),
            ("evidence_plan_digest", self.evidence_plan_digest),
        ):
            if value is not None and (
                not isinstance(value, str)
                or len(value) != 64
                or any(character not in "0123456789abcdef" for character in value)
            ):
                raise BrainRunError(f"workflow portfolio item {name} is not a digest or None")
        stages = _string_list(
            "workflow portfolio item stage_ids",
            self.stage_ids,
            maximum=MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_STAGE_IDS,
        )
        capabilities = _string_list(
            "workflow portfolio item required_capabilities",
            self.required_capabilities,
            maximum=MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_CAPABILITIES,
        )
        if self.status not in {"ready", "blocked", "failed", "route_review_required"}:
            raise BrainRunError("workflow portfolio item status is invalid")
        if self.error_class is not None:
            _identifier("workflow portfolio item error_class", self.error_class)
        if self.status == "ready" and (
            self.workflow_id is None
            or self.workflow_digest is None
            or self.plan_digest is None
            or self.evidence_plan_digest is None
            or not stages
        ):
            raise BrainRunError("ready workflow portfolio item is missing compiled workflow metadata")
        if self.status != "ready" and self.error_class is None:
            raise BrainRunError("non-ready workflow portfolio item requires an error class")
        object.__setattr__(self, "depends_on", dependencies)
        object.__setattr__(self, "stage_ids", stages)
        object.__setattr__(self, "required_capabilities", capabilities)

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_WORKFLOW_PORTFOLIO_SCHEMA,
            "item_id": self.item_id,
            "domain": self.domain,
            "capability": self.capability,
            "depends_on": list(self.depends_on),
            "task_digest": self.task_digest,
            "request_digest": self.request_digest,
            "route_digest": self.route_digest,
            "workflow_id": self.workflow_id,
            "workflow_digest": self.workflow_digest,
            "plan_digest": self.plan_digest,
            "evidence_plan_digest": self.evidence_plan_digest,
            "stage_ids": list(self.stage_ids),
            "required_capabilities": list(self.required_capabilities),
            "status": self.status,
            "error_class": self.error_class,
            "retention": _PORTFOLIO_RETENTION,
            "secret_material": _PORTFOLIO_SECRET_MATERIAL,
        }


@dataclass(frozen=True, slots=True)
class AutonomousWorkflowPortfolioCoverage:
    requested_domains: tuple[str, ...]
    ready_domains: tuple[str, ...]
    missing_domains: tuple[str, ...]
    duplicate_domain_items: tuple[str, ...]
    requested_item_count: int
    ready_item_count: int
    blocked_item_count: int
    failed_item_count: int
    complete: bool

    def to_dict(self) -> dict[str, Any]:
        return {
            "requested_domains": list(self.requested_domains),
            "ready_domains": list(self.ready_domains),
            "missing_domains": list(self.missing_domains),
            "duplicate_domain_items": list(self.duplicate_domain_items),
            "requested_item_count": self.requested_item_count,
            "ready_item_count": self.ready_item_count,
            "blocked_item_count": self.blocked_item_count,
            "failed_item_count": self.failed_item_count,
            "complete": self.complete,
        }


@dataclass(frozen=True, slots=True)
class AutonomousWorkflowPortfolioDependencyGraph:
    topological_order: tuple[str, ...]
    waves: tuple[tuple[str, ...], ...]
    cycle_item_ids: tuple[str, ...]
    edge_count: int

    def to_dict(self) -> dict[str, Any]:
        return {
            "topological_order": list(self.topological_order),
            "waves": [list(wave) for wave in self.waves],
            "cycle_item_ids": list(self.cycle_item_ids),
            "edge_count": self.edge_count,
        }


@dataclass(frozen=True, slots=True)
class AutonomousWorkflowPortfolioPlan:
    """Digest-bound portfolio plan safe to persist or hand to a review UI."""

    status: str
    require_all_domains: bool
    allow_partial: bool
    items: tuple[AutonomousWorkflowPortfolioItem, ...]
    coverage: AutonomousWorkflowPortfolioCoverage
    dependency_graph: AutonomousWorkflowPortfolioDependencyGraph
    portfolio_digest: str

    def __post_init__(self) -> None:
        if self.status not in {"ready", "partial", "blocked"}:
            raise BrainRunError("workflow portfolio status is invalid")
        if not isinstance(self.require_all_domains, bool) or not isinstance(self.allow_partial, bool):
            raise BrainRunError("workflow portfolio policy flags must be booleans")
        if not 1 <= len(self.items) <= MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_ITEMS:
            raise BrainRunError("workflow portfolio item count is outside its bound")
        if len({item.item_id for item in self.items}) != len(self.items):
            raise BrainRunError("workflow portfolio item ids must be unique")
        if not isinstance(self.coverage, AutonomousWorkflowPortfolioCoverage):
            raise BrainRunError("workflow portfolio coverage is invalid")
        if not isinstance(self.dependency_graph, AutonomousWorkflowPortfolioDependencyGraph):
            raise BrainRunError("workflow portfolio dependency graph is invalid")
        if len(self.portfolio_digest) != 64 or any(character not in "0123456789abcdef" for character in self.portfolio_digest):
            raise BrainRunError("workflow portfolio digest is invalid")

    def _digest_payload(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_WORKFLOW_PORTFOLIO_SCHEMA,
            "status": self.status,
            "policy": {
                "require_all_domains": self.require_all_domains,
                "allow_partial": self.allow_partial,
            },
            "items": [item.to_dict() for item in self.items],
            "coverage": self.coverage.to_dict(),
            "dependency_graph": self.dependency_graph.to_dict(),
            "execution": "not_started;planning_and_verification_only",
            "authorization": "portfolio_selection_does_not_authorize_provider_tools_or_effects",
            "retention": _PORTFOLIO_RETENTION,
            "secret_material": _PORTFOLIO_SECRET_MATERIAL,
        }

    def to_dict(self) -> dict[str, Any]:
        return {**self._digest_payload(), "portfolio_digest": self.portfolio_digest}

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> "AutonomousWorkflowPortfolioPlan":
        if not isinstance(value, Mapping) or value.get("schema") != AUTONOMOUS_WORKFLOW_PORTFOLIO_SCHEMA:
            raise BrainRunError("workflow portfolio plan schema is invalid")
        allowed = {
            "schema", "status", "policy", "items", "coverage", "dependency_graph",
            "portfolio_digest", "execution", "authorization", "retention", "secret_material",
        }
        if set(value).difference(allowed):
            raise BrainRunError("workflow portfolio plan contains unsupported fields")
        policy = value.get("policy")
        if not isinstance(policy, Mapping):
            raise BrainRunError("workflow portfolio plan policy is invalid")
        items_value = value.get("items")
        if not isinstance(items_value, Sequence) or isinstance(items_value, (str, bytes)):
            raise BrainRunError("workflow portfolio plan items are invalid")
        items = tuple(_item_from_dict(item) for item in items_value)
        coverage = _coverage_from_dict(value.get("coverage"))
        graph = _graph_from_dict(value.get("dependency_graph"))
        plan = cls(
            status=value.get("status"),
            require_all_domains=policy.get("require_all_domains"),
            allow_partial=policy.get("allow_partial"),
            items=items,
            coverage=coverage,
            dependency_graph=graph,
            portfolio_digest=value.get("portfolio_digest"),
        )
        if value.get("execution") != "not_started;planning_and_verification_only" or value.get("authorization") != "portfolio_selection_does_not_authorize_provider_tools_or_effects" or value.get("retention") != _PORTFOLIO_RETENTION or value.get("secret_material") != _PORTFOLIO_SECRET_MATERIAL:
            raise BrainRunError("workflow portfolio plan authority markers are invalid")
        if content_digest(plan._digest_payload()) != plan.portfolio_digest:
            raise BrainRunError("workflow portfolio plan digest does not match its contents")
        expected_graph = _dependency_graph(items)
        if expected_graph.to_dict() != graph.to_dict():
            raise BrainRunError("workflow portfolio dependency graph is inconsistent")
        expected_coverage = _coverage(items, bool(policy.get("require_all_domains")))
        if expected_coverage.to_dict() != coverage.to_dict():
            raise BrainRunError("workflow portfolio coverage is inconsistent")
        return plan


@dataclass(frozen=True, slots=True)
class AutonomousWorkflowPortfolioVerification:
    status: str
    expected_portfolio_digest: str
    observed_portfolio_digest: str | None
    mismatches: tuple[Mapping[str, Any], ...]
    expected_item_count: int
    observed_item_count: int
    replayed_item_count: int
    verification_digest: str

    def to_dict(self) -> dict[str, Any]:
        payload = {
            "schema": AUTONOMOUS_WORKFLOW_PORTFOLIO_VERIFICATION_SCHEMA,
            "status": self.status,
            "expected_portfolio_digest": self.expected_portfolio_digest,
            "observed_portfolio_digest": self.observed_portfolio_digest,
            "mismatches": [dict(item) for item in self.mismatches],
            "expected_item_count": self.expected_item_count,
            "observed_item_count": self.observed_item_count,
            "replayed_item_count": self.replayed_item_count,
            "execution": "planning_only;no_provider_or_tool_calls",
            "retention": _PORTFOLIO_RETENTION,
            "secret_material": _PORTFOLIO_SECRET_MATERIAL,
        }
        return {**payload, "verification_digest": content_digest(payload)}


def _item_from_dict(value: Any) -> AutonomousWorkflowPortfolioItem:
    if not isinstance(value, Mapping):
        raise BrainRunError("workflow portfolio plan item must be an object")
    if value.get("schema") != AUTONOMOUS_WORKFLOW_PORTFOLIO_SCHEMA:
        raise BrainRunError("workflow portfolio plan item schema is invalid")
    if value.get("retention") != _PORTFOLIO_RETENTION or value.get("secret_material") != _PORTFOLIO_SECRET_MATERIAL:
        raise BrainRunError("workflow portfolio plan item retention markers are invalid")
    return AutonomousWorkflowPortfolioItem(
        item_id=value.get("item_id"),
        domain=value.get("domain"),
        capability=value.get("capability"),
        depends_on=_sequence_value("workflow portfolio plan item depends_on", value.get("depends_on", ())),
        task_digest=value.get("task_digest"),
        request_digest=value.get("request_digest"),
        route_digest=value.get("route_digest"),
        workflow_id=value.get("workflow_id"),
        workflow_digest=value.get("workflow_digest"),
        plan_digest=value.get("plan_digest"),
        evidence_plan_digest=value.get("evidence_plan_digest"),
        stage_ids=_sequence_value("workflow portfolio plan item stage_ids", value.get("stage_ids", ())),
        required_capabilities=_sequence_value(
            "workflow portfolio plan item required_capabilities",
            value.get("required_capabilities", ()),
        ),
        status=value.get("status"),
        error_class=value.get("error_class"),
    )


def _coverage_from_dict(value: Any) -> AutonomousWorkflowPortfolioCoverage:
    if not isinstance(value, Mapping):
        raise BrainRunError("workflow portfolio coverage must be an object")
    return AutonomousWorkflowPortfolioCoverage(
        requested_domains=_sequence_value("workflow portfolio coverage requested_domains", value.get("requested_domains", ())),
        ready_domains=_sequence_value("workflow portfolio coverage ready_domains", value.get("ready_domains", ())),
        missing_domains=_sequence_value("workflow portfolio coverage missing_domains", value.get("missing_domains", ())),
        duplicate_domain_items=_sequence_value(
            "workflow portfolio coverage duplicate_domain_items",
            value.get("duplicate_domain_items", ()),
        ),
        requested_item_count=value.get("requested_item_count"),
        ready_item_count=value.get("ready_item_count"),
        blocked_item_count=value.get("blocked_item_count"),
        failed_item_count=value.get("failed_item_count"),
        complete=value.get("complete"),
    )


def _graph_from_dict(value: Any) -> AutonomousWorkflowPortfolioDependencyGraph:
    if not isinstance(value, Mapping):
        raise BrainRunError("workflow portfolio dependency graph must be an object")
    waves = _sequence_value("workflow portfolio dependency graph waves", value.get("waves", ()))
    return AutonomousWorkflowPortfolioDependencyGraph(
        topological_order=_sequence_value(
            "workflow portfolio dependency graph topological_order",
            value.get("topological_order", ()),
        ),
        waves=tuple(
            _sequence_value("workflow portfolio dependency graph wave", wave)
            for wave in waves
        ),
        cycle_item_ids=_sequence_value(
            "workflow portfolio dependency graph cycle_item_ids",
            value.get("cycle_item_ids", ()),
        ),
        edge_count=value.get("edge_count"),
    )


def _dependency_graph(items: Sequence[AutonomousWorkflowPortfolioItem]) -> AutonomousWorkflowPortfolioDependencyGraph:
    ids = {item.item_id for item in items}
    indegree = {item.item_id: len(item.depends_on) for item in items}
    children = {item.item_id: [] for item in items}
    for item in items:
        for dependency in item.depends_on:
            if dependency not in ids:
                raise BrainRunError("workflow portfolio dependency references an unknown item")
            children[dependency].append(item.item_id)
    ready = sorted(item_id for item_id, count in indegree.items() if count == 0)
    topological: list[str] = []
    waves: list[tuple[str, ...]] = []
    while ready:
        wave = tuple(sorted(ready))
        ready = []
        waves.append(wave)
        for item_id in wave:
            topological.append(item_id)
            for child in sorted(children[item_id]):
                indegree[child] -= 1
                if indegree[child] == 0:
                    ready.append(child)
    cycle_ids = tuple(sorted(ids.difference(topological)))
    return AutonomousWorkflowPortfolioDependencyGraph(
        topological_order=tuple(topological),
        waves=tuple(waves),
        cycle_item_ids=cycle_ids,
        edge_count=sum(len(item.depends_on) for item in items),
    )


def _coverage(items: Sequence[AutonomousWorkflowPortfolioItem], require_all_domains: bool) -> AutonomousWorkflowPortfolioCoverage:
    requested = tuple(sorted({item.domain for item in items}))
    ready = tuple(sorted({item.domain for item in items if item.status == "ready"}))
    counts: dict[str, int] = {}
    for item in items:
        counts[item.domain] = counts.get(item.domain, 0) + 1
    expected = AUTONOMOUS_DOMAINS if require_all_domains else requested
    missing = tuple(sorted(domain for domain in expected if domain not in ready))
    duplicate = tuple(sorted(domain for domain, count in counts.items() if count > 1))
    ready_count = sum(item.status == "ready" for item in items)
    blocked_count = sum(item.status == "blocked" for item in items)
    failed_count = sum(item.status in {"failed", "route_review_required"} for item in items)
    return AutonomousWorkflowPortfolioCoverage(
        requested_domains=requested,
        ready_domains=ready,
        missing_domains=missing,
        duplicate_domain_items=duplicate,
        requested_item_count=len(items),
        ready_item_count=ready_count,
        blocked_item_count=blocked_count,
        failed_item_count=failed_count,
        complete=not missing and ready_count == len(items),
    )


def _normalize_requests(
    requests: Sequence[AutonomousWorkflowPortfolioItemRequest | Mapping[str, Any]],
) -> tuple[AutonomousWorkflowPortfolioItemRequest, ...]:
    if isinstance(requests, (str, bytes)) or not isinstance(requests, Sequence):
        raise BrainRunError("workflow portfolio requests must be a sequence")
    if not 1 <= len(requests) <= MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_ITEMS:
        raise BrainRunError("workflow portfolio requests must contain 1..64 items")
    normalized: list[AutonomousWorkflowPortfolioItemRequest] = []
    ids: set[str] = set()
    for index, raw in enumerate(requests):
        if isinstance(raw, AutonomousWorkflowPortfolioItemRequest):
            item = raw
        elif isinstance(raw, Mapping):
            dependencies = raw.get("depends_on", raw.get("dependsOn", ()))
            item = AutonomousWorkflowPortfolioItemRequest(
                task=raw.get("task"),
                domain=raw.get("domain"),
                id=raw.get("id"),
                capability=raw.get("capability"),
                depends_on=_sequence_value(f"workflow portfolio request {index} depends_on", dependencies),
                hints=_sequence_value(f"workflow portfolio request {index} hints", raw.get("hints", ())),
                context=raw.get("context", {}),
            )
        else:
            raise BrainRunError(f"workflow portfolio request {index} must be an object")
        item_id = item.normalized_id(index)
        _identifier(f"workflow portfolio item {index} id", item_id)
        if item_id in ids:
            raise BrainRunError(f"workflow portfolio item id is duplicated: {item_id}")
        ids.add(item_id)
        normalized.append(item)
    for item in normalized:
        unknown = sorted(set(item.depends_on).difference(ids))
        if unknown:
            raise BrainRunError(
                f"workflow portfolio item {item.normalized_id(normalized.index(item))} depends on unknown items: {', '.join(unknown)}"
            )
    return tuple(normalized)


def _error_class(error: BaseException) -> str:
    name = type(error).__name__
    if not name or len(name) > 128 or any(character not in _IDENTIFIER_CHARS for character in name):
        return "WorkflowPortfolioError"
    return name


def _compile_item(
    agent: "AutonomousAgent",
    request: AutonomousWorkflowPortfolioItemRequest,
    item_id: str,
    request_digest: str,
) -> AutonomousWorkflowPortfolioItem:
    route = agent.route(task=request.task, hints=request.hints, min_confidence=0.0, min_margin=0.0)
    blueprint = _prepare_blueprint(agent, request, item_id, request_digest)
    if not isinstance(blueprint, AutonomousTaskBlueprint):
        raise BrainRunError("workflow portfolio compiler received an invalid blueprint")
    evidence_plan = blueprint.evidence_plan()
    return AutonomousWorkflowPortfolioItem(
        item_id=item_id,
        domain=request.domain,
        capability=blueprint.spec.capability,
        depends_on=request.depends_on,
        task_digest=blueprint.spec.task_digest,
        request_digest=request_digest,
        route_digest=route.route_digest,
        workflow_id=blueprint.workflow.workflow_id,
        workflow_digest=blueprint.workflow.workflow_digest,
        # ``AutonomousPlanBuilder`` intentionally leaves the raw plan transient.  Bind its
        # canonical digest here instead of adding a persistence field to the execution blueprint.
        plan_digest=content_digest(blueprint.plan),
        evidence_plan_digest=evidence_plan.plan_digest,
        stage_ids=tuple(stage.id for stage in blueprint.workflow.stages),
        required_capabilities=tuple(sorted(set(blueprint.required_capabilities))),
        status="ready",
    )


def _prepare_blueprint(
    agent: "AutonomousAgent",
    request: AutonomousWorkflowPortfolioItemRequest,
    item_id: str,
    request_digest: str,
) -> AutonomousTaskBlueprint:
    """Recreate one transient blueprint with the same portfolio binding used at plan time."""

    blueprint = agent.prepare(
        task=request.task,
        domain=request.domain,
        capability=request.capability,
        context={
            **dict(request.context),
            "_aurora_workflow_portfolio": {
                "item_id": item_id,
                "request_digest": request_digest,
                "hints": list(request.hints),
                "does_not_authorize": ["provider_calls", "tools", "external_effects"],
            },
        },
    )
    if not isinstance(blueprint, AutonomousTaskBlueprint):
        raise BrainRunError("workflow portfolio compiler received an invalid blueprint")
    return blueprint


def plan_autonomous_workflow_portfolio(
    agent: "AutonomousAgent",
    requests: Sequence[AutonomousWorkflowPortfolioItemRequest | Mapping[str, Any]],
    *,
    require_all_domains: bool = False,
    allow_partial: bool = True,
) -> AutonomousWorkflowPortfolioPlan:
    """Compile a dependency-aware, metadata-only portfolio without provider dispatch."""

    if not hasattr(agent, "prepare") or not hasattr(agent, "route"):
        raise BrainRunError("workflow portfolio compiler requires an AutonomousAgent")
    if not isinstance(require_all_domains, bool) or not isinstance(allow_partial, bool):
        raise BrainRunError("workflow portfolio policy flags must be booleans")
    normalized = _normalize_requests(requests)
    by_id = {item.normalized_id(index): item for index, item in enumerate(normalized)}
    graph_ready = {
        item_id: len(request.depends_on)
        for item_id, request in by_id.items()
    }
    children: dict[str, list[str]] = {item_id: [] for item_id in by_id}
    for item_id, request in by_id.items():
        for dependency in request.depends_on:
            children[dependency].append(item_id)
    queue = sorted(item_id for item_id, count in graph_ready.items() if count == 0)
    processed: set[str] = set()
    compiled_by_id: dict[str, AutonomousWorkflowPortfolioItem] = {}
    while queue:
        item_id = queue.pop(0)
        request = by_id[item_id]
        request_digest = content_digest(request.request_payload(item_id))
        blocked_dependency = next(
            (
                dependency
                for dependency in request.depends_on
                if dependency in compiled_by_id and compiled_by_id[dependency].status != "ready"
            ),
            None,
        )
        if blocked_dependency is not None:
            item = AutonomousWorkflowPortfolioItem(
                item_id=item_id,
                domain=request.domain,
                capability=request.capability,
                depends_on=request.depends_on,
                task_digest=content_digest({"task": request.task}),
                request_digest=request_digest,
                route_digest=None,
                workflow_id=None,
                workflow_digest=None,
                plan_digest=None,
                evidence_plan_digest=None,
                stage_ids=(),
                required_capabilities=(),
                status="blocked",
                error_class="dependency_not_ready",
            )
        else:
            try:
                item = _compile_item(agent, request, item_id, request_digest)
            except Exception as error:
                item = AutonomousWorkflowPortfolioItem(
                    item_id=item_id,
                    domain=request.domain,
                    capability=request.capability,
                    depends_on=request.depends_on,
                    task_digest=content_digest({"task": request.task}),
                    request_digest=request_digest,
                    route_digest=None,
                    workflow_id=None,
                    workflow_digest=None,
                    plan_digest=None,
                    evidence_plan_digest=None,
                    stage_ids=(),
                    required_capabilities=(),
                    status="failed",
                    error_class=_error_class(error),
                )
        compiled_by_id[item_id] = item
        processed.add(item_id)
        for child in sorted(children[item_id]):
            graph_ready[child] -= 1
            if graph_ready[child] == 0:
                queue.append(child)
                queue.sort()
    for item_id in sorted(set(by_id).difference(processed)):
        request = by_id[item_id]
        compiled_by_id[item_id] = AutonomousWorkflowPortfolioItem(
            item_id=item_id,
            domain=request.domain,
            capability=request.capability,
            depends_on=request.depends_on,
            task_digest=content_digest({"task": request.task}),
            request_digest=content_digest(request.request_payload(item_id)),
            route_digest=None,
            workflow_id=None,
            workflow_digest=None,
            plan_digest=None,
            evidence_plan_digest=None,
            stage_ids=(),
            required_capabilities=(),
            status="blocked",
            error_class="dependency_cycle",
        )
    items = tuple(compiled_by_id[item_id] for item_id in by_id)
    coverage = _coverage(items, require_all_domains)
    graph = _dependency_graph(items)
    status = (
        "ready"
        if coverage.complete
        else "partial"
        if allow_partial and coverage.ready_item_count > 0
        else "blocked"
    )
    payload = {
        "schema": AUTONOMOUS_WORKFLOW_PORTFOLIO_SCHEMA,
        "status": status,
        "policy": {"require_all_domains": require_all_domains, "allow_partial": allow_partial},
        "items": [item.to_dict() for item in items],
        "coverage": coverage.to_dict(),
        "dependency_graph": graph.to_dict(),
        "execution": "not_started;planning_and_verification_only",
        "authorization": "portfolio_selection_does_not_authorize_provider_tools_or_effects",
        "retention": _PORTFOLIO_RETENTION,
        "secret_material": _PORTFOLIO_SECRET_MATERIAL,
    }
    return AutonomousWorkflowPortfolioPlan(
        status=status,
        require_all_domains=require_all_domains,
        allow_partial=allow_partial,
        items=items,
        coverage=coverage,
        dependency_graph=graph,
        portfolio_digest=content_digest(payload),
    )


def verify_autonomous_workflow_portfolio(
    agent: "AutonomousAgent",
    plan: AutonomousWorkflowPortfolioPlan | Mapping[str, Any],
    requests: Sequence[AutonomousWorkflowPortfolioItemRequest | Mapping[str, Any]],
    *,
    require_all_domains: bool | None = None,
    allow_partial: bool | None = None,
) -> AutonomousWorkflowPortfolioVerification:
    """Replay planning and compare stable per-item identities without provider calls."""

    expected = plan if isinstance(plan, AutonomousWorkflowPortfolioPlan) else AutonomousWorkflowPortfolioPlan.from_dict(plan)
    replayed = plan_autonomous_workflow_portfolio(
        agent,
        requests,
        require_all_domains=expected.require_all_domains if require_all_domains is None else require_all_domains,
        allow_partial=expected.allow_partial if allow_partial is None else allow_partial,
    )
    mismatches: list[dict[str, Any]] = []
    expected_by_id = {item.item_id: item for item in expected.items}
    observed_by_id = {item.item_id: item for item in replayed.items}
    for item_id in sorted(set(expected_by_id) | set(observed_by_id)):
        expected_item = expected_by_id.get(item_id)
        observed_item = observed_by_id.get(item_id)
        codes: list[str] = []
        if expected_item is None or observed_item is None:
            codes.append("item_missing")
        else:
            for field_name in (
                "domain", "capability", "depends_on", "task_digest", "request_digest", "route_digest",
                "workflow_id", "workflow_digest", "plan_digest", "evidence_plan_digest", "stage_ids",
                "required_capabilities", "status", "error_class",
            ):
                if getattr(expected_item, field_name) != getattr(observed_item, field_name):
                    codes.append(field_name)
        if codes:
            mismatches.append({"item_id": item_id, "codes": codes})
    if expected.portfolio_digest != replayed.portfolio_digest:
        mismatches.append({"item_id": "portfolio", "codes": ["portfolio_digest"]})
    status = "verified" if not mismatches else "mismatch"
    payload = {
        "schema": AUTONOMOUS_WORKFLOW_PORTFOLIO_VERIFICATION_SCHEMA,
        "status": status,
        "expected_portfolio_digest": expected.portfolio_digest,
        "observed_portfolio_digest": replayed.portfolio_digest,
        "mismatches": mismatches,
        "expected_item_count": len(expected.items),
        "observed_item_count": len(replayed.items),
        "replayed_item_count": len(replayed.items),
        "execution": "planning_only;no_provider_or_tool_calls",
        "retention": _PORTFOLIO_RETENTION,
        "secret_material": _PORTFOLIO_SECRET_MATERIAL,
    }
    return AutonomousWorkflowPortfolioVerification(
        status=status,
        expected_portfolio_digest=expected.portfolio_digest,
        observed_portfolio_digest=replayed.portfolio_digest,
        mismatches=tuple(mismatches),
        expected_item_count=len(expected.items),
        observed_item_count=len(replayed.items),
        replayed_item_count=len(replayed.items),
        verification_digest=content_digest(payload),
    )


@dataclass(frozen=True, slots=True)
class AutonomousWorkflowPortfolioRehydrationContext:
    """Caller-owned context for restoring one previously successful portfolio item."""

    job_id: str
    item_id: str
    plan_digest: str
    request_digest: str
    task_digest: str
    expected_result_digest: str

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": "bioprism-python-autonomous-workflow-portfolio-rehydration/0.1",
            "job_id": self.job_id,
            "item_id": self.item_id,
            "plan_digest": self.plan_digest,
            "request_digest": self.request_digest,
            "task_digest": self.task_digest,
            "expected_result_digest": self.expected_result_digest,
            "retention": "identities_only;caller_rehydrates_transient_result",
            "secret_material": _PORTFOLIO_SECRET_MATERIAL,
        }


@dataclass(frozen=True, slots=True)
class AutonomousWorkflowPortfolioExecutionCheckpoint:
    """Restart-safe metadata for portfolio progress; successful raw runs stay caller-owned."""

    job_id: str
    plan_digest: str
    portfolio_input_digest: str
    item_ids: tuple[str, ...]
    request_digests: tuple[str, ...]
    task_digests: tuple[str, ...]
    settled_item_ids: tuple[str, ...]
    settled_item_statuses: tuple[str, ...]
    settled_result_digests: tuple[str, ...]
    max_parallelism: int
    stop_on_error: bool
    status: str
    checkpoint_digest: str

    def __post_init__(self) -> None:
        _identifier("workflow portfolio checkpoint job_id", self.job_id)
        for label, value in (
            ("plan_digest", self.plan_digest),
            ("portfolio_input_digest", self.portfolio_input_digest),
        ):
            if not isinstance(value, str) or len(value) != 64 or any(character not in "0123456789abcdef" for character in value):
                raise BrainRunError(f"workflow portfolio checkpoint {label} is invalid")
        item_ids = _string_list("workflow portfolio checkpoint item_ids", self.item_ids, maximum=MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_ITEMS)
        request_digests = _digest_list("workflow portfolio checkpoint request_digests", self.request_digests, len(item_ids))
        task_digests = _digest_list("workflow portfolio checkpoint task_digests", self.task_digests, len(item_ids))
        settled_ids = _string_list("workflow portfolio checkpoint settled_item_ids", self.settled_item_ids, maximum=len(item_ids))
        if len(set(item_ids)) != len(item_ids) or len(set(settled_ids)) != len(settled_ids):
            raise BrainRunError("workflow portfolio checkpoint item ids must be unique")
        if any(item_id not in item_ids for item_id in settled_ids):
            raise BrainRunError("workflow portfolio checkpoint settles an unknown item")
        if tuple(sorted(settled_ids)) != settled_ids:
            raise BrainRunError("workflow portfolio checkpoint settled ids must be sorted")
        if (
            isinstance(self.settled_item_statuses, (str, bytes))
            or not isinstance(self.settled_item_statuses, Sequence)
            or len(self.settled_item_statuses) > len(item_ids)
        ):
            raise BrainRunError("workflow portfolio checkpoint settled statuses are invalid")
        settled_statuses = tuple(
            _text("workflow portfolio checkpoint settled_item_statuses", status, maximum=64)
            for status in self.settled_item_statuses
        )
        if len(settled_statuses) != len(settled_ids) or any(status != "succeeded" for status in settled_statuses):
            raise BrainRunError("workflow portfolio checkpoint settled statuses must be succeeded")
        settled_result_digests = _digest_list(
            "workflow portfolio checkpoint settled_result_digests",
            self.settled_result_digests,
            len(settled_ids),
        )
        if isinstance(self.max_parallelism, bool) or not isinstance(self.max_parallelism, int) or not 1 <= self.max_parallelism <= 16:
            raise BrainRunError("workflow portfolio checkpoint max_parallelism is outside its bound")
        if not isinstance(self.stop_on_error, bool):
            raise BrainRunError("workflow portfolio checkpoint stop_on_error must be a boolean")
        if self.status not in {"running", "partial", "completed", "blocked", "approval_required"}:
            raise BrainRunError("workflow portfolio checkpoint status is invalid")
        if not isinstance(self.checkpoint_digest, str) or len(self.checkpoint_digest) != 64 or any(character not in "0123456789abcdef" for character in self.checkpoint_digest):
            raise BrainRunError("workflow portfolio checkpoint digest is invalid")
        object.__setattr__(self, "item_ids", item_ids)
        object.__setattr__(self, "request_digests", request_digests)
        object.__setattr__(self, "task_digests", task_digests)
        object.__setattr__(self, "settled_item_ids", settled_ids)
        object.__setattr__(self, "settled_item_statuses", settled_statuses)
        object.__setattr__(self, "settled_result_digests", settled_result_digests)

    def _digest_payload(self) -> dict[str, Any]:
        return {
            "schema": "bioprism-python-autonomous-workflow-portfolio-execution-checkpoint/0.1",
            "job_id": self.job_id,
            "plan_digest": self.plan_digest,
            "portfolio_input_digest": self.portfolio_input_digest,
            "item_ids": list(self.item_ids),
            "request_digests": list(self.request_digests),
            "task_digests": list(self.task_digests),
            "settled_item_ids": list(self.settled_item_ids),
            "settled_item_statuses": list(self.settled_item_statuses),
            "settled_result_digests": list(self.settled_result_digests),
            "max_parallelism": self.max_parallelism,
            "stop_on_error": self.stop_on_error,
            "status": self.status,
            "retention": "successful_item_metadata_only;raw_runs_caller_owned",
            "secret_material": _PORTFOLIO_SECRET_MATERIAL,
        }

    def to_dict(self) -> dict[str, Any]:
        return {**self._digest_payload(), "checkpoint_digest": self.checkpoint_digest}

    @classmethod
    def create(
        cls,
        *,
        job_id: str,
        plan_digest: str,
        portfolio_input_digest: str,
        item_ids: Sequence[str],
        request_digests: Sequence[str],
        task_digests: Sequence[str],
        settled_item_ids: Sequence[str],
        settled_result_digests: Sequence[str],
        max_parallelism: int,
        stop_on_error: bool,
        status: str,
    ) -> "AutonomousWorkflowPortfolioExecutionCheckpoint":
        payload = {
            "schema": "bioprism-python-autonomous-workflow-portfolio-execution-checkpoint/0.1",
            "job_id": job_id,
            "plan_digest": plan_digest,
            "portfolio_input_digest": portfolio_input_digest,
            "item_ids": list(item_ids),
            "request_digests": list(request_digests),
            "task_digests": list(task_digests),
            "settled_item_ids": list(settled_item_ids),
            "settled_item_statuses": ["succeeded"] * len(settled_item_ids),
            "settled_result_digests": list(settled_result_digests),
            "max_parallelism": max_parallelism,
            "stop_on_error": stop_on_error,
            "status": status,
            "retention": "successful_item_metadata_only;raw_runs_caller_owned",
            "secret_material": _PORTFOLIO_SECRET_MATERIAL,
        }
        return cls(
            job_id=job_id,
            plan_digest=plan_digest,
            portfolio_input_digest=portfolio_input_digest,
            item_ids=tuple(item_ids),
            request_digests=tuple(request_digests),
            task_digests=tuple(task_digests),
            settled_item_ids=tuple(settled_item_ids),
            settled_item_statuses=tuple("succeeded" for _ in settled_item_ids),
            settled_result_digests=tuple(settled_result_digests),
            max_parallelism=max_parallelism,
            stop_on_error=stop_on_error,
            status=status,
            checkpoint_digest=content_digest(payload),
        )

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> "AutonomousWorkflowPortfolioExecutionCheckpoint":
        if not isinstance(value, Mapping) or value.get("schema") != "bioprism-python-autonomous-workflow-portfolio-execution-checkpoint/0.1":
            raise BrainRunError("workflow portfolio checkpoint schema is invalid")
        if value.get("retention") != "successful_item_metadata_only;raw_runs_caller_owned" or value.get("secret_material") != _PORTFOLIO_SECRET_MATERIAL:
            raise BrainRunError("workflow portfolio checkpoint retention markers are invalid")
        checkpoint = cls(
            job_id=value.get("job_id"),
            plan_digest=value.get("plan_digest"),
            portfolio_input_digest=value.get("portfolio_input_digest"),
            item_ids=tuple(value.get("item_ids", ())),
            request_digests=tuple(value.get("request_digests", ())),
            task_digests=tuple(value.get("task_digests", ())),
            settled_item_ids=tuple(value.get("settled_item_ids", ())),
            settled_item_statuses=tuple(value.get("settled_item_statuses", ())),
            settled_result_digests=tuple(value.get("settled_result_digests", ())),
            max_parallelism=value.get("max_parallelism"),
            stop_on_error=value.get("stop_on_error"),
            status=value.get("status"),
            checkpoint_digest=value.get("checkpoint_digest"),
        )
        if content_digest(checkpoint._digest_payload()) != checkpoint.checkpoint_digest:
            raise BrainRunError("workflow portfolio checkpoint digest does not match its contents")
        return checkpoint


@dataclass(frozen=True, slots=True)
class AutonomousWorkflowPortfolioExecutionItem:
    """One transient run plus its safe metadata projection."""

    item_id: str
    domain: str
    depends_on: tuple[str, ...]
    status: str
    result_digest: str | None = None
    result_bytes: int = 0
    error_class: str | None = None
    run: Any | None = field(default=None, repr=False, compare=False)

    def __post_init__(self) -> None:
        _identifier("workflow portfolio execution item id", self.item_id)
        _identifier("workflow portfolio execution item domain", self.domain)
        if self.domain not in AUTONOMOUS_DOMAINS:
            raise BrainRunError("workflow portfolio execution item domain is unsupported")
        object.__setattr__(self, "depends_on", _string_list("workflow portfolio execution dependencies", self.depends_on, maximum=MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_DEPENDENCIES))
        if self.status not in {"succeeded", "failed", "blocked", "approval_required", "reconciliation_required", "not_started"}:
            raise BrainRunError("workflow portfolio execution item status is invalid")
        if self.result_digest is not None:
            _assert_digest(self.result_digest, "workflow portfolio execution result_digest")
        if isinstance(self.result_bytes, bool) or not isinstance(self.result_bytes, int) or not 0 <= self.result_bytes <= 16_000_000:
            raise BrainRunError("workflow portfolio execution result_bytes is outside its bound")
        if self.status == "succeeded" and self.result_digest is None:
            raise BrainRunError("successful portfolio execution item requires a result digest")
        if self.status != "succeeded" and self.error_class is None and self.status != "not_started":
            raise BrainRunError("non-successful portfolio execution item requires an error class")
        if self.error_class is not None:
            _identifier("workflow portfolio execution error_class", self.error_class)

    def to_dict(self) -> dict[str, Any]:
        return {
            "item_id": self.item_id,
            "domain": self.domain,
            "depends_on": list(self.depends_on),
            "status": self.status,
            "result_digest": self.result_digest,
            "result_bytes": self.result_bytes,
            "error_class": self.error_class,
            "result_retention": "raw_run_caller_owned;not_serialized",
            "secret_material": _PORTFOLIO_SECRET_MATERIAL,
        }


@dataclass(frozen=True, slots=True)
class AutonomousWorkflowPortfolioExecutionResult:
    """Portfolio execution with transient runs and a safe restart projection."""

    status: str
    plan: AutonomousWorkflowPortfolioPlan
    items: tuple[AutonomousWorkflowPortfolioExecutionItem, ...]
    executed_waves: tuple[tuple[str, ...], ...]
    completed_count: int
    failed_count: int
    blocked_count: int
    approval_required_count: int
    next_action: str
    checkpoint: AutonomousWorkflowPortfolioExecutionCheckpoint

    def __post_init__(self) -> None:
        if self.status not in {"completed", "partial", "blocked", "approval_required", "reconciliation_required"}:
            raise BrainRunError("workflow portfolio execution status is invalid")
        if not isinstance(self.plan, AutonomousWorkflowPortfolioPlan):
            raise BrainRunError("workflow portfolio execution plan is invalid")
        ids = {item.item_id for item in self.plan.items}
        if {item.item_id for item in self.items} != ids:
            raise BrainRunError("workflow portfolio execution items do not match the plan")
        if self.checkpoint.plan_digest != self.plan.portfolio_digest:
            raise BrainRunError("workflow portfolio execution checkpoint does not match the plan")
        _identifier("workflow portfolio execution next_action", self.next_action)

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": "bioprism-python-autonomous-workflow-portfolio-execution/0.1",
            "status": self.status,
            "plan_digest": self.plan.portfolio_digest,
            "items": [item.to_dict() for item in self.items],
            "executed_waves": [list(wave) for wave in self.executed_waves],
            "completed_count": self.completed_count,
            "failed_count": self.failed_count,
            "blocked_count": self.blocked_count,
            "approval_required_count": self.approval_required_count,
            "next_action": self.next_action,
            "checkpoint": self.checkpoint.to_dict(),
            "execution": "provider_calls_are_caller_approved_per_item;raw_runs_not_serialized",
            "retention": "portfolio_metadata_and_result_digests_only",
            "secret_material": _PORTFOLIO_SECRET_MATERIAL,
        }


def _assert_digest(value: Any, label: str) -> str:
    if not isinstance(value, str) or len(value) != 64 or any(character not in "0123456789abcdef" for character in value):
        raise BrainRunError(f"{label} is not a SHA-256 digest")
    return value


def _digest_list(label: str, values: Any, expected_length: int) -> tuple[str, ...]:
    if isinstance(values, (str, bytes)) or not isinstance(values, Sequence) or len(values) != expected_length:
        raise BrainRunError(f"{label} does not align with the portfolio items")
    return tuple(_assert_digest(value, label) for value in values)


def _result_projection(item_id: str, run: Any) -> tuple[str, int]:
    serializer = getattr(run, "to_dict", None)
    if not callable(serializer):
        raise BrainRunError("portfolio workflow run does not expose a serializable result projection")
    try:
        payload = serializer()
        encoded = json.dumps(payload, ensure_ascii=False, allow_nan=False, sort_keys=True, separators=(",", ":"))
    except (TypeError, ValueError) as error:
        raise BrainRunError("portfolio workflow result is not JSON-safe") from error
    return content_digest({"item_id": item_id, "result": payload}), len(encoded.encode("utf-8"))


def _execution_status(run: Any) -> str:
    status = getattr(run, "status", None)
    if status == "completed":
        return "succeeded"
    if status == "approval_required":
        return "approval_required"
    if status in {"stage_blocked", "stage_proposed", "stage_not_attempted", "paused"}:
        return "blocked"
    return "failed"


def _failure_class(error: BaseException) -> str:
    name = type(error).__name__
    return name if name and len(name) <= 128 and all(character in _IDENTIFIER_CHARS for character in name) else "WorkflowPortfolioExecutionError"


def _portfolio_input_digest(requests: Sequence[AutonomousWorkflowPortfolioItemRequest]) -> str:
    return content_digest(
        {
            "schema": "bioprism-python-autonomous-workflow-portfolio-input/0.1",
            "requests": [request.request_payload(request.normalized_id(index)) for index, request in enumerate(requests)],
        }
    )


def execute_autonomous_workflow_portfolio(
    agent: "AutonomousAgent",
    plan: AutonomousWorkflowPortfolioPlan | Mapping[str, Any],
    requests: Sequence[AutonomousWorkflowPortfolioItemRequest | Mapping[str, Any]],
    *,
    credentials: Mapping[str, Any] | Any,
    model_candidates: Sequence[Mapping[str, Any]] | None = None,
    job_id: str,
    max_parallelism: int = 4,
    stop_on_error: bool = False,
    checkpoint: AutonomousWorkflowPortfolioExecutionCheckpoint | Mapping[str, Any] | None = None,
    checkpoint_sink: Any | None = None,
    rehydrate_result: Any | None = None,
    workflow_options_factory: Any | None = None,
) -> AutonomousWorkflowPortfolioExecutionResult:
    """Execute ready portfolio items in dependency waves through ``agent.run_workflow``.

    The plan is replay-verified before the first provider call.  Independent items in one wave
    may run concurrently, while dependency waves remain ordered.  Only successful item digests
    cross the checkpoint boundary; a restart must rehydrate the corresponding transient run and
    prove its digest before any dependent item can dispatch.
    """

    _identifier("workflow portfolio execution job_id", job_id)
    if isinstance(max_parallelism, bool) or not isinstance(max_parallelism, int) or not 1 <= max_parallelism <= 16:
        raise BrainRunError("workflow portfolio execution max_parallelism must be between 1 and 16")
    if not isinstance(stop_on_error, bool):
        raise BrainRunError("workflow portfolio execution stop_on_error must be a boolean")
    if checkpoint_sink is not None and not callable(checkpoint_sink):
        raise BrainRunError("workflow portfolio checkpoint_sink must be callable or None")
    if rehydrate_result is not None and not callable(rehydrate_result):
        raise BrainRunError("workflow portfolio rehydrate_result must be callable or None")
    if workflow_options_factory is not None and not callable(workflow_options_factory):
        raise BrainRunError("workflow portfolio workflow_options_factory must be callable or None")

    expected_plan = plan if isinstance(plan, AutonomousWorkflowPortfolioPlan) else AutonomousWorkflowPortfolioPlan.from_dict(plan)
    normalized = _normalize_requests(requests)
    replayed = plan_autonomous_workflow_portfolio(
        agent,
        normalized,
        require_all_domains=expected_plan.require_all_domains,
        allow_partial=expected_plan.allow_partial,
    )
    if replayed.portfolio_digest != expected_plan.portfolio_digest:
        raise BrainRunError("workflow portfolio execution refuses a plan that drifted during replay")
    by_id = {request.normalized_id(index): request for index, request in enumerate(normalized)}
    plan_by_id = {item.item_id: item for item in expected_plan.items}
    item_ids = tuple(item.item_id for item in expected_plan.items)
    request_digests = tuple(plan_by_id[item_id].request_digest for item_id in item_ids)
    task_digests = tuple(plan_by_id[item_id].task_digest for item_id in item_ids)
    input_digest = _portfolio_input_digest(normalized)

    current_checkpoint: AutonomousWorkflowPortfolioExecutionCheckpoint | None
    if checkpoint is None:
        current_checkpoint = None
    elif isinstance(checkpoint, AutonomousWorkflowPortfolioExecutionCheckpoint):
        current_checkpoint = checkpoint
    elif isinstance(checkpoint, Mapping):
        current_checkpoint = AutonomousWorkflowPortfolioExecutionCheckpoint.from_dict(checkpoint)
    else:
        raise BrainRunError("workflow portfolio checkpoint must be a checkpoint or mapping")
    if current_checkpoint is not None:
        if current_checkpoint.job_id != job_id or current_checkpoint.plan_digest != expected_plan.portfolio_digest:
            raise BrainRunError("workflow portfolio checkpoint job or plan does not match")
        if current_checkpoint.portfolio_input_digest != input_digest or current_checkpoint.item_ids != item_ids or current_checkpoint.request_digests != request_digests or current_checkpoint.task_digests != task_digests:
            raise BrainRunError("workflow portfolio checkpoint requests do not match the current portfolio")
        if current_checkpoint.max_parallelism != max_parallelism or current_checkpoint.stop_on_error != stop_on_error:
            raise BrainRunError("workflow portfolio checkpoint controls do not match")
        if current_checkpoint.settled_item_ids and rehydrate_result is None:
            raise BrainRunError("resuming a workflow portfolio requires rehydrate_result")

    execution_by_id: dict[str, AutonomousWorkflowPortfolioExecutionItem] = {}
    if current_checkpoint is not None:
        for item_id, expected_result_digest in zip(current_checkpoint.settled_item_ids, current_checkpoint.settled_result_digests):
            plan_item = plan_by_id[item_id]
            context = AutonomousWorkflowPortfolioRehydrationContext(
                job_id=job_id,
                item_id=item_id,
                plan_digest=expected_plan.portfolio_digest,
                request_digest=plan_item.request_digest,
                task_digest=plan_item.task_digest,
                expected_result_digest=expected_result_digest,
            )
            try:
                run = rehydrate_result(context)
                result_digest, result_bytes = _result_projection(item_id, run)
            except Exception as error:
                raise BrainRunError("workflow portfolio result rehydration failed") from error
            if result_digest != expected_result_digest or _execution_status(run) != "succeeded":
                raise BrainRunError("workflow portfolio rehydrated result does not match its checkpoint")
            execution_by_id[item_id] = AutonomousWorkflowPortfolioExecutionItem(
                item_id=item_id,
                domain=plan_item.domain,
                depends_on=plan_item.depends_on,
                status="succeeded",
                result_digest=result_digest,
                result_bytes=result_bytes,
                run=run,
            )

    if hasattr(agent, "_credential_mapping"):
        resolved_credentials = agent._credential_mapping(credentials)
    elif isinstance(credentials, Mapping):
        resolved_credentials = dict(credentials)
    else:
        raise BrainRunError("workflow portfolio credentials must be a mapping or session")
    if model_candidates is None:
        model_candidates = agent._resolve_candidates(None) if hasattr(agent, "_resolve_candidates") else ()
    if isinstance(model_candidates, (str, bytes)) or not isinstance(model_candidates, Sequence):
        raise BrainRunError("workflow portfolio model_candidates must be a sequence")
    if workflow_options_factory is not None:
        def options_for(item_id: str) -> dict[str, Any]:
            index = item_ids.index(item_id)
            generated = workflow_options_factory(dict(by_id[item_id].request_payload(item_id)), index)
            if not isinstance(generated, Mapping):
                raise BrainRunError("workflow portfolio workflow_options_factory must return a mapping")
            options = dict(generated)
            reserved = {"blueprint", "credentials", "model_candidates", "checkpoint"}
            if reserved.intersection(options):
                raise BrainRunError("workflow portfolio item options cannot override execution bindings")
            return options
    else:
        options_for = lambda _item_id: {}

    def persist(status: str) -> AutonomousWorkflowPortfolioExecutionCheckpoint:
        settled = sorted(
            item_id
            for item_id, item in execution_by_id.items()
            if item.status == "succeeded" and item.result_digest is not None
        )
        value = AutonomousWorkflowPortfolioExecutionCheckpoint.create(
            job_id=job_id,
            plan_digest=expected_plan.portfolio_digest,
            portfolio_input_digest=input_digest,
            item_ids=item_ids,
            request_digests=request_digests,
            task_digests=task_digests,
            settled_item_ids=settled,
            settled_result_digests=[execution_by_id[item_id].result_digest for item_id in settled],
            max_parallelism=max_parallelism,
            stop_on_error=stop_on_error,
            status=status,
        )
        if checkpoint_sink is not None:
            checkpoint_sink(value)
        return value

    persist("running")
    executed_waves: list[tuple[str, ...]] = []
    halted = False
    for wave in expected_plan.dependency_graph.waves:
        candidates = tuple(
            item_id
            for item_id in wave
            if plan_by_id[item_id].status == "ready" and item_id not in execution_by_id
        )
        if not candidates:
            continue
        if halted:
            for item_id in candidates:
                plan_item = plan_by_id[item_id]
                execution_by_id[item_id] = AutonomousWorkflowPortfolioExecutionItem(
                    item_id=item_id,
                    domain=plan_item.domain,
                    depends_on=plan_item.depends_on,
                    status="blocked",
                    error_class="stop_on_error",
                )
            continue
        runnable: list[str] = []
        for item_id in candidates:
            plan_item = plan_by_id[item_id]
            if any(
                dependency not in execution_by_id or execution_by_id[dependency].status != "succeeded"
                for dependency in plan_item.depends_on
            ):
                execution_by_id[item_id] = AutonomousWorkflowPortfolioExecutionItem(
                    item_id=item_id,
                    domain=plan_item.domain,
                    depends_on=plan_item.depends_on,
                    status="blocked",
                    error_class="dependency_not_settled",
                )
            else:
                runnable.append(item_id)
        if not runnable:
            persist("blocked")
            continue

        def execute_one(item_id: str) -> AutonomousWorkflowPortfolioExecutionItem:
            request = by_id[item_id]
            plan_item = plan_by_id[item_id]
            try:
                blueprint = _prepare_blueprint(agent, request, item_id, plan_item.request_digest)
                observed_item = _compile_item(agent, request, item_id, plan_item.request_digest)
                if observed_item.to_dict() != plan_item.to_dict():
                    raise BrainRunError("workflow portfolio item changed between verification and dispatch")
                options = options_for(item_id)
                options.setdefault("execution_id", f"{job_id}:{item_id}")
                run = agent.run_workflow(
                    blueprint=blueprint,
                    model_candidates=model_candidates,
                    credentials=resolved_credentials,
                    **options,
                )
                status = _execution_status(run)
                result_digest, result_bytes = _result_projection(item_id, run)
                return AutonomousWorkflowPortfolioExecutionItem(
                    item_id=item_id,
                    domain=plan_item.domain,
                    depends_on=plan_item.depends_on,
                    status=status,
                    result_digest=result_digest,
                    result_bytes=result_bytes,
                    error_class=None if status == "succeeded" else status,
                    run=run,
                )
            except Exception as error:
                return AutonomousWorkflowPortfolioExecutionItem(
                    item_id=item_id,
                    domain=plan_item.domain,
                    depends_on=plan_item.depends_on,
                    status="failed",
                    error_class=_failure_class(error),
                )

        with ThreadPoolExecutor(max_workers=min(max_parallelism, len(runnable)), thread_name_prefix="aurora-workflow-portfolio") as pool:
            future_rows = {item_id: pool.submit(execute_one, item_id) for item_id in runnable}
            for item_id in sorted(future_rows):
                execution_by_id[item_id] = future_rows[item_id].result()
        executed_waves.append(tuple(sorted(runnable)))
        if stop_on_error and any(execution_by_id[item_id].status != "succeeded" for item_id in runnable):
            halted = True
        persist("running")

    for item_id, plan_item in plan_by_id.items():
        if item_id in execution_by_id:
            continue
        status = "blocked" if plan_item.status == "blocked" else "failed" if plan_item.status in {"failed", "route_review_required"} else "not_started"
        execution_by_id[item_id] = AutonomousWorkflowPortfolioExecutionItem(
            item_id=item_id,
            domain=plan_item.domain,
            depends_on=plan_item.depends_on,
            status=status,
            error_class=None if status == "not_started" else plan_item.error_class or "portfolio_not_ready",
        )
    ordered_items = tuple(execution_by_id[item_id] for item_id in item_ids)
    completed = sum(item.status == "succeeded" for item in ordered_items)
    failed = sum(item.status == "failed" for item in ordered_items)
    blocked = sum(item.status in {"blocked", "not_started"} for item in ordered_items)
    approvals = sum(item.status == "approval_required" for item in ordered_items)
    if approvals:
        status = "approval_required"
        next_action = "approve_item"
    elif failed and completed:
        status = "partial"
        next_action = "inspect_failed_item"
    elif failed:
        status = "blocked"
        next_action = "resolve_failed_item"
    elif blocked:
        status = "blocked"
        next_action = "resolve_dependency_or_plan_gap"
    elif completed == len(ordered_items):
        status = "completed"
        next_action = "complete"
    else:
        status = "reconciliation_required"
        next_action = "reconcile_portfolio_state"
    final_checkpoint = persist(status)
    return AutonomousWorkflowPortfolioExecutionResult(
        status=status,
        plan=expected_plan,
        items=ordered_items,
        executed_waves=tuple(executed_waves),
        completed_count=completed,
        failed_count=failed,
        blocked_count=blocked,
        approval_required_count=approvals,
        next_action=next_action,
        checkpoint=final_checkpoint,
    )


__all__ = [
    "AUTONOMOUS_WORKFLOW_PORTFOLIO_SCHEMA",
    "AUTONOMOUS_WORKFLOW_PORTFOLIO_VERIFICATION_SCHEMA",
    "MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_CONTEXT_BYTES",
    "MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_DEPENDENCIES",
    "MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_HINTS",
    "MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_ITEMS",
    "MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_STAGE_IDS",
    "MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_CAPABILITIES",
    "AutonomousWorkflowPortfolioCoverage",
    "AutonomousWorkflowPortfolioDependencyGraph",
    "AutonomousWorkflowPortfolioItem",
    "AutonomousWorkflowPortfolioItemRequest",
    "AutonomousWorkflowPortfolioPlan",
    "AutonomousWorkflowPortfolioVerification",
    "AutonomousWorkflowPortfolioRehydrationContext",
    "AutonomousWorkflowPortfolioExecutionCheckpoint",
    "AutonomousWorkflowPortfolioExecutionItem",
    "AutonomousWorkflowPortfolioExecutionResult",
    "plan_autonomous_workflow_portfolio",
    "verify_autonomous_workflow_portfolio",
    "execute_autonomous_workflow_portfolio",
]
