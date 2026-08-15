"""Bounded, provenance-preserving VCF parsing for the Python adapter layer.

This reader covers the text VCF boundary without claiming to replace ``pysam`` for indexed,
compressed, or random-access workloads. It is intentionally strict about structure and explicit
about meaning: a reference build must come from the caller or the VCF header, INFO/FORMAT values
are decoded only when their header declarations justify it, and raw spellings remain beside typed
values so a downstream world never has to guess what was written.
"""

from __future__ import annotations

from dataclasses import dataclass
from decimal import Decimal, InvalidOperation
import hashlib
import math
import re
from typing import Any, Mapping, Sequence

from .authoring import content_digest
from .errors import ArgumentError


VCF_SCHEMA = "bioprism-python-vcf/0.1"
VCF_ADAPTER = "bioprism.python.vcf_text"
VCF_ADAPTER_VERSION = "0.1.0"
MAX_VCF_BYTES = 10_000_000
MAX_VCF_RECORDS = 100_000
MAX_VCF_ITEMS = 1_000
MAX_VCF_HEADER_LINES = 100_000
_IDENTIFIER = re.compile(r"^[A-Za-z][A-Za-z0-9_.:-]*$")
_META_KEY = re.compile(r"^##([^=\s]+)=(.*)$")
_FIXED_COLUMNS = ("#CHROM", "POS", "ID", "REF", "ALT", "QUAL", "FILTER", "INFO")
_SEVERITY_ORDER = {"advisory": 0, "degrading": 1, "blocking": 2}


class VcfParseError(ArgumentError):
    """A structurally invalid VCF source with a stable line and field locator."""

    def __init__(self, message: str, *, line: int | None = None, field: str | None = None) -> None:
        self.line = line
        self.field = field
        location = ""
        if line is not None:
            location += f" at line {line}"
        if field is not None:
            location += f" field {field!r}"
        super().__init__(f"VCF parse refused{location}: {message}")


def _location(source_id: str, *, record: int | None = None, field: str | None = None) -> dict[str, Any]:
    location: dict[str, Any] = {"source": source_id}
    if record is not None:
        location["record"] = record
    if field is not None:
        location["field"] = field
    return location


@dataclass(frozen=True)
class VcfLoss:
    """One semantic limitation with an exact source location."""

    kind: str
    severity: str
    location: Mapping[str, Any]
    detail: str

    def __post_init__(self) -> None:
        if self.kind not in {
            "coordinate_frame_not_carried",
            "precision_reduced",
            "provenance_unavailable",
            "type_undetermined",
            "content_uninterpreted",
        }:
            raise ArgumentError(f"unsupported VCF semantic-loss kind: {self.kind!r}")
        if self.severity not in _SEVERITY_ORDER:
            raise ArgumentError(f"unsupported VCF semantic-loss severity: {self.severity!r}")

    def to_wire(self) -> dict[str, Any]:
        return {
            "kind": self.kind,
            "severity": self.severity,
            "location": dict(self.location),
            "detail": self.detail,
        }


class _LossLedger:
    def __init__(self, source_id: str, limit: int) -> None:
        self.source_id = source_id
        self.limit = limit
        self.mapped: list[dict[str, Any]] = []
        self.losses: list[VcfLoss] = []
        self.mapped_count = 0
        self.loss_count = 0

    def mapped_location(self, location: Mapping[str, Any]) -> None:
        self.mapped_count += 1
        if len(self.mapped) < self.limit:
            self.mapped.append(dict(location))

    def loss(
        self,
        kind: str,
        severity: str,
        location: Mapping[str, Any],
        detail: str,
    ) -> None:
        self.loss_count += 1
        if len(self.losses) < self.limit:
            self.losses.append(VcfLoss(kind, severity, dict(location), detail))

    def to_wire(self) -> dict[str, Any]:
        max_severity = None
        if self.losses:
            max_severity = max(self.losses, key=lambda loss: _SEVERITY_ORDER[loss.severity]).severity
        return {
            "audit": "lossy" if self.loss_count else "lossless",
            "mapped": self.mapped,
            "mapped_count": self.mapped_count,
            "mapped_omitted": self.mapped_count - len(self.mapped),
            "lost": [loss.to_wire() for loss in self.losses],
            "lost_count": self.loss_count,
            "lost_omitted": self.loss_count - len(self.losses),
            "max_severity": max_severity,
        }


@dataclass(frozen=True)
class VcfParseResult:
    """A validated, bounded VCF projection with source and loss evidence."""

    document: Mapping[str, Any]

    @property
    def variants(self) -> Sequence[Mapping[str, Any]]:
        return self.document["variants"]

    @property
    def semantic_loss(self) -> Mapping[str, Any]:
        return self.document["semantic_loss"]

    def to_wire(self) -> dict[str, Any]:
        return dict(self.document)


class VcfAdapter:
    """Concrete adapter facade matching the registry's dependency-free text VCF route."""

    name = VCF_ADAPTER
    version = VCF_ADAPTER_VERSION
    accepted_formats = ("application/vcf", "text/vcf", "text/x-vcf")
    declared_loss_kinds = frozenset(
        {
            "coordinate_frame_not_carried",
            "precision_reduced",
            "provenance_unavailable",
            "type_undetermined",
            "content_uninterpreted",
        }
    )

    def manifest(self) -> dict[str, Any]:
        return {
            "name": self.name,
            "version": self.version,
            "accepted_formats": list(self.accepted_formats),
            "conformance_level": "normalize",
            "declared_loss_kinds": sorted(self.declared_loss_kinds),
            "scope_dimensions": ["subject", "sample", "variant", "genome"],
            "execution": "python_delegated",
            "optional_dependency": None,
        }

    def parse(
        self,
        vcf: str | bytes,
        *,
        source_id: str,
        reference_build: str | None = None,
        provenance: Mapping[str, str] | None = None,
        max_bytes: int = MAX_VCF_BYTES,
        max_records: int = MAX_VCF_RECORDS,
        max_items: int = MAX_VCF_ITEMS,
    ) -> VcfParseResult:
        return parse_vcf(
            vcf,
            source_id=source_id,
            reference_build=reference_build,
            provenance=provenance,
            max_bytes=max_bytes,
            max_records=max_records,
            max_items=max_items,
        )


def _validate_text(name: str, value: str, maximum: int) -> None:
    if not isinstance(value, str) or not value.strip():
        raise ArgumentError(f"{name} must be a non-empty string")
    if any(ord(character) < 0x20 and character not in "\t" for character in value):
        raise ArgumentError(f"{name} must not contain control characters")
    if len(value.encode("utf-8")) > maximum:
        raise ArgumentError(f"{name} exceeds the {maximum}-byte limit")


def _limit(name: str, value: int, maximum: int) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or not 1 <= value <= maximum:
        raise ArgumentError(f"{name} must be between 1 and {maximum}")
    return value


def _decode_source(source: str | bytes, max_bytes: int) -> tuple[str, bytes]:
    if isinstance(source, str):
        raw = source.encode("utf-8")
        text = source
    elif isinstance(source, bytes):
        raw = source
        try:
            text = source.decode("utf-8")
        except UnicodeDecodeError as error:
            raise VcfParseError("source is not valid UTF-8") from error
    else:
        raise ArgumentError("vcf must be a string or UTF-8 bytes")
    if not raw or len(raw) > max_bytes:
        raise ArgumentError(f"vcf must contain between 1 and {max_bytes} bytes")
    if text.startswith("\ufeff"):
        text = text[1:]
    return text, raw


def _split_attributes(value: str) -> list[str]:
    parts: list[str] = []
    start = 0
    quoted = False
    escaped = False
    for index, character in enumerate(value):
        if escaped:
            escaped = False
        elif character == "\\" and quoted:
            escaped = True
        elif character == '"':
            quoted = not quoted
        elif character == "," and not quoted:
            parts.append(value[start:index])
            start = index + 1
    if quoted:
        raise VcfParseError("unterminated quoted metadata attribute")
    parts.append(value[start:])
    return parts


def _unquote(value: str) -> str:
    value = value.strip()
    if len(value) >= 2 and value[0] == '"' and value[-1] == '"':
        return value[1:-1].replace('\\"', '"').replace("\\\\", "\\")
    return value


def _definition(value: str, *, line: int, kind: str) -> dict[str, Any]:
    if not (value.startswith("<") and value.endswith(">")):
        raise VcfParseError(f"{kind} definition must be angle-bracketed", line=line)
    attributes: dict[str, str] = {}
    for part in _split_attributes(value[1:-1]):
        if "=" not in part:
            raise VcfParseError(f"{kind} definition contains an attribute without '='", line=line)
        key, raw_value = part.split("=", 1)
        key = key.strip()
        if not key or key in attributes:
            raise VcfParseError(f"{kind} definition contains a duplicate or empty key", line=line)
        attributes[key] = _unquote(raw_value)
    identifier = attributes.get("ID")
    if identifier is None or not _IDENTIFIER.fullmatch(identifier):
        raise VcfParseError(f"{kind} definition requires a valid ID", line=line)
    result: dict[str, Any] = {
        "id": identifier,
        "number": attributes.get("Number"),
        "type": attributes.get("Type"),
        "description": attributes.get("Description"),
        "attributes": attributes,
        "raw": value,
    }
    return result


def _header_value(metadata: Mapping[str, list[str]], key: str) -> str | None:
    values = metadata.get(key)
    if not values:
        return None
    return values[-1]


def _typed_scalar(
    raw: str,
    definition: Mapping[str, Any] | None,
    *,
    field: str,
    record: int | None,
    ledger: _LossLedger,
    source_id: str,
) -> Any:
    location = _location(source_id, record=record, field=field)
    if raw == ".":
        return None
    value_type = definition.get("type") if definition else None
    if value_type in (None, ""):
        ledger.loss(
            "type_undetermined",
            "advisory",
            location,
            f"{field} has no declared VCF type; the raw value is preserved",
        )
        return raw
    if value_type == "Flag":
        if raw not in {"1", "true", "True"}:
            ledger.loss(
                "type_undetermined",
                "advisory",
                location,
                f"Flag field {field} used non-canonical value {raw!r}; raw value is preserved",
            )
        return True
    if value_type == "Integer":
        try:
            return int(raw)
        except ValueError:
            ledger.loss(
                "type_undetermined",
                "degrading",
                location,
                f"declared Integer field {field} contains {raw!r}; raw value is preserved",
            )
            return raw
    if value_type == "Float":
        try:
            parsed = float(raw)
        except ValueError:
            ledger.loss(
                "type_undetermined",
                "degrading",
                location,
                f"declared Float field {field} contains {raw!r}; raw value is preserved",
            )
            return raw
        if not math.isfinite(parsed):
            raise VcfParseError(f"Float field {field} is non-finite", field=field)
        try:
            if Decimal(raw) != Decimal(str(parsed)):
                ledger.loss(
                    "precision_reduced",
                    "degrading",
                    location,
                    f"Float field {field} was carried as a binary float; raw value {raw!r} is retained",
                )
        except InvalidOperation:
            ledger.loss(
                "type_undetermined",
                "degrading",
                location,
                f"Float field {field} uses a non-decimal spelling {raw!r}; raw value is preserved",
            )
        return parsed
    if value_type in {"String", "Character"}:
        return raw
    ledger.loss(
        "type_undetermined",
        "advisory",
        location,
        f"unsupported VCF type {value_type!r} for {field}; raw value is preserved",
    )
    return raw


def _typed_value(
    raw: str,
    definition: Mapping[str, Any] | None,
    *,
    field: str,
    record: int,
    ledger: _LossLedger,
    source_id: str,
) -> Any:
    if raw == ".":
        return None
    number = definition.get("number") if definition else None
    values = raw.split(",")
    if number in {"0", 0} and len(values) != 1:
        ledger.loss(
            "type_undetermined",
            "degrading",
            _location(source_id, record=record, field=field),
            f"field {field} declares Number=0 but contains {len(values)} values",
        )
    if number not in {None, ".", "A", "R", "G", 0, "0"} and str(number).isdigit():
        expected = int(number)
        if len(values) != expected:
            ledger.loss(
                "type_undetermined",
                "degrading",
                _location(source_id, record=record, field=field),
                f"field {field} declares Number={expected} but contains {len(values)} values",
            )
    typed = [
        _typed_scalar(
            value,
            definition,
            field=field,
            record=record,
            ledger=ledger,
            source_id=source_id,
        )
        for value in values
    ]
    return typed[0] if len(typed) == 1 else typed


def _genotype(raw: str, *, alt_count: int, record: int, source_id: str) -> dict[str, Any]:
    if raw == ".":
        return {"raw": raw, "alleles": [None], "phased": None}
    separators = set(character for character in raw if character in "/|")
    if len(separators) > 1:
        raise VcfParseError("genotype mixes phased and unphased separators", line=record, field="GT")
    phased = "|" in separators
    delimiter = "|" if phased else "/"
    tokens = raw.split(delimiter)
    alleles: list[int | None] = []
    for token in tokens:
        if token == ".":
            alleles.append(None)
            continue
        try:
            allele = int(token)
        except ValueError as error:
            raise VcfParseError(f"invalid genotype allele {token!r}", line=record, field="GT") from error
        if allele < 0 or allele > alt_count:
            raise VcfParseError(
                f"genotype allele {allele} exceeds the REF/ALT allele range",
                line=record,
                field="GT",
            )
        alleles.append(allele)
    return {"raw": raw, "alleles": alleles, "phased": phased}


def _parse_info(
    raw: str,
    definitions: Mapping[str, Mapping[str, Any]],
    *,
    record: int,
    source_id: str,
    ledger: _LossLedger,
) -> tuple[dict[str, Any], dict[str, str]]:
    if raw == ".":
        return {}, {}
    values: dict[str, Any] = {}
    raw_values: dict[str, str] = {}
    for item in raw.split(";"):
        if not item:
            raise VcfParseError("INFO contains an empty item", line=record, field="INFO")
        if "=" in item:
            key, raw_value = item.split("=", 1)
        else:
            key, raw_value = item, "1"
        if not _IDENTIFIER.fullmatch(key):
            raise VcfParseError(f"INFO key {key!r} is not a valid identifier", line=record, field="INFO")
        if key in values:
            raise VcfParseError(f"INFO key {key!r} occurs more than once", line=record, field="INFO")
        definition = definitions.get(key)
        values[key] = _typed_value(
            raw_value,
            definition,
            field=f"INFO.{key}",
            record=record,
            ledger=ledger,
            source_id=source_id,
        )
        raw_values[key] = raw_value
    return values, raw_values


def _parse_samples(
    format_raw: str,
    sample_raw: Sequence[str],
    samples: Sequence[str],
    definitions: Mapping[str, Mapping[str, Any]],
    *,
    record: int,
    source_id: str,
    alt_count: int,
    ledger: _LossLedger,
) -> tuple[list[str], dict[str, dict[str, Any]], dict[str, dict[str, str]]]:
    if format_raw == ".":
        if sample_raw:
            raise VcfParseError("sample columns exist but FORMAT is '.'", line=record, field="FORMAT")
        return [], {}, {}
    format_keys = format_raw.split(":")
    if not format_keys or any(not _IDENTIFIER.fullmatch(key) for key in format_keys):
        raise VcfParseError("FORMAT contains an invalid key", line=record, field="FORMAT")
    if len(set(format_keys)) != len(format_keys):
        raise VcfParseError("FORMAT contains duplicate keys", line=record, field="FORMAT")
    decoded: dict[str, dict[str, Any]] = {}
    raw_decoded: dict[str, dict[str, str]] = {}
    for sample_name, raw_sample in zip(samples, sample_raw):
        fields = raw_sample.split(":")
        if len(fields) != len(format_keys):
            raise VcfParseError(
                f"sample {sample_name!r} has {len(fields)} FORMAT values for {len(format_keys)} keys",
                line=record,
                field=sample_name,
            )
        values: dict[str, Any] = {}
        raw_values: dict[str, str] = {}
        for key, raw_value in zip(format_keys, fields):
            raw_values[key] = raw_value
            if key == "GT":
                values[key] = _genotype(
                    raw_value,
                    alt_count=alt_count,
                    record=record,
                    source_id=source_id,
                )
                continue
            definition = definitions.get(key)
            values[key] = _typed_value(
                raw_value,
                definition,
                field=f"FORMAT.{key}",
                record=record,
                ledger=ledger,
                source_id=source_id,
            )
        decoded[sample_name] = values
        raw_decoded[sample_name] = raw_values
    return format_keys, decoded, raw_decoded


def parse_vcf(
    vcf: str | bytes,
    *,
    source_id: str,
    reference_build: str | None = None,
    provenance: Mapping[str, str] | None = None,
    max_bytes: int = MAX_VCF_BYTES,
    max_records: int = MAX_VCF_RECORDS,
    max_items: int = MAX_VCF_ITEMS,
) -> VcfParseResult:
    """Parse a bounded text VCF while preserving raw values and semantic-loss evidence.

    ``reference_build`` is caller-supplied context, not a guess. If it is absent, the VCF must
    carry ``##reference=...`` or a consistent ``assembly=...`` on its contig definitions. The
    result validates every source record even when only the first ``max_items`` variants are
    returned, so disclosure bounds never become validation bounds.
    """

    _validate_text("source_id", source_id, 512)
    if reference_build is not None:
        _validate_text("reference_build", reference_build, 256)
    if provenance is not None:
        if not isinstance(provenance, Mapping):
            raise ArgumentError("provenance must be a mapping")
        for key, value in provenance.items():
            if key not in {"accession", "version", "retrieved_at"}:
                raise ArgumentError(f"unsupported provenance field: {key!r}")
            _validate_text(f"provenance.{key}", value, 512)
    max_bytes = _limit("max_bytes", max_bytes, MAX_VCF_BYTES)
    max_records = _limit("max_records", max_records, MAX_VCF_RECORDS)
    max_items = _limit("max_items", max_items, MAX_VCF_ITEMS)
    text, raw_bytes = _decode_source(vcf, max_bytes)
    lines = text.splitlines()
    if len(lines) > MAX_VCF_HEADER_LINES + max_records:
        raise ArgumentError("VCF exceeds the bounded header and record line budget")
    if not lines:
        raise VcfParseError("source is empty")
    if not lines[0].startswith("##fileformat=VCF"):
        raise VcfParseError("first line must declare ##fileformat=VCF...")

    metadata: dict[str, list[str]] = {}
    info_definitions: dict[str, dict[str, Any]] = {}
    format_definitions: dict[str, dict[str, Any]] = {}
    contig_definitions: dict[str, dict[str, Any]] = {}
    header_lines: list[str] = []
    column_line: str | None = None
    data_start = None
    for line_number, line in enumerate(lines, start=1):
        if line.startswith("##"):
            header_lines.append(line)
            match = _META_KEY.match(line)
            if not match:
                raise VcfParseError("metadata line must have the form ##key=value", line=line_number)
            key, value = match.groups()
            metadata.setdefault(key, []).append(value)
            if key in {"INFO", "FORMAT", "contig"}:
                definition = _definition(value, line=line_number, kind=key)
                target = {"INFO": info_definitions, "FORMAT": format_definitions, "contig": contig_definitions}[key]
                if definition["id"] in target:
                    raise VcfParseError(f"duplicate {key} definition {definition['id']!r}", line=line_number)
                target[definition["id"]] = definition
            continue
        if line.startswith("#CHROM"):
            if column_line is not None:
                raise VcfParseError("VCF contains duplicate #CHROM headers", line=line_number)
            column_line = line
            data_start = line_number
            break
        raise VcfParseError("data or #CHROM header appeared before metadata", line=line_number)
    if column_line is None or data_start is None:
        raise VcfParseError("VCF is missing its #CHROM header")
    if len(header_lines) > MAX_VCF_HEADER_LINES:
        raise ArgumentError(f"VCF header exceeds {MAX_VCF_HEADER_LINES} lines")

    columns = column_line.split("\t")
    if columns[:8] != list(_FIXED_COLUMNS):
        raise VcfParseError("#CHROM header must use the eight standard fixed columns", line=data_start)
    if len(columns) == 8:
        sample_names: list[str] = []
    else:
        if columns[8] != "FORMAT":
            raise VcfParseError("column 9 must be FORMAT when samples are present", line=data_start)
        sample_names = columns[9:]
        if not sample_names or any(not _IDENTIFIER.fullmatch(sample) for sample in sample_names):
            raise VcfParseError("sample names must be non-empty VCF identifiers", line=data_start)
        if len(set(sample_names)) != len(sample_names):
            raise VcfParseError("sample names must be unique", line=data_start)

    header_reference = _header_value(metadata, "reference")
    if header_reference is None:
        assemblies = {
            definition["attributes"].get("assembly")
            for definition in contig_definitions.values()
            if definition["attributes"].get("assembly")
        }
        if len(assemblies) == 1:
            header_reference = next(iter(assemblies))
        elif len(assemblies) > 1:
            header_reference = None
    ledger = _LossLedger(source_id, max_items)
    effective_reference = reference_build or header_reference
    if effective_reference is None:
        ledger.loss(
            "coordinate_frame_not_carried",
            "blocking",
            _location(source_id, field="#CHROM"),
            "no caller-supplied reference_build or unambiguous VCF reference/assembly declaration was provided",
        )
    elif reference_build is not None and header_reference is not None and reference_build != header_reference:
        ledger.loss(
            "coordinate_frame_not_carried",
            "blocking",
            _location(source_id, field="#CHROM"),
            f"caller reference_build {reference_build!r} disagrees with VCF header reference {header_reference!r}",
        )
    if not provenance or not any(provenance.values()):
        ledger.loss(
            "provenance_unavailable",
            "degrading",
            _location(source_id),
            "the caller supplied no accession, version, or retrieval time; source bytes are hashed but upstream identity is unknown",
        )

    variants: list[dict[str, Any]] = []
    total_records = 0
    for line_number, line in enumerate(lines[data_start:], start=data_start + 1):
        if not line:
            raise VcfParseError("blank data line is not a VCF record", line=line_number)
        if line.startswith("#"):
            raise VcfParseError("metadata appeared after the #CHROM header", line=line_number)
        total_records += 1
        if total_records > max_records:
            raise ArgumentError(f"VCF contains more than the max_records limit of {max_records}")
        fields = line.split("\t")
        expected_fields = 9 + len(sample_names) if sample_names else 8
        if len(fields) != expected_fields:
            raise VcfParseError(
                f"record has {len(fields)} columns; expected {expected_fields}",
                line=line_number,
            )
        chrom, pos_raw, id_raw, ref, alt_raw, qual_raw, filter_raw, info_raw = fields[:8]
        if not chrom or chrom == ".":
            raise VcfParseError("CHROM must be a named contig", line=line_number, field="#CHROM")
        try:
            position = int(pos_raw)
        except ValueError as error:
            raise VcfParseError("POS must be an integer", line=line_number, field="POS") from error
        if position < 1:
            raise VcfParseError("POS must be one-based and positive", line=line_number, field="POS")
        if not ref or ref == ".":
            raise VcfParseError("REF must be a non-empty allele", line=line_number, field="REF")
        if any(not allele for allele in alt_raw.split(",")):
            raise VcfParseError("ALT contains an empty allele", line=line_number, field="ALT")
        alternate = [] if alt_raw == "." else alt_raw.split(",")
        if qual_raw == ".":
            quality = None
        else:
            try:
                quality = float(qual_raw)
            except ValueError as error:
                raise VcfParseError("QUAL must be a finite number or '.'", line=line_number, field="QUAL") from error
            if not math.isfinite(quality):
                raise VcfParseError("QUAL must be finite", line=line_number, field="QUAL")
            try:
                if Decimal(qual_raw) != Decimal(str(quality)):
                    ledger.loss(
                        "precision_reduced",
                        "degrading",
                        _location(source_id, record=total_records, field="QUAL"),
                        f"QUAL was carried as a binary float; raw value {qual_raw!r} is retained",
                    )
            except InvalidOperation:
                ledger.loss(
                    "type_undetermined",
                    "degrading",
                    _location(source_id, record=total_records, field="QUAL"),
                    f"QUAL uses a non-decimal spelling {qual_raw!r}; raw value is preserved",
                )
        filters = [] if filter_raw == "." else filter_raw.split(";")
        if any(not _IDENTIFIER.fullmatch(value) for value in filters):
            raise VcfParseError("FILTER contains an invalid identifier", line=line_number, field="FILTER")
        info, info_values_raw = _parse_info(
            info_raw,
            info_definitions,
            record=total_records,
            source_id=source_id,
            ledger=ledger,
        )
        format_keys: list[str] = []
        samples: dict[str, dict[str, Any]] = {}
        samples_raw: dict[str, dict[str, str]] = {}
        if sample_names:
            format_keys, samples, samples_raw = _parse_samples(
                fields[8],
                fields[9:],
                sample_names,
                format_definitions,
                record=total_records,
                source_id=source_id,
                alt_count=len(alternate),
                ledger=ledger,
            )
        record_location = _location(source_id, record=total_records)
        ledger.mapped_location(record_location)
        if total_records <= max_items:
            variants.append(
                {
                    "record": total_records,
                    "source_line": line_number,
                    "source_line_sha256": hashlib.sha256(line.encode("utf-8")).hexdigest(),
                    "chrom": chrom,
                    "pos": position,
                    "id": [] if id_raw == "." else id_raw.split(";"),
                    "ref": ref,
                    "alt": alternate,
                    "qual": quality,
                    "qual_raw": qual_raw,
                    "filter": filters,
                    "filter_raw": filter_raw,
                    "info": info,
                    "info_raw": info_values_raw,
                    "format": format_keys,
                    "samples": samples,
                    "samples_raw": samples_raw,
                }
            )

    header = {
        "fileformat": _header_value(metadata, "fileformat"),
        "columns": columns,
        "samples": sample_names,
        "reference_build": effective_reference,
        "reference_source": (
            "caller"
            if reference_build is not None
            else "header"
            if header_reference is not None
            else "missing"
        ),
        "metadata": metadata,
        "info_definitions": info_definitions,
        "format_definitions": format_definitions,
        "contig_definitions": contig_definitions,
        "raw_lines": header_lines + [column_line],
    }
    manifest = {
        "source_id": source_id,
        "declared_format": "text/vcf",
        "source_digest": hashlib.sha256(raw_bytes).hexdigest(),
        "byte_length": len(raw_bytes),
        "adapter": VCF_ADAPTER,
        "adapter_version": VCF_ADAPTER_VERSION,
        "reference_build": effective_reference,
        "provenance": dict(provenance) if provenance else None,
    }
    document: dict[str, Any] = {
        "schema": VCF_SCHEMA,
        "manifest": manifest,
        "header": header,
        "variants": variants,
        "variant_count": total_records,
        "omitted_variants": total_records - len(variants),
        "semantic_loss": ledger.to_wire(),
        "conformance": {
            "passed": True,
            "verified": True,
            "checks": {
                "fileformat_header": "pass",
                "column_header": "pass",
                "record_structure": "pass",
                "typed_value_projection": "pass",
                "semantic_loss_audit": "pass",
            },
            "limitations": [
                "this reader handles bounded text VCF; indexed/compressed/random-access workflows should use pysam behind the same contract",
                "a successful parse validates source structure, not the truth of caller-supplied reference, provenance, or sample declarations",
                "typed values retain raw spellings so downstream code can distinguish representation from interpretation",
            ],
        },
        "max_items": max_items,
        "max_records": max_records,
    }
    document["document_digest"] = content_digest(document)
    return VcfParseResult(document)


__all__ = [
    "MAX_VCF_BYTES",
    "MAX_VCF_ITEMS",
    "MAX_VCF_RECORDS",
    "VCF_ADAPTER",
    "VCF_ADAPTER_VERSION",
    "VCF_SCHEMA",
    "VcfAdapter",
    "VcfLoss",
    "VcfParseError",
    "VcfParseResult",
    "parse_vcf",
]
