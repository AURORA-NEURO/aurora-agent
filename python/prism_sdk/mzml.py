"""Bounded mzML XML metadata auditing for mass-spectrometry workflows.

The reader validates XML and spectrum metadata while refusing to decode binary m/z, intensity, or
time arrays. Spectrum identifiers and binary payloads are source-bound digests; declared array
types, compression, point counts, CV accessions, and structural counts remain available for
quality-control and routing without turning the SDK into a mass-spectrometry interpretation engine.
"""

from __future__ import annotations

from collections import Counter
from dataclasses import dataclass
import io
import re
import xml.etree.ElementTree as ET
from typing import Any, Iterable, Mapping

from .authoring import content_digest
from .errors import ArgumentError


MZML_SCHEMA = "bioprism-python-mzml/0.1"
MZML_ADAPTER = "bioprism.python.mzml_text"
MZML_ADAPTER_VERSION = "0.1.0"
MZML_FORMAT = "application/mzml"
MAX_MZML_BYTES = 100_000_000
MAX_MZML_SPECTRA = 100_000
MAX_MZML_ITEMS = 1_000
MAX_MZML_ELEMENTS = 2_000_000
MAX_MZML_DEPTH = 256
MAX_MZML_TEXT_BYTES = 4_000_000
_SEVERITY_ORDER = {"advisory": 0, "degrading": 1, "blocking": 2}
_BASE64 = re.compile(r"^[A-Za-z0-9+/=\s]*$")
_MS_LEVEL_ACCESSIONS = {"MS:1000511", "MS:1000511"}
_MS_LEVEL_NAMES = {"ms level", "mslevel"}
_ARRAY_TYPES = {
    "MS:1000514": "m/z",
    "MS:1000515": "intensity",
    "MS:1000595": "time",
    "MS:1003008": "ion mobility drift time",
}
_COMPRESSIONS = {
    "MS:1000574": "zlib",
    "MS:1000576": "none",
    "MS:1002312": "numpress linear",
    "MS:1002313": "numpress positive integer",
    "MS:1002314": "numpress short logged float",
}
_PRECISIONS = {
    "MS:1000521": "64-bit float",
    "MS:1000523": "32-bit float",
    "MS:1000522": "32-bit integer",
    "MS:1000520": "64-bit integer",
}


class MzmlParseError(ArgumentError):
    """A structurally invalid or unsafe mzML source."""


@dataclass(frozen=True)
class MzmlFinding:
    """One bounded mzML structural or metadata finding."""

    code: str
    severity: str
    location: Mapping[str, Any]
    detail: str

    def __post_init__(self) -> None:
        if self.severity not in {"warning", "error"}:
            raise ArgumentError(f"unsupported mzML finding severity: {self.severity!r}")

    def to_wire(self) -> dict[str, Any]:
        return {
            "code": self.code,
            "severity": self.severity,
            "location": dict(self.location),
            "detail": self.detail,
        }


@dataclass(frozen=True)
class MzmlParseResult:
    """A validated mzML metadata projection with bounded disclosure."""

    document: Mapping[str, Any]

    @property
    def spectra(self) -> list[Mapping[str, Any]]:
        return list(self.document["spectra"])

    @property
    def valid(self) -> bool:
        return bool(self.document["valid"])

    @property
    def publishable(self) -> bool:
        return bool(self.document["publishable"])

    def to_wire(self) -> dict[str, Any]:
        return dict(self.document)


class _Audit:
    def __init__(self, limit: int) -> None:
        self.limit = limit
        self.findings: list[MzmlFinding] = []
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
            self.findings.append(MzmlFinding(code, severity, dict(location), detail))

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


def _local(tag: Any) -> str:
    if not isinstance(tag, str):
        return ""
    return tag.rsplit("}", 1)[-1]


def _attribute(element: ET.Element, name: str) -> str | None:
    if name in element.attrib:
        return element.attrib[name]
    for key, value in element.attrib.items():
        if _local(key) == name:
            return value
    return None


def _children(element: ET.Element, name: str) -> list[ET.Element]:
    return [child for child in list(element) if _local(child.tag) == name]


def _descendants(element: ET.Element, names: set[str]) -> Iterable[ET.Element]:
    stack = list(reversed(list(element)))
    while stack:
        current = stack.pop()
        if _local(current.tag) in names:
            yield current
        stack.extend(reversed(list(current)))


def _cv_params(element: ET.Element, *, skip_binary: bool = False) -> list[ET.Element]:
    params: list[ET.Element] = []
    stack = list(reversed(list(element)))
    while stack:
        current = stack.pop()
        local = _local(current.tag)
        if local == "cvParam":
            params.append(current)
            continue
        if skip_binary and local in {"binaryDataArray", "binary"}:
            continue
        stack.extend(reversed(list(current)))
    return params


def _digest(source_id: str, value: str) -> str:
    return content_digest({"source_id": source_id, "value": value})


def _validate_limit(name: str, value: int, maximum: int) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or not 1 <= value <= maximum:
        raise ArgumentError(f"{name} must be between 1 and {maximum}")
    return value


def _decode(payload: str | bytes, *, max_bytes: int) -> bytes:
    max_bytes = _validate_limit("max_bytes", max_bytes, MAX_MZML_BYTES)
    if isinstance(payload, bytes):
        raw = payload
    elif isinstance(payload, str):
        raw = payload.encode("utf-8")
    else:
        raise ArgumentError("mzML payload must be text or bytes")
    if len(raw) > max_bytes:
        raise ArgumentError(f"mzML exceeds the {max_bytes}-byte limit")
    if not raw.strip():
        raise MzmlParseError("source is empty")
    try:
        raw.decode("utf-8")
    except UnicodeDecodeError as error:
        raise MzmlParseError(f"source is not valid UTF-8: {error}") from error
    upper = raw.upper()
    if any(marker in upper for marker in (b"<!DOCTYPE", b"<!ENTITY", b"<!ATTLIST")):
        raise MzmlParseError("DTD and entity declarations are not accepted")
    return raw


def _text(element: ET.Element) -> str:
    return "".join(element.itertext())


def _nonnegative_int(value: str | None, *, code: str, location: Mapping[str, Any], audit: _Audit) -> int | None:
    if value is None:
        return None
    try:
        parsed = int(value)
    except (TypeError, ValueError):
        audit.finding(code, "error", location, "value must be a non-negative integer")
        return None
    if parsed < 0:
        audit.finding(code, "error", location, "value must be a non-negative integer")
        return None
    return parsed


def _float_value(value: str | None) -> float | None:
    if value is None:
        return None
    try:
        parsed = float(value)
    except (TypeError, ValueError):
        return None
    return parsed if parsed == parsed and abs(parsed) != float("inf") else None


def _cv_accession(param: ET.Element) -> str | None:
    return _attribute(param, "accession")


def _cv_name(param: ET.Element) -> str:
    return (_attribute(param, "name") or "").strip().lower()


def _binary_array_summary(
    array: ET.Element,
    *,
    source_id: str,
    spectrum_number: int,
    audit: _Audit,
) -> dict[str, Any]:
    location = {"source": source_id, "spectrum": spectrum_number}
    params = _children(array, "cvParam")
    accessions = {accession for accession in (_cv_accession(param) for param in params) if accession}
    array_types = sorted({_ARRAY_TYPES[accession] for accession in accessions if accession in _ARRAY_TYPES})
    compression = sorted({_COMPRESSIONS[accession] for accession in accessions if accession in _COMPRESSIONS})
    precisions = sorted({_PRECISIONS[accession] for accession in accessions if accession in _PRECISIONS})
    if not array_types:
        audit.finding(
            "binary_array_type_missing",
            "warning",
            location,
            "binary array has no recognized m/z, intensity, time, or ion-mobility CV accession",
        )
    binary_nodes = _children(array, "binary")
    if len(binary_nodes) != 1:
        audit.finding(
            "binary_node_count",
            "error",
            location,
            "each binaryDataArray must contain exactly one binary element",
        )
    binary_text = _text(binary_nodes[0]) if binary_nodes else ""
    if binary_text and not _BASE64.fullmatch(binary_text):
        audit.finding(
            "binary_encoding_invalid",
            "error",
            location,
            "binary payload contains characters outside the bounded base64 alphabet",
        )
    encoded_bytes = len(binary_text.encode("ascii", errors="ignore"))
    declared_length = _nonnegative_int(
        _attribute(array, "encodedLength"),
        code="binary_encoded_length",
        location=location,
        audit=audit,
    )
    if declared_length is not None and declared_length != encoded_bytes:
        audit.finding(
            "binary_encoded_length_mismatch",
            "error",
            location,
            "declared encodedLength does not match the non-decoded binary text length",
        )
    return {
        "array_types": array_types,
        "compression": compression,
        "precision": precisions,
        "cv_accession_count": len(accessions),
        "binary_present": bool(binary_nodes),
        "encoded_length": encoded_bytes,
        "binary_digest": _digest(source_id, binary_text) if binary_nodes else None,
    }


def parse_mzml(
    payload: str | bytes,
    *,
    source_id: str,
    provenance: Mapping[str, Any] | None = None,
    max_bytes: int = MAX_MZML_BYTES,
    max_spectra: int = MAX_MZML_SPECTRA,
    max_items: int = MAX_MZML_ITEMS,
) -> MzmlParseResult:
    """Parse bounded mzML XML metadata without decoding binary arrays."""

    if not isinstance(source_id, str) or not source_id.strip():
        raise ArgumentError("source_id must be a non-empty string")
    if provenance is not None and not isinstance(provenance, Mapping):
        raise ArgumentError("provenance must be a mapping when supplied")
    max_spectra = _validate_limit("max_spectra", max_spectra, MAX_MZML_SPECTRA)
    max_items = _validate_limit("max_items", max_items, MAX_MZML_ITEMS)
    raw = _decode(payload, max_bytes=max_bytes)
    try:
        root = ET.fromstring(raw)
    except ET.ParseError as error:
        raise MzmlParseError(f"XML could not be parsed: {error}") from error

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
        "binary m/z, intensity, time, and ion-mobility arrays are inspected only for bounded metadata and are not decoded",
    )
    audit.loss(
        "ontology_term_unmapped",
        "degrading",
        "cvParam",
        "CV accessions are summarized through a bounded known-term table; complete ontology validation and controlled-vocabulary expansion were not performed",
    )
    if provenance_digest is None:
        audit.loss(
            "provenance_unavailable",
            "blocking",
            "provenance",
            "no non-empty provenance projection was supplied",
        )

    element_count = 0
    max_depth_seen = 0
    stack: list[tuple[ET.Element, int]] = [(root, 1)]
    while stack:
        element, depth = stack.pop()
        element_count += 1
        max_depth_seen = max(max_depth_seen, depth)
        if element_count > MAX_MZML_ELEMENTS:
            raise ArgumentError(f"mzML contains more than the {MAX_MZML_ELEMENTS}-element limit")
        if depth > MAX_MZML_DEPTH:
            raise ArgumentError(f"mzML nesting exceeds the {MAX_MZML_DEPTH}-level limit")
        for child in reversed(list(element)):
            stack.append((child, depth + 1))
        if _local(element.tag) == "binary" and len(_text(element).encode("utf-8")) > MAX_MZML_TEXT_BYTES:
            raise ArgumentError(f"mzML binary text exceeds the {MAX_MZML_TEXT_BYTES}-byte element limit")

    if _local(root.tag) != "mzML":
        audit.finding("root_name", "error", {"source": source_id}, "root element must be mzML")
    version = _attribute(root, "version")
    if not version:
        audit.finding("version_missing", "warning", {"source": source_id}, "mzML root has no version attribute")
    spectrum_lists = list(_descendants(root, {"spectrumList"}))
    if len(spectrum_lists) != 1:
        audit.finding("spectrum_list_count", "error", {"source": source_id}, "document must contain exactly one spectrumList")
    spectrum_list = spectrum_lists[0] if spectrum_lists else None
    spectra = list(_descendants(spectrum_list, {"spectrum"})) if spectrum_list is not None else []
    if len(spectra) > max_spectra:
        raise ArgumentError(f"mzML contains more than the {max_spectra}-spectrum limit")
    declared_spectra = _nonnegative_int(
        _attribute(spectrum_list, "count") if spectrum_list is not None else None,
        code="spectrum_list_declared_count",
        location={"source": source_id},
        audit=audit,
    )
    if declared_spectra is None:
        audit.finding(
            "spectrum_list_count_missing",
            "error",
            {"source": source_id},
            "spectrumList must declare its complete spectrum count",
        )
    if declared_spectra is not None and declared_spectra != len(spectra):
        audit.finding(
            "spectrum_count_mismatch",
            "error",
            {"source": source_id},
            "spectrumList count does not match the complete bounded spectrum element count",
        )
    if not spectra:
        audit.finding("spectrum_missing", "error", {"source": source_id}, "document contains no spectrum elements")

    seen_ids: set[str] = set()
    spectrum_rows: list[dict[str, Any]] = []
    ms_levels: Counter[str] = Counter()
    array_types: Counter[str] = Counter()
    compression_types: Counter[str] = Counter()
    total_points = 0
    total_binary_arrays = 0
    scan_times: list[float] = []
    for number, spectrum in enumerate(spectra, start=1):
        location = {"source": source_id, "spectrum": number}
        spectrum_id = _attribute(spectrum, "id")
        if not spectrum_id:
            audit.finding("spectrum_id_missing", "error", location, "every spectrum requires a non-empty id")
            spectrum_id = f"spectrum-{number}"
        elif spectrum_id in seen_ids:
            audit.finding("spectrum_id_duplicate", "error", location, "spectrum id occurs more than once")
        seen_ids.add(spectrum_id)
        index = _nonnegative_int(_attribute(spectrum, "index"), code="spectrum_index", location=location, audit=audit)
        default_length = _nonnegative_int(
            _attribute(spectrum, "defaultArrayLength"),
            code="spectrum_default_array_length",
            location=location,
            audit=audit,
        )
        total_points += default_length or 0
        params = _cv_params(spectrum, skip_binary=True)
        accessions = {accession for accession in (_cv_accession(param) for param in params) if accession}
        names = {_cv_name(param) for param in params}
        ms_level: int | None = None
        for param in params:
            accession = _cv_accession(param)
            if accession in _MS_LEVEL_ACCESSIONS or _cv_name(param) in _MS_LEVEL_NAMES:
                parsed = _nonnegative_int(
                    _attribute(param, "value"),
                    code="ms_level_value",
                    location=location,
                    audit=audit,
                )
                if parsed is not None:
                    ms_level = parsed
                    ms_levels[str(parsed)] += 1
            if accession == "MS:1000016" or _cv_name(param) == "scan start time":
                parsed_time = _float_value(_attribute(param, "value"))
                if parsed_time is not None:
                    scan_times.append(parsed_time)
        if ms_level is None:
            audit.finding("ms_level_missing", "warning", location, "spectrum has no recognized MS level CV parameter")
        arrays = list(_descendants(spectrum, {"binaryDataArray"}))
        array_rows = [
            _binary_array_summary(array, source_id=source_id, spectrum_number=number, audit=audit)
            for array in arrays
        ]
        for array_row in array_rows:
            total_binary_arrays += 1
            for array_type in array_row["array_types"]:
                array_types[array_type] += 1
            for compression in array_row["compression"]:
                compression_types[compression] += 1
        spectrum_rows.append(
            {
                "spectrum_index": index,
                "spectrum_id_digest": _digest(source_id, spectrum_id),
                "default_array_length": default_length,
                "ms_level": ms_level,
                "cv_accession_count": len(accessions),
                "cv_name_count": len(names),
                "binary_array_count": len(arrays),
                "binary_arrays": array_rows,
                "precursor_count": len(list(_descendants(spectrum, {"precursor"}))),
                "product_count": len(list(_descendants(spectrum, {"product"}))),
            }
        )

    valid = audit.errors == 0
    publishable = valid and audit.max_loss_severity != "blocking"
    source_digest = content_digest({"source_id": source_id, "payload": raw.decode("utf-8")})
    manifest = {
        "source_id": source_id,
        "source_digest": source_digest,
        "adapter": MZML_ADAPTER,
        "adapter_version": MZML_ADAPTER_VERSION,
        "declared_format": MZML_FORMAT,
        "version": version,
        "spectrum_count": len(spectra),
        "declared_spectrum_count": declared_spectra,
        "provenance_digest": provenance_digest,
        "bytes_read": True,
        "binary_arrays_decoded": False,
        "spectrum_identifiers_disclosed": False,
    }
    document: dict[str, Any] = {
        "schema": MZML_SCHEMA,
        "workflow": "mzml_metadata_audit",
        "valid": valid,
        "publishable": publishable,
        "source_id": source_id,
        "manifest": manifest,
        "summary": {
            "spectra": len(spectra),
            "max_xml_depth": max_depth_seen,
            "xml_elements": element_count,
            "declared_points": total_points,
            "binary_arrays": total_binary_arrays,
            "array_type_counts": dict(sorted(array_types.items())),
            "compression_counts": dict(sorted(compression_types.items())),
            "ms_level_counts": dict(sorted(ms_levels.items())),
            "scan_time_min": min(scan_times) if scan_times else None,
            "scan_time_max": max(scan_times) if scan_times else None,
            "errors": audit.errors,
            "warnings": audit.warnings,
            "finding_count": audit.finding_count,
            "blocking_loss_count": audit.blocking_loss_count,
        },
        "spectra": spectrum_rows[:max_items],
        "omitted_spectra": max(0, len(spectrum_rows) - max_items),
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
                "root": "pass" if _local(root.tag) == "mzML" else "fail",
                "spectrum_list": "pass" if len(spectrum_lists) == 1 and declared_spectra == len(spectra) else "fail",
                "spectrum_identity": "pass" if len(seen_ids) == len(spectra) and all(_attribute(spectrum, "id") for spectrum in spectra) else "fail",
                "binary_boundaries": "pass" if not any(finding.code.startswith("binary_") and finding.severity == "error" for finding in audit.findings) else "fail",
                "provenance": "pass" if provenance_digest is not None else "fail",
            },
            "limitations": [
                "binary arrays are not decoded, calibrated, converted, or checked for numerical point-level integrity",
                "CV accessions are summarized through a bounded known-term table rather than validated against an external ontology release",
                "the audit proves bounded XML and metadata structure, not peak picking, identification, quantification, or instrument correctness",
                "spectrum and source identifiers are represented by source-bound digests in this projection",
            ],
        },
        "max_spectra": MAX_MZML_SPECTRA,
        "max_items": max_items,
    }
    document["document_digest"] = content_digest(document)
    return MzmlParseResult(document)


class MzmlAdapter:
    """Concrete adapter facade matching the dependency-free bounded mzML route."""

    name = MZML_ADAPTER
    version = MZML_ADAPTER_VERSION
    accepted_formats = ("application/mzml", "application/xml+mass-spectrometry", "text/mzml")
    declared_loss_kinds = frozenset(
        {
            "content_uninterpreted",
            "ontology_term_unmapped",
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
            "scope_dimensions": ["subject", "sample", "assay", "spectrum", "ion"],
            "execution": "python_delegated",
            "optional_dependency": None,
        }

    def parse(
        self,
        mzml: str | bytes,
        *,
        source_id: str,
        provenance: Mapping[str, Any] | None = None,
        max_bytes: int = MAX_MZML_BYTES,
        max_spectra: int = MAX_MZML_SPECTRA,
        max_items: int = MAX_MZML_ITEMS,
    ) -> MzmlParseResult:
        return parse_mzml(
            mzml,
            source_id=source_id,
            provenance=provenance,
            max_bytes=max_bytes,
            max_spectra=max_spectra,
            max_items=max_items,
        )


__all__ = [
    "MAX_MZML_BYTES",
    "MAX_MZML_DEPTH",
    "MAX_MZML_ELEMENTS",
    "MAX_MZML_ITEMS",
    "MAX_MZML_SPECTRA",
    "MZML_ADAPTER",
    "MZML_ADAPTER_VERSION",
    "MZML_FORMAT",
    "MZML_SCHEMA",
    "MzmlAdapter",
    "MzmlFinding",
    "MzmlParseError",
    "MzmlParseResult",
    "parse_mzml",
]
