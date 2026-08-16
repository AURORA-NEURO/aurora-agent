"""Typed routing-decision projections.

Routing selects from a reviewed architecture panel using an evidence ledger.  The projection keeps
abstention and holdout posture explicit: confidence is a routing score, not a posterior, and a
safe-default decision is not evidence that the selected architecture won.
"""

from __future__ import annotations

import json
import math
from dataclasses import dataclass
from typing import Any, Mapping, Sequence

from .capability import _route_count, _route_mapping, _route_strings, _route_text
from .errors import ArgumentError


ARCHITECTURE_KINDS = frozenset(
    {"full_context", "graph_k_hop", "hypergraph_component", "query_graph", "lexical_top_k", "fiber_compiled"}
)
DECISION_REASONS = frozenset({"routed", "insufficient_coverage", "insufficient_margin"})
HOLDOUT_CHECKS = frozenset({"enforced", "caller_must_supply_unseen_identity"})


def _object(name: str, value: Any) -> dict[str, Any]:
    return _route_mapping(name, value)


def _array(name: str, value: Any) -> tuple[Any, ...]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)):
        raise ArgumentError(f"{name} must be an array")
    return tuple(value)


def _bool(name: str, value: Any) -> bool:
    if not isinstance(value, bool):
        raise ArgumentError(f"{name} must be a boolean")
    return value


def _finite(name: str, value: Any) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)) or not math.isfinite(float(value)):
        raise ArgumentError(f"{name} must be a finite number")
    return float(value)


def _probability(name: str, value: Any) -> float:
    result = _finite(name, value)
    if not 0.0 <= result <= 1.0:
        raise ArgumentError(f"{name} must lie in [0, 1]")
    return result


def _payload(value: Mapping[str, Any]) -> dict[str, Any]:
    raw = _route_mapping("routing response", value)

    def matches(candidate: Mapping[str, Any]) -> bool:
        return "ok" in candidate and "decision" in candidate

    if matches(raw):
        return raw
    envelopes: list[Mapping[str, Any]] = [raw]
    mcp = raw.get("mcp")
    if isinstance(mcp, Mapping):
        envelopes.append(mcp)
    for envelope in envelopes:
        result = envelope.get("result")
        candidates: list[Mapping[str, Any]] = [envelope]
        if isinstance(result, Mapping):
            candidates.append(result)
        for candidate in candidates:
            structured = candidate.get("structuredContent")
            if isinstance(structured, Mapping) and matches(structured):
                return dict(structured)
            content = candidate.get("content")
            if not isinstance(content, Sequence) or isinstance(content, (str, bytes)):
                continue
            for block in content:
                if not isinstance(block, Mapping) or not isinstance(block.get("text"), str):
                    continue
                try:
                    decoded = json.loads(block["text"])
                except json.JSONDecodeError as error:
                    raise ArgumentError(f"routing response text is not JSON: {error}") from error
                decoded_mapping = _route_mapping("decoded routing response", decoded)
                if matches(decoded_mapping):
                    return decoded_mapping
    raise ArgumentError("response does not contain a routing decision projection")


def _architecture(name: str, value: Any) -> dict[str, Any]:
    architecture = _object(name, value)
    kind = _route_text(f"{name}.kind", architecture.get("kind"))
    if kind not in ARCHITECTURE_KINDS:
        raise ArgumentError(f"{name}.kind is not a recognized approved architecture")
    if kind == "graph_k_hop":
        depth = architecture.get("depth")
        if isinstance(depth, bool) or not isinstance(depth, int) or depth <= 0:
            raise ArgumentError(f"{name}.depth must be a positive integer")
    if kind == "lexical_top_k":
        k = architecture.get("k")
        if isinstance(k, bool) or not isinstance(k, int) or k <= 0:
            raise ArgumentError(f"{name}.k must be a positive integer")
    return architecture


def _score(name: str, value: Any) -> dict[str, Any]:
    score = _object(name, value)
    _architecture(f"{name}.architecture", score.get("architecture"))
    for field in ("observations", "distinct_tasks"):
        _route_count(f"{name}.{field}", score.get(field))
    _finite(f"{name}.mean_utility", score.get("mean_utility"))
    _probability(f"{name}.admissible_rate", score.get("admissible_rate"))
    return score


def _reason(name: str, value: Any) -> dict[str, Any]:
    reason = _object(name, value)
    tag = _route_text(f"{name}.reason", reason.get("reason"))
    if tag not in DECISION_REASONS:
        raise ArgumentError(f"{name}.reason is not recognized")
    if tag == "routed":
        _finite(f"{name}.margin", reason.get("margin"))
        _route_count(f"{name}.supporting_tasks", reason.get("supporting_tasks"))
        _architecture(f"{name}.runner_up", reason.get("runner_up"))
    elif tag == "insufficient_coverage":
        _route_count(f"{name}.eligible_architectures", reason.get("eligible_architectures"))
        _route_count(f"{name}.neighbouring_observations", reason.get("neighbouring_observations"))
    else:
        _finite(f"{name}.margin", reason.get("margin"))
        _architecture(f"{name}.runner_up", reason.get("runner_up"))
    return reason


@dataclass(frozen=True)
class RoutingDecisionReport:
    raw: dict[str, Any]
    ok: bool
    decision: dict[str, Any]
    architecture: dict[str, Any]
    confidence: float
    abstained: bool
    reason: dict[str, Any]
    considered: tuple[dict[str, Any], ...]
    task_id: str | None
    holdout_check: str
    evidence: dict[str, Any]
    guarantees: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "RoutingDecisionReport":
        raw = _payload(value)
        if not _bool("routing ok", raw.get("ok")):
            raise ArgumentError("routing decision projection must be successful; preserve transport errors raw")
        decision = _object("routing decision", raw.get("decision"))
        architecture = _architecture("routing decision.architecture", decision.get("architecture"))
        confidence = _probability("routing decision.confidence", decision.get("confidence"))
        abstained = _bool("routing decision.abstained", decision.get("abstained"))
        reason = _reason("routing decision.reason", decision.get("reason"))
        if abstained != (reason["reason"] != "routed"):
            raise ArgumentError("routing abstention must agree with its decision reason")
        considered = tuple(_score(f"routing decision.considered[{index}]", item) for index, item in enumerate(_array("routing decision.considered", decision.get("considered"))))
        task_id = None if raw.get("task_id") is None else _route_text("routing task_id", raw.get("task_id"))
        holdout_check = _route_text("routing holdout_check", raw.get("holdout_check"))
        if holdout_check not in HOLDOUT_CHECKS:
            raise ArgumentError("routing holdout_check is not recognized")
        evidence = _object("routing evidence summary", raw.get("evidence"))
        for field in ("observations", "distinct_tasks", "neighbourhood_observations", "neighbourhood_radius"):
            _route_count(f"routing evidence.{field}", evidence.get(field))
        return cls(
            raw=raw,
            ok=True,
            decision=decision,
            architecture=architecture,
            confidence=confidence,
            abstained=abstained,
            reason=reason,
            considered=considered,
            task_id=task_id,
            holdout_check=holdout_check,
            evidence=evidence,
            guarantees=_route_strings("routing guarantees", raw.get("guarantees", ())),
        )

    @property
    def routed(self) -> bool:
        return not self.abstained

    @property
    def safe_default(self) -> bool:
        return self.abstained


def routing_decision_report(value: Mapping[str, Any]) -> RoutingDecisionReport:
    return RoutingDecisionReport.from_wire(value)

