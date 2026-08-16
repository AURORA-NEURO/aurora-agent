"""Typed sandbox runtime-simulation requests and decision-trace reports."""

from __future__ import annotations

from dataclasses import dataclass
import json
from typing import Any, Literal, Mapping, Sequence

from .errors import ArgumentError
from .sandbox_admission import (
    SandboxCapabilityKind,
    SandboxIssueReport,
    SandboxManifestArgs,
    _bool,
    _bounded,
    _enum,
    _integer,
    _mapping,
    _text,
)

SANDBOX_RUNTIME_MANIFEST_SCHEMA = "bioprism-sandbox-runtime/0.1"
SANDBOX_RUNTIME_AUDIT_SCHEMA = "bioprism-sandbox-runtime-audit/0.1"
SANDBOX_RUNTIME_MAX_REQUESTS = 4_096

SandboxRuntimeDecision = Literal["simulated", "refused", "not_run"]
SandboxIssueSeverity = Literal["warning", "blocking"]

_CAPABILITY_KINDS = frozenset(
    {
        "filesystem_read",
        "filesystem_write",
        "network_egress",
        "network_ingress",
        "secret_access",
        "process_spawn",
        "device_access",
        "kernel_access",
        "clock",
        "randomness",
        "artifact_publish",
    }
)


def _target(name: str, value: Any, kind: str) -> str:
    result = _text(name, value)
    assert result is not None
    if result == "*" or ".." in result or "\\" in result:
        raise ArgumentError(f"{name} must be one exact bounded target")
    if kind in {"filesystem_read", "filesystem_write"} and (
        not result.startswith("/")
        or result == "/"
        or result.startswith(("/proc", "/sys", "/dev"))
    ):
        raise ArgumentError(f"{name} must be a private normalized sandbox path")
    if kind in {"network_egress", "network_ingress"} and (
        result in {"0.0.0.0/0", "::/0"} or "*" in result
    ):
        raise ArgumentError(f"{name} must be a bounded network destination")
    return result


def _count(name: str, value: Any) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or value < 0:
        raise ArgumentError(f"{name} must be a non-negative integer")
    return value


def _runtime_payload(value: Mapping[str, Any]) -> dict[str, Any]:
    raw = _mapping("sandbox runtime response", value)
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
        if candidate.get("schema") == SANDBOX_RUNTIME_AUDIT_SCHEMA and "ok" in candidate:
            return dict(candidate)
    raise ArgumentError("response does not contain a sandbox runtime projection")


@dataclass(frozen=True)
class SandboxRuntimeRequestArgs:
    id: str
    kind: SandboxCapabilityKind
    target: str
    cpu_millis: int
    memory_mb: int
    wall_time_seconds: int
    processes: int
    output_bytes: int

    def __post_init__(self) -> None:
        _text("runtime request.id", self.id)
        kind = _enum("runtime request.kind", self.kind, _CAPABILITY_KINDS)
        object.__setattr__(self, "kind", kind)
        object.__setattr__(self, "target", _target("runtime request.target", self.target, kind))
        for name in ("cpu_millis", "memory_mb", "wall_time_seconds", "processes", "output_bytes"):
            object.__setattr__(self, name, _integer(f"runtime request.{name}", getattr(self, name), required=True))

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "SandboxRuntimeRequestArgs":
        raw = _mapping("sandbox runtime request", value)
        return cls(
            _text("runtime request.id", raw.get("id")),
            _enum("runtime request.kind", raw.get("kind"), _CAPABILITY_KINDS),
            _text("runtime request.target", raw.get("target")),
            _integer("runtime request.cpu_millis", raw.get("cpu_millis"), required=True),
            _integer("runtime request.memory_mb", raw.get("memory_mb"), required=True),
            _integer("runtime request.wall_time_seconds", raw.get("wall_time_seconds"), required=True),
            _integer("runtime request.processes", raw.get("processes"), required=True),
            _integer("runtime request.output_bytes", raw.get("output_bytes"), required=True),
        )  # type: ignore[arg-type]

    def to_wire(self) -> dict[str, Any]:
        return {
            "id": self.id,
            "kind": self.kind,
            "target": self.target,
            "cpu_millis": self.cpu_millis,
            "memory_mb": self.memory_mb,
            "wall_time_seconds": self.wall_time_seconds,
            "processes": self.processes,
            "output_bytes": self.output_bytes,
        }


@dataclass(frozen=True)
class SandboxRuntimePoliciesArgs:
    stop_on_refusal: bool = True
    require_admission: bool = True
    max_requests: int = SANDBOX_RUNTIME_MAX_REQUESTS

    def __post_init__(self) -> None:
        object.__setattr__(self, "stop_on_refusal", _bool("runtime policies.stop_on_refusal", self.stop_on_refusal))
        object.__setattr__(self, "require_admission", _bool("runtime policies.require_admission", self.require_admission))
        maximum = _integer("runtime policies.max_requests", self.max_requests, required=True)
        if maximum > SANDBOX_RUNTIME_MAX_REQUESTS:
            raise ArgumentError(f"runtime policies.max_requests is bounded at {SANDBOX_RUNTIME_MAX_REQUESTS}")
        object.__setattr__(self, "max_requests", maximum)

    @classmethod
    def from_wire(cls, value: Mapping[str, Any] | None) -> "SandboxRuntimePoliciesArgs":
        raw = {} if value is None else _mapping("sandbox runtime policies", value)
        return cls(
            raw.get("stop_on_refusal", True),
            raw.get("require_admission", True),
            raw.get("max_requests", SANDBOX_RUNTIME_MAX_REQUESTS),
        )

    def to_wire(self) -> dict[str, Any]:
        return {
            "stop_on_refusal": self.stop_on_refusal,
            "require_admission": self.require_admission,
            "max_requests": self.max_requests,
        }


@dataclass(frozen=True)
class SandboxRuntimeManifestArgs:
    admission: SandboxManifestArgs
    profile: str
    requests: tuple[SandboxRuntimeRequestArgs, ...] = ()
    policies: SandboxRuntimePoliciesArgs = SandboxRuntimePoliciesArgs()
    schema: str = SANDBOX_RUNTIME_MANIFEST_SCHEMA

    def __post_init__(self) -> None:
        if not isinstance(self.admission, SandboxManifestArgs):
            object.__setattr__(self, "admission", SandboxManifestArgs.from_wire(self.admission))
        _text("runtime profile", self.profile)
        object.__setattr__(
            self,
            "requests",
            tuple(
                item if isinstance(item, SandboxRuntimeRequestArgs) else SandboxRuntimeRequestArgs.from_wire(item)
                for item in _bounded("sandbox runtime requests", self.requests, SANDBOX_RUNTIME_MAX_REQUESTS)
            ),
        )
        if not isinstance(self.policies, SandboxRuntimePoliciesArgs):
            object.__setattr__(self, "policies", SandboxRuntimePoliciesArgs.from_wire(self.policies))
        if self.schema != SANDBOX_RUNTIME_MANIFEST_SCHEMA:
            raise ArgumentError(f"runtime schema must be {SANDBOX_RUNTIME_MANIFEST_SCHEMA}")

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "SandboxRuntimeManifestArgs":
        raw = _mapping("sandbox runtime manifest", value)
        return cls(
            SandboxManifestArgs.from_wire(raw.get("admission")),
            _text("runtime profile", raw.get("profile")),
            tuple(SandboxRuntimeRequestArgs.from_wire(item) for item in _bounded("sandbox runtime requests", raw.get("requests", []), SANDBOX_RUNTIME_MAX_REQUESTS)),
            SandboxRuntimePoliciesArgs.from_wire(raw.get("policies")),
            raw.get("schema", SANDBOX_RUNTIME_MANIFEST_SCHEMA),
        )  # type: ignore[arg-type]

    def to_wire(self) -> dict[str, Any]:
        return {
            "schema": self.schema,
            "admission": self.admission.to_wire(),
            "profile": self.profile,
            "requests": [item.to_wire() for item in self.requests],
            "policies": self.policies.to_wire(),
        }

    def to_mcp_arguments(self) -> dict[str, Any]:
        return {"manifest": self.to_wire()}


@dataclass(frozen=True)
class SandboxRuntimeUsageReport:
    cpu_millis: int
    memory_mb_peak: int
    wall_time_seconds: int
    processes_peak: int
    output_bytes: int

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "SandboxRuntimeUsageReport":
        raw = _mapping("sandbox runtime usage", value)
        return cls(
            _count("runtime usage.cpu_millis", raw.get("cpu_millis")),
            _count("runtime usage.memory_mb_peak", raw.get("memory_mb_peak")),
            _count("runtime usage.wall_time_seconds", raw.get("wall_time_seconds")),
            _count("runtime usage.processes_peak", raw.get("processes_peak")),
            _count("runtime usage.output_bytes", raw.get("output_bytes")),
        )  # type: ignore[arg-type]


@dataclass(frozen=True)
class SandboxRuntimeStepReport:
    request_id: str
    kind: SandboxCapabilityKind
    target: str
    capability_id: str | None
    capability_valid: bool
    target_valid: bool
    resource_valid: bool
    decision: SandboxRuntimeDecision
    charged: bool
    usage_after: SandboxRuntimeUsageReport
    refusal: str | None

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "SandboxRuntimeStepReport":
        raw = _mapping("sandbox runtime step", value)
        capability_id = raw.get("capability_id")
        refusal = raw.get("refusal")
        return cls(
            _text("runtime step.request_id", raw.get("request_id")),
            _enum("runtime step.kind", raw.get("kind"), _CAPABILITY_KINDS),
            _text("runtime step.target", raw.get("target")),
            _text("runtime step.capability_id", capability_id, required=False),
            _bool("runtime step.capability_valid", raw.get("capability_valid")),
            _bool("runtime step.target_valid", raw.get("target_valid")),
            _bool("runtime step.resource_valid", raw.get("resource_valid")),
            _enum("runtime step.decision", raw.get("decision"), frozenset({"simulated", "refused", "not_run"})),
            _bool("runtime step.charged", raw.get("charged")),
            SandboxRuntimeUsageReport.from_wire(raw.get("usage_after")),
            _text("runtime step.refusal", refusal, required=False),
        )  # type: ignore[arg-type]


@dataclass(frozen=True)
class SandboxRuntimeAuditReport:
    raw: dict[str, Any]
    ok: bool
    schema: str | None
    workflow: str | None
    manifest_digest: str | None
    admission_digest: str | None
    trace_digest: str | None
    valid: bool | None
    sandbox_runtime_ready_value: bool | None
    profile_id: str | None
    admission_valid: bool | None
    simulation_started: bool | None
    completed: bool | None
    stopped_on_refusal: bool | None
    request_count: int | None
    simulated_count: int | None
    refused_count: int | None
    not_run_count: int | None
    usage: SandboxRuntimeUsageReport | None
    steps: tuple[SandboxRuntimeStepReport, ...]
    admission_issues: tuple[SandboxIssueReport, ...]
    issues: tuple[SandboxIssueReport, ...]
    guarantees: tuple[str, ...]
    limitations: tuple[str, ...]
    refusal: str | None
    fail_closed: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "SandboxRuntimeAuditReport":
        raw = _runtime_payload(value)
        if raw.get("ok") is not True:
            if raw.get("fail_closed") is not True:
                raise ArgumentError("sandbox runtime refusals must be fail-closed")
            return cls(raw, False, raw.get("schema"), raw.get("workflow"), raw.get("manifest_digest"), raw.get("admission_digest"), raw.get("trace_digest"), False, False, None, False, False, False, False, None, None, None, None, None, (), (), (), (), raw.get("refusal") or raw.get("error"), True)
        if raw.get("schema") != SANDBOX_RUNTIME_AUDIT_SCHEMA:
            raise ArgumentError("sandbox runtime projection has an invalid schema")
        audit = _mapping("sandbox runtime audit", raw.get("audit"))
        return cls(
            raw,
            True,
            SANDBOX_RUNTIME_AUDIT_SCHEMA,
            _text("runtime workflow", raw.get("workflow")),
            _text("runtime manifest_digest", raw.get("manifest_digest"), required=False),
            _text("runtime admission_digest", raw.get("admission_digest"), required=False),
            _text("runtime trace_digest", raw.get("trace_digest"), required=False),
            _bool("runtime valid", audit.get("valid")),
            _bool("sandbox_runtime_ready", raw.get("sandbox_runtime_ready")),
            _text("runtime profile_id", audit.get("profile_id")),
            _bool("runtime admission_valid", audit.get("admission_valid")),
            _bool("runtime simulation_started", audit.get("simulation_started")),
            _bool("runtime completed", audit.get("completed")),
            _bool("runtime stopped_on_refusal", audit.get("stopped_on_refusal")),
            _count("runtime request_count", audit.get("request_count")),
            _count("runtime simulated_count", audit.get("simulated_count")),
            _count("runtime refused_count", audit.get("refused_count")),
            _count("runtime not_run_count", audit.get("not_run_count")),
            SandboxRuntimeUsageReport.from_wire(audit.get("usage")),
            tuple(SandboxRuntimeStepReport.from_wire(item) for item in _bounded("runtime steps", audit.get("steps", []), SANDBOX_RUNTIME_MAX_REQUESTS)),
            tuple(SandboxIssueReport.from_wire(item) for item in _bounded("runtime admission issues", audit.get("admission_issues", []), SANDBOX_RUNTIME_MAX_REQUESTS)),
            tuple(SandboxIssueReport.from_wire(item) for item in _bounded("runtime issues", audit.get("issues", []), SANDBOX_RUNTIME_MAX_REQUESTS)),
            tuple(str(item) for item in audit.get("guarantees", raw.get("guarantees", []))),
            tuple(str(item) for item in audit.get("limitations", raw.get("limitations", []))),
            None,
            False,
        )

    @property
    def accepted(self) -> bool:
        return self.ok and self.valid is True and self.sandbox_runtime_ready_value is True

    @property
    def sandbox_runtime_ready(self) -> bool:
        return self.sandbox_runtime_ready_value is True

    @property
    def blocking_issues(self) -> tuple[SandboxIssueReport, ...]:
        return tuple(issue for issue in self.issues if issue.severity == "blocking")

    @property
    def warning_issues(self) -> tuple[SandboxIssueReport, ...]:
        return tuple(issue for issue in self.issues if issue.severity == "warning")

    @property
    def has_blockers(self) -> bool:
        return bool(self.blocking_issues)

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


def sandbox_runtime_simulate_report(value: Mapping[str, Any]) -> SandboxRuntimeAuditReport:
    """Parse direct MCP output or an HTTP REST tool envelope."""

    return SandboxRuntimeAuditReport.from_wire(value)


__all__ = [
    "SANDBOX_RUNTIME_MANIFEST_SCHEMA",
    "SANDBOX_RUNTIME_AUDIT_SCHEMA",
    "SANDBOX_RUNTIME_MAX_REQUESTS",
    "SandboxRuntimeRequestArgs",
    "SandboxRuntimePoliciesArgs",
    "SandboxRuntimeManifestArgs",
    "SandboxRuntimeUsageReport",
    "SandboxRuntimeStepReport",
    "SandboxRuntimeAuditReport",
    "sandbox_runtime_simulate_report",
]
