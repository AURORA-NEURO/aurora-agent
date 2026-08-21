"""Restart-safe, metadata-only persistence for autonomous domain-tool receipts.

``AutonomousDomainToolRuntime`` intentionally keeps execution values transient.  This module adds
the missing persistence seam without changing that rule: it accepts only
``AutonomousDomainToolReceipt`` values, writes a bounded hash-chained JSONL stream, and provides
identity-based idempotency for worker retries.  It never stores tool arguments, outputs, provider
payloads, headers, or credentials.

The journal is a caller-owned sink, so applications may use it directly as
``receipt_sink=journal.append``.  Reopening the same path verifies every existing line before a
new entry is appended; corruption and identity conflicts fail closed instead of being repaired
silently.
"""

from __future__ import annotations

from dataclasses import dataclass
import json
import os
from pathlib import Path
import threading
from typing import Any, Mapping

from .authoring import canonical_bytes, content_digest
from .domain_tools import (
    DOMAIN_TOOL_SCHEMA,
    AutonomousDomainToolReceipt,
)
from .errors import ArgumentError


AUTONOMOUS_DOMAIN_TOOL_RECEIPT_JOURNAL_SCHEMA = (
    "bioprism-python-autonomous-domain-tool-receipt-journal/0.1"
)
AUTONOMOUS_DOMAIN_TOOL_RECEIPT_ENTRY_SCHEMA = (
    "bioprism-python-autonomous-domain-tool-receipt-entry/0.1"
)
MAX_AUTONOMOUS_DOMAIN_TOOL_RECEIPT_JOURNAL_ENTRIES = 100_000
MAX_AUTONOMOUS_DOMAIN_TOOL_RECEIPT_JOURNAL_BYTES = 50_000_000
MAX_AUTONOMOUS_DOMAIN_TOOL_RECEIPT_ENTRY_BYTES = 16_000
_RETENTION = "metadata_only_hash_chained_no_arguments_or_outputs"
_SECRET_MATERIAL = "never_returned"


def _digest(name: str, value: Any, *, allow_none: bool = False) -> str | None:
    if value is None and allow_none:
        return None
    if not isinstance(value, str) or len(value) != 64 or any(
        character not in "0123456789abcdef" for character in value
    ):
        raise ArgumentError(f"{name} must be a lowercase SHA-256 digest")
    return value


def _receipt_from_mapping(value: Mapping[str, Any] | AutonomousDomainToolReceipt) -> AutonomousDomainToolReceipt:
    if isinstance(value, AutonomousDomainToolReceipt):
        return AutonomousDomainToolReceipt(**{
            "call_id": value.call_id,
            "tool": value.tool,
            "status": value.status,
            "schema_digest": value.schema_digest,
            "arguments_digest": value.arguments_digest,
            "output_digest": value.output_digest,
            "execution_id": value.execution_id,
            "domain": value.domain,
            "capability": value.capability,
            "risk_class": value.risk_class,
        })
    if not isinstance(value, Mapping):
        raise ArgumentError("domain tool receipt must be a mapping or AutonomousDomainToolReceipt")
    expected = {
        "schema",
        "call_id",
        "tool",
        "status",
        "schema_digest",
        "arguments_digest",
        "output_digest",
        "execution_id",
        "domain",
        "capability",
        "risk_class",
        "retention",
    }
    if set(value) != expected:
        raise ArgumentError("domain tool receipt contains unsupported or missing fields")
    if value.get("schema") != DOMAIN_TOOL_SCHEMA or value.get("retention") != "metadata_only_no_arguments_or_outputs":
        raise ArgumentError("domain tool receipt retention contract is invalid")
    return AutonomousDomainToolReceipt(
        call_id=value.get("call_id"),
        tool=value.get("tool"),
        status=value.get("status"),
        schema_digest=value.get("schema_digest"),
        arguments_digest=value.get("arguments_digest"),
        output_digest=value.get("output_digest"),
        execution_id=value.get("execution_id"),
        domain=value.get("domain"),
        capability=value.get("capability"),
        risk_class=value.get("risk_class"),
    )


def _receipt_identity_digest(receipt: AutonomousDomainToolReceipt) -> str:
    """Return stable retry identity without depending on arguments or output values."""

    return content_digest(
        {
            "schema": AUTONOMOUS_DOMAIN_TOOL_RECEIPT_ENTRY_SCHEMA,
            "execution_id": receipt.execution_id,
            "call_id": receipt.call_id,
            "tool": receipt.tool,
        }
    )


@dataclass(frozen=True, slots=True)
class AutonomousDomainToolReceiptJournalEntry:
    """One validated metadata-only journal entry."""

    sequence: int
    previous_entry_digest: str | None
    receipt: AutonomousDomainToolReceipt
    receipt_identity_digest: str
    entry_digest: str

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_DOMAIN_TOOL_RECEIPT_ENTRY_SCHEMA,
            "sequence": self.sequence,
            "previous_entry_digest": self.previous_entry_digest,
            "receipt": self.receipt.to_dict(),
            "receipt_identity_digest": self.receipt_identity_digest,
            "entry_digest": self.entry_digest,
            "retention": _RETENTION,
            "secret_material": _SECRET_MATERIAL,
        }


def _entry_from_mapping(value: Mapping[str, Any]) -> AutonomousDomainToolReceiptJournalEntry:
    expected = {
        "schema",
        "sequence",
        "previous_entry_digest",
        "receipt",
        "receipt_identity_digest",
        "entry_digest",
        "retention",
        "secret_material",
    }
    if not isinstance(value, Mapping) or set(value) != expected:
        raise ArgumentError("domain tool receipt journal entry is malformed")
    if value.get("schema") != AUTONOMOUS_DOMAIN_TOOL_RECEIPT_ENTRY_SCHEMA:
        raise ArgumentError("domain tool receipt journal entry schema is invalid")
    if value.get("retention") != _RETENTION or value.get("secret_material") != _SECRET_MATERIAL:
        raise ArgumentError("domain tool receipt journal entry retention is invalid")
    sequence = value.get("sequence")
    if not isinstance(sequence, int) or isinstance(sequence, bool) or not 1 <= sequence <= MAX_AUTONOMOUS_DOMAIN_TOOL_RECEIPT_JOURNAL_ENTRIES:
        raise ArgumentError("domain tool receipt journal sequence is outside its bound")
    previous = _digest(
        "domain tool receipt journal previous_entry_digest",
        value.get("previous_entry_digest"),
        allow_none=True,
    )
    receipt_value = value.get("receipt")
    if not isinstance(receipt_value, Mapping):
        raise ArgumentError("domain tool receipt journal receipt is invalid")
    receipt = _receipt_from_mapping(receipt_value)
    identity = _digest(
        "domain tool receipt journal receipt_identity_digest",
        value.get("receipt_identity_digest"),
    )
    expected_identity = _receipt_identity_digest(receipt)
    if identity != expected_identity:
        raise ArgumentError("domain tool receipt journal identity digest does not match its receipt")
    entry_digest = _digest("domain tool receipt journal entry_digest", value.get("entry_digest"))
    descriptor = {
        "schema": AUTONOMOUS_DOMAIN_TOOL_RECEIPT_ENTRY_SCHEMA,
        "sequence": sequence,
        "previous_entry_digest": previous,
        "receipt": receipt.to_dict(),
        "receipt_identity_digest": identity,
        "retention": _RETENTION,
        "secret_material": _SECRET_MATERIAL,
    }
    if entry_digest != content_digest(descriptor):
        raise ArgumentError("domain tool receipt journal entry digest does not match its metadata")
    return AutonomousDomainToolReceiptJournalEntry(
        sequence,
        previous,
        receipt,
        identity,
        entry_digest,
    )


class AutonomousDomainToolReceiptJournal:
    """Bounded append-only JSONL store suitable for ``AutonomousDomainToolRuntime.receipt_sink``."""

    def __init__(
        self,
        path: str | os.PathLike[str],
        *,
        max_entries: int = MAX_AUTONOMOUS_DOMAIN_TOOL_RECEIPT_JOURNAL_ENTRIES,
        max_bytes: int = MAX_AUTONOMOUS_DOMAIN_TOOL_RECEIPT_JOURNAL_BYTES,
    ) -> None:
        if not isinstance(path, (str, os.PathLike)) or not str(path):
            raise ArgumentError("domain tool receipt journal path must be non-empty")
        if isinstance(max_entries, bool) or not isinstance(max_entries, int) or not 1 <= max_entries <= MAX_AUTONOMOUS_DOMAIN_TOOL_RECEIPT_JOURNAL_ENTRIES:
            raise ArgumentError("domain tool receipt journal max_entries is outside its bound")
        if isinstance(max_bytes, bool) or not isinstance(max_bytes, int) or not 1 <= max_bytes <= MAX_AUTONOMOUS_DOMAIN_TOOL_RECEIPT_JOURNAL_BYTES:
            raise ArgumentError("domain tool receipt journal max_bytes is outside its bound")
        self.path = Path(path)
        self.max_entries = max_entries
        self.max_bytes = max_bytes
        self._lock = threading.RLock()
        with self._lock:
            self._read_rows_locked()

    def append(
        self,
        receipt: AutonomousDomainToolReceipt | Mapping[str, Any],
    ) -> AutonomousDomainToolReceiptJournalEntry:
        normalized = _receipt_from_mapping(receipt)
        identity = _receipt_identity_digest(normalized)
        with self._lock:
            rows = self._read_rows_locked()
            existing = next((row for row in rows if row.receipt_identity_digest == identity), None)
            if existing is not None:
                if content_digest(existing.receipt.to_dict()) == content_digest(normalized.to_dict()):
                    return existing
                raise ArgumentError("domain tool receipt journal identity conflict")
            if len(rows) >= self.max_entries:
                raise ArgumentError("domain tool receipt journal entry capacity is exhausted")
            descriptor = {
                "schema": AUTONOMOUS_DOMAIN_TOOL_RECEIPT_ENTRY_SCHEMA,
                "sequence": len(rows) + 1,
                "previous_entry_digest": rows[-1].entry_digest if rows else None,
                "receipt": normalized.to_dict(),
                "receipt_identity_digest": identity,
                "retention": _RETENTION,
                "secret_material": _SECRET_MATERIAL,
            }
            entry = AutonomousDomainToolReceiptJournalEntry(
                descriptor["sequence"],
                descriptor["previous_entry_digest"],
                normalized,
                identity,
                content_digest(descriptor),
            )
            line = canonical_bytes(entry.to_dict()) + b"\n"
            if len(line) > MAX_AUTONOMOUS_DOMAIN_TOOL_RECEIPT_ENTRY_BYTES:
                raise ArgumentError("domain tool receipt journal entry exceeds its byte bound")
            current_size = self.path.stat().st_size if self.path.exists() else 0
            if current_size + len(line) > self.max_bytes:
                raise ArgumentError("domain tool receipt journal byte capacity is exhausted")
            self.path.parent.mkdir(parents=True, exist_ok=True)
            with self.path.open("ab") as handle:
                handle.write(line)
                handle.flush()
                os.fsync(handle.fileno())
            return entry

    def find(
        self,
        *,
        execution_id: str | None,
        call_id: str,
        tool: str,
    ) -> AutonomousDomainToolReceipt | None:
        probe = AutonomousDomainToolReceipt(
            call_id=call_id,
            tool=tool,
            status="executed",
            execution_id=execution_id,
        )
        identity = _receipt_identity_digest(probe)
        with self._lock:
            for row in reversed(self._read_rows_locked()):
                if row.receipt_identity_digest == identity:
                    return row.receipt
        return None

    def receipts(
        self,
        *,
        execution_id: str | None = None,
        after_sequence: int = 0,
        limit: int = 256,
    ) -> tuple[AutonomousDomainToolReceiptJournalEntry, ...]:
        if not isinstance(after_sequence, int) or isinstance(after_sequence, bool) or after_sequence < 0:
            raise ArgumentError("domain tool receipt journal after_sequence must be non-negative")
        if not isinstance(limit, int) or isinstance(limit, bool) or not 1 <= limit <= self.max_entries:
            raise ArgumentError("domain tool receipt journal limit is outside its bound")
        with self._lock:
            rows = self._read_rows_locked()
        return tuple(
            row
            for row in rows
            if row.sequence > after_sequence
            and (execution_id is None or row.receipt.execution_id == execution_id)
        )[:limit]

    def verify_integrity(self) -> dict[str, Any]:
        with self._lock:
            rows = self._read_rows_locked()
        return {
            "schema": AUTONOMOUS_DOMAIN_TOOL_RECEIPT_JOURNAL_SCHEMA,
            "verified": True,
            "entries": len(rows),
            "head_digest": rows[-1].entry_digest if rows else None,
            "retention": _RETENTION,
            "secret_material": _SECRET_MATERIAL,
        }

    def _read_rows_locked(self) -> list[AutonomousDomainToolReceiptJournalEntry]:
        if not self.path.exists():
            return []
        if self.path.stat().st_size > self.max_bytes:
            raise ArgumentError("domain tool receipt journal exceeds max_bytes")
        rows: list[AutonomousDomainToolReceiptJournalEntry] = []
        identities: set[str] = set()
        with self.path.open("rb") as handle:
            for raw_line in handle:
                if len(rows) >= self.max_entries:
                    raise ArgumentError("domain tool receipt journal exceeds max_entries")
                try:
                    value = json.loads(raw_line.decode("utf-8"))
                except (UnicodeDecodeError, json.JSONDecodeError) as error:
                    raise ArgumentError("domain tool receipt journal contains invalid JSON") from error
                if not isinstance(value, Mapping):
                    raise ArgumentError("domain tool receipt journal line must be an object")
                entry = _entry_from_mapping(value)
                expected_previous = rows[-1].entry_digest if rows else None
                if entry.sequence != len(rows) + 1 or entry.previous_entry_digest != expected_previous:
                    raise ArgumentError("domain tool receipt journal hash chain is invalid")
                if entry.receipt_identity_digest in identities:
                    raise ArgumentError("domain tool receipt journal contains duplicate identities")
                identities.add(entry.receipt_identity_digest)
                if len(canonical_bytes(entry.to_dict())) > MAX_AUTONOMOUS_DOMAIN_TOOL_RECEIPT_ENTRY_BYTES:
                    raise ArgumentError("domain tool receipt journal entry exceeds its byte bound")
                rows.append(entry)
        return rows


__all__ = [
    "AUTONOMOUS_DOMAIN_TOOL_RECEIPT_JOURNAL_SCHEMA",
    "AUTONOMOUS_DOMAIN_TOOL_RECEIPT_ENTRY_SCHEMA",
    "MAX_AUTONOMOUS_DOMAIN_TOOL_RECEIPT_JOURNAL_ENTRIES",
    "MAX_AUTONOMOUS_DOMAIN_TOOL_RECEIPT_JOURNAL_BYTES",
    "MAX_AUTONOMOUS_DOMAIN_TOOL_RECEIPT_ENTRY_BYTES",
    "AutonomousDomainToolReceiptJournalEntry",
    "AutonomousDomainToolReceiptJournal",
]
