"""Bounded AnnData/Zarr projection and matrix-metadata auditing.

The auditor consumes a parsed dataset projection. It checks AnnData-like dimensions, index
identity, ``obs``/``var`` column lengths, layers, embeddings, pairwise matrices, sparse metadata,
and raw-shape relationships without opening HDF5/Zarr stores or reading matrix payloads.
"""

from __future__ import annotations

from dataclasses import dataclass
import hashlib
import math
import re
from typing import Any, Mapping, Sequence

from .authoring import canonical_json, content_digest
from .errors import ArgumentError


ANNDATA_SCHEMA = "bioprism-python-anndata/0.1"
ANNDATA_ADAPTER = "bioprism.python.anndata_metadata"
ANNDATA_ADAPTER_VERSION = "0.1.0"
MAX_ANNDATA_ROWS = 10_000_000
MAX_ANNDATA_ITEMS = 1_000
_NAME = re.compile(r"^[A-Za-z_][A-Za-z0-9_.:-]{0,255}$")
_DTYPE = re.compile(r"^(?:bool|u?int|float|complex|string|category|object)[0-9_]*$")
_MATRIX_FORMATS = {"dense", "csr", "csc", "coo"}


@dataclass(frozen=True)
class AnnDataFinding:
    code: str
    severity: str
    path: str
    detail: str
    related_paths: tuple[str, ...] = ()

    def __post_init__(self) -> None:
        if self.severity not in {"error", "warning", "info"}:
            raise ArgumentError(f"invalid AnnData finding severity: {self.severity!r}")

    def to_dict(self) -> dict[str, Any]:
        result: dict[str, Any] = {"code": self.code, "severity": self.severity, "path": self.path, "detail": self.detail}
        if self.related_paths:
            result["related_paths"] = list(self.related_paths)
        return result


@dataclass(frozen=True)
class AnnDataAuditResult:
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
        self.findings: list[AnnDataFinding] = []
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
            self.findings.append(AnnDataFinding(code, severity, path, detail, tuple(related_paths)))

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


class AnnDataAdapter:
    name = ANNDATA_ADAPTER
    version = ANNDATA_ADAPTER_VERSION
    accepted_formats = ("application/anndata-manifest",)
    declared_loss_kinds = frozenset({"coordinate_frame_not_carried", "provenance_unavailable", "content_uninterpreted", "type_undetermined"})

    def manifest(self) -> dict[str, Any]:
        return {
            "name": self.name,
            "version": self.version,
            "accepted_formats": list(self.accepted_formats),
            "conformance_level": "normalize",
            "declared_loss_kinds": sorted(self.declared_loss_kinds),
            "scope_dimensions": ["subject", "cell", "feature", "assay", "embedding"],
            "execution": "python_delegated",
            "optional_dependency": None,
        }

    def audit(
        self,
        dataset: Mapping[str, Any],
        *,
        source_id: str,
        provenance: Mapping[str, Any] | None = None,
        max_items: int = MAX_ANNDATA_ITEMS,
    ) -> AnnDataAuditResult:
        return audit_anndata(dataset, source_id=source_id, provenance=provenance, max_items=max_items)


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


def _name(value: Any, *, path: str, audit: _Audit, field: str) -> str | None:
    if not isinstance(value, str) or not _NAME.fullmatch(value):
        audit.add("name_invalid", "error", path, f"{field} must be a bounded schema-safe name")
        return None
    return value


def _count(value: Any, *, path: str, field: str, audit: _Audit) -> int | None:
    if isinstance(value, bool) or not isinstance(value, int) or not 0 <= value <= MAX_ANNDATA_ROWS:
        audit.add("count_invalid", "error", path, f"{field} must be an integer from 0 through {MAX_ANNDATA_ROWS}")
        return None
    return value


def _dtype(value: Any, *, path: str, field: str, audit: _Audit) -> str | None:
    if not isinstance(value, str) or not _DTYPE.fullmatch(value):
        audit.add("dtype_invalid", "error", path, f"{field} must be an explicit dtype label")
        return None
    return value


def _shape(value: Any, *, path: str, field: str, audit: _Audit, expected: tuple[int, int] | None = None) -> tuple[int, int] | None:
    if isinstance(value, (str, bytes)) or not isinstance(value, Sequence) or len(value) != 2:
        audit.add("shape_invalid", "error", path, f"{field} must be a two-dimensional shape")
        return None
    shape: list[int] = []
    for item in value:
        if isinstance(item, bool) or not isinstance(item, int) or not 0 <= item <= MAX_ANNDATA_ROWS:
            audit.add("shape_invalid", "error", path, f"{field} dimensions must be bounded non-negative integers")
            return None
        shape.append(item)
    result = (shape[0], shape[1])
    if expected is not None and result != expected:
        audit.add("shape_mismatch", "error", path, f"{field} shape {result!r} disagrees with expected {expected!r}")
    return result


def _index_digest(source_id: str, values: Sequence[Any], *, path: str, audit: _Audit) -> tuple[str | None, int, int]:
    if len(values) > MAX_ANNDATA_ROWS:
        audit.add("index_too_large", "error", path, f"index exceeds {MAX_ANNDATA_ROWS} rows")
        return None, 0, 0
    normalized: list[str] = []
    seen: dict[str, int] = {}
    for index, value in enumerate(values):
        if not isinstance(value, str) or not value:
            audit.add("index_value_invalid", "error", f"{path}[{index}]", "index values must be non-empty strings")
            continue
        if len(value.encode("utf-8")) > 512 or any(ord(character) < 0x20 for character in value):
            audit.add("index_value_invalid", "error", f"{path}[{index}]", "index values must be bounded and printable")
            continue
        if value in seen:
            audit.add("index_duplicate", "error", f"{path}[{index}]", "index value occurs more than once", (f"{path}[{seen[value]}]",))
        else:
            seen[value] = index
        normalized.append(value)
    digest = hashlib.sha256((source_id + "\0" + "\n".join(normalized)).encode("utf-8")).hexdigest()[:24]
    return digest, len(normalized), len(seen)


def _column(
    value: Any,
    *,
    path: str,
    axis_count: int,
    audit: _Audit,
) -> dict[str, Any]:
    if not isinstance(value, Mapping):
        audit.add("column_invalid", "error", path, "column projection must be a mapping")
        return {"length": None, "dtype": None}
    length = _count(value.get("length"), path=path, field="column length", audit=audit)
    if length is not None and length != axis_count:
        audit.add("column_length_mismatch", "error", path, f"column length {length} disagrees with axis length {axis_count}")
    dtype = _dtype(value.get("dtype"), path=path, field="column dtype", audit=audit)
    missing = value.get("missing_count")
    if missing is not None and (isinstance(missing, bool) or not isinstance(missing, int) or missing < 0 or (length is not None and missing > length)):
        audit.add("missing_count_invalid", "error", path, "missing_count must be within the column length")
        missing = None
    categories = value.get("categories")
    category_count = None
    if categories is not None:
        if isinstance(categories, (str, bytes)) or not isinstance(categories, Sequence):
            audit.add("categories_invalid", "error", path, "categories must be a sequence")
        else:
            category_count = len(categories)
            seen: set[str] = set()
            for index, category in enumerate(categories):
                if not isinstance(category, str) or not category or category in seen:
                    audit.add("categories_invalid", "error", f"{path}.categories[{index}]", "categories must be unique non-empty strings")
                elif len(category.encode("utf-8")) > 512:
                    audit.add("categories_invalid", "error", f"{path}.categories[{index}]", "category label exceeds the bounded size")
                seen.add(category)
    return {"length": length, "dtype": dtype, "missing_count": missing, "category_count": category_count}


def _matrix(
    value: Any,
    *,
    path: str,
    expected_shape: tuple[int, int],
    audit: _Audit,
) -> dict[str, Any]:
    if not isinstance(value, Mapping):
        audit.add("matrix_invalid", "error", path, "matrix projection must be a mapping")
        return {"shape": None, "dtype": None, "format": None}
    shape = _shape(value.get("shape"), path=path, field="matrix", audit=audit, expected=expected_shape)
    dtype = _dtype(value.get("dtype"), path=path, field="matrix dtype", audit=audit)
    matrix_format = value.get("format", "dense")
    if matrix_format not in _MATRIX_FORMATS:
        audit.add("matrix_format_invalid", "error", path, "matrix format must be dense, csr, csc, or coo")
        matrix_format = None
    nnz = value.get("nnz")
    if nnz is not None:
        if isinstance(nnz, bool) or not isinstance(nnz, int) or nnz < 0 or (shape is not None and nnz > shape[0] * shape[1]):
            audit.add("nnz_invalid", "error", path, "nnz must be within the matrix capacity")
            nnz = None
    if matrix_format == "dense" and nnz is not None:
        audit.add("dense_nnz_unexpected", "warning", path, "dense matrices should not need sparse nnz metadata")
    if matrix_format in {"csr", "csc"}:
        axis = shape[0] if shape is not None and matrix_format == "csr" else shape[1] if shape is not None else None
        indptr_length = value.get("indptr_length")
        if axis is not None and indptr_length is not None and indptr_length != axis + 1:
            audit.add("sparse_indptr_invalid", "error", path, "sparse indptr_length must equal the compressed axis plus one")
        indices_length = value.get("indices_length")
        if nnz is not None and indices_length is not None and indices_length != nnz:
            audit.add("sparse_indices_invalid", "error", path, "sparse indices_length must equal nnz")
    if matrix_format == "coo" and nnz is not None:
        coordinate_length = value.get("coordinate_length")
        if coordinate_length is not None and coordinate_length != nnz:
            audit.add("sparse_coordinates_invalid", "error", path, "COO coordinate_length must equal nnz")
    sorted_indices = value.get("sorted_indices")
    if sorted_indices is not None and not isinstance(sorted_indices, bool):
        audit.add("sparse_metadata_invalid", "error", path, "sorted_indices must be boolean")
        sorted_indices = None
    return {"shape": list(shape) if shape else None, "dtype": dtype, "format": matrix_format, "nnz": nnz, "sorted_indices": sorted_indices}


def _mapping_names(value: Any, *, path: str, audit: _Audit) -> Mapping[str, Any]:
    if value is None:
        return {}
    if not isinstance(value, Mapping):
        audit.add("mapping_invalid", "error", path, "expected a mapping of named projections")
        return {}
    result: dict[str, Any] = {}
    for key, item in value.items():
        name = _name(key, path=f"{path}.{key}", audit=audit, field="mapping key")
        if name is not None:
            result[name] = item
    return result


def _validate_provenance(provenance: Mapping[str, Any] | None, audit: _Audit) -> str | None:
    if not provenance:
        audit.loss("provenance_unavailable", "blocking", "provenance", "no non-empty provenance projection was supplied")
        return None
    try:
        encoded = canonical_json(dict(provenance)).encode("utf-8")
    except Exception as error:  # noqa: BLE001
        audit.add("provenance_not_json", "error", "provenance", f"provenance is not canonical JSON-safe: {error}")
        audit.loss("provenance_unavailable", "blocking", "provenance", "provenance could not be represented canonically")
        return None
    if len(encoded) > 10_000_000:
        audit.add("provenance_too_large", "error", "provenance", "provenance exceeds the bounded audit limit")
        audit.loss("provenance_unavailable", "blocking", "provenance", "provenance exceeds the bounded audit limit")
        return None
    return content_digest(dict(provenance))


def audit_anndata(
    dataset: Mapping[str, Any],
    *,
    source_id: str,
    provenance: Mapping[str, Any] | None = None,
    max_items: int = MAX_ANNDATA_ITEMS,
) -> AnnDataAuditResult:
    """Audit a parsed AnnData/Zarr projection without reading matrix payloads."""

    _text("source_id", source_id)
    max_items = _limit("max_items", max_items, MAX_ANNDATA_ITEMS)
    if not isinstance(dataset, Mapping):
        raise ArgumentError("dataset must be a JSON-object AnnData projection")
    if provenance is not None and not isinstance(provenance, Mapping):
        raise ArgumentError("provenance must be a JSON-object mapping when supplied")

    audit = _Audit(max_items)
    audit.loss("content_uninterpreted", "minor", source_id, "AnnData/Zarr matrix payloads and store bytes were not decoded")
    provenance_digest = _validate_provenance(provenance, audit)
    n_obs = _count(dataset.get("n_obs"), path="n_obs", field="n_obs", audit=audit)
    n_vars = _count(dataset.get("n_vars"), path="n_vars", field="n_vars", audit=audit)
    if n_obs is None:
        n_obs = 0
    if n_vars is None:
        n_vars = 0
    x_value = dataset.get("X", dataset.get("x"))
    if x_value is None:
        x_value = {"shape": [n_obs, n_vars], "dtype": dataset.get("x_dtype"), "format": "dense"}
    x_matrix = _matrix(x_value, path="X", expected_shape=(n_obs, n_vars), audit=audit)
    obs_index = dataset.get("obs_index", [])
    var_index = dataset.get("var_index", [])
    if isinstance(obs_index, (str, bytes)) or not isinstance(obs_index, Sequence):
        audit.add("index_invalid", "error", "obs_index", "obs_index must be a sequence")
        obs_index = []
    if isinstance(var_index, (str, bytes)) or not isinstance(var_index, Sequence):
        audit.add("index_invalid", "error", "var_index", "var_index must be a sequence")
        var_index = []
    if len(obs_index) != n_obs:
        audit.add("index_length_mismatch", "error", "obs_index", f"obs_index has {len(obs_index)} values but n_obs is {n_obs}")
    if len(var_index) != n_vars:
        audit.add("index_length_mismatch", "error", "var_index", f"var_index has {len(var_index)} values but n_vars is {n_vars}")
    obs_digest, obs_index_count, obs_unique = _index_digest(source_id, obs_index, path="obs_index", audit=audit)
    var_digest, var_index_count, var_unique = _index_digest(source_id, var_index, path="var_index", audit=audit)

    obs_columns = _mapping_names(dataset.get("obs"), path="obs", audit=audit)
    if not obs_columns and "obs_columns" in dataset:
        obs_columns = _mapping_names(dataset.get("obs_columns"), path="obs_columns", audit=audit)
    var_columns = _mapping_names(dataset.get("var"), path="var", audit=audit)
    if not var_columns and "var_columns" in dataset:
        var_columns = _mapping_names(dataset.get("var_columns"), path="var_columns", audit=audit)
    obs_rows = [{"name": name, "projection": _column(value, path=f"obs.{name}", axis_count=n_obs, audit=audit)} for name, value in sorted(obs_columns.items())]
    var_rows = [{"name": name, "projection": _column(value, path=f"var.{name}", axis_count=n_vars, audit=audit)} for name, value in sorted(var_columns.items())]

    layers = _mapping_names(dataset.get("layers"), path="layers", audit=audit)
    layer_rows = [{"name": name, "projection": _matrix(value, path=f"layers.{name}", expected_shape=(n_obs, n_vars), audit=audit)} for name, value in sorted(layers.items())]
    obsm = _mapping_names(dataset.get("obsm"), path="obsm", audit=audit)
    obsm_rows = [{"name": name, "projection": _matrix(value, path=f"obsm.{name}", expected_shape=(n_obs, _embedding_width(value, f"obsm.{name}", audit)), audit=audit)} for name, value in sorted(obsm.items())]
    varm = _mapping_names(dataset.get("varm"), path="varm", audit=audit)
    varm_rows = [{"name": name, "projection": _matrix(value, path=f"varm.{name}", expected_shape=(n_vars, _embedding_width(value, f"varm.{name}", audit)), audit=audit)} for name, value in sorted(varm.items())]
    obsp = _mapping_names(dataset.get("obsp"), path="obsp", audit=audit)
    obsp_rows = [{"name": name, "projection": _matrix(value, path=f"obsp.{name}", expected_shape=(n_obs, n_obs), audit=audit)} for name, value in sorted(obsp.items())]
    varp = _mapping_names(dataset.get("varp"), path="varp", audit=audit)
    varp_rows = [{"name": name, "projection": _matrix(value, path=f"varp.{name}", expected_shape=(n_vars, n_vars), audit=audit)} for name, value in sorted(varp.items())]

    raw = dataset.get("raw")
    raw_projection = None
    if raw is not None:
        if not isinstance(raw, Mapping):
            audit.add("raw_invalid", "error", "raw", "raw must be a mapping when supplied")
        else:
            raw_vars = _count(raw.get("n_vars"), path="raw.n_vars", field="raw.n_vars", audit=audit)
            raw_shape = _shape(raw.get("shape"), path="raw.shape", field="raw", audit=audit, expected=(n_obs, raw_vars or 0))
            raw_projection = {"n_vars": raw_vars, "shape": list(raw_shape) if raw_shape else None, "var_index_digest": None}
            if isinstance(raw.get("var_index"), Sequence) and not isinstance(raw.get("var_index"), (str, bytes)):
                raw_digest, _, _ = _index_digest(source_id, raw["var_index"], path="raw.var_index", audit=audit)
                raw_projection["var_index_digest"] = raw_digest

    uns = _mapping_names(dataset.get("uns"), path="uns", audit=audit)
    uns_rows = [{"name": name, "kind": _safe_kind(value)} for name, value in sorted(uns.items())]
    try:
        source_digest = content_digest({"source_id": source_id, "dataset": dict(dataset)})
    except Exception as error:  # noqa: BLE001
        audit.add("projection_not_json", "error", source_id, f"dataset projection is not canonical JSON-safe: {error}")
        source_digest = content_digest({"source_id": source_id, "obs_index_digest": obs_digest, "var_index_digest": var_digest})

    valid = audit.error_count == 0
    publishable = valid and audit.blocking_loss_count == 0
    document: dict[str, Any] = {
        "schema": ANNDATA_SCHEMA,
        "workflow": "anndata_projection_audit",
        "valid": valid,
        "publishable": publishable,
        "source_id": source_id,
        "manifest": {
            "source_id": source_id,
            "source_digest": source_digest,
            "adapter": ANNDATA_ADAPTER,
            "adapter_version": ANNDATA_ADAPTER_VERSION,
            "declared_format": "application/anndata-manifest",
            "n_obs": n_obs,
            "n_vars": n_vars,
            "provenance_digest": provenance_digest,
            "bytes_read": False,
        },
        "summary": {
            "n_obs": n_obs,
            "n_vars": n_vars,
            "obs_columns": len(obs_rows),
            "var_columns": len(var_rows),
            "layers": len(layer_rows),
            "obsm": len(obsm_rows),
            "varm": len(varm_rows),
            "obsp": len(obsp_rows),
            "varp": len(varp_rows),
            "uns": len(uns_rows),
            "errors": audit.error_count,
            "warnings": audit.warning_count,
            "finding_count": audit.total,
            "blocking_loss_count": audit.blocking_loss_count,
        },
        "X": x_matrix,
        "indices": {
            "obs": {"count": obs_index_count, "unique": obs_unique, "digest": obs_digest},
            "var": {"count": var_index_count, "unique": var_unique, "digest": var_digest},
        },
        "obs": obs_rows[:max_items],
        "omitted_obs_columns": max(0, len(obs_rows) - max_items),
        "var": var_rows[:max_items],
        "omitted_var_columns": max(0, len(var_rows) - max_items),
        "layers": layer_rows[:max_items],
        "omitted_layers": max(0, len(layer_rows) - max_items),
        "obsm": obsm_rows[:max_items],
        "omitted_obsm": max(0, len(obsm_rows) - max_items),
        "varm": varm_rows[:max_items],
        "omitted_varm": max(0, len(varm_rows) - max_items),
        "obsp": obsp_rows[:max_items],
        "omitted_obsp": max(0, len(obsp_rows) - max_items),
        "varp": varp_rows[:max_items],
        "omitted_varp": max(0, len(varp_rows) - max_items),
        "raw": raw_projection,
        "uns": uns_rows[:max_items],
        "omitted_uns": max(0, len(uns_rows) - max_items),
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
                "dimensions": "pass" if not any(code in audit.codes for code in {"count_invalid", "shape_invalid", "shape_mismatch"}) else "fail",
                "indices": "pass" if not any(code in audit.codes for code in {"index_invalid", "index_length_mismatch", "index_duplicate", "index_value_invalid"}) else "fail",
                "annotations": "pass" if not any(code in audit.codes for code in {"column_invalid", "column_length_mismatch", "dtype_invalid", "categories_invalid"}) else "fail",
                "matrix_shapes": "pass" if not any(code in audit.codes for code in {"matrix_invalid", "matrix_format_invalid", "nnz_invalid", "sparse_indptr_invalid", "sparse_indices_invalid", "sparse_coordinates_invalid"}) else "fail",
                "provenance": "pass" if provenance_digest is not None else "loss",
            },
            "limitations": [
                "the audit consumes a caller-supplied parsed projection and does not access HDF5/Zarr bytes",
                "matrix values, categorical codes, backed-store chunks, compression, and arbitrary uns payloads are not decoded",
                "a valid report proves only the bounded dimensions, metadata, and sparse-structure checks represented here; it is not a biological interpretation",
            ],
        },
        "max_items": max_items,
    }
    document["document_digest"] = content_digest(document)
    return AnnDataAuditResult(document)


def _embedding_width(value: Any, path: str, audit: _Audit) -> int:
    if not isinstance(value, Mapping):
        audit.add("matrix_invalid", "error", path, "embedding projection must be a mapping")
        return 0
    shape = value.get("shape")
    if isinstance(shape, Sequence) and not isinstance(shape, (str, bytes)) and len(shape) == 2 and isinstance(shape[1], int) and shape[1] >= 0:
        return shape[1]
    audit.add("shape_invalid", "error", path, "embedding must disclose a two-dimensional shape")
    return 0


def _safe_kind(value: Any) -> str:
    if value is None:
        return "null"
    if isinstance(value, Mapping):
        return "mapping"
    if isinstance(value, (list, tuple)):
        return "sequence"
    if isinstance(value, bool):
        return "boolean"
    if isinstance(value, (int, float)):
        return "number"
    if isinstance(value, str):
        return "string"
    return "unsupported"


__all__ = [
    "ANNDATA_ADAPTER",
    "ANNDATA_ADAPTER_VERSION",
    "ANNDATA_SCHEMA",
    "AnnDataAdapter",
    "AnnDataAuditResult",
    "AnnDataFinding",
    "MAX_ANNDATA_ITEMS",
    "MAX_ANNDATA_ROWS",
    "audit_anndata",
]
