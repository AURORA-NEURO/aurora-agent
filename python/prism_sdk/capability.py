"""Typed builders for cross-domain capability discovery."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any

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


__all__ = ["CapabilityQuery"]
