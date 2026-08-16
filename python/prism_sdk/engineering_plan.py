"""Typed deterministic engineering execution-plan requests and reports."""

from __future__ import annotations

from dataclasses import dataclass
import json
from typing import Any, Literal, Mapping, Sequence

from .engineering_manifest import (
    ENGINEERING_MANIFEST_MAX_LIST_ITEMS,
    EngineeringIssueReport,
    EngineeringManifestArgs,
    TicketStatus,
    _bool,
    _bounded_items,
    _mapping,
    _status,
    _text,
)
from .errors import ArgumentError

ENGINEERING_PLAN_REQUEST_SCHEMA = "bioprism-engineering-plan/0.1"
ENGINEERING_PLAN_AUDIT_SCHEMA = "bioprism-engineering-plan-audit/0.1"
ENGINEERING_PLAN_MAX_TICKETS = 100
ENGINEERING_PLAN_MAX_PARALLELISM = 100

EngineeringPlanIssueSeverity = Literal["warning", "blocking"]


def _positive(name: str, value: Any, maximum: int) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or value <= 0 or value > maximum:
        raise ArgumentError(f"{name} must be between 1 and {maximum}")
    return value


def _count(name: str, value: Any) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or value < 0:
        raise ArgumentError(f"{name} must be a non-negative integer")
    return value


@dataclass(frozen=True)
class EngineeringPlanPoliciesArgs:
    require_valid_manifest: bool = True
    allow_truncation: bool = False
    include_completed: bool = False
    serialize_same_package: bool = True
    max_tickets: int = ENGINEERING_PLAN_MAX_TICKETS
    max_parallelism: int = 16

    def __post_init__(self) -> None:
        for name in ("require_valid_manifest", "allow_truncation", "include_completed", "serialize_same_package"):
            object.__setattr__(self, name, _bool(f"plan policies.{name}", getattr(self, name)))
        object.__setattr__(self, "max_tickets", _positive("plan policies.max_tickets", self.max_tickets, ENGINEERING_PLAN_MAX_TICKETS))
        object.__setattr__(self, "max_parallelism", _positive("plan policies.max_parallelism", self.max_parallelism, ENGINEERING_PLAN_MAX_PARALLELISM))

    @classmethod
    def from_wire(cls, value: Mapping[str, Any] | None) -> "EngineeringPlanPoliciesArgs":
        raw = {} if value is None else _mapping("engineering plan policies", value)
        return cls(
            raw.get("require_valid_manifest", True),
            raw.get("allow_truncation", False),
            raw.get("include_completed", False),
            raw.get("serialize_same_package", True),
            raw.get("max_tickets", ENGINEERING_PLAN_MAX_TICKETS),
            raw.get("max_parallelism", 16),
        )

    def to_wire(self) -> dict[str, Any]:
        return {
            "require_valid_manifest": self.require_valid_manifest,
            "allow_truncation": self.allow_truncation,
            "include_completed": self.include_completed,
            "serialize_same_package": self.serialize_same_package,
            "max_tickets": self.max_tickets,
            "max_parallelism": self.max_parallelism,
        }


@dataclass(frozen=True)
class EngineeringPlanRequestArgs:
    manifest: EngineeringManifestArgs
    policies: EngineeringPlanPoliciesArgs = EngineeringPlanPoliciesArgs()
    schema: str = ENGINEERING_PLAN_REQUEST_SCHEMA

    def __post_init__(self) -> None:
        if not isinstance(self.manifest, EngineeringManifestArgs):
            object.__setattr__(self, "manifest", EngineeringManifestArgs.from_wire(self.manifest))
        if not isinstance(self.policies, EngineeringPlanPoliciesArgs):
            object.__setattr__(self, "policies", EngineeringPlanPoliciesArgs.from_wire(self.policies))
        if self.schema != ENGINEERING_PLAN_REQUEST_SCHEMA:
            raise ArgumentError(f"engineering plan schema must be {ENGINEERING_PLAN_REQUEST_SCHEMA}")

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "EngineeringPlanRequestArgs":
        raw = _mapping("engineering plan request", value)
        return cls(
            EngineeringManifestArgs.from_wire(raw.get("manifest")),
            EngineeringPlanPoliciesArgs.from_wire(raw.get("policies")),
            raw.get("schema", ENGINEERING_PLAN_REQUEST_SCHEMA),
        )  # type: ignore[arg-type]

    def to_wire(self) -> dict[str, Any]:
        return {
            "schema": self.schema,
            "manifest": self.manifest.to_wire(),
            "policies": self.policies.to_wire(),
        }

    def to_mcp_arguments(self) -> dict[str, Any]:
        return {"request": self.to_wire()}


@dataclass(frozen=True)
class EngineeringTicketPlanReport:
    ticket_id: str
    package: str
    contract: str
    status: TicketStatus
    state: str
    dependency_ids: tuple[str, ...]
    blocking_dependencies: tuple[str, ...]
    dependency_ready: bool
    scheduled: bool
    wave: int | None
    critical_path_length: int

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "EngineeringTicketPlanReport":
        raw = _mapping("engineering ticket plan", value)
        wave = raw.get("wave")
        if wave is not None:
            wave = _count("engineering ticket plan.wave", wave)
        return cls(
            _text("ticket plan.ticket_id", raw.get("ticket_id")),
            _text("ticket plan.package", raw.get("package")),
            _text("ticket plan.contract", raw.get("contract")),
            _status("ticket plan.status", raw.get("status"), frozenset({"planned", "in_progress", "blocked", "done"})),  # type: ignore[arg-type]
            _text("ticket plan.state", raw.get("state")),
            tuple(_text(f"ticket plan.dependency_ids[{index}]", item) for index, item in enumerate(_bounded_items("ticket plan.dependency_ids", raw.get("dependency_ids", []), ENGINEERING_MANIFEST_MAX_LIST_ITEMS))),  # type: ignore[misc]
            tuple(_text(f"ticket plan.blocking_dependencies[{index}]", item) for index, item in enumerate(_bounded_items("ticket plan.blocking_dependencies", raw.get("blocking_dependencies", []), ENGINEERING_MANIFEST_MAX_LIST_ITEMS))),  # type: ignore[misc]
            _bool("ticket plan.dependency_ready", raw.get("dependency_ready")),
            _bool("ticket plan.scheduled", raw.get("scheduled")),
            wave,
            _count("ticket plan.critical_path_length", raw.get("critical_path_length")),
        )


@dataclass(frozen=True)
class EngineeringPlanWaveReport:
    index: int
    ticket_ids: tuple[str, ...]
    package_ids: tuple[str, ...]
    depends_on_waves: tuple[int, ...]
    parallelism: int

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "EngineeringPlanWaveReport":
        raw = _mapping("engineering plan wave", value)
        return cls(
            _count("plan wave.index", raw.get("index")),
            tuple(_text(f"plan wave.ticket_ids[{index}]", item) for index, item in enumerate(_bounded_items("plan wave.ticket_ids", raw.get("ticket_ids", []), ENGINEERING_PLAN_MAX_TICKETS))),  # type: ignore[misc]
            tuple(_text(f"plan wave.package_ids[{index}]", item) for index, item in enumerate(_bounded_items("plan wave.package_ids", raw.get("package_ids", []), ENGINEERING_PLAN_MAX_TICKETS))),  # type: ignore[misc]
            tuple(_count(f"plan wave.depends_on_waves[{index}]", item) for index, item in enumerate(_bounded_items("plan wave.depends_on_waves", raw.get("depends_on_waves", []), ENGINEERING_PLAN_MAX_TICKETS))),
            _count("plan wave.parallelism", raw.get("parallelism")),
        )


@dataclass(frozen=True)
class EngineeringPlanGateReport:
    name: str
    passed: bool
    required: bool
    detail: str

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "EngineeringPlanGateReport":
        raw = _mapping("engineering plan gate", value)
        return cls(
            _text("plan gate.name", raw.get("name")),
            _bool("plan gate.passed", raw.get("passed")),
            _bool("plan gate.required", raw.get("required")),
            _text("plan gate.detail", raw.get("detail")),
        )


def _plan_payload(value: Mapping[str, Any]) -> dict[str, Any]:
    raw = _mapping("engineering plan response", value)
    candidates: list[Mapping[str, Any]] = [raw]

    def add(container: Any) -> None:
        if not isinstance(container, Mapping):
            return
        candidates.append(container)
        nested = container.get("result")
        if isinstance(nested, Mapping):
            candidates.append(nested)
            add(nested.get("structuredContent"))
            content = nested.get("content")
            if isinstance(content, Sequence) and not isinstance(content, (str, bytes)):
                for block in content:
                    if isinstance(block, Mapping) and isinstance(block.get("text"), str):
                        decoded = json.loads(block["text"])
                        if isinstance(decoded, Mapping):
                            candidates.append(decoded)
        add(container.get("structuredContent"))

    for key in ("mcp", "result", "structuredContent"):
        add(raw.get(key))
    for candidate in candidates:
        if candidate.get("schema") == ENGINEERING_PLAN_AUDIT_SCHEMA and "ok" in candidate:
            return dict(candidate)
    raise ArgumentError("response does not contain an engineering execution-plan projection")


@dataclass(frozen=True)
class EngineeringPlanReport:
    raw: dict[str, Any]
    ok: bool
    schema: str | None
    workflow: str | None
    request_digest: str | None
    manifest_digest: str | None
    plan_digest: str | None
    valid: bool | None
    engineering_plan_ready_value: bool | None
    planning_started: bool | None
    truncated: bool | None
    ticket_count: int | None
    planned_ticket_count: int | None
    omitted_ticket_count: int | None
    package_order: tuple[str, ...]
    ticket_plans: tuple[EngineeringTicketPlanReport, ...]
    waves: tuple[EngineeringPlanWaveReport, ...]
    critical_path: tuple[str, ...]
    gates: tuple[EngineeringPlanGateReport, ...]
    manifest_issues: tuple[EngineeringIssueReport, ...]
    issues: tuple[EngineeringIssueReport, ...]
    guarantees: tuple[str, ...]
    limitations: tuple[str, ...]
    refusal: str | None
    fail_closed: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "EngineeringPlanReport":
        raw = _plan_payload(value)
        if raw.get("ok") is not True:
            if raw.get("fail_closed") is not True:
                raise ArgumentError("engineering plan refusals must be fail-closed")
            return cls(
                raw=raw,
                ok=False,
                schema=raw.get("schema"),
                workflow=raw.get("workflow"),
                request_digest=raw.get("request_digest"),
                manifest_digest=raw.get("manifest_digest"),
                plan_digest=raw.get("plan_digest"),
                valid=False,
                engineering_plan_ready_value=False,
                planning_started=False,
                truncated=False,
                ticket_count=None,
                planned_ticket_count=None,
                omitted_ticket_count=None,
                package_order=(),
                ticket_plans=(),
                waves=(),
                critical_path=(),
                gates=(),
                manifest_issues=(),
                issues=(),
                guarantees=(),
                limitations=(),
                refusal=raw.get("refusal") or raw.get("error"),
                fail_closed=True,
            )
        if raw.get("schema") != ENGINEERING_PLAN_AUDIT_SCHEMA:
            raise ArgumentError("engineering plan projection has an invalid schema")
        audit = _mapping("engineering plan audit", raw.get("audit"))
        return cls(
            raw,
            True,
            ENGINEERING_PLAN_AUDIT_SCHEMA,
            _text("engineering plan workflow", raw.get("workflow")),
            _text("engineering plan request_digest", raw.get("request_digest"), required=False),
            _text("engineering plan manifest_digest", raw.get("manifest_digest"), required=False),
            _text("engineering plan plan_digest", raw.get("plan_digest"), required=False),
            _bool("engineering plan valid", audit.get("valid")),
            _bool("engineering_plan_ready", raw.get("engineering_plan_ready")),
            _bool("engineering plan planning_started", audit.get("planning_started")),
            _bool("engineering plan truncated", audit.get("truncated")),
            _count("engineering plan ticket_count", audit.get("ticket_count")),
            _count("engineering plan planned_ticket_count", audit.get("planned_ticket_count")),
            _count("engineering plan omitted_ticket_count", audit.get("omitted_ticket_count")),
            tuple(_text(f"engineering plan package_order[{index}]", item) for index, item in enumerate(_bounded_items("engineering plan package_order", audit.get("package_order", []), ENGINEERING_MANIFEST_MAX_LIST_ITEMS))),  # type: ignore[misc]
            tuple(EngineeringTicketPlanReport.from_wire(item) for item in _bounded_items("engineering ticket plans", audit.get("ticket_plans", []), ENGINEERING_PLAN_MAX_TICKETS)),
            tuple(EngineeringPlanWaveReport.from_wire(item) for item in _bounded_items("engineering plan waves", audit.get("waves", []), ENGINEERING_PLAN_MAX_TICKETS)),
            tuple(_text(f"engineering critical_path[{index}]", item) for index, item in enumerate(_bounded_items("engineering critical_path", audit.get("critical_path", []), ENGINEERING_PLAN_MAX_TICKETS))),  # type: ignore[misc]
            tuple(EngineeringPlanGateReport.from_wire(item) for item in _bounded_items("engineering plan gates", audit.get("gates", []), 32)),
            tuple(EngineeringIssueReport.from_wire(item) for item in _bounded_items("engineering manifest issues", audit.get("manifest_issues", []), ENGINEERING_MANIFEST_MAX_LIST_ITEMS)),
            tuple(EngineeringIssueReport.from_wire(item) for item in _bounded_items("engineering plan issues", audit.get("issues", []), ENGINEERING_MANIFEST_MAX_LIST_ITEMS)),
            tuple(str(item) for item in audit.get("guarantees", raw.get("guarantees", []))),
            tuple(str(item) for item in audit.get("limitations", raw.get("limitations", []))),
            None,
            False,
        )

    @property
    def accepted(self) -> bool:
        return self.ok and self.valid is True and self.engineering_plan_ready_value is True

    @property
    def engineering_plan_ready(self) -> bool:
        return self.engineering_plan_ready_value is True

    @property
    def blocking_issues(self) -> tuple[EngineeringIssueReport, ...]:
        return tuple(issue for issue in self.issues if issue.severity == "blocking")

    @property
    def warning_issues(self) -> tuple[EngineeringIssueReport, ...]:
        return tuple(issue for issue in self.issues if issue.severity == "warning")

    @property
    def has_blockers(self) -> bool:
        return bool(self.blocking_issues)

    @property
    def scheduled_ticket_ids(self) -> tuple[str, ...]:
        return tuple(plan.ticket_id for plan in self.ticket_plans if plan.scheduled)

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


def engineering_execution_plan_report(value: Mapping[str, Any]) -> EngineeringPlanReport:
    """Parse direct MCP output or an HTTP REST tool envelope."""

    return EngineeringPlanReport.from_wire(value)


__all__ = [
    "ENGINEERING_PLAN_REQUEST_SCHEMA",
    "ENGINEERING_PLAN_AUDIT_SCHEMA",
    "ENGINEERING_PLAN_MAX_TICKETS",
    "ENGINEERING_PLAN_MAX_PARALLELISM",
    "EngineeringPlanPoliciesArgs",
    "EngineeringPlanRequestArgs",
    "EngineeringTicketPlanReport",
    "EngineeringPlanWaveReport",
    "EngineeringPlanGateReport",
    "EngineeringPlanReport",
    "engineering_execution_plan_report",
]
