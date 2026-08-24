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
import math
from collections.abc import Mapping, Sequence
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
        depends_on=tuple(value.get("depends_on", ())),
        task_digest=value.get("task_digest"),
        request_digest=value.get("request_digest"),
        route_digest=value.get("route_digest"),
        workflow_id=value.get("workflow_id"),
        workflow_digest=value.get("workflow_digest"),
        plan_digest=value.get("plan_digest"),
        evidence_plan_digest=value.get("evidence_plan_digest"),
        stage_ids=tuple(value.get("stage_ids", ())),
        required_capabilities=tuple(value.get("required_capabilities", ())),
        status=value.get("status"),
        error_class=value.get("error_class"),
    )


def _coverage_from_dict(value: Any) -> AutonomousWorkflowPortfolioCoverage:
    if not isinstance(value, Mapping):
        raise BrainRunError("workflow portfolio coverage must be an object")
    return AutonomousWorkflowPortfolioCoverage(
        requested_domains=tuple(value.get("requested_domains", ())),
        ready_domains=tuple(value.get("ready_domains", ())),
        missing_domains=tuple(value.get("missing_domains", ())),
        duplicate_domain_items=tuple(value.get("duplicate_domain_items", ())),
        requested_item_count=value.get("requested_item_count"),
        ready_item_count=value.get("ready_item_count"),
        blocked_item_count=value.get("blocked_item_count"),
        failed_item_count=value.get("failed_item_count"),
        complete=value.get("complete"),
    )


def _graph_from_dict(value: Any) -> AutonomousWorkflowPortfolioDependencyGraph:
    if not isinstance(value, Mapping):
        raise BrainRunError("workflow portfolio dependency graph must be an object")
    waves = value.get("waves", ())
    if not isinstance(waves, Sequence) or isinstance(waves, (str, bytes)):
        raise BrainRunError("workflow portfolio dependency graph waves are invalid")
    return AutonomousWorkflowPortfolioDependencyGraph(
        topological_order=tuple(value.get("topological_order", ())),
        waves=tuple(tuple(wave) for wave in waves),
        cycle_item_ids=tuple(value.get("cycle_item_ids", ())),
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
                depends_on=tuple(dependencies),
                hints=tuple(raw.get("hints", ())),
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
    provisional: list[AutonomousWorkflowPortfolioItem] = []
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
    "plan_autonomous_workflow_portfolio",
    "verify_autonomous_workflow_portfolio",
]
