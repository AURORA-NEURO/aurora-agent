"""Bounded request models for the FIBER progressive-disclosure context lifecycle.

These models make the decision-context pipeline explicit at the Python boundary without copying
Rust's compiler semantics.  A caller can compile a minimal contract, refine it through a
content-addressed handle, inspect the compile plan, verify a certificate, or request graph/table
projections.  Paths remain caller-owned relative paths; the Rust server resolves and authorizes
them against its root and remains authoritative for sufficiency, omission, and certificate rules.
"""

from __future__ import annotations

from dataclasses import dataclass
from enum import Enum
from typing import Any, Mapping

from .authoring import canonical_json
from .errors import ArgumentError


CONTEXT_REQUEST_SCHEMA = "bioprism-python-context-requests/0.1"
MAX_CONTEXT_PATH_BYTES = 4_096
MAX_CONTEXT_HANDLE_BYTES = 5_000_000


class ContextLayer(str, Enum):
    """Progressive-disclosure layers exposed by the FIBER compiler."""

    L0 = "l0"
    L1 = "l1"
    L2 = "l2"
    L3 = "l3"
    L4 = "l4"


def _path(name: str, value: str) -> str:
    if not isinstance(value, str) or not value.strip():
        raise ArgumentError(f"{name} must be a non-empty relative path string")
    if len(value.encode("utf-8")) > MAX_CONTEXT_PATH_BYTES:
        raise ArgumentError(f"{name} exceeds the {MAX_CONTEXT_PATH_BYTES}-byte limit")
    if any(ord(character) < 32 for character in value):
        raise ArgumentError(f"{name} must not contain control characters")
    normalized = value.replace("\\", "/")
    if normalized.startswith("/") or ":" in normalized.split("/", 1)[0]:
        raise ArgumentError(f"{name} must be relative to the server root")
    if ".." in normalized.split("/"):
        raise ArgumentError(f"{name} must not traverse outside the server root")
    return value


def _layer(value: ContextLayer | str) -> ContextLayer:
    if isinstance(value, ContextLayer):
        return value
    try:
        return ContextLayer(value)
    except (TypeError, ValueError) as error:
        raise ArgumentError("layer must be one of 'l0', 'l1', 'l2', 'l3', or 'l4'") from error


def _handle(value: Mapping[str, Any]) -> dict[str, Any]:
    if not isinstance(value, Mapping):
        raise ArgumentError("handle must be a mapping")
    result = dict(value)
    try:
        encoded = canonical_json(result).encode("utf-8")
    except (TypeError, ValueError) as error:
        raise ArgumentError(f"handle is not canonical JSON-safe: {error}") from error
    if len(encoded) > MAX_CONTEXT_HANDLE_BYTES:
        raise ArgumentError(f"handle exceeds the {MAX_CONTEXT_HANDLE_BYTES}-byte limit")
    return result


def _source_pair(
    handle: Mapping[str, Any] | None,
    world: str | None,
    query: str | None,
) -> tuple[dict[str, Any] | None, str | None, str | None]:
    if handle is not None and (world is not None or query is not None):
        raise ArgumentError("handle is mutually exclusive with world and query")
    if handle is None and (world is None or query is None):
        raise ArgumentError("supply handle or both world and query")
    if handle is not None:
        return _handle(handle), None, None
    assert world is not None and query is not None
    return None, _path("world", world), _path("query", query)


@dataclass(frozen=True)
class FiberCompileRequest:
    """Compile a world/query pair starting at an explicit progressive layer."""

    world: str
    query: str
    layer: ContextLayer | str = ContextLayer.L0

    def __post_init__(self) -> None:
        object.__setattr__(self, "world", _path("world", self.world))
        object.__setattr__(self, "query", _path("query", self.query))
        object.__setattr__(self, "layer", _layer(self.layer))

    def to_mcp_arguments(self) -> dict[str, Any]:
        return {"world": self.world, "query": self.query, "layer": self.layer.value}


@dataclass(frozen=True)
class FiberRefineRequest:
    """Refine a compiled handle or recompile from world/query input at a target layer."""

    layer: ContextLayer | str
    handle: Mapping[str, Any] | None = None
    world: str | None = None
    query: str | None = None

    def __post_init__(self) -> None:
        normalized_handle, normalized_world, normalized_query = _source_pair(self.handle, self.world, self.query)
        object.__setattr__(self, "handle", normalized_handle)
        object.__setattr__(self, "world", normalized_world)
        object.__setattr__(self, "query", normalized_query)
        object.__setattr__(self, "layer", _layer(self.layer))

    def to_mcp_arguments(self) -> dict[str, Any]:
        arguments: dict[str, Any] = {"layer": self.layer.value}
        if self.handle is not None:
            arguments["handle"] = dict(self.handle)
        else:
            arguments["world"] = self.world
            arguments["query"] = self.query
        return arguments


@dataclass(frozen=True)
class FiberExplainRequest:
    """Request the compiler plan and omission explanation for a world/query pair."""

    world: str
    query: str

    def __post_init__(self) -> None:
        object.__setattr__(self, "world", _path("world", self.world))
        object.__setattr__(self, "query", _path("query", self.query))

    def to_mcp_arguments(self) -> dict[str, Any]:
        return {"world": self.world, "query": self.query}


@dataclass(frozen=True)
class FiberVerifyRequest:
    """Verify a certificate document before a downstream consumer trusts it."""

    certificate: str

    def __post_init__(self) -> None:
        object.__setattr__(self, "certificate", _path("certificate", self.certificate))

    def to_mcp_arguments(self) -> dict[str, Any]:
        return {"certificate": self.certificate}


@dataclass(frozen=True)
class ProjectionBundleRequest:
    """Request bounded graph/hypergraph/timeline/table projections from one context source."""

    handle: Mapping[str, Any] | None = None
    world: str | None = None
    query: str | None = None
    include_views: bool = False

    def __post_init__(self) -> None:
        normalized_handle, normalized_world, normalized_query = _source_pair(self.handle, self.world, self.query)
        object.__setattr__(self, "handle", normalized_handle)
        object.__setattr__(self, "world", normalized_world)
        object.__setattr__(self, "query", normalized_query)
        if not isinstance(self.include_views, bool):
            raise ArgumentError("include_views must be a boolean")

    def to_mcp_arguments(self) -> dict[str, Any]:
        arguments: dict[str, Any] = {"include_views": self.include_views}
        if self.handle is not None:
            arguments["handle"] = dict(self.handle)
        else:
            arguments["world"] = self.world
            arguments["query"] = self.query
        return arguments


# Context-prefixed aliases make the models discoverable beside the existing Workspace helpers
# while keeping the wire-facing FIBER names available to callers who prefer exact terminology.
ContextCompileRequest = FiberCompileRequest
ContextRefineRequest = FiberRefineRequest
ContextExplainRequest = FiberExplainRequest
ContextVerifyRequest = FiberVerifyRequest


__all__ = [
    "CONTEXT_REQUEST_SCHEMA",
    "MAX_CONTEXT_HANDLE_BYTES",
    "MAX_CONTEXT_PATH_BYTES",
    "ContextCompileRequest",
    "ContextExplainRequest",
    "ContextLayer",
    "ContextRefineRequest",
    "ContextVerifyRequest",
    "FiberCompileRequest",
    "FiberExplainRequest",
    "FiberRefineRequest",
    "FiberVerifyRequest",
    "ProjectionBundleRequest",
]
