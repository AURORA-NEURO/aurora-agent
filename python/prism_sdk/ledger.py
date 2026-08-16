"""Typed bounded event-ledger ingestion and projection reports.

The ledger tool accepts caller-supplied bitemporal events and returns the evidence needed to audit
what happened: per-event admission, released causal dependants, duplicate convergence, quarantine,
chain status, clock anomalies, temporal cuts, and digest-only projections.  This module validates
that response without copying payload bodies into projections or implying durable storage.
"""

from __future__ import annotations

from dataclasses import dataclass
import json
from typing import Any, Mapping, Sequence

from .capability import _route_mapping, _route_strings, _route_text
from .errors import ArgumentError


LEDGER_INGEST_SCHEMA = "bioprism-mcp/ledger-ingest/0.1"
LEDGER_MAX_EVENTS = 50_000
LEDGER_MAX_INPUT_BYTES = 20_000_000
LEDGER_MAX_ITEMS = 1_000
LEDGER_INGEST_STAGES = frozenset({"append"})
LEDGER_ADMISSION_KINDS = frozenset({"recorded", "duplicate", "quarantined"})
LEDGER_CHAIN_STATUSES = frozenset({"intact", "broken"})


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


def _text_allow_empty(name: str, value: Any) -> str:
    if not isinstance(value, str) or any(ord(character) < 32 for character in value):
        raise ArgumentError(f"{name} must be a string without control characters")
    return value


def _sequence(name: str, value: Any) -> tuple[Any, ...]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes, bytearray)):
        raise ArgumentError(f"{name} must be an array")
    return tuple(value)


def _bounded_strings(name: str, value: Any) -> tuple[str, ...]:
    return tuple(_route_text(f"{name}[{index}]", item) for index, item in enumerate(_sequence(name, value)))


def _mapping(name: str, value: Any) -> dict[str, Any]:
    return _route_mapping(name, value)


def _payload(value: Mapping[str, Any]) -> dict[str, Any]:
    """Extract a ledger projection from direct MCP output or an HTTP REST envelope."""

    raw = _route_mapping("ledger ingest response", value)
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
            if isinstance(content, Sequence) and not isinstance(content, (str, bytes)):
                for block in content:
                    if not isinstance(block, Mapping) or not isinstance(block.get("text"), str):
                        continue
                    try:
                        decoded = json.loads(block["text"])
                    except json.JSONDecodeError as error:
                        raise ArgumentError(f"ledger ingest response text is not JSON: {error}") from error
                    if isinstance(decoded, Mapping):
                        candidates.append(decoded)
        structured = container.get("structuredContent")
        if isinstance(structured, Mapping):
            candidates.append(structured)

    add_container(raw.get("mcp"))
    add_container(raw.get("result"))
    add_container(raw.get("structuredContent"))
    for candidate in candidates:
        if candidate.get("schema") == LEDGER_INGEST_SCHEMA and "ok" in candidate:
            return dict(candidate)
    raise ArgumentError("response does not contain a ledger ingest projection")


@dataclass(frozen=True)
class LedgerTemporalCut:
    """Independent optional upper bounds for valid, record, and release time."""

    as_of_valid: str | None = None
    as_of_record: str | None = None
    as_of_release: str | None = None

    def __post_init__(self) -> None:
        for name in ("as_of_valid", "as_of_record", "as_of_release"):
            value = getattr(self, name)
            if value is not None:
                object.__setattr__(self, name, _route_text(f"ledger cut {name}", value))

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "LedgerTemporalCut":
        raw = _mapping("ledger temporal cut", value)
        return cls(raw.get("as_of_valid"), raw.get("as_of_record"), raw.get("as_of_release"))

    def to_mcp_arguments(self) -> dict[str, str]:
        return {name: value for name, value in (("as_of_valid", self.as_of_valid), ("as_of_record", self.as_of_record), ("as_of_release", self.as_of_release)) if value is not None}


@dataclass(frozen=True)
class LedgerIngestArgs:
    """Bounded serialized event stream and optional temporal/projection controls."""

    events: tuple[dict[str, Any], ...]
    cut: LedgerTemporalCut | dict[str, Any] | None = None
    include_receipts: bool = False
    max_items: int = 100

    def __init__(self, events: Sequence[Mapping[str, Any]], cut: LedgerTemporalCut | Mapping[str, Any] | None = None, include_receipts: bool = False, max_items: int = 100) -> None:
        normalized_events = tuple(_mapping(f"ledger events[{index}]", event) for index, event in enumerate(_sequence("ledger events", events)))
        if not 1 <= len(normalized_events) <= LEDGER_MAX_EVENTS:
            raise ArgumentError(f"ledger events must contain between 1 and {LEDGER_MAX_EVENTS} events")
        if not isinstance(include_receipts, bool):
            raise ArgumentError("ledger include_receipts must be a boolean")
        if isinstance(max_items, bool) or not isinstance(max_items, int) or not 1 <= max_items <= LEDGER_MAX_ITEMS:
            raise ArgumentError(f"ledger max_items must be between 1 and {LEDGER_MAX_ITEMS}")
        normalized_cut = None if cut is None else cut if isinstance(cut, LedgerTemporalCut) else LedgerTemporalCut.from_wire(cut)
        arguments: dict[str, Any] = {
            "events": [dict(event) for event in normalized_events],
            "include_receipts": include_receipts,
            "max_items": max_items,
        }
        if normalized_cut is not None:
            arguments["cut"] = normalized_cut.to_mcp_arguments()
        try:
            encoded = json.dumps(arguments, ensure_ascii=False, separators=(",", ":"), allow_nan=False).encode("utf-8")
        except (TypeError, ValueError) as error:
            raise ArgumentError(f"ledger arguments are not JSON serializable: {error}") from error
        if len(encoded) > LEDGER_MAX_INPUT_BYTES:
            raise ArgumentError("ledger input exceeds the 20 MB safety bound")
        object.__setattr__(self, "events", normalized_events)
        object.__setattr__(self, "cut", normalized_cut)
        object.__setattr__(self, "include_receipts", include_receipts)
        object.__setattr__(self, "max_items", max_items)

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "LedgerIngestArgs":
        raw = _mapping("ledger ingest arguments", value)
        return cls(raw.get("events"), raw.get("cut"), raw.get("include_receipts", False), raw.get("max_items", 100))

    def to_mcp_arguments(self) -> dict[str, Any]:
        result: dict[str, Any] = {"events": [dict(event) for event in self.events], "include_receipts": self.include_receipts, "max_items": self.max_items}
        if self.cut is not None:
            result["cut"] = self.cut.to_mcp_arguments()
        return result


@dataclass(frozen=True)
class LedgerAdmissionReport:
    raw: dict[str, Any]
    kind: str
    event_id: str | None
    seq: int | None
    key: str | None
    missing: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "LedgerAdmissionReport":
        raw = _mapping("ledger admission", value)
        kind = _route_text("ledger admission kind", raw.get("admission"))
        if kind not in LEDGER_ADMISSION_KINDS:
            raise ArgumentError(f"unknown ledger admission kind {kind!r}")
        event_id = _optional_text("ledger admission event id", raw.get("id"))
        seq = None if raw.get("seq") is None else _integer("ledger admission sequence", raw.get("seq"))
        key = _optional_text("ledger admission key", raw.get("key"))
        missing = _bounded_strings("ledger admission missing parents", raw.get("missing", []))
        if kind == "recorded" and (event_id is None or seq is None or key is not None or missing):
            raise ArgumentError("recorded admissions must contain only an id and sequence")
        if kind == "duplicate" and (event_id is None or seq is not None or key is not None or missing):
            raise ArgumentError("duplicate admissions must contain only an id")
        if kind == "quarantined" and (key is None or event_id is not None or seq is not None or not missing):
            raise ArgumentError("quarantined admissions must contain a key and missing parents")
        return cls(raw, kind, event_id, seq, key, missing)


@dataclass(frozen=True)
class LedgerAppendReceiptReport:
    raw: dict[str, Any]
    event_index: int
    admission: LedgerAdmissionReport
    released: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "LedgerAppendReceiptReport":
        raw = _mapping("ledger append receipt", value)
        receipt = _mapping("ledger receipt", raw.get("receipt"))
        return cls(raw, _integer("ledger receipt event index", raw.get("event_index")), LedgerAdmissionReport.from_wire(receipt.get("admission")), _bounded_strings("ledger receipt released", receipt.get("released", [])))


@dataclass(frozen=True)
class LedgerAdmissionsReport:
    raw: dict[str, Any]
    recorded: int
    duplicates: int
    quarantined: int
    released: int
    receipts: tuple[LedgerAppendReceiptReport, ...] | None

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "LedgerAdmissionsReport":
        raw = _mapping("ledger admissions", value)
        receipt_raw = raw.get("receipts")
        receipts = None if receipt_raw is None else tuple(LedgerAppendReceiptReport.from_wire(item) for item in _sequence("ledger receipts", receipt_raw))
        return cls(raw, _integer("ledger recorded admissions", raw.get("recorded")), _integer("ledger duplicate admissions", raw.get("duplicates")), _integer("ledger quarantined admissions", raw.get("quarantined")), _integer("ledger released admissions", raw.get("released")), receipts)


@dataclass(frozen=True)
class LedgerChainReport:
    raw: dict[str, Any]
    status: str
    at_seq: int | None
    reason: str | None

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "LedgerChainReport":
        raw = _mapping("ledger chain", value)
        status = _route_text("ledger chain status", raw.get("status"))
        if status not in LEDGER_CHAIN_STATUSES:
            raise ArgumentError(f"unknown ledger chain status {status!r}")
        at_seq = None if raw.get("at_seq") is None else _integer("ledger chain at_seq", raw.get("at_seq"))
        reason = _optional_text("ledger chain reason", raw.get("reason"))
        if status == "intact" and (at_seq is not None or reason is not None):
            raise ArgumentError("intact ledger chains cannot retain a break witness")
        if status == "broken" and (at_seq is None or reason is None):
            raise ArgumentError("broken ledger chains must retain a break witness")
        return cls(raw, status, at_seq, reason)

    @property
    def intact(self) -> bool:
        return self.status == "intact"


@dataclass(frozen=True)
class LedgerClockAnomalyReport:
    raw: dict[str, Any]
    seq: int
    previous_record: str
    record: str

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "LedgerClockAnomalyReport":
        raw = _mapping("ledger clock anomaly", value)
        return cls(raw, _integer("ledger clock anomaly sequence", raw.get("seq")), _route_text("ledger previous record time", raw.get("previous_record")), _route_text("ledger record time", raw.get("record")))


@dataclass(frozen=True)
class LedgerQuarantineItemReport:
    raw: dict[str, Any]
    key: str
    missing: tuple[str, ...]
    note: str | None

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "LedgerQuarantineItemReport":
        raw = _mapping("ledger quarantine item", value)
        return cls(raw, _route_text("ledger quarantine key", raw.get("key")), _bounded_strings("ledger quarantine missing", raw.get("missing", [])), _optional_text("ledger quarantine note", raw.get("note")))


@dataclass(frozen=True)
class LedgerQuarantineReport:
    raw: dict[str, Any]
    count: int
    items: tuple[LedgerQuarantineItemReport, ...]
    omitted: int

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "LedgerQuarantineReport":
        raw = _mapping("ledger quarantine", value)
        count = _integer("ledger quarantine count", raw.get("count"))
        items = tuple(LedgerQuarantineItemReport.from_wire(item) for item in _sequence("ledger quarantine items", raw.get("items", [])))
        omitted = _integer("ledger quarantine omitted", raw.get("omitted"))
        if len(items) + omitted != count:
            raise ArgumentError("ledger quarantine count does not reconcile with bounded items")
        return cls(raw, count, items, omitted)


@dataclass(frozen=True)
class LedgerLatestFactReport:
    raw: dict[str, Any]
    subject: str
    event: str
    seq: int
    valid: str
    payload_digest: str

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "LedgerLatestFactReport":
        raw = _mapping("ledger latest fact", value)
        return cls(raw, _route_text("ledger latest subject", raw.get("subject")), _route_text("ledger latest event", raw.get("event")), _integer("ledger latest sequence", raw.get("seq")), _route_text("ledger latest valid time", raw.get("valid")), _route_text("ledger latest payload digest", raw.get("payload_digest")))


@dataclass(frozen=True)
class LedgerLatestBySubjectReport:
    raw: dict[str, Any]
    count: int
    items: tuple[LedgerLatestFactReport, ...]
    omitted: int

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "LedgerLatestBySubjectReport":
        raw = _mapping("ledger latest-by-subject projection", value)
        count = _integer("ledger latest subject count", raw.get("count"))
        items = tuple(LedgerLatestFactReport.from_wire(item) for item in _sequence("ledger latest subject items", raw.get("items", [])))
        omitted = _integer("ledger latest subject omitted", raw.get("omitted"))
        if len(items) + omitted != count:
            raise ArgumentError("ledger latest subject count does not reconcile with bounded items")
        return cls(raw, count, items, omitted)


@dataclass(frozen=True)
class LedgerCutEntryReport:
    raw: dict[str, Any]
    seq: int
    event_id: str
    event_class: str
    kind: str
    subject: str
    valid: str
    record: str
    release: str

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "LedgerCutEntryReport":
        raw = _mapping("ledger cut entry", value)
        return cls(raw, _integer("ledger cut sequence", raw.get("seq")), _route_text("ledger cut event id", raw.get("id")), _route_text("ledger cut class", raw.get("class")), _route_text("ledger cut kind", raw.get("kind")), _route_text("ledger cut subject", raw.get("subject")), _route_text("ledger cut valid time", raw.get("valid")), _route_text("ledger cut record time", raw.get("record")), _route_text("ledger cut release time", raw.get("release")))


@dataclass(frozen=True)
class LedgerCutReport:
    raw: dict[str, Any]
    requested: dict[str, Any]
    ok: bool
    count: int | None
    entries: tuple[LedgerCutEntryReport, ...]
    omitted: int
    refusal: str | None
    fail_closed: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "LedgerCutReport":
        raw = _mapping("ledger cut report", value)
        ok = _bool("ledger cut ok", raw.get("ok", True))
        requested = _mapping("ledger cut requested", raw.get("requested"))
        refusal = _optional_text("ledger cut refusal", raw.get("refusal"))
        fail_closed = _bool("ledger cut fail_closed", raw.get("fail_closed", False))
        if not ok:
            if refusal is None or not fail_closed:
                raise ArgumentError("failed ledger cuts must be fail-closed")
            return cls(raw, requested, False, None, (), 0, refusal, True)
        count = _integer("ledger cut count", raw.get("count"))
        entries = tuple(LedgerCutEntryReport.from_wire(item) for item in _sequence("ledger cut entries", raw.get("entries", [])))
        omitted = _integer("ledger cut omitted", raw.get("omitted"))
        if len(entries) + omitted != count:
            raise ArgumentError("ledger cut count does not reconcile with bounded entries")
        return cls(raw, requested, True, count, entries, omitted, None, False)


@dataclass(frozen=True)
class LedgerBeforeRefusalReport:
    raw: dict[str, Any]
    recorded_entries: int
    quarantined: int
    next_seq: int
    chain: LedgerChainReport

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "LedgerBeforeRefusalReport":
        raw = _mapping("ledger state before refusal", value)
        return cls(raw, _integer("ledger recorded entries before refusal", raw.get("recorded_entries")), _integer("ledger quarantined before refusal", raw.get("quarantined")), _integer("ledger next sequence before refusal", raw.get("next_seq")), LedgerChainReport.from_wire(raw.get("chain")))


@dataclass(frozen=True)
class LedgerIngestReport:
    raw: dict[str, Any]
    ok: bool
    schema: str
    stage: str | None
    event_index: int | None
    refusal: str | None
    fail_closed: bool
    ledger_before_refusal: LedgerBeforeRefusalReport | None
    entries: int | None
    next_seq: int | None
    head: str | None
    admissions: LedgerAdmissionsReport | None
    chain: LedgerChainReport | None
    clock_anomalies: tuple[LedgerClockAnomalyReport, ...]
    quarantine: LedgerQuarantineReport | None
    class_counts: tuple[tuple[str, int], ...]
    latest_by_subject: LedgerLatestBySubjectReport | None
    cut: LedgerCutReport | None
    guarantees: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "LedgerIngestReport":
        raw = _payload(value)
        ok = _bool("ledger ingest ok", raw.get("ok"))
        schema = _route_text("ledger ingest schema", raw.get("schema"))
        if schema != LEDGER_INGEST_SCHEMA:
            raise ArgumentError(f"unsupported ledger ingest schema {schema!r}")
        stage_raw = raw.get("stage")
        stage = None if stage_raw is None else _route_text("ledger ingest stage", stage_raw)
        if stage is not None and stage not in LEDGER_INGEST_STAGES:
            raise ArgumentError(f"unknown ledger ingest stage {stage!r}")
        refusal = _optional_text("ledger ingest refusal", raw.get("refusal"))
        fail_closed = _bool("ledger ingest fail_closed", raw.get("fail_closed", False))
        guarantees = _route_strings("ledger ingest guarantees", raw.get("guarantees", []))
        if not ok:
            if stage != "append" or refusal is None or not fail_closed:
                raise ArgumentError("failed ledger ingestion must retain an append-stage fail-closed refusal")
            before = LedgerBeforeRefusalReport.from_wire(raw.get("ledger_before_refusal"))
            return cls(raw, False, schema, stage, _integer("ledger refusal event index", raw.get("event_index")), refusal, True, before, None, None, None, None, None, (), None, (), None, None, guarantees)
        if stage is not None or refusal is not None or fail_closed:
            raise ArgumentError("successful ledger ingestion cannot retain refusal metadata")
        entries = _integer("ledger entries", raw.get("entries"))
        next_seq = _integer("ledger next sequence", raw.get("next_seq"))
        head = _text_allow_empty("ledger head", raw.get("head"))
        admissions = LedgerAdmissionsReport.from_wire(raw.get("admissions"))
        chain = LedgerChainReport.from_wire(raw.get("chain"))
        anomalies = tuple(LedgerClockAnomalyReport.from_wire(item) for item in _sequence("ledger clock anomalies", raw.get("clock_anomalies", [])))
        quarantine = LedgerQuarantineReport.from_wire(raw.get("quarantine"))
        class_counts_raw = _mapping("ledger class counts", raw.get("class_counts", {}))
        class_counts = tuple((_route_text("ledger event class", key), _integer("ledger class count", value)) for key, value in class_counts_raw.items())
        latest = LedgerLatestBySubjectReport.from_wire(raw.get("latest_by_subject"))
        cut_raw = raw.get("cut")
        cut = None if cut_raw is None else LedgerCutReport.from_wire(cut_raw)
        if entries < admissions.recorded - admissions.released:
            raise ArgumentError("ledger entries cannot be below unreleased recorded admissions")
        return cls(raw, True, schema, None, None, None, False, None, entries, next_seq, head, admissions, chain, anomalies, quarantine, class_counts, latest, cut, guarantees)

    @property
    def chain_intact(self) -> bool:
        return self.chain is not None and self.chain.intact

    @property
    def receipts_included(self) -> bool:
        return self.admissions is not None and self.admissions.receipts is not None

    @property
    def causal_releases_are_visible(self) -> bool:
        return self.ok and self.admissions is not None and self.admissions.released >= 0

    @property
    def projections_are_digest_only(self) -> bool:
        return any("digest" in guarantee and "payload" in guarantee for guarantee in self.guarantees)

    @property
    def durable_storage_is_not_claimed(self) -> bool:
        return any("no durable storage" in guarantee for guarantee in self.guarantees)

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


def ledger_ingest(value: Mapping[str, Any]) -> LedgerIngestReport:
    """Parse direct MCP output or an HTTP REST tool envelope."""

    return LedgerIngestReport.from_wire(value)


__all__ = [
    "LEDGER_INGEST_SCHEMA",
    "LEDGER_MAX_EVENTS",
    "LEDGER_MAX_INPUT_BYTES",
    "LEDGER_MAX_ITEMS",
    "LEDGER_INGEST_STAGES",
    "LEDGER_ADMISSION_KINDS",
    "LEDGER_CHAIN_STATUSES",
    "LedgerTemporalCut",
    "LedgerIngestArgs",
    "LedgerAdmissionReport",
    "LedgerAppendReceiptReport",
    "LedgerAdmissionsReport",
    "LedgerChainReport",
    "LedgerClockAnomalyReport",
    "LedgerQuarantineItemReport",
    "LedgerQuarantineReport",
    "LedgerLatestFactReport",
    "LedgerLatestBySubjectReport",
    "LedgerCutEntryReport",
    "LedgerCutReport",
    "LedgerBeforeRefusalReport",
    "LedgerIngestReport",
    "ledger_ingest",
]
