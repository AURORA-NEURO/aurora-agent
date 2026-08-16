"""Typed public-hub disclosure and headline-publication projections.

The public hub has a deliberately asymmetric disclosure model.  A pack is not held out merely
because nobody has reported a leak, contamination is terminal for a digest, a split-integrity
oracle can invalidate a pack without proving that another pack is secret, and scores computed
after disclosure must acknowledge that fact before they can become headline values.  This module
keeps those evidence planes visible while Rust remains the authority for the state machine.

The projection is intentionally richer than a publishable boolean.  Callers can inspect every
ordered action, each fail-closed refusal, the tagged disclosure state and witness, the headline
label/caveat, and the serialized continuation ledger.  That makes it suitable for audits and
replays where a withheld score, an unacknowledged visible benchmark, and a contaminated pack must
not be confused with one another.
"""

from __future__ import annotations

from dataclasses import dataclass
import json
from typing import Any, Mapping, Sequence

from .capability import _route_mapping, _route_strings, _route_text
from .errors import ArgumentError


HUB_DISCLOSURE_SCHEMA = "bioprism-mcp/hub-disclosure/0.1"
HUB_DISCLOSURE_MAX_INPUT_BYTES = 20_000_000
HUB_DISCLOSURE_MAX_ACTIONS = 256
HUB_DISCLOSURE_ACTIONS = frozenset(
    {"declare_held_out", "disclose", "contaminate", "split_integrity", "headline_eligibility"}
)
HUB_DISCLOSURE_STATES = frozenset({"unknown", "held_out", "disclosed", "contaminated"})
HUB_DISCLOSURE_LABELS = frozenset({"held_out", "computed_before_disclosure", "disclosed_pack"})
HUB_CONTAMINATION_KINDS = frozenset(
    {
        "instances_published",
        "solutions_published",
        "training_corpus_overlap",
        "submitter_authored_pack",
        "grader_leak",
        "split_integrity_failure",
    }
)
HUB_ORACLE_STATUSES = frozenset({"valid", "invalid", "underdetermined"})


def _bool(name: str, value: Any) -> bool:
    if not isinstance(value, bool):
        raise ArgumentError(f"{name} must be a boolean")
    return value


def _integer(name: str, value: Any) -> int:
    if not isinstance(value, int) or isinstance(value, bool) or value < 0:
        raise ArgumentError(f"{name} must be a non-negative integer")
    return value


def _sequence(name: str, value: Any) -> tuple[Any, ...]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes, bytearray)):
        raise ArgumentError(f"{name} must be an array")
    return tuple(value)


def _optional_text(name: str, value: Any) -> str | None:
    return None if value is None else _route_text(name, value)


def _payload(value: Mapping[str, Any]) -> dict[str, Any]:
    """Extract the direct projection from direct MCP and REST/MCP envelopes."""

    raw = _route_mapping("hub disclosure response", value)
    candidates: list[Mapping[str, Any]] = [raw]

    def add_container(container: Any) -> None:
        if not isinstance(container, Mapping):
            return
        candidates.append(container)
        nested = container.get("result")
        if isinstance(nested, Mapping):
            candidates.append(nested)
            structured = nested.get("structuredContent")
            if isinstance(structured, Mapping):
                candidates.append(structured)
            content = nested.get("content")
            if isinstance(content, list):
                for block in content:
                    if isinstance(block, Mapping) and isinstance(block.get("text"), str):
                        try:
                            decoded = json.loads(block["text"])
                        except json.JSONDecodeError as error:
                            raise ArgumentError(f"hub disclosure response text is not JSON: {error}") from error
                        if isinstance(decoded, Mapping):
                            candidates.append(decoded)
        structured = container.get("structuredContent")
        if isinstance(structured, Mapping):
            candidates.append(structured)

    add_container(raw.get("mcp"))
    add_container(raw.get("result"))
    add_container(raw.get("structuredContent"))
    for candidate in candidates:
        if (
            candidate.get("ok") is not None
            and candidate.get("schema") == HUB_DISCLOSURE_SCHEMA
            and isinstance(candidate.get("trace"), list)
            and isinstance(candidate.get("entries"), list)
            and isinstance(candidate.get("ledger"), Mapping)
        ):
            return dict(candidate)
    raise ArgumentError("response does not contain a hub disclosure projection")


@dataclass(frozen=True)
class HubDisclosureReviewArgs:
    """Bounded disclosure operations and an optional serialized continuation ledger."""

    actions: tuple[Any, ...] = ()
    ledger: dict[str, Any] | None = None

    def __init__(self, actions: Sequence[Any] = (), ledger: Mapping[str, Any] | None = None) -> None:
        normalized_actions = _sequence("hub disclosure actions", actions)
        if len(normalized_actions) > HUB_DISCLOSURE_MAX_ACTIONS:
            raise ArgumentError("hub disclosure actions must contain at most 256 operations")
        normalized_ledger = None if ledger is None else _route_mapping("hub disclosure ledger", ledger)
        arguments = {"actions": list(normalized_actions), "ledger": normalized_ledger}
        try:
            encoded = json.dumps(arguments, ensure_ascii=False, separators=(",", ":"), allow_nan=False).encode("utf-8")
        except (TypeError, ValueError) as error:
            raise ArgumentError(f"hub disclosure arguments are not JSON serializable: {error}") from error
        if len(encoded) > HUB_DISCLOSURE_MAX_INPUT_BYTES:
            raise ArgumentError("hub disclosure input exceeds the 20 MB safety bound")
        object.__setattr__(self, "actions", normalized_actions)
        object.__setattr__(self, "ledger", normalized_ledger)

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "HubDisclosureReviewArgs":
        raw = _route_mapping("hub disclosure arguments", value)
        return cls(raw.get("actions", []), raw.get("ledger"))

    def to_mcp_arguments(self) -> dict[str, Any]:
        result: dict[str, Any] = {"actions": list(self.actions)}
        if self.ledger is not None:
            result["ledger"] = dict(self.ledger)
        return result


@dataclass(frozen=True)
class HubContaminationWitnessReport:
    raw: dict[str, Any]
    kind: str
    detail: str
    observed_at: int
    reported_by: str

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "HubContaminationWitnessReport":
        raw = _route_mapping("hub contamination witness", value)
        kind = _route_text("hub contamination kind", raw.get("kind"))
        if kind not in HUB_CONTAMINATION_KINDS:
            raise ArgumentError(f"unknown hub contamination kind {kind!r}")
        return cls(raw, kind, _route_text("hub contamination detail", raw.get("detail")), _integer("hub contamination observed_at", raw.get("observed_at")), _route_text("hub contamination reported_by", raw.get("reported_by")))


@dataclass(frozen=True)
class HubDisclosureStateReport:
    raw: dict[str, Any]
    kind: str
    since: int | None
    witness: HubContaminationWitnessReport | None

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "HubDisclosureStateReport":
        raw = _route_mapping("hub disclosure state", value)
        kind = _route_text("hub disclosure state tag", raw.get("disclosure"))
        if kind not in HUB_DISCLOSURE_STATES:
            raise ArgumentError(f"unknown hub disclosure state {kind!r}")
        since = None if raw.get("since") is None else _integer("hub disclosure since", raw.get("since"))
        witness_raw = raw.get("witness")
        witness = None if witness_raw is None else HubContaminationWitnessReport.from_wire(witness_raw)
        if kind == "disclosed" and since is None:
            raise ArgumentError("disclosed hub states must retain since")
        if kind == "contaminated" and witness is None:
            raise ArgumentError("contaminated hub states must retain a witness")
        if kind != "disclosed" and since is not None:
            raise ArgumentError("only disclosed hub states may retain since")
        if kind != "contaminated" and witness is not None:
            raise ArgumentError("only contaminated hub states may retain a witness")
        return cls(raw, kind, since, witness)

    @property
    def headline_blocked(self) -> bool:
        return self.kind in {"unknown", "contaminated"}


@dataclass(frozen=True)
class HubHeadlineLabelReport:
    raw: dict[str, Any]
    kind: str
    disclosed_at: int | None
    caveat: str

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "HubHeadlineLabelReport":
        raw = _route_mapping("hub headline label", value)
        kind = _route_text("hub headline label tag", raw.get("label"))
        if kind not in HUB_DISCLOSURE_LABELS:
            raise ArgumentError(f"unknown hub headline label {kind!r}")
        disclosed_at = None if raw.get("disclosed_at") is None else _integer("hub headline disclosed_at", raw.get("disclosed_at"))
        if kind != "held_out" and disclosed_at is None:
            raise ArgumentError("disclosed headline labels must retain disclosed_at")
        return cls(raw, kind, disclosed_at, _route_text("hub headline caveat", raw.get("caveat")))


@dataclass(frozen=True)
class HubDisclosureActionReport:
    raw: dict[str, Any]
    index: int
    kind: str
    ok: bool
    result: Any
    refusal: str | None
    fail_closed: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "HubDisclosureActionReport":
        raw = _route_mapping("hub disclosure action", value)
        ok = _bool("hub disclosure action ok", raw.get("ok"))
        refusal = _optional_text("hub disclosure action refusal", raw.get("refusal"))
        fail_closed = _bool("hub disclosure action fail_closed", raw.get("fail_closed", False))
        if not ok and (refusal is None or not fail_closed):
            raise ArgumentError("failed hub disclosure actions must be fail-closed")
        return cls(raw, _integer("hub disclosure action index", raw.get("index")), _route_text("hub disclosure action kind", raw.get("kind")), ok, raw.get("result"), refusal, fail_closed)

    @property
    def eligible(self) -> bool | None:
        if not isinstance(self.result, Mapping) or "eligible" not in self.result:
            return None
        return _bool("hub headline eligibility", self.result.get("eligible"))

    @property
    def state(self) -> HubDisclosureStateReport | None:
        if not isinstance(self.result, Mapping):
            return None
        raw_state = self.result.get("state")
        return None if raw_state is None else HubDisclosureStateReport.from_wire(raw_state)

    @property
    def label(self) -> HubHeadlineLabelReport | None:
        if not isinstance(self.result, Mapping):
            return None
        raw_label = self.result.get("label")
        return None if raw_label is None else HubHeadlineLabelReport.from_wire(raw_label)


@dataclass(frozen=True)
class HubDisclosureEntryReport:
    raw: dict[str, Any]
    pack: str
    state: HubDisclosureStateReport

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "HubDisclosureEntryReport":
        raw = _route_mapping("hub disclosure entry", value)
        return cls(raw, _route_text("hub disclosure pack", raw.get("pack")), HubDisclosureStateReport.from_wire(raw.get("state")))


@dataclass(frozen=True)
class HubDisclosureLedgerReport:
    raw: dict[str, Any]
    packs: tuple[tuple[str, HubDisclosureStateReport], ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "HubDisclosureLedgerReport":
        raw = _route_mapping("hub disclosure ledger", value)
        packed = _route_mapping("hub disclosure ledger packs", raw.get("packs", {}))
        rows = tuple((_route_text("hub disclosure ledger digest", pack), HubDisclosureStateReport.from_wire(state)) for pack, state in packed.items())
        return cls(raw, rows)

    def state_for(self, pack: str) -> HubDisclosureStateReport | None:
        return next((state for digest, state in self.packs if digest == pack), None)


@dataclass(frozen=True)
class HubDisclosureReviewReport:
    raw: dict[str, Any]
    ok: bool
    schema: str
    action_count: int
    action_failures: int
    trace: tuple[HubDisclosureActionReport, ...]
    entries: tuple[HubDisclosureEntryReport, ...]
    ledger: HubDisclosureLedgerReport
    guarantees: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "HubDisclosureReviewReport":
        raw = _payload(value)
        trace = tuple(HubDisclosureActionReport.from_wire(item) for item in _sequence("hub disclosure trace", raw.get("trace")))
        entries = tuple(HubDisclosureEntryReport.from_wire(item) for item in _sequence("hub disclosure entries", raw.get("entries")))
        action_count = _integer("hub disclosure action_count", raw.get("action_count"))
        action_failures = _integer("hub disclosure action_failures", raw.get("action_failures"))
        if action_count != len(trace) or action_failures != sum(not item.ok for item in trace):
            raise ArgumentError("hub disclosure action counts do not reconcile")
        return cls(raw, _bool("hub disclosure ok", raw.get("ok")), _route_text("hub disclosure schema", raw.get("schema")), action_count, action_failures, trace, entries, HubDisclosureLedgerReport.from_wire(raw.get("ledger")), _route_strings("hub disclosure guarantees", raw.get("guarantees", [])))

    @property
    def fail_closed_refusal_count(self) -> int:
        return sum(not row.ok and row.fail_closed for row in self.trace)

    @property
    def headline_check_count(self) -> int:
        return sum(row.kind == "headline_eligibility" for row in self.trace)

    @property
    def headline_eligible_count(self) -> int:
        return sum(row.kind == "headline_eligibility" and row.eligible is True for row in self.trace)

    @property
    def headline_withheld_count(self) -> int:
        return sum(row.kind == "headline_eligibility" and row.eligible is False for row in self.trace)

    @property
    def contaminated_count(self) -> int:
        return sum(entry.state.kind == "contaminated" for entry in self.entries)

    @property
    def split_integrity_failure_count(self) -> int:
        return sum(entry.state.witness is not None and entry.state.witness.kind == "split_integrity_failure" for entry in self.entries)

    @property
    def disclosed_count(self) -> int:
        return sum(entry.state.kind == "disclosed" for entry in self.entries)

    @property
    def held_out_count(self) -> int:
        return sum(entry.state.kind == "held_out" for entry in self.entries)

    @property
    def digest_bound(self) -> bool:
        return any("immutable pack digest" in item for item in self.guarantees)

    @property
    def ratchet_is_explicit(self) -> bool:
        return any("ratchet" in item and "contamination" in item for item in self.guarantees)

    @property
    def caveats_are_required(self) -> bool:
        return any("caveat" in item and "bare score" in item for item in self.guarantees)

    @property
    def leak_detection_is_not_claimed(self) -> bool:
        return any("does not detect leaks" in item for item in self.guarantees)

    @property
    def all_refusals_are_fail_closed(self) -> bool:
        return all(row.ok or (row.refusal is not None and row.fail_closed) for row in self.trace)

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


def hub_disclosure_review(value: Mapping[str, Any]) -> HubDisclosureReviewReport:
    """Parse direct MCP output or an HTTP REST tool envelope."""

    return HubDisclosureReviewReport.from_wire(value)


__all__ = [
    "HUB_DISCLOSURE_SCHEMA",
    "HUB_DISCLOSURE_MAX_INPUT_BYTES",
    "HUB_DISCLOSURE_MAX_ACTIONS",
    "HUB_DISCLOSURE_ACTIONS",
    "HUB_DISCLOSURE_STATES",
    "HUB_DISCLOSURE_LABELS",
    "HUB_CONTAMINATION_KINDS",
    "HUB_ORACLE_STATUSES",
    "HubDisclosureReviewArgs",
    "HubContaminationWitnessReport",
    "HubDisclosureStateReport",
    "HubHeadlineLabelReport",
    "HubDisclosureActionReport",
    "HubDisclosureEntryReport",
    "HubDisclosureLedgerReport",
    "HubDisclosureReviewReport",
    "hub_disclosure_review",
]
