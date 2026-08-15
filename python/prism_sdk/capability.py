"""Typed builders for cross-domain capability discovery."""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any, Mapping, Sequence

from .errors import ArgumentError


def _optional_text(name: str, value: str | None) -> None:
    if value is not None and (not isinstance(value, str) or not value.strip()):
        raise ArgumentError(f"{name} must be a non-empty string when supplied")


@dataclass(frozen=True)
class CapabilityQuery:
    """Conjunctive filters for the digest-bound workspace capability catalogue."""

    query: str | None = None
    group_id: str | None = None
    domain: str | None = None
    tool: str | None = None
    max_items: int = 50
    include_tools: bool = False

    def __post_init__(self) -> None:
        for name, value in (
            ("query", self.query),
            ("group_id", self.group_id),
            ("domain", self.domain),
            ("tool", self.tool),
        ):
            _optional_text(name, value)
        if (
            not isinstance(self.max_items, int)
            or isinstance(self.max_items, bool)
            or not 1 <= self.max_items <= 500
        ):
            raise ArgumentError("max_items must be between 1 and 500")
        if not isinstance(self.include_tools, bool):
            raise ArgumentError("include_tools must be a boolean")

    def to_mcp_arguments(self) -> dict[str, Any]:
        arguments: dict[str, Any] = {
            "max_items": self.max_items,
            "include_tools": self.include_tools,
        }
        for name in ("query", "group_id", "domain", "tool"):
            value = getattr(self, name)
            if value is not None:
                arguments[name] = value
        return arguments


@dataclass(frozen=True)
class CapabilityRouteNeed:
    """One named requirement in a batched cross-domain route."""

    id: str
    query: CapabilityQuery = field(default_factory=CapabilityQuery)

    def __post_init__(self) -> None:
        _optional_text("need.id", self.id)
        if not isinstance(self.query, CapabilityQuery):
            raise ArgumentError("need.query must be a CapabilityQuery")
        if self.query.include_tools:
            raise ArgumentError("nested need queries cannot request tool schemas")

    def to_mcp_arguments(self) -> dict[str, Any]:
        return {"id": self.id, **self.query.to_mcp_arguments()}


def _route_need(value: CapabilityRouteNeed | Mapping[str, Any]) -> CapabilityRouteNeed:
    if isinstance(value, CapabilityRouteNeed):
        return value
    if not isinstance(value, Mapping):
        raise ArgumentError("route need must be a CapabilityRouteNeed or mapping")
    raw = dict(value)
    if "id" not in raw:
        raise ArgumentError("route need requires id")
    return CapabilityRouteNeed(
        id=raw["id"],
        query=CapabilityQuery(
            query=raw.get("query"),
            group_id=raw.get("group_id"),
            domain=raw.get("domain"),
            tool=raw.get("tool"),
            max_items=raw.get("max_items", 50),
            include_tools=raw.get("include_tools", False),
        ),
    )


@dataclass(frozen=True)
class CapabilityRouteRequest:
    """Bounded multi-need routing that never executes the returned candidates."""

    goal: str
    needs: Sequence[CapabilityRouteNeed | Mapping[str, Any]]
    max_candidates_per_need: int = 10
    max_tools: int = 128
    include_tools: bool = False

    def __post_init__(self) -> None:
        _optional_text("goal", self.goal)
        if (
            not isinstance(self.needs, Sequence)
            or isinstance(self.needs, (str, bytes))
            or not self.needs
            or len(self.needs) > 32
        ):
            raise ArgumentError("needs must contain between 1 and 32 named requirements")
        ids: set[str] = set()
        for value in self.needs:
            need = _route_need(value)
            if need.id in ids:
                raise ArgumentError(f"duplicate route need id: {need.id}")
            ids.add(need.id)
        for name, value, maximum in (
            ("max_candidates_per_need", self.max_candidates_per_need, 50),
            ("max_tools", self.max_tools, 256),
        ):
            if not isinstance(value, int) or isinstance(value, bool) or not 1 <= value <= maximum:
                raise ArgumentError(f"{name} must be between 1 and {maximum}")
        if not isinstance(self.include_tools, bool):
            raise ArgumentError("include_tools must be a boolean")

    def to_mcp_arguments(self) -> dict[str, Any]:
        return {
            "goal": self.goal,
            "needs": [_route_need(value).to_mcp_arguments() for value in self.needs],
            "max_candidates_per_need": self.max_candidates_per_need,
            "max_tools": self.max_tools,
            "include_tools": self.include_tools,
        }


__all__ = ["CapabilityQuery", "CapabilityRouteNeed", "CapabilityRouteRequest"]
