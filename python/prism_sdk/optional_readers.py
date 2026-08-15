"""Verified optional-reader bindings for raw NIfTI and AnnData/Zarr sources.

These bindings are intentionally thin: they inspect bounded headers/metadata and immediately feed
the result into the dependency-free projection auditors. They never call ``get_fdata`` or materialize
matrix values, and a missing optional dependency becomes a typed runtime refusal rather than an
implicit fallback.
"""

from __future__ import annotations

import importlib
from pathlib import Path
from typing import Any, Mapping

from .anndata import audit_anndata
from .errors import ArgumentError
from .nifti import audit_nifti


MAX_READER_FILE_BYTES = 4_000_000_000
MAX_READER_INDEX_VALUES = 1_000_000
MAX_READER_CATEGORIES = 10_000


class OptionalDependencyUnavailable(ArgumentError):
    """An optional scientific package is required for the requested raw-reader route."""

    def __init__(self, dependency: str) -> None:
        self.dependency = dependency
        super().__init__(f"optional dependency {dependency!r} is not installed")


def _path(value: Any, *, field: str, directory: bool = False) -> Path:
    if not isinstance(value, str) or not value.strip():
        raise ArgumentError(f"{field} must be a non-empty path string")
    candidate = Path(value)
    if not candidate.exists():
        raise ArgumentError(f"{field} does not exist: {value!r}")
    if directory and not candidate.is_dir():
        raise ArgumentError(f"{field} must be a directory: {value!r}")
    if not directory and not candidate.is_file():
        raise ArgumentError(f"{field} must be a file: {value!r}")
    if not directory and candidate.stat().st_size > MAX_READER_FILE_BYTES:
        raise ArgumentError(f"{field} exceeds the {MAX_READER_FILE_BYTES}-byte reader limit")
    return candidate


def _import(name: str) -> Any:
    try:
        return importlib.import_module(name)
    except ModuleNotFoundError as error:
        if error.name == name or (error.name and error.name.startswith(name + ".")):
            raise OptionalDependencyUnavailable(name) from error
        raise


def _nifti_space(code: int) -> str | None:
    return {
        1: "scanner",
        2: "aligned_anatomical",
        3: "talairach",
        4: "mni152",
        5: "template_other",
    }.get(code)


def read_nifti_header(
    path: str,
    *,
    source_id: str,
    provenance: Mapping[str, Any] | None = None,
    reference_space: str | None = None,
    max_items: int = 1_000,
) -> Mapping[str, Any]:
    """Read only a NIfTI header/affine through nibabel and return the audited document."""

    candidate = _path(path, field="nifti path")
    nib = _import("nibabel")
    try:
        image = nib.load(str(candidate), mmap=True)
        header = image.header
        shape = [int(value) for value in image.shape]
        affine = image.affine.tolist()
        qform_affine, qform_code = image.get_qform(coded=True)
        sform_affine, sform_code = image.get_sform(coded=True)
        zooms = [float(value) for value in header.get_zooms()[:3]]
        axis_codes = nib.aff2axcodes(image.affine)
        space_units, time_units = header.get_xyzt_units()
        selected_code = int(sform_code or qform_code or 0)
        intent_code, _, intent_name = header.get_intent()
        projection: dict[str, Any] = {
            "image_id": str(candidate),
            "shape": shape,
            "dtype": str(image.get_data_dtype()),
            "affine": affine,
            "qform_code": int(qform_code or 0),
            "sform_code": int(sform_code or 0),
            "qform_affine": qform_affine.tolist() if qform_affine is not None else None,
            "sform_affine": sform_affine.tolist() if sform_affine is not None else None,
            "voxel_sizes": zooms,
            "axis_codes": list(axis_codes) if all(code is not None for code in axis_codes) else None,
            "coordinate_system": _nifti_space(selected_code),
            "reference_space": reference_space,
            "units": {"space": space_units or "unknown", "time": time_units or "unknown"},
            "intent": str(intent_name) if intent_name else str(intent_code),
        }
        return audit_nifti([projection], source_id=source_id, provenance=provenance, max_items=max_items).to_wire()
    except (AttributeError, TypeError, ValueError, OSError) as error:
        raise ArgumentError(f"nifti header inspection failed for {str(candidate)!r}: {error}") from error
    finally:
        try:
            image.uncache()  # type: ignore[union-attr]
        except (AttributeError, UnboundLocalError):
            pass


def _dtype(value: Any) -> str:
    text = str(value)
    if text == "category":
        return text
    normalized = text.replace("<", "").replace(">", "").replace("|", "").lower()
    if "string" in normalized or normalized in {"str", "unicode"}:
        return "string"
    if "object" in normalized:
        return "object"
    return normalized


def _index_values(index: Any, *, field: str) -> list[str]:
    size = len(index)
    if size > MAX_READER_INDEX_VALUES:
        raise ArgumentError(f"{field} has {size} values; refusing to materialize more than {MAX_READER_INDEX_VALUES}")
    return [str(value) for value in index]


def _column_projection(column: Any, length: int) -> dict[str, Any]:
    projection: dict[str, Any] = {
        "length": length,
        "dtype": _dtype(getattr(column, "dtype", "object")),
        "missing_count": int(column.isna().sum()) if hasattr(column, "isna") else 0,
    }
    dtype = getattr(column, "dtype", None)
    categories = getattr(dtype, "categories", None)
    if categories is not None:
        if len(categories) <= MAX_READER_CATEGORIES:
            projection["categories"] = [str(value) for value in categories]
        else:
            projection["category_count"] = len(categories)
    return projection


def _matrix_projection(value: Any, *, shape: tuple[int, int]) -> dict[str, Any]:
    matrix_format = "dense"
    module_name = type(value).__module__
    class_name = type(value).__name__.lower()
    if "sparse" in module_name or class_name in {"csr_matrix", "csc_matrix", "coo_matrix"}:
        if class_name.startswith("csr"):
            matrix_format = "csr"
        elif class_name.startswith("csc"):
            matrix_format = "csc"
        elif class_name.startswith("coo"):
            matrix_format = "coo"
    projection: dict[str, Any] = {"shape": [int(shape[0]), int(shape[1])], "dtype": _dtype(getattr(value, "dtype", "object")), "format": matrix_format}
    if matrix_format != "dense":
        projection["nnz"] = int(getattr(value, "nnz", 0))
        if matrix_format in {"csr", "csc"}:
            projection["indptr_length"] = len(value.indptr)
            projection["indices_length"] = len(value.indices)
            projection["sorted_indices"] = bool(getattr(value, "has_sorted_indices", False))
        else:
            projection["coordinate_length"] = len(value.data)
    return projection


def _named_matrix_mapping(mapping: Any, *, shape_axis: int) -> dict[str, dict[str, Any]]:
    result: dict[str, dict[str, Any]] = {}
    for name in mapping.keys():
        value = mapping[name]
        shape = tuple(int(item) for item in value.shape)
        if len(shape) != 2:
            raise ArgumentError(f"matrix {name!r} is not two-dimensional")
        result[str(name)] = _matrix_projection(value, shape=shape)
    return result


def _safe_kind(value: Any) -> str:
    if value is None:
        return "null"
    if isinstance(value, Mapping):
        return "mapping"
    if isinstance(value, (list, tuple)):
        return "sequence"
    if isinstance(value, (str, bytes)):
        return "string"
    if isinstance(value, (bool, int, float)):
        return "scalar"
    return type(value).__name__


def _zarr_matrix(value: Any) -> dict[str, Any]:
    """Project a dense Zarr array or an AnnData sparse-matrix group without reading values."""

    if hasattr(value, "shape") and hasattr(value, "dtype"):
        return {"shape": [int(item) for item in value.shape], "dtype": _dtype(value.dtype), "format": "dense"}
    attrs = dict(getattr(value, "attrs", {}))
    encoding = str(attrs.get("encoding-type", ""))
    matrix_format = {"csr_matrix": "csr", "csc_matrix": "csc", "coo_matrix": "coo"}.get(encoding)
    if matrix_format is None:
        raise ArgumentError(f"unsupported Zarr matrix encoding {encoding!r}")
    shape = attrs.get("shape")
    if shape is None and "shape" in value:
        shape = value["shape"]
    if not isinstance(shape, (list, tuple)) or len(shape) != 2:
        raise ArgumentError("Zarr sparse matrix is missing a two-dimensional shape")
    data = value["data"]
    result: dict[str, Any] = {
        "shape": [int(shape[0]), int(shape[1])],
        "dtype": _dtype(data.dtype),
        "format": matrix_format,
        "nnz": int(data.shape[0]),
    }
    if matrix_format in {"csr", "csc"}:
        result["indptr_length"] = int(value["indptr"].shape[0])
        result["indices_length"] = int(value["indices"].shape[0])
    else:
        result["coordinate_length"] = int(data.shape[0])
    return result


def _zarr_index(group: Any, name: str) -> list[str]:
    index_name = group.attrs.get("_index", "_index")
    values = group[index_name]
    size = int(values.shape[0])
    if size > MAX_READER_INDEX_VALUES:
        raise ArgumentError(f"{name} has {size} values; refusing to materialize more than {MAX_READER_INDEX_VALUES}")
    raw = values[:]
    if hasattr(raw, "tolist"):
        raw = raw.tolist()
    return [item.decode("utf-8") if isinstance(item, bytes) else str(item) for item in raw]


def _zarr_column(value: Any, length: int) -> dict[str, Any]:
    attrs = dict(getattr(value, "attrs", {}))
    encoding = str(attrs.get("encoding-type", ""))
    if encoding == "categorical" and "categories" in value:
        categories = value["categories"]
        result: dict[str, Any] = {"length": length, "dtype": "category"}
        if int(categories.shape[0]) <= MAX_READER_CATEGORIES:
            raw = categories[:]
            if hasattr(raw, "tolist"):
                raw = raw.tolist()
            result["categories"] = [item.decode("utf-8") if isinstance(item, bytes) else str(item) for item in raw]
        else:
            result["category_count"] = int(categories.shape[0])
        return result
    if hasattr(value, "dtype"):
        return {"length": length, "dtype": _dtype(value.dtype)}
    raise ArgumentError("unsupported Zarr dataframe column encoding")


def _zarr_dataframe(group: Any, *, axis_name: str) -> tuple[list[str], dict[str, dict[str, Any]]]:
    index = _zarr_index(group, f"{axis_name}_index")
    index_name = group.attrs.get("_index", "_index")
    columns: dict[str, dict[str, Any]] = {}
    ordered = group.attrs.get("column-order", list(group.keys()))
    for name in ordered:
        if name == index_name or name not in group:
            continue
        columns[str(name)] = _zarr_column(group[name], len(index))
    return index, columns


def _zarr_named_matrices(group: Any) -> dict[str, dict[str, Any]]:
    return {str(name): _zarr_matrix(group[name]) for name in group.keys()}


def _zarr_dataset(group: Any) -> dict[str, Any]:
    obs_index, obs_columns = _zarr_dataframe(group["obs"], axis_name="obs")
    var_index, var_columns = _zarr_dataframe(group["var"], axis_name="var")
    x = _zarr_matrix(group["X"])
    dataset: dict[str, Any] = {
        "n_obs": len(obs_index),
        "n_vars": len(var_index),
        "X": x,
        "obs_index": obs_index,
        "var_index": var_index,
        "obs": obs_columns,
        "var": var_columns,
        "layers": _zarr_named_matrices(group["layers"]),
        "obsm": _zarr_named_matrices(group["obsm"]),
        "varm": _zarr_named_matrices(group["varm"]),
        "obsp": _zarr_named_matrices(group["obsp"]),
        "varp": _zarr_named_matrices(group["varp"]),
        "uns": {str(name): {"zarr_kind": type(group["uns"][name]).__name__} for name in group["uns"].keys()},
    }
    raw = group.get("raw")
    if raw is not None and hasattr(raw, "keys") and "X" in raw and "var" in raw:
        raw_index, _ = _zarr_dataframe(raw["var"], axis_name="raw.var")
        raw_shape = [len(obs_index), len(raw_index)]
        dataset["raw"] = {"n_vars": len(raw_index), "shape": raw_shape, "var_index": raw_index}
    return dataset


def read_anndata_projection(
    path: str,
    *,
    source_id: str,
    provenance: Mapping[str, Any] | None = None,
    storage_format: str = "auto",
    max_items: int = 1_000,
) -> Mapping[str, Any]:
    """Read bounded AnnData metadata from H5AD-backed or Zarr-backed storage."""

    candidate = _path(path, field="anndata path", directory=Path(path).is_dir())
    normalized_format = storage_format.lower()
    if normalized_format == "auto":
        normalized_format = "zarr" if candidate.is_dir() or candidate.name.endswith(".zarr") else "h5ad"
    if normalized_format not in {"h5ad", "zarr"}:
        raise ArgumentError("storage_format must be auto, h5ad, or zarr")
    if normalized_format == "zarr":
        zarr = _import("zarr")
        group = zarr.open_group(str(candidate), mode="r")
        dataset = _zarr_dataset(group)
        return audit_anndata(dataset, source_id=source_id, provenance=provenance, max_items=max_items).to_wire()
    anndata = _import("anndata")
    adata = anndata.read_h5ad(str(candidate), backed="r")
    try:
        n_obs, n_vars = (int(adata.shape[0]), int(adata.shape[1]))
        obs_index = _index_values(adata.obs_names, field="obs_index")
        var_index = _index_values(adata.var_names, field="var_index")
        dataset: dict[str, Any] = {
            "n_obs": n_obs,
            "n_vars": n_vars,
            "X": _matrix_projection(adata.X, shape=(n_obs, n_vars)),
            "obs_index": obs_index,
            "var_index": var_index,
            "obs": {str(name): _column_projection(adata.obs[name], n_obs) for name in adata.obs.columns},
            "var": {str(name): _column_projection(adata.var[name], n_vars) for name in adata.var.columns},
            "layers": _named_matrix_mapping(adata.layers, shape_axis=n_obs),
            "obsm": _named_matrix_mapping(adata.obsm, shape_axis=n_obs),
            "varm": _named_matrix_mapping(adata.varm, shape_axis=n_vars),
            "obsp": _named_matrix_mapping(adata.obsp, shape_axis=n_obs),
            "varp": _named_matrix_mapping(adata.varp, shape_axis=n_vars),
            "uns": {str(name): {"kind": _safe_kind(value)} for name, value in adata.uns.items()},
        }
        raw = getattr(adata, "raw", None)
        if raw is not None:
            raw_index = _index_values(raw.var_names, field="raw.var_index")
            dataset["raw"] = {"n_vars": int(raw.shape[1]), "shape": [int(raw.shape[0]), int(raw.shape[1])], "var_index": raw_index}
        return audit_anndata(dataset, source_id=source_id, provenance=provenance, max_items=max_items).to_wire()
    finally:
        file_handle = getattr(adata, "file", None)
        close = getattr(file_handle, "close", None)
        if callable(close):
            close()


__all__ = [
    "MAX_READER_CATEGORIES",
    "MAX_READER_FILE_BYTES",
    "MAX_READER_INDEX_VALUES",
    "OptionalDependencyUnavailable",
    "read_anndata_projection",
    "read_nifti_header",
]
