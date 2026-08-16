"""Typed prospective seal/reveal audits."""

from __future__ import annotations

import json
from dataclasses import dataclass
from typing import Any, Mapping, Sequence

from .capability import _route_mapping, _route_strings, _route_text
from .errors import ArgumentError


BIOEVAL_REVEAL_SCHEMA = "bioprism-mcp/bioeval-reveal-audit/0.1"
MAX_BIOEVAL_REVEAL_COMMITMENTS = 4_096
MAX_BIOEVAL_REVEAL_OUTCOMES = 4_096
MAX_BIOEVAL_REVEAL_OUTPUT_ITEMS = 1_000
MAX_BIOEVAL_REVEAL_ID_BYTES = 256
MAX_BIOEVAL_REVEAL_TEXT_BYTES = 4_096
MAX_BIOEVAL_REVEAL_INPUT_BYTES = 20_000_000


def _text(name: str, value: Any, limit: int = MAX_BIOEVAL_REVEAL_TEXT_BYTES) -> str:
    if not isinstance(value, str) or not value.strip():
        raise ArgumentError(f"{name} must be a non-empty string")
    if len(value.encode("utf-8")) > limit:
        raise ArgumentError(f"{name} exceeds {limit} UTF-8 bytes")
    return value


def _array(name: str, value: Any) -> tuple[Any, ...]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)):
        raise ArgumentError(f"{name} must be an array")
    return tuple(value)


def _payload(value: Mapping[str, Any]) -> dict[str, Any]:
    raw = _route_mapping("bioeval reveal response", value)

    def matches(candidate: Mapping[str, Any]) -> bool:
        if candidate.get("ok") is True:
            return candidate.get("schema") == BIOEVAL_REVEAL_SCHEMA and isinstance(candidate.get("scoring"), Mapping)
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
                        raise ArgumentError(f"bioeval reveal response text is not JSON: {error}") from error
                    if isinstance(decoded, Mapping):
                        candidates.append(decoded)
    for candidate in candidates:
        if matches(candidate):
            return dict(candidate)
    raise ArgumentError("response does not contain a bioeval reveal projection")


@dataclass(frozen=True)
class BioevalRevealCommitmentArgs:
    target: str
    prediction: Any
    analysis_plan: str

    def __post_init__(self) -> None:
        object.__setattr__(self, "target", _text("bioeval reveal commitment target", self.target, MAX_BIOEVAL_REVEAL_ID_BYTES))
        object.__setattr__(self, "analysis_plan", _text("bioeval reveal analysis plan", self.analysis_plan))
        try:
            json.dumps(self.prediction, ensure_ascii=False, separators=(",", ":"))
        except (TypeError, ValueError) as error:
            raise ArgumentError(f"bioeval reveal prediction must be JSON-compatible: {error}") from error

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "BioevalRevealCommitmentArgs":
        raw = _route_mapping("bioeval reveal commitment", value)
        return cls(_text("bioeval reveal commitment target", raw.get("target"), MAX_BIOEVAL_REVEAL_ID_BYTES), raw.get("prediction"), _text("bioeval reveal analysis plan", raw.get("analysis_plan")))

    def to_wire(self) -> dict[str, Any]:
        return {"target": self.target, "prediction": self.prediction, "analysis_plan": self.analysis_plan}


@dataclass(frozen=True)
class BioevalRevealOutcomeArgs:
    target: str
    observed: Any

    def __post_init__(self) -> None:
        object.__setattr__(self, "target", _text("bioeval reveal outcome target", self.target, MAX_BIOEVAL_REVEAL_ID_BYTES))
        try:
            json.dumps(self.observed, ensure_ascii=False, separators=(",", ":"))
        except (TypeError, ValueError) as error:
            raise ArgumentError(f"bioeval reveal observed value must be JSON-compatible: {error}") from error

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "BioevalRevealOutcomeArgs":
        raw = _route_mapping("bioeval reveal outcome", value)
        return cls(_text("bioeval reveal outcome target", raw.get("target"), MAX_BIOEVAL_REVEAL_ID_BYTES), raw.get("observed"))

    def to_wire(self) -> dict[str, Any]:
        return {"target": self.target, "observed": self.observed}


@dataclass(frozen=True)
class BioevalRevealAuditArgs:
    study: str
    commitments: tuple[BioevalRevealCommitmentArgs, ...]
    rubric: Any
    sealed_at: str
    outcomes: tuple[BioevalRevealOutcomeArgs, ...] = ()
    score_rubric: Any = None
    require_scoring: bool = False
    require_rubric_match: bool = False
    require_complete: bool = False

    def __post_init__(self) -> None:
        study = _text("bioeval reveal study", self.study, MAX_BIOEVAL_REVEAL_ID_BYTES)
        sealed_at = _text("bioeval reveal sealed_at", self.sealed_at)
        commitments = tuple(item if isinstance(item, BioevalRevealCommitmentArgs) else BioevalRevealCommitmentArgs.from_wire(item) for item in self.commitments)
        if not commitments or len(commitments) > MAX_BIOEVAL_REVEAL_COMMITMENTS:
            raise ArgumentError("bioeval reveal commitments must contain 1 to 4096 rows")
        if len({item.target for item in commitments}) != len(commitments):
            raise ArgumentError("bioeval reveal commitment targets must be unique")
        outcomes = tuple(item if isinstance(item, BioevalRevealOutcomeArgs) else BioevalRevealOutcomeArgs.from_wire(item) for item in self.outcomes)
        if len(outcomes) > MAX_BIOEVAL_REVEAL_OUTCOMES:
            raise ArgumentError("bioeval reveal outcomes are bounded at 4096 rows")
        if len({item.target for item in outcomes}) != len(outcomes):
            raise ArgumentError("bioeval reveal outcome targets must be unique")
        for name, value in (("rubric", self.rubric), ("score_rubric", self.score_rubric)):
            if value is None and name == "rubric":
                continue
            try:
                json.dumps(value, ensure_ascii=False, separators=(",", ":"))
            except (TypeError, ValueError) as error:
                raise ArgumentError(f"bioeval reveal {name} must be JSON-compatible: {error}") from error
        for name in ("require_scoring", "require_rubric_match", "require_complete"):
            if not isinstance(getattr(self, name), bool):
                raise ArgumentError(f"bioeval reveal {name} must be a boolean")
        object.__setattr__(self, "study", study)
        object.__setattr__(self, "sealed_at", sealed_at)
        object.__setattr__(self, "commitments", commitments)
        object.__setattr__(self, "outcomes", outcomes)
        encoded = json.dumps(self.to_mcp_arguments(), ensure_ascii=False, separators=(",", ":")).encode("utf-8")
        if len(encoded) > MAX_BIOEVAL_REVEAL_INPUT_BYTES:
            raise ArgumentError("bioeval reveal input exceeds the 20000000-byte safety bound")

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "BioevalRevealAuditArgs":
        raw = _route_mapping("bioeval reveal arguments", value)
        return cls(
            _text("bioeval reveal study", raw.get("study"), MAX_BIOEVAL_REVEAL_ID_BYTES),
            tuple(BioevalRevealCommitmentArgs.from_wire(item) for item in _array("bioeval reveal commitments", raw.get("commitments"))),
            raw.get("rubric"),
            _text("bioeval reveal sealed_at", raw.get("sealed_at")),
            tuple(BioevalRevealOutcomeArgs.from_wire(item) for item in _array("bioeval reveal outcomes", raw.get("outcomes", []))),
            raw.get("score_rubric"),
            raw.get("require_scoring", False),
            raw.get("require_rubric_match", False),
            raw.get("require_complete", False),
        )

    def to_mcp_arguments(self) -> dict[str, Any]:
        result: dict[str, Any] = {
            "study": self.study,
            "commitments": [item.to_wire() for item in self.commitments],
            "rubric": self.rubric,
            "sealed_at": self.sealed_at,
            "outcomes": [item.to_wire() for item in self.outcomes],
            "require_scoring": self.require_scoring,
            "require_rubric_match": self.require_rubric_match,
            "require_complete": self.require_complete,
        }
        if self.score_rubric is not None:
            result["score_rubric"] = self.score_rubric
        return result


@dataclass(frozen=True)
class BioevalRevealAuditReport:
    raw: dict[str, Any]
    ok: bool
    schema: str | None
    workflow: str | None
    study: str | None
    sealed_at: str | None
    digests: Mapping[str, Any] | None
    commitments: Mapping[str, Any] | None
    outcomes: Mapping[str, Any] | None
    seal_lock: Mapping[str, Any] | None
    reveal_lock: Mapping[str, Any] | None
    scoring: Mapping[str, Any] | None
    findings: Mapping[str, Any] | None
    stage: str | None
    refusal: str | None
    guarantees: tuple[str, ...]
    limitations: tuple[str, ...]
    fail_closed: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "BioevalRevealAuditReport":
        raw = _payload(value)
        if raw.get("ok") is False:
            if raw.get("fail_closed") is not True:
                raise ArgumentError("bioeval reveal refusals must be fail-closed")
            return cls(
                raw,
                False,
                raw.get("schema"),
                raw.get("workflow"),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                _route_text("bioeval reveal refusal stage", raw.get("stage")),
                _route_text("bioeval reveal refusal", raw.get("refusal")),
                _route_strings("bioeval reveal refusal guarantees", raw.get("guarantees", [])),
                _route_strings("bioeval reveal refusal limitations", raw.get("limitations", [])),
                True,
            )
        if raw.get("ok") is not True or raw.get("schema") != BIOEVAL_REVEAL_SCHEMA:
            raise ArgumentError("bioeval reveal projection has an invalid schema")
        return cls(raw, True, BIOEVAL_REVEAL_SCHEMA, _route_text("bioeval reveal workflow", raw.get("workflow")), _route_text("bioeval reveal study", raw.get("study")), _route_text("bioeval reveal sealed_at", raw.get("sealed_at")), _route_mapping("bioeval reveal digests", raw.get("digests")), _route_mapping("bioeval reveal commitments", raw.get("commitments")), _route_mapping("bioeval reveal outcomes", raw.get("outcomes")), _route_mapping("bioeval reveal seal lock", raw.get("seal_lock")), _route_mapping("bioeval reveal reveal lock", raw.get("reveal_lock")), _route_mapping("bioeval reveal scoring", raw.get("scoring")), _route_mapping("bioeval reveal findings", raw.get("findings")), None, None, _route_strings("bioeval reveal guarantees", raw.get("guarantees", [])), _route_strings("bioeval reveal limitations", raw.get("limitations", [])), False)

    @property
    def accepted(self) -> bool:
        return self.ok

    @property
    def refused(self) -> bool:
        return not self.ok

    @property
    def selective_publication(self) -> bool | None:
        if self.findings is None:
            return None
        value = self.findings.get("selective_publication")
        return value if isinstance(value, bool) else None

    @property
    def rubric_match_refused(self) -> bool | None:
        if self.findings is None:
            return None
        value = self.findings.get("rubric_match_refused")
        return value if isinstance(value, bool) else None

    @property
    def unrevealed_targets(self) -> tuple[str, ...]:
        if self.findings is None or not isinstance(self.findings.get("unrevealed_commitments"), Mapping):
            return ()
        values = self.findings["unrevealed_commitments"].get("ids", [])
        return tuple(value for value in values if isinstance(value, str)) if isinstance(values, Sequence) and not isinstance(values, (str, bytes)) else ()

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


def bioeval_reveal_audit_report(value: Mapping[str, Any]) -> BioevalRevealAuditReport:
    """Parse direct MCP output or an HTTP REST tool envelope."""

    return BioevalRevealAuditReport.from_wire(value)


__all__ = [
    "BIOEVAL_REVEAL_SCHEMA",
    "MAX_BIOEVAL_REVEAL_COMMITMENTS",
    "MAX_BIOEVAL_REVEAL_OUTCOMES",
    "MAX_BIOEVAL_REVEAL_OUTPUT_ITEMS",
    "MAX_BIOEVAL_REVEAL_ID_BYTES",
    "MAX_BIOEVAL_REVEAL_TEXT_BYTES",
    "MAX_BIOEVAL_REVEAL_INPUT_BYTES",
    "BioevalRevealCommitmentArgs",
    "BioevalRevealOutcomeArgs",
    "BioevalRevealAuditArgs",
    "BioevalRevealAuditReport",
    "bioeval_reveal_audit_report",
]
