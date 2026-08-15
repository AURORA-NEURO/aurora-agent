"""Typed transport request for the Rust developer workbench.

The Rust workbench owns session validation, dependency ordering, stale detection, dashboard
projection, and CI YAML generation. This facade validates only that the nested wire objects are
JSON mappings and preserves them unchanged.
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Mapping

from .errors import ArgumentError


def _mapping(name: str, value: Mapping[str, Any] | None) -> dict[str, Any] | None:
    if value is None:
        return None
    if not isinstance(value, Mapping):
        raise ArgumentError(f"{name} must be a mapping")
    return dict(value)


@dataclass(frozen=True)
class WorkbenchRequest:
    """Compose authoring-session audit, dashboard query, and optional CI planning."""

    session: Mapping[str, Any]
    dashboard: Mapping[str, Any] | None = None
    ci: Mapping[str, Any] | None = None

    def __post_init__(self) -> None:
        if not isinstance(self.session, Mapping):
            raise ArgumentError("session must be a mapping")
        if not self.session:
            raise ArgumentError("session must not be empty")
        _mapping("dashboard", self.dashboard)
        _mapping("ci", self.ci)

    def to_mcp_arguments(self) -> dict[str, Any]:
        arguments: dict[str, Any] = {"session": dict(self.session)}
        dashboard = _mapping("dashboard", self.dashboard)
        ci = _mapping("ci", self.ci)
        if dashboard is not None:
            arguments["dashboard"] = dashboard
        if ci is not None:
            arguments["ci"] = ci
        return arguments


__all__ = ["WorkbenchRequest"]
