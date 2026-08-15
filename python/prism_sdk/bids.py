"""Bounded BIDS layout and metadata-inheritance validation.

This module validates a caller-supplied dataset manifest and parsed JSON/TSV projections. It does
not read a filesystem, decompress archives, parse NIfTI headers, inspect DICOM, or infer an affine
from image bytes. That boundary is deliberate: layout/provenance checks are safe and useful in a
dependency-free SDK, while binary image interpretation remains a separate ``nibabel``/``pydicom``
adapter that must emit its own conformance report.
"""

from __future__ import annotations

import csv
from dataclasses import dataclass
import io
import re
from typing import Any, Mapping, Sequence

from .authoring import canonical_json, content_digest
from .errors import ArgumentError


BIDS_SCHEMA = "bioprism-python-bids/0.1"
BIDS_ADAPTER = "bioprism.python.bids_manifest"
BIDS_ADAPTER_VERSION = "0.1.0"
MAX_BIDS_FILES = 50_000
MAX_BIDS_ITEMS = 1_000
MAX_BIDS_METADATA_BYTES = 10_000_000
_ENTITY_KEY = re.compile(r"^[a-z][a-z0-9]*$")
_ENTITY_VALUE = re.compile(r"^[A-Za-z0-9+.-]+$")
_SUFFIX = re.compile(r"^[A-Za-z][A-Za-z0-9]*$")
_KNOWN_DATA_EXTENSIONS = {
    ".nii",
    ".nii.gz",
    ".tsv",
    ".tsv.gz",
    ".bval",
    ".bvec",
    ".edf",
    ".eeg",
    ".set",
    ".fdt",
    ".vhdr",
    ".vmrk",
    ".dat",
    ".con",
    ".sqd",
    ".mef",
    ".snirf",
}
_KNOWN_ENTITY_KEYS = {
    "sub",
    "ses",
    "task",
    "acq",
    "ce",
    "rec",
    "dir",
    "run",
    "echo",
    "flip",
    "inv",
    "mt",
    "part",
    "recording",
    "space",
    "split",
    "desc",
    "hemi",
    "res",
    "den",
    "label",
    "from",
    "to",
    "mode",
    "proc",
    "atlas",
    "roi",
    "stim",
}
_SPECIAL_ROOT_FILES = {
    "dataset_description.json",
    "participants.tsv",
    "participants.json",
    "samples.tsv",
    "samples.json",
    "README",
    "CHANGES",
    "LICENSE",
    "LICENSE.txt",
}


@dataclass(frozen=True)
class BidsFinding:
    code: str
    severity: str
    path: str
    detail: str
    related_paths: tuple[str, ...] = ()

    def __post_init__(self) -> None:
        if self.severity not in {"error", "warning", "info"}:
            raise ArgumentError(f"invalid BIDS finding severity: {self.severity!r}")

    def to_dict(self) -> dict[str, Any]:
        result: dict[str, Any] = {
            "code": self.code,
            "severity": self.severity,
            "path": self.path,
            "detail": self.detail,
        }
        if self.related_paths:
            result["related_paths"] = list(self.related_paths)
        return result


@dataclass(frozen=True)
class _BidsPath:
    path: str
    directory: str
    filename: str
    stem: str
    extension: str
    entities: Mapping[str, str]
    suffix: str
    is_json: bool
    is_data: bool
    is_special: bool


class _Audit:
    def __init__(self, limit: int) -> None:
        self.limit = limit
        self.findings: list[BidsFinding] = []
        self.total = 0
        self.error_count = 0
        self.warning_count = 0
        self.info_count = 0
        self.codes: set[str] = set()

    def add(
        self,
        code: str,
        severity: str,
        path: str,
        detail: str,
        related_paths: Sequence[str] = (),
    ) -> None:
        self.total += 1
        self.codes.add(code)
        if severity == "error":
            self.error_count += 1
        elif severity == "warning":
            self.warning_count += 1
        elif severity == "info":
            self.info_count += 1
        if len(self.findings) < self.limit:
            self.findings.append(BidsFinding(code, severity, path, detail, tuple(related_paths)))

    def has(self, code: str) -> bool:
        return code in self.codes


@dataclass(frozen=True)
class BidsAuditResult:
    """A digest-bound BIDS manifest audit with bounded disclosure."""

    document: Mapping[str, Any]

    @property
    def valid(self) -> bool:
        return bool(self.document["valid"])

    @property
    def findings(self) -> Sequence[Mapping[str, Any]]:
        return self.document["findings"]

    def to_wire(self) -> dict[str, Any]:
        return dict(self.document)


class BidsAdapter:
    """Concrete dependency-free manifest adapter matching the adapter registry route."""

    name = BIDS_ADAPTER
    version = BIDS_ADAPTER_VERSION
    accepted_formats = ("application/bids-manifest",)
    declared_loss_kinds = frozenset(
        {
            "coordinate_frame_not_carried",
            "provenance_unavailable",
            "content_uninterpreted",
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
            "scope_dimensions": ["subject", "session", "acquisition", "image", "event"],
            "execution": "python_delegated",
            "optional_dependency": None,
        }

    def audit(
        self,
        files: Sequence[str],
        *,
        source_id: str,
        metadata: Mapping[str, Mapping[str, Any]] | None = None,
        participants_tsv: str | None = None,
        max_files: int = MAX_BIDS_FILES,
        max_items: int = MAX_BIDS_ITEMS,
    ) -> BidsAuditResult:
        return audit_bids(
            files,
            source_id=source_id,
            metadata=metadata,
            participants_tsv=participants_tsv,
            max_files=max_files,
            max_items=max_items,
        )


def _text(name: str, value: str, maximum: int = 512, *, allow_tsv_controls: bool = False) -> None:
    if not isinstance(value, str) or not value.strip():
        raise ArgumentError(f"{name} must be a non-empty string")
    allowed = {"\t", "\n", "\r"} if allow_tsv_controls else set()
    if any(ord(character) < 0x20 and character not in allowed for character in value):
        raise ArgumentError(f"{name} must not contain control characters")
    if len(value.encode("utf-8")) > maximum:
        raise ArgumentError(f"{name} exceeds the {maximum}-byte limit")


def _limit(name: str, value: int, maximum: int) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or not 1 <= value <= maximum:
        raise ArgumentError(f"{name} must be between 1 and {maximum}")
    return value


def _normalize_path(path: str) -> str:
    _text("BIDS path", path, 2_048)
    if "\\" in path or path.startswith("/") or path.startswith("./"):
        raise ArgumentError(f"BIDS path must be a relative forward-slash path: {path!r}")
    parts = path.split("/")
    if any(not part or part in {".", ".."} for part in parts):
        raise ArgumentError(f"BIDS path contains an empty or traversal segment: {path!r}")
    return "/".join(parts)


def _split_extension(filename: str) -> tuple[str, str]:
    for extension in (".nii.gz", ".tsv.gz"):
        if filename.endswith(extension):
            return filename[: -len(extension)], extension
    if "." not in filename:
        return filename, ""
    stem, extension = filename.rsplit(".", 1)
    return stem, f".{extension}"


def _parse_entities(stem: str, *, path: str) -> tuple[dict[str, str], str]:
    tokens = stem.split("_")
    if not tokens or not tokens[-1]:
        raise ArgumentError(f"BIDS file has no suffix: {path!r}")
    suffix = tokens[-1]
    entities: dict[str, str] = {}
    for token in tokens[:-1]:
        if "-" not in token:
            raise ArgumentError(f"BIDS token {token!r} is neither an entity nor the suffix in {path!r}")
        key, value = token.split("-", 1)
        if not _ENTITY_KEY.fullmatch(key) or not _ENTITY_VALUE.fullmatch(value):
            raise ArgumentError(f"invalid BIDS entity token {token!r} in {path!r}")
        if key in entities:
            raise ArgumentError(f"BIDS entity {key!r} occurs more than once in {path!r}")
        entities[key] = value
    if not _SUFFIX.fullmatch(suffix):
        raise ArgumentError(f"invalid BIDS suffix {suffix!r} in {path!r}")
    return entities, suffix


def _parse_path(path: str) -> _BidsPath:
    normalized = _normalize_path(path)
    directory, _, filename = normalized.rpartition("/")
    if not filename:
        raise ArgumentError(f"BIDS path has no filename: {path!r}")
    is_derivative_description = (
        filename == "dataset_description.json" and normalized.startswith("derivatives/")
    )
    is_special = normalized in _SPECIAL_ROOT_FILES or is_derivative_description or filename in {"README", "CHANGES"}
    stem, extension = _split_extension(filename)
    is_json = extension == ".json"
    if is_special:
        return _BidsPath(normalized, directory, filename, stem, extension, {}, stem, is_json, False, True)
    entities, suffix = _parse_entities(stem, path=normalized)
    return _BidsPath(
        normalized,
        directory,
        filename,
        stem,
        extension,
        entities,
        suffix,
        is_json,
        extension in _KNOWN_DATA_EXTENSIONS,
        False,
    )


def _directory_entities(directory: str) -> dict[str, str]:
    result: dict[str, str] = {}
    for segment in directory.split("/") if directory else ():
        if "-" not in segment:
            continue
        key, value = segment.split("-", 1)
        if key in _KNOWN_ENTITY_KEYS and _ENTITY_VALUE.fullmatch(value):
            result[key] = value
    return result


def _is_ancestor(candidate_directory: str, data_directory: str) -> bool:
    if candidate_directory == data_directory:
        return True
    if not candidate_directory:
        return True
    return data_directory.startswith(candidate_directory + "/")


def _metadata_candidates(data: _BidsPath, sidecars: Sequence[_BidsPath]) -> list[_BidsPath]:
    candidates: list[tuple[int, int, str, _BidsPath]] = []
    for sidecar in sidecars:
        if sidecar.suffix != data.suffix or not _is_ancestor(sidecar.directory, data.directory):
            continue
        if any(data.entities.get(key) != value for key, value in sidecar.entities.items()):
            continue
        specificity = len(sidecar.entities)
        depth = sidecar.directory.count("/")
        candidates.append((specificity, depth, sidecar.path, sidecar))
    candidates.sort(key=lambda item: (item[0], item[1], item[2]))
    return [item[3] for item in candidates]


def _validate_metadata_value(path: str, metadata: Mapping[str, Any], audit: _Audit) -> None:
    try:
        encoded = canonical_json(dict(metadata)).encode("utf-8")
    except Exception as error:  # noqa: BLE001 - convert any non-canonical value into a finding
        audit.add("metadata_not_json", "error", path, f"metadata is not canonical JSON-safe: {error}")
        return
    if len(encoded) > MAX_BIDS_METADATA_BYTES:
        audit.add(
            "metadata_too_large",
            "error",
            path,
            f"parsed metadata exceeds the {MAX_BIDS_METADATA_BYTES}-byte audit limit",
        )


def _participants_audit(
    text: str,
    *,
    source_id: str,
    subjects: set[str],
    audit: _Audit,
) -> dict[str, Any]:
    _text("participants_tsv", text, MAX_BIDS_METADATA_BYTES, allow_tsv_controls=True)
    try:
        rows = list(csv.DictReader(io.StringIO(text), delimiter="\t"))
    except csv.Error as error:
        audit.add("participants_malformed", "error", "participants.tsv", str(error))
        return {"rows": 0, "participants": [], "missing_subjects": sorted(subjects)}
    if not rows or "participant_id" not in (rows[0].keys() if rows else {}):
        audit.add("participants_id_missing", "error", "participants.tsv", "participant_id column is required")
        return {"rows": len(rows), "participants": [], "missing_subjects": sorted(subjects)}
    participants: list[str] = []
    for index, row in enumerate(rows, start=2):
        participant = (row.get("participant_id") or "").strip()
        if not participant.startswith("sub-") or not _ENTITY_VALUE.fullmatch(participant[4:]):
            audit.add(
                "participant_id_invalid",
                "error",
                f"participants.tsv#row={index}",
                "participant_id must be a sub- entity label",
            )
            continue
        if participant in participants:
            audit.add("participant_duplicate", "error", "participants.tsv", f"duplicate participant {participant!r}")
        participants.append(participant)
    listed = set(participants)
    missing = sorted(subjects - listed)
    unobserved = sorted(listed - subjects)
    for subject in missing:
        audit.add("participant_missing", "error", "participants.tsv", f"dataset subject {subject!r} has no participant row")
    for subject in unobserved:
        audit.add("participant_unobserved", "warning", "participants.tsv", f"participant row {subject!r} has no manifest data file")
    return {
        "rows": len(rows),
        "participants": sorted(listed),
        "missing_subjects": missing,
        "unobserved_participants": unobserved,
    }


def audit_bids(
    files: Sequence[str],
    *,
    source_id: str,
    metadata: Mapping[str, Mapping[str, Any]] | None = None,
    participants_tsv: str | None = None,
    max_files: int = MAX_BIDS_FILES,
    max_items: int = MAX_BIDS_ITEMS,
) -> BidsAuditResult:
    """Audit a caller-owned BIDS manifest and parsed sidecar projections.

    The validator checks the entire manifest before applying ``max_items`` disclosure limits. A
    valid result therefore means the supplied manifest is internally consistent under these
    checks; it does not mean that the caller's file inventory is complete or that any binary image
    bytes satisfy NIfTI, DICOM, MEG, EEG, or microscopy conformance.
    """

    _text("source_id", source_id)
    max_files = _limit("max_files", max_files, MAX_BIDS_FILES)
    max_items = _limit("max_items", max_items, MAX_BIDS_ITEMS)
    if isinstance(files, (str, bytes)):
        raise ArgumentError("files must be a sequence of relative paths")
    if len(files) == 0 or len(files) > max_files:
        raise ArgumentError(f"files must contain between 1 and {max_files} paths")
    normalized_files = tuple(sorted({_normalize_path(path) for path in files}))
    if len(normalized_files) != len(files):
        raise ArgumentError("files must contain unique normalized paths")
    if metadata is not None and not isinstance(metadata, Mapping):
        raise ArgumentError("metadata must be a mapping from sidecar path to parsed JSON object")
    normalized_metadata: dict[str, Mapping[str, Any]] = {}
    for path, value in (metadata or {}).items():
        normalized = _normalize_path(path)
        if normalized in normalized_metadata:
            raise ArgumentError(f"metadata contains duplicate normalized path {normalized!r}")
        if not isinstance(value, Mapping):
            raise ArgumentError(f"metadata for {normalized!r} must be a JSON object")
        normalized_metadata[normalized] = dict(value)

    audit = _Audit(max_items)
    parsed: list[_BidsPath] = []
    for path in normalized_files:
        try:
            parsed.append(_parse_path(path))
        except ArgumentError as error:
            audit.add("filename_invalid", "error", path, str(error))
    by_path = {item.path: item for item in parsed}
    sidecars = [item for item in parsed if item.is_json and not item.is_special]
    data_files = [item for item in parsed if item.is_data]
    subjects: set[str] = set()
    sessions: set[str] = set()
    canonical_keys: dict[tuple[Any, ...], str] = {}
    inventory: list[dict[str, Any]] = []

    for item in parsed:
        directory_entities = _directory_entities(item.directory)
        for key, value in directory_entities.items():
            if key in item.entities and item.entities[key] != value:
                audit.add(
                    "directory_entity_mismatch",
                    "error",
                    item.path,
                    f"directory entity {key}={value!r} disagrees with filename value {item.entities[key]!r}",
                )
        if item.is_data:
            subject = item.entities.get("sub")
            if subject is None:
                audit.add("subject_missing", "error", item.path, "data files must carry a sub- entity")
            else:
                subjects.add(f"sub-{subject}")
            if "ses" in item.entities:
                sessions.add(f"ses-{item.entities['ses']}")
            if item.extension not in _KNOWN_DATA_EXTENSIONS:
                audit.add("extension_unknown", "warning", item.path, f"extension {item.extension!r} is not in the bounded BIDS extension set")
            key = (
                item.directory,
                item.entities.get("sub"),
                item.entities.get("ses"),
                item.entities.get("task"),
                item.entities.get("acq"),
                item.entities.get("run"),
                item.entities.get("echo"),
                item.entities.get("space"),
                item.entities.get("desc"),
                item.suffix,
                item.extension,
            )
            if key in canonical_keys:
                audit.add(
                    "data_duplicate",
                    "error",
                    item.path,
                    "another file has the same bounded BIDS identity",
                    (canonical_keys[key],),
                )
            else:
                canonical_keys[key] = item.path
        inventory.append(
            {
                "path": item.path,
                "directory": item.directory,
                "extension": item.extension,
                "suffix": item.suffix,
                "entities": dict(sorted(item.entities.items())),
                "kind": "data" if item.is_data else "sidecar" if item.is_json else "special_or_other",
            }
        )

    for path, value in normalized_metadata.items():
        _validate_metadata_value(path, value, audit)
        if path not in by_path:
            audit.add("metadata_orphan", "error", path, "parsed metadata has no corresponding manifest file")
    if "dataset_description.json" not in by_path:
        audit.add("dataset_description_missing", "error", "dataset_description.json", "BIDS requires a root dataset_description.json")
    dataset_description = normalized_metadata.get("dataset_description.json")
    if dataset_description is not None:
        for key in ("Name", "BIDSVersion"):
            if not isinstance(dataset_description.get(key), str) or not dataset_description[key].strip():
                audit.add("dataset_description_field_missing", "error", "dataset_description.json", f"required field {key!r} is missing or not a non-empty string")
    for item in parsed:
        if item.is_special and item.path == "dataset_description.json" and item.path not in normalized_metadata:
            audit.add("metadata_unavailable", "warning", item.path, "manifest names the dataset description but no parsed JSON projection was supplied")

    resolved: list[dict[str, Any]] = []
    for data in data_files:
        candidates = _metadata_candidates(data, sidecars)
        merged: dict[str, Any] = {}
        applied: list[str] = []
        key_sources: dict[str, str] = {}
        key_ranks: dict[str, tuple[int, int]] = {}
        for sidecar in candidates:
            if sidecar.path not in normalized_metadata:
                audit.add("metadata_unavailable", "warning", sidecar.path, "sidecar is in the manifest but its parsed JSON projection was not supplied")
                continue
            applied.append(sidecar.path)
            for key, value in normalized_metadata[sidecar.path].items():
                rank = (len(sidecar.entities), sidecar.directory.count("/"))
                if key in merged and merged[key] != value and key_ranks[key] == rank:
                    audit.add("metadata_conflict", "error", data.path, f"metadata key {key!r} has conflicting values at equal specificity", (key_sources[key], sidecar.path))
                merged[key] = value
                key_sources[key] = sidecar.path
                key_ranks[key] = rank
        if data.suffix in {"bold", "eeg", "meg", "ieeg"}:
            task = data.entities.get("task")
            if task is not None and "TaskName" not in merged:
                audit.add("task_name_missing", "error", data.path, "task-bearing functional data requires inherited TaskName metadata")
            elif task is not None and merged.get("TaskName") != task:
                audit.add("task_name_mismatch", "error", data.path, f"TaskName {merged.get('TaskName')!r} disagrees with task-{task}")
        if data.extension in {".nii", ".nii.gz"} and not applied:
            audit.add("sidecar_missing", "warning", data.path, "no applicable JSON sidecar was resolved; binary image headers were not inspected")
        resolved.append(
            {
                "data_path": data.path,
                "sidecars": applied,
                "metadata_keys": sorted(merged),
                "metadata": merged if len(resolved) < max_items else None,
            }
        )

    participants = None
    if participants_tsv is not None:
        participants = _participants_audit(participants_tsv, source_id=source_id, subjects=subjects, audit=audit)
    elif "participants.tsv" in by_path:
        audit.add("participants_unavailable", "warning", "participants.tsv", "manifest names participants.tsv but no parsed TSV projection was supplied")

    derivatives: dict[str, list[str]] = {}
    for item in parsed:
        if item.path.startswith("derivatives/"):
            parts = item.path.split("/")
            if len(parts) >= 2:
                derivatives.setdefault(parts[1], []).append(item.path)
    for pipeline, paths in sorted(derivatives.items()):
        description_path = f"derivatives/{pipeline}/dataset_description.json"
        if description_path not in by_path:
            audit.add("derivative_description_missing", "error", description_path, f"derivative pipeline {pipeline!r} has no dataset_description.json", paths[:3])

    errors = audit.error_count
    warnings = audit.warning_count
    manifest_input = {
        "source_id": source_id,
        "files": normalized_files,
        "metadata": normalized_metadata,
        "participants_tsv": participants_tsv,
    }
    source_digest = content_digest(manifest_input)
    document: dict[str, Any] = {
        "schema": BIDS_SCHEMA,
        "workflow": "bids_manifest_audit",
        "valid": errors == 0,
        "source_id": source_id,
        "manifest": {
            "source_id": source_id,
            "source_digest": source_digest,
            "adapter": BIDS_ADAPTER,
            "adapter_version": BIDS_ADAPTER_VERSION,
            "declared_format": "application/bids-manifest",
            "file_count": len(parsed),
            "data_file_count": len(data_files),
            "sidecar_count": len(sidecars),
            "subject_count": len(subjects),
            "session_count": len(sessions),
            "bytes_read": False,
        },
        "summary": {
            "files": len(parsed),
            "data_files": len(data_files),
            "sidecars": len(sidecars),
            "subjects": sorted(subjects),
            "sessions": sorted(sessions),
            "errors": errors,
            "warnings": warnings,
            "finding_count": audit.total,
        },
        "files": inventory[:max_items],
        "omitted_files": max(0, len(inventory) - max_items),
        "resolved_metadata": resolved[:max_items],
        "omitted_resolved_metadata": max(0, len(resolved) - max_items),
        "participants": participants,
        "findings": [finding.to_dict() for finding in audit.findings],
        "omitted_findings": max(0, audit.total - len(audit.findings)),
        "conformance": {
            "passed": errors == 0,
            "checks": {
                "relative_paths": "pass",
                "entity_syntax": "pass" if not audit.has("filename_invalid") else "fail",
                "sidecar_inheritance": "pass" if not audit.has("metadata_conflict") and not audit.has("metadata_orphan") else "fail",
                "participant_coverage": "pass" if not any(code.startswith("participant_") for code in audit.codes) else "fail",
                "dataset_description": "pass" if not any(code.startswith("dataset_description_") for code in audit.codes) else "fail",
            },
            "limitations": [
                "the validator audits a caller-supplied manifest and parsed projections; it does not access filesystem bytes",
                "it does not parse NIfTI, DICOM, EEG, MEG, iEEG, microscopy, archives, or derivative binary content",
                "a valid report proves only the bounded checks represented here and never proves inventory completeness or scientific validity",
            ],
        },
        "max_files": max_files,
        "max_items": max_items,
    }
    document["document_digest"] = content_digest(document)
    return BidsAuditResult(document)


__all__ = [
    "BIDS_ADAPTER",
    "BIDS_ADAPTER_VERSION",
    "BIDS_SCHEMA",
    "BidsAdapter",
    "BidsAuditResult",
    "BidsFinding",
    "MAX_BIDS_FILES",
    "MAX_BIDS_ITEMS",
    "audit_bids",
]
