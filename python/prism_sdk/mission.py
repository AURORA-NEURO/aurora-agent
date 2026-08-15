"""Typed request builders for bounded cross-domain agent missions.

Rust owns DAG validation, deterministic waves, refusal propagation, and execution budgets. These
models make composing missions ergonomic while deliberately leaving tool semantics and scientific
interpretation to the authoritative MCP server.
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Mapping, Sequence

from .errors import ArgumentError


def _text(name: str, value: str) -> None:
    if not isinstance(value, str) or not value.strip():
        raise ArgumentError(f"{name} must be a non-empty string")


def _mapping(name: str, value: Mapping[str, Any]) -> dict[str, Any]:
    if not isinstance(value, Mapping):
        raise ArgumentError(f"{name} must be a mapping")
    return dict(value)


@dataclass(frozen=True)
class MissionPolicy:
    """Execution opt-in, least-authority, and output-budget policy."""

    execute: bool = False
    stop_on_error: bool = True
    allow_side_effects: bool = False
    max_steps: int = 64
    max_step_output_bytes: int = 2_000_000
    max_total_output_bytes: int = 10_000_000
    allowed_tools: tuple[str, ...] = ()

    def __post_init__(self) -> None:
        for name, value in (
            ("max_steps", self.max_steps),
            ("max_step_output_bytes", self.max_step_output_bytes),
            ("max_total_output_bytes", self.max_total_output_bytes),
        ):
            if not isinstance(value, int) or isinstance(value, bool) or value < 1:
                raise ArgumentError(f"{name} must be a positive integer")
        if self.max_step_output_bytes > self.max_total_output_bytes:
            raise ArgumentError("max_step_output_bytes cannot exceed max_total_output_bytes")
        if not isinstance(self.allowed_tools, Sequence) or isinstance(self.allowed_tools, (str, bytes)):
            raise ArgumentError("allowed_tools must be a sequence of tool names")
        for tool in self.allowed_tools:
            _text("allowed_tools entry", tool)

    def to_dict(self) -> dict[str, Any]:
        return {
            "execute": self.execute,
            "stop_on_error": self.stop_on_error,
            "allow_side_effects": self.allow_side_effects,
            "max_steps": self.max_steps,
            "max_step_output_bytes": self.max_step_output_bytes,
            "max_total_output_bytes": self.max_total_output_bytes,
            "allowed_tools": list(self.allowed_tools),
        }


@dataclass(frozen=True)
class MissionBinding:
    """Copy a JSON-pointer value from a direct prerequisite into a step argument slot."""

    from_step: str
    source_pointer: str
    target_pointer: str

    def __post_init__(self) -> None:
        _text("binding.from_step", self.from_step)
        if not isinstance(self.source_pointer, str) or (
            self.source_pointer and not self.source_pointer.startswith("/")
        ):
            raise ArgumentError("binding.source_pointer must be empty or an RFC 6901 pointer")
        if not isinstance(self.target_pointer, str) or not self.target_pointer.startswith("/"):
            raise ArgumentError("binding.target_pointer must be an RFC 6901 pointer")

    def to_dict(self) -> dict[str, str]:
        return {
            "from_step": self.from_step,
            "source_pointer": self.source_pointer,
            "target_pointer": self.target_pointer,
        }


def _binding(value: MissionBinding | Mapping[str, Any]) -> dict[str, str]:
    if isinstance(value, MissionBinding):
        return value.to_dict()
    raw = _mapping("mission binding", value)
    for name in ("from_step", "source_pointer", "target_pointer"):
        if name not in raw:
            raise ArgumentError(f"mission binding requires {name}")
    return MissionBinding(
        from_step=raw["from_step"],
        source_pointer=raw["source_pointer"],
        target_pointer=raw["target_pointer"],
    ).to_dict()


@dataclass(frozen=True)
class MissionStep:
    """One domain-labelled tool call in a mission dependency graph."""

    id: str
    domain: str
    capability: str
    objective: str
    tool: str
    arguments: Mapping[str, Any] | None = None
    depends_on: tuple[str, ...] = ()
    required: bool = True
    bindings: Sequence[MissionBinding | Mapping[str, Any]] = ()

    def __post_init__(self) -> None:
        for name in ("id", "domain", "capability", "objective", "tool"):
            _text(name, getattr(self, name))
        _mapping("arguments", {} if self.arguments is None else self.arguments)
        if not isinstance(self.depends_on, Sequence) or isinstance(self.depends_on, (str, bytes)):
            raise ArgumentError("depends_on must be a sequence of step ids")
        for dependency in self.depends_on:
            _text("depends_on entry", dependency)
        if not isinstance(self.bindings, Sequence) or isinstance(self.bindings, (str, bytes)):
            raise ArgumentError("bindings must be a sequence")
        for binding in self.bindings:
            _binding(binding)

    def to_dict(self) -> dict[str, Any]:
        return {
            "id": self.id,
            "domain": self.domain,
            "capability": self.capability,
            "objective": self.objective,
            "tool": self.tool,
            "arguments": _mapping("arguments", {} if self.arguments is None else self.arguments),
            "depends_on": list(self.depends_on),
            "required": self.required,
            "bindings": [_binding(value) for value in self.bindings],
        }


def _step(value: MissionStep | Mapping[str, Any]) -> dict[str, Any]:
    if isinstance(value, MissionStep):
        return value.to_dict()
    raw = _mapping("mission step", value)
    if "arguments" in raw:
        raw["arguments"] = _mapping("step arguments", raw["arguments"])
    if "depends_on" in raw and (
        not isinstance(raw["depends_on"], Sequence)
        or isinstance(raw["depends_on"], (str, bytes))
    ):
        raise ArgumentError("step depends_on must be a sequence")
    if "bindings" in raw:
        if not isinstance(raw["bindings"], Sequence) or isinstance(raw["bindings"], (str, bytes)):
            raise ArgumentError("step bindings must be a sequence")
        raw["bindings"] = [_binding(value) for value in raw["bindings"]]
    return raw


@dataclass(frozen=True)
class MissionRequest:
    """Build a previewable or explicitly executable cross-domain mission."""

    mission_id: str
    goal: str
    steps: Sequence[MissionStep | Mapping[str, Any]]
    policy: MissionPolicy | Mapping[str, Any] | None = None

    def __post_init__(self) -> None:
        _text("mission_id", self.mission_id)
        _text("goal", self.goal)
        if not isinstance(self.steps, Sequence) or isinstance(self.steps, (str, bytes)) or not self.steps:
            raise ArgumentError("steps must be a non-empty sequence")
        for value in self.steps:
            _step(value)
        if self.policy is not None and not isinstance(self.policy, (MissionPolicy, Mapping)):
            raise ArgumentError("policy must be a MissionPolicy or mapping")

    def to_mcp_arguments(self) -> dict[str, Any]:
        arguments: dict[str, Any] = {
            "mission_id": self.mission_id,
            "goal": self.goal,
            "steps": [_step(value) for value in self.steps],
        }
        if self.policy is not None:
            arguments["policy"] = (
                self.policy.to_dict()
                if isinstance(self.policy, MissionPolicy)
                else _mapping("policy", self.policy)
            )
        return arguments


__all__ = ["MissionBinding", "MissionPolicy", "MissionRequest", "MissionStep"]
