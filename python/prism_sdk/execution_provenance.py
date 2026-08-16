"""Typed mission/delegated-check provenance requests and structural reports."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Mapping, Sequence

from .capability import _route_count, _route_mapping, _route_strings, _route_text, _tool_payload
from .errors import ArgumentError

EXECUTION_PROVENANCE_SCHEMA = "bioprism-devplat-execution-provenance/0.1"
MAX_DELEGATED_CHECKS = 64


def _mapping(name: str, value: Any) -> dict[str, Any]:
    if not isinstance(value, Mapping) or not value:
        raise ArgumentError(f"{name} must be a non-empty mapping")
    return dict(value)


def _bool(name: str, value: Any) -> bool:
    if not isinstance(value, bool):
        raise ArgumentError(f"{name} must be a boolean")
    return value


def _digest(name: str, value: Any) -> str:
    text = _route_text(name, value)
    if len(text) != 64 or any(character not in "0123456789abcdef" for character in text.lower()):
        raise ArgumentError(f"{name} must be a 64-character hexadecimal digest")
    return text


@dataclass(frozen=True)
class DelegatedCheckEvidenceArgs:
    name: str
    kind: str
    required: bool
    status: str
    result_digest: str
    source: str
    trace_sequence: int | None = None

    def __post_init__(self) -> None:
        for name, value in (("name", self.name), ("kind", self.kind), ("status", self.status), ("source", self.source)):
            if not isinstance(value, str) or not value.strip():
                raise ArgumentError(f"delegated check {name} must be a non-empty string")
        if not isinstance(self.required, bool):
            raise ArgumentError("delegated check required must be a boolean")
        _digest("delegated check result_digest", self.result_digest)
        if self.trace_sequence is not None and (
            not isinstance(self.trace_sequence, int) or isinstance(self.trace_sequence, bool) or self.trace_sequence < 0
        ):
            raise ArgumentError("delegated check trace_sequence must be a non-negative integer")

    def to_mcp_arguments(self) -> dict[str, Any]:
        value: dict[str, Any] = {
            "name": self.name,
            "kind": self.kind,
            "required": self.required,
            "status": self.status,
            "result_digest": self.result_digest,
            "source": self.source,
        }
        if self.trace_sequence is not None:
            value["trace_sequence"] = self.trace_sequence
        return value


def _check(value: DelegatedCheckEvidenceArgs | Mapping[str, Any]) -> dict[str, Any]:
    if isinstance(value, DelegatedCheckEvidenceArgs):
        return value.to_mcp_arguments()
    raw = _mapping("delegated check", value)
    return DelegatedCheckEvidenceArgs(
        name=raw.get("name"),
        kind=raw.get("kind"),
        required=raw.get("required"),
        status=raw.get("status"),
        result_digest=raw.get("result_digest"),
        source=raw.get("source"),
        trace_sequence=raw.get("trace_sequence"),
    ).to_mcp_arguments()


@dataclass(frozen=True)
class ExecutionProvenanceRequest:
    mission: Mapping[str, Any]
    delegated_checks: Sequence[DelegatedCheckEvidenceArgs | Mapping[str, Any]] = ()

    def __post_init__(self) -> None:
        _mapping("mission", self.mission)
        if not isinstance(self.delegated_checks, Sequence) or isinstance(self.delegated_checks, (str, bytes)):
            raise ArgumentError("delegated_checks must be an array")
        if len(self.delegated_checks) > MAX_DELEGATED_CHECKS:
            raise ArgumentError(f"delegated_checks may contain at most {MAX_DELEGATED_CHECKS} rows")
        for value in self.delegated_checks:
            _check(value)

    def to_mcp_arguments(self) -> dict[str, Any]:
        return {
            "mission": dict(self.mission),
            "delegated_checks": [_check(value) for value in self.delegated_checks],
        }


@dataclass(frozen=True)
class ExecutionProvenanceFindingReport:
    raw: dict[str, Any]
    code: str
    severity: str
    subject: str
    detail: str

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "ExecutionProvenanceFindingReport":
        raw = _route_mapping("execution provenance finding", value)
        return cls(
            raw=raw,
            code=_route_text("execution provenance finding code", raw.get("code")),
            severity=_route_text("execution provenance finding severity", raw.get("severity")),
            subject=_route_text("execution provenance finding subject", raw.get("subject")),
            detail=_route_text("execution provenance finding detail", raw.get("detail")),
        )


@dataclass(frozen=True)
class ExecutionProvenanceReport:
    raw: dict[str, Any]
    schema: str
    workflow: str
    mission_id: str
    plan_digest: str
    trace_digest: str
    provenance_digest: str
    valid: bool
    provenance_ready: bool
    complete: bool
    structurally_valid: bool
    release_candidate: bool
    planned_step_count: int
    result_count: int
    trace_event_count: int
    delegated_check_count: int
    required_failure_count: int
    required_check_count: int
    passed_check_count: int
    nonpassing_required_checks: tuple[str, ...]
    missing_step_results: tuple[str, ...]
    unknown_step_results: tuple[str, ...]
    findings: tuple[ExecutionProvenanceFindingReport, ...]
    execution: str
    verification: str

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "ExecutionProvenanceReport":
        raw = _tool_payload(value, "execution_provenance_audit")
        if raw.get("ok") is not True:
            raise ArgumentError("execution provenance report is not successful")
        findings = raw.get("findings", [])
        if not isinstance(findings, list):
            raise ArgumentError("execution provenance findings must be an array")
        return cls(
            raw=raw,
            schema=_route_text("execution provenance schema", raw.get("schema")),
            workflow=_route_text("execution provenance workflow", raw.get("workflow")),
            mission_id=_route_text("execution provenance mission_id", raw.get("mission_id")),
            plan_digest=_digest("execution provenance plan_digest", raw.get("plan_digest")),
            trace_digest=_digest("execution provenance trace_digest", raw.get("trace_digest")),
            provenance_digest=_digest("execution provenance provenance_digest", raw.get("provenance_digest")),
            valid=_bool("execution provenance valid", raw.get("valid")),
            provenance_ready=_bool("execution provenance provenance_ready", raw.get("provenance_ready")),
            complete=_bool("execution provenance complete", raw.get("complete")),
            structurally_valid=_bool("execution provenance structurally_valid", raw.get("structurally_valid")),
            release_candidate=_bool("execution provenance release_candidate", raw.get("release_candidate")),
            planned_step_count=_route_count("execution provenance planned_step_count", raw.get("planned_step_count")),
            result_count=_route_count("execution provenance result_count", raw.get("result_count")),
            trace_event_count=_route_count("execution provenance trace_event_count", raw.get("trace_event_count")),
            delegated_check_count=_route_count("execution provenance delegated_check_count", raw.get("delegated_check_count")),
            required_failure_count=_route_count("execution provenance required_failure_count", raw.get("required_failure_count")),
            required_check_count=_route_count("execution provenance required_check_count", raw.get("required_check_count")),
            passed_check_count=_route_count("execution provenance passed_check_count", raw.get("passed_check_count")),
            nonpassing_required_checks=_route_strings("execution provenance nonpassing_required_checks", raw.get("nonpassing_required_checks", [])),
            missing_step_results=_route_strings("execution provenance missing_step_results", raw.get("missing_step_results", [])),
            unknown_step_results=_route_strings("execution provenance unknown_step_results", raw.get("unknown_step_results", [])),
            findings=tuple(ExecutionProvenanceFindingReport.from_wire(item) for item in findings),
            execution=_route_text("execution provenance execution", raw.get("execution")),
            verification=_route_text("execution provenance verification", raw.get("verification")),
        )

    @property
    def blocking_findings(self) -> tuple[ExecutionProvenanceFindingReport, ...]:
        return tuple(item for item in self.findings if item.severity == "blocking")

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


def execution_provenance_report(value: Mapping[str, Any]) -> ExecutionProvenanceReport:
    """Parse a direct MCP result or HTTP REST tool envelope."""

    return ExecutionProvenanceReport.from_wire(value)


__all__ = [
    "EXECUTION_PROVENANCE_SCHEMA",
    "MAX_DELEGATED_CHECKS",
    "DelegatedCheckEvidenceArgs",
    "ExecutionProvenanceRequest",
    "ExecutionProvenanceFindingReport",
    "ExecutionProvenanceReport",
    "execution_provenance_report",
]
