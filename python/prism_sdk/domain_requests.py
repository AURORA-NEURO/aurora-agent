"""Bounded request models for the world, inference-lab, and routing workflows.

These models validate envelope shape, sizes, and explicit option bounds without reimplementing
Rust's world provenance, obligation graph, acquisition, or routing semantics. The server remains
authoritative for support, reachability, privacy, abstention, and safe-default decisions.
"""

from __future__ import annotations

from dataclasses import dataclass
import math
from typing import Any, Mapping, Sequence

from .authoring import canonical_json
from .errors import ArgumentError


MAX_DOMAIN_REQUEST_BYTES = 20_000_000
MAX_LAB_ACTIONS = 1_000
MAX_LAB_ITEMS = 1_000
MAX_ROUTING_EVIDENCE = 20_000


def _mapping(name: str, value: Mapping[str, Any], *, maximum: int = MAX_DOMAIN_REQUEST_BYTES) -> dict[str, Any]:
    if not isinstance(value, Mapping):
        raise ArgumentError(f"{name} must be a mapping")
    result = dict(value)
    try:
        encoded = canonical_json(result).encode("utf-8")
    except (TypeError, ValueError) as error:
        raise ArgumentError(f"{name} is not canonical JSON-safe: {error}") from error
    if len(encoded) > maximum:
        raise ArgumentError(f"{name} exceeds the {maximum}-byte limit")
    return result


def _mapping_sequence(name: str, value: Sequence[Mapping[str, Any]], maximum: int) -> tuple[dict[str, Any], ...]:
    if isinstance(value, (str, bytes)) or not isinstance(value, Sequence):
        raise ArgumentError(f"{name} must be a sequence")
    if len(value) > maximum:
        raise ArgumentError(f"{name} may contain at most {maximum} items")
    result: list[dict[str, Any]] = []
    for index, item in enumerate(value):
        if not isinstance(item, Mapping):
            raise ArgumentError(f"{name}[{index}] must be a mapping")
        result.append(dict(item))
    try:
        encoded = canonical_json(result).encode("utf-8")
    except (TypeError, ValueError) as error:
        raise ArgumentError(f"{name} is not canonical JSON-safe: {error}") from error
    if len(encoded) > MAX_DOMAIN_REQUEST_BYTES:
        raise ArgumentError(f"{name} exceeds the {MAX_DOMAIN_REQUEST_BYTES}-byte limit")
    return tuple(result)


def _optional_mapping(name: str, value: Mapping[str, Any] | None) -> dict[str, Any] | None:
    return None if value is None else _mapping(name, value)


def _text(name: str, value: str, *, maximum: int = 512) -> str:
    if not isinstance(value, str) or not value.strip():
        raise ArgumentError(f"{name} must be a non-empty string")
    if len(value.encode("utf-8")) > maximum:
        raise ArgumentError(f"{name} exceeds the {maximum}-byte limit")
    return value


@dataclass(frozen=True)
class WorldClaimCheckRequest:
    """Serialized world provenance and claim for a fail-closed support check."""

    provenance: Mapping[str, Any]
    claim: Mapping[str, Any]

    def __post_init__(self) -> None:
        object.__setattr__(self, "provenance", _mapping("provenance", self.provenance))
        object.__setattr__(self, "claim", _mapping("claim", self.claim))

    def to_mcp_arguments(self) -> dict[str, Any]:
        return {"provenance": dict(self.provenance), "claim": dict(self.claim)}


@dataclass(frozen=True)
class LabPlanRequest:
    """Serialized obligation graph, acquisition actions, and bounded lab budget."""

    graph: Mapping[str, Any]
    actions: Sequence[Mapping[str, Any]]
    budget: Mapping[str, Any]
    marginal_value_floor: float = 0.0
    hypotheses: Mapping[str, Any] | None = None
    observations: Mapping[str, Any] | None = None
    max_items: int = 100

    def __post_init__(self) -> None:
        object.__setattr__(self, "graph", _mapping("graph", self.graph))
        object.__setattr__(self, "actions", _mapping_sequence("actions", self.actions, MAX_LAB_ACTIONS))
        object.__setattr__(self, "budget", _mapping("budget", self.budget))
        if isinstance(self.marginal_value_floor, bool) or not isinstance(self.marginal_value_floor, (int, float)):
            raise ArgumentError("marginal_value_floor must be a finite non-negative number")
        if not math.isfinite(float(self.marginal_value_floor)) or self.marginal_value_floor < 0:
            raise ArgumentError("marginal_value_floor must be a finite non-negative number")
        object.__setattr__(self, "marginal_value_floor", float(self.marginal_value_floor))
        object.__setattr__(self, "hypotheses", _optional_mapping("hypotheses", self.hypotheses))
        object.__setattr__(self, "observations", _optional_mapping("observations", self.observations))
        if isinstance(self.max_items, bool) or not isinstance(self.max_items, int) or not 1 <= self.max_items <= MAX_LAB_ITEMS:
            raise ArgumentError(f"max_items must be between 1 and {MAX_LAB_ITEMS}")

    def to_mcp_arguments(self) -> dict[str, Any]:
        arguments: dict[str, Any] = {
            "graph": dict(self.graph),
            "actions": [dict(action) for action in self.actions],
            "budget": dict(self.budget),
            "marginal_value_floor": self.marginal_value_floor,
            "max_items": self.max_items,
        }
        if self.hypotheses is not None:
            arguments["hypotheses"] = dict(self.hypotheses)
        if self.observations is not None:
            arguments["observations"] = dict(self.observations)
        return arguments


@dataclass(frozen=True)
class RoutingDecisionRequest:
    """Unseen-task fingerprint, approved evidence ledger, and routing policy."""

    fingerprint: Mapping[str, Any]
    evidence: Sequence[Mapping[str, Any]]
    policy: Mapping[str, Any]
    task_id: str | None = None

    def __post_init__(self) -> None:
        object.__setattr__(self, "fingerprint", _mapping("fingerprint", self.fingerprint))
        object.__setattr__(self, "evidence", _mapping_sequence("evidence", self.evidence, MAX_ROUTING_EVIDENCE))
        object.__setattr__(self, "policy", _mapping("policy", self.policy))
        if self.task_id is not None:
            object.__setattr__(self, "task_id", _text("task_id", self.task_id))

    def to_mcp_arguments(self) -> dict[str, Any]:
        arguments: dict[str, Any] = {
            "fingerprint": dict(self.fingerprint),
            "evidence": [dict(item) for item in self.evidence],
            "policy": dict(self.policy),
        }
        if self.task_id is not None:
            arguments["task_id"] = self.task_id
        return arguments


__all__ = [
    "MAX_DOMAIN_REQUEST_BYTES",
    "MAX_LAB_ACTIONS",
    "MAX_LAB_ITEMS",
    "MAX_ROUTING_EVIDENCE",
    "LabPlanRequest",
    "RoutingDecisionRequest",
    "WorldClaimCheckRequest",
]
