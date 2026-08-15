"""Bounded GFF3/GTF-style genomic feature auditing without attribute disclosure."""

from __future__ import annotations

from collections import Counter
from dataclasses import dataclass
import math
import re
from typing import Any, Mapping
from urllib.parse import unquote

from .authoring import content_digest
from .errors import ArgumentError


GFF3_SCHEMA = "bioprism-python-gff3/0.1"
GFF3_ADAPTER = "bioprism.python.gff3_text"
GFF3_ADAPTER_VERSION = "0.1.0"
GFF3_FORMAT = "text/gff3"
GTF_FORMAT = "text/gtf"
MAX_GFF3_BYTES = 50_000_000
MAX_GFF3_FEATURES = 500_000
MAX_GFF3_ITEMS = 1_000
MAX_GFF3_ATTRIBUTE_BYTES = 1_000_000
_SEVERITY_ORDER = {"advisory": 0, "degrading": 1, "blocking": 2}
_GFF_KEY = re.compile(r"^[A-Za-z][A-Za-z0-9_.:-]*$")
_GTF_ATTRIBUTE = re.compile(r"^\s*([A-Za-z][A-Za-z0-9_.:-]*)\s+\"([^\"]*)\"\s*$")


class Gff3ParseError(ArgumentError):
    """A structurally invalid GFF3/GTF source with a stable line locator."""

    def __init__(self, message: str, *, line: int | None = None, feature: int | None = None) -> None:
        location = ""
        if line is not None:
            location += f" at line {line}"
        if feature is not None:
            location += f" feature {feature}"
        super().__init__(f"GFF3 parse refused{location}: {message}")


@dataclass(frozen=True)
class Gff3Finding:
    """One bounded genomic annotation finding."""

    code: str
    severity: str
    location: Mapping[str, Any]
    detail: str

    def __post_init__(self) -> None:
        if self.severity not in {"warning", "error"}:
            raise ArgumentError(f"unsupported GFF3 finding severity: {self.severity!r}")

    def to_wire(self) -> dict[str, Any]:
        return {
            "code": self.code,
            "severity": self.severity,
            "location": dict(self.location),
            "detail": self.detail,
        }


@dataclass(frozen=True)
class Gff3ParseResult:
    """A validated, bounded genomic feature projection."""

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
class _Feature:
    seqid: str
    source: str
    feature_type: str
    start: int
    end: int
    score: float | None
    strand: str
    phase: str
    attributes: Mapping[str, tuple[str, ...]]
    line: int


class _Audit:
    def __init__(self, limit: int) -> None:
        self.limit = limit
        self.findings: list[Gff3Finding] = []
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
            self.findings.append(Gff3Finding(code, severity, dict(location), detail))

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
    max_bytes = _validate_limit("max_bytes", max_bytes, MAX_GFF3_BYTES)
    if isinstance(payload, bytes):
        if len(payload) > max_bytes:
            raise ArgumentError(f"GFF3 exceeds the {max_bytes}-byte limit")
        try:
            return payload.decode("utf-8")
        except UnicodeDecodeError as error:
            raise ArgumentError(f"GFF3 is not valid UTF-8: {error}") from error
    if isinstance(payload, str):
        if len(payload.encode("utf-8")) > max_bytes:
            raise ArgumentError(f"GFF3 exceeds the {max_bytes}-byte limit")
        return payload
    raise ArgumentError("GFF3 payload must be text or bytes")


def _parse_gff_attributes(raw: str, *, line: int, feature: int, annotation_format: str) -> dict[str, tuple[str, ...]]:
    if raw == ".":
        return {}
    if len(raw.encode("utf-8")) > MAX_GFF3_ATTRIBUTE_BYTES:
        raise ArgumentError(f"GFF3 attributes exceed the {MAX_GFF3_ATTRIBUTE_BYTES}-byte limit")
    result: dict[str, tuple[str, ...]] = {}
    for item in raw.split(";"):
        item = item.strip()
        if not item:
            if annotation_format == "gtf":
                continue
            raise Gff3ParseError("attributes contain an empty item", line=line, feature=feature)
        if annotation_format == "gtf":
            match = _GTF_ATTRIBUTE.fullmatch(item)
            if match is None:
                raise Gff3ParseError("GTF attribute must use key \"value\" syntax", line=line, feature=feature)
            key, value = match.groups()
            values = (value,)
        else:
            if "=" not in item:
                raise Gff3ParseError("GFF3 attribute must use key=value syntax", line=line, feature=feature)
            key, raw_value = item.split("=", 1)
            if not _GFF_KEY.fullmatch(key):
                raise Gff3ParseError("GFF3 attribute key is not a valid identifier", line=line, feature=feature)
            values = tuple(unquote(value) for value in raw_value.split(","))
            if any(value == "" for value in values):
                raise Gff3ParseError("GFF3 attribute contains an empty value", line=line, feature=feature)
        if key in result:
            raise Gff3ParseError(f"attribute key {key!r} occurs more than once", line=line, feature=feature)
        result[key] = values
    return result


def _parse_feature(line_text: str, *, line: int, feature: int, annotation_format: str) -> _Feature:
    columns = line_text.split("\t")
    if len(columns) != 9:
        raise Gff3ParseError("feature row must contain exactly nine tab-separated columns", line=line, feature=feature)
    seqid, source, feature_type, raw_start, raw_end, raw_score, strand, phase, raw_attributes = columns
    if not seqid or not feature_type or any(ord(character) < 33 for character in seqid + feature_type):
        raise Gff3ParseError("seqid and feature type must be non-empty printable values", line=line, feature=feature)
    try:
        start = int(raw_start)
        end = int(raw_end)
    except ValueError as error:
        raise Gff3ParseError("start and end must be integers", line=line, feature=feature) from error
    if start < 1 or end < start:
        raise Gff3ParseError("coordinates must satisfy 1 <= start <= end", line=line, feature=feature)
    score: float | None
    if raw_score == ".":
        score = None
    else:
        try:
            score = float(raw_score)
        except ValueError as error:
            raise Gff3ParseError("score must be '.' or a finite number", line=line, feature=feature) from error
        if not math.isfinite(score):
            raise Gff3ParseError("score must be finite", line=line, feature=feature)
    if strand not in {"+", "-", ".", "?"}:
        raise Gff3ParseError("strand must be one of '+', '-', '.', or '?'", line=line, feature=feature)
    if phase not in {"0", "1", "2", "."}:
        raise Gff3ParseError("phase must be '.', '0', '1', or '2'", line=line, feature=feature)
    attributes = _parse_gff_attributes(raw_attributes, line=line, feature=feature, annotation_format=annotation_format)
    if feature_type.lower() == "cds" and phase == ".":
        raise Gff3ParseError("CDS feature requires a phase of 0, 1, or 2", line=line, feature=feature)
    return _Feature(seqid, source, feature_type, start, end, score, strand, phase, attributes, line)


def parse_gff3(
    payload: str | bytes,
    *,
    source_id: str,
    provenance: Mapping[str, Any] | None = None,
    annotation_format: str = "gff3",
    max_bytes: int = MAX_GFF3_BYTES,
    max_features: int = MAX_GFF3_FEATURES,
    max_items: int = MAX_GFF3_ITEMS,
) -> Gff3ParseResult:
    """Parse bounded GFF3 or GTF features and audit coordinate/reference integrity."""

    if not isinstance(source_id, str) or not source_id.strip():
        raise ArgumentError("source_id must be a non-empty string")
    if provenance is not None and not isinstance(provenance, Mapping):
        raise ArgumentError("provenance must be a mapping when supplied")
    if not isinstance(annotation_format, str) or annotation_format.lower() not in {"gff3", "gtf"}:
        raise ArgumentError("annotation_format must be 'gff3' or 'gtf'")
    annotation_format = annotation_format.lower()
    max_features = _validate_limit("max_features", max_features, MAX_GFF3_FEATURES)
    max_items = _validate_limit("max_items", max_items, MAX_GFF3_ITEMS)
    text = _decode(payload, max_bytes=max_bytes)
    if not text:
        raise Gff3ParseError("source is empty")
    lines = text.split("\n")
    if lines and lines[-1] == "":
        lines.pop()
    features: list[_Feature] = []
    directives = 0
    comments = 0
    embedded_fasta = False
    embedded_fasta_lines = 0
    gff_version_seen = False
    sequence_regions: list[tuple[str, int, int]] = []
    for line_number, raw_line in enumerate(lines, start=1):
        line = raw_line[:-1] if raw_line.endswith("\r") else raw_line
        if "\r" in line:
            raise Gff3ParseError("lone carriage return is not a record separator", line=line_number)
        if embedded_fasta:
            embedded_fasta_lines += 1
            continue
        if line == "##FASTA":
            embedded_fasta = True
            directives += 1
            continue
        if line.startswith("##"):
            directives += 1
            if line.lower().startswith("##gff-version"):
                gff_version_seen = True
            if line.startswith("##sequence-region"):
                parts = line.split()
                if len(parts) != 4:
                    raise Gff3ParseError("sequence-region directive requires seqid, start, and end", line=line_number)
                try:
                    region_start = int(parts[2])
                    region_end = int(parts[3])
                except ValueError as error:
                    raise Gff3ParseError("sequence-region coordinates must be integers", line=line_number) from error
                if region_start < 1 or region_end < region_start:
                    raise Gff3ParseError("sequence-region coordinates are invalid", line=line_number)
                sequence_regions.append((parts[1], region_start, region_end))
            continue
        if line.startswith("#"):
            comments += 1
            continue
        if not line:
            raise Gff3ParseError("blank lines are not accepted between feature rows", line=line_number)
        if len(features) >= max_features:
            raise ArgumentError(f"GFF3 contains more than the {max_features}-feature limit")
        features.append(_parse_feature(line, line=line_number, feature=len(features) + 1, annotation_format=annotation_format))
    if annotation_format == "gff3" and not gff_version_seen:
        # A missing pragma is useful evidence, but many valid exporters omit it; keep parsing and warn.
        missing_version = True
    else:
        missing_version = False
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
        "attribute values, feature identifiers, and embedded FASTA bases are not emitted; bounded keys, coordinates, and source-bound digests are carried",
    )
    audit.loss(
        "ontology_term_unmapped",
        "degrading",
        "feature_type",
        "feature types and attribute keys are preserved as bounded labels without external ontology or vocabulary resolution",
    )
    audit.loss(
        "coordinate_frame_not_carried",
        "degrading",
        "coordinates",
        "coordinates are validated as source-local intervals; assembly/reference-build identity is not inferred",
    )
    if provenance_digest is None:
        audit.loss("provenance_unavailable", "blocking", "provenance", "no non-empty provenance projection was supplied")
    if missing_version:
        audit.finding("gff_version_missing", "warning", {"source": source_id}, "GFF3 stream has no ##gff-version directive")
    if not features:
        audit.finding("feature_missing", "error", {"source": source_id}, "annotation source contains no feature rows")

    feature_ids: dict[str, int] = {}
    parent_map: dict[str, tuple[str, ...]] = {}
    type_counts: Counter[str] = Counter()
    seqid_digests: set[str] = set()
    source_digests: set[str] = set()
    record_rows: list[dict[str, Any]] = []
    total_span = 0
    parent_edges = 0
    duplicate_id_count = 0
    for number, feature in enumerate(features, start=1):
        location = {"source": source_id, "feature": number, "line": feature.line}
        feature_id_values = feature.attributes.get("ID", ())
        feature_id = feature_id_values[0] if feature_id_values else None
        if len(feature_id_values) > 1:
            audit.finding("feature_id_cardinality", "error", location, "feature ID attribute must contain exactly one identifier")
        if feature_id is not None:
            if feature_id in feature_ids:
                duplicate_id_count += 1
                audit.finding("feature_id_duplicate", "error", location, "feature ID occurs more than once")
            feature_ids.setdefault(feature_id, number)
            parent_map[feature_id] = feature.attributes.get("Parent", ())
        parent_values = feature.attributes.get("Parent", ())
        parent_edges += len(parent_values)
        type_counts[feature.feature_type] += 1
        seqid_digest = _digest(source_id, feature.seqid)
        source_digest = _digest(source_id, feature.source)
        seqid_digests.add(seqid_digest)
        source_digests.add(source_digest)
        total_span += feature.end - feature.start + 1
        identity = feature_id or f"line:{feature.line}:{feature.seqid}:{feature.start}:{feature.end}:{feature.feature_type}"
        record_rows.append(
            {
                "feature": number,
                "line": feature.line,
                "feature_id_digest": _digest(source_id, identity),
                "parent_id_digests": sorted(_digest(source_id, parent) for parent in parent_values),
                "seqid_digest": seqid_digest,
                "source_digest": source_digest,
                "type": feature.feature_type,
                "start": feature.start,
                "end": feature.end,
                "span": feature.end - feature.start + 1,
                "score_present": feature.score is not None,
                "strand": feature.strand,
                "phase": feature.phase,
                "attribute_keys": sorted(feature.attributes),
                "attribute_count": len(feature.attributes),
            }
        )
    unresolved_parents = 0
    for feature_number, feature in enumerate(features, start=1):
        for parent in feature.attributes.get("Parent", ()):
            if parent not in feature_ids:
                unresolved_parents += 1
                audit.finding(
                    "parent_unresolved",
                    "error",
                    {"source": source_id, "feature": feature_number},
                    "Parent reference does not resolve to an ID in this bounded source",
                )

    cycle_count = 0
    for start_id in parent_map:
        seen: set[str] = set()
        current = start_id
        while current in parent_map:
            if current in seen:
                cycle_count += 1
                audit.finding(
                    "parent_cycle",
                    "error",
                    {"source": source_id, "feature_id_digest": _digest(source_id, current)},
                    "feature Parent references contain a cycle",
                )
                break
            seen.add(current)
            parents = parent_map[current]
            if not parents:
                break
            current = parents[0]

    valid = audit.errors == 0
    publishable = valid and audit.max_loss_severity != "blocking"
    source_digest = content_digest({"source_id": source_id, "payload": text})
    manifest = {
        "source_id": source_id,
        "source_digest": source_digest,
        "adapter": GFF3_ADAPTER,
        "adapter_version": GFF3_ADAPTER_VERSION,
        "declared_format": GFF3_FORMAT if annotation_format == "gff3" else GTF_FORMAT,
        "annotation_format": annotation_format,
        "feature_count": len(features),
        "directive_count": directives,
        "comment_count": comments,
        "embedded_fasta_present": embedded_fasta,
        "embedded_fasta_lines": embedded_fasta_lines,
        "sequence_region_count": len(sequence_regions),
        "provenance_digest": provenance_digest,
        "bytes_read": True,
        "feature_identifiers_disclosed": False,
        "attribute_values_disclosed": False,
        "embedded_fasta_disclosed": False,
    }
    document: dict[str, Any] = {
        "schema": GFF3_SCHEMA,
        "workflow": "gff3_feature_hierarchy_audit",
        "valid": valid,
        "publishable": publishable,
        "source_id": source_id,
        "manifest": manifest,
        "summary": {
            "features": len(features),
            "feature_type_counts": dict(sorted(type_counts.items())),
            "unique_seqid_count": len(seqid_digests),
            "unique_source_count": len(source_digests),
            "total_annotated_span": total_span,
            "parent_edges": parent_edges,
            "unresolved_parents": unresolved_parents,
            "parent_cycles": cycle_count,
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
                "feature_structure": "pass" if features else "fail",
                "coordinate_order": "pass",
                "identifier_uniqueness": "pass" if duplicate_id_count == 0 else "fail",
                "parent_resolution": "pass" if unresolved_parents == 0 and cycle_count == 0 else "fail",
                "provenance": "pass" if provenance_digest is not None else "fail",
            },
            "limitations": [
                "feature IDs, Parent values, attribute values, and embedded FASTA bases are represented by source-bound digests or counts rather than disclosed",
                "coordinates are source-local and do not establish an assembly, reference build, or coordinate-frame identity",
                "feature types and attribute keys are not resolved against an external ontology release",
                "the audit validates annotation structure and hierarchy, not biological correctness or transcript/protein consequences",
            ],
        },
        "max_features": MAX_GFF3_FEATURES,
        "max_items": max_items,
    }
    document["document_digest"] = content_digest(document)
    return Gff3ParseResult(document)


class Gff3Adapter:
    """Concrete adapter facade matching the dependency-free bounded GFF3 route."""

    name = GFF3_ADAPTER
    version = GFF3_ADAPTER_VERSION
    accepted_formats = ("application/gff3", "text/gff3", "application/gtf", "text/x-gtf")
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
            "scope_dimensions": ["subject", "sample", "reference", "feature", "interval"],
            "execution": "python_delegated",
            "optional_dependency": None,
        }

    def parse(
        self,
        annotation: str | bytes,
        *,
        source_id: str,
        provenance: Mapping[str, Any] | None = None,
        annotation_format: str = "gff3",
        max_bytes: int = MAX_GFF3_BYTES,
        max_features: int = MAX_GFF3_FEATURES,
        max_items: int = MAX_GFF3_ITEMS,
    ) -> Gff3ParseResult:
        return parse_gff3(
            annotation,
            source_id=source_id,
            provenance=provenance,
            annotation_format=annotation_format,
            max_bytes=max_bytes,
            max_features=max_features,
            max_items=max_items,
        )


__all__ = [
    "GFF3_ADAPTER",
    "GFF3_ADAPTER_VERSION",
    "GFF3_FORMAT",
    "GFF3_SCHEMA",
    "GTF_FORMAT",
    "Gff3Adapter",
    "Gff3Finding",
    "Gff3ParseError",
    "Gff3ParseResult",
    "MAX_GFF3_ATTRIBUTE_BYTES",
    "MAX_GFF3_BYTES",
    "MAX_GFF3_FEATURES",
    "MAX_GFF3_ITEMS",
    "parse_gff3",
]
