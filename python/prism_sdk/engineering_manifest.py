"""Typed engineering-manifest validation and audit projections.

The engineering manifest is a bounded, content-addressed artifact.  It describes a project's
technology baseline, package graph, implementation tickets, ADR history, and ownership rows.  The
SDK validates authoring shape and parses the server's audit without pretending to inspect a
checkout, execute tests, query GitHub, or grant release authority.
"""

from __future__ import annotations

from dataclasses import dataclass
import json
from typing import Any, Literal, Mapping, Sequence

from .capability import _route_mapping, _route_strings, _route_text
from .errors import ArgumentError


ENGINEERING_MANIFEST_SCHEMA = "bioprism-engineering-manifest/0.1"
ENGINEERING_AUDIT_SCHEMA = "bioprism-engineering-audit/0.1"
ENGINEERING_MANIFEST_MAX_INPUT_BYTES = 20_000_000
ENGINEERING_MANIFEST_MAX_PACKAGES = 4_096
ENGINEERING_MANIFEST_MAX_TICKETS = 10_000
ENGINEERING_MANIFEST_MAX_ADRS = 4_096
ENGINEERING_MANIFEST_MAX_OWNERSHIP = 4_096
ENGINEERING_MANIFEST_MAX_LIST_ITEMS = 16_384
ENGINEERING_MANIFEST_MAX_TEXT_BYTES = 4_096

TicketStatus = Literal["planned", "in_progress", "blocked", "done"]
AdrStatus = Literal["proposed", "accepted", "superseded", "rejected"]
IssueSeverity = Literal["warning", "blocking"]


def _mapping(name: str, value: Any) -> dict[str, Any]:
    return _route_mapping(name, value)


def _sequence(name: str, value: Any) -> tuple[Any, ...]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes, bytearray)):
        raise ArgumentError(f"{name} must be an array")
    return tuple(value)


def _text(name: str, value: Any, *, required: bool = True) -> str | None:
    if value is None and not required:
        return None
    result = _route_text(name, value)
    if required and not result.strip():
        raise ArgumentError(f"{name} must not be empty")
    if len(result.encode("utf-8")) > ENGINEERING_MANIFEST_MAX_TEXT_BYTES:
        raise ArgumentError(f"{name} exceeds {ENGINEERING_MANIFEST_MAX_TEXT_BYTES} UTF-8 bytes")
    return result


def _bool(name: str, value: Any) -> bool:
    if not isinstance(value, bool):
        raise ArgumentError(f"{name} must be a boolean")
    return value


def _bounded_items(name: str, value: Any, limit: int) -> tuple[Any, ...]:
    items = _sequence(name, value)
    if len(items) > limit:
        raise ArgumentError(f"{name} is bounded at {limit} items")
    return items


def _text_tuple(name: str, value: Any, *, limit: int = ENGINEERING_MANIFEST_MAX_LIST_ITEMS) -> tuple[str, ...]:
    return tuple(_text(f"{name}[{index}]", item) for index, item in enumerate(_bounded_items(name, value, limit)))  # type: ignore[misc]


def _status(name: str, value: Any, allowed: frozenset[str]) -> str:
    result = _text(name, value)
    assert result is not None
    if result not in allowed:
        raise ArgumentError(f"{name} must be one of {sorted(allowed)}")
    return result


def _json_size(name: str, value: Any) -> None:
    try:
        encoded = json.dumps(value, separators=(",", ":"), ensure_ascii=False).encode("utf-8")
    except (TypeError, ValueError) as error:
        raise ArgumentError(f"{name} must be JSON serializable: {error}") from error
    if len(encoded) > ENGINEERING_MANIFEST_MAX_INPUT_BYTES:
        raise ArgumentError(
            f"{name} exceeds the {ENGINEERING_MANIFEST_MAX_INPUT_BYTES}-byte safety bound"
        )


def _payload(value: Mapping[str, Any]) -> dict[str, Any]:
    raw = _mapping("engineering manifest response", value)
    candidates: list[Mapping[str, Any]] = [raw]

    def add_container(container: Any) -> None:
        if not isinstance(container, Mapping):
            return
        candidates.append(container)
        nested = container.get("result")
        if isinstance(nested, Mapping):
            candidates.append(nested)
            structured = nested.get("structuredContent")
            if isinstance(structured, Mapping):
                candidates.append(structured)
            content = nested.get("content")
            if isinstance(content, Sequence) and not isinstance(content, (str, bytes)):
                for block in content:
                    if not isinstance(block, Mapping) or not isinstance(block.get("text"), str):
                        continue
                    try:
                        decoded = json.loads(block["text"])
                    except json.JSONDecodeError as error:
                        raise ArgumentError(f"engineering manifest response text is not JSON: {error}") from error
                    if isinstance(decoded, Mapping):
                        candidates.append(decoded)
        structured = container.get("structuredContent")
        if isinstance(structured, Mapping):
            candidates.append(structured)

    add_container(raw.get("mcp"))
    add_container(raw.get("result"))
    add_container(raw.get("structuredContent"))
    for candidate in candidates:
        if candidate.get("schema") == ENGINEERING_AUDIT_SCHEMA and "ok" in candidate:
            return dict(candidate)
    raise ArgumentError("response does not contain an engineering manifest audit projection")


@dataclass(frozen=True)
class ProjectIdentityArgs:
    id: str
    version: str
    repository: str

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "ProjectIdentityArgs":
        raw = _mapping("engineering project", value)
        return cls(_text("project.id", raw.get("id")), _text("project.version", raw.get("version")), _text("project.repository", raw.get("repository")))  # type: ignore[arg-type]

    def to_wire(self) -> dict[str, Any]:
        return {"id": self.id, "version": self.version, "repository": self.repository}


@dataclass(frozen=True)
class TechnologyBaselineArgs:
    language: str
    runtime: str
    api: str
    storage: str
    observability: str
    deployment: str
    reasons: Mapping[str, str] = None  # type: ignore[assignment]

    def __post_init__(self) -> None:
        for name in ("language", "runtime", "api", "storage", "observability", "deployment"):
            _text(f"baseline.{name}", getattr(self, name))
        reasons = {} if self.reasons is None else _mapping("baseline.reasons", self.reasons)
        if len(reasons) > ENGINEERING_MANIFEST_MAX_LIST_ITEMS:
            raise ArgumentError("baseline.reasons is bounded at 16384 entries")
        normalized = {
            _text(f"baseline.reasons[{key}].key", key): _text(f"baseline.reasons[{key}].value", value)
            for key, value in reasons.items()
        }
        object.__setattr__(self, "reasons", normalized)

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "TechnologyBaselineArgs":
        raw = _mapping("engineering baseline", value)
        return cls(
            _text("baseline.language", raw.get("language")),
            _text("baseline.runtime", raw.get("runtime")),
            _text("baseline.api", raw.get("api")),
            _text("baseline.storage", raw.get("storage")),
            _text("baseline.observability", raw.get("observability")),
            _text("baseline.deployment", raw.get("deployment")),
            raw.get("reasons", {}),
        )  # type: ignore[arg-type]

    def to_wire(self) -> dict[str, Any]:
        return {
            "language": self.language,
            "runtime": self.runtime,
            "api": self.api,
            "storage": self.storage,
            "observability": self.observability,
            "deployment": self.deployment,
            "reasons": dict(self.reasons),
        }


@dataclass(frozen=True)
class PackageSpecArgs:
    id: str
    path: str
    language: str
    kind: str
    owner: str
    depends_on: tuple[str, ...] = ()
    public: bool = False
    test_command: str | None = None

    def __post_init__(self) -> None:
        for name in ("id", "path", "language", "kind", "owner"):
            _text(f"package.{name}", getattr(self, name))
        object.__setattr__(self, "depends_on", _text_tuple("package.depends_on", self.depends_on))
        object.__setattr__(self, "public", _bool("package.public", self.public))
        object.__setattr__(self, "test_command", _text("package.test_command", self.test_command, required=False))

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "PackageSpecArgs":
        raw = _mapping("engineering package", value)
        return cls(
            _text("package.id", raw.get("id")),
            _text("package.path", raw.get("path")),
            _text("package.language", raw.get("language")),
            _text("package.kind", raw.get("kind")),
            _text("package.owner", raw.get("owner")),
            _text_tuple("package.depends_on", raw.get("depends_on", [])),
            raw.get("public", False),
            raw.get("test_command"),
        )  # type: ignore[arg-type]

    def to_wire(self) -> dict[str, Any]:
        result = {
            "id": self.id,
            "path": self.path,
            "language": self.language,
            "kind": self.kind,
            "owner": self.owner,
            "depends_on": list(self.depends_on),
            "public": self.public,
        }
        if self.test_command is not None:
            result["test_command"] = self.test_command
        return result


@dataclass(frozen=True)
class TicketSpecArgs:
    id: str
    title: str
    package: str
    contract: str
    status: TicketStatus
    depends_on: tuple[str, ...] = ()
    acceptance: tuple[str, ...] = ()
    blocker: str | None = None

    def __post_init__(self) -> None:
        for name in ("id", "title", "package", "contract"):
            _text(f"ticket.{name}", getattr(self, name))
        object.__setattr__(self, "status", _status("ticket.status", self.status, frozenset({"planned", "in_progress", "blocked", "done"})))
        object.__setattr__(self, "depends_on", _text_tuple("ticket.depends_on", self.depends_on))
        acceptance = _text_tuple("ticket.acceptance", self.acceptance)
        if not acceptance:
            raise ArgumentError("ticket.acceptance must contain at least one condition")
        object.__setattr__(self, "acceptance", acceptance)
        object.__setattr__(self, "blocker", _text("ticket.blocker", self.blocker, required=False))

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "TicketSpecArgs":
        raw = _mapping("engineering ticket", value)
        return cls(
            _text("ticket.id", raw.get("id")),
            _text("ticket.title", raw.get("title")),
            _text("ticket.package", raw.get("package")),
            _text("ticket.contract", raw.get("contract")),
            _status("ticket.status", raw.get("status"), frozenset({"planned", "in_progress", "blocked", "done"})),  # type: ignore[arg-type]
            _text_tuple("ticket.depends_on", raw.get("depends_on", [])),
            _text_tuple("ticket.acceptance", raw.get("acceptance", [])),
            raw.get("blocker"),
        )  # type: ignore[arg-type]

    def to_wire(self) -> dict[str, Any]:
        result = {
            "id": self.id,
            "title": self.title,
            "package": self.package,
            "contract": self.contract,
            "status": self.status,
            "depends_on": list(self.depends_on),
            "acceptance": list(self.acceptance),
        }
        if self.blocker is not None:
            result["blocker"] = self.blocker
        return result


@dataclass(frozen=True)
class AdrSpecArgs:
    id: str
    title: str
    status: AdrStatus
    decision: str
    affects: tuple[str, ...]
    supersedes: str | None = None

    def __post_init__(self) -> None:
        for name in ("id", "title", "decision"):
            _text(f"adr.{name}", getattr(self, name))
        object.__setattr__(self, "status", _status("adr.status", self.status, frozenset({"proposed", "accepted", "superseded", "rejected"})))
        affects = _text_tuple("adr.affects", self.affects)
        if not affects:
            raise ArgumentError("adr.affects must name at least one surface")
        object.__setattr__(self, "affects", affects)
        object.__setattr__(self, "supersedes", _text("adr.supersedes", self.supersedes, required=False))

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "AdrSpecArgs":
        raw = _mapping("engineering ADR", value)
        return cls(
            _text("adr.id", raw.get("id")),
            _text("adr.title", raw.get("title")),
            _status("adr.status", raw.get("status"), frozenset({"proposed", "accepted", "superseded", "rejected"})),  # type: ignore[arg-type]
            _text("adr.decision", raw.get("decision")),
            _text_tuple("adr.affects", raw.get("affects", [])),
            raw.get("supersedes"),
        )  # type: ignore[arg-type]

    def to_wire(self) -> dict[str, Any]:
        result = {
            "id": self.id,
            "title": self.title,
            "status": self.status,
            "decision": self.decision,
            "affects": list(self.affects),
        }
        if self.supersedes is not None:
            result["supersedes"] = self.supersedes
        return result


@dataclass(frozen=True)
class OwnershipSpecArgs:
    surface: str
    accountable: str
    responsible: tuple[str, ...]
    consulted: tuple[str, ...] = ()
    informed: tuple[str, ...] = ()
    independent_reviewer: str | None = None

    def __post_init__(self) -> None:
        _text("ownership.surface", self.surface)
        _text("ownership.accountable", self.accountable)
        responsible = _text_tuple("ownership.responsible", self.responsible)
        if not responsible:
            raise ArgumentError("ownership.responsible must contain at least one party")
        object.__setattr__(self, "responsible", responsible)
        object.__setattr__(self, "consulted", _text_tuple("ownership.consulted", self.consulted))
        object.__setattr__(self, "informed", _text_tuple("ownership.informed", self.informed))
        object.__setattr__(self, "independent_reviewer", _text("ownership.independent_reviewer", self.independent_reviewer, required=False))

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "OwnershipSpecArgs":
        raw = _mapping("engineering ownership", value)
        return cls(
            _text("ownership.surface", raw.get("surface")),
            _text("ownership.accountable", raw.get("accountable")),
            _text_tuple("ownership.responsible", raw.get("responsible", [])),
            _text_tuple("ownership.consulted", raw.get("consulted", [])),
            _text_tuple("ownership.informed", raw.get("informed", [])),
            raw.get("independent_reviewer"),
        )  # type: ignore[arg-type]

    def to_wire(self) -> dict[str, Any]:
        result = {
            "surface": self.surface,
            "accountable": self.accountable,
            "responsible": list(self.responsible),
            "consulted": list(self.consulted),
            "informed": list(self.informed),
        }
        if self.independent_reviewer is not None:
            result["independent_reviewer"] = self.independent_reviewer
        return result


@dataclass(frozen=True)
class EngineeringPoliciesArgs:
    require_acyclic_packages: bool = True
    require_ticket_contracts: bool = True
    require_ownership: bool = True
    require_adr_targets: bool = True

    def __post_init__(self) -> None:
        for name in ("require_acyclic_packages", "require_ticket_contracts", "require_ownership", "require_adr_targets"):
            _bool(f"policies.{name}", getattr(self, name))

    @classmethod
    def from_wire(cls, value: Mapping[str, Any] | None) -> "EngineeringPoliciesArgs":
        raw = {} if value is None else _mapping("engineering policies", value)
        return cls(
            raw.get("require_acyclic_packages", True),
            raw.get("require_ticket_contracts", True),
            raw.get("require_ownership", True),
            raw.get("require_adr_targets", True),
        )

    def to_wire(self) -> dict[str, Any]:
        return {
            "require_acyclic_packages": self.require_acyclic_packages,
            "require_ticket_contracts": self.require_ticket_contracts,
            "require_ownership": self.require_ownership,
            "require_adr_targets": self.require_adr_targets,
        }


@dataclass(frozen=True, init=False)
class EngineeringManifestArgs:
    schema: str
    project: ProjectIdentityArgs
    baseline: TechnologyBaselineArgs
    packages: tuple[PackageSpecArgs, ...]
    tickets: tuple[TicketSpecArgs, ...]
    adrs: tuple[AdrSpecArgs, ...]
    ownership: tuple[OwnershipSpecArgs, ...]
    policies: EngineeringPoliciesArgs

    def __init__(
        self,
        project: ProjectIdentityArgs | Mapping[str, Any],
        baseline: TechnologyBaselineArgs | Mapping[str, Any],
        packages: Sequence[PackageSpecArgs | Mapping[str, Any]] = (),
        tickets: Sequence[TicketSpecArgs | Mapping[str, Any]] = (),
        adrs: Sequence[AdrSpecArgs | Mapping[str, Any]] = (),
        ownership: Sequence[OwnershipSpecArgs | Mapping[str, Any]] = (),
        policies: EngineeringPoliciesArgs | Mapping[str, Any] | None = None,
        schema: str = ENGINEERING_MANIFEST_SCHEMA,
    ) -> None:
        normalized_schema = _text("engineering manifest schema", schema)
        normalized_project = project if isinstance(project, ProjectIdentityArgs) else ProjectIdentityArgs.from_wire(project)
        normalized_baseline = baseline if isinstance(baseline, TechnologyBaselineArgs) else TechnologyBaselineArgs.from_wire(baseline)
        package_values = _bounded_items("engineering packages", packages, ENGINEERING_MANIFEST_MAX_PACKAGES)
        ticket_values = _bounded_items("engineering tickets", tickets, ENGINEERING_MANIFEST_MAX_TICKETS)
        adr_values = _bounded_items("engineering adrs", adrs, ENGINEERING_MANIFEST_MAX_ADRS)
        ownership_values = _bounded_items("engineering ownership", ownership, ENGINEERING_MANIFEST_MAX_OWNERSHIP)
        normalized_policies = (
            policies if isinstance(policies, EngineeringPoliciesArgs) else EngineeringPoliciesArgs.from_wire(policies)
        )
        normalized_packages = tuple(item if isinstance(item, PackageSpecArgs) else PackageSpecArgs.from_wire(item) for item in package_values)
        normalized_tickets = tuple(item if isinstance(item, TicketSpecArgs) else TicketSpecArgs.from_wire(item) for item in ticket_values)
        normalized_adrs = tuple(item if isinstance(item, AdrSpecArgs) else AdrSpecArgs.from_wire(item) for item in adr_values)
        normalized_ownership = tuple(item if isinstance(item, OwnershipSpecArgs) else OwnershipSpecArgs.from_wire(item) for item in ownership_values)
        wire = {
            "schema": normalized_schema,
            "project": normalized_project.to_wire(),
            "baseline": normalized_baseline.to_wire(),
            "packages": [item.to_wire() for item in normalized_packages],
            "tickets": [item.to_wire() for item in normalized_tickets],
            "adrs": [item.to_wire() for item in normalized_adrs],
            "ownership": [item.to_wire() for item in normalized_ownership],
            "policies": normalized_policies.to_wire(),
        }
        _json_size("engineering manifest", wire)
        object.__setattr__(self, "schema", normalized_schema)
        object.__setattr__(self, "project", normalized_project)
        object.__setattr__(self, "baseline", normalized_baseline)
        object.__setattr__(self, "packages", normalized_packages)
        object.__setattr__(self, "tickets", normalized_tickets)
        object.__setattr__(self, "adrs", normalized_adrs)
        object.__setattr__(self, "ownership", normalized_ownership)
        object.__setattr__(self, "policies", normalized_policies)

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "EngineeringManifestArgs":
        raw = _mapping("engineering manifest", value)
        return cls(
            raw.get("project"),
            raw.get("baseline"),
            _bounded_items("engineering packages", raw.get("packages", []), ENGINEERING_MANIFEST_MAX_PACKAGES),
            _bounded_items("engineering tickets", raw.get("tickets", []), ENGINEERING_MANIFEST_MAX_TICKETS),
            _bounded_items("engineering adrs", raw.get("adrs", []), ENGINEERING_MANIFEST_MAX_ADRS),
            _bounded_items("engineering ownership", raw.get("ownership", []), ENGINEERING_MANIFEST_MAX_OWNERSHIP),
            raw.get("policies"),
            raw.get("schema", ENGINEERING_MANIFEST_SCHEMA),
        )

    def to_wire(self) -> dict[str, Any]:
        return {
            "schema": self.schema,
            "project": self.project.to_wire(),
            "baseline": self.baseline.to_wire(),
            "packages": [item.to_wire() for item in self.packages],
            "tickets": [item.to_wire() for item in self.tickets],
            "adrs": [item.to_wire() for item in self.adrs],
            "ownership": [item.to_wire() for item in self.ownership],
            "policies": self.policies.to_wire(),
        }

    def to_mcp_arguments(self) -> dict[str, Any]:
        return {"manifest": self.to_wire()}


@dataclass(frozen=True)
class EngineeringIssueReport:
    code: str
    severity: IssueSeverity
    subject: str
    detail: str
    remediation: str

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "EngineeringIssueReport":
        raw = _mapping("engineering issue", value)
        severity = _status("engineering issue severity", raw.get("severity"), frozenset({"warning", "blocking"}))
        return cls(_text("engineering issue code", raw.get("code")), severity, _text("engineering issue subject", raw.get("subject")), _text("engineering issue detail", raw.get("detail")), _text("engineering issue remediation", raw.get("remediation")))  # type: ignore[arg-type]


@dataclass(frozen=True)
class EngineeringTicketReadinessReport:
    ticket_id: str
    status: TicketStatus
    state: str
    blocking_dependencies: tuple[str, ...]
    dependency_ready: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "EngineeringTicketReadinessReport":
        raw = _mapping("engineering ticket readiness", value)
        return cls(
            _text("ticket readiness ticket_id", raw.get("ticket_id")),
            _status("ticket readiness status", raw.get("status"), frozenset({"planned", "in_progress", "blocked", "done"})),  # type: ignore[arg-type]
            _text("ticket readiness state", raw.get("state")),
            _text_tuple("ticket readiness blocking_dependencies", raw.get("blocking_dependencies", [])),
            _bool("ticket readiness dependency_ready", raw.get("dependency_ready")),
        )  # type: ignore[arg-type]


@dataclass(frozen=True)
class EngineeringAuditReport:
    raw: dict[str, Any]
    ok: bool
    schema: str | None
    workflow: str | None
    manifest_digest: str | None
    valid: bool | None
    counts: Mapping[str, Any] | None
    package_order: tuple[str, ...]
    cyclic_packages: tuple[tuple[str, ...], ...]
    ticket_readiness: tuple[EngineeringTicketReadinessReport, ...]
    adr_supersession: tuple[Mapping[str, Any], ...]
    ownership_surfaces: tuple[str, ...]
    issues: tuple[EngineeringIssueReport, ...]
    guarantees: tuple[str, ...]
    limitations: tuple[str, ...]
    stage: str | None
    refusal: str | None
    fail_closed: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "EngineeringAuditReport":
        raw = _payload(value)
        ok = raw.get("ok") is True
        if not ok:
            if raw.get("fail_closed") is not True:
                raise ArgumentError("engineering manifest refusals must be fail-closed")
            return cls(
                raw,
                False,
                raw.get("schema"),
                raw.get("workflow"),
                raw.get("manifest_digest"),
                False,
                None,
                (),
                (),
                (),
                (),
                (),
                (),
                _route_strings("engineering refusal guarantees", raw.get("guarantees", [])),
                _route_strings("engineering refusal limitations", raw.get("limitations", [])),
                raw.get("stage"),
                raw.get("refusal"),
                True,
            )
        if raw.get("schema") != ENGINEERING_AUDIT_SCHEMA:
            raise ArgumentError("engineering manifest projection has an invalid schema")
        audit = _mapping("engineering audit", raw.get("audit"))
        issues = tuple(
            EngineeringIssueReport.from_wire(item)
            for item in _bounded_items("engineering audit issues", audit.get("issues", []), ENGINEERING_MANIFEST_MAX_LIST_ITEMS)
        )
        readiness = tuple(
            EngineeringTicketReadinessReport.from_wire(item)
            for item in _bounded_items("engineering ticket readiness", audit.get("ticket_readiness", []), ENGINEERING_MANIFEST_MAX_TICKETS)
        )
        cycles = tuple(
            _text_tuple(f"engineering cycle[{index}]", item)
            for index, item in enumerate(_bounded_items("engineering cyclic_packages", audit.get("cyclic_packages", []), ENGINEERING_MANIFEST_MAX_PACKAGES))
        )
        return cls(
            raw,
            True,
            ENGINEERING_AUDIT_SCHEMA,
            _text("engineering workflow", raw.get("workflow")),
            _text("engineering manifest_digest", raw.get("manifest_digest"), required=False),
            _bool("engineering audit valid", audit.get("valid")),
            _mapping("engineering audit counts", audit.get("counts")),
            _text_tuple("engineering package_order", audit.get("package_order", [])),
            cycles,
            readiness,
            tuple(_mapping("engineering ADR supersession", item) for item in _bounded_items("engineering adr_supersession", audit.get("adr_supersession", []), ENGINEERING_MANIFEST_MAX_ADRS)),
            _text_tuple("engineering ownership_surfaces", audit.get("ownership_surfaces", [])),
            issues,
            _route_strings("engineering guarantees", raw.get("guarantees", audit.get("guarantees", []))),
            _route_strings("engineering limitations", raw.get("limitations", audit.get("limitations", []))),
            None,
            None,
            False,
        )

    @property
    def accepted(self) -> bool:
        return self.ok and self.valid is True

    @property
    def refused(self) -> bool:
        return not self.ok

    @property
    def blocking_issues(self) -> tuple[EngineeringIssueReport, ...]:
        return tuple(issue for issue in self.issues if issue.severity == "blocking")

    @property
    def warning_issues(self) -> tuple[EngineeringIssueReport, ...]:
        return tuple(issue for issue in self.issues if issue.severity == "warning")

    @property
    def actionable_tickets(self) -> tuple[str, ...]:
        return tuple(item.ticket_id for item in self.ticket_readiness if item.state == "actionable")

    @property
    def has_blockers(self) -> bool:
        return bool(self.blocking_issues)

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


def engineering_manifest_audit_report(value: Mapping[str, Any]) -> EngineeringAuditReport:
    """Parse direct MCP output or an HTTP REST tool envelope."""

    return EngineeringAuditReport.from_wire(value)


__all__ = [
    "ENGINEERING_MANIFEST_SCHEMA",
    "ENGINEERING_AUDIT_SCHEMA",
    "ENGINEERING_MANIFEST_MAX_INPUT_BYTES",
    "ENGINEERING_MANIFEST_MAX_PACKAGES",
    "ENGINEERING_MANIFEST_MAX_TICKETS",
    "ENGINEERING_MANIFEST_MAX_ADRS",
    "ENGINEERING_MANIFEST_MAX_OWNERSHIP",
    "ProjectIdentityArgs",
    "TechnologyBaselineArgs",
    "PackageSpecArgs",
    "TicketSpecArgs",
    "AdrSpecArgs",
    "OwnershipSpecArgs",
    "EngineeringPoliciesArgs",
    "EngineeringManifestArgs",
    "EngineeringIssueReport",
    "EngineeringTicketReadinessReport",
    "EngineeringAuditReport",
    "engineering_manifest_audit_report",
]
