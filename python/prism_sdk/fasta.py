"""Bounded FASTA reference/assembly sequence auditing without sequence disclosure."""

from __future__ import annotations

from collections import Counter
from dataclasses import dataclass
from typing import Any, Mapping

from .authoring import content_digest
from .errors import ArgumentError


FASTA_SCHEMA = "bioprism-python-fasta/0.1"
FASTA_ADAPTER = "bioprism.python.fasta_text"
FASTA_ADAPTER_VERSION = "0.1.0"
FASTA_FORMAT = "text/fasta"
MAX_FASTA_BYTES = 50_000_000
MAX_FASTA_RECORDS = 100_000
MAX_FASTA_ITEMS = 1_000
_SEVERITY_ORDER = {"advisory": 0, "degrading": 1, "blocking": 2}
_NUCLEOTIDE_ALPHABET = frozenset("ACGTUNRYKMSWBDHVX.-*")
_PROTEIN_ALPHABET = frozenset("ABCDEFGHIKLMNPQRSTVWYBXZJUO.-*")


class FastaParseError(ArgumentError):
    """A structurally invalid FASTA source with a stable line locator."""

    def __init__(self, message: str, *, line: int | None = None, record: int | None = None) -> None:
        location = ""
        if line is not None:
            location += f" at line {line}"
        if record is not None:
            location += f" record {record}"
        super().__init__(f"FASTA parse refused{location}: {message}")


@dataclass(frozen=True)
class FastaFinding:
    """One bounded FASTA quality or alphabet finding."""

    code: str
    severity: str
    location: Mapping[str, Any]
    detail: str

    def __post_init__(self) -> None:
        if self.severity not in {"warning", "error"}:
            raise ArgumentError(f"unsupported FASTA finding severity: {self.severity!r}")

    def to_wire(self) -> dict[str, Any]:
        return {
            "code": self.code,
            "severity": self.severity,
            "location": dict(self.location),
            "detail": self.detail,
        }


@dataclass(frozen=True)
class FastaParseResult:
    """A validated FASTA projection with bounded sequence evidence."""

    document: Mapping[str, Any]

    @property
    def records(self) -> list[Mapping[str, Any]]:
        return list(self.document["records"])

    @property
    def valid(self) -> bool:
        return bool(self.document["valid"])

    @property
    def publishable(self) -> bool:
        return bool(self.document["publishable"])

    def to_wire(self) -> dict[str, Any]:
        return dict(self.document)


@dataclass(frozen=True)
class _Record:
    header: str
    identifier: str
    sequence: str
    header_line: int


class _Audit:
    def __init__(self, limit: int) -> None:
        self.limit = limit
        self.findings: list[FastaFinding] = []
        self.finding_count = 0
        self.error_count = 0
        self.losses: list[dict[str, Any]] = []
        self.loss_count = 0
        self.blocking_loss_count = 0
        self.max_loss_severity: str | None = None

    def finding(self, code: str, severity: str, location: Mapping[str, Any], detail: str) -> None:
        self.finding_count += 1
        if severity == "error":
            self.error_count += 1
        if len(self.findings) < self.limit:
            self.findings.append(FastaFinding(code, severity, dict(location), detail))

    def loss(self, kind: str, severity: str, location: str, detail: str) -> None:
        self.loss_count += 1
        if severity == "blocking":
            self.blocking_loss_count += 1
        if self.max_loss_severity is None or _SEVERITY_ORDER[severity] > _SEVERITY_ORDER[self.max_loss_severity]:
            self.max_loss_severity = severity
        if len(self.losses) < self.limit:
            self.losses.append(
                {
                    "kind": kind,
                    "severity": severity,
                    "location": location,
                    "detail": detail,
                }
            )

    @property
    def errors(self) -> int:
        return self.error_count

    @property
    def warnings(self) -> int:
        return self.finding_count - self.error_count


def _digest(source_id: str, value: str) -> str:
    return content_digest({"source_id": source_id, "value": value})


def _validate_limit(name: str, value: int, maximum: int) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or not 1 <= value <= maximum:
        raise ArgumentError(f"{name} must be between 1 and {maximum}")
    return value


def _decode(payload: str | bytes, *, max_bytes: int) -> str:
    max_bytes = _validate_limit("max_bytes", max_bytes, MAX_FASTA_BYTES)
    if isinstance(payload, bytes):
        if len(payload) > max_bytes:
            raise ArgumentError(f"FASTA exceeds the {max_bytes}-byte limit")
        try:
            return payload.decode("utf-8")
        except UnicodeDecodeError as error:
            raise ArgumentError(f"FASTA is not valid UTF-8: {error}") from error
    if isinstance(payload, str):
        if len(payload.encode("utf-8")) > max_bytes:
            raise ArgumentError(f"FASTA exceeds the {max_bytes}-byte limit")
        return payload
    raise ArgumentError("FASTA payload must be text or bytes")


def _lines(text: str) -> list[str]:
    if not text:
        raise FastaParseError("source is empty")
    lines = text.split("\n")
    if lines and lines[-1] == "":
        lines.pop()
    normalized: list[str] = []
    for line_number, line in enumerate(lines, start=1):
        if line.endswith("\r"):
            line = line[:-1]
        if "\r" in line:
            raise FastaParseError("lone carriage return is not a record separator", line=line_number)
        normalized.append(line)
    if not normalized:
        raise FastaParseError("source contains no records")
    return normalized


def _validate_header(header: str, *, line: int, record: int) -> str:
    if not header:
        raise FastaParseError("header is empty", line=line, record=record)
    if any(ord(character) < 32 or ord(character) > 126 for character in header):
        raise FastaParseError("header contains a non-printable ASCII character", line=line, record=record)
    tokens = header.split()
    if not tokens:
        raise FastaParseError("header has no sequence identifier", line=line, record=record)
    identifier = tokens[0]
    if len(identifier.encode("utf-8")) > 4_096:
        raise FastaParseError("sequence identifier exceeds the 4096-byte limit", line=line, record=record)
    return identifier


def _parse_records(text: str, *, max_records: int) -> tuple[list[_Record], int]:
    lines = _lines(text)
    records: list[_Record] = []
    comments = 0
    current_header: str | None = None
    current_identifier: str | None = None
    current_line = 0
    sequence_parts: list[str] = []

    def finish(line_number: int) -> None:
        if current_header is None or current_identifier is None:
            return
        if not sequence_parts:
            raise FastaParseError("record has no sequence lines", line=current_line, record=len(records) + 1)
        records.append(_Record(current_header, current_identifier, "".join(sequence_parts), current_line))

    for line_number, line in enumerate(lines, start=1):
        if line.startswith(">"):
            if current_header is not None:
                finish(line_number)
            if len(records) >= max_records:
                raise ArgumentError(f"FASTA contains more than the {max_records}-record limit")
            header = line[1:]
            current_header = header
            current_identifier = _validate_header(header, line=line_number, record=len(records) + 1)
            current_line = line_number
            sequence_parts = []
            continue
        if line.startswith(";"):
            comments += 1
            continue
        if current_header is None:
            raise FastaParseError("sequence or comment occurs before the first header", line=line_number)
        if not line:
            raise FastaParseError("sequence line is empty", line=line_number, record=len(records) + 1)
        if ">" in line or any(character.isspace() or ord(character) < 33 or ord(character) > 126 for character in line):
            raise FastaParseError("sequence contains whitespace, control, or reserved '>' characters", line=line_number, record=len(records) + 1)
        sequence_parts.append(line)
    finish(len(lines) + 1)
    if not records:
        raise FastaParseError("source contains no sequence records")
    return records, comments


def parse_fasta(
    payload: str | bytes,
    *,
    source_id: str,
    provenance: Mapping[str, Any] | None = None,
    sequence_type: str = "unknown",
    max_bytes: int = MAX_FASTA_BYTES,
    max_records: int = MAX_FASTA_RECORDS,
    max_items: int = MAX_FASTA_ITEMS,
) -> FastaParseResult:
    """Parse bounded FASTA records and audit optional nucleotide/protein alphabet claims."""

    if not isinstance(source_id, str) or not source_id.strip():
        raise ArgumentError("source_id must be a non-empty string")
    if provenance is not None and not isinstance(provenance, Mapping):
        raise ArgumentError("provenance must be a mapping when supplied")
    if not isinstance(sequence_type, str) or sequence_type.lower() not in {"unknown", "nucleotide", "protein"}:
        raise ArgumentError("sequence_type must be 'unknown', 'nucleotide', or 'protein'")
    sequence_type = sequence_type.lower()
    max_records = _validate_limit("max_records", max_records, MAX_FASTA_RECORDS)
    max_items = _validate_limit("max_items", max_items, MAX_FASTA_ITEMS)
    text = _decode(payload, max_bytes=max_bytes)
    records, comments = _parse_records(text, max_records=max_records)
    audit = _Audit(max_items)
    provenance_digest: str | None = None
    if provenance:
        try:
            provenance_digest = content_digest(dict(provenance))
        except (TypeError, ValueError) as error:
            raise ArgumentError(f"provenance is not canonical JSON-safe: {error}") from error
    audit.loss(
        "content_uninterpreted",
        "degrading",
        source_id,
        "sequence strings and headers are not emitted; bounded lengths, symbol counts, and source-bound digests are carried",
    )
    if sequence_type == "unknown":
        audit.loss(
            "type_undetermined",
            "degrading",
            "sequence_type",
            "caller did not declare nucleotide or protein alphabet semantics",
        )
    if provenance_digest is None:
        audit.loss(
            "provenance_unavailable",
            "blocking",
            "provenance",
            "no non-empty provenance projection was supplied",
        )

    allowed = _NUCLEOTIDE_ALPHABET if sequence_type == "nucleotide" else _PROTEIN_ALPHABET if sequence_type == "protein" else None
    seen_ids: set[str] = set()
    symbol_counts: Counter[str] = Counter()
    record_rows: list[dict[str, Any]] = []
    lengths: list[int] = []
    total_bases = 0
    total_gc = 0
    alphabet_mismatch_count = 0
    for number, record in enumerate(records, start=1):
        location = {"source": source_id, "record": number}
        if record.identifier in seen_ids:
            audit.finding("sequence_id_duplicate", "error", location, "sequence identifier occurs more than once; the identifier is not echoed")
        seen_ids.add(record.identifier)
        sequence = record.sequence.upper()
        symbols = Counter(sequence)
        symbol_counts.update(symbols)
        if allowed is not None:
            invalid_symbols = sorted(set(sequence).difference(allowed))
            if invalid_symbols:
                alphabet_mismatch_count += 1
                audit.finding(
                    "alphabet_mismatch",
                    "error",
                    location,
                    f"declared {sequence_type} alphabet contains {len(invalid_symbols)} unsupported symbol class(es)",
                )
        gc = symbols.get("G", 0) + symbols.get("C", 0)
        total_gc += gc
        lengths.append(len(sequence))
        total_bases += len(sequence)
        record_rows.append(
            {
                "record": number,
                "sequence_id_digest": _digest(source_id, record.identifier),
                "sequence_digest": _digest(source_id, sequence),
                "length": len(sequence),
                "gc_bases": gc if sequence_type == "nucleotide" else None,
                "observed_symbol_count": len(symbols),
            }
        )

    valid = audit.errors == 0
    publishable = valid and audit.max_loss_severity != "blocking"
    source_digest = content_digest({"source_id": source_id, "payload": text})
    manifest = {
        "source_id": source_id,
        "source_digest": source_digest,
        "adapter": FASTA_ADAPTER,
        "adapter_version": FASTA_ADAPTER_VERSION,
        "declared_format": FASTA_FORMAT,
        "record_count": len(records),
        "comment_line_count": comments,
        "sequence_type": sequence_type,
        "provenance_digest": provenance_digest,
        "bytes_read": True,
        "sequence_identifiers_disclosed": False,
        "sequence_bases_disclosed": False,
    }
    document: dict[str, Any] = {
        "schema": FASTA_SCHEMA,
        "workflow": "fasta_sequence_reference_audit",
        "valid": valid,
        "publishable": publishable,
        "source_id": source_id,
        "manifest": manifest,
        "summary": {
            "records": len(records),
            "total_bases": total_bases,
            "sequence_length_min": min(lengths),
            "sequence_length_max": max(lengths),
            "gc_bases": total_gc if sequence_type == "nucleotide" else None,
            "sequence_symbol_counts": dict(sorted(symbol_counts.items())),
            "unique_identifier_count": len(seen_ids),
            "errors": audit.errors,
            "warnings": audit.warnings,
            "finding_count": audit.finding_count,
            "blocking_loss_count": audit.blocking_loss_count,
        },
        "records": record_rows[:max_items],
        "omitted_records": max(0, len(record_rows) - max_items),
        "findings": [finding.to_wire() for finding in audit.findings],
        "omitted_findings": max(0, audit.finding_count - len(audit.findings)),
        "semantic_loss": {
            "audit": "lossy" if audit.loss_count else "lossless",
            "lost_count": audit.loss_count,
            "max_severity": audit.max_loss_severity,
            "lost": list(audit.losses),
            "omitted_lost": max(0, audit.loss_count - len(audit.losses)),
        },
        "conformance": {
            "level": "normalize",
            "passed": valid,
            "publishable": publishable,
            "checks": {
                "record_structure": "pass" if records else "fail",
                "identifier_uniqueness": "pass" if len(seen_ids) == len(records) else "fail",
                "alphabet": "pass" if alphabet_mismatch_count == 0 else "fail",
                "provenance": "pass" if provenance_digest is not None else "fail",
            },
            "limitations": [
                "sequence strings and identifiers are represented by source-bound digests rather than disclosed",
                "alphabet validation is only applied when the caller declares nucleotide or protein semantics",
                "the audit does not establish reference-build identity, assembly completeness, homology, contamination, or biological function",
            ],
        },
        "max_records": MAX_FASTA_RECORDS,
        "max_items": max_items,
    }
    document["document_digest"] = content_digest(document)
    return FastaParseResult(document)


class FastaAdapter:
    """Concrete adapter facade matching the dependency-free bounded FASTA route."""

    name = FASTA_ADAPTER
    version = FASTA_ADAPTER_VERSION
    accepted_formats = ("application/fasta", "text/fasta", "text/x-fasta")
    declared_loss_kinds = frozenset(
        {
            "content_uninterpreted",
            "provenance_unavailable",
            "type_undetermined",
        }
    )

    def manifest(self) -> dict[str, Any]:
        return {
            "name": self.name,
            "version": self.version,
            "accepted_formats": list(self.accepted_formats),
            "conformance_level": "normalize",
            "declared_loss_kinds": sorted(self.declared_loss_kinds),
            "scope_dimensions": ["subject", "sample", "reference", "sequence"],
            "execution": "python_delegated",
            "optional_dependency": None,
        }

    def parse(
        self,
        fasta: str | bytes,
        *,
        source_id: str,
        provenance: Mapping[str, Any] | None = None,
        sequence_type: str = "unknown",
        max_bytes: int = MAX_FASTA_BYTES,
        max_records: int = MAX_FASTA_RECORDS,
        max_items: int = MAX_FASTA_ITEMS,
    ) -> FastaParseResult:
        return parse_fasta(
            fasta,
            source_id=source_id,
            provenance=provenance,
            sequence_type=sequence_type,
            max_bytes=max_bytes,
            max_records=max_records,
            max_items=max_items,
        )


__all__ = [
    "FASTA_ADAPTER",
    "FASTA_ADAPTER_VERSION",
    "FASTA_FORMAT",
    "FASTA_SCHEMA",
    "FastaAdapter",
    "FastaFinding",
    "FastaParseError",
    "FastaParseResult",
    "MAX_FASTA_BYTES",
    "MAX_FASTA_ITEMS",
    "MAX_FASTA_RECORDS",
    "parse_fasta",
]
