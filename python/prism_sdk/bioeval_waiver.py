"""Typed release-gate waiver audits for bioevaluation decisions.

The authoring layer validates the closed gate/verdict vocabulary and required waiver fields,
then delegates expiry, version coverage, veto protection, and blocking posture to Rust. A waiver
never rewrites the underlying verdict into ``met``.
"""

from __future__ import annotations

import json
from datetime import datetime
from dataclasses import dataclass
from typing import Any, Mapping, Sequence

from .capability import _route_mapping, _route_strings, _route_text
from .errors import ArgumentError


BIOEVAL_WAIVER_SCHEMA = "bioprism-mcp/bioeval-waiver-audit/0.1"
BIOEVAL_WAIVER_GATE_KINDS = frozenset({
    "safety_veto",
    "benchmark_health",
    "capability_floor",
    "non_inferiority",
    "required_improvement",
    "cost_ceiling",
    "confidence_requirement",
    "maximum_unknown_rate",
})
BIOEVAL_WAIVER_VERDICTS = frozenset({"met", "violated", "unevaluable"})
MAX_BIOEVAL_WAIVER_GATES = 1_024
MAX_BIOEVAL_WAIVER_ROWS = 1_024
MAX_BIOEVAL_WAIVER_OUTPUT_ITEMS = 1_000
MAX_BIOEVAL_WAIVER_TEXT_BYTES = 4_096
MAX_BIOEVAL_WAIVER_INPUT_BYTES = 20_000_000


def _text(name: str, value: Any) -> str:
    if not isinstance(value, str) or not value.strip():
        raise ArgumentError(f"{name} must be a non-empty string")
    if len(value.encode("utf-8")) > MAX_BIOEVAL_WAIVER_TEXT_BYTES:
        raise ArgumentError(f"{name} exceeds {MAX_BIOEVAL_WAIVER_TEXT_BYTES} UTF-8 bytes")
    return value


def _timestamp(name: str, value: Any) -> str:
    timestamp = _text(name, value)
    try:
        datetime.fromisoformat(timestamp.replace("Z", "+00:00"))
    except ValueError as error:
        raise ArgumentError(f"{name} must be valid RFC-3339 text") from error
    return timestamp


def _array(name: str, value: Any) -> tuple[Any, ...]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)):
        raise ArgumentError(f"{name} must be an array")
    return tuple(value)


def _payload(value: Mapping[str, Any]) -> dict[str, Any]:
    raw = _route_mapping("bioeval waiver response", value)

    def matches(candidate: Mapping[str, Any]) -> bool:
        if candidate.get("ok") is True:
            return candidate.get("schema") == BIOEVAL_WAIVER_SCHEMA and isinstance(candidate.get("release"), Mapping)
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
                        raise ArgumentError(f"bioeval waiver response text is not JSON: {error}") from error
                    if isinstance(decoded, Mapping):
                        candidates.append(decoded)
    for candidate in candidates:
        if matches(candidate):
            return dict(candidate)
    raise ArgumentError("response does not contain a bioeval waiver projection")


@dataclass(frozen=True)
class BioevalWaiverGateVerdictArgs:
    verdict: str
    detail: str | None = None
    missing: str | None = None

    def __post_init__(self) -> None:
        verdict = _text("bioeval waiver gate verdict", self.verdict)
        if verdict not in BIOEVAL_WAIVER_VERDICTS:
            raise ArgumentError("bioeval waiver gate verdict must be met, violated, or unevaluable")
        detail = None if self.detail is None else _text("bioeval violated gate detail", self.detail)
        missing = None if self.missing is None else _text("bioeval unevaluable gate missing", self.missing)
        if verdict == "met" and (detail is not None or missing is not None):
            raise ArgumentError("met gates cannot carry detail or missing evidence")
        if verdict == "violated" and not detail:
            raise ArgumentError("violated gates require detail")
        if verdict == "unevaluable" and not missing:
            raise ArgumentError("unevaluable gates require missing evidence")
        if verdict != "violated" and detail is not None:
            raise ArgumentError("only violated gates may carry detail")
        if verdict != "unevaluable" and missing is not None:
            raise ArgumentError("only unevaluable gates may carry missing evidence")
        object.__setattr__(self, "verdict", verdict)
        object.__setattr__(self, "detail", detail)
        object.__setattr__(self, "missing", missing)

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "BioevalWaiverGateVerdictArgs":
        raw = _route_mapping("bioeval waiver gate verdict", value)
        return cls(raw.get("verdict"), raw.get("detail"), raw.get("missing"))

    def to_wire(self) -> dict[str, Any]:
        result: dict[str, Any] = {"verdict": self.verdict}
        if self.detail is not None:
            result["detail"] = self.detail
        if self.missing is not None:
            result["missing"] = self.missing
        return result


@dataclass(frozen=True)
class BioevalWaiverGateArgs:
    id: str
    kind: str
    verdict: BioevalWaiverGateVerdictArgs | Mapping[str, Any]

    def __post_init__(self) -> None:
        identifier = _text("bioeval waiver gate id", self.id)
        kind = _text("bioeval waiver gate kind", self.kind)
        if kind not in BIOEVAL_WAIVER_GATE_KINDS:
            raise ArgumentError("bioeval waiver gate kind is not recognized")
        verdict = self.verdict if isinstance(self.verdict, BioevalWaiverGateVerdictArgs) else BioevalWaiverGateVerdictArgs.from_wire(self.verdict)
        object.__setattr__(self, "id", identifier)
        object.__setattr__(self, "kind", kind)
        object.__setattr__(self, "verdict", verdict)

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "BioevalWaiverGateArgs":
        raw = _route_mapping("bioeval waiver gate", value)
        return cls(_text("bioeval waiver gate id", raw.get("id")), _text("bioeval waiver gate kind", raw.get("kind")), BioevalWaiverGateVerdictArgs.from_wire(raw.get("verdict")))

    def to_wire(self) -> dict[str, Any]:
        return {"id": self.id, "kind": self.kind, "verdict": self.verdict.to_wire()}  # type: ignore[union-attr]


@dataclass(frozen=True)
class BioevalWaiverArgs:
    gate: str
    authoriser: str
    rationale: str
    expiry: str
    affected_versions: tuple[str, ...]
    follow_up: str

    def __post_init__(self) -> None:
        gate = _text("bioeval waiver gate", self.gate)
        authoriser = _text("bioeval waiver authoriser", self.authoriser)
        rationale = _text("bioeval waiver rationale", self.rationale)
        expiry = _timestamp("bioeval waiver expiry", self.expiry)
        versions = tuple(_text("bioeval waiver affected version", item) for item in self.affected_versions)
        if not versions or not any(item.strip() for item in versions):
            raise ArgumentError("bioeval waiver must name at least one affected version")
        follow_up = _text("bioeval waiver follow_up", self.follow_up)
        object.__setattr__(self, "gate", gate)
        object.__setattr__(self, "authoriser", authoriser)
        object.__setattr__(self, "rationale", rationale)
        object.__setattr__(self, "expiry", expiry)
        object.__setattr__(self, "affected_versions", versions)
        object.__setattr__(self, "follow_up", follow_up)

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "BioevalWaiverArgs":
        raw = _route_mapping("bioeval waiver", value)
        return cls(
            _text("bioeval waiver gate", raw.get("gate")),
            _text("bioeval waiver authoriser", raw.get("authoriser")),
            _text("bioeval waiver rationale", raw.get("rationale")),
            _timestamp("bioeval waiver expiry", raw.get("expiry")),
            tuple(_text("bioeval waiver affected version", item) for item in _array("bioeval waiver affected_versions", raw.get("affected_versions"))),
            _text("bioeval waiver follow_up", raw.get("follow_up")),
        )

    def to_wire(self) -> dict[str, Any]:
        return {"gate": self.gate, "authoriser": self.authoriser, "rationale": self.rationale, "expiry": self.expiry, "affected_versions": list(self.affected_versions), "follow_up": self.follow_up}


@dataclass(frozen=True)
class BioevalWaiverAuditArgs:
    version: str
    at: str
    gates: tuple[BioevalWaiverGateArgs, ...]
    waivers: tuple[BioevalWaiverArgs, ...] = ()
    max_items: int = 100
    require_releasable: bool = False
    require_no_unevaluable: bool = False

    def __post_init__(self) -> None:
        version = _text("bioeval waiver release version", self.version)
        at = _timestamp("bioeval waiver evaluation time", self.at)
        gates = tuple(item if isinstance(item, BioevalWaiverGateArgs) else BioevalWaiverGateArgs.from_wire(item) for item in self.gates)
        if not gates or len(gates) > MAX_BIOEVAL_WAIVER_GATES:
            raise ArgumentError("bioeval waiver gates must contain 1 to 1024 rows")
        if len({item.id for item in gates}) != len(gates):
            raise ArgumentError("bioeval waiver gate ids must be unique")
        waivers = tuple(item if isinstance(item, BioevalWaiverArgs) else BioevalWaiverArgs.from_wire(item) for item in self.waivers)
        if len(waivers) > MAX_BIOEVAL_WAIVER_ROWS:
            raise ArgumentError("bioeval waiver rows are bounded at 1024")
        if len({item.gate for item in waivers}) != len(waivers):
            raise ArgumentError("bioeval waiver gate names must be unique")
        if isinstance(self.max_items, bool) or not isinstance(self.max_items, int) or not 1 <= self.max_items <= MAX_BIOEVAL_WAIVER_OUTPUT_ITEMS:
            raise ArgumentError("bioeval waiver max_items must be between 1 and 1000")
        for name in ("require_releasable", "require_no_unevaluable"):
            if not isinstance(getattr(self, name), bool):
                raise ArgumentError(f"bioeval waiver {name} must be a boolean")
        object.__setattr__(self, "version", version)
        object.__setattr__(self, "at", at)
        object.__setattr__(self, "gates", gates)
        object.__setattr__(self, "waivers", waivers)
        encoded = json.dumps(self.to_mcp_arguments(), ensure_ascii=False, separators=(",", ":")).encode("utf-8")
        if len(encoded) > MAX_BIOEVAL_WAIVER_INPUT_BYTES:
            raise ArgumentError("bioeval waiver input exceeds the 20000000-byte safety bound")

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "BioevalWaiverAuditArgs":
        raw = _route_mapping("bioeval waiver arguments", value)
        return cls(
            _text("bioeval waiver release version", raw.get("version")),
            _timestamp("bioeval waiver evaluation time", raw.get("at")),
            tuple(BioevalWaiverGateArgs.from_wire(item) for item in _array("bioeval waiver gates", raw.get("gates"))),
            tuple(BioevalWaiverArgs.from_wire(item) for item in _array("bioeval waiver waivers", raw.get("waivers", []))),
            raw.get("max_items", 100),
            raw.get("require_releasable", False),
            raw.get("require_no_unevaluable", False),
        )

    def to_mcp_arguments(self) -> dict[str, Any]:
        return {"version": self.version, "at": self.at, "gates": [item.to_wire() for item in self.gates], "waivers": [item.to_wire() for item in self.waivers], "max_items": self.max_items, "require_releasable": self.require_releasable, "require_no_unevaluable": self.require_no_unevaluable}


@dataclass(frozen=True)
class BioevalWaiverAuditReport:
    raw: dict[str, Any]
    ok: bool
    schema: str | None
    workflow: str | None
    release: Mapping[str, Any] | None
    gates: Mapping[str, Any] | None
    waivers: Mapping[str, Any] | None
    findings: Mapping[str, Any] | None
    stage: str | None
    refusal: str | None
    guarantees: tuple[str, ...]
    limitations: tuple[str, ...]
    fail_closed: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "BioevalWaiverAuditReport":
        raw = _payload(value)
        if raw.get("ok") is False:
            if raw.get("fail_closed") is not True:
                raise ArgumentError("bioeval waiver refusals must be fail-closed")
            return cls(raw, False, raw.get("schema"), raw.get("workflow"), None, None, None, None, _route_text("bioeval waiver refusal stage", raw.get("stage")), _route_text("bioeval waiver refusal", raw.get("refusal")), _route_strings("bioeval waiver refusal guarantees", raw.get("guarantees", [])), _route_strings("bioeval waiver refusal limitations", raw.get("limitations", [])), True)
        if raw.get("ok") is not True or raw.get("schema") != BIOEVAL_WAIVER_SCHEMA:
            raise ArgumentError("bioeval waiver projection has an invalid schema")
        return cls(raw, True, BIOEVAL_WAIVER_SCHEMA, _route_text("bioeval waiver workflow", raw.get("workflow")), _route_mapping("bioeval waiver release", raw.get("release")), _route_mapping("bioeval waiver gates", raw.get("gates")), _route_mapping("bioeval waiver waivers", raw.get("waivers")), _route_mapping("bioeval waiver findings", raw.get("findings")), None, None, _route_strings("bioeval waiver guarantees", raw.get("guarantees", [])), _route_strings("bioeval waiver limitations", raw.get("limitations", [])), False)

    @property
    def accepted(self) -> bool:
        return self.ok

    @property
    def refused(self) -> bool:
        return not self.ok

    @property
    def releasable(self) -> bool | None:
        if self.release is None:
            return None
        value = self.release.get("releasable")
        return value if isinstance(value, bool) else None

    def finding_ids(self, name: str) -> tuple[str, ...]:
        if self.findings is None or not isinstance(self.findings.get(name), Mapping):
            return ()
        values = self.findings[name].get("ids", [])
        return tuple(value for value in values if isinstance(value, str)) if isinstance(values, Sequence) and not isinstance(values, (str, bytes)) else ()

    @property
    def still_blocking(self) -> tuple[str, ...]:
        return self.finding_ids("still_blocking")

    @property
    def waived_gates(self) -> tuple[str, ...]:
        return self.finding_ids("waived_gates")

    @property
    def unevaluable_gates(self) -> tuple[str, ...]:
        return self.finding_ids("unevaluable_gates")

    @property
    def safety_vetoes(self) -> tuple[str, ...]:
        return self.finding_ids("safety_vetoes")

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


def bioeval_waiver_audit_report(value: Mapping[str, Any]) -> BioevalWaiverAuditReport:
    """Parse direct MCP output or an HTTP REST tool envelope."""

    return BioevalWaiverAuditReport.from_wire(value)


__all__ = [
    "BIOEVAL_WAIVER_SCHEMA",
    "BIOEVAL_WAIVER_GATE_KINDS",
    "BIOEVAL_WAIVER_VERDICTS",
    "MAX_BIOEVAL_WAIVER_GATES",
    "MAX_BIOEVAL_WAIVER_ROWS",
    "MAX_BIOEVAL_WAIVER_OUTPUT_ITEMS",
    "MAX_BIOEVAL_WAIVER_TEXT_BYTES",
    "MAX_BIOEVAL_WAIVER_INPUT_BYTES",
    "BioevalWaiverGateVerdictArgs",
    "BioevalWaiverGateArgs",
    "BioevalWaiverArgs",
    "BioevalWaiverAuditArgs",
    "BioevalWaiverAuditReport",
    "bioeval_waiver_audit_report",
]
