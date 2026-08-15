"""Bounded NIfTI header and affine projection auditing.

The module accepts parsed header projections from an optional reader such as ``nibabel``. It
validates shape, datatype, affine form, qform/sform declarations, voxel-size agreement, and
coordinate-frame disclosures without opening files or touching image arrays. Raw NIfTI decoding
and BIDS sidecar loading remain separate responsibilities.
"""

from __future__ import annotations

from dataclasses import dataclass
import hashlib
import math
import re
from typing import Any, Mapping, Sequence

from .authoring import canonical_json, content_digest
from .errors import ArgumentError


NIFTI_SCHEMA = "bioprism-python-nifti/0.1"
NIFTI_ADAPTER = "bioprism.python.nifti_metadata"
NIFTI_ADAPTER_VERSION = "0.1.0"
MAX_NIFTI_IMAGES = 10_000
MAX_NIFTI_ITEMS = 1_000
MAX_NIFTI_VOXELS = 1_000_000_000_000
GEOMETRY_TOLERANCE = 1e-4
_DTYPE = re.compile(r"^(?:bool|u?int|float|complex)[0-9]+$")
_AXIS_CODES = {"R", "L", "A", "P", "S", "I"}
_SPACE_UNITS = {"meter", "mm", "micron", "unknown"}
_TIME_UNITS = {"sec", "msec", "usec", "hz", "ppm", "rads", "unknown"}


@dataclass(frozen=True)
class NiftiFinding:
    code: str
    severity: str
    path: str
    detail: str
    related_paths: tuple[str, ...] = ()

    def __post_init__(self) -> None:
        if self.severity not in {"error", "warning", "info"}:
            raise ArgumentError(f"invalid NIfTI finding severity: {self.severity!r}")

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
class NiftiAuditResult:
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
class _Image:
    image_id: str
    series_id: str
    shape: tuple[int, ...] | None
    dtype: str | None
    affine: tuple[tuple[float, ...], ...] | None
    qform: tuple[tuple[float, ...], ...] | None
    sform: tuple[tuple[float, ...], ...] | None
    qform_code: int
    sform_code: int
    voxel_sizes: tuple[float, float, float] | None
    axis_codes: tuple[str, str, str] | None
    coordinate_system: str | None
    reference_space: str | None
    space_units: str | None
    time_units: str | None
    intent: str | None


class _Audit:
    def __init__(self, limit: int) -> None:
        self.limit = limit
        self.findings: list[NiftiFinding] = []
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
            self.findings.append(NiftiFinding(code, severity, path, detail, tuple(related_paths)))

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


class NiftiAdapter:
    name = NIFTI_ADAPTER
    version = NIFTI_ADAPTER_VERSION
    accepted_formats = ("application/nifti-manifest",)
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
            "scope_dimensions": ["subject", "session", "acquisition", "image", "voxel"],
            "execution": "python_delegated",
            "optional_dependency": None,
        }

    def audit(
        self,
        images: Sequence[Mapping[str, Any]],
        *,
        source_id: str,
        provenance: Mapping[str, Any] | None = None,
        max_images: int = MAX_NIFTI_IMAGES,
        max_items: int = MAX_NIFTI_ITEMS,
    ) -> NiftiAuditResult:
        return audit_nifti(
            images,
            source_id=source_id,
            provenance=provenance,
            max_images=max_images,
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


def _number(value: Any, *, path: str, field: str, audit: _Audit) -> float | None:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        audit.add("number_invalid", "error", path, f"{field} must be a finite number")
        return None
    converted = float(value)
    if not math.isfinite(converted):
        audit.add("number_invalid", "error", path, f"{field} must be a finite number")
        return None
    return converted


def _matrix(value: Any, *, path: str, field: str, audit: _Audit) -> tuple[tuple[float, ...], ...] | None:
    if value is None:
        return None
    if isinstance(value, (str, bytes)) or not isinstance(value, Sequence) or len(value) != 4:
        audit.add("matrix_invalid", "error", path, f"{field} must be a 4x4 numeric matrix")
        return None
    rows: list[tuple[float, ...]] = []
    for row_index, row in enumerate(value):
        if isinstance(row, (str, bytes)) or not isinstance(row, Sequence) or len(row) != 4:
            audit.add("matrix_invalid", "error", path, f"{field} row {row_index} must contain four numbers")
            return None
        values: list[float] = []
        for column_index, item in enumerate(row):
            number = _number(item, path=path, field=f"{field}[{row_index}][{column_index}]", audit=audit)
            if number is None:
                return None
            values.append(number)
        rows.append(tuple(values))
    return tuple(rows)


def _vector(value: Any, *, path: str, field: str, size: int, audit: _Audit, positive: bool = False) -> tuple[float, ...] | None:
    if value is None:
        return None
    if isinstance(value, (str, bytes)) or not isinstance(value, Sequence) or len(value) != size:
        audit.add("vector_invalid", "error", path, f"{field} must contain exactly {size} numbers")
        return None
    values: list[float] = []
    for index, item in enumerate(value):
        number = _number(item, path=path, field=f"{field}[{index}]", audit=audit)
        if number is None:
            return None
        if positive and number <= 0:
            audit.add("vector_invalid", "error", path, f"{field} values must be positive")
            return None
        values.append(number)
    return tuple(values)


def _shape(value: Any, *, path: str, audit: _Audit) -> tuple[int, ...] | None:
    if value is None:
        audit.add("shape_missing", "error", path, "NIfTI shape is required")
        return None
    if isinstance(value, (str, bytes)) or not isinstance(value, Sequence) or not 3 <= len(value) <= 7:
        audit.add("shape_invalid", "error", path, "NIfTI shape must contain three to seven dimensions")
        return None
    result: list[int] = []
    for item in value:
        if isinstance(item, bool) or not isinstance(item, int) or item <= 0:
            audit.add("shape_invalid", "error", path, "NIfTI dimensions must be positive integers")
            return None
        result.append(item)
    voxels = math.prod(result)
    if voxels > MAX_NIFTI_VOXELS:
        audit.add("shape_too_large", "error", path, f"NIfTI voxel count exceeds {MAX_NIFTI_VOXELS}")
        return None
    return tuple(result)


def _code(value: Any, *, path: str, field: str, audit: _Audit) -> int:
    if value is None:
        return 0
    if isinstance(value, bool) or not isinstance(value, int) or value not in range(6):
        audit.add("form_code_invalid", "error", path, f"{field} must be an integer NIfTI form code from 0 through 5")
        return 0
    return value


def _parse_image(mapping: Mapping[str, Any], index: int, audit: _Audit) -> _Image:
    path = f"images[{index}]"
    raw_id = mapping.get("image_id", mapping.get("path"))
    if not isinstance(raw_id, str) or not raw_id.strip():
        audit.add("image_id_missing", "error", path, "each projection requires a non-empty image_id or path")
        image_id = path
    else:
        try:
            _text("image_id", raw_id, 2_048)
            image_id = raw_id
        except ArgumentError as error:
            audit.add("image_id_invalid", "error", path, str(error))
            image_id = path
    series_id = mapping.get("series_id", image_id)
    if not isinstance(series_id, str) or not series_id.strip():
        audit.add("series_id_invalid", "error", image_id, "series_id must be a non-empty string")
        series_id = image_id
    else:
        try:
            _text("series_id", series_id, 512)
        except ArgumentError as error:
            audit.add("series_id_invalid", "error", image_id, str(error))
            series_id = image_id
    shape = _shape(mapping.get("shape"), path=image_id, audit=audit)
    dtype = mapping.get("dtype")
    if not isinstance(dtype, str) or not _DTYPE.fullmatch(dtype):
        audit.add("dtype_invalid", "error", image_id, "dtype must be an explicit primitive NIfTI datatype such as float32 or int16")
        dtype = None
    affine = _matrix(mapping.get("affine"), path=image_id, field="affine", audit=audit)
    if affine is None:
        audit.add("affine_missing", "error", image_id, "the effective affine matrix is required")
    qform_code = _code(mapping.get("qform_code"), path=image_id, field="qform_code", audit=audit)
    sform_code = _code(mapping.get("sform_code"), path=image_id, field="sform_code", audit=audit)
    qform = _matrix(mapping.get("qform_affine"), path=image_id, field="qform_affine", audit=audit)
    sform = _matrix(mapping.get("sform_affine"), path=image_id, field="sform_affine", audit=audit)
    if qform_code > 0 and qform is None:
        audit.add("form_missing", "error", image_id, "qform_code is non-zero but qform_affine is absent")
    if sform_code > 0 and sform is None:
        audit.add("form_missing", "error", image_id, "sform_code is non-zero but sform_affine is absent")
    voxel_values = _vector(mapping.get("voxel_sizes"), path=image_id, field="voxel_sizes", size=3, audit=audit, positive=True)
    voxel_sizes = None if voxel_values is None else (voxel_values[0], voxel_values[1], voxel_values[2])
    axis_raw = mapping.get("axis_codes")
    axis_codes: tuple[str, str, str] | None = None
    if axis_raw is not None:
        if isinstance(axis_raw, (str, bytes)) or not isinstance(axis_raw, Sequence) or len(axis_raw) != 3 or any(not isinstance(item, str) or item not in _AXIS_CODES for item in axis_raw):
            audit.add("axis_codes_invalid", "error", image_id, "axis_codes must contain three R/L/A/P/S/I labels")
        else:
            axis_codes = (axis_raw[0], axis_raw[1], axis_raw[2])
    coordinate_system = mapping.get("coordinate_system")
    if coordinate_system is not None:
        if not isinstance(coordinate_system, str):
            audit.add("coordinate_system_invalid", "error", image_id, "coordinate_system must be a string")
            coordinate_system = None
        else:
            try:
                _text("coordinate_system", coordinate_system, 128)
            except ArgumentError as error:
                audit.add("coordinate_system_invalid", "error", image_id, str(error))
                coordinate_system = None
    reference_space = mapping.get("reference_space")
    if reference_space is not None:
        if not isinstance(reference_space, str):
            audit.add("reference_space_invalid", "error", image_id, "reference_space must be a string")
            reference_space = None
        else:
            try:
                _text("reference_space", reference_space, 256)
            except ArgumentError as error:
                audit.add("reference_space_invalid", "error", image_id, str(error))
                reference_space = None
    units = mapping.get("units")
    space_units: str | None = None
    time_units: str | None = None
    if units is not None:
        if not isinstance(units, Mapping):
            audit.add("units_invalid", "error", image_id, "units must be a mapping")
        else:
            space_units = units.get("space")
            time_units = units.get("time")
            if space_units is not None and (not isinstance(space_units, str) or space_units not in _SPACE_UNITS):
                audit.add("units_invalid", "error", image_id, "units.space is not a supported NIfTI unit label")
                space_units = None
            if time_units is not None and (not isinstance(time_units, str) or time_units not in _TIME_UNITS):
                audit.add("units_invalid", "error", image_id, "units.time is not a supported NIfTI unit label")
                time_units = None
    intent = mapping.get("intent")
    if intent is not None:
        if not isinstance(intent, str):
            audit.add("intent_invalid", "error", image_id, "intent must be a bounded string")
            intent = None
        else:
            try:
                _text("intent", intent, 256)
            except ArgumentError as error:
                audit.add("intent_invalid", "error", image_id, str(error))
                intent = None
    return _Image(
        image_id,
        series_id,
        shape,
        dtype,
        affine,
        qform,
        sform,
        qform_code,
        sform_code,
        voxel_sizes,
        axis_codes,
        coordinate_system,
        reference_space,
        space_units,
        time_units,
        intent,
    )


def _close_matrix(left: tuple[tuple[float, ...], ...] | None, right: tuple[tuple[float, ...], ...] | None) -> bool:
    return left is not None and right is not None and all(abs(a - b) <= GEOMETRY_TOLERANCE for left_row, right_row in zip(left, right) for a, b in zip(left_row, right_row))


def _determinant3(matrix: tuple[tuple[float, ...], ...]) -> float:
    a, b, c = matrix[0][:3]
    d, e, f = matrix[1][:3]
    g, h, i = matrix[2][:3]
    return a * (e * i - f * h) - b * (d * i - f * g) + c * (d * h - e * g)


def _affine_audit(image: _Image, audit: _Audit) -> dict[str, Any]:
    if image.affine is None:
        audit.loss("coordinate_frame_not_carried", "blocking", image.image_id, "effective affine is absent")
        return {"valid": False, "affine_digest": None}
    affine = image.affine
    if any(abs(affine[3][index] - (1.0 if index == 3 else 0.0)) > GEOMETRY_TOLERANCE for index in range(4)):
        audit.add("affine_last_row_invalid", "error", image.image_id, "affine last row must be [0, 0, 0, 1]")
    determinant = _determinant3(affine)
    if abs(determinant) <= GEOMETRY_TOLERANCE:
        audit.add("affine_singular", "error", image.image_id, "affine spatial transform must be non-singular")
    column_norms = tuple(math.sqrt(sum(affine[row][column] ** 2 for row in range(3))) for column in range(3))
    if image.voxel_sizes is None:
        audit.loss("type_undetermined", "major", image.image_id, "voxel sizes were not carried in the parsed header projection")
    elif any(abs(expected - actual) > max(GEOMETRY_TOLERANCE, expected * 0.01) for expected, actual in zip(image.voxel_sizes, column_norms)):
        audit.add("voxel_size_affine_mismatch", "error", image.image_id, "voxel_sizes disagree with affine column norms")
    if image.qform_code == 0 and image.sform_code == 0:
        audit.loss("coordinate_frame_not_carried", "blocking", image.image_id, "both qform_code and sform_code are zero")
    if image.qform_code > 0 and image.qform is not None and image.sform_code > 0 and image.sform is not None and not _close_matrix(image.qform, image.sform):
        audit.add("qform_sform_disagree", "warning", image.image_id, "qform and sform carry different coordinate transforms")
        audit.loss("coordinate_frame_not_carried", "major", image.image_id, "qform/sform disagreement leaves frame precedence to the caller")
    if image.sform_code > 0 and image.sform is not None and not _close_matrix(image.affine, image.sform):
        audit.add("effective_affine_mismatch", "error", image.image_id, "effective affine disagrees with the declared sform")
    elif image.sform_code == 0 and image.qform_code > 0 and image.qform is not None and not _close_matrix(image.affine, image.qform):
        audit.add("effective_affine_mismatch", "error", image.image_id, "effective affine disagrees with the declared qform")
    if image.coordinate_system is None:
        audit.loss("coordinate_frame_not_carried", "major", image.image_id, "coordinate_system was not disclosed")
    if image.space_units is None:
        audit.loss("type_undetermined", "major", image.image_id, "spatial units were not disclosed")
    return {
        "valid": not any(code in audit.codes for code in {"affine_last_row_invalid", "affine_singular", "voxel_size_affine_mismatch", "effective_affine_mismatch"}),
        "affine_digest": hashlib.sha256(canonical_json(affine).encode("utf-8")).hexdigest()[:24],
        "determinant": determinant,
        "column_norms": list(column_norms),
    }


def _validate_provenance(provenance: Mapping[str, Any] | None, audit: _Audit) -> str | None:
    if not provenance:
        audit.loss("provenance_unavailable", "blocking", "provenance", "no non-empty provenance projection was supplied")
        return None
    try:
        encoded = canonical_json(dict(provenance)).encode("utf-8")
    except Exception as error:  # noqa: BLE001 - make projection failure evidence-bearing
        audit.add("provenance_not_json", "error", "provenance", f"provenance is not canonical JSON-safe: {error}")
        audit.loss("provenance_unavailable", "blocking", "provenance", "provenance could not be represented canonically")
        return None
    if len(encoded) > MAX_NIFTI_ITEMS * 10_000:
        audit.add("provenance_too_large", "error", "provenance", "provenance exceeds the bounded audit limit")
        audit.loss("provenance_unavailable", "blocking", "provenance", "provenance exceeds the bounded audit limit")
        return None
    return content_digest(dict(provenance))


def audit_nifti(
    images: Sequence[Mapping[str, Any]],
    *,
    source_id: str,
    provenance: Mapping[str, Any] | None = None,
    max_images: int = MAX_NIFTI_IMAGES,
    max_items: int = MAX_NIFTI_ITEMS,
) -> NiftiAuditResult:
    """Audit parsed NIfTI header projections without reading image arrays or files."""

    _text("source_id", source_id)
    max_images = _limit("max_images", max_images, MAX_NIFTI_IMAGES)
    max_items = _limit("max_items", max_items, MAX_NIFTI_ITEMS)
    if isinstance(images, (str, bytes)) or not isinstance(images, Sequence):
        raise ArgumentError("images must be a sequence of parsed NIfTI mappings")
    if len(images) == 0 or len(images) > max_images:
        raise ArgumentError(f"images must contain between 1 and {max_images} projections")
    if provenance is not None and not isinstance(provenance, Mapping):
        raise ArgumentError("provenance must be a JSON-object mapping when supplied")

    audit = _Audit(max_items)
    audit.loss("content_uninterpreted", "minor", source_id, "NIfTI image arrays and file bytes were not decoded")
    provenance_digest = _validate_provenance(provenance, audit)
    parsed: list[_Image] = []
    for index, mapping in enumerate(images):
        if not isinstance(mapping, Mapping):
            audit.add("image_not_mapping", "error", f"images[{index}]", "each projection must be a JSON object")
            continue
        parsed.append(_parse_image(mapping, index, audit))

    ids: dict[str, str] = {}
    series: dict[str, list[_Image]] = {}
    image_rows: list[dict[str, Any]] = []
    for image in parsed:
        if image.image_id in ids:
            audit.add("image_id_duplicate", "error", image.image_id, "image_id occurs more than once", (ids[image.image_id],))
        else:
            ids[image.image_id] = image.image_id
        series.setdefault(image.series_id, []).append(image)
        affine = _affine_audit(image, audit)
        image_rows.append(
            {
                "image_id": image.image_id,
                "series_id": image.series_id,
                "shape": list(image.shape) if image.shape else None,
                "dtype": image.dtype,
                "affine_digest": affine["affine_digest"],
                "qform_code": image.qform_code,
                "sform_code": image.sform_code,
                "voxel_sizes": list(image.voxel_sizes) if image.voxel_sizes else None,
                "axis_codes": list(image.axis_codes) if image.axis_codes else None,
                "coordinate_system": image.coordinate_system,
                "reference_space": image.reference_space,
                "units": {"space": image.space_units, "time": image.time_units},
                "intent": image.intent,
                "affine_geometry": affine,
            }
        )

    series_rows: list[dict[str, Any]] = []
    for series_id, members in sorted(series.items()):
        shapes = {member.shape for member in members}
        voxel_sets = {member.voxel_sizes for member in members}
        if len(shapes) > 1:
            audit.add("series_shape_inconsistent", "error", series_id, "NIfTI images in one series disagree on shape")
        if len(voxel_sets) > 1:
            audit.add("series_voxel_size_inconsistent", "error", series_id, "NIfTI images in one series disagree on voxel sizes")
        series_rows.append(
            {
                "series_id": series_id,
                "image_count": len(members),
                "shape": list(next(iter(shapes))) if len(shapes) == 1 and None not in shapes else None,
                "voxel_sizes": list(next(iter(voxel_sets))) if len(voxel_sets) == 1 and None not in voxel_sets else None,
                "coordinate_systems": sorted({member.coordinate_system for member in members if member.coordinate_system}),
                "reference_spaces": sorted({member.reference_space for member in members if member.reference_space}),
            }
        )

    try:
        source_digest = content_digest({"source_id": source_id, "images": [dict(item) for item in images]})
    except Exception as error:  # noqa: BLE001 - keep a safe identity for malformed projections
        audit.add("projection_not_json", "error", source_id, f"image projections are not canonical JSON-safe: {error}")
        source_digest = content_digest({"source_id": source_id, "image_ids": [row["image_id"] for row in image_rows]})

    valid = audit.error_count == 0
    publishable = valid and audit.blocking_loss_count == 0
    document: dict[str, Any] = {
        "schema": NIFTI_SCHEMA,
        "workflow": "nifti_projection_audit",
        "valid": valid,
        "publishable": publishable,
        "source_id": source_id,
        "manifest": {
            "source_id": source_id,
            "source_digest": source_digest,
            "adapter": NIFTI_ADAPTER,
            "adapter_version": NIFTI_ADAPTER_VERSION,
            "declared_format": "application/nifti-manifest",
            "image_count": len(parsed),
            "series_count": len(series),
            "provenance_digest": provenance_digest,
            "bytes_read": False,
        },
        "summary": {
            "images": len(parsed),
            "series": len(series),
            "errors": audit.error_count,
            "warnings": audit.warning_count,
            "finding_count": audit.total,
            "blocking_loss_count": audit.blocking_loss_count,
        },
        "series": series_rows[:max_items],
        "omitted_series": max(0, len(series_rows) - max_items),
        "images": image_rows[:max_items],
        "omitted_images": max(0, len(image_rows) - max_items),
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
                "shape_and_datatype": "pass" if not any(code in audit.codes for code in {"shape_missing", "shape_invalid", "shape_too_large", "dtype_invalid"}) else "fail",
                "affine": "pass" if not any(code in audit.codes for code in {"affine_missing", "affine_last_row_invalid", "affine_singular", "effective_affine_mismatch", "voxel_size_affine_mismatch"}) else "fail",
                "form_declarations": "pass" if not any(code in audit.codes for code in {"form_code_invalid", "form_missing"}) else "fail",
                "series_consistency": "pass" if not any(code in audit.codes for code in {"series_shape_inconsistent", "series_voxel_size_inconsistent"}) else "fail",
                "provenance": "pass" if provenance_digest is not None else "loss",
            },
            "limitations": [
                "the audit consumes caller-supplied parsed header projections and does not access filesystem bytes",
                "image arrays, compression, extensions, BIDS sidecars, slice timing, and scanner-specific private fields are not independently decoded",
                "a valid report proves only the bounded header and affine checks represented here; it is not a registration, segmentation, or clinical interpretation",
            ],
        },
        "max_images": max_images,
        "max_items": max_items,
    }
    document["document_digest"] = content_digest(document)
    return NiftiAuditResult(document)


__all__ = [
    "GEOMETRY_TOLERANCE",
    "MAX_NIFTI_IMAGES",
    "MAX_NIFTI_ITEMS",
    "NIFTI_ADAPTER",
    "NIFTI_ADAPTER_VERSION",
    "NIFTI_SCHEMA",
    "NiftiAdapter",
    "NiftiAuditResult",
    "NiftiFinding",
    "audit_nifti",
]
