"""Typed request builders for bounded cross-domain agent missions.

Rust owns DAG validation, deterministic waves, refusal propagation, and execution budgets. These
models make composing missions ergonomic while deliberately leaving tool semantics and scientific
interpretation to the authoritative MCP server.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any, Mapping, Sequence

from .authoring import content_digest
from .errors import ArgumentError
from .tooling import ToolCatalogue, ToolSchemaError, ToolValidationReport


MAX_MISSION_STEPS = 128
MAX_ALLOWED_TOOLS = 512
MAX_STEP_OUTPUT_BYTES = 20_000_000
MAX_TOTAL_OUTPUT_BYTES = 20_000_000
MAX_PARALLEL_WAVE_WIDTH = 16
MISSION_ASSEMBLY_SCHEMA = "bioprism-python-mission-assembly/0.1"


def _text(name: str, value: str) -> None:
    if not isinstance(value, str) or not value.strip():
        raise ArgumentError(f"{name} must be a non-empty string")


def _mapping(name: str, value: Mapping[str, Any]) -> dict[str, Any]:
    if not isinstance(value, Mapping):
        raise ArgumentError(f"{name} must be a mapping")
    return dict(value)


def _valid_pointer(pointer: str, *, allow_empty: bool) -> bool:
    if pointer == "":
        return allow_empty
    if not isinstance(pointer, str) or not pointer.startswith("/"):
        return False
    if any(ord(character) < 32 for character in pointer):
        return False
    index = 0
    while index < len(pointer):
        if pointer[index] == "~":
            if index + 1 >= len(pointer) or pointer[index + 1] not in "01":
                return False
            index += 2
        else:
            index += 1
    return True


@dataclass(frozen=True)
class MissionPolicy:
    """Execution opt-in, least-authority, and output-budget policy."""

    execute: bool = False
    stop_on_error: bool = True
    allow_side_effects: bool = False
    max_steps: int = 64
    max_step_output_bytes: int = 2_000_000
    max_total_output_bytes: int = 10_000_000
    execution_mode: str = "serial"
    allowed_tools: tuple[str, ...] = ()

    def __post_init__(self) -> None:
        for name, value in (
            ("execute", self.execute),
            ("stop_on_error", self.stop_on_error),
            ("allow_side_effects", self.allow_side_effects),
        ):
            if not isinstance(value, bool):
                raise ArgumentError(f"{name} must be a boolean")
        for name, value in (
            ("max_steps", self.max_steps),
            ("max_step_output_bytes", self.max_step_output_bytes),
            ("max_total_output_bytes", self.max_total_output_bytes),
        ):
            maximum = {
                "max_steps": MAX_MISSION_STEPS,
                "max_step_output_bytes": MAX_STEP_OUTPUT_BYTES,
                "max_total_output_bytes": MAX_TOTAL_OUTPUT_BYTES,
            }[name]
            if not isinstance(value, int) or isinstance(value, bool) or not 1 <= value <= maximum:
                raise ArgumentError(f"{name} must be between 1 and {maximum}")
        if self.max_step_output_bytes > self.max_total_output_bytes:
            raise ArgumentError("max_step_output_bytes cannot exceed max_total_output_bytes")
        if self.execution_mode not in ("serial", "parallel_waves"):
            raise ArgumentError("execution_mode must be serial or parallel_waves")
        if not isinstance(self.allowed_tools, Sequence) or isinstance(self.allowed_tools, (str, bytes)):
            raise ArgumentError("allowed_tools must be a sequence of tool names")
        if len(self.allowed_tools) > MAX_ALLOWED_TOOLS:
            raise ArgumentError(f"allowed_tools may contain at most {MAX_ALLOWED_TOOLS} items")
        seen: set[str] = set()
        for tool in self.allowed_tools:
            _text("allowed_tools entry", tool)
            if not all(character.isalnum() or character == "_" for character in tool):
                raise ArgumentError(f"allowed_tools entry is not a safe tool name: {tool}")
            if tool == "agent_mission":
                raise ArgumentError("agent_mission cannot invoke itself")
            if tool in seen:
                raise ArgumentError(f"duplicate allowed tool: {tool}")
            seen.add(tool)
        if self.execute and not self.allowed_tools:
            raise ArgumentError("execute requires a non-empty allowed_tools list")

    def to_dict(self) -> dict[str, Any]:
        return {
            "execute": self.execute,
            "stop_on_error": self.stop_on_error,
            "allow_side_effects": self.allow_side_effects,
            "max_steps": self.max_steps,
            "max_step_output_bytes": self.max_step_output_bytes,
            "max_total_output_bytes": self.max_total_output_bytes,
            "execution_mode": self.execution_mode,
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
        if not isinstance(self.source_pointer, str) or not _valid_pointer(self.source_pointer, allow_empty=True):
            raise ArgumentError("binding.source_pointer must be empty or an RFC 6901 pointer")
        if not isinstance(self.target_pointer, str) or not _valid_pointer(self.target_pointer, allow_empty=False):
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
        if not isinstance(self.required, bool):
            raise ArgumentError("required must be a boolean")

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
    for name in ("id", "domain", "capability", "objective", "tool"):
        if name not in raw:
            raise ArgumentError(f"mission step requires {name}")
        _text(f"step.{name}", raw[name])
    if "arguments" in raw:
        raw["arguments"] = _mapping("step arguments", raw["arguments"])
    else:
        raw["arguments"] = {}
    if "depends_on" in raw and (
        not isinstance(raw["depends_on"], Sequence)
        or isinstance(raw["depends_on"], (str, bytes))
    ):
        raise ArgumentError("step depends_on must be a sequence")
    if "bindings" in raw:
        if not isinstance(raw["bindings"], Sequence) or isinstance(raw["bindings"], (str, bytes)):
            raise ArgumentError("step bindings must be a sequence")
        raw["bindings"] = [_binding(value) for value in raw["bindings"]]
    else:
        raw["bindings"] = []
    if "required" in raw and not isinstance(raw["required"], bool):
        raise ArgumentError("step required must be a boolean")
    raw.setdefault("required", True)
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


@dataclass(frozen=True)
class MissionRouteSelection:
    """One explicit, caller-reviewed choice that turns a routed need into a mission step."""

    need_id: str
    tool: str
    domain: str
    capability: str
    objective: str
    arguments: Mapping[str, Any]
    depends_on: Sequence[str] = ()
    required: bool = True
    bindings: Sequence[MissionBinding | Mapping[str, Any]] = ()

    def __post_init__(self) -> None:
        for name in ("need_id", "tool", "domain", "capability", "objective"):
            _text(f"route selection.{name}", getattr(self, name))
        _mapping("route selection.arguments", self.arguments)
        if not isinstance(self.depends_on, Sequence) or isinstance(self.depends_on, (str, bytes)):
            raise ArgumentError("route selection.depends_on must be a sequence")
        for dependency in self.depends_on:
            _text("route selection dependency", dependency)
        if not isinstance(self.required, bool):
            raise ArgumentError("route selection.required must be a boolean")
        if not isinstance(self.bindings, Sequence) or isinstance(self.bindings, (str, bytes)):
            raise ArgumentError("route selection.bindings must be a sequence")
        for binding in self.bindings:
            _binding(binding)

    def to_step(self) -> MissionStep:
        """Convert the reviewed selection into the existing typed mission-step contract."""

        return MissionStep(
            id=self.need_id,
            domain=self.domain,
            capability=self.capability,
            objective=self.objective,
            tool=self.tool,
            arguments=self.arguments,
            depends_on=tuple(self.depends_on),
            required=self.required,
            bindings=self.bindings,
        )


def _route_selection(value: MissionRouteSelection | Mapping[str, Any]) -> MissionRouteSelection:
    if isinstance(value, MissionRouteSelection):
        return value
    raw = _mapping("route selection", value)
    for name in ("need_id", "tool", "domain", "capability", "objective", "arguments"):
        if name not in raw:
            raise ArgumentError(f"route selection requires {name}")
    return MissionRouteSelection(
        need_id=raw["need_id"],
        tool=raw["tool"],
        domain=raw["domain"],
        capability=raw["capability"],
        objective=raw["objective"],
        arguments=raw["arguments"],
        depends_on=raw.get("depends_on", ()),
        required=raw.get("required", True),
        bindings=raw.get("bindings", ()),
    )


@dataclass(frozen=True)
class MissionAssembly:
    """A route-bound mission draft whose candidate choices and provenance remain inspectable."""

    route_id: str
    catalog_digest: str
    request: MissionRequest
    selected_tools: tuple[str, ...] = field(default_factory=tuple)

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": MISSION_ASSEMBLY_SCHEMA,
            "route_id": self.route_id,
            "catalog_digest": self.catalog_digest,
            "mission": self.request.to_mcp_arguments(),
            "selected_tools": list(self.selected_tools),
            "limitations": [
                "tool and argument choices are caller-selected; routing scores do not authorize execution",
                "the route catalogue digest is provenance, not a guarantee that the live catalogue is unchanged",
                "mission graph and per-tool schema validity still require mission_preflight",
            ],
        }


def mission_from_route(
    route: Mapping[str, Any],
    mission_id: str,
    selections: Sequence[MissionRouteSelection | Mapping[str, Any]],
    *,
    policy: MissionPolicy | Mapping[str, Any] | None = None,
) -> MissionAssembly:
    """Build an explicit mission only from tools selected out of one capability-route result.

    This function never executes a routed candidate and never invents tool arguments. Every route
    need must have exactly one caller-supplied selection whose tool is present in that need's
    bounded candidate list; the returned request can then be reviewed with ``preflight_mission``.
    """

    if not isinstance(route, Mapping):
        raise ArgumentError("route must be a mapping")
    if route.get("workflow") != "capability_route":
        raise ArgumentError("route.workflow must be capability_route")
    route_id = route.get("route_id")
    catalog_digest = route.get("catalog_digest")
    goal = route.get("goal")
    _text("route_id", route_id)
    _text("catalog_digest", catalog_digest)
    _text("route.goal", goal)
    raw_needs = route.get("needs")
    if (
        not isinstance(raw_needs, Sequence)
        or isinstance(raw_needs, (str, bytes))
        or not raw_needs
        or len(raw_needs) > MAX_MISSION_STEPS
    ):
        raise ArgumentError("route.needs must contain between 1 and 128 needs")
    unresolved = route.get("unresolved_needs", ())
    if not isinstance(unresolved, Sequence) or isinstance(unresolved, (str, bytes)):
        raise ArgumentError("route.unresolved_needs must be a sequence")
    if unresolved:
        raise ArgumentError(f"route contains unresolved needs: {list(unresolved)!r}")

    candidates_by_need: dict[str, tuple[str, ...]] = {}
    ordered_need_ids: list[str] = []
    for raw_need in raw_needs:
        if not isinstance(raw_need, Mapping):
            raise ArgumentError("route.needs entries must be mappings")
        need_id = raw_need.get("id")
        _text("route need.id", need_id)
        if need_id in candidates_by_need:
            raise ArgumentError(f"route contains duplicate need id: {need_id}")
        raw_candidates = raw_need.get("candidate_tools")
        if not isinstance(raw_candidates, Sequence) or isinstance(raw_candidates, (str, bytes)):
            raise ArgumentError(f"route need {need_id!r} has no candidate_tools array")
        candidates: list[str] = []
        for candidate in raw_candidates:
            _text("route candidate tool", candidate)
            if candidate not in candidates:
                candidates.append(candidate)
        if not candidates:
            raise ArgumentError(f"route need {need_id!r} is unresolved")
        candidates_by_need[need_id] = tuple(candidates)
        ordered_need_ids.append(need_id)

    if (
        not isinstance(selections, Sequence)
        or isinstance(selections, (str, bytes))
        or len(selections) != len(ordered_need_ids)
    ):
        raise ArgumentError("selections must contain exactly one choice for every routed need")
    selected_by_need: dict[str, MissionRouteSelection] = {}
    for value in selections:
        selection = _route_selection(value)
        if selection.need_id in selected_by_need:
            raise ArgumentError(f"duplicate route selection for need: {selection.need_id}")
        candidates = candidates_by_need.get(selection.need_id)
        if candidates is None:
            raise ArgumentError(f"selection refers to unknown route need: {selection.need_id}")
        if selection.tool not in candidates:
            raise ArgumentError(
                f"tool {selection.tool!r} is not a candidate for route need {selection.need_id!r}"
            )
        for dependency in selection.depends_on:
            if dependency not in candidates_by_need:
                raise ArgumentError(
                    f"route selection {selection.need_id!r} depends on unknown need: {dependency}"
                )
        selected_by_need[selection.need_id] = selection
    missing = [need_id for need_id in ordered_need_ids if need_id not in selected_by_need]
    if missing:
        raise ArgumentError(f"route needs have no explicit selection: {missing!r}")

    request = MissionRequest(
        mission_id,
        goal,
        [selected_by_need[need_id].to_step() for need_id in ordered_need_ids],
        policy,
    )
    return MissionAssembly(
        route_id=route_id,
        catalog_digest=catalog_digest,
        request=request,
        selected_tools=tuple(selected_by_need[need_id].tool for need_id in ordered_need_ids),
    )


class MissionPreflightError(ArgumentError):
    """A mission cannot be safely submitted after local graph/schema preflight."""


@dataclass(frozen=True)
class MissionStepPreflight:
    """Transport and graph findings for one mission step; no tool has executed."""

    id: str
    tool: str
    depends_on: tuple[str, ...]
    wave: int | None
    status: str
    schema: ToolValidationReport | None
    issues: tuple[str, ...] = ()
    warnings: tuple[str, ...] = ()

    @property
    def ok(self) -> bool:
        return self.status == "ready" and not self.issues and (self.schema is None or self.schema.ok)

    def to_dict(self) -> dict[str, Any]:
        schema: dict[str, Any] | None = None
        if self.schema is not None:
            schema = {
                "tool": self.schema.tool,
                "schema_digest": self.schema.schema_digest,
                "ok": self.schema.ok,
                "fully_checked": self.schema.fully_checked,
                "issues": [
                    {"path": issue.path, "code": issue.code, "message": issue.message}
                    for issue in self.schema.issues
                ],
                "warnings": [
                    {"path": issue.path, "code": issue.code, "message": issue.message}
                    for issue in self.schema.warnings
                ],
            }
        return {
            "id": self.id,
            "tool": self.tool,
            "depends_on": list(self.depends_on),
            "wave": self.wave,
            "status": self.status,
            "schema": schema,
            "issues": list(self.issues),
            "warnings": list(self.warnings),
        }


@dataclass(frozen=True)
class MissionPreflight:
    """A deterministic, no-side-effect review of a cross-domain mission request."""

    mission_id: str
    goal: str
    request_digest: str
    catalogue_digest: str
    execution: str
    execution_mode: str
    waves: tuple[tuple[str, ...], ...]
    steps: tuple[MissionStepPreflight, ...]
    issues: tuple[str, ...] = ()
    warnings: tuple[str, ...] = ()

    @property
    def ok(self) -> bool:
        return not self.issues and all(step.ok for step in self.steps)

    @property
    def fully_checked(self) -> bool:
        return self.ok and not self.warnings and all(
            step.schema is None or step.schema.fully_checked for step in self.steps
        )

    @property
    def ordered_steps(self) -> tuple[str, ...]:
        return tuple(step_id for wave in self.waves for step_id in wave)

    def raise_if_invalid(self) -> None:
        if not self.ok:
            details = "; ".join(self.issues)
            if not details:
                details = "; ".join(
                    f"{step.id}: {issue}" for step in self.steps for issue in step.issues
                )
            raise MissionPreflightError(f"mission {self.mission_id!r} failed preflight: {details}")

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": "bioprism-python-mission-preflight/0.1",
            "mission_id": self.mission_id,
            "goal": self.goal,
            "request_digest": self.request_digest,
            "catalogue_digest": self.catalogue_digest,
            "execution": self.execution,
            "execution_mode": self.execution_mode,
            "ok": self.ok,
            "fully_checked": self.fully_checked,
            "ordered_steps": list(self.ordered_steps),
            "waves": [list(wave) for wave in self.waves],
            "issues": list(self.issues),
            "warnings": list(self.warnings),
            "steps": [step.to_dict() for step in self.steps],
            "limitations": [
                "preflight checks transport shape and mission graph invariants only",
                "the remote MCP server remains authoritative for domain semantics and refusal results",
                "no step is executed by this report",
            ],
        }


def preflight_mission(request: MissionRequest, catalogue: ToolCatalogue) -> MissionPreflight:
    """Review a mission against a live schema snapshot without dispatching any tool."""

    if not isinstance(request, MissionRequest):
        raise ArgumentError("request must be a MissionRequest")
    if not isinstance(catalogue, ToolCatalogue):
        raise ArgumentError("catalogue must be a ToolCatalogue")
    arguments = request.to_mcp_arguments()
    request_digest = content_digest(arguments)
    issues: list[str] = []
    warnings: list[str] = []
    try:
        policy = _mission_policy(request.policy)
    except (ArgumentError, TypeError, ValueError) as error:
        policy = MissionPolicy()
        issues.append(f"policy: {error}")

    raw_steps = [_step(value) for value in request.steps]
    if len(raw_steps) > MAX_MISSION_STEPS:
        issues.append(f"mission has {len(raw_steps)} steps; maximum is {MAX_MISSION_STEPS}")
    if len(raw_steps) > policy.max_steps:
        issues.append(f"mission has {len(raw_steps)} steps; policy.max_steps is {policy.max_steps}")
    if policy.execute and not policy.allowed_tools:
        issues.append("execution requires a non-empty explicit allowed_tools list")
    allowed = set(policy.allowed_tools)
    step_issues: dict[str, list[str]] = {}
    step_warnings: dict[str, list[str]] = {}
    by_id: dict[str, dict[str, Any]] = {}
    for raw in raw_steps:
        step_id = raw["id"]
        if step_id in by_id:
            message = f"duplicate mission step id: {step_id}"
            issues.append(message)
            step_issues.setdefault(step_id, []).append(message)
        else:
            by_id[step_id] = raw

    dependencies: dict[str, set[str]] = {}
    for raw in raw_steps:
        step_id = raw["id"]
        local = step_issues.setdefault(step_id, [])
        seen_dependencies: set[str] = set()
        dependencies[step_id] = set()
        for dependency in raw.get("depends_on", []):
            if dependency == step_id:
                message = "step depends on itself"
                issues.append(f"{step_id}: {message}")
                local.append(message)
            elif dependency in seen_dependencies:
                message = f"duplicate dependency: {dependency}"
                issues.append(f"{step_id}: {message}")
                local.append(message)
            elif dependency not in by_id:
                message = f"unknown dependency: {dependency}"
                issues.append(f"{step_id}: {message}")
                local.append(message)
            else:
                seen_dependencies.add(dependency)
                dependencies[step_id].add(dependency)

        binding_targets: set[str] = set()
        for binding in raw.get("bindings", []):
            source = binding["from_step"]
            target = binding["target_pointer"]
            if source not in by_id:
                message = f"binding source is unknown: {source}"
                issues.append(f"{step_id}: {message}")
                local.append(message)
            elif source not in dependencies[step_id]:
                message = f"binding source is not a direct dependency: {source}"
                issues.append(f"{step_id}: {message}")
                local.append(message)
            if target in binding_targets:
                message = f"duplicate binding target: {target}"
                issues.append(f"{step_id}: {message}")
                local.append(message)
            binding_targets.add(target)
            if not _pointer_exists(raw.get("arguments", {}), target):
                message = f"binding target does not exist: {target}"
                issues.append(f"{step_id}: {message}")
                local.append(message)

        tool = raw["tool"]
        if tool == "agent_mission":
            message = "agent_mission cannot invoke itself"
            issues.append(f"{step_id}: {message}")
            local.append(message)
        if policy.execute and tool not in allowed:
            message = f"tool is not allow-listed: {tool}"
            issues.append(f"{step_id}: {message}")
            local.append(message)
        if policy.execute and not policy.allow_side_effects and _contains_confirmation(raw.get("arguments", {})):
            message = "confirmation flag is present while side effects are disabled"
            issues.append(f"{step_id}: {message}")
            local.append(message)

    waves: list[tuple[str, ...]] = []
    remaining = {step_id: set(values) for step_id, values in dependencies.items()}
    while remaining:
        ready = tuple(sorted(step_id for step_id, values in remaining.items() if not values))
        if not ready:
            cycle = tuple(sorted(remaining))
            message = f"dependency cycle contains: {', '.join(cycle)}"
            issues.append(message)
            for step_id in cycle:
                step_issues.setdefault(step_id, []).append(message)
            break
        waves.append(ready)
        for step_id in ready:
            remaining.pop(step_id, None)
        for values in remaining.values():
            values.difference_update(ready)
    if policy.execution_mode == "parallel_waves":
        max_width = max((len(wave) for wave in waves), default=0)
        if max_width > MAX_PARALLEL_WAVE_WIDTH:
            issues.append(
                f"parallel_waves supports at most {MAX_PARALLEL_WAVE_WIDTH} steps in one wave; got {max_width}"
            )
        required_budget = policy.max_step_output_bytes * max_width
        if required_budget > policy.max_total_output_bytes:
            issues.append(
                f"parallel_waves requires {required_budget} bytes for a worst-case wave; "
                f"policy.max_total_output_bytes is {policy.max_total_output_bytes}"
            )
    wave_by_id = {step_id: wave for wave, values in enumerate(waves) for step_id in values}

    step_results: list[MissionStepPreflight] = []
    for raw in raw_steps:
        step_id = raw["id"]
        schema: ToolValidationReport | None = None
        local_issues = step_issues.setdefault(step_id, [])
        local_warnings = step_warnings.setdefault(step_id, [])
        try:
            schema = catalogue.validate(raw["tool"], raw.get("arguments", {}))
            if not schema.ok:
                local_issues.extend(
                    f"{issue.path}: {issue.code}: {issue.message}" for issue in schema.issues
                )
            for warning in schema.warnings:
                message = f"{warning.path}: {warning.code}: {warning.message}"
                local_warnings.append(message)
                warnings.append(f"{step_id}: {message}")
        except ToolSchemaError as error:
            message = str(error)
            local_issues.append(message)
            issues.append(f"{step_id}: {message}")
        status = "ready"
        if local_issues:
            status = "blocked" if any("dependency" in issue or "binding" in issue for issue in local_issues) else "invalid"
        step_results.append(
            MissionStepPreflight(
                id=step_id,
                tool=raw["tool"],
                depends_on=tuple(raw.get("depends_on", [])),
                wave=wave_by_id.get(step_id),
                status=status,
                schema=schema,
                issues=tuple(dict.fromkeys(local_issues)),
                warnings=tuple(dict.fromkeys(local_warnings)),
            )
        )
    return MissionPreflight(
        mission_id=request.mission_id,
        goal=request.goal,
        request_digest=request_digest,
        catalogue_digest=catalogue.digest,
        execution="authorized" if policy.execute else "planned",
        execution_mode=policy.execution_mode,
        waves=tuple(waves),
        steps=tuple(step_results),
        issues=tuple(dict.fromkeys(issues)),
        warnings=tuple(dict.fromkeys(warnings)),
    )


def _mission_policy(value: MissionPolicy | Mapping[str, Any] | None) -> MissionPolicy:
    if value is None:
        return MissionPolicy()
    if isinstance(value, MissionPolicy):
        return value
    raw = _mapping("policy", value)
    allowed = raw.get("allowed_tools", ())
    if isinstance(allowed, (str, bytes)) or not isinstance(allowed, Sequence):
        raise ArgumentError("policy.allowed_tools must be a sequence")
    return MissionPolicy(
        execute=raw.get("execute", False),
        stop_on_error=raw.get("stop_on_error", True),
        allow_side_effects=raw.get("allow_side_effects", False),
        max_steps=raw.get("max_steps", 64),
        max_step_output_bytes=raw.get("max_step_output_bytes", 2_000_000),
        max_total_output_bytes=raw.get("max_total_output_bytes", 10_000_000),
        execution_mode=raw.get("execution_mode", "serial"),
        allowed_tools=tuple(allowed),
    )


def _pointer_exists(value: Any, pointer: str) -> bool:
    if not _valid_pointer(pointer, allow_empty=False):
        return False
    current = value
    for token in pointer[1:].split("/"):
        token = token.replace("~1", "/").replace("~0", "~")
        if isinstance(current, Mapping):
            if token not in current:
                return False
            current = current[token]
        elif isinstance(current, list) and token.isdigit():
            index = int(token)
            if index >= len(current):
                return False
            current = current[index]
        else:
            return False
    return True


def _contains_confirmation(value: Any) -> bool:
    if isinstance(value, Mapping):
        if value.get("confirm") is True:
            return True
        return any(_contains_confirmation(child) for child in value.values())
    if isinstance(value, list):
        return any(_contains_confirmation(child) for child in value)
    return False


__all__ = [
    "MAX_ALLOWED_TOOLS",
    "MAX_MISSION_STEPS",
    "MAX_STEP_OUTPUT_BYTES",
    "MAX_TOTAL_OUTPUT_BYTES",
    "MAX_PARALLEL_WAVE_WIDTH",
    "MISSION_ASSEMBLY_SCHEMA",
    "MissionBinding",
    "MissionAssembly",
    "MissionPolicy",
    "MissionPreflight",
    "MissionPreflightError",
    "MissionRouteSelection",
    "MissionRequest",
    "MissionStep",
    "MissionStepPreflight",
    "mission_from_route",
    "preflight_mission",
]
