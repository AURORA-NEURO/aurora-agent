"""Bounded BAM/CRAM alignment-record projection auditing.

The auditor consumes parsed alignment records and a reference dictionary. It validates CIGAR
accounting, coordinate bounds, flags, pairing metadata, coordinate sort order, and aggregate
coverage without reading BAM/CRAM bytes, decoding sequences, or opening an index.
"""

from __future__ import annotations

from dataclasses import dataclass
import hashlib
import re
from typing import Any, Mapping, Sequence

from .authoring import canonical_json, content_digest
from .errors import ArgumentError


ALIGNMENT_SCHEMA = "bioprism-python-alignment/0.1"
ALIGNMENT_ADAPTER = "bioprism.python.alignment_metadata"
ALIGNMENT_ADAPTER_VERSION = "0.1.0"
MAX_ALIGNMENT_RECORDS = 100_000
MAX_ALIGNMENT_ITEMS = 1_000
MAX_REFERENCE_LENGTH = 10_000_000_000
_NAME = re.compile(r"^[A-Za-z0-9_.:*#-]{1,255}$")
_CIGAR = re.compile(r"(\d+)([MIDNSHP=X])")
_CIGAR_OPS = set("MIDNSHP=X")
_FLAG_MASK = 0xFFF
_FLAG_PAIRED = 0x1
_FLAG_UNMAPPED = 0x4
_FLAG_MATE_UNMAPPED = 0x8
_FLAG_FIRST = 0x40
_FLAG_SECOND = 0x80
_FLAG_SECONDARY = 0x100
_FLAG_SUPPLEMENTARY = 0x800


@dataclass(frozen=True)
class AlignmentFinding:
    code: str
    severity: str
    path: str
    detail: str
    related_paths: tuple[str, ...] = ()

    def __post_init__(self) -> None:
        if self.severity not in {"error", "warning", "info"}:
            raise ArgumentError(f"invalid alignment finding severity: {self.severity!r}")

    def to_dict(self) -> dict[str, Any]:
        result: dict[str, Any] = {"code": self.code, "severity": self.severity, "path": self.path, "detail": self.detail}
        if self.related_paths:
            result["related_paths"] = list(self.related_paths)
        return result


@dataclass(frozen=True)
class AlignmentAuditResult:
    document: Mapping[str, Any]

    @property
    def valid(self) -> bool:
        return bool(self.document["valid"])

    @property
    def publishable(self) -> bool:
        return bool(self.document["publishable"])

    @property
    def findings(self) -> Sequence[Mapping[str, Any]]:
        return self.document["findings"]

    def to_wire(self) -> dict[str, Any]:
        return dict(self.document)


@dataclass(frozen=True)
class _Record:
    record_id: str
    read_id: str
    reference_name: str | None
    start: int | None
    end: int | None
    cigar: str | None
    query_span: int
    reference_span: int
    flags: int
    mapping_quality: int | None
    sequence_length: int | None
    mate_reference_name: str | None
    mate_start: int | None
    template_length: int | None
    read_group: str | None

    @property
    def mapped(self) -> bool:
        return self.reference_name is not None and not (self.flags & _FLAG_UNMAPPED)

    @property
    def duplicate(self) -> bool:
        return bool(self.flags & 0x400)

    @property
    def secondary(self) -> bool:
        return bool(self.flags & _FLAG_SECONDARY)

    @property
    def supplementary(self) -> bool:
        return bool(self.flags & _FLAG_SUPPLEMENTARY)


class _Audit:
    def __init__(self, limit: int) -> None:
        self.limit = limit
        self.findings: list[AlignmentFinding] = []
        self.total = 0
        self.error_count = 0
        self.warning_count = 0
        self.codes: set[str] = set()
        self.losses: list[dict[str, Any]] = []
        self.loss_total = 0
        self.blocking_loss_count = 0
        self.max_loss_severity: str | None = None

    def add(self, code: str, severity: str, path: str, detail: str, related_paths: Sequence[str] = ()) -> None:
        self.total += 1
        self.codes.add(code)
        if severity == "error":
            self.error_count += 1
        elif severity == "warning":
            self.warning_count += 1
        if len(self.findings) < self.limit:
            self.findings.append(AlignmentFinding(code, severity, path, detail, tuple(related_paths)))

    def loss(self, kind: str, severity: str, path: str, detail: str, related_paths: Sequence[str] = ()) -> None:
        self.loss_total += 1
        if severity == "blocking":
            self.blocking_loss_count += 1
        ranks = {"minor": 1, "major": 2, "blocking": 3}
        if self.max_loss_severity is None or ranks[severity] > ranks[self.max_loss_severity]:
            self.max_loss_severity = severity
        if len(self.losses) < self.limit:
            entry: dict[str, Any] = {"kind": kind, "severity": severity, "path": path, "detail": detail}
            if related_paths:
                entry["related_paths"] = list(related_paths)
            self.losses.append(entry)


class AlignmentAdapter:
    name = ALIGNMENT_ADAPTER
    version = ALIGNMENT_ADAPTER_VERSION
    accepted_formats = ("application/alignment-manifest",)
    declared_loss_kinds = frozenset({"coordinate_frame_not_carried", "provenance_unavailable", "content_uninterpreted", "type_undetermined"})

    def manifest(self) -> dict[str, Any]:
        return {
            "name": self.name,
            "version": self.version,
            "accepted_formats": list(self.accepted_formats),
            "conformance_level": "normalize",
            "declared_loss_kinds": sorted(self.declared_loss_kinds),
            "scope_dimensions": ["subject", "sample", "read", "reference", "locus"],
            "execution": "python_delegated",
            "optional_dependency": None,
        }

    def audit(
        self,
        references: Mapping[str, int],
        records: Sequence[Mapping[str, Any]],
        *,
        source_id: str,
        reference_build: str | None = None,
        provenance: Mapping[str, Any] | None = None,
        max_records: int = MAX_ALIGNMENT_RECORDS,
        max_items: int = MAX_ALIGNMENT_ITEMS,
    ) -> AlignmentAuditResult:
        return audit_alignments(
            references,
            records,
            source_id=source_id,
            reference_build=reference_build,
            provenance=provenance,
            max_records=max_records,
            max_items=max_items,
        )


def _text(name: str, value: str, maximum: int = 512) -> None:
    if not isinstance(value, str) or not value.strip():
        raise ArgumentError(f"{name} must be a non-empty string")
    if any(ord(character) < 0x20 for character in value):
        raise ArgumentError(f"{name} must not contain control characters")
    if len(value.encode("utf-8")) > maximum:
        raise ArgumentError(f"{name} exceeds the {maximum}-byte limit")


def _limit(name: str, value: int, maximum: int) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or not 1 <= value <= maximum:
        raise ArgumentError(f"{name} must be between 1 and {maximum}")
    return value


def _name(value: Any, *, path: str, field: str, audit: _Audit) -> str | None:
    if not isinstance(value, str) or not _NAME.fullmatch(value):
        audit.add("name_invalid", "error", path, f"{field} must be a bounded reference-safe name")
        return None
    return value


def _nonnegative(value: Any, *, path: str, field: str, audit: _Audit) -> int | None:
    if isinstance(value, bool) or not isinstance(value, int) or value < 0:
        audit.add("integer_invalid", "error", path, f"{field} must be a non-negative integer")
        return None
    return value


def _cigar(value: Any, *, path: str, audit: _Audit) -> tuple[str | None, int, int, int]:
    if value is None or value == "*":
        return None, 0, 0, 0
    if not isinstance(value, str) or len(value) > 10_000:
        audit.add("cigar_invalid", "error", path, "CIGAR must be a bounded string")
        return None, 0, 0, 0
    cursor = 0
    query_span = 0
    reference_span = 0
    operation_count = 0
    previous: str | None = None
    for match in _CIGAR.finditer(value):
        if match.start() != cursor:
            audit.add("cigar_invalid", "error", path, "CIGAR contains an unparsed operation")
            return None, 0, 0, 0
        length = int(match.group(1))
        operation = match.group(2)
        if operation not in _CIGAR_OPS or length <= 0:
            audit.add("cigar_invalid", "error", path, "CIGAR operation has invalid length or code")
            return None, 0, 0, 0
        if operation == previous and operation not in {"H", "S"}:
            audit.add("cigar_noncanonical", "warning", path, "adjacent identical CIGAR operations should be merged")
        if operation in "MIS=X":
            query_span += length
        if operation in "MDN=X":
            reference_span += length
        previous = operation
        cursor = match.end()
        operation_count += 1
    if cursor != len(value) or operation_count == 0:
        audit.add("cigar_invalid", "error", path, "CIGAR contains no complete operation sequence")
        return None, 0, 0, 0
    return value, query_span, reference_span, operation_count


def _parse_record(mapping: Mapping[str, Any], index: int, audit: _Audit) -> _Record:
    path = f"records[{index}]"
    record_id = mapping.get("record_id", f"record-{index}")
    if not isinstance(record_id, str) or not record_id.strip():
        audit.add("record_id_invalid", "error", path, "record_id must be a non-empty string")
        record_id = f"record-{index}"
    else:
        try:
            _text("record_id", record_id, 512)
        except ArgumentError as error:
            audit.add("record_id_invalid", "error", path, str(error))
            record_id = f"record-{index}"
    read_id = mapping.get("read_id")
    if not isinstance(read_id, str) or not read_id:
        audit.add("read_id_invalid", "error", record_id, "read_id must be a non-empty string")
        read_id = record_id
    reference_name = mapping.get("reference_name")
    if reference_name is not None and _name(reference_name, path=record_id, field="reference_name", audit=audit) is None:
        reference_name = None
    start = _nonnegative(mapping.get("start"), path=record_id, field="start", audit=audit) if mapping.get("start") is not None else None
    cigar, query_span, reference_span, _ = _cigar(mapping.get("cigar"), path=record_id, audit=audit)
    flags = mapping.get("flags", 0)
    if isinstance(flags, bool) or not isinstance(flags, int) or flags < 0 or flags > 0xFFFF:
        audit.add("flags_invalid", "error", record_id, "flags must be a 16-bit non-negative integer")
        flags = 0
    if flags & ~_FLAG_MASK:
        audit.add("flags_unknown", "warning", record_id, "flags contain non-standard bits outside the bounded mask")
    mapping_quality = mapping.get("mapping_quality")
    if mapping_quality is not None and (isinstance(mapping_quality, bool) or not isinstance(mapping_quality, int) or not 0 <= mapping_quality <= 255):
        audit.add("mapping_quality_invalid", "error", record_id, "mapping_quality must be from 0 through 255")
        mapping_quality = None
    sequence_length = mapping.get("sequence_length")
    if sequence_length is not None:
        sequence_length = _nonnegative(sequence_length, path=record_id, field="sequence_length", audit=audit)
        expected = query_span
        if sequence_length is not None and expected and sequence_length != expected:
            audit.add("sequence_length_mismatch", "error", record_id, f"sequence_length {sequence_length} disagrees with CIGAR query span {expected}")
    reference_end = mapping.get("reference_end")
    if reference_end is not None:
        reference_end = _nonnegative(reference_end, path=record_id, field="reference_end", audit=audit)
        if start is not None and reference_end is not None and reference_end < start:
            audit.add("coordinate_invalid", "error", record_id, "reference_end must not precede start")
        if start is not None and reference_end is not None and reference_span and reference_end - start != reference_span:
            audit.add("cigar_coordinate_mismatch", "error", record_id, "reference_end-start disagrees with CIGAR reference span")
    elif start is not None and reference_name is not None and cigar is not None:
        reference_end = start + reference_span
    mate_reference_name = mapping.get("mate_reference_name")
    if mate_reference_name is not None and _name(mate_reference_name, path=record_id, field="mate_reference_name", audit=audit) is None:
        mate_reference_name = None
    mate_start = _nonnegative(mapping.get("mate_start"), path=record_id, field="mate_start", audit=audit) if mapping.get("mate_start") is not None else None
    template_length = mapping.get("template_length")
    if template_length is not None and (isinstance(template_length, bool) or not isinstance(template_length, int)):
        audit.add("template_length_invalid", "error", record_id, "template_length must be an integer")
        template_length = None
    read_group = mapping.get("read_group")
    if read_group is not None and _name(read_group, path=record_id, field="read_group", audit=audit) is None:
        read_group = None
    if not (flags & _FLAG_PAIRED) and flags & (_FLAG_FIRST | _FLAG_SECOND):
        audit.add("pair_flags_invalid", "error", record_id, "first/second-of-pair flags require the paired flag")
    if flags & _FLAG_FIRST and flags & _FLAG_SECOND:
        audit.add("pair_flags_invalid", "error", record_id, "a record cannot be both first and second of pair")
    if flags & _FLAG_UNMAPPED and (reference_name is not None or start is not None or cigar is not None):
        audit.add("unmapped_coordinates_present", "warning", record_id, "unmapped record carries coordinate fields")
    return _Record(
        record_id,
        read_id,
        reference_name,
        start,
        reference_end,
        cigar,
        query_span,
        reference_span,
        flags,
        mapping_quality,
        sequence_length,
        mate_reference_name,
        mate_start,
        template_length,
        read_group,
    )


def _validate_provenance(provenance: Mapping[str, Any] | None, audit: _Audit) -> str | None:
    if not provenance:
        audit.loss("provenance_unavailable", "blocking", "provenance", "no non-empty provenance projection was supplied")
        return None
    try:
        encoded = canonical_json(dict(provenance)).encode("utf-8")
    except Exception as error:  # noqa: BLE001
        audit.add("provenance_not_json", "error", "provenance", f"provenance is not canonical JSON-safe: {error}")
        audit.loss("provenance_unavailable", "blocking", "provenance", "provenance could not be represented canonically")
        return None
    if len(encoded) > 10_000_000:
        audit.add("provenance_too_large", "error", "provenance", "provenance exceeds the bounded audit limit")
        audit.loss("provenance_unavailable", "blocking", "provenance", "provenance exceeds the bounded audit limit")
        return None
    return content_digest(dict(provenance))


def audit_alignments(
    references: Mapping[str, int],
    records: Sequence[Mapping[str, Any]],
    *,
    source_id: str,
    reference_build: str | None = None,
    provenance: Mapping[str, Any] | None = None,
    max_records: int = MAX_ALIGNMENT_RECORDS,
    max_items: int = MAX_ALIGNMENT_ITEMS,
) -> AlignmentAuditResult:
    """Audit parsed BAM/CRAM alignment records using 0-based half-open coordinates."""

    _text("source_id", source_id)
    max_records = _limit("max_records", max_records, MAX_ALIGNMENT_RECORDS)
    max_items = _limit("max_items", max_items, MAX_ALIGNMENT_ITEMS)
    if not isinstance(references, Mapping) or not references:
        raise ArgumentError("references must be a non-empty mapping of contig names to lengths")
    if isinstance(records, (str, bytes)) or not isinstance(records, Sequence):
        raise ArgumentError("records must be a sequence of parsed alignment mappings")
    if len(records) == 0 or len(records) > max_records:
        raise ArgumentError(f"records must contain between 1 and {max_records} projections")
    if provenance is not None and not isinstance(provenance, Mapping):
        raise ArgumentError("provenance must be a JSON-object mapping when supplied")
    if reference_build is not None:
        _text("reference_build", reference_build, 256)

    audit = _Audit(max_items)
    audit.loss("content_uninterpreted", "minor", source_id, "BAM/CRAM bytes, sequences, qualities, and auxiliary tags were not decoded")
    if reference_build is None:
        audit.loss("coordinate_frame_not_carried", "blocking", "reference_build", "reference build was not supplied")
    provenance_digest = _validate_provenance(provenance, audit)
    reference_lengths: dict[str, int] = {}
    for name, length in references.items():
        if _name(name, path=f"references.{name}", field="reference name", audit=audit) is None:
            continue
        if isinstance(length, bool) or not isinstance(length, int) or not 0 < length <= MAX_REFERENCE_LENGTH:
            audit.add("reference_length_invalid", "error", f"references.{name}", f"reference length must be from 1 through {MAX_REFERENCE_LENGTH}")
            continue
        reference_lengths[name] = length
    parsed: list[_Record] = []
    for index, mapping in enumerate(records):
        if not isinstance(mapping, Mapping):
            audit.add("record_not_mapping", "error", f"records[{index}]", "each alignment projection must be a JSON object")
            continue
        parsed.append(_parse_record(mapping, index, audit))

    record_ids: dict[str, str] = {}
    read_groups: dict[str, list[_Record]] = {}
    reference_rank = {name: rank for rank, name in enumerate(reference_lengths)}
    previous_key: tuple[int, int, str] | None = None
    coverage: dict[str, dict[str, Any]] = {name: {"record_count": 0, "mapped_bases": 0, "duplicate_count": 0, "mapping_quality_sum": 0, "mapping_quality_count": 0} for name in reference_lengths}
    mapped_count = 0
    unmapped_count = 0
    record_rows: list[dict[str, Any]] = []
    for record in parsed:
        if record.record_id in record_ids:
            audit.add("record_id_duplicate", "error", record.record_id, "record_id occurs more than once", (record_ids[record.record_id],))
        else:
            record_ids[record.record_id] = record.record_id
        read_groups.setdefault(record.read_id, []).append(record)
        if record.reference_name is None or record.flags & _FLAG_UNMAPPED:
            unmapped_count += 1
        else:
            mapped_count += 1
            if record.reference_name not in reference_lengths:
                audit.add("reference_unknown", "error", record.record_id, f"reference {record.reference_name!r} is absent from the dictionary")
            elif record.start is None or record.end is None:
                audit.add("coordinate_missing", "error", record.record_id, "mapped record requires start and end coordinates")
            else:
                if record.end > reference_lengths[record.reference_name]:
                    audit.add("coordinate_out_of_bounds", "error", record.record_id, "alignment end exceeds reference length")
                bucket = coverage[record.reference_name]
                bucket["record_count"] += 1
                bucket["mapped_bases"] += record.reference_span
                bucket["duplicate_count"] += int(record.duplicate)
                if record.mapping_quality is not None:
                    bucket["mapping_quality_sum"] += record.mapping_quality
                    bucket["mapping_quality_count"] += 1
                key = (reference_rank.get(record.reference_name, len(reference_rank)), record.start, record.record_id)
                if previous_key is not None and key < previous_key:
                    audit.add("coordinate_sort_violation", "error", record.record_id, "mapped records are not in reference/start order")
                previous_key = key
        record_rows.append(
            {
                "record_id": record.record_id,
                "read_id_digest": hashlib.sha256((source_id + "\0" + record.read_id).encode("utf-8")).hexdigest()[:24],
                "reference_name": record.reference_name,
                "start": record.start,
                "end": record.end,
                "reference_span": record.reference_span,
                "query_span": record.query_span,
                "flags": record.flags,
                "mapped": record.mapped,
                "mapping_quality": record.mapping_quality,
                "paired": bool(record.flags & _FLAG_PAIRED),
                "secondary": record.secondary,
                "supplementary": record.supplementary,
                "duplicate": record.duplicate,
                "read_group": record.read_group,
            }
        )

    for read_id, group in read_groups.items():
        paired = [record for record in group if record.flags & _FLAG_PAIRED and not record.secondary and not record.supplementary]
        first = [record for record in paired if record.flags & _FLAG_FIRST]
        second = [record for record in paired if record.flags & _FLAG_SECOND]
        if paired and (len(first) > 1 or len(second) > 1):
            audit.add("pair_duplicate_mate", "error", paired[0].record_id, "a primary read has multiple first/second mate records")
        if len(first) != len(second):
            audit.add("pair_mate_missing", "warning", paired[0].record_id, "paired primary records do not contain both first and second mates")

    coverage_rows: list[dict[str, Any]] = []
    for name, length in reference_lengths.items():
        bucket = coverage[name]
        quality_count = bucket.pop("mapping_quality_count")
        quality_sum = bucket.pop("mapping_quality_sum")
        coverage_rows.append(
            {
                "reference_name": name,
                "length": length,
                "record_count": bucket["record_count"],
                "mapped_bases": bucket["mapped_bases"],
                "duplicate_count": bucket["duplicate_count"],
                "mean_mapping_quality": quality_sum / quality_count if quality_count else None,
            }
        )

    try:
        source_digest = content_digest({"source_id": source_id, "references": dict(references), "records": [dict(record) for record in records]})
    except Exception as error:  # noqa: BLE001
        audit.add("projection_not_json", "error", source_id, f"alignment projection is not canonical JSON-safe: {error}")
        source_digest = content_digest({"source_id": source_id, "record_ids": [row["record_id"] for row in record_rows]})
    valid = audit.error_count == 0
    publishable = valid and audit.blocking_loss_count == 0
    document: dict[str, Any] = {
        "schema": ALIGNMENT_SCHEMA,
        "workflow": "alignment_projection_audit",
        "valid": valid,
        "publishable": publishable,
        "source_id": source_id,
        "manifest": {
            "source_id": source_id,
            "source_digest": source_digest,
            "adapter": ALIGNMENT_ADAPTER,
            "adapter_version": ALIGNMENT_ADAPTER_VERSION,
            "declared_format": "application/alignment-manifest",
            "reference_build": reference_build,
            "reference_count": len(reference_lengths),
            "record_count": len(parsed),
            "mapped_count": mapped_count,
            "unmapped_count": unmapped_count,
            "read_group_count": len({record.read_group for record in parsed if record.read_group}),
            "bytes_read": False,
        },
        "summary": {
            "references": len(reference_lengths),
            "records": len(parsed),
            "mapped": mapped_count,
            "unmapped": unmapped_count,
            "paired_reads": sum(1 for group in read_groups.values() if any(record.flags & _FLAG_PAIRED for record in group)),
            "errors": audit.error_count,
            "warnings": audit.warning_count,
            "finding_count": audit.total,
            "blocking_loss_count": audit.blocking_loss_count,
        },
        "references": [{"name": name, "length": length} for name, length in reference_lengths.items()][:max_items],
        "omitted_references": max(0, len(reference_lengths) - max_items),
        "coverage": coverage_rows[:max_items],
        "omitted_coverage": max(0, len(coverage_rows) - max_items),
        "records": record_rows[:max_items],
        "omitted_records": max(0, len(record_rows) - max_items),
        "findings": [finding.to_dict() for finding in audit.findings],
        "omitted_findings": max(0, audit.total - len(audit.findings)),
        "semantic_loss": {
            "audit": "lossy" if audit.loss_total else "lossless",
            "lost_count": audit.loss_total,
            "max_severity": audit.max_loss_severity,
            "lost": audit.losses,
            "omitted_lost": max(0, audit.loss_total - len(audit.losses)),
        },
        "conformance": {
            "passed": valid,
            "publishable": publishable,
            "checks": {
                "reference_dictionary": "pass" if not any(code in audit.codes for code in {"reference_length_invalid", "name_invalid"}) else "fail",
                "cigar": "pass" if not any(code in audit.codes for code in {"cigar_invalid", "cigar_coordinate_mismatch", "sequence_length_mismatch"}) else "fail",
                "coordinates": "pass" if not any(code in audit.codes for code in {"reference_unknown", "coordinate_missing", "coordinate_out_of_bounds", "coordinate_invalid"}) else "fail",
                "pairing": "pass" if not any(code in audit.codes for code in {"pair_flags_invalid", "pair_duplicate_mate"}) else "fail",
                "sort_order": "pass" if "coordinate_sort_violation" not in audit.codes else "fail",
                "provenance": "pass" if provenance_digest is not None and reference_build is not None else "loss",
            },
            "limitations": [
                "the audit consumes caller-supplied parsed records and does not access BAM/CRAM bytes or indexes",
                "read sequences, qualities, auxiliary tags, base-level mismatches, and reference sequence content are not decoded",
                "a valid report proves only the bounded coordinate, CIGAR, flag, pairing, sorting, and aggregate checks represented here",
            ],
        },
        "max_records": max_records,
        "max_items": max_items,
    }
    document["document_digest"] = content_digest(document)
    return AlignmentAuditResult(document)


__all__ = [
    "ALIGNMENT_ADAPTER",
    "ALIGNMENT_ADAPTER_VERSION",
    "ALIGNMENT_SCHEMA",
    "AlignmentAdapter",
    "AlignmentAuditResult",
    "AlignmentFinding",
    "MAX_ALIGNMENT_ITEMS",
    "MAX_ALIGNMENT_RECORDS",
    "audit_alignments",
]
