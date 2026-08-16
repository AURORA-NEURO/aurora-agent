"""Typed benchmark-gated evolution-card audit requests and projections."""

from __future__ import annotations

import json
from dataclasses import dataclass
from typing import Any, Mapping, Sequence

from .capability import _route_count, _route_mapping, _route_strings, _route_text
from .errors import ArgumentError


LAB_EVOLUTION_SCHEMA = "bioprism-mcp/lab-evolution-audit/0.1"
LAB_EVOLUTION_STATUSES = frozenset({"improvement_claimed", "contaminated", "claim_refused"})
LAB_EVOLUTION_DIRECTIONS = frozenset({"higher_is_better", "lower_is_better"})
MAX_LAB_EVOLUTION_CANDIDATES = 2
MAX_LAB_EVOLUTION_MEASUREMENTS = 256
MAX_LAB_EVOLUTION_ROWS = 1_000
MAX_LAB_EVOLUTION_INPUT_BYTES = 10_000_000


def _array(name: str, value: Any) -> tuple[Any, ...]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)):
        raise ArgumentError(f"{name} must be an array")
    return tuple(value)


def _payload(value: Mapping[str, Any]) -> dict[str, Any]:
    raw = _route_mapping("lab evolution response", value)

    def matches(candidate: Mapping[str, Any]) -> bool:
        if candidate.get("ok") is True:
            return candidate.get("schema") == LAB_EVOLUTION_SCHEMA and isinstance(candidate.get("status"), str)
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
                        raise ArgumentError(f"lab evolution response text is not JSON: {error}") from error
                    if isinstance(decoded, Mapping):
                        candidates.append(decoded)
    for candidate in candidates:
        if matches(candidate):
            return dict(candidate)
    raise ArgumentError("response does not contain a lab evolution projection")


@dataclass(frozen=True)
class LabEvolutionAuditArgs:
    cost_ceiling: int
    candidates: tuple[Mapping[str, Any], ...]
    baseline: str
    candidate: str
    holdout: Mapping[str, Any]
    measurements: tuple[Mapping[str, Any], ...]
    card_id: str
    proposal: Mapping[str, Any]
    rollback_handle: str
    direction: str
    would_have_to_be_true: tuple[str, ...]
    max_rows: int = 100

    def __post_init__(self) -> None:
        if not isinstance(self.cost_ceiling, int) or isinstance(self.cost_ceiling, bool) or not 0 <= self.cost_ceiling <= 1_000_000_000:
            raise ArgumentError("lab evolution cost_ceiling must be between 0 and 1000000000")
        candidates = tuple(_route_mapping(f"lab evolution candidates[{index}]", item) for index, item in enumerate(_array("lab evolution candidates", self.candidates)))
        if len(candidates) != MAX_LAB_EVOLUTION_CANDIDATES:
            raise ArgumentError("lab evolution candidates must contain exactly two objects")
        for index, item in enumerate(candidates):
            _route_text(f"lab evolution candidates[{index}].id", item.get("id"))
        baseline = _route_text("lab evolution baseline", self.baseline)
        candidate = _route_text("lab evolution candidate", self.candidate)
        if baseline == candidate:
            raise ArgumentError("lab evolution baseline and candidate must differ")
        holdout = _route_mapping("lab evolution holdout", self.holdout)
        _route_text("lab evolution holdout.id", holdout.get("id"))
        _route_text("lab evolution holdout.partition", holdout.get("partition"))
        budget = holdout.get("query_budget")
        if not isinstance(budget, int) or isinstance(budget, bool) or budget < 0:
            raise ArgumentError("lab evolution holdout.query_budget must be a non-negative integer")
        measurements = tuple(_route_mapping(f"lab evolution measurements[{index}]", item) for index, item in enumerate(_array("lab evolution measurements", self.measurements)))
        if not 1 <= len(measurements) <= MAX_LAB_EVOLUTION_MEASUREMENTS:
            raise ArgumentError("lab evolution measurements must contain between 1 and 256 objects")
        for index, measurement in enumerate(measurements):
            _route_text(f"lab evolution measurements[{index}].configuration", measurement.get("configuration"))
            _route_text(f"lab evolution measurements[{index}].metric", measurement.get("metric"))
            if isinstance(measurement.get("value"), bool) or not isinstance(measurement.get("value"), (int, float)):
                raise ArgumentError(f"lab evolution measurements[{index}].value must be a number")
        card_id = _route_text("lab evolution card_id", self.card_id)
        proposal = _route_mapping("lab evolution proposal", self.proposal)
        rollback_handle = _route_text("lab evolution rollback_handle", self.rollback_handle)
        direction = _route_text("lab evolution direction", self.direction)
        if direction not in LAB_EVOLUTION_DIRECTIONS:
            raise ArgumentError("lab evolution direction is not recognized")
        defeaters = tuple(_route_text(f"lab evolution defeaters[{index}]", item) for index, item in enumerate(_array("lab evolution would_have_to_be_true", self.would_have_to_be_true)))
        if not 1 <= len(defeaters) <= 128:
            raise ArgumentError("lab evolution defeaters must contain between 1 and 128 statements")
        if any(len(statement.encode("utf-8")) > 2_000 for statement in defeaters):
            raise ArgumentError("lab evolution defeaters must contain at most 2000 bytes each")
        if not isinstance(self.max_rows, int) or isinstance(self.max_rows, bool) or not 1 <= self.max_rows <= MAX_LAB_EVOLUTION_ROWS:
            raise ArgumentError("lab evolution max_rows must be between 1 and 1000")
        arguments = {
            "cost_ceiling": self.cost_ceiling,
            "candidates": [dict(item) for item in candidates],
            "baseline": baseline,
            "candidate": candidate,
            "holdout": holdout,
            "measurements": [dict(item) for item in measurements],
            "card_id": card_id,
            "proposal": proposal,
            "rollback_handle": rollback_handle,
            "direction": direction,
            "would_have_to_be_true": list(defeaters),
            "max_rows": self.max_rows,
        }
        try:
            encoded = json.dumps(arguments, ensure_ascii=False, separators=(",", ":")).encode("utf-8")
        except (TypeError, ValueError) as error:
            raise ArgumentError(f"lab evolution arguments are not JSON serializable: {error}") from error
        if len(encoded) > MAX_LAB_EVOLUTION_INPUT_BYTES:
            raise ArgumentError("lab evolution input exceeds the 10000000-byte safety bound")
        object.__setattr__(self, "candidates", candidates)
        object.__setattr__(self, "baseline", baseline)
        object.__setattr__(self, "candidate", candidate)
        object.__setattr__(self, "holdout", holdout)
        object.__setattr__(self, "measurements", measurements)
        object.__setattr__(self, "card_id", card_id)
        object.__setattr__(self, "proposal", proposal)
        object.__setattr__(self, "rollback_handle", rollback_handle)
        object.__setattr__(self, "direction", direction)
        object.__setattr__(self, "would_have_to_be_true", defeaters)

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "LabEvolutionAuditArgs":
        raw = _route_mapping("lab evolution arguments", value)
        return cls(
            raw.get("cost_ceiling"),
            tuple(_route_mapping(f"lab evolution candidates[{index}]", item) for index, item in enumerate(_array("lab evolution candidates", raw.get("candidates")))),
            raw.get("baseline"),
            raw.get("candidate"),
            _route_mapping("lab evolution holdout", raw.get("holdout")),
            tuple(_route_mapping(f"lab evolution measurements[{index}]", item) for index, item in enumerate(_array("lab evolution measurements", raw.get("measurements")))),
            raw.get("card_id"),
            _route_mapping("lab evolution proposal", raw.get("proposal")),
            raw.get("rollback_handle"),
            raw.get("direction"),
            tuple(_route_text(f"lab evolution defeaters[{index}]", item) for index, item in enumerate(_array("lab evolution would_have_to_be_true", raw.get("would_have_to_be_true")))),
            raw.get("max_rows", 100),
        )

    def to_mcp_arguments(self) -> dict[str, Any]:
        return {
            "cost_ceiling": self.cost_ceiling,
            "candidates": [dict(item) for item in self.candidates],
            "baseline": self.baseline,
            "candidate": self.candidate,
            "holdout": dict(self.holdout),
            "measurements": [dict(item) for item in self.measurements],
            "card_id": self.card_id,
            "proposal": dict(self.proposal),
            "rollback_handle": self.rollback_handle,
            "direction": self.direction,
            "would_have_to_be_true": list(self.would_have_to_be_true),
            "max_rows": self.max_rows,
        }


@dataclass(frozen=True)
class LabEvolutionAuditReport:
    raw: dict[str, Any]
    ok: bool
    schema: str | None
    status: str | None
    claimable: bool
    card: Mapping[str, Any] | None
    claim: Mapping[str, Any] | None
    claim_refusal: str | None
    measurement_count: int | None
    measurement_rows: tuple[Mapping[str, Any], ...]
    measurement_rows_omitted: int
    guarantees: tuple[str, ...]
    limitations: tuple[str, ...]
    stage: str | None
    refusal: str | None
    fail_closed: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "LabEvolutionAuditReport":
        raw = _payload(value)
        if raw.get("ok") is False:
            if raw.get("fail_closed") is not True:
                raise ArgumentError("lab evolution refusals must be fail-closed")
            rows = tuple(_route_mapping("lab evolution refusal measurement row", item) for item in _array("lab evolution refusal measurement rows", raw.get("measurement_rows", [])))
            omitted = _route_count("lab evolution refusal measurement rows omitted", raw.get("measurement_rows_omitted", 0))
            count = _route_count("lab evolution refusal measurement count", raw.get("measurement_count", len(rows) + omitted))
            if len(rows) + omitted != count:
                raise ArgumentError("lab evolution refusal measurement rows do not reconcile")
            return cls(raw, False, raw.get("schema"), None, False, None, None, None, count, rows, omitted, _route_strings("lab evolution refusal guarantees", raw.get("guarantees", [])), (), _route_text("lab evolution refusal stage", raw.get("stage")), _route_text("lab evolution refusal", raw.get("refusal")), True)
        if raw.get("ok") is not True or raw.get("schema") != LAB_EVOLUTION_SCHEMA:
            raise ArgumentError("lab evolution projection has an invalid schema")
        status = _route_text("lab evolution status", raw.get("status"))
        if status not in LAB_EVOLUTION_STATUSES:
            raise ArgumentError("lab evolution status is not recognized")
        claimable = raw.get("claimable")
        if not isinstance(claimable, bool) or claimable != (status == "improvement_claimed"):
            raise ArgumentError("lab evolution claimable flag disagrees with status")
        card = _route_mapping("lab evolution card", raw.get("card"))
        claim = None if raw.get("claim") is None else _route_mapping("lab evolution claim", raw.get("claim"))
        if claimable and claim is None:
            raise ArgumentError("a claimable evolution result must include a claim")
        claim_refusal = None if raw.get("claim_refusal") is None else _route_text("lab evolution claim refusal", raw.get("claim_refusal"))
        if not claimable and not claim_refusal:
            raise ArgumentError("a non-claimable evolution result must include a claim refusal")
        measurement_count = _route_count("lab evolution measurement count", raw.get("measurement_count"))
        rows = tuple(_route_mapping("lab evolution measurement row", item) for item in _array("lab evolution measurement rows", raw.get("measurement_rows", [])))
        omitted = _route_count("lab evolution measurement rows omitted", raw.get("measurement_rows_omitted"))
        if len(rows) + omitted != measurement_count:
            raise ArgumentError("lab evolution measurement rows do not reconcile")
        max_rows = _route_count("lab evolution max_rows", raw.get("max_rows", 100))
        if not 1 <= max_rows <= MAX_LAB_EVOLUTION_ROWS:
            raise ArgumentError("lab evolution max_rows is outside the declared bounds")
        return cls(raw, True, LAB_EVOLUTION_SCHEMA, status, claimable, card, claim, claim_refusal, measurement_count, rows, omitted, _route_strings("lab evolution guarantees", raw.get("guarantees", [])), _route_strings("lab evolution limitations", raw.get("limitations", [])), None, None, False)

    @property
    def accepted(self) -> bool:
        return self.ok

    @property
    def refused(self) -> bool:
        return not self.ok

    @property
    def contaminated(self) -> bool:
        return self.status == "contaminated"

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


def lab_evolution_audit_report(value: Mapping[str, Any]) -> LabEvolutionAuditReport:
    """Parse direct MCP output or an HTTP REST tool envelope."""

    return LabEvolutionAuditReport.from_wire(value)


__all__ = [
    "LAB_EVOLUTION_SCHEMA",
    "LAB_EVOLUTION_STATUSES",
    "LAB_EVOLUTION_DIRECTIONS",
    "MAX_LAB_EVOLUTION_CANDIDATES",
    "MAX_LAB_EVOLUTION_MEASUREMENTS",
    "MAX_LAB_EVOLUTION_ROWS",
    "MAX_LAB_EVOLUTION_INPUT_BYTES",
    "LabEvolutionAuditArgs",
    "LabEvolutionAuditReport",
    "lab_evolution_audit_report",
]
