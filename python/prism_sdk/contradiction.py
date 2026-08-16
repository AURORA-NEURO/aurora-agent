"""Typed multimodal contradiction-review contracts.

Contradiction review is a set-valued workflow, not a winner-selection endpoint.  The SDK keeps
pose/validation/examination refusals distinct, preserves typed hypothesis and action projections,
and refuses to collapse ``not_yet_examined`` into ``unresolvable``.
"""

from __future__ import annotations

import json
from dataclasses import dataclass
from typing import Any, Mapping, Sequence

from .capability import _route_count, _route_mapping, _route_strings, _route_text
from .errors import ArgumentError


CONTRADICTION_INTENTS = frozenset({"expected", "resolvable", "irreducible"})
CONTRADICTION_STATES = frozenset({"resolved", "not_yet_examined", "unresolvable"})
CONTRADICTION_CUES = frozenset(
    {
        "account_named_in_annotation",
        "intent_named_in_annotation",
        "sole_annotated_reading",
        "sole_declared_floor_matches_account",
    }
)


def _bool(name: str, value: Any) -> bool:
    if not isinstance(value, bool):
        raise ArgumentError(f"{name} must be a boolean")
    return value


def _optional_text(name: str, value: Any) -> str | None:
    return None if value is None else _route_text(name, value)


def _array(name: str, value: Any) -> tuple[Any, ...]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)):
        raise ArgumentError(f"{name} must be an array")
    return tuple(value)


def _mapping_array(name: str, value: Any) -> tuple[dict[str, Any], ...]:
    return tuple(_route_mapping(f"{name}[{index}]", item) for index, item in enumerate(_array(name, value)))


def _payload(value: Mapping[str, Any]) -> dict[str, Any]:
    raw = _route_mapping("contradiction response", value)
    if "ok" in raw and any(key in raw for key in ("validated", "stage", "contradiction")):
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
            if isinstance(structured, Mapping) and "ok" in structured:
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
                    raise ArgumentError(f"contradiction response text is not JSON: {error}") from error
                decoded_mapping = _route_mapping("decoded contradiction response", decoded)
                if "ok" in decoded_mapping:
                    return decoded_mapping
    raise ArgumentError("response does not contain a contradiction projection")


@dataclass(frozen=True)
class ContradictionReviewArgs:
    left: Mapping[str, Any]
    right: Mapping[str, Any]
    intent: str
    hypotheses: tuple[Mapping[str, Any], ...]
    actions: tuple[Mapping[str, Any], ...] = ()
    missing_evidence: tuple[Mapping[str, Any], ...] = ()
    references: tuple[Mapping[str, Any], ...] = ()
    examine: tuple[str, ...] = ()
    notable_below_per_ten_thousand: int | None = None
    max_items: int = 100

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "ContradictionReviewArgs":
        raw = _route_mapping("contradiction arguments", value)
        return cls(
            raw.get("left"),
            raw.get("right"),
            _route_text("contradiction intent", raw.get("intent")),
            _mapping_array("contradiction hypotheses", raw.get("hypotheses")),
            _mapping_array("contradiction actions", raw.get("actions", [])),
            _mapping_array("contradiction missing_evidence", raw.get("missing_evidence", [])),
            _mapping_array("contradiction references", raw.get("references", [])),
            tuple(_route_text(f"contradiction examine[{index}]", item) for index, item in enumerate(_array("contradiction examine", raw.get("examine", [])))),
            raw.get("notable_below_per_ten_thousand"),
            raw.get("max_items", 100),
        )

    def __post_init__(self) -> None:
        object.__setattr__(self, "left", _route_mapping("contradiction left reading", self.left))
        object.__setattr__(self, "right", _route_mapping("contradiction right reading", self.right))
        if self.intent not in CONTRADICTION_INTENTS:
            raise ArgumentError(f"unknown contradiction intent: {self.intent!r}")
        if not self.hypotheses:
            raise ArgumentError("contradiction hypotheses must contain at least one account")
        for name, items, maximum in (
            ("hypotheses", self.hypotheses, 1_000),
            ("actions", self.actions, 1_000),
            ("missing_evidence", self.missing_evidence, 1_000),
            ("references", self.references, 1_000),
            ("examine", self.examine, 1_000),
        ):
            if len(items) > maximum:
                raise ArgumentError(f"contradiction {name} may contain at most {maximum} entries")
        hypothesis_ids = [
            _route_text(f"contradiction hypotheses[{index}].id", item.get("id"))
            for index, item in enumerate(self.hypotheses)
        ]
        if len(hypothesis_ids) != len(set(hypothesis_ids)):
            raise ArgumentError("contradiction hypotheses must have unique ids")
        action_ids = [
            _route_text(f"contradiction actions[{index}].evidence", item.get("evidence"))
            for index, item in enumerate(self.actions)
        ]
        if len(action_ids) != len(set(action_ids)):
            raise ArgumentError("contradiction actions must have unique evidence ids")
        if len(self.examine) != len(set(self.examine)):
            raise ArgumentError("contradiction examine must have unique evidence ids")
        if self.notable_below_per_ten_thousand is not None and (
            isinstance(self.notable_below_per_ten_thousand, bool)
            or not isinstance(self.notable_below_per_ten_thousand, int)
            or self.notable_below_per_ten_thousand < 0
        ):
            raise ArgumentError("contradiction notable threshold must be non-negative")
        if isinstance(self.max_items, bool) or not isinstance(self.max_items, int) or not 1 <= self.max_items <= 1_000:
            raise ArgumentError("contradiction max_items must be between 1 and 1000")

    def to_mcp_arguments(self) -> dict[str, Any]:
        result: dict[str, Any] = {
            "left": dict(self.left),
            "right": dict(self.right),
            "intent": self.intent,
            "hypotheses": [dict(item) for item in self.hypotheses],
            "actions": [dict(item) for item in self.actions],
            "missing_evidence": [dict(item) for item in self.missing_evidence],
            "references": [dict(item) for item in self.references],
            "examine": list(self.examine),
            "max_items": self.max_items,
        }
        if self.notable_below_per_ten_thousand is not None:
            result["notable_below_per_ten_thousand"] = self.notable_below_per_ten_thousand
        return result


@dataclass(frozen=True)
class ContradictionReadingReport:
    raw: dict[str, Any]
    modality: str
    quantity: str
    lens: Mapping[str, Any]
    scope: Mapping[str, Any]
    reported: Mapping[str, Any]
    annotations: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "ContradictionReadingReport":
        raw = _route_mapping("contradiction reading", value)
        return cls(
            raw,
            _route_text("contradiction reading modality", raw.get("modality")),
            _route_text("contradiction reading quantity", raw.get("quantity")),
            _route_mapping("contradiction reading lens", raw.get("lens")),
            _route_mapping("contradiction reading scope", raw.get("scope")),
            _route_mapping("contradiction reading reported", raw.get("reported")),
            _route_strings("contradiction reading annotations", raw.get("annotations", [])),
        )


@dataclass(frozen=True)
class ContradictionHypothesisReport:
    raw: dict[str, Any]
    id: str
    account: Mapping[str, Any]
    account_kind: str

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "ContradictionHypothesisReport":
        raw = _route_mapping("contradiction hypothesis", value)
        account = _route_mapping("contradiction hypothesis account", raw.get("account"))
        account_kind = _route_text("contradiction hypothesis kind", account.get("discordance"))
        return cls(raw, _route_text("contradiction hypothesis id", raw.get("id")), account, account_kind)


@dataclass(frozen=True)
class ContradictionActionReport:
    raw: dict[str, Any]
    evidence: str
    refutes: tuple[str, ...]
    cost: int
    refutes_live: int | None

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "ContradictionActionReport":
        raw = _route_mapping("contradiction action", value)
        refutes_live = raw.get("refutes_live")
        if refutes_live is not None:
            refutes_live = _route_count("contradiction action refutes_live", refutes_live)
        return cls(
            raw,
            _route_text("contradiction action evidence", raw.get("evidence")),
            _route_strings("contradiction action refutes", raw.get("refutes", [])),
            _route_count("contradiction action cost", raw.get("cost")),
            refutes_live,
        )


@dataclass(frozen=True)
class ContradictionStateReport:
    raw: dict[str, Any]
    state: str
    available: tuple[ContradictionActionReport, ...]
    examined: tuple[str, ...]
    would_resolve: tuple[Mapping[str, Any], ...]
    by: tuple[str, ...]
    surviving: tuple[ContradictionHypothesisReport, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "ContradictionStateReport":
        raw = _route_mapping("contradiction state", value)
        state = _route_text("contradiction state kind", raw.get("state"))
        if state not in CONTRADICTION_STATES:
            raise ArgumentError(f"unknown contradiction state: {state!r}")
        available = tuple(ContradictionActionReport.from_wire(item) for item in _array("contradiction available actions", raw.get("available", [])))
        examined = _route_strings("contradiction examined evidence", raw.get("examined", []))
        would = tuple(_route_mapping(f"contradiction would_resolve[{index}]", item) for index, item in enumerate(_array("contradiction would_resolve", raw.get("would_resolve", []))))
        by = _route_strings("contradiction resolved by", raw.get("by", []))
        surviving_raw = raw.get("surviving", {})
        if isinstance(surviving_raw, Mapping) and not ("id" in surviving_raw and "account" in surviving_raw):
            surviving_items = tuple(surviving_raw.values())
        else:
            surviving_items = _array("contradiction surviving", surviving_raw)
        surviving = tuple(ContradictionHypothesisReport.from_wire(item) for item in surviving_items)
        if state == "not_yet_examined" and not available:
            raise ArgumentError("not_yet_examined contradiction state must expose available actions")
        if state == "resolved" and not by:
            raise ArgumentError("resolved contradiction state must name examined evidence")
        if state == "unresolvable" and available:
            raise ArgumentError("unresolvable contradiction state cannot retain unexamined actions")
        return cls(raw, state, available, examined, would, by, surviving)


@dataclass(frozen=True)
class ContradictionExpectednessReport:
    raw: dict[str, Any]
    ok: bool
    value: Mapping[str, Any] | None
    kind: str | None
    threshold: int
    refusal: str | None
    fail_closed: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "ContradictionExpectednessReport":
        raw = _route_mapping("contradiction expectedness", value)
        ok = _bool("contradiction expectedness ok", raw.get("ok"))
        threshold = _route_count("contradiction expectedness threshold", raw.get("threshold"))
        if ok:
            result = _route_mapping("contradiction expectedness value", raw.get("value"))
            kind = _route_text("contradiction expectedness kind", result.get("expectedness"))
            if kind not in {"routine", "notable"}:
                raise ArgumentError(f"unknown contradiction expectedness: {kind!r}")
            if raw.get("fail_closed", False):
                raise ArgumentError("successful contradiction expectedness cannot fail closed")
            return cls(raw, True, result, kind, threshold, None, False)
        refusal = _route_text("contradiction expectedness refusal", raw.get("refusal"))
        fail_closed = _bool("contradiction expectedness fail_closed", raw.get("fail_closed"))
        if not fail_closed:
            raise ArgumentError("refused contradiction expectedness must fail closed")
        return cls(raw, False, None, None, threshold, refusal, True)


@dataclass(frozen=True)
class ContradictionReviewReport:
    raw: dict[str, Any]
    ok: bool
    validated: bool
    stage: str | None
    refusal: str | None
    fail_closed: bool
    contradiction: Mapping[str, Any] | None
    intent: str | None
    declared_hypothesis_count: int | None
    admissible_hypothesis_count: int | None
    admissible_hypotheses: tuple[ContradictionHypothesisReport, ...]
    validation_intent_check: Mapping[str, Any] | None
    post_examination_intent_check: Mapping[str, Any] | None
    examined: tuple[str, ...]
    state: ContradictionStateReport | None
    state_name: str | None
    live_hypothesis_count: int | None
    next_actions: tuple[ContradictionActionReport, ...]
    omitted_next_actions: int
    cue_count: int
    cues: tuple[Mapping[str, Any], ...]
    omitted_cues: int
    expectedness: ContradictionExpectednessReport | None
    guarantees: tuple[str, ...]
    limitations: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "ContradictionReviewReport":
        raw = _payload(value)
        ok = _bool("contradiction ok", raw.get("ok"))
        validated = _bool("contradiction validated", raw.get("validated", False))
        stage = _optional_text("contradiction stage", raw.get("stage"))
        refusal = _optional_text("contradiction refusal", raw.get("refusal"))
        fail_closed_value = raw.get("fail_closed", False)
        fail_closed = _bool("contradiction fail_closed", fail_closed_value)
        if not ok:
            if refusal is None or not fail_closed:
                raise ArgumentError("refused contradiction review requires a fail-closed refusal")
            state_value = raw.get("state")
            state = None if state_value is None else ContradictionStateReport.from_wire(state_value)
            examined = _route_strings("contradiction examined", raw.get("examined", []))
            return cls(
                raw=raw,
                ok=False,
                validated=validated,
                stage=stage,
                refusal=refusal,
                fail_closed=True,
                contradiction=None,
                intent=None,
                declared_hypothesis_count=None,
                admissible_hypothesis_count=None,
                admissible_hypotheses=(),
                validation_intent_check=None,
                post_examination_intent_check=None,
                examined=examined,
                state=state,
                state_name=None,
                live_hypothesis_count=None,
                next_actions=(),
                omitted_next_actions=0,
                cue_count=0,
                cues=(),
                omitted_cues=0,
                expectedness=None,
                guarantees=(),
                limitations=(),
            )
        if not validated or stage is not None or refusal is not None or fail_closed:
            raise ArgumentError("successful contradiction review must be fully validated and non-refused")
        contradiction = _route_mapping("contradiction returned contradiction", raw.get("contradiction"))
        intent = _route_text("contradiction returned intent", raw.get("intent"))
        if intent not in CONTRADICTION_INTENTS:
            raise ArgumentError(f"unknown contradiction returned intent: {intent!r}")
        admissible_raw = _route_mapping("contradiction admissible hypotheses", raw.get("admissible_hypotheses"))
        admissible = tuple(ContradictionHypothesisReport.from_wire({"id": key, "account": item}) for key, item in admissible_raw.items())
        declared_count = _route_count("contradiction declared_hypothesis_count", raw.get("declared_hypothesis_count"))
        admissible_count = _route_count("contradiction admissible_hypothesis_count", raw.get("admissible_hypothesis_count"))
        if admissible_count != len(admissible) or admissible_count > declared_count:
            raise ArgumentError("contradiction hypothesis counts do not reconcile")
        validation_check = _route_mapping("contradiction validation_intent_check", raw.get("validation_intent_check"))
        post_check = _route_mapping("contradiction post_examination_intent_check", raw.get("post_examination_intent_check"))
        examined = _route_strings("contradiction examined", raw.get("examined", []))
        state = ContradictionStateReport.from_wire(raw.get("state"))
        state_name = _route_text("contradiction state_name", raw.get("state_name"))
        if state_name != state.state:
            raise ArgumentError("contradiction state_name does not reconcile with state")
        live_count = _route_count("contradiction live_hypothesis_count", raw.get("live_hypothesis_count"))
        if state.state == "resolved" and live_count != len(state.surviving):
            raise ArgumentError("contradiction live hypothesis count does not reconcile with state")
        next_raw = _array("contradiction next_actions", raw.get("next_actions"))
        next_actions = tuple(ContradictionActionReport.from_wire(item) for item in next_raw)
        omitted_next = _route_count("contradiction omitted_next_actions", raw.get("omitted_next_actions"))
        cue_count = _route_count("contradiction cue_count", raw.get("cue_count"))
        cues = tuple(_route_mapping(f"contradiction cues[{index}]", item) for index, item in enumerate(_array("contradiction cues", raw.get("cues"))))
        for cue in cues:
            cue_kind = _route_text("contradiction cue kind", cue.get("cue"))
            if cue_kind not in CONTRADICTION_CUES:
                raise ArgumentError(f"unknown contradiction cue: {cue_kind!r}")
        omitted_cues = _route_count("contradiction omitted_cues", raw.get("omitted_cues"))
        if len(cues) != min(cue_count, len(cues)) or len(cues) + omitted_cues != cue_count:
            raise ArgumentError("contradiction cue counts do not reconcile")
        expected_value = raw.get("expectedness")
        expectedness = None if expected_value is None else ContradictionExpectednessReport.from_wire(expected_value)
        return cls(raw, True, True, None, None, False, contradiction, intent, declared_count, admissible_count, admissible, validation_check, post_check, examined, state, state_name, live_count, next_actions, omitted_next, cue_count, cues, omitted_cues, expectedness, _route_strings("contradiction guarantees", raw.get("guarantees")), _route_strings("contradiction limitations", raw.get("limitations")))

    @property
    def refused(self) -> bool:
        return not self.ok

    @property
    def resolution_pending(self) -> bool:
        return self.state_name == "not_yet_examined"

    @property
    def unresolvable(self) -> bool:
        return self.state_name == "unresolvable"


def contradiction_review_report(value: Mapping[str, Any]) -> ContradictionReviewReport:
    """Parse direct MCP or HTTP contradiction-review output."""

    return ContradictionReviewReport.from_wire(value)


__all__ = [
    "CONTRADICTION_CUES",
    "CONTRADICTION_INTENTS",
    "CONTRADICTION_STATES",
    "ContradictionActionReport",
    "ContradictionExpectednessReport",
    "ContradictionHypothesisReport",
    "ContradictionReadingReport",
    "ContradictionReviewArgs",
    "ContradictionReviewReport",
    "ContradictionStateReport",
    "contradiction_review_report",
]
