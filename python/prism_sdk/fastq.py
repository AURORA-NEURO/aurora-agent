"""Bounded FASTQ parsing with privacy-preserving sequencing-read evidence.

The parser validates the complete bounded record stream, including multiline sequences and
qualities, without returning base or quality strings. Read identifiers, sequences, and qualities
are represented by source-bound digests; lengths, quality ranges, symbol counts, and pairing
evidence remain available for routing and quality-control workflows.
"""

from __future__ import annotations

from collections import Counter
from dataclasses import dataclass
from typing import Any, Mapping

from .authoring import content_digest
from .errors import ArgumentError


FASTQ_SCHEMA = "bioprism-python-fastq/0.1"
FASTQ_ADAPTER = "bioprism.python.fastq_text"
FASTQ_ADAPTER_VERSION = "0.1.0"
FASTQ_FORMAT = "text/fastq"
MAX_FASTQ_BYTES = 50_000_000
MAX_FASTQ_RECORDS = 100_000
MAX_FASTQ_ITEMS = 1_000
_SEVERITY_ORDER = {"advisory": 0, "degrading": 1, "blocking": 2}


class FastqParseError(ArgumentError):
    """A structurally invalid FASTQ source with a stable record and line locator."""

    def __init__(self, message: str, *, line: int | None = None, record: int | None = None) -> None:
        self.line = line
        self.record = record
        location = ""
        if line is not None:
            location += f" at line {line}"
        if record is not None:
            location += f" record {record}"
        super().__init__(f"FASTQ parse refused{location}: {message}")


@dataclass(frozen=True)
class FastqFinding:
    """One bounded sequencing-file quality or pairing finding."""

    code: str
    severity: str
    location: Mapping[str, Any]
    detail: str

    def __post_init__(self) -> None:
        if self.severity not in {"warning", "error"}:
            raise ArgumentError(f"unsupported FASTQ finding severity: {self.severity!r}")

    def to_wire(self) -> dict[str, Any]:
        return {
            "code": self.code,
            "severity": self.severity,
            "location": dict(self.location),
            "detail": self.detail,
        }


@dataclass(frozen=True)
class FastqParseResult:
    """A validated FASTQ projection with bounded summaries and loss evidence."""

    document: Mapping[str, Any]

    @property
    def reads(self) -> list[Mapping[str, Any]]:
        return list(self.document["reads"])

    @property
    def valid(self) -> bool:
        return bool(self.document["valid"])

    @property
    def publishable(self) -> bool:
        return bool(self.document["publishable"])

    def to_wire(self) -> dict[str, Any]:
        return dict(self.document)


@dataclass(frozen=True)
class _Read:
    header: str
    identifier: str
    sequence: str
    quality: str
    header_line: int
    quality_line: int


class _Audit:
    def __init__(self, limit: int) -> None:
        self.limit = limit
        self.findings: list[FastqFinding] = []
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
            self.findings.append(FastqFinding(code, severity, dict(location), detail))

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

    @property
    def max_loss(self) -> str | None:
        return self.max_loss_severity


def _digest(source_id: str, value: str) -> str:
    return content_digest({"source_id": source_id, "value": value})


def _validate_limit(name: str, value: int, maximum: int) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or not 1 <= value <= maximum:
        raise ArgumentError(f"{name} must be between 1 and {maximum}")
    return value


def _decode(payload: str | bytes, *, max_bytes: int) -> str:
    max_bytes = _validate_limit("max_bytes", max_bytes, MAX_FASTQ_BYTES)
    if isinstance(payload, bytes):
        if len(payload) > max_bytes:
            raise ArgumentError(f"FASTQ exceeds the {max_bytes}-byte limit")
        try:
            return payload.decode("utf-8")
        except UnicodeDecodeError as error:
            raise ArgumentError(f"FASTQ is not valid UTF-8: {error}") from error
    if isinstance(payload, str):
        if len(payload.encode("utf-8")) > max_bytes:
            raise ArgumentError(f"FASTQ exceeds the {max_bytes}-byte limit")
        return payload
    raise ArgumentError("FASTQ payload must be text or bytes")


def _lines(text: str) -> list[str]:
    if not text:
        raise FastqParseError("source is empty")
    lines = text.split("\n")
    if lines and lines[-1] == "":
        lines.pop()
    normalized: list[str] = []
    for line_number, line in enumerate(lines, start=1):
        if line.endswith("\r"):
            line = line[:-1]
        if "\r" in line:
            raise FastqParseError("lone carriage return is not a record separator", line=line_number)
        normalized.append(line)
    if not normalized:
        raise FastqParseError("source contains no records")
    return normalized


def _validate_printable(
    line: str,
    *,
    field: str,
    line_number: int,
    record: int,
    allow_spaces: bool = False,
) -> None:
    if not line:
        raise FastqParseError(f"{field} line is empty", line=line_number, record=record)
    minimum = 32 if allow_spaces else 33
    if any(ord(character) < minimum or ord(character) > 126 for character in line):
        raise FastqParseError(
            f"{field} contains a non-printable ASCII character",
            line=line_number,
            record=record,
        )


def _parse_records(text: str, *, max_records: int) -> list[_Read]:
    lines = _lines(text)
    reads: list[_Read] = []
    index = 0
    while index < len(lines):
        if len(reads) >= max_records:
            raise ArgumentError(f"FASTQ contains more than the {max_records}-record limit")
        record = len(reads) + 1
        header_line = index + 1
        header = lines[index]
        if not header.startswith("@"):
            raise FastqParseError("record must start with '@'", line=header_line, record=record)
        header_text = header[1:]
        _validate_printable(header_text, field="header", line_number=header_line, record=record, allow_spaces=True)
        header_tokens = header_text.split()
        if not header_tokens:
            raise FastqParseError("header has no read identifier", line=header_line, record=record)
        identifier = header_tokens[0]
        if len(identifier.encode("utf-8")) > 4_096:
            raise FastqParseError("read identifier exceeds the 4096-byte limit", line=header_line, record=record)
        index += 1

        sequence_start = index + 1
        sequence_parts: list[str] = []
        while index < len(lines) and not lines[index].startswith("+"):
            _validate_printable(lines[index], field="sequence", line_number=index + 1, record=record)
            if "+" in lines[index]:
                raise FastqParseError("sequence contains reserved '+' character", line=index + 1, record=record)
            sequence_parts.append(lines[index])
            index += 1
        if not sequence_parts:
            raise FastqParseError("record has no sequence lines", line=sequence_start, record=record)
        sequence = "".join(sequence_parts)
        if index >= len(lines):
            raise FastqParseError("record is missing its '+' separator", line=header_line, record=record)

        plus_line = lines[index]
        plus_suffix = plus_line[1:]
        if plus_suffix:
            _validate_printable(plus_suffix, field="'+' identifier", line_number=index + 1, record=record, allow_spaces=True)
        plus_identifier = plus_suffix.strip()
        if plus_identifier and plus_identifier.split()[0] != identifier:
            raise FastqParseError("'+' identifier does not match the read header", line=index + 1, record=record)
        index += 1

        quality_start = index + 1
        quality_parts: list[str] = []
        quality_length = 0
        while index < len(lines) and quality_length < len(sequence):
            quality_line = lines[index]
            _validate_printable(quality_line, field="quality", line_number=index + 1, record=record)
            quality_parts.append(quality_line)
            quality_length += len(quality_line)
            if quality_length > len(sequence):
                raise FastqParseError("quality length exceeds sequence length", line=index + 1, record=record)
            index += 1
        if quality_length != len(sequence):
            raise FastqParseError(
                f"quality length {quality_length} does not match sequence length {len(sequence)}",
                line=quality_start,
                record=record,
            )
        reads.append(_Read(header, identifier, sequence, "".join(quality_parts), header_line, quality_start))
    return reads


def _pairing(header: str, identifier: str) -> tuple[str, int | None, str]:
    if identifier.endswith("/1"):
        return identifier[:-2], 1, "first"
    if identifier.endswith("/2"):
        return identifier[:-2], 2, "second"
    tokens = header.split()
    if len(tokens) > 1 and tokens[1][:2] in {"1:", "2:"}:
        mate = int(tokens[1][0])
        return identifier, mate, "first" if mate == 1 else "second"
    return identifier, None, "unpaired"


def parse_fastq(
    payload: str | bytes,
    *,
    source_id: str,
    provenance: Mapping[str, Any] | None = None,
    max_bytes: int = MAX_FASTQ_BYTES,
    max_records: int = MAX_FASTQ_RECORDS,
    max_items: int = MAX_FASTQ_ITEMS,
) -> FastqParseResult:
    """Parse and audit a bounded FASTQ stream without disclosing read content."""

    if not isinstance(source_id, str) or not source_id.strip():
        raise ArgumentError("source_id must be a non-empty string")
    if provenance is not None and not isinstance(provenance, Mapping):
        raise ArgumentError("provenance must be a mapping when supplied")
    max_records = _validate_limit("max_records", max_records, MAX_FASTQ_RECORDS)
    max_items = _validate_limit("max_items", max_items, MAX_FASTQ_ITEMS)
    text = _decode(payload, max_bytes=max_bytes)
    reads = _parse_records(text, max_records=max_records)
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
        "base and quality strings are not emitted; only bounded digests and quality-control summaries are carried",
    )
    audit.loss(
        "type_undetermined",
        "degrading",
        "quality",
        "FASTQ quality values are summarized under the conventional printable Phred+33 interpretation; no external encoding declaration was supplied",
    )
    if provenance_digest is None:
        audit.loss(
            "provenance_unavailable",
            "blocking",
            "provenance",
            "no non-empty provenance projection was supplied",
        )

    symbol_counts: Counter[str] = Counter()
    pair_groups: dict[str, Counter[int]] = {}
    seen_identifiers: set[str] = set()
    read_rows: list[dict[str, Any]] = []
    total_bases = 0
    sequence_lengths: list[int] = []
    quality_min: int | None = None
    quality_max: int | None = None
    for number, read in enumerate(reads, start=1):
        pair_key, mate, pair_label = _pairing(read.header, read.identifier)
        if read.identifier in seen_identifiers:
            audit.finding(
                "read_id_duplicate",
                "error",
                {"source": source_id, "record": number},
                "read identifier occurs more than once; the identifier is intentionally not echoed",
            )
        seen_identifiers.add(read.identifier)
        if mate is not None:
            pair_groups.setdefault(pair_key, Counter())[mate] += 1
        symbols = Counter(character.upper() for character in read.sequence)
        symbol_counts.update(symbols)
        phred_values = [ord(character) - 33 for character in read.quality]
        read_rows.append(
            {
                "record": number,
                "read_id_digest": _digest(source_id, read.identifier),
                "sequence_digest": _digest(source_id, read.sequence),
                "quality_digest": _digest(source_id, read.quality),
                "sequence_length": len(read.sequence),
                "quality_length": len(read.quality),
                "pair": pair_label,
                "quality_phred_min": min(phred_values),
                "quality_phred_max": max(phred_values),
            }
        )
        total_bases += len(read.sequence)
        sequence_lengths.append(len(read.sequence))
        current_min = min(phred_values)
        current_max = max(phred_values)
        quality_min = current_min if quality_min is None else min(quality_min, current_min)
        quality_max = current_max if quality_max is None else max(quality_max, current_max)

    duplicate_mate_groups = 0
    complete_pairs = 0
    incomplete_pairs = 0
    for pair_key, mates in pair_groups.items():
        if any(count > 1 for count in mates.values()):
            duplicate_mate_groups += 1
            audit.finding(
                "pair_mate_duplicate",
                "error",
                {"source": source_id, "pair_digest": _digest(source_id, pair_key)},
                "a paired-read mate occurs more than once",
            )
        if set(mates) == {1, 2}:
            complete_pairs += 1
        else:
            incomplete_pairs += 1
            audit.finding(
                "pair_incomplete",
                "warning",
                {"source": source_id, "pair_digest": _digest(source_id, pair_key)},
                "a paired-read group contains only one mate in this source",
            )

    valid = audit.errors == 0
    publishable = valid and audit.max_loss != "blocking"
    source_digest = content_digest({"source_id": source_id, "payload": text})
    manifest = {
        "source_id": source_id,
        "source_digest": source_digest,
        "adapter": FASTQ_ADAPTER,
        "adapter_version": FASTQ_ADAPTER_VERSION,
        "declared_format": FASTQ_FORMAT,
        "record_count": len(reads),
        "provenance_digest": provenance_digest,
        "bytes_read": True,
        "read_identifiers_disclosed": False,
        "sequence_bases_disclosed": False,
        "quality_values_disclosed": False,
    }
    document: dict[str, Any] = {
        "schema": FASTQ_SCHEMA,
        "workflow": "fastq_sequence_quality_audit",
        "valid": valid,
        "publishable": publishable,
        "source_id": source_id,
        "manifest": manifest,
        "summary": {
            "reads": len(reads),
            "total_sequence_bases": total_bases,
            "sequence_length_min": min(sequence_lengths),
            "sequence_length_max": max(sequence_lengths),
            "quality_phred_min": quality_min,
            "quality_phred_max": quality_max,
            "sequence_symbol_counts": dict(sorted(symbol_counts.items())),
            "paired_read_groups": len(pair_groups),
            "complete_pairs": complete_pairs,
            "incomplete_pairs": incomplete_pairs,
            "duplicate_mate_groups": duplicate_mate_groups,
            "errors": audit.errors,
            "warnings": audit.warnings,
            "finding_count": audit.finding_count,
            "blocking_loss_count": audit.blocking_loss_count,
        },
        "reads": read_rows[:max_items],
        "omitted_reads": max(0, len(read_rows) - max_items),
        "findings": [finding.to_wire() for finding in audit.findings],
        "omitted_findings": max(0, audit.finding_count - len(audit.findings)),
        "semantic_loss": {
            "audit": "lossy" if audit.loss_count else "lossless",
            "lost_count": audit.loss_count,
            "max_severity": audit.max_loss,
            "lost": list(audit.losses),
            "omitted_lost": max(0, audit.loss_count - len(audit.losses)),
        },
        "conformance": {
            "level": "normalize",
            "passed": valid,
            "publishable": publishable,
            "checks": {
                "record_structure": "pass" if valid else "fail",
                "sequence_quality_lengths": "pass",
                "quality_printability": "pass",
                "pairing_evidence": "pass" if duplicate_mate_groups == 0 else "fail",
                "provenance": "pass" if provenance_digest is not None else "fail",
            },
            "limitations": [
                "base strings, quality strings, and read identifiers are represented by source-bound digests rather than disclosed",
                "the audit validates FASTQ structure and quality character bounds, not biological alignment, contamination, adapter content, or taxonomic identity",
                "quality scores are summarized under a conventional printable Phred+33 interpretation and are not independently calibrated",
            ],
        },
        "max_records": MAX_FASTQ_RECORDS,
        "max_items": max_items,
    }
    document["document_digest"] = content_digest(document)
    return FastqParseResult(document)


class FastqAdapter:
    """Concrete adapter facade matching the dependency-free bounded FASTQ route."""

    name = FASTQ_ADAPTER
    version = FASTQ_ADAPTER_VERSION
    accepted_formats = ("application/fastq", "text/fastq", "text/x-fastq")
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
            "scope_dimensions": ["subject", "sample", "read", "sequence", "quality"],
            "execution": "python_delegated",
            "optional_dependency": None,
        }

    def parse(
        self,
        fastq: str | bytes,
        *,
        source_id: str,
        provenance: Mapping[str, Any] | None = None,
        max_bytes: int = MAX_FASTQ_BYTES,
        max_records: int = MAX_FASTQ_RECORDS,
        max_items: int = MAX_FASTQ_ITEMS,
    ) -> FastqParseResult:
        return parse_fastq(
            fastq,
            source_id=source_id,
            provenance=provenance,
            max_bytes=max_bytes,
            max_records=max_records,
            max_items=max_items,
        )


__all__ = [
    "FASTQ_ADAPTER",
    "FASTQ_ADAPTER_VERSION",
    "FASTQ_FORMAT",
    "FASTQ_SCHEMA",
    "FastqAdapter",
    "FastqFinding",
    "FastqParseError",
    "FastqParseResult",
    "MAX_FASTQ_BYTES",
    "MAX_FASTQ_ITEMS",
    "MAX_FASTQ_RECORDS",
    "parse_fastq",
]
