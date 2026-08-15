"""Bounded DICOM metadata, hierarchy, geometry, and provenance auditing.

This module deliberately consumes parsed DICOM projections rather than raw bytes. A caller can
use ``pydicom`` (or another trusted reader) to produce the projection, then ask this dependency-
free layer to check study/series identity, frame geometry, dimensions, and disclosure boundaries.
It never decodes pixels, decompresses transfer syntaxes, or emits patient identifiers from arbitrary
input tags.
"""

from __future__ import annotations

from dataclasses import dataclass
import hashlib
import math
import re
from typing import Any, Mapping, Sequence

from .authoring import canonical_json, content_digest
from .errors import ArgumentError


DICOM_SCHEMA = "bioprism-python-dicom/0.1"
DICOM_ADAPTER = "bioprism.python.dicom_metadata"
DICOM_ADAPTER_VERSION = "0.1.0"
MAX_DICOM_INSTANCES = 100_000
MAX_DICOM_ITEMS = 1_000
MAX_DICOM_PROVENANCE_BYTES = 1_000_000
GEOMETRY_TOLERANCE = 1e-4
_UID = re.compile(r"^[0-9]+(?:\.[0-9]+)*$")
_MODALITY = re.compile(r"^[A-Z0-9]{2,16}$")


@dataclass(frozen=True)
class DicomFinding:
    code: str
    severity: str
    path: str
    detail: str
    related_paths: tuple[str, ...] = ()

    def __post_init__(self) -> None:
        if self.severity not in {"error", "warning", "info"}:
            raise ArgumentError(f"invalid DICOM finding severity: {self.severity!r}")

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
class DicomAuditResult:
    """A digest-bound DICOM projection audit with bounded disclosure."""

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
class _Instance:
    instance_id: str
    study_uid: str | None
    series_uid: str | None
    sop_instance_uid: str | None
    sop_class_uid: str | None
    frame_of_reference_uid: str | None
    transfer_syntax_uid: str | None
    modality: str | None
    rows: int | None
    columns: int | None
    number_of_frames: int
    instance_number: int | None
    pixel_spacing: tuple[float, float] | None
    orientation: tuple[float, float, float, float, float, float] | None
    position: tuple[float, float, float] | None
    frame_positions: tuple[tuple[float, float, float], ...] | None
    spacing_between_slices: float | None
    tags_present: bool


class _Audit:
    def __init__(self, limit: int) -> None:
        self.limit = limit
        self.findings: list[DicomFinding] = []
        self.total = 0
        self.error_count = 0
        self.warning_count = 0
        self.codes: set[str] = set()
        self.losses: list[dict[str, Any]] = []
        self.loss_total = 0
        self.loss_kinds: set[str] = set()
        self.blocking_loss_count = 0
        self.max_loss_severity: str | None = None

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
        if len(self.findings) < self.limit:
            self.findings.append(DicomFinding(code, severity, path, detail, tuple(related_paths)))

    def loss(
        self,
        kind: str,
        severity: str,
        path: str,
        detail: str,
        related_paths: Sequence[str] = (),
    ) -> None:
        self.loss_total += 1
        self.loss_kinds.add(kind)
        if severity == "blocking":
            self.blocking_loss_count += 1
        severity_rank = {"minor": 1, "major": 2, "blocking": 3}
        if self.max_loss_severity is None or severity_rank[severity] > severity_rank[self.max_loss_severity]:
            self.max_loss_severity = severity
        if len(self.losses) < self.limit:
            entry: dict[str, Any] = {
                "kind": kind,
                "severity": severity,
                "path": path,
                "detail": detail,
            }
            if related_paths:
                entry["related_paths"] = list(related_paths)
            self.losses.append(entry)


class DicomAdapter:
    """Concrete dependency-free parsed-projection adapter for the registry route."""

    name = DICOM_ADAPTER
    version = DICOM_ADAPTER_VERSION
    accepted_formats = ("application/dicom-manifest",)
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
            "scope_dimensions": ["subject", "specimen", "acquisition", "image"],
            "execution": "python_delegated",
            "optional_dependency": None,
        }

    def audit(
        self,
        instances: Sequence[Mapping[str, Any]],
        *,
        source_id: str,
        provenance: Mapping[str, Any] | None = None,
        max_instances: int = MAX_DICOM_INSTANCES,
        max_items: int = MAX_DICOM_ITEMS,
    ) -> DicomAuditResult:
        return audit_dicom(
            instances,
            source_id=source_id,
            provenance=provenance,
            max_instances=max_instances,
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


def _uid(value: Any, *, path: str, field: str, audit: _Audit, required: bool) -> str | None:
    if value is None or value == "":
        if required:
            audit.add("tag_missing", "error", path, f"required DICOM tag {field!r} is missing")
        return None
    if not isinstance(value, str) or len(value) > 64 or not _UID.fullmatch(value):
        audit.add("uid_invalid", "error", path, f"{field} must be a dotted numeric DICOM UID")
        return None
    if any(component != "0" and component.startswith("0") for component in value.split(".")):
        audit.add("uid_invalid", "error", path, f"{field} contains a leading-zero UID component")
        return None
    return value


def _modality(value: Any, *, path: str, audit: _Audit) -> str | None:
    if value is None or value == "":
        audit.add("tag_missing", "error", path, "required DICOM Modality is missing")
        return None
    if not isinstance(value, str) or not _MODALITY.fullmatch(value):
        audit.add("modality_invalid", "error", path, "Modality must be an uppercase DICOM modality code")
        return None
    return value


def _positive_int(value: Any, *, path: str, field: str, audit: _Audit, required: bool = False) -> int | None:
    if value is None:
        if required:
            audit.add("tag_missing", "error", path, f"required DICOM tag {field!r} is missing")
        return None
    if isinstance(value, bool) or not isinstance(value, int) or value <= 0:
        audit.add("integer_invalid", "error", path, f"{field} must be a positive integer")
        return None
    return value


def _finite_number(value: Any, *, path: str, field: str, audit: _Audit) -> float | None:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        audit.add("number_invalid", "error", path, f"{field} must be a finite number")
        return None
    converted = float(value)
    if not math.isfinite(converted):
        audit.add("number_invalid", "error", path, f"{field} must be a finite number")
        return None
    return converted


def _vector(
    value: Any,
    *,
    path: str,
    field: str,
    size: int,
    audit: _Audit,
    positive: bool = False,
) -> tuple[float, ...] | None:
    if value is None:
        return None
    if isinstance(value, (str, bytes)) or not isinstance(value, Sequence) or len(value) != size:
        audit.add("vector_invalid", "error", path, f"{field} must contain exactly {size} numeric values")
        return None
    values: list[float] = []
    for index, item in enumerate(value):
        number = _finite_number(item, path=path, field=f"{field}[{index}]", audit=audit)
        if number is None:
            return None
        if positive and number <= 0:
            audit.add("vector_invalid", "error", path, f"{field} values must be positive")
            return None
        values.append(number)
    return tuple(values)


def _frame_positions(
    value: Any,
    *,
    path: str,
    expected: int,
    audit: _Audit,
) -> tuple[tuple[float, float, float], ...] | None:
    if value is None:
        return None
    if isinstance(value, (str, bytes)) or not isinstance(value, Sequence) or len(value) != expected:
        audit.add("frame_geometry_invalid", "error", path, "per_frame_positions must match NumberOfFrames")
        return None
    result: list[tuple[float, float, float]] = []
    for index, position in enumerate(value):
        parsed = _vector(position, path=path, field=f"per_frame_positions[{index}]", size=3, audit=audit)
        if parsed is None:
            return None
        result.append((parsed[0], parsed[1], parsed[2]))
    return tuple(result)


def _parse_instance(mapping: Mapping[str, Any], index: int, audit: _Audit) -> _Instance:
    path = f"instances[{index}]"
    raw_id = mapping.get("instance_id", mapping.get("path"))
    if not isinstance(raw_id, str) or not raw_id.strip():
        audit.add("instance_id_missing", "error", path, "each projection requires a non-empty instance_id or path")
        instance_id = path
    else:
        try:
            _text("instance_id", raw_id, 2_048)
            instance_id = raw_id
        except ArgumentError as error:
            audit.add("instance_id_invalid", "error", path, str(error))
            instance_id = path

    study_uid = _uid(mapping.get("study_uid"), path=instance_id, field="StudyInstanceUID", audit=audit, required=True)
    series_uid = _uid(mapping.get("series_uid"), path=instance_id, field="SeriesInstanceUID", audit=audit, required=True)
    sop_instance_uid = _uid(mapping.get("sop_instance_uid"), path=instance_id, field="SOPInstanceUID", audit=audit, required=True)
    sop_class_uid = _uid(mapping.get("sop_class_uid"), path=instance_id, field="SOPClassUID", audit=audit, required=True)
    frame_uid = _uid(mapping.get("frame_of_reference_uid"), path=instance_id, field="FrameOfReferenceUID", audit=audit, required=False)
    transfer_uid = _uid(mapping.get("transfer_syntax_uid"), path=instance_id, field="TransferSyntaxUID", audit=audit, required=False)
    modality = _modality(mapping.get("modality"), path=instance_id, audit=audit)

    rows = _positive_int(mapping.get("rows"), path=instance_id, field="Rows", audit=audit)
    columns = _positive_int(mapping.get("columns"), path=instance_id, field="Columns", audit=audit)
    if (rows is None) != (columns is None):
        audit.add("dimensions_incomplete", "error", instance_id, "Rows and Columns must be supplied together")
    frames = _positive_int(mapping.get("number_of_frames"), path=instance_id, field="NumberOfFrames", audit=audit) or 1
    instance_number = _positive_int(mapping.get("instance_number"), path=instance_id, field="InstanceNumber", audit=audit)
    spacing = _vector(mapping.get("pixel_spacing"), path=instance_id, field="PixelSpacing", size=2, audit=audit, positive=True)
    orientation_raw = _vector(mapping.get("image_orientation_patient"), path=instance_id, field="ImageOrientationPatient", size=6, audit=audit)
    orientation = None if orientation_raw is None else tuple(orientation_raw)  # type: ignore[assignment]
    position_raw = _vector(mapping.get("image_position_patient"), path=instance_id, field="ImagePositionPatient", size=3, audit=audit)
    position = None if position_raw is None else tuple(position_raw)  # type: ignore[assignment]
    frame_positions = _frame_positions(mapping.get("per_frame_positions"), path=instance_id, expected=frames, audit=audit)
    raw_between = mapping.get("spacing_between_slices")
    between = _finite_number(raw_between, path=instance_id, field="SpacingBetweenSlices", audit=audit) if raw_between is not None else None
    if between is not None and between == 0:
        audit.add("number_invalid", "error", instance_id, "SpacingBetweenSlices must not be zero")
    tags = mapping.get("tags")
    tags_present = tags is not None
    if tags is not None and not isinstance(tags, Mapping):
        audit.add("tags_invalid", "error", instance_id, "tags must be a mapping when supplied")
    elif isinstance(tags, Mapping) and len(tags) > MAX_DICOM_ITEMS:
        audit.add("tags_too_many", "error", instance_id, f"parsed tags exceed the {MAX_DICOM_ITEMS}-item audit limit")

    return _Instance(
        instance_id,
        study_uid,
        series_uid,
        sop_instance_uid,
        sop_class_uid,
        frame_uid,
        transfer_uid,
        modality,
        rows,
        columns,
        frames,
        instance_number,
        None if spacing is None else (spacing[0], spacing[1]),
        orientation,
        position,
        frame_positions,
        between,
        tags_present,
    )


def _dot(left: Sequence[float], right: Sequence[float]) -> float:
    return sum(a * b for a, b in zip(left, right))


def _cross(left: Sequence[float], right: Sequence[float]) -> tuple[float, float, float]:
    return (
        left[1] * right[2] - left[2] * right[1],
        left[2] * right[0] - left[0] * right[2],
        left[0] * right[1] - left[1] * right[0],
    )


def _norm(vector: Sequence[float]) -> float:
    return math.sqrt(_dot(vector, vector))


def _close(left: Sequence[float], right: Sequence[float], tolerance: float = GEOMETRY_TOLERANCE) -> bool:
    return len(left) == len(right) and all(abs(a - b) <= tolerance for a, b in zip(left, right))


def _uid_digest(source_id: str, uid: str) -> str:
    return hashlib.sha256(f"{source_id}\0{uid}".encode("utf-8")).hexdigest()[:24]


def _validate_provenance(provenance: Mapping[str, Any] | None, audit: _Audit) -> str | None:
    if not provenance:
        audit.loss("provenance_unavailable", "blocking", "provenance", "no non-empty provenance projection was supplied")
        return None
    try:
        encoded = canonical_json(dict(provenance)).encode("utf-8")
    except Exception as error:  # noqa: BLE001 - convert projection failures into findings
        audit.add("provenance_not_json", "error", "provenance", f"provenance is not canonical JSON-safe: {error}")
        audit.loss("provenance_unavailable", "blocking", "provenance", "provenance could not be represented canonically")
        return None
    if len(encoded) > MAX_DICOM_PROVENANCE_BYTES:
        audit.add("provenance_too_large", "error", "provenance", f"provenance exceeds {MAX_DICOM_PROVENANCE_BYTES} bytes")
        audit.loss("provenance_unavailable", "blocking", "provenance", "provenance exceeds the bounded audit limit")
        return None
    return content_digest(dict(provenance))


def _geometry_for_series(members: Sequence[_Instance], audit: _Audit) -> dict[str, Any]:
    series_path = members[0].series_uid or members[0].instance_id
    if any(member.frame_of_reference_uid is None for member in members):
        audit.loss("coordinate_frame_not_carried", "blocking", series_path, "at least one instance lacks FrameOfReferenceUID")
    frame_uids = {member.frame_of_reference_uid for member in members if member.frame_of_reference_uid is not None}
    if len(frame_uids) > 1:
        first = next(member for member in members if member.frame_of_reference_uid is not None)
        for member in members:
            if member.frame_of_reference_uid not in {None, first.frame_of_reference_uid}:
                audit.add("frame_of_reference_mismatch", "error", member.instance_id, "instances in one series disagree on FrameOfReferenceUID", (first.instance_id,))

    dimensions = [(member.rows, member.columns) for member in members if member.rows is not None and member.columns is not None]
    if dimensions and any(item != dimensions[0] for item in dimensions[1:]):
        audit.add("dimensions_inconsistent", "error", series_path, "Rows/Columns differ within a DICOM series")
    pixel_spacings = [member.pixel_spacing for member in members if member.pixel_spacing is not None]
    if pixel_spacings and any(not _close(item, pixel_spacings[0]) for item in pixel_spacings[1:]):
        audit.add("pixel_spacing_inconsistent", "error", series_path, "PixelSpacing differs within a DICOM series")

    orientations = [member.orientation for member in members if member.orientation is not None]
    positions = [member.position for member in members if member.position is not None]
    if len(orientations) != len(members) or len(positions) != len(members):
        missing = [member.instance_id for member in members if member.orientation is None or member.position is None]
        audit.loss("coordinate_frame_not_carried", "blocking", series_path, "ImageOrientationPatient and ImagePositionPatient are incomplete", missing[:3])
    normal: tuple[float, float, float] | None = None
    if orientations:
        for member in members:
            if member.orientation is None:
                continue
            row = member.orientation[:3]
            column = member.orientation[3:]
            row_norm = _norm(row)
            column_norm = _norm(column)
            if abs(row_norm - 1.0) > GEOMETRY_TOLERANCE or abs(column_norm - 1.0) > GEOMETRY_TOLERANCE or abs(_dot(row, column)) > GEOMETRY_TOLERANCE:
                audit.add("orientation_invalid", "error", member.instance_id, "ImageOrientationPatient row/column vectors must be orthonormal")
            candidate = _cross(row, column)
            candidate_norm = _norm(candidate)
            if candidate_norm <= GEOMETRY_TOLERANCE:
                audit.add("orientation_degenerate", "error", member.instance_id, "ImageOrientationPatient vectors do not define a plane")
            elif normal is None:
                normal = tuple(component / candidate_norm for component in candidate)
            elif not _close(candidate, tuple(component * candidate_norm for component in normal), 1e-3):
                audit.add("orientation_inconsistent", "error", member.instance_id, "slice orientation differs within a DICOM series")

    projected_positions: list[tuple[float, str]] = []
    if normal is not None:
        for member in members:
            if member.position is not None:
                projected_positions.append((_dot(member.position, normal), member.instance_id))
    projected_positions.sort()
    spacings: list[float] = []
    for (left, left_id), (right, right_id) in zip(projected_positions, projected_positions[1:]):
        difference = right - left
        if abs(difference) <= GEOMETRY_TOLERANCE:
            audit.add("slice_position_duplicate", "error", right_id, "two instances occupy the same projected slice position", (left_id,))
        else:
            spacings.append(abs(difference))
    if spacings and max(spacings) - min(spacings) > max(GEOMETRY_TOLERANCE, max(spacings) * 0.01):
        audit.add("slice_spacing_inconsistent", "warning", series_path, "projected slice spacing varies by more than one percent")

    frame_counts = {member.number_of_frames for member in members}
    for member in members:
        if member.number_of_frames > 1 and member.frame_positions is None:
            audit.loss("coordinate_frame_not_carried", "blocking", member.instance_id, "multi-frame instance lacks per-frame positions")
    return {
        "dimensions": list(dimensions[0]) if dimensions and all(item == dimensions[0] for item in dimensions) else None,
        "pixel_spacing": list(pixel_spacings[0]) if pixel_spacings and all(_close(item, pixel_spacings[0]) for item in pixel_spacings) else None,
        "orientation_present": len(orientations) == len(members),
        "position_present": len(positions) == len(members),
        "frame_of_reference_present": len(frame_uids) == 1 and all(member.frame_of_reference_uid is not None for member in members),
        "slice_spacing": (sum(spacings) / len(spacings)) if spacings else None,
        "frame_counts": sorted(frame_counts),
    }


def audit_dicom(
    instances: Sequence[Mapping[str, Any]],
    *,
    source_id: str,
    provenance: Mapping[str, Any] | None = None,
    max_instances: int = MAX_DICOM_INSTANCES,
    max_items: int = MAX_DICOM_ITEMS,
) -> DicomAuditResult:
    """Audit parsed DICOM instance projections without reading pixel bytes.

    Required projection keys are ``instance_id`` (or ``path``), ``study_uid``, ``series_uid``,
    ``sop_instance_uid``, ``sop_class_uid``, and ``modality``. Geometry keys use DICOM names in
    snake case: ``frame_of_reference_uid``, ``rows``, ``columns``, ``pixel_spacing``,
    ``image_orientation_patient``, ``image_position_patient``, and, for enhanced objects,
    ``number_of_frames`` plus ``per_frame_positions``. The report separates structural validity
    from publishability: missing provenance or coordinate geometry is a blocking semantic loss
    even when the supplied metadata is otherwise internally valid.
    """

    _text("source_id", source_id)
    max_instances = _limit("max_instances", max_instances, MAX_DICOM_INSTANCES)
    max_items = _limit("max_items", max_items, MAX_DICOM_ITEMS)
    if isinstance(instances, (str, bytes)) or not isinstance(instances, Sequence):
        raise ArgumentError("instances must be a sequence of parsed DICOM mappings")
    if len(instances) == 0 or len(instances) > max_instances:
        raise ArgumentError(f"instances must contain between 1 and {max_instances} projections")
    if provenance is not None and not isinstance(provenance, Mapping):
        raise ArgumentError("provenance must be a JSON-object mapping when supplied")

    audit = _Audit(max_items)
    audit.loss("content_uninterpreted", "minor", source_id, "DICOM pixel bytes and transfer-syntax payloads were not decoded")
    provenance_digest = _validate_provenance(provenance, audit)
    parsed: list[_Instance] = []
    for index, mapping in enumerate(instances):
        if not isinstance(mapping, Mapping):
            audit.add("instance_not_mapping", "error", f"instances[{index}]", "each projection must be a JSON object")
            continue
        parsed.append(_parse_instance(mapping, index, audit))

    ids: dict[str, str] = {}
    sop_ids: dict[str, str] = {}
    series_studies: dict[str, str] = {}
    series_members: dict[str, list[_Instance]] = {}
    study_series: dict[str, set[str]] = {}
    for member in parsed:
        if member.instance_id in ids:
            audit.add("instance_id_duplicate", "error", member.instance_id, "instance_id occurs more than once", (ids[member.instance_id],))
        else:
            ids[member.instance_id] = member.instance_id
        if member.sop_instance_uid is not None:
            if member.sop_instance_uid in sop_ids:
                audit.add("sop_instance_duplicate", "error", member.instance_id, "SOPInstanceUID occurs more than once", (sop_ids[member.sop_instance_uid],))
            else:
                sop_ids[member.sop_instance_uid] = member.instance_id
        if member.series_uid is not None:
            series_members.setdefault(member.series_uid, []).append(member)
            if member.study_uid is not None:
                study_series.setdefault(member.study_uid, set()).add(member.series_uid)
                previous_study = series_studies.get(member.series_uid)
                if previous_study is not None and previous_study != member.study_uid:
                    audit.add("series_cross_study", "error", member.instance_id, "one SeriesInstanceUID is associated with multiple studies")
                else:
                    series_studies[member.series_uid] = member.study_uid

    series_rows: list[dict[str, Any]] = []
    for series_uid, members in sorted(series_members.items()):
        geometry = _geometry_for_series(members, audit)
        modalities = sorted({member.modality for member in members if member.modality is not None})
        if len(modalities) > 1:
            audit.add("modality_inconsistent", "error", members[0].instance_id, "Modality differs within a DICOM series")
        series_rows.append(
            {
                "series_uid_digest": _uid_digest(source_id, series_uid),
                "study_uid_digest": _uid_digest(source_id, series_studies[series_uid]) if series_uid in series_studies else None,
                "instance_count": len(members),
                "modalities": modalities,
                "geometry": geometry,
            }
        )

    modality_counts: dict[str, int] = {}
    instance_rows: list[dict[str, Any]] = []
    unknown_tag_instances = 0
    for member in parsed:
        if member.modality is not None:
            modality_counts[member.modality] = modality_counts.get(member.modality, 0) + 1
        if member.tags_present:
            unknown_tag_instances += 1
        geometry_present = member.frame_of_reference_uid is not None and member.orientation is not None and member.position is not None
        if not geometry_present:
            audit.loss("coordinate_frame_not_carried", "blocking", member.instance_id, "instance projection is missing frame or image geometry")
        instance_rows.append(
            {
                "instance_id": member.instance_id,
                "study_uid_digest": _uid_digest(source_id, member.study_uid) if member.study_uid else None,
                "series_uid_digest": _uid_digest(source_id, member.series_uid) if member.series_uid else None,
                "sop_instance_uid_digest": _uid_digest(source_id, member.sop_instance_uid) if member.sop_instance_uid else None,
                "sop_class_uid_present": member.sop_class_uid is not None,
                "modality": member.modality,
                "dimensions": [member.rows, member.columns] if member.rows is not None and member.columns is not None else None,
                "number_of_frames": member.number_of_frames,
                "instance_number": member.instance_number,
                "geometry_present": geometry_present,
                "transfer_syntax_present": member.transfer_syntax_uid is not None,
            }
        )

    try:
        source_digest = content_digest({"source_id": source_id, "instances": [dict(item) for item in instances]})
    except Exception as error:  # noqa: BLE001 - retain a safe digest even for malformed projections
        audit.add("projection_not_json", "error", source_id, f"instance projections are not canonical JSON-safe: {error}")
        source_digest = content_digest({"source_id": source_id, "instance_ids": [row["instance_id"] for row in instance_rows]})

    errors = audit.error_count
    warnings = audit.warning_count
    blocking_losses = audit.blocking_loss_count
    blocking_loss_count = audit.blocking_loss_count
    valid = errors == 0
    publishable = valid and blocking_loss_count == 0
    studies = [
        {
            "study_uid_digest": _uid_digest(source_id, study_uid),
            "series_count": len(series_uids),
            "series_uid_digests": [_uid_digest(source_id, series_uid) for series_uid in sorted(series_uids)],
            "instance_count": sum(len(series_members.get(series_uid, ())) for series_uid in series_uids),
        }
        for study_uid, series_uids in sorted(study_series.items())
    ]
    document: dict[str, Any] = {
        "schema": DICOM_SCHEMA,
        "workflow": "dicom_projection_audit",
        "valid": valid,
        "publishable": publishable,
        "source_id": source_id,
        "manifest": {
            "source_id": source_id,
            "source_digest": source_digest,
            "adapter": DICOM_ADAPTER,
            "adapter_version": DICOM_ADAPTER_VERSION,
            "declared_format": "application/dicom-manifest",
            "instance_count": len(parsed),
            "study_count": len(study_series),
            "series_count": len(series_members),
            "modality_counts": dict(sorted(modality_counts.items())),
            "provenance_digest": provenance_digest,
            "bytes_read": False,
            "patient_identifiers_disclosed": False,
        },
        "summary": {
            "instances": len(parsed),
            "studies": len(study_series),
            "series": len(series_members),
            "modality_counts": dict(sorted(modality_counts.items())),
            "tag_projection_instances": unknown_tag_instances,
            "errors": errors,
            "warnings": warnings,
            "finding_count": audit.total,
            "blocking_loss_count": blocking_losses,
        },
        "studies": studies[:max_items],
        "omitted_studies": max(0, len(studies) - max_items),
        "series": series_rows[:max_items],
        "omitted_series": max(0, len(series_rows) - max_items),
        "instances": instance_rows[:max_items],
        "omitted_instances": max(0, len(instance_rows) - max_items),
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
                "identity_hierarchy": "pass" if not any(code in audit.codes for code in {"series_cross_study", "sop_instance_duplicate", "instance_id_duplicate"}) else "fail",
                "required_tags": "pass" if not any(code in audit.codes for code in {"tag_missing", "instance_not_mapping"}) else "fail",
                "dimensions": "pass" if not any(code in audit.codes for code in {"dimensions_incomplete", "dimensions_inconsistent"}) else "fail",
                "geometry": "pass" if not any(code in audit.codes for code in {"orientation_invalid", "orientation_degenerate", "orientation_inconsistent", "slice_position_duplicate", "frame_of_reference_mismatch"}) else "fail",
                "provenance": "pass" if provenance_digest is not None else "loss",
            },
            "limitations": [
                "the audit consumes caller-supplied parsed projections and does not access filesystem bytes",
                "pixel data, transfer-syntax decompression, overlays, private-tag semantics, and per-frame functional groups are not independently decoded",
                "a valid report proves only the bounded identity and geometry checks represented here; it is not a clinical or diagnostic interpretation",
            ],
        },
        "max_instances": max_instances,
        "max_items": max_items,
    }
    document["document_digest"] = content_digest(document)
    return DicomAuditResult(document)


__all__ = [
    "DICOM_ADAPTER",
    "DICOM_ADAPTER_VERSION",
    "DICOM_SCHEMA",
    "DicomAdapter",
    "DicomAuditResult",
    "DicomFinding",
    "MAX_DICOM_INSTANCES",
    "MAX_DICOM_ITEMS",
    "audit_dicom",
]
