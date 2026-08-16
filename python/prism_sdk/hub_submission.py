"""Typed submission-acceptance and public moderation-ledger projections.

The hub submission endpoint has two distinct phases: contract acceptance creates a submission
object, and an optional append-only moderation replay changes its standing.  This module keeps
those phases and every refusal stage visible.  It does not authenticate submitters, verify
provenance externally, persist a ledger, or publish a network page.
"""

from __future__ import annotations

from dataclasses import dataclass
import json
from typing import Any, Mapping, Sequence

from .capability import _route_mapping, _route_strings, _route_text
from .errors import ArgumentError


HUB_SUBMISSION_SCHEMA = "bioprism-mcp/hub-submission/0.1"
HUB_SUBMISSION_MAX_INPUT_BYTES = 20_000_000
HUB_MODERATION_MAX_ACTIONS = 32
HUB_MODERATION_STATES = frozenset({"submitted", "under-review", "accepted", "rejected", "withdrawn", "superseded"})
HUB_VERIFICATION_STATES = frozenset({"self-reported", "reproduced", "verified", "prospectively-validated"})
HUB_EVENT_KINDS = frozenset({"opened", "transition", "attestation"})
HUB_SUBMISSION_STAGES = frozenset({"submission_acceptance", "moderation_open", "moderation_transition", "moderation_attestation", "moderation_attestation_revocation", "moderation_ledger"})


def _bool(name: str, value: Any) -> bool:
    if not isinstance(value, bool):
        raise ArgumentError(f"{name} must be a boolean")
    return value


def _integer(name: str, value: Any) -> int:
    if not isinstance(value, int) or isinstance(value, bool) or value < 0:
        raise ArgumentError(f"{name} must be a non-negative integer")
    return value


def _optional_text(name: str, value: Any) -> str | None:
    return None if value is None else _route_text(name, value)


def _sequence(name: str, value: Any) -> tuple[Any, ...]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes, bytearray)):
        raise ArgumentError(f"{name} must be an array")
    return tuple(value)


def _payload(value: Mapping[str, Any]) -> dict[str, Any]:
    raw = _route_mapping("hub submission response", value)
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
                            raise ArgumentError(f"hub submission response text is not JSON: {error}") from error
                        if isinstance(decoded, Mapping):
                            candidates.append(decoded)
        structured = container.get("structuredContent")
        if isinstance(structured, Mapping):
            candidates.append(structured)

    add_container(raw.get("mcp"))
    add_container(raw.get("result"))
    add_container(raw.get("structuredContent"))
    for candidate in candidates:
        if candidate.get("schema") == HUB_SUBMISSION_SCHEMA and "ok" in candidate and "stage" in candidate:
            return dict(candidate)
    for candidate in candidates:
        if "ok" in candidate and "stage" in candidate and "submission" in candidate:
            return dict(candidate)
    raise ArgumentError("response does not contain a hub submission projection")


@dataclass(frozen=True)
class HubSubmissionReviewArgs:
    draft: dict[str, Any]
    submitter: dict[str, Any]
    moderation: dict[str, Any] | None = None

    def __init__(self, draft: Mapping[str, Any], submitter: Mapping[str, Any], moderation: Mapping[str, Any] | None = None) -> None:
        normalized_draft = _route_mapping("hub submission draft", draft)
        normalized_submitter = _route_mapping("hub submission submitter", submitter)
        normalized_moderation = None if moderation is None else _route_mapping("hub submission moderation", moderation)
        arguments = {"draft": normalized_draft, "submitter": normalized_submitter, "moderation": normalized_moderation}
        try:
            encoded = json.dumps(arguments, ensure_ascii=False, separators=(",", ":"), allow_nan=False).encode("utf-8")
        except (TypeError, ValueError) as error:
            raise ArgumentError(f"hub submission arguments are not JSON serializable: {error}") from error
        if len(encoded) > HUB_SUBMISSION_MAX_INPUT_BYTES:
            raise ArgumentError("hub submission input exceeds the 20 MB safety bound")
        object.__setattr__(self, "draft", normalized_draft)
        object.__setattr__(self, "submitter", normalized_submitter)
        object.__setattr__(self, "moderation", normalized_moderation)

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "HubSubmissionReviewArgs":
        raw = _route_mapping("hub submission arguments", value)
        return cls(raw.get("draft"), raw.get("submitter"), raw.get("moderation"))

    def to_mcp_arguments(self) -> dict[str, Any]:
        result = {"draft": dict(self.draft), "submitter": dict(self.submitter)}
        if self.moderation is not None:
            result["moderation"] = dict(self.moderation)
        return result


@dataclass(frozen=True)
class HubModerationEventReport:
    raw: dict[str, Any]
    submission: str
    kind: str
    actor: str
    at: int
    reason: str | None
    superseded_by: str | None
    from_state: str | None
    to_state: str | None
    from_verification: str | None
    to_verification: str | None

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "HubModerationEventReport":
        raw = _route_mapping("hub moderation event", value)
        raw_kind = raw.get("kind")
        event_fields = raw_kind if isinstance(raw_kind, Mapping) else raw
        kind = _route_text("hub moderation event kind", event_fields.get("kind") if isinstance(raw_kind, Mapping) else raw_kind)
        if kind not in HUB_EVENT_KINDS:
            raise ArgumentError(f"unknown hub moderation event kind {kind!r}")
        from_state = _optional_text("hub moderation from state", event_fields.get("from"))
        to_state = _optional_text("hub moderation to state", event_fields.get("to"))
        from_verification = _optional_text("hub moderation from verification", event_fields.get("from")) if kind == "attestation" else None
        to_verification = _optional_text("hub moderation to verification", event_fields.get("to")) if kind == "attestation" else None
        if kind == "transition":
            if from_state not in HUB_MODERATION_STATES or to_state not in HUB_MODERATION_STATES:
                raise ArgumentError("transition moderation events must retain valid from/to states")
        if kind == "attestation":
            if from_verification not in HUB_VERIFICATION_STATES or to_verification not in HUB_VERIFICATION_STATES:
                raise ArgumentError("attestation events must retain valid verification states")
        return cls(raw, _route_text("hub moderation submission", raw.get("submission")), kind, _route_text("hub moderation actor", raw.get("actor")), _integer("hub moderation event at", raw.get("at")), _optional_text("hub moderation reason", raw.get("reason")), _optional_text("hub superseded_by", raw.get("superseded_by")), from_state if kind == "transition" else None, to_state if kind == "transition" else None, from_verification, to_verification)


@dataclass(frozen=True)
class HubTombstoneReport:
    raw: dict[str, Any]
    submission: str
    submitter: str
    content: str
    withdrawn_at: int
    actor: str
    reason: str
    states_traversed: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "HubTombstoneReport":
        raw = _route_mapping("hub tombstone", value)
        states = tuple(_route_text("hub tombstone state", item) for item in _sequence("hub tombstone states", raw.get("states_traversed", [])))
        if any(state not in HUB_MODERATION_STATES for state in states):
            raise ArgumentError("hub tombstones must retain valid moderation states")
        return cls(raw, _route_text("hub tombstone submission", raw.get("submission")), _route_text("hub tombstone submitter", raw.get("submitter")), _route_text("hub tombstone content", raw.get("content")), _integer("hub tombstone withdrawn_at", raw.get("withdrawn_at")), _route_text("hub tombstone actor", raw.get("actor")), _route_text("hub tombstone reason", raw.get("reason")), states)


@dataclass(frozen=True)
class HubModerationRecordReport:
    raw: dict[str, Any]
    submission: dict[str, Any]
    state: str
    verification: str
    history: tuple[HubModerationEventReport, ...]
    tombstone: HubTombstoneReport | None

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "HubModerationRecordReport":
        raw = _route_mapping("hub moderation record", value)
        state = _route_text("hub moderation record state", raw.get("state"))
        verification = _route_text("hub moderation record verification", raw.get("verification"))
        if state not in HUB_MODERATION_STATES:
            raise ArgumentError(f"unknown hub moderation state {state!r}")
        if verification not in HUB_VERIFICATION_STATES:
            raise ArgumentError(f"unknown hub verification state {verification!r}")
        tombstone_raw = raw.get("tombstone")
        tombstone = None if tombstone_raw is None else HubTombstoneReport.from_wire(tombstone_raw)
        if state == "withdrawn" and tombstone is None:
            raise ArgumentError("withdrawn moderation records must retain a tombstone")
        return cls(raw, _route_mapping("hub moderation submission record", raw.get("submission")), state, verification, tuple(HubModerationEventReport.from_wire(item) for item in _sequence("hub moderation history", raw.get("history", []))), tombstone)


@dataclass(frozen=True)
class HubModerationLedgerReport:
    raw: dict[str, Any]
    records: tuple[tuple[str, HubModerationRecordReport], ...]
    events: tuple[HubModerationEventReport, ...]
    last_epoch: int

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "HubModerationLedgerReport":
        raw = _route_mapping("hub moderation ledger", value)
        raw_records = _route_mapping("hub moderation records", raw.get("records", {}))
        records = tuple((_route_text("hub moderation record id", key), HubModerationRecordReport.from_wire(item)) for key, item in raw_records.items())
        events = tuple(HubModerationEventReport.from_wire(item) for item in _sequence("hub moderation events", raw.get("events", [])))
        last_epoch = _integer("hub moderation last_epoch", raw.get("last_epoch"))
        if len(events) < sum(len(record.history) for _, record in records):
            raise ArgumentError("moderation ledger events cannot omit record history")
        return cls(raw, records, events, last_epoch)

    @property
    def published_ids(self) -> tuple[str, ...]:
        return tuple(key for key, record in self.records if record.state == "accepted")

    @property
    def withdrawn_count(self) -> int:
        return sum(record.state == "withdrawn" for _, record in self.records)


@dataclass(frozen=True)
class HubSubmissionReviewReport:
    raw: dict[str, Any]
    ok: bool
    schema: str
    stage: str
    submission: dict[str, Any] | None
    limitation_card: str | None
    moderation: HubModerationLedgerReport | None
    state: str | None
    verification: str | None
    published: tuple[str, ...]
    event_count: int
    refusal: str | None
    fail_closed: bool
    guarantees: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "HubSubmissionReviewReport":
        raw = _payload(value)
        ok = _bool("hub submission ok", raw.get("ok"))
        stage = _route_text("hub submission stage", raw.get("stage"))
        if stage not in HUB_SUBMISSION_STAGES:
            raise ArgumentError(f"unknown hub submission stage {stage!r}")
        refusal = _optional_text("hub submission refusal", raw.get("refusal"))
        fail_closed = _bool("hub submission fail_closed", raw.get("fail_closed", False))
        if not ok and (refusal is None or not fail_closed):
            raise ArgumentError("failed hub submission reviews must be fail-closed")
        submission_raw = raw.get("submission")
        submission = None if submission_raw is None else _route_mapping("hub accepted submission", submission_raw)
        ledger_raw = raw.get("ledger")
        ledger = None if ledger_raw is None else HubModerationLedgerReport.from_wire(ledger_raw)
        state = _optional_text("hub submission state", raw.get("state"))
        verification = _optional_text("hub submission verification", raw.get("verification"))
        if state is not None and state not in HUB_MODERATION_STATES:
            raise ArgumentError(f"unknown hub submission state {state!r}")
        if verification is not None and verification not in HUB_VERIFICATION_STATES:
            raise ArgumentError(f"unknown hub submission verification {verification!r}")
        published = tuple(_route_text("hub published submission", item) for item in _sequence("hub published submissions", raw.get("published", [])))
        event_count = _integer("hub submission event_count", raw.get("event_count", 0))
        if ledger is not None and event_count != len(ledger.events):
            raise ArgumentError("hub submission event_count does not reconcile with ledger")
        return cls(raw, ok, _route_text("hub submission schema", raw.get("schema", HUB_SUBMISSION_SCHEMA)), stage, submission, _optional_text("hub limitation card", raw.get("limitation_card")), ledger, state, verification, published, event_count, refusal, fail_closed, _route_strings("hub submission guarantees", raw.get("guarantees", [])))

    @property
    def accepted(self) -> bool:
        return self.submission is not None and self.refusal is None

    @property
    def moderation_replayed(self) -> bool:
        return self.moderation is not None

    @property
    def append_only_history_is_visible(self) -> bool:
        return any("append-only" in item and "epochs" in item for item in self.guarantees)

    @property
    def self_review_is_refused(self) -> bool:
        return any("self-review" in item and "refused" in item for item in self.guarantees)

    @property
    def withdrawal_is_tombstoned(self) -> bool:
        return self.moderation is not None and self.moderation.withdrawn_count == 0 or (self.moderation is not None and all(record.tombstone is not None for key, record in self.moderation.records if record.state == "withdrawn"))

    @property
    def network_publication_is_not_claimed(self) -> bool:
        return any("does not persist" in item and "publish to a network" in item for item in self.guarantees)

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


def hub_submission_review(value: Mapping[str, Any]) -> HubSubmissionReviewReport:
    return HubSubmissionReviewReport.from_wire(value)


__all__ = [
    "HUB_SUBMISSION_SCHEMA",
    "HUB_SUBMISSION_MAX_INPUT_BYTES",
    "HUB_MODERATION_MAX_ACTIONS",
    "HUB_MODERATION_STATES",
    "HUB_VERIFICATION_STATES",
    "HUB_EVENT_KINDS",
    "HUB_SUBMISSION_STAGES",
    "HubSubmissionReviewArgs",
    "HubModerationEventReport",
    "HubTombstoneReport",
    "HubModerationRecordReport",
    "HubModerationLedgerReport",
    "HubSubmissionReviewReport",
    "hub_submission_review",
]
