"""Typed request and receipt projections for adaptive policy execution.

The MCP endpoint is deliberately conservative: its built-in adapter only simulates caller-scripted
outcomes, while real providers remain outside the server and must implement the typed Rust seam.
This module keeps authorization, partial completion, and observed/simulated/replayed provenance
distinct for Python callers instead of reducing them to one boolean.
"""

from __future__ import annotations

import math
import re
from dataclasses import dataclass
from typing import Any, Mapping, Sequence

from .capability import _route_count, _route_mapping, _route_text
from .errors import ArgumentError

ADAPTIVE_EXECUTION_SCHEMA = "bioprism-epistemic/adaptive-execution/0.1"
ADAPTIVE_COSTED_SCHEMA = "bioprism-mcp/epistemic-adaptive-costed/0.1"
COST_DIMENSIONS = (
    "tokens",
    "compute_ms",
    "latency_ms",
    "money_usd",
    "privacy_loss",
    "specimen_units",
    "expert_minutes",
)
_DIGEST = re.compile(r"^[0-9a-f]{64}$")


def _finite(name: str, value: Any) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)) or not math.isfinite(float(value)):
        raise ArgumentError(f"{name} must be a finite number")
    return float(value)


def _digest(name: str, value: Any) -> str:
    text = _route_text(name, value)
    if not _DIGEST.fullmatch(text):
        raise ArgumentError(f"{name} must be a lowercase 64-character SHA-256 digest")
    return text


def _array(name: str, value: Any) -> list[Any]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)):
        raise ArgumentError(f"{name} must be an array")
    return list(value)


def _payload(value: Mapping[str, Any]) -> dict[str, Any]:
    raw = dict(value)
    if raw.get("schema") in {ADAPTIVE_EXECUTION_SCHEMA, ADAPTIVE_COSTED_SCHEMA}:
        return raw
    mcp = raw.get("mcp")
    if isinstance(mcp, Mapping):
        result = mcp.get("result")
        if isinstance(result, Mapping):
            structured = result.get("structuredContent")
            if isinstance(structured, Mapping):
                return _payload(structured)
            content = result.get("content")
            if isinstance(content, Sequence) and not isinstance(content, (str, bytes)):
                for block in content:
                    if isinstance(block, Mapping) and isinstance(block.get("text"), str):
                        import json

                        decoded = json.loads(block["text"])
                        if isinstance(decoded, Mapping):
                            return _payload(decoded)
    raise ArgumentError("response does not contain an adaptive execution projection")


@dataclass(frozen=True)
class AdaptiveExecutionRequest:
    problem: Mapping[str, Any]
    belief: Mapping[str, Any]
    acquisitions: Sequence[Mapping[str, Any]]
    budget: float
    max_steps: int
    mode: str = "simulate"
    provider: str = "mcp-simulated"
    authorization: Mapping[str, Any] | None = None
    observations: Sequence[Mapping[str, Any]] = ()
    receipt: Mapping[str, Any] | None = None

    def __post_init__(self) -> None:
        if not isinstance(self.problem, Mapping) or not self.problem:
            raise ArgumentError("problem must be a non-empty mapping")
        if not isinstance(self.belief, Mapping) or not self.belief:
            raise ArgumentError("belief must be a non-empty mapping")
        if not isinstance(self.acquisitions, Sequence) or isinstance(self.acquisitions, (str, bytes)) or not 1 <= len(self.acquisitions) <= 16:
            raise ArgumentError("acquisitions must contain 1..=16 rows")
        if any(not isinstance(item, Mapping) for item in self.acquisitions):
            raise ArgumentError("each acquisition must be an object")
        if self.mode not in {"simulate", "replay"}:
            raise ArgumentError("mode must be simulate or replay")
        if not isinstance(self.provider, str) or not self.provider.strip() or len(self.provider) > 256:
            raise ArgumentError("provider must be a visible string of at most 256 bytes")
        if not isinstance(self.max_steps, int) or isinstance(self.max_steps, bool) or not 0 <= self.max_steps <= 16:
            raise ArgumentError("max_steps must be 0..=16")
        _finite("budget", self.budget)
        if self.budget < 0.0:
            raise ArgumentError("budget must be non-negative")
        if not isinstance(self.observations, Sequence) or isinstance(self.observations, (str, bytes)) or len(self.observations) > 16:
            raise ArgumentError("observations must contain at most 16 rows")
        if any(not isinstance(item, Mapping) for item in self.observations):
            raise ArgumentError("each observation must be an object")
        if self.authorization is not None and not isinstance(self.authorization, Mapping):
            raise ArgumentError("authorization must be an object")
        if self.receipt is not None and not isinstance(self.receipt, Mapping):
            raise ArgumentError("receipt must be an object")
        if self.mode == "replay" and self.receipt is None:
            raise ArgumentError("receipt is required in replay mode")

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "AdaptiveExecutionRequest":
        raw = _route_mapping("adaptive execution request", value)
        return cls(
            problem=_route_mapping("adaptive execution problem", raw.get("problem")),
            belief=_route_mapping("adaptive execution belief", raw.get("belief")),
            acquisitions=tuple(_route_mapping("adaptive execution acquisition", item) for item in _array("adaptive execution acquisitions", raw.get("acquisitions"))),
            budget=_finite("adaptive execution budget", raw.get("budget")),
            max_steps=raw.get("max_steps"),
            mode=raw.get("mode", "simulate"),
            provider=raw.get("provider", "mcp-simulated"),
            authorization=dict(raw["authorization"]) if isinstance(raw.get("authorization"), Mapping) else raw.get("authorization"),
            observations=tuple(_route_mapping("adaptive execution observation", item) for item in _array("adaptive execution observations", raw.get("observations", []))),
            receipt=dict(raw["receipt"]) if isinstance(raw.get("receipt"), Mapping) else raw.get("receipt"),
        )

    def to_mcp_arguments(self) -> dict[str, Any]:
        result: dict[str, Any] = {
            "problem": dict(self.problem),
            "belief": dict(self.belief),
            "acquisitions": [dict(item) for item in self.acquisitions],
            "budget": self.budget,
            "max_steps": self.max_steps,
            "mode": self.mode,
            "provider": self.provider,
            "observations": [dict(item) for item in self.observations],
        }
        if self.authorization is not None:
            result["authorization"] = dict(self.authorization)
        if self.receipt is not None:
            result["receipt"] = dict(self.receipt)
        return result


def _cost_vector(name: str, value: Any, *, weights: bool = False) -> dict[str, float]:
    mapping = _route_mapping(name, value)
    result: dict[str, float] = {}
    for dimension in COST_DIMENSIONS:
        if dimension not in mapping:
            raise ArgumentError(f"{name}.{dimension} is required")
        number = _finite(f"{name}.{dimension}", mapping[dimension])
        if number < 0.0:
            raise ArgumentError(f"{name}.{dimension} must be non-negative")
        result[dimension] = number
    if weights and not any(number > 0.0 for number in result.values()):
        raise ArgumentError("cost weights must contain at least one positive dimension")
    return result


@dataclass(frozen=True)
class AdaptiveCostedRequest:
    """Typed request for component-wise feasible adaptive planning."""

    problem: Mapping[str, Any]
    belief: Mapping[str, Any]
    acquisitions: Sequence[Mapping[str, Any]]
    budget: Mapping[str, Any]
    weights: Mapping[str, Any]
    max_steps: int

    def __post_init__(self) -> None:
        if not isinstance(self.problem, Mapping) or not self.problem:
            raise ArgumentError("problem must be a non-empty mapping")
        if not isinstance(self.belief, Mapping) or not self.belief:
            raise ArgumentError("belief must be a non-empty mapping")
        if not isinstance(self.acquisitions, Sequence) or isinstance(self.acquisitions, (str, bytes)) or not 1 <= len(self.acquisitions) <= 16:
            raise ArgumentError("acquisitions must contain 1..=16 rows")
        if any(not isinstance(item, Mapping) for item in self.acquisitions):
            raise ArgumentError("each costed acquisition must be an object")
        _cost_vector("budget", self.budget)
        _cost_vector("weights", self.weights, weights=True)
        if not isinstance(self.max_steps, int) or isinstance(self.max_steps, bool) or not 0 <= self.max_steps <= 16:
            raise ArgumentError("max_steps must be 0..=16")

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "AdaptiveCostedRequest":
        raw = _route_mapping("adaptive costed request", value)
        return cls(
            problem=_route_mapping("adaptive costed problem", raw.get("problem")),
            belief=_route_mapping("adaptive costed belief", raw.get("belief")),
            acquisitions=tuple(_route_mapping("adaptive costed acquisition", item) for item in _array("adaptive costed acquisitions", raw.get("acquisitions"))),
            budget=_cost_vector("adaptive costed budget", raw.get("budget")),
            weights=_cost_vector("adaptive costed weights", raw.get("weights"), weights=True),
            max_steps=raw.get("max_steps"),
        )

    def to_mcp_arguments(self) -> dict[str, Any]:
        return {
            "problem": dict(self.problem),
            "belief": dict(self.belief),
            "acquisitions": [dict(item) for item in self.acquisitions],
            "budget": dict(self.budget),
            "weights": dict(self.weights),
            "max_steps": self.max_steps,
        }


@dataclass(frozen=True)
class AdaptiveObservationReport:
    raw: dict[str, Any]
    sequence: int
    acquisition_id: str
    outcome_label: str
    provider: str
    provenance: str
    evidence_digest: str


@dataclass(frozen=True)
class AdaptiveExecutionReport:
    raw: dict[str, Any]
    schema: str
    mode: str
    plan_digest: str
    provider: str
    status: str
    completed: bool
    observations: tuple[AdaptiveObservationReport, ...]
    actual_acquisition_cost: float
    terminal_action: int | None
    terminal_risk: float | None
    refusal: str | None
    provenance_counts: Mapping[str, int]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "AdaptiveExecutionReport":
        raw = _payload(value)
        if raw.get("ok") is not True:
            raise ArgumentError("adaptive execution response is not successful")
        schema = _route_text("adaptive execution schema", raw.get("schema"))
        if schema != ADAPTIVE_EXECUTION_SCHEMA:
            raise ArgumentError("adaptive execution schema is invalid")
        mode = _route_text("adaptive execution mode", raw.get("mode"))
        if mode not in {"simulate", "replay"}:
            raise ArgumentError("adaptive execution mode is invalid")
        receipt = _route_mapping("adaptive execution receipt", raw.get("receipt"))
        plan_digest = _digest("adaptive execution plan_digest", receipt.get("plan_digest"))
        status = _route_text("adaptive execution status", receipt.get("status"))
        if status not in {"completed", "partial", "refused"}:
            raise ArgumentError("adaptive execution status is invalid")
        completed = raw.get("completed")
        if not isinstance(completed, bool) or completed != (status == "completed"):
            raise ArgumentError("adaptive execution completed flag does not reconcile")
        provider = _route_text("adaptive execution provider", receipt.get("provider"))
        cost = _finite("adaptive execution actual cost", receipt.get("actual_acquisition_cost"))
        if cost < 0.0:
            raise ArgumentError("adaptive execution actual cost must be non-negative")
        rows = _array("adaptive execution observations", receipt.get("observations"))
        if len(rows) > 16:
            raise ArgumentError("adaptive execution observations exceed the exact bound")
        observations: list[AdaptiveObservationReport] = []
        for index, item in enumerate(rows):
            row = _route_mapping("adaptive execution observation receipt", item)
            if row.get("sequence") != index:
                raise ArgumentError("adaptive execution observation sequences must be contiguous")
            request = _route_mapping("adaptive execution observation request", row.get("request"))
            observation = _route_mapping("adaptive execution observation", row.get("observation"))
            if request.get("plan_digest") != plan_digest or request.get("sequence") != index:
                raise ArgumentError("adaptive execution observation request does not bind to the receipt")
            acquisition_id = _route_text("adaptive execution acquisition id", observation.get("acquisition_id"))
            if request.get("acquisition_id") != acquisition_id:
                raise ArgumentError("adaptive execution acquisition identity does not reconcile")
            provenance = _route_text("adaptive execution provenance", observation.get("provenance"))
            if provenance not in {"observed", "simulated", "replayed"}:
                raise ArgumentError("adaptive execution provenance is invalid")
            observations.append(AdaptiveObservationReport(
                raw=row,
                sequence=index,
                acquisition_id=acquisition_id,
                outcome_label=_route_text("adaptive execution outcome label", observation.get("outcome_label")),
                provider=_route_text("adaptive execution observation provider", observation.get("provider")),
                provenance=provenance,
                evidence_digest=_digest("adaptive execution evidence digest", observation.get("evidence_digest")),
            ))
        terminal_action = receipt.get("terminal_action")
        if terminal_action is not None:
            terminal_action = _route_count("adaptive execution terminal action", terminal_action)
        terminal_risk = receipt.get("terminal_risk")
        if terminal_risk is not None:
            terminal_risk = _finite("adaptive execution terminal risk", terminal_risk)
        refusal = receipt.get("refusal")
        if refusal is not None:
            refusal = _route_text("adaptive execution refusal", refusal)
        if status == "completed" and (terminal_action is None or terminal_risk is None or refusal is not None):
            raise ArgumentError("completed adaptive execution must carry a terminal action/risk and no refusal")
        counts = _route_mapping("adaptive execution provenance_counts", raw.get("provenance_counts"))
        normalized_counts = {}
        for name in ("observed", "simulated", "replayed"):
            count = _route_count(f"adaptive execution {name} count", counts.get(name))
            if count != sum(row.provenance == name for row in observations):
                raise ArgumentError("adaptive execution provenance counts do not reconcile")
            normalized_counts[name] = count
        return cls(raw, schema, mode, plan_digest, provider, status, completed, tuple(observations), cost, terminal_action, terminal_risk, refusal, normalized_counts)

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


def adaptive_execution_report(value: Mapping[str, Any]) -> AdaptiveExecutionReport:
    """Parse a direct MCP result or an HTTP REST tool envelope."""

    return AdaptiveExecutionReport.from_wire(value)


@dataclass(frozen=True)
class AdaptiveCostedReport:
    raw: dict[str, Any]
    schema: str
    ok: bool
    cost_dimensions: tuple[str, ...]
    policy: Mapping[str, Any] | None
    refusal: str | None

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "AdaptiveCostedReport":
        raw = _payload(value)
        schema = _route_text("adaptive costed schema", raw.get("schema"))
        if schema != ADAPTIVE_COSTED_SCHEMA:
            raise ArgumentError("adaptive costed schema is invalid")
        ok = raw.get("ok")
        if not isinstance(ok, bool):
            raise ArgumentError("adaptive costed ok must be boolean")
        dimensions = tuple(_route_text("cost dimension", item) for item in _array("cost_dimensions", raw.get("cost_dimensions")))
        if dimensions != COST_DIMENSIONS:
            raise ArgumentError("adaptive cost dimensions are not canonical")
        policy = raw.get("policy")
        if ok and not isinstance(policy, Mapping):
            raise ArgumentError("successful adaptive costed response must contain a policy")
        refusal = raw.get("refusal")
        if not ok and not isinstance(refusal, str) or isinstance(refusal, str) and not refusal:
            raise ArgumentError("refused adaptive costed response must contain a refusal")
        return cls(raw, schema, ok, dimensions, dict(policy) if isinstance(policy, Mapping) else None, refusal)


def adaptive_costed_report(value: Mapping[str, Any]) -> AdaptiveCostedReport:
    """Parse a direct MCP result or HTTP REST envelope for vector-cost planning."""

    return AdaptiveCostedReport.from_wire(value)


__all__ = [
    "ADAPTIVE_EXECUTION_SCHEMA",
    "ADAPTIVE_COSTED_SCHEMA",
    "COST_DIMENSIONS",
    "AdaptiveCostedRequest",
    "AdaptiveCostedReport",
    "adaptive_costed_report",
    "AdaptiveExecutionRequest",
    "AdaptiveObservationReport",
    "AdaptiveExecutionReport",
    "adaptive_execution_report",
]
