"""Bounded request models for repository knowledge navigation and telemetry projection.

The Rust kernel owns documentation graph traversal, route completeness, redaction semantics, and
observed-versus-asserted metric treatment.  These models only make the transport boundary safer:
they bound serialized inputs, reject path traversal and ambiguous route selections, and preserve
the server's progressive-disclosure and refusal behavior.
"""

from __future__ import annotations

from dataclasses import dataclass
from enum import Enum
from typing import Any, Mapping, Sequence

from .authoring import canonical_json
from .errors import ArgumentError


REPOSITORY_REQUEST_SCHEMA = "bioprism-python-repository-requests/0.1"
MAX_REPOSITORY_REQUEST_BYTES = 20_000_000
MAX_REPOSITORY_PREFIX_BYTES = 4_096
MAX_REPOSITORY_ITEMS = 1_000
MAX_REPOSITORY_LABELS = 1_000
MAX_REPOSITORY_DEPTH = 1_000
MAX_MARKDOWN_CHARS = 2_000_000
MAX_TELEMETRY_TRACE_BYTES = 512


class RepositoryTraversalPolicy(str, Enum):
    """Documentation graph traversal policy accepted by ``repository_bundle``."""

    NORMATIVE = "normative"
    EXHAUSTIVE = "exhaustive"


def _mapping(name: str, value: Mapping[str, Any]) -> dict[str, Any]:
    if not isinstance(value, Mapping):
        raise ArgumentError(f"{name} must be a mapping")
    result = dict(value)
    try:
        encoded = canonical_json(result).encode("utf-8")
    except (TypeError, ValueError) as error:
        raise ArgumentError(f"{name} is not canonical JSON-safe: {error}") from error
    if len(encoded) > MAX_REPOSITORY_REQUEST_BYTES:
        raise ArgumentError(f"{name} exceeds the {MAX_REPOSITORY_REQUEST_BYTES}-byte limit")
    return result


def _text(name: str, value: str, *, maximum: int = MAX_REPOSITORY_PREFIX_BYTES) -> str:
    if not isinstance(value, str) or not value.strip():
        raise ArgumentError(f"{name} must be a non-empty string")
    if len(value.encode("utf-8")) > maximum:
        raise ArgumentError(f"{name} exceeds the {maximum}-byte limit")
    if any(ord(character) < 32 for character in value):
        raise ArgumentError(f"{name} must not contain control characters")
    return value


def _repository_id(name: str, value: str) -> str:
    normalized = _text(name, value)
    portable = normalized.replace("\\", "/")
    if portable.startswith("/") or ":" in portable.split("/", 1)[0]:
        raise ArgumentError(f"{name} must be repository-relative")
    if ".." in portable.split("/"):
        raise ArgumentError(f"{name} must not traverse outside the repository root")
    return normalized


def _policy(value: RepositoryTraversalPolicy | str) -> RepositoryTraversalPolicy:
    if isinstance(value, RepositoryTraversalPolicy):
        return value
    try:
        return RepositoryTraversalPolicy(value)
    except (TypeError, ValueError) as error:
        raise ArgumentError("policy must be 'normative' or 'exhaustive'") from error


def _strings(name: str, value: Sequence[str], maximum: int) -> tuple[str, ...]:
    if isinstance(value, (str, bytes)) or not isinstance(value, Sequence):
        raise ArgumentError(f"{name} must be a sequence of strings")
    if len(value) > maximum:
        raise ArgumentError(f"{name} may contain at most {maximum} items")
    result: list[str] = []
    for index, item in enumerate(value):
        result.append(_text(f"{name}[{index}]", item, maximum=MAX_REPOSITORY_PREFIX_BYTES))
    try:
        encoded = canonical_json(result).encode("utf-8")
    except (TypeError, ValueError) as error:
        raise ArgumentError(f"{name} is not canonical JSON-safe: {error}") from error
    if len(encoded) > MAX_REPOSITORY_REQUEST_BYTES:
        raise ArgumentError(f"{name} exceeds the {MAX_REPOSITORY_REQUEST_BYTES}-byte limit")
    return tuple(result)


def _optional_nonnegative(name: str, value: int | None, maximum: int) -> int | None:
    if value is None:
        return None
    if isinstance(value, bool) or not isinstance(value, int) or not 0 <= value <= maximum:
        raise ArgumentError(f"{name} must be between 0 and {maximum}")
    return value


def _optional_positive(name: str, value: int | None, maximum: int) -> int | None:
    if value is None:
        return None
    if isinstance(value, bool) or not isinstance(value, int) or not 1 <= value <= maximum:
        raise ArgumentError(f"{name} must be between 1 and {maximum}")
    return value


@dataclass(frozen=True)
class RepositoryCatalogRequest:
    """Bounded repository module discovery request."""

    prefix: str | None = None
    limit: int = 200
    include_briefs: bool = False
    include_findings: bool = False

    def __post_init__(self) -> None:
        if self.prefix is not None:
            object.__setattr__(self, "prefix", _repository_id("prefix", self.prefix))
        if isinstance(self.limit, bool) or not isinstance(self.limit, int) or not 1 <= self.limit <= MAX_REPOSITORY_ITEMS:
            raise ArgumentError(f"limit must be between 1 and {MAX_REPOSITORY_ITEMS}")
        if not isinstance(self.include_briefs, bool) or not isinstance(self.include_findings, bool):
            raise ArgumentError("include_briefs and include_findings must be booleans")

    def to_mcp_arguments(self) -> dict[str, Any]:
        arguments: dict[str, Any] = {
            "limit": self.limit,
            "include_briefs": self.include_briefs,
            "include_findings": self.include_findings,
        }
        if self.prefix is not None:
            arguments["prefix"] = self.prefix
        return arguments


@dataclass(frozen=True)
class RepositoryBundleRequest:
    """Route-specific documentation context request with explicit disclosure bounds."""

    route: Mapping[str, Any]
    policy: RepositoryTraversalPolicy | str = RepositoryTraversalPolicy.NORMATIVE
    max_depth: int | None = None
    denied_labels: Sequence[str] = ()
    follow: Sequence[str] = ()
    include_markdown: bool = False
    max_markdown_chars: int | None = None

    def __post_init__(self) -> None:
        object.__setattr__(self, "route", _mapping("route", self.route))
        object.__setattr__(self, "policy", _policy(self.policy))
        object.__setattr__(self, "max_depth", _optional_nonnegative("max_depth", self.max_depth, MAX_REPOSITORY_DEPTH))
        object.__setattr__(self, "denied_labels", _strings("denied_labels", self.denied_labels, MAX_REPOSITORY_LABELS))
        object.__setattr__(self, "follow", _strings("follow", self.follow, MAX_REPOSITORY_LABELS))
        if not isinstance(self.include_markdown, bool):
            raise ArgumentError("include_markdown must be a boolean")
        object.__setattr__(self, "max_markdown_chars", _optional_positive("max_markdown_chars", self.max_markdown_chars, MAX_MARKDOWN_CHARS))

    def to_mcp_arguments(self) -> dict[str, Any]:
        arguments: dict[str, Any] = {
            "route": dict(self.route),
            "policy": self.policy.value,
            "denied_labels": list(self.denied_labels),
            "follow": list(self.follow),
            "include_markdown": self.include_markdown,
        }
        if self.max_depth is not None:
            arguments["max_depth"] = self.max_depth
        if self.max_markdown_chars is not None:
            arguments["max_markdown_chars"] = self.max_markdown_chars
        return arguments


@dataclass(frozen=True)
class RepositoryImpactRequest:
    """Conservative impact request for one changed module and optional route checks."""

    changed: str
    route: Mapping[str, Any] | None = None
    routes: Sequence[Mapping[str, Any]] | None = None

    def __post_init__(self) -> None:
        object.__setattr__(self, "changed", _repository_id("changed", self.changed))
        if self.route is not None and self.routes is not None:
            raise ArgumentError("route and routes are mutually exclusive")
        if self.route is not None:
            object.__setattr__(self, "route", _mapping("route", self.route))
        if self.routes is not None:
            if isinstance(self.routes, (str, bytes)) or not isinstance(self.routes, Sequence):
                raise ArgumentError("routes must be a sequence of mappings")
            if not 1 <= len(self.routes) <= MAX_REPOSITORY_ITEMS:
                raise ArgumentError(f"routes must contain between 1 and {MAX_REPOSITORY_ITEMS} items")
            normalized = tuple(_mapping(f"routes[{index}]", route) for index, route in enumerate(self.routes))
            object.__setattr__(self, "routes", normalized)

    def to_mcp_arguments(self) -> dict[str, Any]:
        arguments: dict[str, Any] = {"changed": self.changed}
        if self.route is not None:
            arguments["route"] = dict(self.route)
        if self.routes is not None:
            arguments["routes"] = [dict(route) for route in self.routes]
        return arguments


@dataclass(frozen=True)
class TelemetryProjectRequest:
    """Redacted telemetry projection request with optional metric evidence."""

    event: Mapping[str, Any]
    policy: Mapping[str, Any]
    trace: str
    metric: Mapping[str, Any] | None = None
    observations: Mapping[str, Any] | None = None

    def __post_init__(self) -> None:
        object.__setattr__(self, "event", _mapping("event", self.event))
        object.__setattr__(self, "policy", _mapping("policy", self.policy))
        object.__setattr__(self, "trace", _text("trace", self.trace, maximum=MAX_TELEMETRY_TRACE_BYTES))
        if self.metric is not None:
            object.__setattr__(self, "metric", _mapping("metric", self.metric))
            if self.observations is None:
                raise ArgumentError("observations are required when metric is supplied")
        if self.observations is not None:
            object.__setattr__(self, "observations", _mapping("observations", self.observations))

    def to_mcp_arguments(self) -> dict[str, Any]:
        arguments: dict[str, Any] = {
            "event": dict(self.event),
            "policy": dict(self.policy),
            "trace": self.trace,
        }
        if self.metric is not None:
            arguments["metric"] = dict(self.metric)
        if self.observations is not None:
            arguments["observations"] = dict(self.observations)
        return arguments


__all__ = [
    "MAX_MARKDOWN_CHARS",
    "MAX_REPOSITORY_DEPTH",
    "MAX_REPOSITORY_ITEMS",
    "MAX_REPOSITORY_LABELS",
    "MAX_REPOSITORY_PREFIX_BYTES",
    "MAX_REPOSITORY_REQUEST_BYTES",
    "MAX_TELEMETRY_TRACE_BYTES",
    "REPOSITORY_REQUEST_SCHEMA",
    "RepositoryBundleRequest",
    "RepositoryCatalogRequest",
    "RepositoryImpactRequest",
    "RepositoryTraversalPolicy",
    "TelemetryProjectRequest",
]
