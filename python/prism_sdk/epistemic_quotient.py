"""Typed decision-equivalence quotient projections.

The quotient is deliberately narrower than model equivalence: the Rust kernel compares only the
loss-difference profiles of the explicitly permitted actions. This module validates that boundary
and keeps a structured refusal distinct from a compressed result.
"""

from __future__ import annotations

import json
import math
from dataclasses import dataclass
from typing import Any, Mapping, Sequence

from .capability import _route_count, _route_mapping, _route_strings, _route_text
from .errors import ArgumentError
from .epistemic import EpistemicDecisionProblemArgs, EpistemicRefusalReport


EPISTEMIC_QUOTIENT_SCHEMA = "bioprism-mcp/epistemic-decision-quotient/0.1"
EPISTEMIC_QUOTIENT_KERNEL_SCHEMA = "bioprism-epistemic-decision-quotient/0.1"
EPISTEMIC_QUOTIENT_BASIS = "permitted_loss_difference_profile"
EPISTEMIC_MAX_PERMITTED_ACTIONS = 1_000
EPISTEMIC_QUOTIENT_MAX_INPUT_BYTES = 20_000_000


def _array(name: str, value: Any) -> tuple[Any, ...]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)):
        raise ArgumentError(f"{name} must be an array")
    return tuple(value)


def _finite(name: str, value: Any) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)) or not math.isfinite(float(value)):
        raise ArgumentError(f"{name} must be a finite number")
    return float(value)


def _payload(value: Mapping[str, Any]) -> dict[str, Any]:
    raw = _route_mapping("epistemic decision quotient response", value)

    def matches(candidate: Mapping[str, Any]) -> bool:
        if candidate.get("ok") is True:
            return candidate.get("schema") == EPISTEMIC_QUOTIENT_SCHEMA and isinstance(candidate.get("quotient"), Mapping)
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
                        raise ArgumentError(f"epistemic decision quotient response text is not JSON: {error}") from error
                    if isinstance(decoded, Mapping):
                        candidates.append(decoded)
    for candidate in candidates:
        if matches(candidate):
            return dict(candidate)
    raise ArgumentError("response does not contain an epistemic decision quotient projection")


@dataclass(frozen=True)
class EpistemicDecisionQuotientArgs:
    """Explicit decision problem plus the action boundary it is allowed to distinguish."""

    problem: EpistemicDecisionProblemArgs
    permitted_actions: tuple[str, ...]

    def __post_init__(self) -> None:
        problem = self.problem if isinstance(self.problem, EpistemicDecisionProblemArgs) else EpistemicDecisionProblemArgs.from_wire(self.problem)
        actions = tuple(_route_text(f"epistemic permitted_actions[{index}]", value) for index, value in enumerate(self.permitted_actions))
        if not 1 <= len(actions) <= EPISTEMIC_MAX_PERMITTED_ACTIONS:
            raise ArgumentError("epistemic permitted_actions must contain between 1 and 1000 actions")
        if any(len(action.encode("utf-8")) > 256 for action in actions):
            raise ArgumentError("epistemic permitted action names must contain at most 256 UTF-8 bytes")
        if len(actions) != len(set(actions)):
            raise ArgumentError("epistemic permitted_actions must be unique")
        unknown = sorted(set(actions).difference(problem.actions))
        if unknown:
            raise ArgumentError(f"epistemic permitted_actions contain unknown actions: {unknown!r}")
        encoded = json.dumps({"problem": problem.to_wire(), "permitted_actions": list(actions)}, ensure_ascii=False, separators=(",", ":")).encode("utf-8")
        if len(encoded) > EPISTEMIC_QUOTIENT_MAX_INPUT_BYTES:
            raise ArgumentError("epistemic decision quotient input exceeds the 20000000-byte safety bound")
        object.__setattr__(self, "problem", problem)
        object.__setattr__(self, "permitted_actions", actions)

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "EpistemicDecisionQuotientArgs":
        raw = _route_mapping("epistemic decision quotient arguments", value)
        return cls(
            EpistemicDecisionProblemArgs.from_wire(raw.get("problem")),
            tuple(_route_text(f"epistemic permitted_actions[{index}]", item) for index, item in enumerate(_array("epistemic permitted_actions", raw.get("permitted_actions")))),
        )

    def to_mcp_arguments(self) -> dict[str, Any]:
        return {"problem": self.problem.to_wire(), "permitted_actions": list(self.permitted_actions)}


@dataclass(frozen=True)
class EpistemicDecisionQuotientClass:
    class_index: int
    representative_model: str
    members: tuple[str, ...]
    loss_differences: Mapping[str, float]
    preferred_actions: tuple[str, ...]


@dataclass(frozen=True)
class EpistemicDecisionQuotientReport:
    """Validated quotient evidence, including the exact compression counts."""

    raw: dict[str, Any]
    ok: bool
    quotient: Mapping[str, Any] | None
    classes: tuple[EpistemicDecisionQuotientClass, ...]
    original_model_count: int | None
    quotient_model_count: int | None
    merged_model_count: int | None
    stage: str | None
    refusal: EpistemicRefusalReport | None
    guarantees: tuple[str, ...]
    limitations: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "EpistemicDecisionQuotientReport":
        raw = _payload(value)
        if raw.get("ok") is False:
            refusal = EpistemicRefusalReport.from_wire(raw)
            return cls(raw, False, None, (), None, None, None, refusal.stage, refusal, refusal.guarantees, _route_strings("epistemic quotient refusal limitations", raw.get("limitations", [])))
        if raw.get("ok") is not True or raw.get("schema") != EPISTEMIC_QUOTIENT_SCHEMA:
            raise ArgumentError("epistemic decision quotient projection has an invalid schema")
        quotient = _route_mapping("epistemic decision quotient", raw.get("quotient"))
        if quotient.get("schema_version") != EPISTEMIC_QUOTIENT_KERNEL_SCHEMA:
            raise ArgumentError("epistemic decision quotient kernel schema is invalid")
        if quotient.get("basis") != EPISTEMIC_QUOTIENT_BASIS:
            raise ArgumentError("epistemic decision quotient basis is invalid")
        actions = _route_strings("epistemic quotient permitted actions", quotient.get("permitted_actions"))
        if tuple(actions) != tuple(sorted(actions)) or len(actions) != len(set(actions)) or not actions:
            raise ArgumentError("epistemic quotient permitted actions must be non-empty, unique, and canonical")
        original = _route_count("epistemic quotient original model count", quotient.get("original_model_count"))
        count = _route_count("epistemic quotient model count", quotient.get("quotient_model_count"))
        merged = _route_count("epistemic quotient merged model count", quotient.get("merged_model_count"))
        if count == 0 or count > original or merged != original - count:
            raise ArgumentError("epistemic quotient model counts do not reconcile")
        raw_classes = _array("epistemic quotient classes", quotient.get("classes"))
        if len(raw_classes) != count:
            raise ArgumentError("epistemic quotient class count does not reconcile")
        classes: list[EpistemicDecisionQuotientClass] = []
        seen_models: set[str] = set()
        for expected_index, item in enumerate(raw_classes):
            row = _route_mapping("epistemic quotient class", item)
            class_index = _route_count("epistemic quotient class index", row.get("class_index"))
            members = _route_strings("epistemic quotient class members", row.get("members"))
            if class_index != expected_index or not members or tuple(members) != tuple(sorted(members)) or len(members) != len(set(members)):
                raise ArgumentError("epistemic quotient class indexes or members are not canonical")
            if seen_models.intersection(members):
                raise ArgumentError("epistemic quotient classes repeat a model")
            seen_models.update(members)
            representative = _route_text("epistemic quotient representative model", row.get("representative_model"))
            if representative != members[0]:
                raise ArgumentError("epistemic quotient representative is not the lexical first member")
            profile_raw = _route_mapping("epistemic quotient loss differences", row.get("loss_differences"))
            if tuple(profile_raw) != tuple(actions):
                raise ArgumentError("epistemic quotient loss profile does not cover exactly the permitted actions")
            profile = {action: _finite(f"epistemic quotient loss difference {action!r}", profile_raw[action]) for action in actions}
            preferred = _route_strings("epistemic quotient preferred actions", row.get("preferred_actions"))
            if any(action not in actions for action in preferred) or tuple(preferred) != tuple(sorted(preferred)):
                raise ArgumentError("epistemic quotient preferred actions cross the permitted boundary")
            classes.append(EpistemicDecisionQuotientClass(class_index, representative, tuple(members), profile, tuple(preferred)))
        mapping = _route_mapping("epistemic quotient model mapping", quotient.get("model_to_class"))
        if set(mapping) != seen_models:
            raise ArgumentError("epistemic quotient model mapping does not cover exactly the class members")
        for model, class_index in mapping.items():
            if _route_count(f"epistemic quotient mapping for {model!r}", class_index) >= count or model not in classes[class_index].members:
                raise ArgumentError("epistemic quotient model mapping points outside its class")
        summary = _route_mapping("epistemic quotient summary", raw.get("summary"))
        if _route_count("epistemic quotient summary original count", summary.get("original_model_count")) != original or _route_count("epistemic quotient summary count", summary.get("quotient_model_count")) != count or _route_count("epistemic quotient summary merged count", summary.get("merged_model_count")) != merged:
            raise ArgumentError("epistemic quotient summary does not reconcile")
        return cls(raw, True, quotient, tuple(classes), original, count, merged, None, None, _route_strings("epistemic quotient guarantees", raw.get("guarantees", [])), _route_strings("epistemic quotient limitations", raw.get("limitations", [])))

    @property
    def accepted(self) -> bool:
        return self.ok

    @property
    def refused(self) -> bool:
        return not self.ok

    @property
    def compressed(self) -> bool | None:
        if self.original_model_count is None or self.quotient_model_count is None:
            return None
        return self.quotient_model_count < self.original_model_count

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


def epistemic_decision_quotient_report(value: Mapping[str, Any]) -> EpistemicDecisionQuotientReport:
    """Parse direct MCP output or an HTTP REST tool envelope."""

    return EpistemicDecisionQuotientReport.from_wire(value)


__all__ = [
    "EPISTEMIC_QUOTIENT_SCHEMA",
    "EPISTEMIC_QUOTIENT_KERNEL_SCHEMA",
    "EPISTEMIC_QUOTIENT_BASIS",
    "EpistemicDecisionQuotientArgs",
    "EpistemicDecisionQuotientClass",
    "EpistemicDecisionQuotientReport",
    "epistemic_decision_quotient_report",
]
