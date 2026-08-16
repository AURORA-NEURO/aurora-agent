"""Typed boundary for modality-aware cross-measurement comparability."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Mapping

from .capability import _route_mapping, _route_text
from .errors import ArgumentError


MODALITY_COMPARABILITY_SCHEMA = "bioprism-mcp/modality-comparability-check/0.1"
MODALITY_COMPARABILITY_OUTCOME_KINDS = frozenset({"comparable", "blocked"})


def _object(name: str, value: Any) -> dict[str, Any]:
    if not isinstance(value, Mapping):
        raise ArgumentError(f"{name} must be an object")
    return dict(value)


@dataclass(frozen=True)
class ModalityComparabilityCheckArgs:
    """Two serialized ``ModalMeasurement`` values and an optional standards policy."""

    left: Mapping[str, Any]
    right: Mapping[str, Any]
    policy: Mapping[str, Any] | None = None

    def __post_init__(self) -> None:
        left = _object("left modality measurement", self.left)
        right = _object("right modality measurement", self.right)
        policy = None if self.policy is None else _object("comparability policy", self.policy)
        object.__setattr__(self, "left", left)
        object.__setattr__(self, "right", right)
        object.__setattr__(self, "policy", policy)

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "ModalityComparabilityCheckArgs":
        raw = _object("modality comparability arguments", value)
        return cls(raw.get("left"), raw.get("right"), raw.get("policy"))

    def to_mcp_arguments(self) -> dict[str, Any]:
        result: dict[str, Any] = {"left": dict(self.left), "right": dict(self.right)}
        if self.policy is not None:
            result["policy"] = dict(self.policy)
        return result


@dataclass(frozen=True)
class ModalityComparabilityCheckReport:
    raw: dict[str, Any]
    ok: bool
    outcome_kind: str
    comparable: bool
    left: dict[str, Any]
    right: dict[str, Any]
    report: dict[str, Any]
    verdict: dict[str, Any]
    report_sha256: str

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "ModalityComparabilityCheckReport":
        raw = _object("modality comparability report", value)
        if raw.get("ok") is not True:
            raise ArgumentError("modality comparability report transport projection is not successful")
        if raw.get("schema") != MODALITY_COMPARABILITY_SCHEMA:
            raise ArgumentError(f"unknown modality comparability schema: {raw.get('schema')!r}")
        outcome_kind = _route_text("modality comparability outcome kind", raw.get("outcome_kind"))
        if outcome_kind not in MODALITY_COMPARABILITY_OUTCOME_KINDS:
            raise ArgumentError(f"unknown modality comparability outcome kind: {outcome_kind!r}")
        comparable = raw.get("comparable")
        if not isinstance(comparable, bool) or comparable != (outcome_kind == "comparable"):
            raise ArgumentError("modality comparability outcome and comparable flag do not reconcile")
        left = _route_mapping("left modality comparability evidence", raw.get("left"))
        right = _route_mapping("right modality comparability evidence", raw.get("right"))
        report = _route_mapping("modality comparability report evidence", raw.get("report"))
        verdict = _route_mapping("modality comparability verdict", raw.get("verdict"))
        digest = _route_text("modality comparability report digest", raw.get("report_sha256"))
        if len(digest) != 64 or any(character not in "0123456789abcdef" for character in digest.lower()):
            raise ArgumentError("modality comparability report digest must be a 64-character hexadecimal SHA-256")
        return cls(raw, True, outcome_kind, comparable, left, right, report, verdict, digest)


def modality_comparability_check_report(value: Mapping[str, Any]) -> ModalityComparabilityCheckReport:
    return ModalityComparabilityCheckReport.from_wire(value)


__all__ = [
    "MODALITY_COMPARABILITY_SCHEMA",
    "MODALITY_COMPARABILITY_OUTCOME_KINDS",
    "ModalityComparabilityCheckArgs",
    "ModalityComparabilityCheckReport",
    "modality_comparability_check_report",
]
