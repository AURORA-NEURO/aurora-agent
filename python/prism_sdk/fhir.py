"""Bounded structural and provenance auditing for FHIR JSON resources.

This module is intentionally not a clinical terminology validator or a patient-record
interpreter. It audits the portion of a FHIR resource set that a transport, indexing, or
cross-domain planning layer can prove without pretending to understand every profile, extension,
code system, narrative, or clinical value. Identifiers and references are represented by
source-bound digests; resource values remain outside the projection unless they are safe
structural fields such as ``resourceType`` or ``status``.
"""

from __future__ import annotations

from collections import Counter
from dataclasses import dataclass
import json
import re
from typing import Any, Mapping, Sequence

from .authoring import content_digest
from .errors import ArgumentError


FHIR_SCHEMA = "bioprism-python-fhir/0.1"
FHIR_ADAPTER = "bioprism.python.fhir_manifest"
FHIR_ADAPTER_VERSION = "0.1.0"
FHIR_JSON_ADAPTER = "bioprism.python.fhir_json"
FHIR_NDJSON_ADAPTER = "bioprism.python.fhir_ndjson"
FHIR_NDJSON_FORMAT = "application/fhir+ndjson"
MAX_FHIR_BYTES = 50_000_000
MAX_FHIR_RESOURCES = 100_000
MAX_FHIR_ITEMS = 1_000
MAX_FHIR_ELEMENTS = 1_000_000
MAX_FHIR_DEPTH = 32
MAX_FHIR_PROFILES = 256
MAX_FHIR_REFERENCES = 1_000_000
MAX_FHIR_STRING_BYTES = 4_096

_RESOURCE_TYPE = re.compile(r"^[A-Z][A-Za-z0-9]{0,63}$")
_RESOURCE_ID = re.compile(r"^[A-Za-z0-9\-.]{1,64}$")
_REFERENCE = re.compile(r"^([A-Z][A-Za-z0-9]{0,63})/([A-Za-z0-9\-.]{1,64})$")
_BUNDLE_TYPES = frozenset(
    {
        "batch",
        "batch-response",
        "collection",
        "document",
        "history",
        "message",
        "searchset",
        "transaction",
        "transaction-response",
    }
)


@dataclass(frozen=True)
class FhirFinding:
    code: str
    severity: str
    path: str
    detail: str
    related_paths: tuple[str, ...] = ()

    def __post_init__(self) -> None:
        if self.severity not in {"error", "warning", "info"}:
            raise ArgumentError(f"invalid FHIR finding severity: {self.severity!r}")

    def to_dict(self) -> dict[str, Any]:
        result: dict[str, Any] = {"code": self.code, "severity": self.severity, "path": self.path, "detail": self.detail}
        if self.related_paths:
            result["related_paths"] = list(self.related_paths)
        return result


@dataclass(frozen=True)
class FhirAuditResult:
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


class FhirAdapter:
    """Small adapter descriptor for callers that use Python directly."""

    name = FHIR_ADAPTER
    version = FHIR_ADAPTER_VERSION
    accepted_formats = ("application/fhir-manifest",)
    declared_loss_kinds = frozenset({"provenance_unavailable", "ontology_term_unmapped", "content_uninterpreted", "type_undetermined"})

    def manifest(self) -> dict[str, Any]:
        return {
            "name": self.name,
            "version": self.version,
            "accepted_formats": list(self.accepted_formats),
            "conformance_level": "normalize",
            "declared_loss_kinds": sorted(self.declared_loss_kinds),
            "scope_dimensions": ["subject", "encounter", "resource", "terminology", "time"],
            "execution": "python_delegated",
            "optional_dependency": None,
        }

    def audit(
        self,
        document: Mapping[str, Any],
        *,
        source_id: str,
        provenance: Mapping[str, Any] | None = None,
        max_items: int = MAX_FHIR_ITEMS,
    ) -> FhirAuditResult:
        return audit_fhir(document, source_id=source_id, provenance=provenance, max_items=max_items)


class _Audit:
    def __init__(self, limit: int) -> None:
        self.limit = limit
        self.findings: list[FhirFinding] = []
        self.total = 0
        self.errors = 0
        self.warnings = 0
        self.codes: set[str] = set()
        self.losses: list[dict[str, Any]] = []
        self.loss_total = 0
        self.blocking_losses = 0
        self.max_loss: str | None = None

    def add(self, code: str, severity: str, path: str, detail: str, related_paths: Sequence[str] = ()) -> None:
        self.total += 1
        self.codes.add(code)
        if severity == "error":
            self.errors += 1
        elif severity == "warning":
            self.warnings += 1
        if len(self.findings) < self.limit:
            self.findings.append(FhirFinding(code, severity, path, detail, tuple(related_paths)))

    def loss(self, kind: str, severity: str, path: str, detail: str) -> None:
        if severity not in {"minor", "degrading", "blocking"}:
            raise ArgumentError(f"invalid FHIR loss severity: {severity!r}")
        self.loss_total += 1
        if severity == "blocking":
            self.blocking_losses += 1
        rank = {"minor": 1, "degrading": 2, "blocking": 3}
        if self.max_loss is None or rank[severity] > rank[self.max_loss]:
            self.max_loss = severity
        if len(self.losses) < self.limit:
            self.losses.append({"kind": kind, "severity": severity, "path": path, "detail": detail})


@dataclass(frozen=True)
class _Reference:
    path: str
    value: str


def _bounded_text(value: Any, *, path: str, field: str, audit: _Audit, maximum: int = MAX_FHIR_STRING_BYTES) -> str | None:
    if not isinstance(value, str) or not value.strip() or len(value.encode("utf-8")) > maximum or any(ord(character) < 0x20 for character in value):
        audit.add("text_invalid", "error", path, f"{field} must be bounded printable text")
        return None
    return value


def _validate_provenance(provenance: Mapping[str, Any] | None, audit: _Audit) -> str | None:
    if not provenance:
        audit.loss(
            "provenance_unavailable",
            "blocking",
            "provenance",
            "no non-empty accession, version, or retrieval context was supplied",
        )
        return None
    try:
        return content_digest(dict(provenance))
    except Exception as error:  # noqa: BLE001 - the audit must name malformed provenance
        audit.add("provenance_invalid", "error", "provenance", f"provenance is not canonical JSON-safe: {error}")
        return None


def _walk(value: Any, *, path: str, depth: int, audit: _Audit, references: list[_Reference], counters: Counter[str]) -> None:
    if depth > MAX_FHIR_DEPTH:
        audit.add("nesting_too_deep", "error", path, f"resource nesting exceeds the {MAX_FHIR_DEPTH}-level bound")
        return
    counters["elements"] += 1
    if counters["elements"] > MAX_FHIR_ELEMENTS:
        raise ArgumentError(f"FHIR document exceeds the {MAX_FHIR_ELEMENTS}-element audit limit")
    if isinstance(value, Mapping):
        for key, child in value.items():
            if not isinstance(key, str) or not key.strip():
                audit.add("key_invalid", "error", path, "FHIR object keys must be non-empty strings")
                continue
            child_path = f"{path}.{key}"
            if key == "reference":
                if not isinstance(child, str):
                    audit.add("reference_invalid", "error", child_path, "Reference.reference must be a bounded string")
                    reference = None
                else:
                    reference = _bounded_text(child, path=child_path, field="Reference.reference", audit=audit)
                if reference is not None:
                    if len(references) >= MAX_FHIR_REFERENCES:
                        raise ArgumentError(f"FHIR document exceeds the {MAX_FHIR_REFERENCES}-reference audit limit")
                    references.append(_Reference(child_path, reference))
            if key == "extension" and isinstance(child, Sequence) and not isinstance(child, (str, bytes)):
                counters["extensions"] += len(child)
            if key == "narrative" or key == "div":
                counters["narrative"] += 1
            _walk(child, path=child_path, depth=depth + 1, audit=audit, references=references, counters=counters)
        if "code" in value and "system" not in value:
            counters["codes_without_system"] += 1
    elif isinstance(value, Sequence) and not isinstance(value, (str, bytes)):
        for index, child in enumerate(value):
            _walk(child, path=f"{path}[{index}]", depth=depth + 1, audit=audit, references=references, counters=counters)
    elif isinstance(value, (str, bytes)):
        if isinstance(value, str) and len(value.encode("utf-8")) > MAX_FHIR_STRING_BYTES * 16:
            audit.add("text_unbounded", "error", path, "FHIR string exceeds the bounded projection limit")


def _resource_entries(document: Mapping[str, Any], audit: _Audit) -> tuple[str | None, list[tuple[str, Mapping[str, Any]]]]:
    root_type = document.get("resourceType")
    if not isinstance(root_type, str) or not _RESOURCE_TYPE.fullmatch(root_type):
        audit.add("resource_type_invalid", "error", "resourceType", "root resourceType must be a FHIR resource name")
        return None, []
    if root_type != "Bundle":
        return None, [("resource[0]", document)]
    bundle_type = document.get("type")
    if not isinstance(bundle_type, str) or bundle_type not in _BUNDLE_TYPES:
        audit.add("bundle_type_invalid", "error", "Bundle.type", f"Bundle.type must be one of {sorted(_BUNDLE_TYPES)!r}")
        bundle_type = None
    entries = document.get("entry", [])
    if not isinstance(entries, Sequence) or isinstance(entries, (str, bytes)):
        audit.add("bundle_entries_invalid", "error", "Bundle.entry", "Bundle.entry must be an array")
        return bundle_type, []
    if len(entries) > MAX_FHIR_RESOURCES:
        raise ArgumentError(f"FHIR Bundle contains more than the {MAX_FHIR_RESOURCES}-resource audit limit")
    result: list[tuple[str, Mapping[str, Any]]] = []
    for index, entry in enumerate(entries):
        path = f"Bundle.entry[{index}]"
        if not isinstance(entry, Mapping):
            audit.add("bundle_entry_invalid", "error", path, "Bundle entries must be objects")
            continue
        resource = entry.get("resource")
        if not isinstance(resource, Mapping):
            audit.add("bundle_resource_missing", "error", path, "Bundle entries must carry a resource object")
            continue
        result.append((f"{path}.resource", resource))
        if "fullUrl" in entry:
            _bounded_text(entry["fullUrl"], path=f"{path}.fullUrl", field="Bundle.fullUrl", audit=audit, maximum=8_192)
    return bundle_type, result


def _reference_class(value: str) -> str:
    if value.startswith("#"):
        return "contained"
    if value.startswith("urn:"):
        return "urn"
    if value.startswith("http://") or value.startswith("https://"):
        return "external"
    if _REFERENCE.fullmatch(value):
        return "local"
    return "opaque"


def _digest(source_id: str, value: str) -> str:
    return content_digest({"source_id": source_id, "value": value})[:24]


def _resource_summary(
    resource: Mapping[str, Any],
    *,
    path: str,
    source_id: str,
    audit: _Audit,
    references: list[_Reference],
    counters: Counter[str],
) -> tuple[dict[str, Any], str | None]:
    resource_type = _bounded_text(resource.get("resourceType"), path=f"{path}.resourceType", field="resourceType", audit=audit, maximum=64)
    if resource_type is None or not _RESOURCE_TYPE.fullmatch(resource_type or ""):
        audit.add("resource_type_invalid", "error", f"{path}.resourceType", "resourceType must match the FHIR resource-name grammar")
        resource_type = None
    raw_id = resource.get("id")
    resource_id: str | None = None
    if raw_id is not None:
        resource_id = _bounded_text(raw_id, path=f"{path}.id", field="id", audit=audit, maximum=64)
        if resource_id is not None and not _RESOURCE_ID.fullmatch(resource_id):
            audit.add("resource_id_invalid", "error", f"{path}.id", "resource id contains characters outside the FHIR id grammar")
            resource_id = None
    else:
        audit.add("resource_id_missing", "warning", path, "resource has no id; it cannot be addressed within a normalized Bundle")

    meta = resource.get("meta")
    profiles: list[str] = []
    if meta is not None:
        if not isinstance(meta, Mapping):
            audit.add("meta_invalid", "error", f"{path}.meta", "meta must be an object when supplied")
        else:
            raw_profiles = meta.get("profile", [])
            if not isinstance(raw_profiles, Sequence) or isinstance(raw_profiles, (str, bytes)):
                audit.add("profiles_invalid", "error", f"{path}.meta.profile", "meta.profile must be an array")
            elif len(raw_profiles) > MAX_FHIR_PROFILES:
                raise ArgumentError(f"{path}.meta.profile exceeds the {MAX_FHIR_PROFILES}-profile audit limit")
            else:
                for index, profile in enumerate(raw_profiles):
                    parsed = _bounded_text(profile, path=f"{path}.meta.profile[{index}]", field="profile URL", audit=audit, maximum=8_192)
                    if parsed is not None:
                        profiles.append(parsed)

    status = resource.get("status")
    safe_status = None
    if status is not None:
        safe_status = _bounded_text(status, path=f"{path}.status", field="status", audit=audit, maximum=128)

    contained = resource.get("contained", [])
    contained_types: list[str] = []
    if contained is not None:
        if not isinstance(contained, Sequence) or isinstance(contained, (str, bytes)):
            audit.add("contained_invalid", "error", f"{path}.contained", "contained must be an array")
        else:
            for index, child in enumerate(contained):
                if not isinstance(child, Mapping):
                    audit.add("contained_resource_invalid", "error", f"{path}.contained[{index}]", "contained resources must be objects")
                    continue
                child_type = child.get("resourceType")
                if isinstance(child_type, str) and _RESOURCE_TYPE.fullmatch(child_type):
                    contained_types.append(child_type)

    before_refs = len(references)
    before_narrative = counters["narrative"]
    _walk(resource, path=path, depth=0, audit=audit, references=references, counters=counters)
    resource_key = f"{resource_type}/{resource_id}" if resource_type and resource_id else None
    summary: dict[str, Any] = {
        "resource_type": resource_type,
        "resource_id_digest": _digest(source_id, resource_key or path),
        "id_present": resource_id is not None,
        "profile_count": len(profiles),
        "profile_digests": [_digest(source_id, profile) for profile in profiles[:MAX_FHIR_ITEMS]],
        "status": safe_status,
        "reference_count": len(references) - before_refs,
        "contained_count": len(contained_types),
        "contained_types": sorted(Counter(contained_types)),
        "has_meta": meta is not None,
        "has_narrative": counters["narrative"] > before_narrative,
    }
    if resource_type in {"Observation", "DiagnosticReport", "Condition", "Procedure", "MedicationRequest"} and not profiles:
        audit.loss(
            "ontology_term_unmapped",
            "degrading",
            path,
            f"{resource_type} has no declared meta.profile; profile-specific terminology and invariants were not established",
        )
    return summary, resource_key


def audit_fhir(
    document: Mapping[str, Any],
    *,
    source_id: str,
    provenance: Mapping[str, Any] | None = None,
    max_items: int = MAX_FHIR_ITEMS,
) -> FhirAuditResult:
    """Audit a FHIR resource or Bundle without interpreting clinical values."""

    if not isinstance(source_id, str) or not source_id.strip():
        raise ArgumentError("source_id must be a non-empty string")
    if not isinstance(document, Mapping):
        raise ArgumentError("FHIR document must be a mapping")
    if isinstance(max_items, bool) or not isinstance(max_items, int) or not 1 <= max_items <= MAX_FHIR_ITEMS:
        raise ArgumentError(f"max_items must be between 1 and {MAX_FHIR_ITEMS}")
    if provenance is not None and not isinstance(provenance, Mapping):
        raise ArgumentError("provenance must be a mapping when supplied")

    audit = _Audit(max_items)
    audit.loss("content_uninterpreted", "degrading", source_id, "clinical values, narrative text, extensions, and profile-specific invariants were not interpreted")
    provenance_digest = _validate_provenance(provenance, audit)
    bundle_type, entries = _resource_entries(document, audit)
    if len(entries) > MAX_FHIR_RESOURCES:
        raise ArgumentError(f"FHIR document contains more than the {MAX_FHIR_RESOURCES}-resource audit limit")

    references: list[_Reference] = []
    counters: Counter[str] = Counter()
    summaries: list[dict[str, Any]] = []
    resource_keys: dict[str, str] = {}
    type_counts: Counter[str] = Counter()
    for path, resource in entries:
        summary, resource_key = _resource_summary(
            resource,
            path=path,
            source_id=source_id,
            audit=audit,
            references=references,
            counters=counters,
        )
        summaries.append(summary)
        if summary["resource_type"]:
            type_counts[summary["resource_type"]] += 1
        if resource_key is not None:
            if resource_key in resource_keys:
                audit.add("resource_duplicate", "error", path, f"resource key {resource_key!r} is duplicated", (resource_keys[resource_key],))
            else:
                resource_keys[resource_key] = path

    reference_counts: Counter[str] = Counter()
    unresolved_internal = 0
    patient_reference_digests: list[str] = []
    for reference in references:
        kind = _reference_class(reference.value)
        reference_counts[kind] += 1
        if kind == "local":
            if reference.value not in resource_keys:
                unresolved_internal += 1
        if reference.path.endswith(".subject.reference") or reference.path.endswith(".patient.reference"):
            patient_reference_digests.append(_digest(source_id, reference.value))
    if unresolved_internal:
        audit.add(
            "reference_unresolved",
            "warning",
            "references",
            f"{unresolved_internal} local-looking references do not resolve within this Bundle; external resolution was not attempted",
        )
    if counters["codes_without_system"]:
        audit.loss(
            "ontology_term_unmapped",
            "degrading",
            "resources",
            f"{counters['codes_without_system']} coded object(s) lack a terminology system and were not mapped",
        )
    if counters["extensions"]:
        audit.loss(
            "content_uninterpreted",
            "degrading",
            "resources.extension",
            f"{counters['extensions']} extension value(s) were counted but not interpreted",
        )

    valid = audit.errors == 0
    publishable = valid and audit.blocking_losses == 0
    source_digest = content_digest(dict(document))
    manifest: dict[str, Any] = {
        "source_id": source_id,
        "source_digest": source_digest,
        "adapter": FHIR_ADAPTER,
        "adapter_version": FHIR_ADAPTER_VERSION,
        "declared_format": "application/fhir-manifest",
        "bundle_type": bundle_type,
        "resource_count": len(entries),
        "resource_type_counts": dict(sorted(type_counts.items())),
        "provenance_digest": provenance_digest,
        "bytes_read": False,
        "patient_identifiers_disclosed": False,
    }
    document_out: dict[str, Any] = {
        "schema": FHIR_SCHEMA,
        "workflow": "fhir_resource_projection_audit",
        "valid": valid,
        "publishable": publishable,
        "source_id": source_id,
        "manifest": manifest,
        "summary": {
            "resources": len(entries),
            "resource_type_counts": dict(sorted(type_counts.items())),
            "references": len(references),
            "reference_scope_counts": dict(sorted(reference_counts.items())),
            "unresolved_internal_references": unresolved_internal,
            "profiles": sum(item["profile_count"] for item in summaries),
            "extensions": counters["extensions"],
            "errors": audit.errors,
            "warnings": audit.warnings,
            "finding_count": audit.total,
            "blocking_loss_count": audit.blocking_losses,
        },
        "references": {
            "count": len(references),
            "scope_counts": dict(sorted(reference_counts.items())),
            "patient_reference_digests": sorted(set(patient_reference_digests))[:max_items],
            "omitted_patient_reference_digests": max(0, len(set(patient_reference_digests)) - max_items),
        },
        "resources": summaries[:max_items],
        "omitted_resources": max(0, len(summaries) - max_items),
        "findings": [finding.to_dict() for finding in audit.findings],
        "omitted_findings": max(0, audit.total - len(audit.findings)),
        "semantic_loss": {
            "audit": "lossy" if audit.loss_total else "lossless",
            "lost_count": audit.loss_total,
            "max_severity": audit.max_loss,
            "lost": audit.losses,
            "omitted_lost": max(0, audit.loss_total - len(audit.losses)),
        },
        "conformance": {
            "level": "normalize",
            "passed": valid,
            "publishable": publishable,
            "checks": {
                "resource_identity": "pass" if "resource_type_invalid" not in audit.codes and "resource_id_invalid" not in audit.codes else "fail",
                "bundle_structure": "pass" if not any(code.startswith("bundle_") for code in audit.codes) else "fail",
                "reference_scope": "pass" if "reference_invalid" not in audit.codes else "fail",
                "provenance": "pass" if provenance_digest is not None else "fail",
            },
            "limitations": [
                "the audit proves bounded FHIR structure and reference scope, not clinical correctness or medical safety",
                "profile invariants, terminology validation, narrative meaning, extensions, contained semantics, and external reference resolution are not independently established",
                "patient and resource identifiers are represented by source-bound digests in this projection",
            ],
        },
        "max_resources": MAX_FHIR_RESOURCES,
        "max_items": max_items,
    }
    document_out["document_digest"] = content_digest(document_out)
    return FhirAuditResult(document_out)


def _unique_object(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise ValueError(f"duplicate JSON object key {key!r}")
        result[key] = value
    return result


def _reject_constant(value: str) -> None:
    raise ValueError(f"non-standard JSON constant {value!r}")


def _parse_json_object(text: str) -> Mapping[str, Any]:
    document = json.loads(text, object_pairs_hook=_unique_object, parse_constant=_reject_constant)
    if not isinstance(document, Mapping):
        raise ValueError("FHIR JSON root must be an object")
    return document


def parse_fhir_json(
    payload: str | bytes,
    *,
    source_id: str,
    provenance: Mapping[str, Any] | None = None,
    max_bytes: int = MAX_FHIR_BYTES,
    max_items: int = MAX_FHIR_ITEMS,
) -> FhirAuditResult:
    """Parse bounded FHIR JSON with duplicate-key and non-standard-number rejection."""

    if isinstance(max_bytes, bool) or not isinstance(max_bytes, int) or not 1 <= max_bytes <= MAX_FHIR_BYTES:
        raise ArgumentError(f"max_bytes must be between 1 and {MAX_FHIR_BYTES}")
    if isinstance(payload, bytes):
        if len(payload) > max_bytes:
            raise ArgumentError(f"FHIR JSON exceeds the {max_bytes}-byte limit")
        try:
            text = payload.decode("utf-8")
        except UnicodeDecodeError as error:
            raise ArgumentError(f"FHIR JSON is not valid UTF-8: {error}") from error
    elif isinstance(payload, str):
        if len(payload.encode("utf-8")) > max_bytes:
            raise ArgumentError(f"FHIR JSON exceeds the {max_bytes}-byte limit")
        text = payload
    else:
        raise ArgumentError("FHIR JSON payload must be text or bytes")
    try:
        document = _parse_json_object(text)
    except (TypeError, ValueError, json.JSONDecodeError) as error:
        raise ArgumentError(f"FHIR JSON could not be parsed: {error}") from error
    return audit_fhir(document, source_id=source_id, provenance=provenance, max_items=max_items)


def parse_fhir_ndjson(
    payload: str | bytes,
    *,
    source_id: str,
    provenance: Mapping[str, Any] | None = None,
    max_bytes: int = MAX_FHIR_BYTES,
    max_records: int = MAX_FHIR_RESOURCES,
    max_items: int = MAX_FHIR_ITEMS,
) -> FhirAuditResult:
    """Parse bounded FHIR Bulk Data NDJSON and audit the complete resource collection."""

    if isinstance(max_records, bool) or not isinstance(max_records, int) or not 1 <= max_records <= MAX_FHIR_RESOURCES:
        raise ArgumentError(f"max_records must be between 1 and {MAX_FHIR_RESOURCES}")
    if isinstance(max_bytes, bool) or not isinstance(max_bytes, int) or not 1 <= max_bytes <= MAX_FHIR_BYTES:
        raise ArgumentError(f"max_bytes must be between 1 and {MAX_FHIR_BYTES}")
    if isinstance(payload, bytes):
        if len(payload) > max_bytes:
            raise ArgumentError(f"FHIR NDJSON exceeds the {max_bytes}-byte limit")
        try:
            text = payload.decode("utf-8")
        except UnicodeDecodeError as error:
            raise ArgumentError(f"FHIR NDJSON is not valid UTF-8: {error}") from error
    elif isinstance(payload, str):
        if len(payload.encode("utf-8")) > max_bytes:
            raise ArgumentError(f"FHIR NDJSON exceeds the {max_bytes}-byte limit")
        text = payload
    else:
        raise ArgumentError("FHIR NDJSON payload must be text or bytes")
    lines = text.splitlines()
    if not lines:
        raise ArgumentError("FHIR NDJSON must contain at least one resource")
    if len(lines) > max_records:
        raise ArgumentError(f"FHIR NDJSON contains more than the {max_records}-record limit")
    resources: list[Mapping[str, Any]] = []
    for line_number, line in enumerate(lines, start=1):
        if not line.strip():
            raise ArgumentError(f"FHIR NDJSON line {line_number} is empty")
        try:
            resources.append(_parse_json_object(line))
        except (TypeError, ValueError, json.JSONDecodeError) as error:
            raise ArgumentError(f"FHIR NDJSON line {line_number} could not be parsed: {error}") from error
    synthetic_bundle = {
        "resourceType": "Bundle",
        "type": "collection",
        "entry": [{"resource": resource} for resource in resources],
    }
    result = audit_fhir(synthetic_bundle, source_id=source_id, provenance=provenance, max_items=max_items)
    document = result.to_wire()
    manifest = dict(document["manifest"])
    manifest.update(
        {
            "adapter": FHIR_NDJSON_ADAPTER,
            "declared_format": FHIR_NDJSON_FORMAT,
            "record_count": len(resources),
            "bytes_read": True,
        }
    )
    document["manifest"] = manifest
    document["document_digest"] = content_digest(document)
    return FhirAuditResult(document)


__all__ = [
    "FHIR_ADAPTER",
    "FHIR_ADAPTER_VERSION",
    "FHIR_JSON_ADAPTER",
    "FHIR_NDJSON_ADAPTER",
    "FHIR_NDJSON_FORMAT",
    "FHIR_SCHEMA",
    "FhirAdapter",
    "FhirAuditResult",
    "FhirFinding",
    "MAX_FHIR_BYTES",
    "MAX_FHIR_ELEMENTS",
    "MAX_FHIR_ITEMS",
    "MAX_FHIR_RESOURCES",
    "audit_fhir",
    "parse_fhir_json",
    "parse_fhir_ndjson",
]
