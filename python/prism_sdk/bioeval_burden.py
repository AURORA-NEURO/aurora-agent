"""Typed nonrenewable-resource and branch-feasibility audits."""

from __future__ import annotations

import json
from dataclasses import dataclass
from typing import Any, Mapping, Sequence

from .capability import _route_mapping, _route_strings, _route_text
from .errors import ArgumentError


BIOEVAL_BURDEN_SCHEMA = "bioprism-mcp/bioeval-burden-audit/0.1"
BIOEVAL_BURDEN_CLASSES = frozenset({
    "tissue_aliquot",
    "viable_cells",
    "assay_capacity",
    "expert_time",
    "participant_burden",
    "privacy_access",
    "compute_and_money",
})
BIOEVAL_BURDEN_OUTCOMES = frozenset({"productive", "wasted"})
MAX_BIOEVAL_BURDEN_RESOURCES = 4_096
MAX_BIOEVAL_BURDEN_BRANCHES = 4_096
MAX_BIOEVAL_BURDEN_DRAWS = 16_384
MAX_BIOEVAL_BURDEN_OUTPUT_ITEMS = 1_000
MAX_BIOEVAL_BURDEN_TEXT_BYTES = 4_096
MAX_BIOEVAL_BURDEN_INPUT_BYTES = 20_000_000


def _text(name: str, value: Any) -> str:
    if not isinstance(value, str) or not value.strip():
        raise ArgumentError(f"{name} must be a non-empty string")
    if len(value.encode("utf-8")) > MAX_BIOEVAL_BURDEN_TEXT_BYTES:
        raise ArgumentError(f"{name} exceeds {MAX_BIOEVAL_BURDEN_TEXT_BYTES} UTF-8 bytes")
    return value


def _array(name: str, value: Any) -> tuple[Any, ...]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)):
        raise ArgumentError(f"{name} must be an array")
    return tuple(value)


def _payload(value: Mapping[str, Any]) -> dict[str, Any]:
    raw = _route_mapping("bioeval burden response", value)

    def matches(candidate: Mapping[str, Any]) -> bool:
        if candidate.get("ok") is True:
            return candidate.get("schema") == BIOEVAL_BURDEN_SCHEMA and isinstance(candidate.get("burden"), Mapping)
        return candidate.get("ok") is False and isinstance(candidate.get("stage"), str) and isinstance(candidate.get("refusal"), str)

    candidates: list[Mapping[str, Any]] = [raw]
    mcp = raw.get("mcp")
    if isinstance(mcp, Mapping):
        candidates.append(mcp)
        result = mcp.get("result")
        if isinstance(result, Mapping):
            candidates.append(result)
            structured = result.get("structuredContent")
            if isinstance(structured, Mapping):
                candidates.append(structured)
            content = result.get("content")
            if isinstance(content, Sequence) and not isinstance(content, (str, bytes)):
                for block in content:
                    if not isinstance(block, Mapping) or not isinstance(block.get("text"), str):
                        continue
                    try:
                        decoded = json.loads(block["text"])
                    except json.JSONDecodeError as error:
                        raise ArgumentError(f"bioeval burden response text is not JSON: {error}") from error
                    if isinstance(decoded, Mapping):
                        candidates.append(decoded)
    for candidate in candidates:
        if matches(candidate):
            return dict(candidate)
    raise ArgumentError("response does not contain a bioeval burden projection")


@dataclass(frozen=True)
class BioevalBurdenResourceArgs:
    id: str
    resource_class: str
    initial: int
    unit: str

    def __post_init__(self) -> None:
        identifier = _text("bioeval burden resource id", self.id)
        resource_class = _text("bioeval burden resource class", self.resource_class)
        if resource_class not in BIOEVAL_BURDEN_CLASSES:
            raise ArgumentError("bioeval burden resource class is not recognized")
        if isinstance(self.initial, bool) or not isinstance(self.initial, int) or self.initial < 0:
            raise ArgumentError("bioeval burden resource initial must be a non-negative integer")
        unit = _text("bioeval burden resource unit", self.unit)
        object.__setattr__(self, "id", identifier)
        object.__setattr__(self, "resource_class", resource_class)
        object.__setattr__(self, "unit", unit)

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "BioevalBurdenResourceArgs":
        raw = _route_mapping("bioeval burden resource", value)
        return cls(
            _text("bioeval burden resource id", raw.get("id")),
            _text("bioeval burden resource class", raw.get("class")),
            raw.get("initial"),
            _text("bioeval burden resource unit", raw.get("unit")),
        )

    def to_wire(self) -> dict[str, Any]:
        return {"id": self.id, "class": self.resource_class, "initial": self.initial, "unit": self.unit}


@dataclass(frozen=True)
class BioevalBurdenBranchArgs:
    id: str
    parent: str | None = None

    def __post_init__(self) -> None:
        identifier = _text("bioeval burden branch id", self.id)
        parent = None if self.parent is None else _text("bioeval burden branch parent", self.parent)
        if parent == identifier:
            raise ArgumentError("bioeval burden branch cannot parent itself")
        object.__setattr__(self, "id", identifier)
        object.__setattr__(self, "parent", parent)

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "BioevalBurdenBranchArgs":
        raw = _route_mapping("bioeval burden branch", value)
        return cls(_text("bioeval burden branch id", raw.get("id")), None if raw.get("parent") is None else _text("bioeval burden branch parent", raw.get("parent")))

    def to_wire(self) -> dict[str, Any]:
        result = {"id": self.id}
        if self.parent is not None:
            result["parent"] = self.parent
        return result


@dataclass(frozen=True)
class BioevalBurdenDrawArgs:
    branch: str
    action: str
    resource: str
    amount: int
    unit: str
    outcome: str = "productive"
    destructive: bool = True

    def __post_init__(self) -> None:
        for name in ("branch", "action", "resource", "unit"):
            object.__setattr__(self, name, _text(f"bioeval burden draw {name}", getattr(self, name)))
        if isinstance(self.amount, bool) or not isinstance(self.amount, int) or self.amount < 0:
            raise ArgumentError("bioeval burden draw amount must be a non-negative integer")
        outcome = _text("bioeval burden draw outcome", self.outcome)
        if outcome not in BIOEVAL_BURDEN_OUTCOMES:
            raise ArgumentError("bioeval burden draw outcome is not recognized")
        if not isinstance(self.destructive, bool):
            raise ArgumentError("bioeval burden draw destructive must be a boolean")
        object.__setattr__(self, "outcome", outcome)

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "BioevalBurdenDrawArgs":
        raw = _route_mapping("bioeval burden draw", value)
        return cls(
            _text("bioeval burden draw branch", raw.get("branch")),
            _text("bioeval burden draw action", raw.get("action")),
            _text("bioeval burden draw resource", raw.get("resource")),
            raw.get("amount"),
            _text("bioeval burden draw unit", raw.get("unit")),
            raw.get("outcome", "productive"),
            raw.get("destructive", True),
        )

    def to_wire(self) -> dict[str, Any]:
        return {
            "branch": self.branch,
            "action": self.action,
            "resource": self.resource,
            "amount": self.amount,
            "unit": self.unit,
            "outcome": self.outcome,
            "destructive": self.destructive,
        }


@dataclass(frozen=True)
class BioevalBurdenAuditArgs:
    root: str
    resources: tuple[BioevalBurdenResourceArgs, ...]
    branches: tuple[BioevalBurdenBranchArgs, ...] = ()
    draws: tuple[BioevalBurdenDrawArgs, ...] = ()
    inspect_branches: tuple[str, ...] | None = None
    joint_branches: tuple[str, ...] | None = None
    max_items: int = 100
    require_joint_feasible: bool = False
    require_no_wasted_nonrenewable: bool = False

    def __post_init__(self) -> None:
        root = _text("bioeval burden root", self.root)
        resources = tuple(item if isinstance(item, BioevalBurdenResourceArgs) else BioevalBurdenResourceArgs.from_wire(item) for item in self.resources)
        if not resources or len(resources) > MAX_BIOEVAL_BURDEN_RESOURCES:
            raise ArgumentError("bioeval burden resources must contain 1 to 4096 rows")
        if len({item.id for item in resources}) != len(resources):
            raise ArgumentError("bioeval burden resource ids must be unique")
        branches = tuple(item if isinstance(item, BioevalBurdenBranchArgs) else BioevalBurdenBranchArgs.from_wire(item) for item in self.branches)
        if len(branches) > MAX_BIOEVAL_BURDEN_BRANCHES:
            raise ArgumentError("bioeval burden branches are bounded at 4096 rows")
        branch_ids = {root}
        for branch in branches:
            if branch.id in branch_ids:
                raise ArgumentError("bioeval burden branch ids must be unique and cannot equal root")
            if branch.parent is not None and branch.parent not in branch_ids:
                raise ArgumentError(f"bioeval burden branch parent {branch.parent!r} must be declared earlier")
            branch_ids.add(branch.id)
        draws = tuple(item if isinstance(item, BioevalBurdenDrawArgs) else BioevalBurdenDrawArgs.from_wire(item) for item in self.draws)
        if len(draws) > MAX_BIOEVAL_BURDEN_DRAWS:
            raise ArgumentError("bioeval burden draws are bounded at 16384 rows")
        resource_ids = {item.id for item in resources}
        if any(item.branch not in branch_ids for item in draws):
            raise ArgumentError("bioeval burden draws must name declared branches")
        if any(item.resource not in resource_ids for item in draws):
            raise ArgumentError("bioeval burden draws must name declared resources")
        def branch_list(name: str, value: tuple[str, ...] | None) -> tuple[str, ...] | None:
            if value is None:
                return None
            values = tuple(_text(f"bioeval burden {name} branch", item) for item in value)
            if len(set(values)) != len(values):
                raise ArgumentError(f"bioeval burden {name} must contain unique branch ids")
            if any(item not in branch_ids for item in values):
                raise ArgumentError(f"bioeval burden {name} names an undeclared branch")
            return values
        inspect_branches = branch_list("inspect", self.inspect_branches)
        joint_branches = branch_list("joint", self.joint_branches)
        if isinstance(self.max_items, bool) or not isinstance(self.max_items, int) or not 1 <= self.max_items <= MAX_BIOEVAL_BURDEN_OUTPUT_ITEMS:
            raise ArgumentError("bioeval burden max_items must be between 1 and 1000")
        for name in ("require_joint_feasible", "require_no_wasted_nonrenewable"):
            if not isinstance(getattr(self, name), bool):
                raise ArgumentError(f"bioeval burden {name} must be a boolean")
        object.__setattr__(self, "root", root)
        object.__setattr__(self, "resources", resources)
        object.__setattr__(self, "branches", branches)
        object.__setattr__(self, "draws", draws)
        object.__setattr__(self, "inspect_branches", inspect_branches)
        object.__setattr__(self, "joint_branches", joint_branches)
        encoded = json.dumps(self.to_mcp_arguments(), ensure_ascii=False, separators=(",", ":")).encode("utf-8")
        if len(encoded) > MAX_BIOEVAL_BURDEN_INPUT_BYTES:
            raise ArgumentError("bioeval burden input exceeds the 20000000-byte safety bound")

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "BioevalBurdenAuditArgs":
        raw = _route_mapping("bioeval burden arguments", value)
        return cls(
            _text("bioeval burden root", raw.get("root")),
            tuple(BioevalBurdenResourceArgs.from_wire(item) for item in _array("bioeval burden resources", raw.get("resources"))),
            tuple(BioevalBurdenBranchArgs.from_wire(item) for item in _array("bioeval burden branches", raw.get("branches", []))),
            tuple(BioevalBurdenDrawArgs.from_wire(item) for item in _array("bioeval burden draws", raw.get("draws", []))),
            None if raw.get("inspect_branches") is None else tuple(_text("bioeval burden inspect branch", item) for item in _array("bioeval burden inspect_branches", raw.get("inspect_branches"))),
            None if raw.get("joint_branches") is None else tuple(_text("bioeval burden joint branch", item) for item in _array("bioeval burden joint_branches", raw.get("joint_branches"))),
            raw.get("max_items", 100),
            raw.get("require_joint_feasible", False),
            raw.get("require_no_wasted_nonrenewable", False),
        )

    def to_mcp_arguments(self) -> dict[str, Any]:
        result: dict[str, Any] = {
            "root": self.root,
            "resources": [item.to_wire() for item in self.resources],
            "branches": [item.to_wire() for item in self.branches],
            "draws": [item.to_wire() for item in self.draws],
            "max_items": self.max_items,
            "require_joint_feasible": self.require_joint_feasible,
            "require_no_wasted_nonrenewable": self.require_no_wasted_nonrenewable,
        }
        if self.inspect_branches is not None:
            result["inspect_branches"] = list(self.inspect_branches)
        if self.joint_branches is not None:
            result["joint_branches"] = list(self.joint_branches)
        return result


@dataclass(frozen=True)
class BioevalBurdenAuditReport:
    raw: dict[str, Any]
    ok: bool
    schema: str | None
    workflow: str | None
    burden: Mapping[str, Any] | None
    resources: Mapping[str, Any] | None
    branches: Mapping[str, Any] | None
    draws: Mapping[str, Any] | None
    joint_feasibility: Mapping[str, Any] | None
    wasted_nonrenewable: Mapping[str, Any] | None
    findings: Mapping[str, Any] | None
    stage: str | None
    refusal: str | None
    guarantees: tuple[str, ...]
    limitations: tuple[str, ...]
    fail_closed: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "BioevalBurdenAuditReport":
        raw = _payload(value)
        if raw.get("ok") is False:
            if raw.get("fail_closed") is not True:
                raise ArgumentError("bioeval burden refusals must be fail-closed")
            return cls(raw, False, raw.get("schema"), raw.get("workflow"), None, None, None, None, None, None, None, _route_text("bioeval burden refusal stage", raw.get("stage")), _route_text("bioeval burden refusal", raw.get("refusal")), _route_strings("bioeval burden refusal guarantees", raw.get("guarantees", [])), _route_strings("bioeval burden refusal limitations", raw.get("limitations", [])), True)
        if raw.get("ok") is not True or raw.get("schema") != BIOEVAL_BURDEN_SCHEMA:
            raise ArgumentError("bioeval burden projection has an invalid schema")
        return cls(raw, True, BIOEVAL_BURDEN_SCHEMA, _route_text("bioeval burden workflow", raw.get("workflow")), _route_mapping("bioeval burden summary", raw.get("burden")), _route_mapping("bioeval burden resources", raw.get("resources")), _route_mapping("bioeval burden branches", raw.get("branches")), _route_mapping("bioeval burden draws", raw.get("draws")), _route_mapping("bioeval burden joint feasibility", raw.get("joint_feasibility")), _route_mapping("bioeval burden wasted resources", raw.get("wasted_nonrenewable")), _route_mapping("bioeval burden findings", raw.get("findings")), None, None, _route_strings("bioeval burden guarantees", raw.get("guarantees", [])), _route_strings("bioeval burden limitations", raw.get("limitations", [])), False)

    @property
    def accepted(self) -> bool:
        return self.ok

    @property
    def refused(self) -> bool:
        return not self.ok

    @property
    def joint_refused(self) -> bool | None:
        if self.joint_feasibility is None:
            return None
        return self.joint_feasibility.get("status") == "refused"

    @property
    def wasted_nonrenewable_count(self) -> int | None:
        if self.wasted_nonrenewable is None:
            return None
        value = self.wasted_nonrenewable.get("total")
        return value if isinstance(value, int) and not isinstance(value, bool) else None

    @property
    def branch_count(self) -> int | None:
        if self.burden is None:
            return None
        value = self.burden.get("branch_count")
        return value if isinstance(value, int) and not isinstance(value, bool) else None

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


def bioeval_burden_audit_report(value: Mapping[str, Any]) -> BioevalBurdenAuditReport:
    """Parse direct MCP output or an HTTP REST tool envelope."""

    return BioevalBurdenAuditReport.from_wire(value)


__all__ = [
    "BIOEVAL_BURDEN_SCHEMA",
    "BIOEVAL_BURDEN_CLASSES",
    "BIOEVAL_BURDEN_OUTCOMES",
    "MAX_BIOEVAL_BURDEN_RESOURCES",
    "MAX_BIOEVAL_BURDEN_BRANCHES",
    "MAX_BIOEVAL_BURDEN_DRAWS",
    "MAX_BIOEVAL_BURDEN_OUTPUT_ITEMS",
    "MAX_BIOEVAL_BURDEN_TEXT_BYTES",
    "MAX_BIOEVAL_BURDEN_INPUT_BYTES",
    "BioevalBurdenResourceArgs",
    "BioevalBurdenBranchArgs",
    "BioevalBurdenDrawArgs",
    "BioevalBurdenAuditArgs",
    "BioevalBurdenAuditReport",
    "bioeval_burden_audit_report",
]
