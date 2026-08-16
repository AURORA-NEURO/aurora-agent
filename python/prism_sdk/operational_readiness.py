"""Typed operational-readiness manifests and fail-closed report projections.

The facade validates authoring shape and preserves the Rust audit's separate evidence layers. It
does not query telemetry, page operators, inspect dependencies, create incidents, or infer uptime
from a caller-declared boolean.
"""

from __future__ import annotations

from dataclasses import dataclass
import json
from typing import Any, Literal, Mapping, Sequence

from .capability import _route_mapping, _route_strings, _route_text
from .errors import ArgumentError


OPERATIONAL_READINESS_MANIFEST_SCHEMA = "bioprism-operational-readiness/0.1"
OPERATIONAL_READINESS_AUDIT_SCHEMA = "bioprism-operational-readiness-audit/0.1"
OPERATIONAL_READINESS_MAX_INPUT_BYTES = 20_000_000
OPERATIONAL_READINESS_MAX_CONTRACTS = 4_096
OPERATIONAL_READINESS_MAX_INDICATORS = 8_192
OPERATIONAL_READINESS_MAX_DEPENDENCIES = 8_192
OPERATIONAL_READINESS_MAX_RUNBOOKS = 4_096
OPERATIONAL_READINESS_MAX_INCIDENTS = 4_096
OPERATIONAL_READINESS_MAX_LIST_ITEMS = 16_384
OPERATIONAL_READINESS_MAX_TEXT_BYTES = 4_096

OperationalCriticality = Literal["critical", "important", "advisory"]
OperationalContractKind = Literal["availability", "latency", "durability", "recovery", "security", "privacy", "capacity"]
IndicatorStatus = Literal["observed", "not_observed", "blocked", "not_applicable"]
DependencyCriticality = Literal["critical", "important", "advisory"]
RunbookReviewStatus = Literal["draft", "reviewed", "expired"]
IncidentSeverity = Literal["sev1", "sev2", "sev3", "sev4"]
IncidentState = Literal["open", "contained", "resolved", "closed"]
OperationalIssueSeverity = Literal["warning", "blocking"]


def _mapping(name: str, value: Any) -> dict[str, Any]:
    return _route_mapping(name, value)


def _sequence(name: str, value: Any) -> tuple[Any, ...]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes, bytearray)):
        raise ArgumentError(f"{name} must be an array")
    return tuple(value)


def _bounded(name: str, value: Any, limit: int) -> tuple[Any, ...]:
    result = _sequence(name, value)
    if len(result) > limit:
        raise ArgumentError(f"{name} is bounded at {limit} items")
    return result


def _text(name: str, value: Any, *, required: bool = True) -> str | None:
    if value is None and not required:
        return None
    result = _route_text(name, value)
    if required and not result.strip():
        raise ArgumentError(f"{name} must not be empty")
    if len(result.encode("utf-8")) > OPERATIONAL_READINESS_MAX_TEXT_BYTES:
        raise ArgumentError(f"{name} exceeds {OPERATIONAL_READINESS_MAX_TEXT_BYTES} UTF-8 bytes")
    return result


def _text_tuple(name: str, value: Any) -> tuple[str, ...]:
    return tuple(_text(f"{name}[{index}]", item) for index, item in enumerate(_bounded(name, value, OPERATIONAL_READINESS_MAX_LIST_ITEMS)))  # type: ignore[misc]


def _bool(name: str, value: Any) -> bool:
    if not isinstance(value, bool):
        raise ArgumentError(f"{name} must be a boolean")
    return value


def _enum(name: str, value: Any, allowed: frozenset[str]) -> str:
    result = _text(name, value)
    assert result is not None
    if result not in allowed:
        raise ArgumentError(f"{name} must be one of {sorted(allowed)}")
    return result


def _digest(name: str, value: Any) -> str:
    result = _text(name, value)
    assert result is not None
    if len(result) != 64 or any(character not in "0123456789abcdefABCDEF" for character in result):
        raise ArgumentError(f"{name} must be 64 hexadecimal characters")
    return result


def _json_size(name: str, value: Any) -> None:
    try:
        encoded = json.dumps(value, separators=(",", ":"), ensure_ascii=False).encode("utf-8")
    except (TypeError, ValueError) as error:
        raise ArgumentError(f"{name} must be JSON serializable: {error}") from error
    if len(encoded) > OPERATIONAL_READINESS_MAX_INPUT_BYTES:
        raise ArgumentError(f"{name} exceeds the {OPERATIONAL_READINESS_MAX_INPUT_BYTES}-byte safety bound")


@dataclass(frozen=True)
class OperationalServiceArgs:
    id: str
    version: str
    owner: str
    criticality: OperationalCriticality

    def __post_init__(self) -> None:
        for name in ("id", "version", "owner"):
            _text(f"service.{name}", getattr(self, name))
        object.__setattr__(self, "criticality", _enum("service.criticality", self.criticality, frozenset({"critical", "important", "advisory"})))

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "OperationalServiceArgs":
        raw = _mapping("operational service", value)
        return cls(_text("service.id", raw.get("id")), _text("service.version", raw.get("version")), _text("service.owner", raw.get("owner")), _enum("service.criticality", raw.get("criticality"), frozenset({"critical", "important", "advisory"})))  # type: ignore[arg-type]

    def to_wire(self) -> dict[str, Any]:
        return {"id": self.id, "version": self.version, "owner": self.owner, "criticality": self.criticality}


@dataclass(frozen=True)
class OperationalContractArgs:
    id: str
    kind: OperationalContractKind
    objective: str
    target: str
    required: bool = False

    def __post_init__(self) -> None:
        _text("contract.id", self.id)
        object.__setattr__(self, "kind", _enum("contract.kind", self.kind, frozenset({"availability", "latency", "durability", "recovery", "security", "privacy", "capacity"})))
        _text("contract.objective", self.objective)
        _text("contract.target", self.target)
        object.__setattr__(self, "required", _bool("contract.required", self.required))

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "OperationalContractArgs":
        raw = _mapping("operational contract", value)
        return cls(_text("contract.id", raw.get("id")), _enum("contract.kind", raw.get("kind"), frozenset({"availability", "latency", "durability", "recovery", "security", "privacy", "capacity"})), _text("contract.objective", raw.get("objective")), _text("contract.target", raw.get("target")), raw.get("required", False))  # type: ignore[arg-type]

    def to_wire(self) -> dict[str, Any]:
        return {"id": self.id, "kind": self.kind, "objective": self.objective, "target": self.target, "required": self.required}


@dataclass(frozen=True)
class OperationalIndicatorArgs:
    id: str
    contract: str
    metric: str
    source: str
    status: IndicatorStatus
    measurement: str | None = None
    evidence_digest: str | None = None

    def __post_init__(self) -> None:
        _text("indicator.id", self.id)
        _text("indicator.contract", self.contract)
        _text("indicator.metric", self.metric)
        _text("indicator.source", self.source)
        object.__setattr__(self, "status", _enum("indicator.status", self.status, frozenset({"observed", "not_observed", "blocked", "not_applicable"})))
        object.__setattr__(self, "measurement", _text("indicator.measurement", self.measurement, required=False))
        object.__setattr__(self, "evidence_digest", None if self.evidence_digest is None else _digest("indicator.evidence_digest", self.evidence_digest))

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "OperationalIndicatorArgs":
        raw = _mapping("operational indicator", value)
        return cls(_text("indicator.id", raw.get("id")), _text("indicator.contract", raw.get("contract")), _text("indicator.metric", raw.get("metric")), _text("indicator.source", raw.get("source")), _enum("indicator.status", raw.get("status"), frozenset({"observed", "not_observed", "blocked", "not_applicable"})), raw.get("measurement"), raw.get("evidence_digest"))  # type: ignore[arg-type]

    def to_wire(self) -> dict[str, Any]:
        result: dict[str, Any] = {"id": self.id, "contract": self.contract, "metric": self.metric, "source": self.source, "status": self.status}
        if self.measurement is not None:
            result["measurement"] = self.measurement
        if self.evidence_digest is not None:
            result["evidence_digest"] = self.evidence_digest
        return result


@dataclass(frozen=True)
class OperationalDependencyArgs:
    id: str
    name: str
    owner: str
    criticality: DependencyCriticality
    failure_mode: str
    fallback: str | None = None

    def __post_init__(self) -> None:
        for name in ("id", "name", "owner", "failure_mode"):
            _text(f"dependency.{name}", getattr(self, name))
        object.__setattr__(self, "criticality", _enum("dependency.criticality", self.criticality, frozenset({"critical", "important", "advisory"})))
        object.__setattr__(self, "fallback", _text("dependency.fallback", self.fallback, required=False))

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "OperationalDependencyArgs":
        raw = _mapping("operational dependency", value)
        return cls(_text("dependency.id", raw.get("id")), _text("dependency.name", raw.get("name")), _text("dependency.owner", raw.get("owner")), _enum("dependency.criticality", raw.get("criticality"), frozenset({"critical", "important", "advisory"})), _text("dependency.failure_mode", raw.get("failure_mode")), raw.get("fallback"))  # type: ignore[arg-type]

    def to_wire(self) -> dict[str, Any]:
        result: dict[str, Any] = {"id": self.id, "name": self.name, "owner": self.owner, "criticality": self.criticality, "failure_mode": self.failure_mode}
        if self.fallback is not None:
            result["fallback"] = self.fallback
        return result


@dataclass(frozen=True)
class OperationalRunbookArgs:
    id: str
    trigger: str
    owner: str
    steps: tuple[str, ...]
    review_status: RunbookReviewStatus
    incident_classes: tuple[str, ...] = ()

    def __post_init__(self) -> None:
        for name in ("id", "trigger", "owner"):
            _text(f"runbook.{name}", getattr(self, name))
        object.__setattr__(self, "steps", _text_tuple("runbook.steps", self.steps))
        if not self.steps:
            raise ArgumentError("runbook.steps must contain at least one step")
        object.__setattr__(self, "review_status", _enum("runbook.review_status", self.review_status, frozenset({"draft", "reviewed", "expired"})))
        object.__setattr__(self, "incident_classes", _text_tuple("runbook.incident_classes", self.incident_classes))

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "OperationalRunbookArgs":
        raw = _mapping("operational runbook", value)
        return cls(_text("runbook.id", raw.get("id")), _text("runbook.trigger", raw.get("trigger")), _text("runbook.owner", raw.get("owner")), _text_tuple("runbook.steps", raw.get("steps", [])), _enum("runbook.review_status", raw.get("review_status"), frozenset({"draft", "reviewed", "expired"})), _text_tuple("runbook.incident_classes", raw.get("incident_classes", [])))  # type: ignore[arg-type]

    def to_wire(self) -> dict[str, Any]:
        return {"id": self.id, "trigger": self.trigger, "owner": self.owner, "steps": list(self.steps), "review_status": self.review_status, "incident_classes": list(self.incident_classes)}


@dataclass(frozen=True)
class OperationalIncidentArgs:
    id: str
    severity: IncidentSeverity
    state: IncidentState
    runbook: str
    owner: str
    timeline: tuple[str, ...] = ()
    postmortem: str | None = None

    def __post_init__(self) -> None:
        _text("incident.id", self.id)
        object.__setattr__(self, "severity", _enum("incident.severity", self.severity, frozenset({"sev1", "sev2", "sev3", "sev4"})))
        object.__setattr__(self, "state", _enum("incident.state", self.state, frozenset({"open", "contained", "resolved", "closed"})))
        _text("incident.runbook", self.runbook)
        _text("incident.owner", self.owner)
        object.__setattr__(self, "timeline", _text_tuple("incident.timeline", self.timeline))
        object.__setattr__(self, "postmortem", _text("incident.postmortem", self.postmortem, required=False))

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "OperationalIncidentArgs":
        raw = _mapping("operational incident", value)
        return cls(_text("incident.id", raw.get("id")), _enum("incident.severity", raw.get("severity"), frozenset({"sev1", "sev2", "sev3", "sev4"})), _enum("incident.state", raw.get("state"), frozenset({"open", "contained", "resolved", "closed"})), _text("incident.runbook", raw.get("runbook")), _text("incident.owner", raw.get("owner")), _text_tuple("incident.timeline", raw.get("timeline", [])), raw.get("postmortem"))  # type: ignore[arg-type]

    def to_wire(self) -> dict[str, Any]:
        result: dict[str, Any] = {"id": self.id, "severity": self.severity, "state": self.state, "runbook": self.runbook, "owner": self.owner, "timeline": list(self.timeline)}
        if self.postmortem is not None:
            result["postmortem"] = self.postmortem
        return result


@dataclass(frozen=True)
class OperationalControlsArgs:
    on_call: bool = False
    alerting: bool = False
    tracing: bool = False
    audit_logging: bool = False
    backup: bool = False
    restore_test: bool = False
    access_review: bool = False

    def __post_init__(self) -> None:
        for name in ("on_call", "alerting", "tracing", "audit_logging", "backup", "restore_test", "access_review"):
            _bool(f"controls.{name}", getattr(self, name))

    @classmethod
    def from_wire(cls, value: Mapping[str, Any] | None) -> "OperationalControlsArgs":
        raw = {} if value is None else _mapping("operational controls", value)
        return cls(raw.get("on_call", False), raw.get("alerting", False), raw.get("tracing", False), raw.get("audit_logging", False), raw.get("backup", False), raw.get("restore_test", False), raw.get("access_review", False))

    def to_wire(self) -> dict[str, Any]:
        return {"on_call": self.on_call, "alerting": self.alerting, "tracing": self.tracing, "audit_logging": self.audit_logging, "backup": self.backup, "restore_test": self.restore_test, "access_review": self.access_review}


@dataclass(frozen=True)
class OperationalReadinessPoliciesArgs:
    require_contract_evidence: bool = True
    require_observability: bool = True
    require_runbooks: bool = True
    require_restore_test: bool = True
    require_dependency_fallback: bool = True
    require_incident_closure: bool = True
    require_access_review: bool = True

    def __post_init__(self) -> None:
        for name in ("require_contract_evidence", "require_observability", "require_runbooks", "require_restore_test", "require_dependency_fallback", "require_incident_closure", "require_access_review"):
            _bool(f"policies.{name}", getattr(self, name))

    @classmethod
    def from_wire(cls, value: Mapping[str, Any] | None) -> "OperationalReadinessPoliciesArgs":
        raw = {} if value is None else _mapping("operational policies", value)
        return cls(raw.get("require_contract_evidence", True), raw.get("require_observability", True), raw.get("require_runbooks", True), raw.get("require_restore_test", True), raw.get("require_dependency_fallback", True), raw.get("require_incident_closure", True), raw.get("require_access_review", True))

    def to_wire(self) -> dict[str, Any]:
        return {"require_contract_evidence": self.require_contract_evidence, "require_observability": self.require_observability, "require_runbooks": self.require_runbooks, "require_restore_test": self.require_restore_test, "require_dependency_fallback": self.require_dependency_fallback, "require_incident_closure": self.require_incident_closure, "require_access_review": self.require_access_review}


@dataclass(frozen=True, init=False)
class OperationalReadinessManifestArgs:
    schema: str
    service: OperationalServiceArgs
    contracts: tuple[OperationalContractArgs, ...]
    indicators: tuple[OperationalIndicatorArgs, ...]
    dependencies: tuple[OperationalDependencyArgs, ...]
    runbooks: tuple[OperationalRunbookArgs, ...]
    incidents: tuple[OperationalIncidentArgs, ...]
    controls: OperationalControlsArgs
    policies: OperationalReadinessPoliciesArgs

    def __init__(
        self,
        service: OperationalServiceArgs | Mapping[str, Any],
        contracts: Sequence[OperationalContractArgs | Mapping[str, Any]] = (),
        indicators: Sequence[OperationalIndicatorArgs | Mapping[str, Any]] = (),
        dependencies: Sequence[OperationalDependencyArgs | Mapping[str, Any]] = (),
        runbooks: Sequence[OperationalRunbookArgs | Mapping[str, Any]] = (),
        incidents: Sequence[OperationalIncidentArgs | Mapping[str, Any]] = (),
        controls: OperationalControlsArgs | Mapping[str, Any] | None = None,
        policies: OperationalReadinessPoliciesArgs | Mapping[str, Any] | None = None,
        schema: str = OPERATIONAL_READINESS_MANIFEST_SCHEMA,
    ) -> None:
        normalized_schema = _text("operational readiness schema", schema)
        normalized_service = service if isinstance(service, OperationalServiceArgs) else OperationalServiceArgs.from_wire(service)
        contract_values = _bounded("operational contracts", contracts, OPERATIONAL_READINESS_MAX_CONTRACTS)
        indicator_values = _bounded("operational indicators", indicators, OPERATIONAL_READINESS_MAX_INDICATORS)
        dependency_values = _bounded("operational dependencies", dependencies, OPERATIONAL_READINESS_MAX_DEPENDENCIES)
        runbook_values = _bounded("operational runbooks", runbooks, OPERATIONAL_READINESS_MAX_RUNBOOKS)
        incident_values = _bounded("operational incidents", incidents, OPERATIONAL_READINESS_MAX_INCIDENTS)
        normalized_controls = controls if isinstance(controls, OperationalControlsArgs) else OperationalControlsArgs.from_wire(controls)
        normalized_policies = policies if isinstance(policies, OperationalReadinessPoliciesArgs) else OperationalReadinessPoliciesArgs.from_wire(policies)
        normalized_contracts = tuple(item if isinstance(item, OperationalContractArgs) else OperationalContractArgs.from_wire(item) for item in contract_values)
        normalized_indicators = tuple(item if isinstance(item, OperationalIndicatorArgs) else OperationalIndicatorArgs.from_wire(item) for item in indicator_values)
        normalized_dependencies = tuple(item if isinstance(item, OperationalDependencyArgs) else OperationalDependencyArgs.from_wire(item) for item in dependency_values)
        normalized_runbooks = tuple(item if isinstance(item, OperationalRunbookArgs) else OperationalRunbookArgs.from_wire(item) for item in runbook_values)
        normalized_incidents = tuple(item if isinstance(item, OperationalIncidentArgs) else OperationalIncidentArgs.from_wire(item) for item in incident_values)
        wire = {"schema": normalized_schema, "service": normalized_service.to_wire(), "contracts": [item.to_wire() for item in normalized_contracts], "indicators": [item.to_wire() for item in normalized_indicators], "dependencies": [item.to_wire() for item in normalized_dependencies], "runbooks": [item.to_wire() for item in normalized_runbooks], "incidents": [item.to_wire() for item in normalized_incidents], "controls": normalized_controls.to_wire(), "policies": normalized_policies.to_wire()}
        _json_size("operational readiness manifest", wire)
        for name, value in (("schema", normalized_schema), ("service", normalized_service), ("contracts", normalized_contracts), ("indicators", normalized_indicators), ("dependencies", normalized_dependencies), ("runbooks", normalized_runbooks), ("incidents", normalized_incidents), ("controls", normalized_controls), ("policies", normalized_policies)):
            object.__setattr__(self, name, value)

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "OperationalReadinessManifestArgs":
        raw = _mapping("operational readiness manifest", value)
        return cls(raw.get("service"), _bounded("operational contracts", raw.get("contracts", []), OPERATIONAL_READINESS_MAX_CONTRACTS), _bounded("operational indicators", raw.get("indicators", []), OPERATIONAL_READINESS_MAX_INDICATORS), _bounded("operational dependencies", raw.get("dependencies", []), OPERATIONAL_READINESS_MAX_DEPENDENCIES), _bounded("operational runbooks", raw.get("runbooks", []), OPERATIONAL_READINESS_MAX_RUNBOOKS), _bounded("operational incidents", raw.get("incidents", []), OPERATIONAL_READINESS_MAX_INCIDENTS), raw.get("controls"), raw.get("policies"), raw.get("schema", OPERATIONAL_READINESS_MANIFEST_SCHEMA))

    def to_wire(self) -> dict[str, Any]:
        return {"schema": self.schema, "service": self.service.to_wire(), "contracts": [item.to_wire() for item in self.contracts], "indicators": [item.to_wire() for item in self.indicators], "dependencies": [item.to_wire() for item in self.dependencies], "runbooks": [item.to_wire() for item in self.runbooks], "incidents": [item.to_wire() for item in self.incidents], "controls": self.controls.to_wire(), "policies": self.policies.to_wire()}

    def to_mcp_arguments(self) -> dict[str, Any]:
        return {"manifest": self.to_wire()}


def _payload(value: Mapping[str, Any]) -> dict[str, Any]:
    raw = _mapping("operational readiness response", value)
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
                    if not isinstance(block, Mapping) or not isinstance(block.get("text"), str):
                        continue
                    try:
                        decoded = json.loads(block["text"])
                    except json.JSONDecodeError as error:
                        raise ArgumentError(f"operational readiness response text is not JSON: {error}") from error
                    if isinstance(decoded, Mapping):
                        candidates.append(decoded)
        add(container.get("structuredContent"))

    add(raw.get("mcp"))
    add(raw.get("result"))
    add(raw.get("structuredContent"))
    for candidate in candidates:
        if candidate.get("schema") == OPERATIONAL_READINESS_AUDIT_SCHEMA and "ok" in candidate:
            return dict(candidate)
    raise ArgumentError("response does not contain an operational-readiness audit projection")


@dataclass(frozen=True)
class OperationalReadinessIssueReport:
    code: str
    severity: OperationalIssueSeverity
    subject: str
    detail: str
    remediation: str

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "OperationalReadinessIssueReport":
        raw = _mapping("operational issue", value)
        return cls(_text("operational issue code", raw.get("code")), _enum("operational issue severity", raw.get("severity"), frozenset({"warning", "blocking"})), _text("operational issue subject", raw.get("subject")), _text("operational issue detail", raw.get("detail")), _text("operational issue remediation", raw.get("remediation")))  # type: ignore[arg-type]


@dataclass(frozen=True)
class OperationalIndicatorAuditReport:
    indicator_id: str
    contract_valid: bool
    source_valid: bool
    observed: bool
    evidence_valid: bool
    ready: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "OperationalIndicatorAuditReport":
        raw = _mapping("operational indicator audit", value)
        return cls(_text("indicator audit indicator_id", raw.get("indicator_id")), _bool("indicator audit contract_valid", raw.get("contract_valid")), _bool("indicator audit source_valid", raw.get("source_valid")), _bool("indicator audit observed", raw.get("observed")), _bool("indicator audit evidence_valid", raw.get("evidence_valid")), _bool("indicator audit ready", raw.get("ready")))  # type: ignore[arg-type]


@dataclass(frozen=True)
class OperationalDependencyAuditReport:
    dependency_id: str
    owner_valid: bool
    failure_mode_valid: bool
    fallback_present: bool
    critical: bool
    ready: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "OperationalDependencyAuditReport":
        raw = _mapping("operational dependency audit", value)
        return cls(_text("dependency audit dependency_id", raw.get("dependency_id")), _bool("dependency audit owner_valid", raw.get("owner_valid")), _bool("dependency audit failure_mode_valid", raw.get("failure_mode_valid")), _bool("dependency audit fallback_present", raw.get("fallback_present")), _bool("dependency audit critical", raw.get("critical")), _bool("dependency audit ready", raw.get("ready")))  # type: ignore[arg-type]


@dataclass(frozen=True)
class OperationalRunbookAuditReport:
    runbook_id: str
    valid: bool
    review_current: bool
    step_count: int
    referenced_incidents: int

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "OperationalRunbookAuditReport":
        raw = _mapping("operational runbook audit", value)
        return cls(_text("runbook audit runbook_id", raw.get("runbook_id")), _bool("runbook audit valid", raw.get("valid")), _bool("runbook audit review_current", raw.get("review_current")), int(raw.get("step_count")), int(raw.get("referenced_incidents")))  # type: ignore[arg-type]


@dataclass(frozen=True)
class OperationalIncidentAuditReport:
    incident_id: str
    valid: bool
    runbook_valid: bool
    timeline_present: bool
    postmortem_present: bool
    closed: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "OperationalIncidentAuditReport":
        raw = _mapping("operational incident audit", value)
        return cls(_text("incident audit incident_id", raw.get("incident_id")), _bool("incident audit valid", raw.get("valid")), _bool("incident audit runbook_valid", raw.get("runbook_valid")), _bool("incident audit timeline_present", raw.get("timeline_present")), _bool("incident audit postmortem_present", raw.get("postmortem_present")), _bool("incident audit closed", raw.get("closed")))  # type: ignore[arg-type]


@dataclass(frozen=True)
class OperationalControlAuditReport:
    control: str
    enabled: bool
    required: bool
    ready: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "OperationalControlAuditReport":
        raw = _mapping("operational control audit", value)
        return cls(_text("control audit control", raw.get("control")), _bool("control audit enabled", raw.get("enabled")), _bool("control audit required", raw.get("required")), _bool("control audit ready", raw.get("ready")))  # type: ignore[arg-type]


@dataclass(frozen=True)
class OperationalReadinessAuditReport:
    raw: dict[str, Any]
    ok: bool
    schema: str | None
    workflow: str | None
    manifest_digest: str | None
    valid: bool | None
    operationally_ready_value: bool | None
    counts: Mapping[str, Any] | None
    indicator_audits: tuple[OperationalIndicatorAuditReport, ...]
    dependency_audits: tuple[OperationalDependencyAuditReport, ...]
    runbook_audits: tuple[OperationalRunbookAuditReport, ...]
    incident_audits: tuple[OperationalIncidentAuditReport, ...]
    control_audits: tuple[OperationalControlAuditReport, ...]
    issues: tuple[OperationalReadinessIssueReport, ...]
    guarantees: tuple[str, ...]
    limitations: tuple[str, ...]
    refusal: str | None
    fail_closed: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "OperationalReadinessAuditReport":
        raw = _payload(value)
        ok = raw.get("ok") is True
        if not ok:
            if raw.get("fail_closed") is not True:
                raise ArgumentError("operational readiness refusals must be fail-closed")
            return cls(raw, False, raw.get("schema"), raw.get("workflow"), raw.get("manifest_digest"), False, False, None, (), (), (), (), (), (), _route_strings("operational refusal guarantees", raw.get("guarantees", [])), _route_strings("operational refusal limitations", raw.get("limitations", [])), raw.get("refusal") or raw.get("error"), True)
        if raw.get("schema") != OPERATIONAL_READINESS_AUDIT_SCHEMA:
            raise ArgumentError("operational readiness projection has an invalid schema")
        audit = _mapping("operational readiness audit", raw.get("audit"))
        issues = tuple(OperationalReadinessIssueReport.from_wire(item) for item in _bounded("operational audit issues", audit.get("issues", []), OPERATIONAL_READINESS_MAX_LIST_ITEMS))
        indicators = tuple(OperationalIndicatorAuditReport.from_wire(item) for item in _bounded("operational indicator audits", audit.get("indicator_audits", []), OPERATIONAL_READINESS_MAX_INDICATORS))
        dependencies = tuple(OperationalDependencyAuditReport.from_wire(item) for item in _bounded("operational dependency audits", audit.get("dependency_audits", []), OPERATIONAL_READINESS_MAX_DEPENDENCIES))
        runbooks = tuple(OperationalRunbookAuditReport.from_wire(item) for item in _bounded("operational runbook audits", audit.get("runbook_audits", []), OPERATIONAL_READINESS_MAX_RUNBOOKS))
        incidents = tuple(OperationalIncidentAuditReport.from_wire(item) for item in _bounded("operational incident audits", audit.get("incident_audits", []), OPERATIONAL_READINESS_MAX_INCIDENTS))
        controls = tuple(OperationalControlAuditReport.from_wire(item) for item in _bounded("operational control audits", audit.get("control_audits", []), OPERATIONAL_READINESS_MAX_LIST_ITEMS))
        return cls(raw, True, OPERATIONAL_READINESS_AUDIT_SCHEMA, _text("operational workflow", raw.get("workflow")), _text("operational manifest_digest", raw.get("manifest_digest"), required=False), _bool("operational audit valid", audit.get("valid")), _bool("operationally_ready", raw.get("operationally_ready")), _mapping("operational audit counts", audit.get("counts")), indicators, dependencies, runbooks, incidents, controls, issues, _route_strings("operational guarantees", raw.get("guarantees", audit.get("guarantees", []))), _route_strings("operational limitations", raw.get("limitations", audit.get("limitations", []))), None, False)

    @property
    def accepted(self) -> bool:
        return self.ok and self.valid is True and self.operationally_ready_value is True

    @property
    def operationally_ready(self) -> bool:
        return self.operationally_ready_value is True

    @property
    def refused(self) -> bool:
        return not self.ok

    @property
    def blocking_issues(self) -> tuple[OperationalReadinessIssueReport, ...]:
        return tuple(issue for issue in self.issues if issue.severity == "blocking")

    @property
    def warning_issues(self) -> tuple[OperationalReadinessIssueReport, ...]:
        return tuple(issue for issue in self.issues if issue.severity == "warning")

    @property
    def has_blockers(self) -> bool:
        return bool(self.blocking_issues)

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


def operational_readiness_audit_report(value: Mapping[str, Any]) -> OperationalReadinessAuditReport:
    """Parse direct MCP output or an HTTP REST tool envelope."""

    return OperationalReadinessAuditReport.from_wire(value)


__all__ = [
    "OPERATIONAL_READINESS_MANIFEST_SCHEMA",
    "OPERATIONAL_READINESS_AUDIT_SCHEMA",
    "OPERATIONAL_READINESS_MAX_INPUT_BYTES",
    "OPERATIONAL_READINESS_MAX_CONTRACTS",
    "OPERATIONAL_READINESS_MAX_INDICATORS",
    "OPERATIONAL_READINESS_MAX_DEPENDENCIES",
    "OPERATIONAL_READINESS_MAX_RUNBOOKS",
    "OPERATIONAL_READINESS_MAX_INCIDENTS",
    "OperationalServiceArgs",
    "OperationalContractArgs",
    "OperationalIndicatorArgs",
    "OperationalDependencyArgs",
    "OperationalRunbookArgs",
    "OperationalIncidentArgs",
    "OperationalControlsArgs",
    "OperationalReadinessPoliciesArgs",
    "OperationalReadinessManifestArgs",
    "OperationalReadinessIssueReport",
    "OperationalIndicatorAuditReport",
    "OperationalDependencyAuditReport",
    "OperationalRunbookAuditReport",
    "OperationalIncidentAuditReport",
    "OperationalControlAuditReport",
    "OperationalReadinessAuditReport",
    "operational_readiness_audit_report",
]
