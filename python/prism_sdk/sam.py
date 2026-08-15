"""Bounded SAM alignment auditing without read or quality disclosure.

The reader validates SAM headers, sequence dictionaries, alignment fields, CIGAR semantics,
optional-tag types, coordinate bounds, mate flags, and declared sort order. Read names,
reference labels, sequences, qualities, and tag values are represented by source-bound digests
or aggregate evidence rather than emitted as raw content.
"""

from __future__ import annotations

from collections import Counter
from dataclasses import dataclass
import math
import re
from typing import Any, Mapping

from .authoring import content_digest
from .errors import ArgumentError


SAM_SCHEMA = "bioprism-python-sam/0.1"
SAM_ADAPTER = "bioprism.python.sam_text"
SAM_ADAPTER_VERSION = "0.1.0"
SAM_FORMAT = "text/sam"
MAX_SAM_BYTES = 100_000_000
MAX_SAM_RECORDS = 1_000_000
MAX_SAM_HEADERS = 100_000
MAX_SAM_ITEMS = 1_000
MAX_SAM_TAGS = 100_000
MAX_SAM_LINE_BYTES = 1_000_000
_SEVERITY_ORDER = {"advisory": 0, "degrading": 1, "blocking": 2}
_HEADER_TYPES = {"HD", "SQ", "RG", "PG", "CO"}
_TAG_RE = re.compile(r"^[A-Za-z][A-Za-z0-9]$")
_INT_RE = re.compile(r"^-?(?:0|[1-9][0-9]*)$")
_CIGAR_RE = re.compile(r"([0-9]+)([MIDNSHP=X])")
_SEQ_ALPHABET = frozenset("ACMGRSVTWYHKDBNacmgrsvtwyhkdbn=.")
_B_ARRAY_TYPES = frozenset("cCsSiIf")
_KNOWN_FLAG_BITS = {
    0x1: "paired",
    0x2: "proper_pair",
    0x4: "unmapped",
    0x8: "mate_unmapped",
    0x10: "reverse",
    0x20: "mate_reverse",
    0x40: "read1",
    0x80: "read2",
    0x100: "secondary",
    0x200: "qc_fail",
    0x400: "duplicate",
    0x800: "supplementary",
}


class SamParseError(ArgumentError):
    """A structurally invalid SAM source with stable line and record locators."""

    def __init__(self, message: str, *, line: int | None = None, record: int | None = None) -> None:
        location = ""
        if line is not None:
            location += f" at line {line}"
        if record is not None:
            location += f" record {record}"
        super().__init__(f"SAM parse refused{location}: {message}")


@dataclass(frozen=True)
class SamFinding:
    """One bounded SAM structural, alignment, or metadata finding."""

    code: str
    severity: str
    location: Mapping[str, Any]
    detail: str

    def __post_init__(self) -> None:
        if self.severity not in {"warning", "error"}:
            raise ArgumentError(f"unsupported SAM finding severity: {self.severity!r}")

    def to_wire(self) -> dict[str, Any]:
        return {
            "code": self.code,
            "severity": self.severity,
            "location": dict(self.location),
            "detail": self.detail,
        }


@dataclass(frozen=True)
class SamParseResult:
    """A validated bounded SAM projection with privacy-safe alignment evidence."""

    document: Mapping[str, Any]

    @property
    def alignments(self) -> list[Mapping[str, Any]]:
        return list(self.document["alignments"])

    @property
    def valid(self) -> bool:
        return bool(self.document["valid"])

    @property
    def publishable(self) -> bool:
        return bool(self.document["publishable"])

    def to_wire(self) -> dict[str, Any]:
        return dict(self.document)


@dataclass(frozen=True)
class _Header:
    kind: str
    tags: Mapping[str, str]
    line: int


@dataclass(frozen=True)
class _Cigar:
    text: str
    operations: tuple[tuple[int, str], ...]
    query_bases: int
    reference_bases: int
    aligned_bases: int
    inserted_bases: int
    deleted_bases: int
    skipped_bases: int
    soft_clipped_bases: int
    hard_clipped_bases: int


@dataclass(frozen=True)
class _Alignment:
    qname: str
    flag: int
    rname: str
    pos: int
    mapq: int
    cigar: _Cigar
    rnext: str
    pnext: int
    tlen: int
    sequence: str
    quality: str
    tag_types: tuple[tuple[str, str], ...]
    line: int
    record: int


class _Audit:
    def __init__(self, limit: int) -> None:
        self.limit = limit
        self.findings: list[SamFinding] = []
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
            self.findings.append(SamFinding(code, severity, dict(location), detail))

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
    max_bytes = _validate_limit("max_bytes", max_bytes, MAX_SAM_BYTES)
    if isinstance(payload, bytes):
        if len(payload) > max_bytes:
            raise ArgumentError(f"SAM exceeds the {max_bytes}-byte limit")
        try:
            return payload.decode("utf-8")
        except UnicodeDecodeError as error:
            raise ArgumentError(f"SAM is not valid UTF-8: {error}") from error
    if isinstance(payload, str):
        if len(payload.encode("utf-8")) > max_bytes:
            raise ArgumentError(f"SAM exceeds the {max_bytes}-byte limit")
        return payload
    raise ArgumentError("SAM payload must be text or bytes")


def _integer(raw: str, *, field: str, line: int, record: int | None, minimum: int | None = None, maximum: int | None = None) -> int:
    if not _INT_RE.fullmatch(raw):
        raise SamParseError(f"{field} is not an integer", line=line, record=record)
    try:
        value = int(raw)
    except ValueError as error:
        raise SamParseError(f"{field} is not an integer", line=line, record=record) from error
    if minimum is not None and value < minimum:
        raise SamParseError(f"{field} is below {minimum}", line=line, record=record)
    if maximum is not None and value > maximum:
        raise SamParseError(f"{field} exceeds {maximum}", line=line, record=record)
    return value


def _tag_fields(fields: list[str], *, line: int, record: int | None, header: bool = False) -> dict[str, str]:
    tags: dict[str, str] = {}
    for raw in fields:
        parts = raw.split(":", 1)
        if len(parts) != 2 or not _TAG_RE.fullmatch(parts[0]) or not parts[1]:
            raise SamParseError("tag must be a two-character key followed by a non-empty value", line=line, record=record)
        key, value = parts
        if key in tags:
            raise SamParseError(f"duplicate {'header ' if header else ''}tag {key!r}", line=line, record=record)
        if any(ord(character) < 32 or ord(character) == 127 for character in value):
            raise SamParseError("tag value contains a control character", line=line, record=record)
        tags[key] = value
    return tags


def _parse_header(line_text: str, *, line: int) -> _Header:
    if not line_text.startswith("@") or len(line_text) < 3:
        raise SamParseError("header must start with @ followed by a two-character type", line=line)
    kind = line_text[1:3]
    if kind not in _HEADER_TYPES:
        raise SamParseError(f"unsupported header type {kind!r}", line=line)
    fields = line_text.split("\t")[1:]
    if kind == "CO":
        return _Header(kind, {}, line)
    if not fields:
        raise SamParseError(f"@{kind} header has no tags", line=line)
    return _Header(kind, _tag_fields(fields, line=line, record=None, header=True), line)


def _parse_cigar(raw: str, *, line: int, record: int) -> _Cigar:
    if raw == "*":
        return _Cigar(raw, (), 0, 0, 0, 0, 0, 0, 0, 0)
    if not raw or not re.fullmatch(r"(?:[0-9]+[MIDNSHP=X])+", raw):
        raise SamParseError("CIGAR is not a valid operation string", line=line, record=record)
    operations = tuple((int(match.group(1)), match.group(2)) for match in _CIGAR_RE.finditer(raw))
    if any(length <= 0 for length, _ in operations):
        raise SamParseError("CIGAR operations must have positive lengths", line=line, record=record)
    if operations[0][1] == "H" and any(operation == "H" for _, operation in operations[1:-1]):
        raise SamParseError("hard clipping is only permitted at a CIGAR edge", line=line, record=record)
    if operations[-1][1] == "H" and any(operation == "H" for _, operation in operations[:-1]):
        raise SamParseError("hard clipping is only permitted at a CIGAR edge", line=line, record=record)
    if any(operation == "H" for _, operation in operations[1:-1]):
        raise SamParseError("hard clipping is only permitted at a CIGAR edge", line=line, record=record)
    query_bases = sum(length for length, operation in operations if operation in "MIS=X")
    reference_bases = sum(length for length, operation in operations if operation in "MDN=X")
    aligned_bases = sum(length for length, operation in operations if operation in "M=X")
    inserted_bases = sum(length for length, operation in operations if operation == "I")
    deleted_bases = sum(length for length, operation in operations if operation == "D")
    skipped_bases = sum(length for length, operation in operations if operation == "N")
    soft_clipped_bases = sum(length for length, operation in operations if operation == "S")
    hard_clipped_bases = sum(length for length, operation in operations if operation == "H")
    return _Cigar(
        raw,
        operations,
        query_bases,
        reference_bases,
        aligned_bases,
        inserted_bases,
        deleted_bases,
        skipped_bases,
        soft_clipped_bases,
        hard_clipped_bases,
    )


def _validate_optional_tag(raw: str, *, line: int, record: int) -> tuple[str, str]:
    parts = raw.split(":", 2)
    if len(parts) != 3 or not _TAG_RE.fullmatch(parts[0]) or len(parts[1]) != 1:
        raise SamParseError("optional field must have TAG:TYPE:VALUE shape", line=line, record=record)
    tag, type_code, value = parts
    if type_code == "A":
        if len(value) != 1 or not (32 <= ord(value) <= 126):
            raise SamParseError("A optional tag must contain one printable character", line=line, record=record)
    elif type_code == "i":
        _integer(value, field=f"optional tag {tag}", line=line, record=record)
    elif type_code == "f":
        try:
            parsed = float(value)
        except ValueError as error:
            raise SamParseError(f"optional tag {tag} is not a floating-point value", line=line, record=record) from error
        if not math.isfinite(parsed):
            raise SamParseError(f"optional tag {tag} is not finite", line=line, record=record)
    elif type_code == "Z":
        if any(ord(character) < 32 or ord(character) == 127 for character in value):
            raise SamParseError(f"optional tag {tag} contains a control character", line=line, record=record)
    elif type_code == "H":
        if len(value) % 2 or any(character not in "0123456789abcdefABCDEF" for character in value):
            raise SamParseError(f"optional tag {tag} is not an even-length hexadecimal value", line=line, record=record)
    elif type_code == "B":
        array = value.split(",")
        if len(array) < 2 or array[0] not in _B_ARRAY_TYPES:
            raise SamParseError(f"optional tag {tag} has an invalid B-array type", line=line, record=record)
        for item in array[1:]:
            if array[0] in "cCsSiI" and not _INT_RE.fullmatch(item):
                raise SamParseError(f"optional tag {tag} has a non-integer B-array item", line=line, record=record)
            if array[0] == "f":
                try:
                    parsed = float(item)
                except ValueError as error:
                    raise SamParseError(f"optional tag {tag} has a non-numeric B-array item", line=line, record=record) from error
                if not math.isfinite(parsed):
                    raise SamParseError(f"optional tag {tag} has a non-finite B-array item", line=line, record=record)
    else:
        raise SamParseError(f"optional tag {tag} has unsupported type {type_code!r}", line=line, record=record)
    return tag, type_code


def _parse_alignment(line_text: str, *, line: int, record: int, max_tags: int) -> _Alignment:
    fields = line_text.split("\t")
    if len(fields) < 11:
        raise SamParseError(f"alignment has {len(fields)} fields; at least 11 are required", line=line, record=record)
    qname, raw_flag, rname, raw_pos, raw_mapq, raw_cigar, rnext, raw_pnext, raw_tlen, sequence, quality = fields[:11]
    if qname == "" or qname != qname.strip() or any(character.isspace() for character in qname):
        raise SamParseError("QNAME is empty or contains whitespace", line=line, record=record)
    if len(qname.encode("utf-8")) > 4_096:
        raise SamParseError("QNAME exceeds the 4096-byte limit", line=line, record=record)
    if rname != "*" and (not rname or any(character.isspace() for character in rname) or rname == "="):
        raise SamParseError("RNAME is not a valid reference label", line=line, record=record)
    if rnext not in {"*", "="} and (not rnext or any(character.isspace() for character in rnext)):
        raise SamParseError("RNEXT is not a valid reference label", line=line, record=record)
    flag = _integer(raw_flag, field="FLAG", line=line, record=record, minimum=0, maximum=65535)
    pos = _integer(raw_pos, field="POS", line=line, record=record, minimum=0, maximum=2_147_483_647)
    mapq = _integer(raw_mapq, field="MAPQ", line=line, record=record, minimum=0, maximum=255)
    pnext = _integer(raw_pnext, field="PNEXT", line=line, record=record, minimum=0, maximum=2_147_483_647)
    tlen = _integer(raw_tlen, field="TLEN", line=line, record=record, minimum=-2_147_483_648, maximum=2_147_483_647)
    cigar = _parse_cigar(raw_cigar, line=line, record=record)
    if sequence != "*" and (not sequence or any(character not in _SEQ_ALPHABET for character in sequence)):
        raise SamParseError("SEQ contains a symbol outside the bounded SAM alphabet", line=line, record=record)
    if quality != "*" and any(ord(character) < 33 or ord(character) > 126 for character in quality):
        raise SamParseError("QUAL contains a non-printable ASCII character", line=line, record=record)
    if sequence != "*" and raw_cigar != "*" and len(sequence) != cigar.query_bases:
        raise SamParseError("SEQ length does not equal query-consuming CIGAR length", line=line, record=record)
    if quality != "*" and sequence != "*" and len(quality) != len(sequence):
        raise SamParseError("QUAL length does not equal SEQ length", line=line, record=record)
    if quality != "*" and sequence == "*":
        raise SamParseError("QUAL cannot be present when SEQ is '*'")
    if len(fields) - 11 > max_tags:
        raise ArgumentError(f"SAM optional tags exceed the {max_tags}-tag limit")
    tag_types = tuple(_validate_optional_tag(raw, line=line, record=record) for raw in fields[11:])
    if len({tag for tag, _ in tag_types}) != len(tag_types):
        raise SamParseError("optional tag occurs more than once", line=line, record=record)
    return _Alignment(qname, flag, rname, pos, mapq, cigar, rnext, pnext, tlen, sequence, quality, tag_types, line, record)


def _normalise_lines(text: str) -> list[str]:
    if not text:
        raise SamParseError("source is empty")
    lines = text.split("\n")
    if lines and lines[-1] == "":
        lines.pop()
    if not lines:
        raise SamParseError("source contains no records")
    normalized: list[str] = []
    for line_number, line in enumerate(lines, start=1):
        if len(line.encode("utf-8")) > MAX_SAM_LINE_BYTES:
            raise ArgumentError(f"SAM line {line_number} exceeds the {MAX_SAM_LINE_BYTES}-byte limit")
        if line.endswith("\r"):
            line = line[:-1]
        if "\r" in line:
            raise SamParseError("lone carriage return is not a record separator", line=line_number)
        if not line:
            raise SamParseError("blank lines are not valid SAM records", line=line_number)
        normalized.append(line)
    return normalized


def _reference_dictionary(headers: list[_Header], *, source_id: str, audit: _Audit) -> tuple[dict[str, int], list[dict[str, Any]]]:
    dictionary: dict[str, int] = {}
    rows: list[dict[str, Any]] = []
    for header in headers:
        if header.kind != "SQ":
            continue
        name = header.tags.get("SN")
        length_raw = header.tags.get("LN")
        if name is None or length_raw is None:
            audit.finding("sequence_dictionary_incomplete", "error", {"source": source_id, "line": header.line}, "@SQ requires SN and LN tags")
            continue
        if name in dictionary:
            audit.finding("sequence_dictionary_duplicate", "error", {"source": source_id, "line": header.line}, "@SQ SN occurs more than once")
            continue
        try:
            length = _integer(length_raw, field="@SQ LN", line=header.line, record=None, minimum=1, maximum=2_147_483_647)
        except SamParseError:
            audit.finding("sequence_dictionary_length_invalid", "error", {"source": source_id, "line": header.line}, "@SQ LN is not a positive bounded integer")
            continue
        dictionary[name] = length
        rows.append({"name_digest": _digest(source_id, name), "length": length})
    return dictionary, rows


def parse_sam(
    payload: str | bytes,
    *,
    source_id: str,
    provenance: Mapping[str, Any] | None = None,
    max_bytes: int = MAX_SAM_BYTES,
    max_records: int = MAX_SAM_RECORDS,
    max_headers: int = MAX_SAM_HEADERS,
    max_items: int = MAX_SAM_ITEMS,
    max_tags: int = MAX_SAM_TAGS,
) -> SamParseResult:
    """Parse a bounded SAM stream and audit alignment semantics without raw read disclosure."""

    if not isinstance(source_id, str) or not source_id.strip():
        raise ArgumentError("source_id must be a non-empty string")
    if provenance is not None and not isinstance(provenance, Mapping):
        raise ArgumentError("provenance must be a mapping when supplied")
    max_records = _validate_limit("max_records", max_records, MAX_SAM_RECORDS)
    max_headers = _validate_limit("max_headers", max_headers, MAX_SAM_HEADERS)
    max_items = _validate_limit("max_items", max_items, MAX_SAM_ITEMS)
    max_tags = _validate_limit("max_tags", max_tags, MAX_SAM_TAGS)
    text = _decode(payload, max_bytes=max_bytes)
    lines = _normalise_lines(text)
    audit = _Audit(max_items)
    provenance_digest: str | None = None
    if provenance:
        try:
            provenance_digest = content_digest(dict(provenance))
        except (TypeError, ValueError) as error:
            raise ArgumentError(f"provenance is not canonical JSON-safe: {error}") from error
    audit.loss("content_uninterpreted", "degrading", source_id, "read names, sequences, qualities, and optional-tag values are not emitted; bounded alignment evidence and source-bound digests are carried")
    audit.loss("coordinate_frame_not_carried", "degrading", "reference", "reference bases, assembly identity, and full coordinate context are not resolved by this text-only audit")
    audit.loss("type_undetermined", "degrading", "alignment", "reference-build, read-group, and optional-tag ontology semantics are not externally resolved")
    if provenance_digest is None:
        audit.loss("provenance_unavailable", "blocking", "provenance", "no non-empty provenance projection was supplied")

    headers: list[_Header] = []
    alignments: list[_Alignment] = []
    header_mode = True
    total_tags = 0
    for line_number, line in enumerate(lines, start=1):
        if line.startswith("@"):
            if not header_mode:
                raise SamParseError("header record occurs after an alignment record", line=line_number)
            if len(headers) >= max_headers:
                raise ArgumentError(f"SAM contains more than the {max_headers}-header limit")
            headers.append(_parse_header(line, line=line_number))
            continue
        header_mode = False
        if len(alignments) >= max_records:
            raise ArgumentError(f"SAM contains more than the {max_records}-record limit")
        record = len(alignments) + 1
        alignment = _parse_alignment(line, line=line_number, record=record, max_tags=max_tags)
        total_tags += len(alignment.tag_types)
        if total_tags > max_tags:
            raise ArgumentError(f"SAM optional tags exceed the {max_tags}-tag limit")
        alignments.append(alignment)
    if not headers and not alignments:
        raise SamParseError("source contains no records")

    header_counts = Counter(header.kind for header in headers)
    hd_headers = [header for header in headers if header.kind == "HD"]
    if len(hd_headers) > 1:
        audit.finding("header_hd_duplicate", "error", {"source": source_id}, "SAM contains more than one @HD header")
    hd = hd_headers[0] if hd_headers else None
    sort_order = hd.tags.get("SO") if hd is not None else None
    if sort_order is not None and sort_order not in {"unknown", "unsorted", "queryname", "coordinate", "template"}:
        audit.finding("header_sort_order_invalid", "error", {"source": source_id, "line": hd.line}, "@HD SO value is not a recognized SAM sort order")
    dictionary, dictionary_rows = _reference_dictionary(headers, source_id=source_id, audit=audit)
    if not dictionary:
        audit.finding("sequence_dictionary_missing", "warning", {"source": source_id}, "no valid @SQ sequence dictionary is available for reference-bound checks")

    flag_counts: Counter[str] = Counter()
    mapq_counts: Counter[str] = Counter()
    tag_type_counts: Counter[str] = Counter()
    reference_counts: Counter[str] = Counter()
    qname_groups: dict[str, Counter[str]] = {}
    rows: list[dict[str, Any]] = []
    previous_coordinate: tuple[int, int] | None = None
    previous_rname: str | None = None
    total_query_bases = 0
    total_reference_bases = 0
    total_aligned_bases = 0
    total_inserted_bases = 0
    total_deleted_bases = 0
    total_skipped_bases = 0
    total_soft_clipped_bases = 0
    total_hard_clipped_bases = 0
    mapped = 0
    unmapped = 0
    mapq_min: int | None = None
    mapq_max: int | None = None
    quality_min: int | None = None
    quality_max: int | None = None
    for alignment in alignments:
        location = {"source": source_id, "record": alignment.record, "line": alignment.line}
        mapped_record = alignment.flag & 0x4 == 0
        if mapped_record:
            mapped += 1
            if alignment.rname == "*" or alignment.pos == 0:
                audit.finding("mapped_coordinate_missing", "error", location, "a mapped alignment must carry RNAME and positive POS")
            if alignment.cigar.text == "*":
                audit.finding("mapped_cigar_missing", "error", location, "a mapped alignment must carry a CIGAR")
            if alignment.rname in dictionary and alignment.cigar.reference_bases:
                end = alignment.pos + alignment.cigar.reference_bases - 1
                if end > dictionary[alignment.rname]:
                    audit.finding("coordinate_out_of_bounds", "error", location, "reference-consuming CIGAR operations exceed the @SQ LN bound")
            if alignment.rname not in {"*", "="}:
                reference_counts[alignment.rname] += 1
                if dictionary and alignment.rname not in dictionary:
                    audit.finding("reference_not_in_dictionary", "error", location, "mapped RNAME is absent from the @SQ sequence dictionary")
        else:
            unmapped += 1
            if alignment.rname != "*" or alignment.pos != 0:
                audit.finding("unmapped_coordinate_present", "warning", location, "an unmapped alignment carries a reference label or nonzero POS")
        if alignment.rnext == "=" and alignment.rname == "*":
            audit.finding("mate_reference_without_reference", "error", location, "RNEXT '=' requires a current RNAME")
        if alignment.flag & 0x8 and (alignment.rnext != "*" or alignment.pnext != 0):
            audit.finding("mate_unmapped_coordinate_present", "warning", location, "mate-unmapped flag is set but mate coordinates are present")
        if alignment.flag & 0x1 and not alignment.flag & 0x40 and not alignment.flag & 0x80:
            audit.finding("paired_read_side_missing", "warning", location, "paired alignment has neither READ1 nor READ2 flag")
        if not alignment.flag & 0x1 and alignment.flag & (0x2 | 0x40 | 0x80):
            audit.finding("unpaired_flag_inconsistent", "error", location, "proper-pair or mate-side flags require the paired flag")
        if alignment.sequence == "*" and alignment.cigar.query_bases:
            audit.finding("sequence_missing", "warning", location, "query-consuming CIGAR operations exist but SEQ is '*'")
        if alignment.quality != "*":
            values = [ord(character) - 33 for character in alignment.quality]
            quality_min = min(values) if quality_min is None else min(quality_min, min(values))
            quality_max = max(values) if quality_max is None else max(quality_max, max(values))
        if alignment.mapq != 255:
            mapq_min = alignment.mapq if mapq_min is None else min(mapq_min, alignment.mapq)
            mapq_max = alignment.mapq if mapq_max is None else max(mapq_max, alignment.mapq)
        mapq_counts[str(alignment.mapq)] += 1
        for bit, name in _KNOWN_FLAG_BITS.items():
            if alignment.flag & bit:
                flag_counts[name] += 1
        for _, type_code in alignment.tag_types:
            tag_type_counts[type_code] += 1
        qname_groups.setdefault(alignment.qname, Counter())["read1" if alignment.flag & 0x40 else "read2" if alignment.flag & 0x80 else "unpaired"] += 1
        if mapped_record and alignment.rname not in {"*", "="}:
            current_key = (list(dictionary).index(alignment.rname) if alignment.rname in dictionary else 0, alignment.pos)
            if sort_order == "coordinate" and previous_coordinate is not None and current_key < previous_coordinate:
                audit.finding("coordinate_sort_violation", "error", location, "alignment order violates @HD SO:coordinate")
            previous_coordinate = current_key
            previous_rname = alignment.rname
        total_query_bases += alignment.cigar.query_bases
        total_reference_bases += alignment.cigar.reference_bases
        total_aligned_bases += alignment.cigar.aligned_bases
        total_inserted_bases += alignment.cigar.inserted_bases
        total_deleted_bases += alignment.cigar.deleted_bases
        total_skipped_bases += alignment.cigar.skipped_bases
        total_soft_clipped_bases += alignment.cigar.soft_clipped_bases
        total_hard_clipped_bases += alignment.cigar.hard_clipped_bases
        rows.append(
            {
                "record": alignment.record,
                "qname_digest": _digest(source_id, alignment.qname),
                "rname_digest": None if alignment.rname == "*" else _digest(source_id, alignment.rname),
                "flag": alignment.flag,
                "pos": alignment.pos,
                "mapq": alignment.mapq,
                "cigar_digest": _digest(source_id, alignment.cigar.text),
                "query_bases": alignment.cigar.query_bases,
                "reference_bases": alignment.cigar.reference_bases,
                "sequence_length": None if alignment.sequence == "*" else len(alignment.sequence),
                "quality_length": None if alignment.quality == "*" else len(alignment.quality),
                "optional_tag_count": len(alignment.tag_types),
                "optional_tag_types": dict(sorted(Counter(type_code for _, type_code in alignment.tag_types).items())),
            }
        )

    incomplete_pairs = 0
    complete_pairs = 0
    duplicate_sides = 0
    paired_groups = 0
    for qname, sides in qname_groups.items():
        if "read1" not in sides and "read2" not in sides:
            continue
        paired_groups += 1
        if sides["read1"] and sides["read2"]:
            complete_pairs += 1
        else:
            incomplete_pairs += 1
            audit.finding("paired_group_incomplete", "warning", {"source": source_id, "qname_digest": _digest(source_id, qname)}, "a paired-read name has only one mate side in the bounded stream")
        if sides["read1"] > 1 or sides["read2"] > 1:
            duplicate_sides += 1
            audit.finding("paired_side_duplicate", "error", {"source": source_id, "qname_digest": _digest(source_id, qname)}, "a paired-read name occurs more than once for one mate side")

    valid = audit.errors == 0
    publishable = valid and audit.max_loss_severity != "blocking"
    source_digest = content_digest({"source_id": source_id, "payload": text})
    manifest = {
        "source_id": source_id,
        "source_digest": source_digest,
        "adapter": SAM_ADAPTER,
        "adapter_version": SAM_ADAPTER_VERSION,
        "declared_format": SAM_FORMAT,
        "header_count": len(headers),
        "record_count": len(alignments),
        "provenance_digest": provenance_digest,
        "bytes_read": True,
        "read_names_disclosed": False,
        "reference_names_disclosed": False,
        "sequence_bases_disclosed": False,
        "quality_values_disclosed": False,
        "optional_tag_values_disclosed": False,
    }
    document: dict[str, Any] = {
        "schema": SAM_SCHEMA,
        "workflow": "sam_alignment_audit",
        "valid": valid,
        "publishable": publishable,
        "source_id": source_id,
        "manifest": manifest,
        "header": {
            "type_counts": dict(sorted(header_counts.items())),
            "version": None if hd is None else hd.tags.get("VN"),
            "sort_order": sort_order,
            "sequence_dictionary": dictionary_rows[:max_items],
            "omitted_sequence_dictionary": max(0, len(dictionary_rows) - max_items),
        },
        "summary": {
            "alignments": len(alignments),
            "mapped": mapped,
            "unmapped": unmapped,
            "query_bases": total_query_bases,
            "reference_bases": total_reference_bases,
            "aligned_bases": total_aligned_bases,
            "inserted_bases": total_inserted_bases,
            "deleted_bases": total_deleted_bases,
            "skipped_reference_bases": total_skipped_bases,
            "soft_clipped_bases": total_soft_clipped_bases,
            "hard_clipped_bases": total_hard_clipped_bases,
            "spliced_alignments": sum(1 for alignment in alignments if alignment.cigar.skipped_bases),
            "mapq_min": mapq_min,
            "mapq_max": mapq_max,
            "mapq_unknown_255": sum(1 for alignment in alignments if alignment.mapq == 255),
            "quality_phred_min": quality_min,
            "quality_phred_max": quality_max,
            "flag_counts": dict(sorted(flag_counts.items())),
            "mapq_counts": dict(sorted(mapq_counts.items(), key=lambda item: int(item[0]))),
            "optional_tag_type_counts": dict(sorted(tag_type_counts.items())),
            "paired_groups": paired_groups,
            "complete_pairs": complete_pairs,
            "incomplete_pairs": incomplete_pairs,
            "duplicate_pair_sides": duplicate_sides,
            "errors": audit.errors,
            "warnings": audit.warnings,
            "finding_count": audit.finding_count,
            "blocking_loss_count": audit.blocking_loss_count,
        },
        "reference_counts": [
            {"reference_digest": _digest(source_id, name), "alignments": count}
            for name, count in sorted(reference_counts.items())
        ][:max_items],
        "omitted_reference_counts": max(0, len(reference_counts) - max_items),
        "alignments": rows[:max_items],
        "omitted_alignments": max(0, len(rows) - max_items),
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
                "header_structure": "pass",
                "alignment_fields": "pass",
                "cigar_semantics": "pass",
                "coordinate_bounds": "pass" if not any(finding.code == "coordinate_out_of_bounds" for finding in audit.findings) else "fail",
                "pairing": "pass" if duplicate_sides == 0 else "fail",
                "sort_order": "pass" if not any(finding.code == "coordinate_sort_violation" for finding in audit.findings) else "fail",
                "provenance": "pass" if provenance_digest is not None else "fail",
            },
            "limitations": [
                "read names, reference labels, sequences, qualities, and optional-tag values are represented by digests or aggregate summaries rather than disclosed",
                "the route validates SAM text and declared coordinates but does not fetch reference bases, resolve assembly identity, or perform alignment scoring",
                "binary BAM/CRAM decoding, indexing, CRAM reference retrieval, and alignment reprocessing remain separate dependency-gated capabilities",
            ],
        },
        "max_records": MAX_SAM_RECORDS,
        "max_headers": MAX_SAM_HEADERS,
        "max_items": max_items,
        "max_tags": max_tags,
    }
    document["document_digest"] = content_digest(document)
    return SamParseResult(document)


class SamAdapter:
    """Concrete adapter facade matching the dependency-free bounded SAM route."""

    name = SAM_ADAPTER
    version = SAM_ADAPTER_VERSION
    accepted_formats = ("application/sam", "text/sam", "text/x-sam")
    declared_loss_kinds = frozenset(
        {
            "content_uninterpreted",
            "coordinate_frame_not_carried",
            "type_undetermined",
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
            "scope_dimensions": ["subject", "sample", "reference", "read", "alignment", "assay"],
            "execution": "python_delegated",
            "optional_dependency": None,
        }

    def parse(
        self,
        sam: str | bytes,
        *,
        source_id: str,
        provenance: Mapping[str, Any] | None = None,
        max_bytes: int = MAX_SAM_BYTES,
        max_records: int = MAX_SAM_RECORDS,
        max_headers: int = MAX_SAM_HEADERS,
        max_items: int = MAX_SAM_ITEMS,
        max_tags: int = MAX_SAM_TAGS,
    ) -> SamParseResult:
        return parse_sam(
            sam,
            source_id=source_id,
            provenance=provenance,
            max_bytes=max_bytes,
            max_records=max_records,
            max_headers=max_headers,
            max_items=max_items,
            max_tags=max_tags,
        )


__all__ = [
    "MAX_SAM_BYTES",
    "MAX_SAM_HEADERS",
    "MAX_SAM_ITEMS",
    "MAX_SAM_LINE_BYTES",
    "MAX_SAM_RECORDS",
    "MAX_SAM_TAGS",
    "SAM_ADAPTER",
    "SAM_ADAPTER_VERSION",
    "SAM_FORMAT",
    "SAM_SCHEMA",
    "SamAdapter",
    "SamFinding",
    "SamParseError",
    "SamParseResult",
    "parse_sam",
]
