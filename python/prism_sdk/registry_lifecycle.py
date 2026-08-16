"""Typed projections for the local content-addressed registry lifecycle.

The registry authority owns attestation verification, tier policy, immutable artifacts, and
append-only publication events.  This SDK surface keeps the governance evidence visible without
trying to reproduce those predicates in Python: invalid packs remain preflight rows, failed
operations do not erase later independent actions, integrity is checked before mutation, and the
serialized index can be carried into a later simulation.
"""

from __future__ import annotations

from dataclasses import dataclass
import json
from typing import Any, Mapping, Sequence

from .capability import _route_count, _route_mapping, _route_strings, _route_text
from .errors import ArgumentError


REGISTRY_LIFECYCLE_MAX_INPUT_BYTES = 20_000_000
REGISTRY_LIFECYCLE_MAX_PACKS = 64
REGISTRY_LIFECYCLE_MAX_ACTIONS = 256
REGISTRY_OPERATIONS = frozenset({"publish", "promote", "reassess", "supersede", "withdraw", "resolve", "history", "inspect", "revisions", "verify_all"})
REGISTRY_TIERS = frozenset({"unranked", "exploratory", "validated", "trusted"})


def _bool(name: str, value: Any) -> bool:
    if not isinstance(value, bool):
        raise ArgumentError(f"{name} must be a boolean")
    return value


def _sequence(name: str, value: Any) -> tuple[Any, ...]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes, bytearray)):
        raise ArgumentError(f"{name} must be an array")
    return tuple(value)


def _optional_text(name: str, value: Any) -> str | None:
    return None if value is None else _route_text(name, value)


def _payload(value: Mapping[str, Any]) -> dict[str, Any]:
    raw = _route_mapping("registry lifecycle response", value)
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
            if isinstance(content, list):
                for block in content:
                    if isinstance(block, Mapping) and isinstance(block.get("text"), str):
                        try:
                            decoded = json.loads(block["text"])
                        except json.JSONDecodeError as error:
                            raise ArgumentError(f"registry lifecycle response text is not JSON: {error}") from error
                        if isinstance(decoded, Mapping):
                            candidates.append(decoded)
        structured = container.get("structuredContent")
        if isinstance(structured, Mapping):
            candidates.append(structured)

    add_container(raw.get("mcp"))
    add_container(raw.get("result"))
    add_container(raw.get("structuredContent"))
    for candidate in candidates:
        if candidate.get("ok") is True and candidate.get("schema") == "bioprism-mcp/registry-lifecycle/0.1" and isinstance(candidate.get("actions"), list) and isinstance(candidate.get("final"), Mapping):
            return dict(candidate)
    raise ArgumentError("response does not contain a registry lifecycle projection")


@dataclass(frozen=True)
class RegistryLifecycleSimulateArgs:
    """Bounded registry inputs, including an optional serialized continuation index."""

    packs: tuple[Any, ...] = ()
    index: dict[str, Any] | None = None
    policy: dict[str, Any] | None = None
    actions: tuple[Any, ...] = ()
    include_index: bool = True

    def __init__(self, packs: Sequence[Any] = (), index: Mapping[str, Any] | None = None, policy: Mapping[str, Any] | None = None, actions: Sequence[Any] = (), include_index: bool = True) -> None:
        normalized_packs = _sequence("registry lifecycle packs", packs)
        normalized_actions = _sequence("registry lifecycle actions", actions)
        if len(normalized_packs) > REGISTRY_LIFECYCLE_MAX_PACKS:
            raise ArgumentError("registry lifecycle packs must contain at most 64 documents")
        if len(normalized_actions) > REGISTRY_LIFECYCLE_MAX_ACTIONS:
            raise ArgumentError("registry lifecycle actions must contain at most 256 operations")
        normalized_index = None if index is None else _route_mapping("registry lifecycle index", index)
        normalized_policy = None if policy is None else _route_mapping("registry lifecycle policy", policy)
        normalized_include = _bool("registry lifecycle include_index", include_index)
        arguments = {"packs": list(normalized_packs), "index": normalized_index, "policy": normalized_policy, "actions": list(normalized_actions), "include_index": normalized_include}
        try:
            encoded = json.dumps(arguments, ensure_ascii=False, separators=(",", ":"), allow_nan=False).encode("utf-8")
        except (TypeError, ValueError) as error:
            raise ArgumentError(f"registry lifecycle arguments are not JSON serializable: {error}") from error
        if len(encoded) > REGISTRY_LIFECYCLE_MAX_INPUT_BYTES:
            raise ArgumentError("registry lifecycle input exceeds the 20 MB safety bound")
        object.__setattr__(self, "packs", normalized_packs)
        object.__setattr__(self, "index", normalized_index)
        object.__setattr__(self, "policy", normalized_policy)
        object.__setattr__(self, "actions", normalized_actions)
        object.__setattr__(self, "include_index", normalized_include)

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "RegistryLifecycleSimulateArgs":
        raw = _route_mapping("registry lifecycle arguments", value)
        return cls(raw.get("packs", []), raw.get("index"), raw.get("policy"), raw.get("actions", []), raw.get("include_index", True))

    def to_mcp_arguments(self) -> dict[str, Any]:
        result: dict[str, Any] = {"packs": list(self.packs), "actions": list(self.actions), "include_index": self.include_index}
        if self.index is not None:
            result["index"] = dict(self.index)
        if self.policy is not None:
            result["policy"] = dict(self.policy)
        return result


@dataclass(frozen=True)
class RegistryPackPreflightReport:
    raw: dict[str, Any]
    index: int
    valid: bool
    name: str | None
    artifact_digest: str | None
    core_digest: str | None
    publisher: str | None
    instance_count: int | None
    refusal: str | None
    fail_closed: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "RegistryPackPreflightReport":
        raw = _route_mapping("registry pack preflight", value)
        index = _route_count("registry pack preflight index", raw.get("index"))
        valid = _bool("registry pack preflight valid", raw.get("valid"))
        refusal = _optional_text("registry pack preflight refusal", raw.get("refusal"))
        fail_closed = _bool("registry pack preflight fail_closed", raw.get("fail_closed", False))
        if valid and refusal is not None:
            raise ArgumentError("valid registry pack rows cannot retain a refusal")
        if not valid and (refusal is None or not fail_closed):
            raise ArgumentError("invalid registry pack rows must retain a fail-closed refusal")
        instance_count = raw.get("instance_count")
        if instance_count is not None:
            instance_count = _route_count("registry pack instance_count", instance_count)
        return cls(raw, index, valid, _optional_text("registry pack name", raw.get("name")), _optional_text("registry artifact digest", raw.get("artifact_digest")), _optional_text("registry core digest", raw.get("core_digest")), _optional_text("registry publisher", raw.get("publisher")), instance_count, refusal, fail_closed)


@dataclass(frozen=True)
class RegistryBrokenArtifactReport:
    raw: dict[str, Any]
    digest: str
    attestation: str

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "RegistryBrokenArtifactReport":
        raw = _route_mapping("registry broken artifact", value)
        return cls(raw, _route_text("registry broken digest", raw.get("digest")), _route_text("registry broken attestation", raw.get("attestation")))


@dataclass(frozen=True)
class RegistryIntegrityReport:
    raw: dict[str, Any]
    artifact_count: int
    log_count: int
    broken_count: int
    broken: tuple[RegistryBrokenArtifactReport, ...]
    operations_allowed: bool | None

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "RegistryIntegrityReport":
        raw = _route_mapping("registry integrity", value)
        broken = tuple(RegistryBrokenArtifactReport.from_wire(item) for item in _sequence("registry broken artifacts", raw.get("broken", [])))
        broken_count = _route_count("registry broken_count", raw.get("broken_count"))
        if broken_count != len(broken):
            raise ArgumentError("registry broken_count does not match broken rows")
        operations_allowed = raw.get("operations_allowed")
        if operations_allowed is not None:
            operations_allowed = _bool("registry operations_allowed", operations_allowed)
        return cls(raw, _route_count("registry artifact_count", raw.get("artifact_count")), _route_count("registry log_count", raw.get("log_count")), broken_count, broken, operations_allowed)

    @property
    def clean(self) -> bool:
        return self.broken_count == 0


@dataclass(frozen=True)
class RegistryActionReport:
    raw: dict[str, Any]
    index: int
    operation: str | None
    ok: bool
    result: Any
    refusal: str | None
    fail_closed: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "RegistryActionReport":
        raw = _route_mapping("registry action", value)
        index = _route_count("registry action index", raw.get("index"))
        operation = _optional_text("registry action op", raw.get("op"))
        if operation is not None and operation not in REGISTRY_OPERATIONS:
            raise ArgumentError(f"unknown registry operation {operation!r}")
        ok = _bool("registry action ok", raw.get("ok"))
        refusal = _optional_text("registry action refusal", raw.get("refusal"))
        fail_closed = _bool("registry action fail_closed", raw.get("fail_closed", False))
        if ok and refusal is not None:
            raise ArgumentError("successful registry actions cannot retain a refusal")
        if not ok and (refusal is None or not fail_closed):
            raise ArgumentError("failed registry actions must retain a fail-closed refusal")
        return cls(raw, index, operation, ok, raw.get("result"), refusal, fail_closed)

    @property
    def digest(self) -> str | None:
        return _optional_text("registry action result digest", self.result.get("digest")) if isinstance(self.result, Mapping) and self.result.get("digest") is not None else None

    @property
    def found(self) -> bool | None:
        if not isinstance(self.result, Mapping) or "found" not in self.result:
            return None
        return _bool("registry action found", self.result.get("found"))

    @property
    def clean(self) -> bool | None:
        if not isinstance(self.result, Mapping) or "clean" not in self.result:
            return None
        return _bool("registry action clean", self.result.get("clean"))

    @property
    def event_count(self) -> int | None:
        if not isinstance(self.result, Mapping) or self.result.get("event_count") is None:
            return None
        return _route_count("registry action event_count", self.result.get("event_count"))


@dataclass(frozen=True)
class RegistryFinalReport:
    raw: dict[str, Any]
    artifact_count: int
    log_count: int
    broken_count: int
    integrity_clean: bool
    verification: tuple[RegistryBrokenArtifactReport, ...]
    log: tuple[dict[str, Any], ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "RegistryFinalReport":
        raw = _route_mapping("registry final report", value)
        verification = tuple(RegistryBrokenArtifactReport.from_wire(item) for item in _sequence("registry final verification", raw.get("verification", [])))
        broken_count = _route_count("registry final broken_count", raw.get("broken_count"))
        if broken_count != len(verification):
            raise ArgumentError("registry final broken_count does not match verification rows")
        integrity_clean = _bool("registry final integrity_clean", raw.get("integrity_clean"))
        if integrity_clean != (broken_count == 0):
            raise ArgumentError("registry final integrity_clean does not match broken_count")
        return cls(raw, _route_count("registry final artifact_count", raw.get("artifact_count")), _route_count("registry final log_count", raw.get("log_count")), broken_count, integrity_clean, verification, tuple(_route_mapping("registry publication log event", item) for item in _sequence("registry final log", raw.get("log", []))))


@dataclass(frozen=True)
class RegistryLifecycleReport:
    raw: dict[str, Any]
    ok: bool
    schema: str
    policy: dict[str, Any]
    packs: tuple[RegistryPackPreflightReport, ...]
    initial_integrity: RegistryIntegrityReport
    actions: tuple[RegistryActionReport, ...]
    final: RegistryFinalReport
    registry: dict[str, Any] | None
    guarantees: tuple[str, ...]
    limitations: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "RegistryLifecycleReport":
        raw = _payload(value)
        ok = _bool("registry lifecycle ok", raw.get("ok"))
        schema = _route_text("registry lifecycle schema", raw.get("schema"))
        policy = _route_mapping("registry lifecycle policy", raw.get("policy"))
        actions = tuple(RegistryActionReport.from_wire(item) for item in _sequence("registry lifecycle actions", raw.get("actions")))
        registry_raw = raw.get("registry")
        registry = None if registry_raw is None else _route_mapping("registry continuation index", registry_raw)
        return cls(raw, ok, schema, policy, tuple(RegistryPackPreflightReport.from_wire(item) for item in _sequence("registry lifecycle packs", raw.get("packs", []))), RegistryIntegrityReport.from_wire(raw.get("initial_integrity")), actions, RegistryFinalReport.from_wire(raw.get("final")), registry, _route_strings("registry lifecycle guarantees", raw.get("guarantees", [])), _route_strings("registry lifecycle limitations", raw.get("limitations", [])))

    @property
    def valid_pack_count(self) -> int:
        return sum(item.valid for item in self.packs)

    @property
    def invalid_pack_count(self) -> int:
        return sum(not item.valid for item in self.packs)

    @property
    def successful_action_count(self) -> int:
        return sum(item.ok for item in self.actions)

    @property
    def failed_action_count(self) -> int:
        return sum(not item.ok for item in self.actions)

    @property
    def fail_closed_action_count(self) -> int:
        return sum(not item.ok and item.fail_closed for item in self.actions)

    @property
    def continuation_available(self) -> bool:
        return self.registry is not None

    @property
    def append_only_events_are_claimed(self) -> bool:
        return any("append-only" in item and "lifecycle events" in item for item in self.guarantees)

    @property
    def independent_actions_are_claimed(self) -> bool:
        return any("failed actions" in item and "independent later actions" in item for item in self.guarantees)

    @property
    def integrity_checked_before_mutation(self) -> bool:
        return any("integrity" in item and "before any lookup or mutation" in item for item in self.guarantees)

    @property
    def withdrawal_is_non_destructive(self) -> bool:
        return any("withdrawal preserves historical bytes" in item and "does not delete" in item for item in self.limitations)

    @property
    def local_and_side_effect_free(self) -> bool:
        return any("local deterministic registry projection" in item and "network" in item for item in self.limitations)

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


def registry_lifecycle_report(value: Mapping[str, Any]) -> RegistryLifecycleReport:
    """Parse direct MCP output or an HTTP REST tool envelope."""

    return RegistryLifecycleReport.from_wire(value)


__all__ = [
    "REGISTRY_LIFECYCLE_MAX_INPUT_BYTES",
    "REGISTRY_LIFECYCLE_MAX_PACKS",
    "REGISTRY_LIFECYCLE_MAX_ACTIONS",
    "REGISTRY_OPERATIONS",
    "REGISTRY_TIERS",
    "RegistryLifecycleSimulateArgs",
    "RegistryPackPreflightReport",
    "RegistryBrokenArtifactReport",
    "RegistryIntegrityReport",
    "RegistryActionReport",
    "RegistryFinalReport",
    "RegistryLifecycleReport",
    "registry_lifecycle_report",
]
