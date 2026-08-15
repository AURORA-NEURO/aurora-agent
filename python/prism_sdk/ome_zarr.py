"""Bounded OME-Zarr multiscale metadata and spatial-transform auditing."""

from __future__ import annotations

from dataclasses import dataclass
import math
import re
from typing import Any, Mapping, Sequence

from .authoring import content_digest
from .errors import ArgumentError


OME_SCHEMA = "bioprism-python-ome-zarr/0.1"
OME_ADAPTER = "bioprism.python.ome_zarr_metadata"
OME_ADAPTER_VERSION = "0.1.0"
MAX_OME_LEVELS = 1_000
MAX_OME_ITEMS = 1_000
MAX_OME_DIMENSIONS = 8
MAX_OME_DIMENSION = 10_000_000_000
_NAME = re.compile(r"^[A-Za-z][A-Za-z0-9_.:-]{0,255}$")
_PATH = re.compile(r"^[^/].*$")
_AXIS_TYPES = {"space", "time", "channel", "frequency", "t", "x", "y", "z"}


@dataclass(frozen=True)
class OmeFinding:
    code: str
    severity: str
    path: str
    detail: str
    related_paths: tuple[str, ...] = ()

    def __post_init__(self) -> None:
        if self.severity not in {"error", "warning", "info"}:
            raise ArgumentError(f"invalid OME-Zarr finding severity: {self.severity!r}")

    def to_dict(self) -> dict[str, Any]:
        result: dict[str, Any] = {"code": self.code, "severity": self.severity, "path": self.path, "detail": self.detail}
        if self.related_paths:
            result["related_paths"] = list(self.related_paths)
        return result


@dataclass(frozen=True)
class OmeAuditResult:
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


class _Audit:
    def __init__(self, limit: int) -> None:
        self.limit = limit
        self.findings: list[OmeFinding] = []
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
            self.findings.append(OmeFinding(code, severity, path, detail, tuple(related_paths)))

    def loss(self, kind: str, severity: str, path: str, detail: str) -> None:
        self.loss_total += 1
        if severity == "blocking":
            self.blocking_loss_count += 1
        ranks = {"minor": 1, "major": 2, "blocking": 3}
        if self.max_loss_severity is None or ranks[severity] > ranks[self.max_loss_severity]:
            self.max_loss_severity = severity
        if len(self.losses) < self.limit:
            self.losses.append({"kind": kind, "severity": severity, "path": path, "detail": detail})


class OmeZarrAdapter:
    name = OME_ADAPTER
    version = OME_ADAPTER_VERSION
    accepted_formats = ("application/ome-zarr-manifest",)
    declared_loss_kinds = frozenset({"coordinate_frame_not_carried", "provenance_unavailable", "content_uninterpreted", "type_undetermined"})

    def manifest(self) -> dict[str, Any]:
        return {
            "name": self.name,
            "version": self.version,
            "accepted_formats": list(self.accepted_formats),
            "conformance_level": "normalize",
            "declared_loss_kinds": sorted(self.declared_loss_kinds),
            "scope_dimensions": ["subject", "specimen", "image", "tile", "channel", "scale"],
            "execution": "python_delegated",
            "optional_dependency": None,
        }

    def audit(
        self,
        projection: Mapping[str, Any],
        *,
        source_id: str,
        provenance: Mapping[str, Any] | None = None,
        max_items: int = MAX_OME_ITEMS,
    ) -> OmeAuditResult:
        return audit_ome_zarr(projection, source_id=source_id, provenance=provenance, max_items=max_items)


def _text(value: Any, *, path: str, field: str, audit: _Audit, maximum: int = 512) -> str | None:
    if not isinstance(value, str) or not value.strip() or len(value.encode("utf-8")) > maximum or any(ord(character) < 0x20 for character in value):
        audit.add("text_invalid", "error", path, f"{field} must be bounded printable text")
        return None
    return value


def _vector(value: Any, *, path: str, field: str, dimensions: int, audit: _Audit, positive: bool = False) -> list[float] | None:
    if isinstance(value, (str, bytes)) or not isinstance(value, Sequence) or len(value) != dimensions:
        audit.add("transform_invalid", "error", path, f"{field} must contain {dimensions} numbers")
        return None
    result: list[float] = []
    for index, item in enumerate(value):
        if isinstance(item, bool) or not isinstance(item, (int, float)) or not math.isfinite(float(item)) or (positive and float(item) <= 0):
            audit.add("transform_invalid", "error", f"{path}[{index}]", f"{field} contains an invalid numeric value")
            return None
        result.append(float(item))
    return result


def _shape(value: Any, *, path: str, dimensions: int, audit: _Audit) -> list[int] | None:
    if isinstance(value, (str, bytes)) or not isinstance(value, Sequence) or len(value) != dimensions:
        audit.add("shape_invalid", "error", path, f"shape must contain {dimensions} dimensions")
        return None
    result: list[int] = []
    for index, item in enumerate(value):
        if isinstance(item, bool) or not isinstance(item, int) or not 0 < item <= MAX_OME_DIMENSION:
            audit.add("shape_invalid", "error", f"{path}[{index}]", f"dimension must be from 1 through {MAX_OME_DIMENSION}")
            return None
        result.append(item)
    return result


def _dataset(value: Any, *, path: str, dimensions: int, audit: _Audit) -> dict[str, Any]:
    if not isinstance(value, Mapping):
        audit.add("dataset_invalid", "error", path, "multiscale dataset must be a mapping")
        return {"path": None, "shape": None, "chunks": None, "dtype": None}
    dataset_path = value.get("path")
    if not isinstance(dataset_path, str) or not _PATH.fullmatch(dataset_path) or ".." in dataset_path.split("/"):
        audit.add("dataset_path_invalid", "error", path, "dataset path must be relative and must not traverse parents")
        dataset_path = None
    shape = _shape(value.get("shape"), path=f"{path}.shape", dimensions=dimensions, audit=audit)
    chunks = _shape(value.get("chunks"), path=f"{path}.chunks", dimensions=dimensions, audit=audit)
    if shape is not None and chunks is not None and any(chunk > size for chunk, size in zip(chunks, shape)):
        audit.add("chunks_invalid", "error", path, "chunk dimensions must not exceed dataset dimensions")
    dtype = _text(value.get("dtype"), path=f"{path}.dtype", field="dtype", audit=audit, maximum=128)
    transformations = value.get("coordinate_transformations", [])
    if not isinstance(transformations, Sequence) or isinstance(transformations, (str, bytes)):
        audit.add("transform_invalid", "error", path, "coordinate_transformations must be a sequence")
        transformations = []
    parsed_transforms: list[dict[str, Any]] = []
    for index, transform in enumerate(transformations):
        transform_path = f"{path}.coordinate_transformations[{index}]"
        if not isinstance(transform, Mapping):
            audit.add("transform_invalid", "error", transform_path, "coordinate transformation must be a mapping")
            continue
        transform_type = transform.get("type")
        if transform_type == "scale":
            scale = _vector(transform.get("scale"), path=transform_path, field="scale", dimensions=dimensions, audit=audit, positive=True)
            parsed_transforms.append({"type": "scale", "scale": scale})
        elif transform_type == "translation":
            translation = _vector(transform.get("translation"), path=transform_path, field="translation", dimensions=dimensions, audit=audit)
            parsed_transforms.append({"type": "translation", "translation": translation})
        else:
            audit.add("transform_unsupported", "warning", transform_path, f"unsupported transformation type {transform_type!r}")
    return {"path": dataset_path, "shape": shape, "chunks": chunks, "dtype": dtype, "coordinate_transformations": parsed_transforms}


def audit_ome_zarr(
    projection: Mapping[str, Any],
    *,
    source_id: str,
    provenance: Mapping[str, Any] | None = None,
    max_items: int = MAX_OME_ITEMS,
) -> OmeAuditResult:
    """Audit parsed OME-Zarr multiscale metadata without reading image chunks."""

    if not isinstance(source_id, str) or not source_id.strip():
        raise ArgumentError("source_id must be a non-empty string")
    if isinstance(max_items, bool) or not isinstance(max_items, int) or not 1 <= max_items <= MAX_OME_ITEMS:
        raise ArgumentError(f"max_items must be between 1 and {MAX_OME_ITEMS}")
    if not isinstance(projection, Mapping):
        raise ArgumentError("projection must be a mapping")
    if provenance is not None and not isinstance(provenance, Mapping):
        raise ArgumentError("provenance must be a mapping when supplied")

    audit = _Audit(max_items)
    audit.loss("content_uninterpreted", "minor", source_id, "OME-Zarr image chunks were not decoded")
    if not provenance:
        audit.loss("provenance_unavailable", "blocking", "provenance", "no non-empty provenance projection was supplied")
    try:
        provenance_digest = content_digest(dict(provenance)) if provenance else None
    except Exception as error:  # noqa: BLE001
        audit.add("provenance_not_json", "error", "provenance", f"provenance is not canonical JSON-safe: {error}")
        audit.loss("provenance_unavailable", "blocking", "provenance", "provenance could not be represented canonically")
        provenance_digest = None

    multiscales = projection.get("multiscales")
    if not isinstance(multiscales, Sequence) or isinstance(multiscales, (str, bytes)) or not multiscales:
        audit.add("multiscales_missing", "error", "multiscales", "projection requires a non-empty multiscales sequence")
        multiscales = []
    if len(multiscales) > MAX_OME_LEVELS:
        audit.add("multiscales_too_many", "error", "multiscales", f"multiscales exceeds {MAX_OME_LEVELS} entries")
    multiscale_rows: list[dict[str, Any]] = []
    all_paths: set[str] = set()
    for multi_index, multiscale in enumerate(multiscales[:MAX_OME_LEVELS]):
        path = f"multiscales[{multi_index}]"
        if not isinstance(multiscale, Mapping):
            audit.add("multiscale_invalid", "error", path, "multiscale entry must be a mapping")
            continue
        axes = multiscale.get("axes")
        if not isinstance(axes, Sequence) or isinstance(axes, (str, bytes)) or not axes:
            audit.add("axes_missing", "error", path, "multiscale entry requires axes")
            axes = []
        axis_rows: list[dict[str, Any]] = []
        axis_names: list[str] = []
        for axis_index, axis in enumerate(axes[:MAX_OME_DIMENSIONS]):
            axis_path = f"{path}.axes[{axis_index}]"
            if not isinstance(axis, Mapping):
                audit.add("axis_invalid", "error", axis_path, "axis must be a mapping")
                continue
            name = _text(axis.get("name"), path=axis_path, field="axis name", audit=audit, maximum=64)
            if name in axis_names:
                audit.add("axis_duplicate", "error", axis_path, f"axis {name!r} occurs more than once")
            if name is not None:
                axis_names.append(name)
            axis_type = axis.get("type")
            if axis_type is not None and (not isinstance(axis_type, str) or axis_type not in _AXIS_TYPES):
                audit.add("axis_type_invalid", "error", axis_path, f"unsupported axis type {axis_type!r}")
            axis_rows.append({"name": name, "type": axis_type, "unit": axis.get("unit")})
        dimensions = len(axis_rows)
        datasets = multiscale.get("datasets")
        if not isinstance(datasets, Sequence) or isinstance(datasets, (str, bytes)) or not datasets:
            audit.add("datasets_missing", "error", path, "multiscale entry requires datasets")
            datasets = []
        dataset_rows: list[dict[str, Any]] = []
        previous_shape: list[int] | None = None
        for dataset_index, dataset in enumerate(datasets[:MAX_OME_LEVELS]):
            parsed = _dataset(dataset, path=f"{path}.datasets[{dataset_index}]", dimensions=dimensions, audit=audit)
            dataset_path = parsed.get("path")
            if dataset_path is not None and dataset_path in all_paths:
                audit.add("dataset_path_duplicate", "error", path, f"dataset path {dataset_path!r} occurs more than once")
            if dataset_path is not None:
                all_paths.add(dataset_path)
            shape = parsed.get("shape")
            if previous_shape is not None and shape is not None and any(current > previous for current, previous in zip(shape, previous_shape)):
                audit.add("level_shape_increasing", "warning", path, "multiscale level shape increases relative to the previous level")
            if shape is not None:
                previous_shape = shape
            dataset_rows.append(parsed)
        if not axis_rows or not dataset_rows:
            audit.loss("coordinate_frame_not_carried", "blocking", path, "axes and multiscale datasets do not define a complete spatial coordinate projection")
        if not any(transform.get("type") == "scale" for row in dataset_rows for transform in row["coordinate_transformations"]):
            audit.loss("coordinate_frame_not_carried", "major", path, "no scale coordinate transformation was carried")
        multiscale_rows.append({"name": multiscale.get("name"), "version": multiscale.get("version"), "axes": axis_rows, "datasets": dataset_rows})

    omero = projection.get("omero")
    channel_rows: list[dict[str, Any]] = []
    if omero is not None:
        if not isinstance(omero, Mapping):
            audit.add("omero_invalid", "error", "omero", "omero metadata must be a mapping")
        else:
            channels = omero.get("channels", [])
            if not isinstance(channels, Sequence) or isinstance(channels, (str, bytes)):
                audit.add("channels_invalid", "error", "omero.channels", "channels must be a sequence")
                channels = []
            for index, channel in enumerate(channels[:MAX_OME_ITEMS]):
                if not isinstance(channel, Mapping):
                    audit.add("channel_invalid", "error", f"omero.channels[{index}]", "channel must be a mapping")
                    continue
                channel_rows.append({"label": channel.get("label"), "color": channel.get("color"), "active": channel.get("active")})

    labels = projection.get("labels", {})
    label_rows: list[dict[str, Any]] = []
    if labels is not None:
        if not isinstance(labels, Mapping):
            audit.add("labels_invalid", "error", "labels", "labels must be a mapping")
        else:
            for name, value in list(labels.items())[:MAX_OME_ITEMS]:
                if not isinstance(name, str) or not _NAME.fullmatch(name):
                    audit.add("label_invalid", "error", f"labels.{name}", "label name is invalid")
                    continue
                label_rows.append({"name": name, "kind": type(value).__name__})

    try:
        source_digest = content_digest({"source_id": source_id, "projection": dict(projection)})
    except Exception as error:  # noqa: BLE001
        audit.add("projection_not_json", "error", source_id, f"OME-Zarr projection is not canonical JSON-safe: {error}")
        source_digest = content_digest({"source_id": source_id, "dataset_paths": sorted(all_paths)})
    valid = audit.error_count == 0
    publishable = valid and audit.blocking_loss_count == 0
    document: dict[str, Any] = {
        "schema": OME_SCHEMA,
        "workflow": "ome_zarr_metadata_audit",
        "valid": valid,
        "publishable": publishable,
        "source_id": source_id,
        "manifest": {
            "source_id": source_id,
            "source_digest": source_digest,
            "adapter": OME_ADAPTER,
            "adapter_version": OME_ADAPTER_VERSION,
            "declared_format": "application/ome-zarr-manifest",
            "multiscale_count": len(multiscale_rows),
            "dataset_count": sum(len(row["datasets"]) for row in multiscale_rows),
            "provenance_digest": provenance_digest,
            "metadata_read": True,
            "payload_read": False,
        },
        "summary": {
            "multiscales": len(multiscale_rows),
            "datasets": sum(len(row["datasets"]) for row in multiscale_rows),
            "channels": len(channel_rows),
            "labels": len(label_rows),
            "errors": audit.error_count,
            "warnings": audit.warning_count,
            "finding_count": audit.total,
            "blocking_loss_count": audit.blocking_loss_count,
        },
        "multiscales": multiscale_rows[:max_items],
        "omitted_multiscales": max(0, len(multiscale_rows) - max_items),
        "omero_channels": channel_rows[:max_items],
        "omitted_channels": max(0, len(channel_rows) - max_items),
        "labels": label_rows[:max_items],
        "omitted_labels": max(0, len(label_rows) - max_items),
        "findings": [finding.to_dict() for finding in audit.findings],
        "omitted_findings": max(0, audit.total - len(audit.findings)),
        "semantic_loss": {"audit": "lossy" if audit.loss_total else "lossless", "lost_count": audit.loss_total, "max_severity": audit.max_loss_severity, "lost": audit.losses, "omitted_lost": max(0, audit.loss_total - len(audit.losses))},
        "conformance": {
            "passed": valid,
            "publishable": publishable,
            "checks": {
                "axes": "pass" if not any(code in audit.codes for code in {"axes_missing", "axis_invalid", "axis_duplicate", "axis_type_invalid"}) else "fail",
                "datasets": "pass" if not any(code in audit.codes for code in {"datasets_missing", "dataset_invalid", "dataset_path_invalid", "dataset_path_duplicate", "shape_invalid", "chunks_invalid"}) else "fail",
                "transforms": "pass" if not any(code in audit.codes for code in {"transform_invalid"}) else "fail",
                "channels_labels": "pass" if not any(code in audit.codes for code in {"omero_invalid", "channels_invalid", "channel_invalid", "labels_invalid", "label_invalid"}) else "fail",
                "provenance": "pass" if provenance_digest is not None else "loss",
            },
            "limitations": [
                "the audit consumes parsed multiscale metadata and does not read image chunks",
                "it does not validate pixel values, codecs, compressor settings, or scientific registration against an external reference",
                "a valid report proves only the bounded axes, level, chunk, transform, and disclosure checks represented here",
            ],
        },
        "max_items": max_items,
    }
    document["document_digest"] = content_digest(document)
    return OmeAuditResult(document)


__all__ = [
    "MAX_OME_DIMENSION",
    "MAX_OME_ITEMS",
    "MAX_OME_LEVELS",
    "OME_ADAPTER",
    "OME_ADAPTER_VERSION",
    "OME_SCHEMA",
    "OmeAuditResult",
    "OmeFinding",
    "OmeZarrAdapter",
    "audit_ome_zarr",
]
