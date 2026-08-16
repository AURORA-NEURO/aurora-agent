"""Typed contextual-integrity and utility-safety boundary audits."""

from __future__ import annotations

import json
import math
from dataclasses import dataclass
from typing import Any, Mapping, Sequence

from .capability import _route_mapping, _route_strings, _route_text
from .errors import ArgumentError


BIOEVAL_BOUNDARY_SCHEMA = "bioprism-mcp/bioeval-boundary-audit/0.1"
BIOEVAL_BOUNDARY_CHANNELS = frozenset({
    "final_output",
    "tool_arguments",
    "external_queries",
    "inter_agent_messages",
    "shared_memory",
    "logs",
    "artifacts",
    "environment_writes",
    "network_destinations",
})
BIOEVAL_BOUNDARY_EFFECTS = frozenset({"materialized", "proposed", "bypass_attempted"})
MAX_BIOEVAL_BOUNDARY_POLICIES = 4_096
MAX_BIOEVAL_BOUNDARY_FLOWS = 8_192
MAX_BIOEVAL_BOUNDARY_OUTPUT_ITEMS = 1_000
MAX_BIOEVAL_BOUNDARY_TEXT_BYTES = 4_096
MAX_BIOEVAL_BOUNDARY_INPUT_BYTES = 20_000_000


def _text(name: str, value: Any) -> str:
    if not isinstance(value, str) or not value.strip():
        raise ArgumentError(f"{name} must be a non-empty string")
    if len(value.encode("utf-8")) > MAX_BIOEVAL_BOUNDARY_TEXT_BYTES:
        raise ArgumentError(f"{name} exceeds {MAX_BIOEVAL_BOUNDARY_TEXT_BYTES} UTF-8 bytes")
    return value


def _array(name: str, value: Any) -> tuple[Any, ...]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)):
        raise ArgumentError(f"{name} must be an array")
    return tuple(value)


def _payload(value: Mapping[str, Any]) -> dict[str, Any]:
    raw = _route_mapping("bioeval boundary response", value)

    def matches(candidate: Mapping[str, Any]) -> bool:
        if candidate.get("ok") is True:
            return candidate.get("schema") == BIOEVAL_BOUNDARY_SCHEMA and isinstance(candidate.get("boundary"), Mapping)
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
                        raise ArgumentError(f"bioeval boundary response text is not JSON: {error}") from error
                    if isinstance(decoded, Mapping):
                        candidates.append(decoded)
    for candidate in candidates:
        if matches(candidate):
            return dict(candidate)
    raise ArgumentError("response does not contain a bioeval boundary projection")


@dataclass(frozen=True)
class BioevalBoundaryEffectArgs:
    kind: str = "materialized"
    denied_by: str | None = None
    detail: str | None = None

    def __post_init__(self) -> None:
        kind = _text("bioeval boundary effect", self.kind)
        if kind not in BIOEVAL_BOUNDARY_EFFECTS:
            raise ArgumentError("bioeval boundary effect is not recognized")
        if kind == "proposed":
            if self.denied_by is None:
                raise ArgumentError("proposed boundary effects require denied_by")
            object.__setattr__(self, "denied_by", _text("bioeval boundary denied_by", self.denied_by))
        elif kind == "bypass_attempted":
            if self.detail is None:
                raise ArgumentError("bypass boundary effects require detail")
            object.__setattr__(self, "detail", _text("bioeval boundary bypass detail", self.detail))
        object.__setattr__(self, "kind", kind)

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "BioevalBoundaryEffectArgs":
        raw = _route_mapping("bioeval boundary effect", value)
        return cls(raw.get("effect"), raw.get("denied_by"), raw.get("detail"))

    def to_wire(self) -> dict[str, Any]:
        result: dict[str, Any] = {"effect": self.kind}
        if self.denied_by is not None:
            result["denied_by"] = self.denied_by
        if self.detail is not None:
            result["detail"] = self.detail
        return result


@dataclass(frozen=True)
class BioevalBoundaryPolicyArgs:
    id: str
    transmission_principle: str
    sender: str | None = None
    subject: str | None = None
    recipient: str | None = None
    information_type: str | None = None
    purpose: str | None = None
    channels: tuple[str, ...] = ()

    def __post_init__(self) -> None:
        object.__setattr__(self, "id", _text("bioeval boundary policy id", self.id))
        object.__setattr__(self, "transmission_principle", _text("bioeval boundary transmission principle", self.transmission_principle))
        for name in ("sender", "subject", "recipient", "information_type", "purpose"):
            value = getattr(self, name)
            if value is not None:
                object.__setattr__(self, name, _text(f"bioeval boundary policy {name}", value))
        channels = tuple(_text("bioeval boundary policy channel", item) for item in self.channels)
        if any(item not in BIOEVAL_BOUNDARY_CHANNELS for item in channels):
            raise ArgumentError("bioeval boundary policy channel is not recognized")
        if len(set(channels)) != len(channels):
            raise ArgumentError("bioeval boundary policy channels must be unique")
        object.__setattr__(self, "channels", channels)

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "BioevalBoundaryPolicyArgs":
        raw = _route_mapping("bioeval boundary policy", value)
        return cls(_text("bioeval boundary policy id", raw.get("id")), _text("bioeval boundary transmission principle", raw.get("transmission_principle")), raw.get("sender"), raw.get("subject"), raw.get("recipient"), raw.get("information_type"), raw.get("purpose"), tuple(_text("bioeval boundary policy channel", item) for item in _array("bioeval boundary policy channels", raw.get("channels", []))))

    def to_wire(self) -> dict[str, Any]:
        result: dict[str, Any] = {"id": self.id, "transmission_principle": self.transmission_principle, "channels": list(self.channels)}
        for name in ("sender", "subject", "recipient", "information_type", "purpose"):
            value = getattr(self, name)
            if value is not None:
                result[name] = value
        return result


@dataclass(frozen=True)
class BioevalBoundaryFlowArgs:
    id: str
    sender: str
    subject: str
    recipient: str
    information_type: str
    purpose: str
    transmission_principle: str
    channel: str
    effect: BioevalBoundaryEffectArgs = BioevalBoundaryEffectArgs()
    irreversible: bool = False

    def __post_init__(self) -> None:
        for name in ("id", "sender", "subject", "recipient", "information_type", "purpose", "transmission_principle"):
            object.__setattr__(self, name, _text(f"bioeval boundary flow {name}", getattr(self, name)))
        channel = _text("bioeval boundary flow channel", self.channel)
        if channel not in BIOEVAL_BOUNDARY_CHANNELS:
            raise ArgumentError("bioeval boundary flow channel is not recognized")
        effect = self.effect if isinstance(self.effect, BioevalBoundaryEffectArgs) else BioevalBoundaryEffectArgs.from_wire(self.effect)
        if not isinstance(self.irreversible, bool):
            raise ArgumentError("bioeval boundary flow irreversible must be a boolean")
        object.__setattr__(self, "channel", channel)
        object.__setattr__(self, "effect", effect)

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "BioevalBoundaryFlowArgs":
        raw = _route_mapping("bioeval boundary flow", value)
        return cls(_text("bioeval boundary flow id", raw.get("id")), _text("bioeval boundary flow sender", raw.get("sender")), _text("bioeval boundary flow subject", raw.get("subject")), _text("bioeval boundary flow recipient", raw.get("recipient")), _text("bioeval boundary flow information_type", raw.get("information_type")), _text("bioeval boundary flow purpose", raw.get("purpose")), _text("bioeval boundary flow transmission_principle", raw.get("transmission_principle")), _text("bioeval boundary flow channel", raw.get("channel")), BioevalBoundaryEffectArgs.from_wire(raw.get("effect")), raw.get("irreversible", False))

    def to_wire(self) -> dict[str, Any]:
        return {
            "id": self.id,
            "sender": self.sender,
            "subject": self.subject,
            "recipient": self.recipient,
            "information_type": self.information_type,
            "purpose": self.purpose,
            "transmission_principle": self.transmission_principle,
            "channel": self.channel,
            "effect": self.effect.to_wire(),
            "irreversible": self.irreversible,
        }


@dataclass(frozen=True)
class BioevalBoundaryAuditArgs:
    flows: tuple[BioevalBoundaryFlowArgs, ...]
    policies: tuple[BioevalBoundaryPolicyArgs, ...] = ()
    utility: float | None = None
    max_items: int = 100
    require_no_violations: bool = False
    require_no_vetoes: bool = False

    def __post_init__(self) -> None:
        flows = tuple(item if isinstance(item, BioevalBoundaryFlowArgs) else BioevalBoundaryFlowArgs.from_wire(item) for item in self.flows)
        if not flows or len(flows) > MAX_BIOEVAL_BOUNDARY_FLOWS:
            raise ArgumentError("bioeval boundary flows must contain 1 to 8192 rows")
        if len({item.id for item in flows}) != len(flows):
            raise ArgumentError("bioeval boundary flow ids must be unique")
        policies = tuple(item if isinstance(item, BioevalBoundaryPolicyArgs) else BioevalBoundaryPolicyArgs.from_wire(item) for item in self.policies)
        if len(policies) > MAX_BIOEVAL_BOUNDARY_POLICIES:
            raise ArgumentError("bioeval boundary policies are bounded at 4096 rows")
        if len({item.id for item in policies}) != len(policies):
            raise ArgumentError("bioeval boundary policy ids must be unique")
        if self.utility is not None and (isinstance(self.utility, bool) or not isinstance(self.utility, (float, int)) or not math.isfinite(float(self.utility))):
            raise ArgumentError("bioeval boundary utility must be finite")
        if isinstance(self.max_items, bool) or not isinstance(self.max_items, int) or not 1 <= self.max_items <= MAX_BIOEVAL_BOUNDARY_OUTPUT_ITEMS:
            raise ArgumentError("bioeval boundary max_items must be between 1 and 1000")
        for name in ("require_no_violations", "require_no_vetoes"):
            if not isinstance(getattr(self, name), bool):
                raise ArgumentError(f"bioeval boundary {name} must be a boolean")
        object.__setattr__(self, "flows", flows)
        object.__setattr__(self, "policies", policies)
        encoded = json.dumps(self.to_mcp_arguments(), ensure_ascii=False, separators=(",", ":")).encode("utf-8")
        if len(encoded) > MAX_BIOEVAL_BOUNDARY_INPUT_BYTES:
            raise ArgumentError("bioeval boundary input exceeds the 20000000-byte safety bound")

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "BioevalBoundaryAuditArgs":
        raw = _route_mapping("bioeval boundary arguments", value)
        utility = raw.get("utility")
        return cls(
            tuple(BioevalBoundaryFlowArgs.from_wire(item) for item in _array("bioeval boundary flows", raw.get("flows"))),
            tuple(BioevalBoundaryPolicyArgs.from_wire(item) for item in _array("bioeval boundary policies", raw.get("policies", []))),
            utility,
            raw.get("max_items", 100),
            raw.get("require_no_violations", False),
            raw.get("require_no_vetoes", False),
        )

    def to_mcp_arguments(self) -> dict[str, Any]:
        result: dict[str, Any] = {
            "policies": [item.to_wire() for item in self.policies],
            "flows": [item.to_wire() for item in self.flows],
            "max_items": self.max_items,
            "require_no_violations": self.require_no_violations,
            "require_no_vetoes": self.require_no_vetoes,
        }
        if self.utility is not None:
            result["utility"] = self.utility
        return result


@dataclass(frozen=True)
class BioevalBoundaryAuditReport:
    raw: dict[str, Any]
    ok: bool
    schema: str | None
    workflow: str | None
    boundary: Mapping[str, Any] | None
    policies: Mapping[str, Any] | None
    flows: Mapping[str, Any] | None
    violations_by_channel: Mapping[str, Any] | None
    pareto: Mapping[str, Any] | None
    composite: Mapping[str, Any] | None
    findings: Mapping[str, Any] | None
    stage: str | None
    refusal: str | None
    guarantees: tuple[str, ...]
    limitations: tuple[str, ...]
    fail_closed: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "BioevalBoundaryAuditReport":
        raw = _payload(value)
        if raw.get("ok") is False:
            if raw.get("fail_closed") is not True:
                raise ArgumentError("bioeval boundary refusals must be fail-closed")
            return cls(raw, False, raw.get("schema"), raw.get("workflow"), None, None, None, None, None, None, None, _route_text("bioeval boundary refusal stage", raw.get("stage")), _route_text("bioeval boundary refusal", raw.get("refusal")), _route_strings("bioeval boundary refusal guarantees", raw.get("guarantees", [])), _route_strings("bioeval boundary refusal limitations", raw.get("limitations", [])), True)
        if raw.get("ok") is not True or raw.get("schema") != BIOEVAL_BOUNDARY_SCHEMA:
            raise ArgumentError("bioeval boundary projection has an invalid schema")
        return cls(raw, True, BIOEVAL_BOUNDARY_SCHEMA, _route_text("bioeval boundary workflow", raw.get("workflow")), _route_mapping("bioeval boundary summary", raw.get("boundary")), _route_mapping("bioeval boundary policies", raw.get("policies")), _route_mapping("bioeval boundary flows", raw.get("flows")), _route_mapping("bioeval boundary channel violations", raw.get("violations_by_channel")), None if raw.get("pareto") is None else _route_mapping("bioeval boundary pareto", raw.get("pareto")), _route_mapping("bioeval boundary composite", raw.get("composite")), _route_mapping("bioeval boundary findings", raw.get("findings")), None, None, _route_strings("bioeval boundary guarantees", raw.get("guarantees", [])), _route_strings("bioeval boundary limitations", raw.get("limitations", [])), False)

    @property
    def accepted(self) -> bool:
        return self.ok

    @property
    def refused(self) -> bool:
        return not self.ok

    @property
    def violation_count(self) -> int | None:
        if self.boundary is None:
            return None
        value = self.boundary.get("violation_count")
        return value if isinstance(value, int) and not isinstance(value, bool) else None

    @property
    def veto_count(self) -> int | None:
        if self.boundary is None:
            return None
        value = self.boundary.get("veto_count")
        return value if isinstance(value, int) and not isinstance(value, bool) else None

    @property
    def composite_refused(self) -> bool | None:
        if self.findings is None:
            return None
        value = self.findings.get("composite_refused")
        return value if isinstance(value, bool) else None

    def finding_ids(self, name: str) -> tuple[str, ...]:
        if self.findings is None or not isinstance(self.findings.get(name), Mapping):
            return ()
        values = self.findings[name].get("ids", [])
        return tuple(value for value in values if isinstance(value, str)) if isinstance(values, Sequence) and not isinstance(values, (str, bytes)) else ()

    @property
    def violating_flows(self) -> tuple[str, ...]:
        return self.finding_ids("violating_flows")

    @property
    def veto_flows(self) -> tuple[str, ...]:
        return self.finding_ids("veto_flows")

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


def bioeval_boundary_audit_report(value: Mapping[str, Any]) -> BioevalBoundaryAuditReport:
    """Parse direct MCP output or an HTTP REST tool envelope."""

    return BioevalBoundaryAuditReport.from_wire(value)


__all__ = [
    "BIOEVAL_BOUNDARY_SCHEMA",
    "BIOEVAL_BOUNDARY_CHANNELS",
    "BIOEVAL_BOUNDARY_EFFECTS",
    "MAX_BIOEVAL_BOUNDARY_POLICIES",
    "MAX_BIOEVAL_BOUNDARY_FLOWS",
    "MAX_BIOEVAL_BOUNDARY_OUTPUT_ITEMS",
    "MAX_BIOEVAL_BOUNDARY_TEXT_BYTES",
    "MAX_BIOEVAL_BOUNDARY_INPUT_BYTES",
    "BioevalBoundaryEffectArgs",
    "BioevalBoundaryPolicyArgs",
    "BioevalBoundaryFlowArgs",
    "BioevalBoundaryAuditArgs",
    "BioevalBoundaryAuditReport",
    "bioeval_boundary_audit_report",
]
