"""Typed admission models for untrusted code and research artifacts."""

from __future__ import annotations

from dataclasses import dataclass
import json
from typing import Any, Literal, Mapping, Sequence

from .capability import _route_mapping, _route_strings, _route_text
from .errors import ArgumentError


SANDBOX_MANIFEST_SCHEMA = "bioprism-sandbox/0.1"
SANDBOX_AUDIT_SCHEMA = "bioprism-sandbox-audit/0.1"
SANDBOX_MAX_INPUT_BYTES = 20_000_000
SANDBOX_MAX_ARTIFACTS = 4_096
SANDBOX_MAX_PROFILES = 4_096
SANDBOX_MAX_CAPABILITIES = 16_384
SANDBOX_MAX_MOUNTS = 16_384
SANDBOX_MAX_OUTPUTS = 8_192
SANDBOX_MAX_LIST_ITEMS = 32_768

SandboxArtifactKind = Literal["source_code", "notebook", "dataset", "model", "container", "package", "plugin", "generated_output"]
SandboxTrust = Literal["untrusted", "internal", "reviewed", "trusted"]
SandboxNetworkMode = Literal["deny", "allowlist", "unrestricted"]
SandboxMountMode = Literal["read_only", "read_write"]
SandboxCapabilityKind = Literal["filesystem_read", "filesystem_write", "network_egress", "network_ingress", "secret_access", "process_spawn", "device_access", "kernel_access", "clock", "randomness", "artifact_publish"]
SandboxDecision = Literal["allow", "deny"]
SandboxIssueSeverity = Literal["warning", "blocking"]


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
    if len(result.encode("utf-8")) > 4_096:
        raise ArgumentError(f"{name} exceeds 4096 UTF-8 bytes")
    return result


def _bool(name: str, value: Any) -> bool:
    if not isinstance(value, bool):
        raise ArgumentError(f"{name} must be a boolean")
    return value


def _integer(name: str, value: Any, *, required: bool = False) -> int | None:
    if value is None and not required:
        return None
    if isinstance(value, bool) or not isinstance(value, int) or value <= 0:
        raise ArgumentError(f"{name} must be a positive integer")
    return value


def _enum(name: str, value: Any, allowed: frozenset[str]) -> str:
    result = _text(name, value)
    assert result is not None
    if result not in allowed:
        raise ArgumentError(f"{name} must be one of {sorted(allowed)}")
    return result


def _digest(name: str, value: Any, *, required: bool = True) -> str | None:
    if value is None and not required:
        return None
    result = _text(name, value)
    assert result is not None
    if len(result) != 64 or any(char not in "0123456789abcdefABCDEF" for char in result):
        raise ArgumentError(f"{name} must be 64 hexadecimal characters")
    return result


def _json_size(name: str, value: Any) -> None:
    try:
        encoded = json.dumps(value, separators=(",", ":"), ensure_ascii=False).encode("utf-8")
    except (TypeError, ValueError) as error:
        raise ArgumentError(f"{name} must be JSON serializable: {error}") from error
    if len(encoded) > SANDBOX_MAX_INPUT_BYTES:
        raise ArgumentError(f"{name} exceeds the {SANDBOX_MAX_INPUT_BYTES}-byte safety bound")


def _strings(name: str, value: Any) -> tuple[str, ...]:
    return tuple(_text(f"{name}[{index}]", item) for index, item in enumerate(_bounded(name, value, SANDBOX_MAX_LIST_ITEMS)))  # type: ignore[misc]


def _path(name: str, value: Any) -> str:
    result = _text(name, value)
    assert result is not None
    if not result.startswith("/") or result == "/" or ".." in result or "\\" in result or result.startswith(("/proc", "/sys", "/dev")):
        raise ArgumentError(f"{name} must be a private normalized sandbox path")
    return result


def _network_target(name: str, value: Any) -> str:
    result = _text(name, value)
    assert result is not None
    if result == "*" or result in {"0.0.0.0/0", "::/0"} or "*" in result or ".." in result:
        raise ArgumentError(f"{name} must be a bounded network destination")
    return result


@dataclass(frozen=True)
class SandboxSystemArgs:
    id: str
    version: str
    owner: str

    def __post_init__(self) -> None:
        for name in ("id", "version", "owner"):
            _text(f"system.{name}", getattr(self, name))

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "SandboxSystemArgs":
        raw = _mapping("sandbox system", value)
        return cls(_text("system.id", raw.get("id")), _text("system.version", raw.get("version")), _text("system.owner", raw.get("owner")))  # type: ignore[arg-type]

    def to_wire(self) -> dict[str, Any]:
        return {"id": self.id, "version": self.version, "owner": self.owner}


@dataclass(frozen=True)
class SandboxArtifactArgs:
    id: str
    kind: SandboxArtifactKind
    digest: str
    source: str
    producer: str
    trust: SandboxTrust
    inputs: tuple[str, ...] = ()

    def __post_init__(self) -> None:
        for name in ("id", "source", "producer"):
            _text(f"artifact.{name}", getattr(self, name))
        object.__setattr__(self, "kind", _enum("artifact.kind", self.kind, frozenset({"source_code", "notebook", "dataset", "model", "container", "package", "plugin", "generated_output"})))
        object.__setattr__(self, "digest", _digest("artifact.digest", self.digest))
        object.__setattr__(self, "trust", _enum("artifact.trust", self.trust, frozenset({"untrusted", "internal", "reviewed", "trusted"})))
        object.__setattr__(self, "inputs", _strings("artifact.inputs", self.inputs))

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "SandboxArtifactArgs":
        raw = _mapping("sandbox artifact", value)
        return cls(_text("artifact.id", raw.get("id")), _enum("artifact.kind", raw.get("kind"), frozenset({"source_code", "notebook", "dataset", "model", "container", "package", "plugin", "generated_output"})), _digest("artifact.digest", raw.get("digest")), _text("artifact.source", raw.get("source")), _text("artifact.producer", raw.get("producer")), _enum("artifact.trust", raw.get("trust"), frozenset({"untrusted", "internal", "reviewed", "trusted"})), _strings("artifact.inputs", raw.get("inputs", [])))  # type: ignore[arg-type]

    def to_wire(self) -> dict[str, Any]:
        return {"id": self.id, "kind": self.kind, "digest": self.digest, "source": self.source, "producer": self.producer, "trust": self.trust, "inputs": list(self.inputs)}


@dataclass(frozen=True)
class SandboxMountArgs:
    id: str
    source_artifact: str
    target: str
    mode: SandboxMountMode

    def __post_init__(self) -> None:
        _text("mount.id", self.id)
        _text("mount.source_artifact", self.source_artifact)
        object.__setattr__(self, "target", _path("mount.target", self.target))
        object.__setattr__(self, "mode", _enum("mount.mode", self.mode, frozenset({"read_only", "read_write"})))

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "SandboxMountArgs":
        raw = _mapping("sandbox mount", value)
        return cls(_text("mount.id", raw.get("id")), _text("mount.source_artifact", raw.get("source_artifact")), _path("mount.target", raw.get("target")), _enum("mount.mode", raw.get("mode"), frozenset({"read_only", "read_write"})))  # type: ignore[arg-type]

    def to_wire(self) -> dict[str, Any]:
        return {"id": self.id, "source_artifact": self.source_artifact, "target": self.target, "mode": self.mode}


@dataclass(frozen=True)
class SandboxResourceLimitsArgs:
    cpu_millis: int | None = None
    memory_mb: int | None = None
    wall_time_seconds: int | None = None
    processes: int | None = None
    output_bytes: int | None = None

    def __post_init__(self) -> None:
        for name in ("cpu_millis", "memory_mb", "wall_time_seconds", "processes", "output_bytes"):
            object.__setattr__(self, name, _integer(f"resources.{name}", getattr(self, name)))

    @classmethod
    def from_wire(cls, value: Mapping[str, Any] | None) -> "SandboxResourceLimitsArgs":
        raw = {} if value is None else _mapping("sandbox resources", value)
        return cls(*(raw.get(name) for name in ("cpu_millis", "memory_mb", "wall_time_seconds", "processes", "output_bytes")))

    def to_wire(self) -> dict[str, Any]:
        return {name: getattr(self, name) for name in ("cpu_millis", "memory_mb", "wall_time_seconds", "processes", "output_bytes") if getattr(self, name) is not None}


@dataclass(frozen=True)
class SandboxExecutionProfileArgs:
    id: str
    artifact: str
    runtime: str
    image_digest: str | None
    environment_digest: str | None
    user: str
    rootless: bool
    read_only_root: bool
    no_privilege_escalation: bool
    network: SandboxNetworkMode
    network_allowlist: tuple[str, ...]
    mounts: tuple[SandboxMountArgs, ...]
    capabilities: tuple[str, ...]
    resources: SandboxResourceLimitsArgs
    output_quarantine: bool
    release_requires_review: bool

    def __post_init__(self) -> None:
        for name in ("id", "artifact", "runtime", "user"):
            _text(f"profile.{name}", getattr(self, name))
        object.__setattr__(self, "image_digest", _digest("profile.image_digest", self.image_digest, required=False))
        object.__setattr__(self, "environment_digest", _digest("profile.environment_digest", self.environment_digest, required=False))
        object.__setattr__(self, "network", _enum("profile.network", self.network, frozenset({"deny", "allowlist", "unrestricted"})))
        object.__setattr__(self, "network_allowlist", tuple(_network_target(f"profile.network_allowlist[{i}]", item) for i, item in enumerate(_bounded("profile.network_allowlist", self.network_allowlist, SANDBOX_MAX_LIST_ITEMS))))
        object.__setattr__(self, "mounts", tuple(item if isinstance(item, SandboxMountArgs) else SandboxMountArgs.from_wire(item) for item in _bounded("profile.mounts", self.mounts, SANDBOX_MAX_MOUNTS)))
        object.__setattr__(self, "capabilities", _strings("profile.capabilities", self.capabilities))
        if not isinstance(self.resources, SandboxResourceLimitsArgs):
            object.__setattr__(self, "resources", SandboxResourceLimitsArgs.from_wire(self.resources))
        for name in ("rootless", "read_only_root", "no_privilege_escalation", "output_quarantine", "release_requires_review"):
            object.__setattr__(self, name, _bool(f"profile.{name}", getattr(self, name)))

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "SandboxExecutionProfileArgs":
        raw = _mapping("sandbox execution profile", value)
        return cls(_text("profile.id", raw.get("id")), _text("profile.artifact", raw.get("artifact")), _text("profile.runtime", raw.get("runtime")), raw.get("image_digest"), raw.get("environment_digest"), _text("profile.user", raw.get("user")), _bool("profile.rootless", raw.get("rootless")), _bool("profile.read_only_root", raw.get("read_only_root")), _bool("profile.no_privilege_escalation", raw.get("no_privilege_escalation")), _enum("profile.network", raw.get("network"), frozenset({"deny", "allowlist", "unrestricted"})), tuple(_network_target(f"profile.network_allowlist[{i}]", item) for i, item in enumerate(_bounded("profile.network_allowlist", raw.get("network_allowlist", []), SANDBOX_MAX_LIST_ITEMS))), tuple(SandboxMountArgs.from_wire(item) for item in _bounded("profile.mounts", raw.get("mounts", []), SANDBOX_MAX_MOUNTS)), _strings("profile.capabilities", raw.get("capabilities", [])), SandboxResourceLimitsArgs.from_wire(raw.get("resources")), _bool("profile.output_quarantine", raw.get("output_quarantine")), _bool("profile.release_requires_review", raw.get("release_requires_review")))

    def to_wire(self) -> dict[str, Any]:
        return {"id": self.id, "artifact": self.artifact, "runtime": self.runtime, "image_digest": self.image_digest, "environment_digest": self.environment_digest, "user": self.user, "rootless": self.rootless, "read_only_root": self.read_only_root, "no_privilege_escalation": self.no_privilege_escalation, "network": self.network, "network_allowlist": list(self.network_allowlist), "mounts": [item.to_wire() for item in self.mounts], "capabilities": list(self.capabilities), "resources": self.resources.to_wire(), "output_quarantine": self.output_quarantine, "release_requires_review": self.release_requires_review}


@dataclass(frozen=True)
class SandboxCapabilityArgs:
    id: str
    profile: str
    kind: SandboxCapabilityKind
    target: str
    decision: SandboxDecision
    evidence_digest: str | None

    def __post_init__(self) -> None:
        for name in ("id", "profile"):
            _text(f"capability.{name}", getattr(self, name))
        target = _text("capability.target", self.target)
        assert target is not None
        if target == "*" or ".." in target:
            raise ArgumentError("capability.target must be a bounded resource target")
        object.__setattr__(self, "kind", _enum("capability.kind", self.kind, frozenset({"filesystem_read", "filesystem_write", "network_egress", "network_ingress", "secret_access", "process_spawn", "device_access", "kernel_access", "clock", "randomness", "artifact_publish"})))
        object.__setattr__(self, "decision", _enum("capability.decision", self.decision, frozenset({"allow", "deny"})))
        object.__setattr__(self, "evidence_digest", _digest("capability.evidence_digest", self.evidence_digest, required=False))

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "SandboxCapabilityArgs":
        raw = _mapping("sandbox capability", value)
        return cls(_text("capability.id", raw.get("id")), _text("capability.profile", raw.get("profile")), _enum("capability.kind", raw.get("kind"), frozenset({"filesystem_read", "filesystem_write", "network_egress", "network_ingress", "secret_access", "process_spawn", "device_access", "kernel_access", "clock", "randomness", "artifact_publish"})), _text("capability.target", raw.get("target")), _enum("capability.decision", raw.get("decision"), frozenset({"allow", "deny"})), raw.get("evidence_digest"))  # type: ignore[arg-type]

    def to_wire(self) -> dict[str, Any]:
        return {"id": self.id, "profile": self.profile, "kind": self.kind, "target": self.target, "decision": self.decision, "evidence_digest": self.evidence_digest}


@dataclass(frozen=True)
class SandboxOutputArgs:
    id: str
    profile: str
    artifact: str
    digest: str
    destination: str
    quarantined: bool
    released: bool
    reviewed: bool
    review_evidence: str | None
    parents: tuple[str, ...]

    def __post_init__(self) -> None:
        for name in ("id", "profile", "artifact", "destination"):
            _text(f"output.{name}", getattr(self, name))
        object.__setattr__(self, "digest", _digest("output.digest", self.digest))
        object.__setattr__(self, "review_evidence", _digest("output.review_evidence", self.review_evidence, required=False))
        object.__setattr__(self, "parents", _strings("output.parents", self.parents))
        for name in ("quarantined", "released", "reviewed"):
            object.__setattr__(self, name, _bool(f"output.{name}", getattr(self, name)))

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "SandboxOutputArgs":
        raw = _mapping("sandbox output", value)
        return cls(_text("output.id", raw.get("id")), _text("output.profile", raw.get("profile")), _text("output.artifact", raw.get("artifact")), _digest("output.digest", raw.get("digest")), _text("output.destination", raw.get("destination")), _bool("output.quarantined", raw.get("quarantined")), _bool("output.released", raw.get("released")), _bool("output.reviewed", raw.get("reviewed")), raw.get("review_evidence"), _strings("output.parents", raw.get("parents", [])))  # type: ignore[arg-type]

    def to_wire(self) -> dict[str, Any]:
        return {"id": self.id, "profile": self.profile, "artifact": self.artifact, "digest": self.digest, "destination": self.destination, "quarantined": self.quarantined, "released": self.released, "reviewed": self.reviewed, "review_evidence": self.review_evidence, "parents": list(self.parents)}


@dataclass(frozen=True)
class SandboxPoliciesArgs:
    default_deny: bool = True
    require_digests: bool = True
    require_lineage: bool = True
    require_rootless: bool = True
    require_read_only_root: bool = True
    require_no_privilege_escalation: bool = True
    require_network_allowlist: bool = True
    require_resource_limits: bool = True
    require_quarantine: bool = True
    require_output_review: bool = True
    require_reproducible_environment: bool = True

    def __post_init__(self) -> None:
        for name in ("default_deny", "require_digests", "require_lineage", "require_rootless", "require_read_only_root", "require_no_privilege_escalation", "require_network_allowlist", "require_resource_limits", "require_quarantine", "require_output_review", "require_reproducible_environment"):
            object.__setattr__(self, name, _bool(f"policies.{name}", getattr(self, name)))

    @classmethod
    def from_wire(cls, value: Mapping[str, Any] | None) -> "SandboxPoliciesArgs":
        raw = {} if value is None else _mapping("sandbox policies", value)
        return cls(*(raw.get(name, True) for name in ("default_deny", "require_digests", "require_lineage", "require_rootless", "require_read_only_root", "require_no_privilege_escalation", "require_network_allowlist", "require_resource_limits", "require_quarantine", "require_output_review", "require_reproducible_environment")))

    def to_wire(self) -> dict[str, Any]:
        return {name: getattr(self, name) for name in ("default_deny", "require_digests", "require_lineage", "require_rootless", "require_read_only_root", "require_no_privilege_escalation", "require_network_allowlist", "require_resource_limits", "require_quarantine", "require_output_review", "require_reproducible_environment")}


@dataclass(frozen=True, init=False)
class SandboxManifestArgs:
    schema: str
    system: SandboxSystemArgs
    artifacts: tuple[SandboxArtifactArgs, ...]
    profiles: tuple[SandboxExecutionProfileArgs, ...]
    capabilities: tuple[SandboxCapabilityArgs, ...]
    outputs: tuple[SandboxOutputArgs, ...]
    policies: SandboxPoliciesArgs

    def __init__(self, system: SandboxSystemArgs | Mapping[str, Any], artifacts: Sequence[SandboxArtifactArgs | Mapping[str, Any]] = (), profiles: Sequence[SandboxExecutionProfileArgs | Mapping[str, Any]] = (), capabilities: Sequence[SandboxCapabilityArgs | Mapping[str, Any]] = (), outputs: Sequence[SandboxOutputArgs | Mapping[str, Any]] = (), policies: SandboxPoliciesArgs | Mapping[str, Any] | None = None, schema: str = SANDBOX_MANIFEST_SCHEMA) -> None:
        normalized_schema = _text("sandbox schema", schema)
        normalized_system = system if isinstance(system, SandboxSystemArgs) else SandboxSystemArgs.from_wire(system)
        values = [("artifacts", artifacts, SANDBOX_MAX_ARTIFACTS, SandboxArtifactArgs), ("profiles", profiles, SANDBOX_MAX_PROFILES, SandboxExecutionProfileArgs), ("capabilities", capabilities, SANDBOX_MAX_CAPABILITIES, SandboxCapabilityArgs), ("outputs", outputs, SANDBOX_MAX_OUTPUTS, SandboxOutputArgs)]
        normalized: dict[str, tuple[Any, ...]] = {}
        for name, raw_values, limit, item_class in values:
            normalized[name] = tuple(item if isinstance(item, item_class) else item_class.from_wire(item) for item in _bounded(f"sandbox {name}", raw_values, limit))
        normalized_policies = policies if isinstance(policies, SandboxPoliciesArgs) else SandboxPoliciesArgs.from_wire(policies)
        wire = {"schema": normalized_schema, "system": normalized_system.to_wire(), **{name: [item.to_wire() for item in items] for name, items in normalized.items()}, "policies": normalized_policies.to_wire()}
        _json_size("sandbox manifest", wire)
        for name, value in (("schema", normalized_schema), ("system", normalized_system), *normalized.items(), ("policies", normalized_policies)):
            object.__setattr__(self, name, value)

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "SandboxManifestArgs":
        raw = _mapping("sandbox manifest", value)
        return cls(raw.get("system"), _bounded("sandbox artifacts", raw.get("artifacts", []), SANDBOX_MAX_ARTIFACTS), _bounded("sandbox profiles", raw.get("profiles", []), SANDBOX_MAX_PROFILES), _bounded("sandbox capabilities", raw.get("capabilities", []), SANDBOX_MAX_CAPABILITIES), _bounded("sandbox outputs", raw.get("outputs", []), SANDBOX_MAX_OUTPUTS), raw.get("policies"), raw.get("schema", SANDBOX_MANIFEST_SCHEMA))

    def to_wire(self) -> dict[str, Any]:
        return {"schema": self.schema, "system": self.system.to_wire(), "artifacts": [item.to_wire() for item in self.artifacts], "profiles": [item.to_wire() for item in self.profiles], "capabilities": [item.to_wire() for item in self.capabilities], "outputs": [item.to_wire() for item in self.outputs], "policies": self.policies.to_wire()}

    def to_mcp_arguments(self) -> dict[str, Any]:
        return {"manifest": self.to_wire()}


def _payload(value: Mapping[str, Any]) -> dict[str, Any]:
    raw = _mapping("sandbox response", value)
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
        if candidate.get("schema") == SANDBOX_AUDIT_SCHEMA and "ok" in candidate:
            return dict(candidate)
    raise ArgumentError("response does not contain a sandbox admission projection")


@dataclass(frozen=True)
class SandboxIssueReport:
    code: str
    severity: SandboxIssueSeverity
    subject: str
    detail: str
    remediation: str

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "SandboxIssueReport":
        raw = _mapping("sandbox issue", value)
        return cls(_text("sandbox issue code", raw.get("code")), _enum("sandbox issue severity", raw.get("severity"), frozenset({"warning", "blocking"})), _text("sandbox issue subject", raw.get("subject")), _text("sandbox issue detail", raw.get("detail")), _text("sandbox issue remediation", raw.get("remediation")))  # type: ignore[arg-type]


def _row_text(name: str, raw: Mapping[str, Any], key: str) -> str:
    return _text(f"{name}.{key}", raw.get(key))  # type: ignore[return-value]


def _row_bool(name: str, raw: Mapping[str, Any], key: str) -> bool:
    return _bool(f"{name}.{key}", raw.get(key))


@dataclass(frozen=True)
class SandboxArtifactAuditReport:
    artifact_id: str
    digest_valid: bool
    lineage_valid: bool
    source_valid: bool
    trust: SandboxTrust
    hardening_required: bool
    ready: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "SandboxArtifactAuditReport":
        raw = _mapping("sandbox artifact audit", value)
        return cls(_row_text("artifact audit", raw, "artifact_id"), _row_bool("artifact audit", raw, "digest_valid"), _row_bool("artifact audit", raw, "lineage_valid"), _row_bool("artifact audit", raw, "source_valid"), _enum("artifact audit.trust", raw.get("trust"), frozenset({"untrusted", "internal", "reviewed", "trusted"})), _row_bool("artifact audit", raw, "hardening_required"), _row_bool("artifact audit", raw, "ready"))  # type: ignore[arg-type]


@dataclass(frozen=True)
class SandboxProfileAuditReport:
    profile_id: str
    artifact_valid: bool
    isolation_valid: bool
    network_valid: bool
    mounts_valid: bool
    capabilities_valid: bool
    resources_valid: bool
    output_valid: bool
    ready: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "SandboxProfileAuditReport":
        raw = _mapping("sandbox profile audit", value)
        return cls(_row_text("profile audit", raw, "profile_id"), *(_row_bool("profile audit", raw, key) for key in ("artifact_valid", "isolation_valid", "network_valid", "mounts_valid", "capabilities_valid", "resources_valid", "output_valid", "ready")))  # type: ignore[arg-type]


@dataclass(frozen=True)
class SandboxCapabilityAuditReport:
    capability_id: str
    profile_valid: bool
    target_valid: bool
    approved: bool
    dangerous: bool
    evidence_valid: bool
    ready: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "SandboxCapabilityAuditReport":
        raw = _mapping("sandbox capability audit", value)
        return cls(_row_text("capability audit", raw, "capability_id"), *(_row_bool("capability audit", raw, key) for key in ("profile_valid", "target_valid", "approved", "dangerous", "evidence_valid", "ready")))  # type: ignore[arg-type]


@dataclass(frozen=True)
class SandboxBoundaryAuditReport:
    profile_id: str
    default_deny: bool
    network_mode: SandboxNetworkMode
    allowlist_valid: bool
    host_paths_rejected: bool
    dangerous_capabilities: int
    ready: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "SandboxBoundaryAuditReport":
        raw = _mapping("sandbox boundary audit", value)
        return cls(_row_text("boundary audit", raw, "profile_id"), _row_bool("boundary audit", raw, "default_deny"), _enum("boundary audit.network_mode", raw.get("network_mode"), frozenset({"deny", "allowlist", "unrestricted"})), _row_bool("boundary audit", raw, "allowlist_valid"), _row_bool("boundary audit", raw, "host_paths_rejected"), _integer("boundary audit.dangerous_capabilities", raw.get("dangerous_capabilities"), required=True), _row_bool("boundary audit", raw, "ready"))  # type: ignore[arg-type]


@dataclass(frozen=True)
class SandboxResourceAuditReport:
    profile_id: str
    cpu_bounded: bool
    memory_bounded: bool
    wall_time_bounded: bool
    processes_bounded: bool
    output_bounded: bool
    ready: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "SandboxResourceAuditReport":
        raw = _mapping("sandbox resource audit", value)
        return cls(_row_text("resource audit", raw, "profile_id"), *(_row_bool("resource audit", raw, key) for key in ("cpu_bounded", "memory_bounded", "wall_time_bounded", "processes_bounded", "output_bounded", "ready")))  # type: ignore[arg-type]


@dataclass(frozen=True)
class SandboxOutputAuditReport:
    output_id: str
    profile_valid: bool
    artifact_valid: bool
    digest_valid: bool
    lineage_valid: bool
    quarantined: bool
    review_valid: bool
    release_valid: bool
    ready: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "SandboxOutputAuditReport":
        raw = _mapping("sandbox output audit", value)
        return cls(_row_text("output audit", raw, "output_id"), *(_row_bool("output audit", raw, key) for key in ("profile_valid", "artifact_valid", "digest_valid", "lineage_valid", "quarantined", "review_valid", "release_valid", "ready")))  # type: ignore[arg-type]


@dataclass(frozen=True)
class SandboxAuditReport:
    raw: dict[str, Any]
    ok: bool
    schema: str | None
    workflow: str | None
    manifest_digest: str | None
    valid: bool | None
    sandbox_ready_value: bool | None
    counts: Mapping[str, Any] | None
    artifact_audits: tuple[SandboxArtifactAuditReport, ...]
    profile_audits: tuple[SandboxProfileAuditReport, ...]
    capability_audits: tuple[SandboxCapabilityAuditReport, ...]
    boundary_audits: tuple[SandboxBoundaryAuditReport, ...]
    resource_audits: tuple[SandboxResourceAuditReport, ...]
    output_audits: tuple[SandboxOutputAuditReport, ...]
    issues: tuple[SandboxIssueReport, ...]
    guarantees: tuple[str, ...]
    limitations: tuple[str, ...]
    refusal: str | None
    fail_closed: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "SandboxAuditReport":
        raw = _payload(value)
        if raw.get("ok") is not True:
            if raw.get("fail_closed") is not True:
                raise ArgumentError("sandbox refusals must be fail-closed")
            return cls(raw, False, raw.get("schema"), raw.get("workflow"), raw.get("manifest_digest"), False, False, None, (), (), (), (), (), (), (), _route_strings("sandbox refusal guarantees", raw.get("guarantees", [])), _route_strings("sandbox refusal limitations", raw.get("limitations", [])), raw.get("refusal") or raw.get("error"), True)
        if raw.get("schema") != SANDBOX_AUDIT_SCHEMA:
            raise ArgumentError("sandbox projection has an invalid schema")
        audit = _mapping("sandbox audit", raw.get("audit"))
        return cls(raw, True, SANDBOX_AUDIT_SCHEMA, _text("sandbox workflow", raw.get("workflow")), _text("sandbox manifest_digest", raw.get("manifest_digest"), required=False), _bool("sandbox valid", audit.get("valid")), _bool("sandbox_ready", raw.get("sandbox_ready")), _mapping("sandbox counts", audit.get("counts")), tuple(SandboxArtifactAuditReport.from_wire(item) for item in _bounded("sandbox artifact audits", audit.get("artifact_audits", []), SANDBOX_MAX_ARTIFACTS)), tuple(SandboxProfileAuditReport.from_wire(item) for item in _bounded("sandbox profile audits", audit.get("profile_audits", []), SANDBOX_MAX_PROFILES)), tuple(SandboxCapabilityAuditReport.from_wire(item) for item in _bounded("sandbox capability audits", audit.get("capability_audits", []), SANDBOX_MAX_CAPABILITIES)), tuple(SandboxBoundaryAuditReport.from_wire(item) for item in _bounded("sandbox boundary audits", audit.get("boundary_audits", []), SANDBOX_MAX_PROFILES)), tuple(SandboxResourceAuditReport.from_wire(item) for item in _bounded("sandbox resource audits", audit.get("resource_audits", []), SANDBOX_MAX_PROFILES)), tuple(SandboxOutputAuditReport.from_wire(item) for item in _bounded("sandbox output audits", audit.get("output_audits", []), SANDBOX_MAX_OUTPUTS)), tuple(SandboxIssueReport.from_wire(item) for item in _bounded("sandbox issues", audit.get("issues", []), SANDBOX_MAX_LIST_ITEMS)), _route_strings("sandbox guarantees", raw.get("guarantees", audit.get("guarantees", []))), _route_strings("sandbox limitations", raw.get("limitations", audit.get("limitations", []))), None, False)

    @property
    def accepted(self) -> bool:
        return self.ok and self.valid is True and self.sandbox_ready_value is True

    @property
    def sandbox_ready(self) -> bool:
        return self.sandbox_ready_value is True

    @property
    def refused(self) -> bool:
        return not self.ok

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


def sandbox_admission_audit_report(value: Mapping[str, Any]) -> SandboxAuditReport:
    """Parse direct MCP output or an HTTP REST tool envelope."""

    return SandboxAuditReport.from_wire(value)


__all__ = [
    "SANDBOX_MANIFEST_SCHEMA", "SANDBOX_AUDIT_SCHEMA", "SANDBOX_MAX_INPUT_BYTES", "SANDBOX_MAX_ARTIFACTS", "SANDBOX_MAX_PROFILES", "SANDBOX_MAX_CAPABILITIES", "SANDBOX_MAX_MOUNTS", "SANDBOX_MAX_OUTPUTS",
    "SandboxSystemArgs", "SandboxArtifactArgs", "SandboxMountArgs", "SandboxResourceLimitsArgs", "SandboxExecutionProfileArgs", "SandboxCapabilityArgs", "SandboxOutputArgs", "SandboxPoliciesArgs", "SandboxManifestArgs", "SandboxIssueReport", "SandboxArtifactAuditReport", "SandboxProfileAuditReport", "SandboxCapabilityAuditReport", "SandboxBoundaryAuditReport", "SandboxResourceAuditReport", "SandboxOutputAuditReport", "SandboxAuditReport", "sandbox_admission_audit_report",
]
