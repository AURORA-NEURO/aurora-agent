"""Bounded BED3--BED12 interval auditing without label or sequence disclosure.

The BED format is deceptively small: a three-column interval can be used as a simple region
list, while BED12 adds thick coding bounds, display color, and a block structure that is often
used for transcripts.  This module validates the complete bounded stream, retains interval
geometry and aggregate evidence, and keeps chromosome/name labels source-bound.  It deliberately
does not infer an assembly, gene identity, ontology, or biological meaning from a coordinate.
"""

from __future__ import annotations

from collections import Counter
from dataclasses import dataclass
import re
from typing import Any, Mapping

from .authoring import content_digest
from .errors import ArgumentError


BED_SCHEMA = "bioprism-python-bed/0.1"
BED_ADAPTER = "bioprism.python.bed_text"
BED_ADAPTER_VERSION = "0.1.0"
BED_FORMAT = "text/bed"
MAX_BED_BYTES = 50_000_000
MAX_BED_FEATURES = 500_000
MAX_BED_ITEMS = 1_000
MAX_BED_BLOCKS = 100_000
MAX_BED_COORDINATE = 2**63 - 1
MAX_BED_LINE_BYTES = 1_000_000
_SEVERITY_ORDER = {"advisory": 0, "degrading": 1, "blocking": 2}
_UNSIGNED_INTEGER = re.compile(r"^[0-9]+$")
_CHROMOSOME = re.compile(r"^[^\s\x00-\x1f\x7f]+$")


class BedParseError(ArgumentError):
    """A structurally invalid BED source with a stable line/feature locator."""

    def __init__(self, message: str, *, line: int | None = None, feature: int | None = None) -> None:
        location = ""
        if line is not None:
            location += f" at line {line}"
        if feature is not None:
            location += f" feature {feature}"
        super().__init__(f"BED parse refused{location}: {message}")


@dataclass(frozen=True)
class BedFinding:
    """One bounded BED audit finding."""

    code: str
    severity: str
    location: Mapping[str, Any]
    detail: str

    def __post_init__(self) -> None:
        if self.severity not in {"warning", "error"}:
            raise ArgumentError(f"unsupported BED finding severity: {self.severity!r}")

    def to_wire(self) -> dict[str, Any]:
        return {
            "code": self.code,
            "severity": self.severity,
            "location": dict(self.location),
            "detail": self.detail,
        }


@dataclass(frozen=True)
class BedParseResult:
    """A validated, bounded BED interval projection."""

    document: Mapping[str, Any]

    @property
    def features(self) -> list[Mapping[str, Any]]:
        return list(self.document["features"])

    @property
    def valid(self) -> bool:
        return bool(self.document["valid"])

    @property
    def publishable(self) -> bool:
        return bool(self.document["publishable"])

    def to_wire(self) -> dict[str, Any]:
        return dict(self.document)


@dataclass(frozen=True)
class _Interval:
    chrom: str
    start: int
    end: int
    name: str | None
    score: int | None
    strand: str | None
    thick_start: int | None
    thick_end: int | None
    item_rgb: tuple[int, int, int] | None
    block_sizes: tuple[int, ...]
    block_starts: tuple[int, ...]
    line: int


class _Audit:
    def __init__(self, limit: int) -> None:
        self.limit = limit
        self.findings: list[BedFinding] = []
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
            self.findings.append(BedFinding(code, severity, dict(location), detail))

    def loss(self, kind: str, severity: str, location: str, detail: str) -> None:
        self.loss_count += 1
        if severity == "blocking":
            self.blocking_loss_count += 1
        if self.max_loss_severity is None or _SEVERITY_ORDER[severity] > _SEVERITY_ORDER[self.max_loss_severity]:
            self.max_loss_severity = severity
        if len(self.losses) < self.limit:
            self.losses.append({"kind": kind, "severity": severity, "location": location, "detail": detail})

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
    max_bytes = _validate_limit("max_bytes", max_bytes, MAX_BED_BYTES)
    if isinstance(payload, bytes):
        if len(payload) > max_bytes:
            raise ArgumentError(f"BED exceeds the {max_bytes}-byte limit")
        try:
            return payload.decode("utf-8")
        except UnicodeDecodeError as error:
            raise ArgumentError(f"BED is not valid UTF-8: {error}") from error
    if isinstance(payload, str):
        try:
            encoded_length = len(payload.encode("utf-8"))
        except UnicodeEncodeError as error:
            raise ArgumentError(f"BED is not valid UTF-8 text: {error}") from error
        if encoded_length > max_bytes:
            raise ArgumentError(f"BED exceeds the {max_bytes}-byte limit")
        return payload
    raise ArgumentError("BED payload must be text or bytes")


def _integer(raw: str, *, label: str, line: int, feature: int, maximum: int = MAX_BED_COORDINATE) -> int:
    if not _UNSIGNED_INTEGER.fullmatch(raw):
        raise BedParseError(f"{label} must be a non-negative decimal integer", line=line, feature=feature)
    value = int(raw)
    if value > maximum:
        raise BedParseError(f"{label} exceeds the {maximum} coordinate bound", line=line, feature=feature)
    return value


def _parse_blocks(raw_sizes: str, raw_starts: str, *, start: int, end: int, line: int, feature: int) -> tuple[tuple[int, ...], tuple[int, ...]]:
    def split_list(raw: str, label: str) -> list[str]:
        values = raw.split(",")
        if values and values[-1] == "":
            values.pop()
        if not values or any(value == "" for value in values):
            raise BedParseError(f"{label} must be a comma-separated non-empty integer list", line=line, feature=feature)
        return values

    size_tokens = split_list(raw_sizes, "blockSizes")
    start_tokens = split_list(raw_starts, "blockStarts")
    if len(size_tokens) != len(start_tokens):
        raise BedParseError("blockSizes and blockStarts must have equal cardinality", line=line, feature=feature)
    if len(size_tokens) > MAX_BED_BLOCKS:
        raise BedParseError(f"BED feature exceeds the {MAX_BED_BLOCKS}-block limit", line=line, feature=feature)
    sizes = tuple(_integer(value, label="block size", line=line, feature=feature, maximum=MAX_BED_COORDINATE) for value in size_tokens)
    starts = tuple(_integer(value, label="block start", line=line, feature=feature, maximum=MAX_BED_COORDINATE) for value in start_tokens)
    interval_length = end - start
    previous_end = -1
    for block_start, block_size in zip(starts, sizes):
        if block_size == 0:
            raise BedParseError("block sizes must be positive", line=line, feature=feature)
        block_end = block_start + block_size
        if block_end > interval_length:
            raise BedParseError("BED block extends beyond the parent interval", line=line, feature=feature)
        if block_start < previous_end:
            raise BedParseError("BED blocks must be ordered and non-overlapping", line=line, feature=feature)
        previous_end = block_end
    return sizes, starts


def _parse_rgb(raw: str, *, line: int, feature: int) -> tuple[int, int, int] | None:
    if raw == ".":
        return None
    values = raw.split(",")
    if len(values) != 3:
        raise BedParseError("itemRgb must be '.' or three comma-separated channels", line=line, feature=feature)
    channels = tuple(_integer(value, label="itemRgb channel", line=line, feature=feature, maximum=255) for value in values)
    return channels  # type: ignore[return-value]


def _parse_interval(line_text: str, *, line: int, feature: int) -> _Interval:
    if " " in line_text or "\v" in line_text or "\f" in line_text:
        raise BedParseError("BED rows must be tab-delimited and must not contain unescaped spaces", line=line, feature=feature)
    columns = line_text.split("\t")
    if not 3 <= len(columns) <= 12:
        raise BedParseError("BED row must contain between three and twelve tab-separated columns", line=line, feature=feature)
    chrom, raw_start, raw_end = columns[:3]
    if not chrom or _CHROMOSOME.fullmatch(chrom) is None:
        raise BedParseError("chrom must be non-empty and contain no whitespace or control characters", line=line, feature=feature)
    start = _integer(raw_start, label="chromStart", line=line, feature=feature)
    end = _integer(raw_end, label="chromEnd", line=line, feature=feature)
    if end <= start:
        raise BedParseError("coordinates must satisfy chromStart < chromEnd", line=line, feature=feature)

    name = columns[3] if len(columns) >= 4 and columns[3] not in {"", "."} else None
    score: int | None = None
    if len(columns) >= 5 and columns[4] != ".":
        score = _integer(columns[4], label="score", line=line, feature=feature, maximum=1000)
    strand: str | None = None
    if len(columns) >= 6:
        strand = columns[5]
        if strand not in {"+", "-", "."}:
            raise BedParseError("strand must be '+', '-', or '.'", line=line, feature=feature)

    thick_start: int | None = None
    thick_end: int | None = None
    if len(columns) >= 7:
        if len(columns) < 8:
            raise BedParseError("thickStart and thickEnd must be supplied together", line=line, feature=feature)
        thick_start = _integer(columns[6], label="thickStart", line=line, feature=feature)
        thick_end = _integer(columns[7], label="thickEnd", line=line, feature=feature)
        if thick_start < start or thick_end > end or thick_end < thick_start:
            raise BedParseError("thickStart/thickEnd must be an ordered subinterval of the BED interval", line=line, feature=feature)

    item_rgb: tuple[int, int, int] | None = None
    if len(columns) >= 9:
        item_rgb = _parse_rgb(columns[8], line=line, feature=feature)

    if len(columns) >= 10:
        block_count = _integer(columns[9], label="blockCount", line=line, feature=feature, maximum=MAX_BED_BLOCKS)
        if block_count == 0:
            raise BedParseError("blockCount must be positive", line=line, feature=feature)
        if len(columns) < 12:
            raise BedParseError("blockCount requires blockSizes and blockStarts", line=line, feature=feature)
        block_sizes, block_starts = _parse_blocks(columns[10], columns[11], start=start, end=end, line=line, feature=feature)
        if len(block_sizes) != block_count:
            raise BedParseError("blockCount does not match blockSizes/blockStarts", line=line, feature=feature)
    else:
        block_sizes = (end - start,)
        block_starts = (0,)

    return _Interval(chrom, start, end, name, score, strand, thick_start, thick_end, item_rgb, block_sizes, block_starts, line)


def parse_bed(
    payload: str | bytes,
    *,
    source_id: str,
    provenance: Mapping[str, Any] | None = None,
    max_bytes: int = MAX_BED_BYTES,
    max_features: int = MAX_BED_FEATURES,
    max_items: int = MAX_BED_ITEMS,
) -> BedParseResult:
    """Parse bounded BED3--BED12 rows and audit interval/block integrity."""

    if not isinstance(source_id, str) or not source_id.strip():
        raise ArgumentError("source_id must be a non-empty string")
    if provenance is not None and not isinstance(provenance, Mapping):
        raise ArgumentError("provenance must be a mapping when supplied")
    max_features = _validate_limit("max_features", max_features, MAX_BED_FEATURES)
    max_items = _validate_limit("max_items", max_items, MAX_BED_ITEMS)
    text = _decode(payload, max_bytes=max_bytes)
    if not text:
        raise BedParseError("source is empty")

    lines = text.split("\n")
    if lines and lines[-1] == "":
        lines.pop()
    intervals: list[_Interval] = []
    directives = 0
    comments = 0
    for line_number, raw_line in enumerate(lines, start=1):
        line = raw_line[:-1] if raw_line.endswith("\r") else raw_line
        if "\r" in line:
            raise BedParseError("lone carriage return is not a record separator", line=line_number)
        if len(line.encode("utf-8")) > MAX_BED_LINE_BYTES:
            raise BedParseError(f"line exceeds the {MAX_BED_LINE_BYTES}-byte bound", line=line_number)
        if not line:
            raise BedParseError("blank lines are not accepted between BED records", line=line_number)
        if line.startswith("#"):
            comments += 1
            continue
        if line == "track" or line.startswith("track ") or line == "browser" or line.startswith("browser "):
            directives += 1
            continue
        if len(intervals) >= max_features:
            raise ArgumentError(f"BED contains more than the {max_features}-feature limit")
        intervals.append(_parse_interval(line, line=line_number, feature=len(intervals) + 1))
    if not intervals:
        raise BedParseError("source contains no interval records")

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
        "chromosome and item labels are not emitted; bounded interval geometry, structural fields, and source-bound digests are carried",
    )
    audit.loss(
        "coordinate_frame_not_carried",
        "degrading",
        "coordinates",
        "coordinates are validated as zero-based half-open source-local intervals; assembly/reference-build identity is not inferred",
    )
    audit.loss(
        "ontology_term_unmapped",
        "degrading",
        "interval_labels",
        "BED names and track metadata are not resolved against an external ontology, annotation release, or vocabulary",
    )
    if provenance_digest is None:
        audit.loss("provenance_unavailable", "blocking", "provenance", "no non-empty provenance projection was supplied")

    chrom_counts: Counter[str] = Counter()
    strand_counts: Counter[str] = Counter()
    score_values: list[int] = []
    block_counts: Counter[int] = Counter()
    seen_intervals: set[tuple[str, int, int]] = set()
    seen_names: set[str] = set()
    record_rows: list[dict[str, Any]] = []
    total_span = 0
    total_block_span = 0
    duplicate_interval_count = 0
    duplicate_name_count = 0
    coordinate_sorted = True
    previous_key: tuple[str, int, int] | None = None
    for number, interval in enumerate(intervals, start=1):
        location = {"source": source_id, "feature": number, "line": interval.line}
        key = (interval.chrom, interval.start, interval.end)
        if previous_key is not None and key < previous_key:
            coordinate_sorted = False
        previous_key = key
        if key in seen_intervals:
            duplicate_interval_count += 1
            audit.finding("interval_duplicate", "warning", location, "identical chromosome and interval coordinates occur more than once")
        seen_intervals.add(key)
        if interval.name is not None:
            if interval.name in seen_names:
                duplicate_name_count += 1
                audit.finding("name_duplicate", "warning", location, "BED name occurs more than once; the name is not echoed")
            seen_names.add(interval.name)
        chrom_counts[interval.chrom] += 1
        if interval.strand is not None:
            strand_counts[interval.strand] += 1
        if interval.score is not None:
            score_values.append(interval.score)
        block_counts[len(interval.block_sizes)] += 1
        span = interval.end - interval.start
        block_span = sum(interval.block_sizes)
        total_span += span
        total_block_span += block_span
        row: dict[str, Any] = {
            "feature": number,
            "line": interval.line,
            "interval_digest": _digest(source_id, f"{interval.chrom}:{interval.start}-{interval.end}"),
            "chrom_digest": _digest(source_id, interval.chrom),
            "name_digest": _digest(source_id, interval.name) if interval.name is not None else None,
            "name_present": interval.name is not None,
            "start": interval.start,
            "end": interval.end,
            "span": span,
            "score": interval.score,
            "score_present": interval.score is not None,
            "strand": interval.strand,
            "thick_start": interval.thick_start,
            "thick_end": interval.thick_end,
            "item_rgb": list(interval.item_rgb) if interval.item_rgb is not None else None,
            "block_count": len(interval.block_sizes),
            "block_sizes": list(interval.block_sizes),
            "block_starts": list(interval.block_starts),
            "block_span": block_span,
        }
        record_rows.append(row)
    if not coordinate_sorted:
        audit.finding("coordinate_sort_violation", "warning", {"source": source_id}, "intervals are not ordered by chromosome label and zero-based coordinates")
    if not score_values:
        audit.finding("score_missing", "warning", {"source": source_id}, "no interval has a numeric BED score")

    source_digest = content_digest({"source_id": source_id, "payload": text})
    valid = audit.errors == 0
    publishable = valid and audit.max_loss_severity != "blocking"
    manifest = {
        "source_id": source_id,
        "source_digest": source_digest,
        "adapter": BED_ADAPTER,
        "adapter_version": BED_ADAPTER_VERSION,
        "declared_format": BED_FORMAT,
        "feature_count": len(intervals),
        "directive_count": directives,
        "comment_count": comments,
        "bed_columns_max": max(len(line.split("\t")) for line in lines if not line.startswith(("#", "track", "browser"))),
        "provenance_digest": provenance_digest,
        "bytes_read": True,
        "chromosome_labels_disclosed": False,
        "item_names_disclosed": False,
        "track_metadata_disclosed": False,
    }
    document: dict[str, Any] = {
        "schema": BED_SCHEMA,
        "workflow": "bed_interval_block_audit",
        "valid": valid,
        "publishable": publishable,
        "source_id": source_id,
        "manifest": manifest,
        "summary": {
            "features": len(intervals),
            "unique_chromosome_count": len(chrom_counts),
            "chromosome_feature_counts": { _digest(source_id, chrom): count for chrom, count in sorted(chrom_counts.items()) },
            "total_span": total_span,
            "total_block_span": total_block_span,
            "total_blocks": sum(len(interval.block_sizes) for interval in intervals),
            "block_count_distribution": {str(count): value for count, value in sorted(block_counts.items())},
            "strand_counts": dict(sorted(strand_counts.items())),
            "scored_features": len(score_values),
            "score_min": min(score_values) if score_values else None,
            "score_max": max(score_values) if score_values else None,
            "duplicate_interval_count": duplicate_interval_count,
            "duplicate_name_count": duplicate_name_count,
            "coordinate_sorted": coordinate_sorted,
            "errors": audit.errors,
            "warnings": audit.warnings,
            "finding_count": audit.finding_count,
            "blocking_loss_count": audit.blocking_loss_count,
        },
        "features": record_rows[:max_items],
        "omitted_features": max(0, len(record_rows) - max_items),
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
                "interval_structure": "pass" if intervals else "fail",
                "coordinate_order": "pass",
                "block_structure": "pass",
                "provenance": "pass" if provenance_digest is not None else "fail",
            },
            "limitations": [
                "chromosome labels, BED names, and track metadata are represented by source-bound digests or counts rather than disclosed",
                "coordinates are zero-based half-open source-local intervals and do not establish an assembly or reference build",
                "BED names and display fields are not resolved against external ontologies or annotation releases",
                "the audit validates interval structure and geometry, not biological correctness, feature identity, or causal meaning",
            ],
        },
        "max_features": MAX_BED_FEATURES,
        "max_items": max_items,
    }
    document["document_digest"] = content_digest(document)
    return BedParseResult(document)


class BedAdapter:
    """Concrete adapter facade for the dependency-free bounded BED route."""

    name = BED_ADAPTER
    version = BED_ADAPTER_VERSION
    accepted_formats = ("application/bed", "text/bed", "text/x-bed")
    declared_loss_kinds = frozenset(
        {
            "content_uninterpreted",
            "coordinate_frame_not_carried",
            "ontology_term_unmapped",
            "provenance_unavailable",
        }
    )

    def manifest(self) -> dict[str, Any]:
        return {
            "name": self.name,
            "version": self.version,
            "accepted_formats": list(self.accepted_formats),
            "conformance_level": "normalize",
            "declared_loss_kinds": sorted(self.declared_loss_kinds),
            "scope_dimensions": ["subject", "sample", "reference", "feature", "interval", "transcript"],
            "execution": "python_delegated",
            "optional_dependency": None,
        }

    def parse(
        self,
        bed: str | bytes,
        *,
        source_id: str,
        provenance: Mapping[str, Any] | None = None,
        max_bytes: int = MAX_BED_BYTES,
        max_features: int = MAX_BED_FEATURES,
        max_items: int = MAX_BED_ITEMS,
    ) -> BedParseResult:
        return parse_bed(
            bed,
            source_id=source_id,
            provenance=provenance,
            max_bytes=max_bytes,
            max_features=max_features,
            max_items=max_items,
        )


__all__ = [
    "BED_ADAPTER",
    "BED_ADAPTER_VERSION",
    "BED_FORMAT",
    "BED_SCHEMA",
    "BedAdapter",
    "BedFinding",
    "BedParseError",
    "BedParseResult",
    "MAX_BED_BLOCKS",
    "MAX_BED_BYTES",
    "MAX_BED_COORDINATE",
    "MAX_BED_FEATURES",
    "MAX_BED_ITEMS",
    "MAX_BED_LINE_BYTES",
    "parse_bed",
]
