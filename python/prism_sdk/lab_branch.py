"""Typed risk-triggered branch-audit requests and ledger projections."""

from __future__ import annotations

import json
import math
from dataclasses import dataclass
from typing import Any, Mapping, Sequence

from .capability import _route_count, _route_mapping, _route_strings, _route_text
from .errors import ArgumentError


LAB_BRANCH_SCHEMA = "bioprism-mcp/lab-branch-audit/0.1"
LAB_BRANCH_VERDICTS = frozenset({"nothing_triggered", "paid_and_caught_nothing", "mixed", "every_escalation_caught_something"})
MAX_LAB_BRANCH_DECISIONS = 512
MAX_LAB_BRANCH_ROWS = 1_000
MAX_LAB_BRANCH_INPUT_BYTES = 10_000_000


def _array(name: str, value: Any) -> tuple[Any, ...]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)):
        raise ArgumentError(f"{name} must be an array")
    return tuple(value)


def _payload(value: Mapping[str, Any]) -> dict[str, Any]:
    raw = _route_mapping("lab branch response", value)

    def matches(candidate: Mapping[str, Any]) -> bool:
        if candidate.get("ok") is True:
            return candidate.get("schema") == LAB_BRANCH_SCHEMA and isinstance(candidate.get("yield"), Mapping)
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
                        raise ArgumentError(f"lab branch response text is not JSON: {error}") from error
                    if isinstance(decoded, Mapping):
                        candidates.append(decoded)
    for candidate in candidates:
        if matches(candidate):
            return dict(candidate)
    raise ArgumentError("response does not contain a lab branch projection")


def _finite_optional(name: str, value: Any) -> None:
    if value is not None and (isinstance(value, bool) or not isinstance(value, (int, float)) or not math.isfinite(float(value))):
        raise ArgumentError(f"{name} must be finite when supplied")


@dataclass(frozen=True)
class LabBranchAuditArgs:
    policy: Mapping[str, Any]
    decisions: tuple[Mapping[str, Any], ...]
    max_rows: int = 100

    def __post_init__(self) -> None:
        policy = _route_mapping("lab branch policy", self.policy)
        decisions = tuple(
            _route_mapping(f"lab branch decisions[{index}]", item)
            for index, item in enumerate(_array("lab branch decisions", self.decisions))
        )
        if not 1 <= len(decisions) <= MAX_LAB_BRANCH_DECISIONS:
            raise ArgumentError("lab branch decisions must contain between 1 and 512 objects")
        for index, decision in enumerate(decisions):
            label = _route_text(f"lab branch decisions[{index}].decision", decision.get("decision"))
            if len(label.encode("utf-8")) > 512:
                raise ArgumentError("lab branch decision labels must contain at most 512 bytes")
            _route_mapping(f"lab branch decisions[{index}].features", decision.get("features"))
            caught = decision.get("caught")
            if caught is not None:
                caught_object = _route_mapping(f"lab branch decisions[{index}].caught", caught)
                _route_text(f"lab branch decisions[{index}].caught.what", caught_object.get("what"))
                _route_text(f"lab branch decisions[{index}].caught.would_have_been", caught_object.get("would_have_been"))
            if "escaped" in decision:
                _route_text(f"lab branch decisions[{index}].escaped", decision.get("escaped"))
        if not isinstance(self.max_rows, int) or isinstance(self.max_rows, bool) or not 1 <= self.max_rows <= MAX_LAB_BRANCH_ROWS:
            raise ArgumentError("lab branch max_rows must be between 1 and 1000")
        arguments = {"policy": policy, "decisions": [dict(item) for item in decisions], "max_rows": self.max_rows}
        try:
            encoded = json.dumps(arguments, ensure_ascii=False, separators=(",", ":")).encode("utf-8")
        except (TypeError, ValueError) as error:
            raise ArgumentError(f"lab branch arguments are not JSON serializable: {error}") from error
        if len(encoded) > MAX_LAB_BRANCH_INPUT_BYTES:
            raise ArgumentError("lab branch input exceeds the 10000000-byte safety bound")
        object.__setattr__(self, "policy", policy)
        object.__setattr__(self, "decisions", decisions)

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "LabBranchAuditArgs":
        raw = _route_mapping("lab branch arguments", value)
        return cls(
            _route_mapping("lab branch policy", raw.get("policy")),
            tuple(_route_mapping(f"lab branch decisions[{index}]", item) for index, item in enumerate(_array("lab branch decisions", raw.get("decisions")))),
            raw.get("max_rows", 100),
        )

    def to_mcp_arguments(self) -> dict[str, Any]:
        return {"policy": dict(self.policy), "decisions": [dict(item) for item in self.decisions], "max_rows": self.max_rows}


@dataclass(frozen=True)
class LabBranchAuditReport:
    raw: dict[str, Any]
    ok: bool
    schema: str | None
    decision_count: int | None
    policy: Mapping[str, Any] | None
    yielded: Mapping[str, Any] | None
    verdict: str | None
    rows: tuple[Mapping[str, Any], ...]
    rows_omitted: int
    guarantees: tuple[str, ...]
    limitations: tuple[str, ...]
    stage: str | None
    refusal: str | None
    fail_closed: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "LabBranchAuditReport":
        raw = _payload(value)
        if raw.get("ok") is False:
            if raw.get("fail_closed") is not True:
                raise ArgumentError("lab branch refusals must be fail-closed")
            return cls(raw, False, raw.get("schema"), None, None, None, None, (), 0, _route_strings("lab branch refusal guarantees", raw.get("guarantees", [])), (), _route_text("lab branch refusal stage", raw.get("stage")), _route_text("lab branch refusal", raw.get("refusal")), True)
        if raw.get("ok") is not True or raw.get("schema") != LAB_BRANCH_SCHEMA:
            raise ArgumentError("lab branch projection has an invalid schema")
        decision_count = _route_count("lab branch decision count", raw.get("decision_count"))
        if not 1 <= decision_count <= MAX_LAB_BRANCH_DECISIONS:
            raise ArgumentError("lab branch decision count is outside the declared bounds")
        policy = _route_mapping("lab branch policy", raw.get("policy"))
        yielded = _route_mapping("lab branch yield", raw.get("yield"))
        for field in ("decisions", "escalations", "escalations_on_undetermined", "catches", "wasted_escalations", "escaped_after_escalation", "escaped_without_escalation"):
            _route_count(f"lab branch yield.{field}", yielded.get(field))
        _finite_optional("lab branch yield.branches_per_catch", yielded.get("branches_per_catch"))
        if yielded.get("branches_per_catch") is not None and float(yielded["branches_per_catch"]) < 0:
            raise ArgumentError("lab branch yield.branches_per_catch must be non-negative")
        if yielded["decisions"] != decision_count:
            raise ArgumentError("lab branch yield decisions do not reconcile with decision_count")
        verdict_object = _route_mapping("lab branch verdict", raw.get("verdict"))
        verdict = _route_text("lab branch verdict label", verdict_object.get("verdict"))
        if verdict not in LAB_BRANCH_VERDICTS:
            raise ArgumentError("lab branch verdict label is not recognized")
        rows = tuple(_route_mapping("lab branch row", item) for item in _array("lab branch rows", raw.get("rows", [])))
        rows_omitted = _route_count("lab branch rows omitted", raw.get("rows_omitted"))
        if len(rows) + rows_omitted != decision_count:
            raise ArgumentError("lab branch rows do not reconcile with decision_count")
        max_rows = _route_count("lab branch max_rows", raw.get("max_rows"))
        if not 1 <= max_rows <= MAX_LAB_BRANCH_ROWS:
            raise ArgumentError("lab branch max_rows is outside the declared bounds")
        return cls(raw, True, LAB_BRANCH_SCHEMA, decision_count, policy, yielded, verdict, rows, rows_omitted, _route_strings("lab branch guarantees", raw.get("guarantees", [])), _route_strings("lab branch limitations", raw.get("limitations", [])), None, None, False)

    @property
    def accepted(self) -> bool:
        return self.ok

    @property
    def refused(self) -> bool:
        return not self.ok

    @property
    def paid_and_caught_nothing(self) -> bool:
        return self.verdict == "paid_and_caught_nothing"

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


def lab_branch_audit_report(value: Mapping[str, Any]) -> LabBranchAuditReport:
    """Parse direct MCP output or an HTTP REST tool envelope."""

    return LabBranchAuditReport.from_wire(value)


__all__ = [
    "LAB_BRANCH_SCHEMA",
    "LAB_BRANCH_VERDICTS",
    "MAX_LAB_BRANCH_DECISIONS",
    "MAX_LAB_BRANCH_ROWS",
    "MAX_LAB_BRANCH_INPUT_BYTES",
    "LabBranchAuditArgs",
    "LabBranchAuditReport",
    "lab_branch_audit_report",
]
