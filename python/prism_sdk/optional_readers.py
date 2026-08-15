"""Verified and dependency-gated readers for bounded raw scientific and clinical sources.

These bindings are intentionally thin: they inspect bounded headers/metadata and immediately feed
the result into the dependency-free projection auditors. They never call ``get_fdata`` or materialize
matrix values, and a missing optional dependency becomes a typed runtime refusal rather than an
implicit fallback. FHIR JSON is dependency-free but follows the same raw-file boundary and duplicate-
key protections.
"""

from __future__ import annotations

import importlib
from pathlib import Path
from typing import Any, Mapping

from .alignment import MAX_ALIGNMENT_ITEMS, audit_alignments
from .anndata import audit_anndata
from .authoring import content_digest
from .dicom import audit_dicom
from .errors import ArgumentError
from .fhir import MAX_FHIR_BYTES, parse_fhir_json, parse_fhir_ndjson
from .nifti import audit_nifti
from .ome_zarr import audit_ome_zarr
from .vcf import MAX_VCF_ITEMS, MAX_VCF_RECORDS, parse_vcf


MAX_READER_FILE_BYTES = 4_000_000_000
MAX_READER_INDEX_VALUES = 1_000_000
MAX_READER_CATEGORIES = 10_000
MAX_READER_RECORDS = 100_000


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


def _input_files(path: str, *, field: str) -> list[Path]:
    candidate = _path(path, field=field, directory=Path(path).is_dir())
    if candidate.is_file():
        return [candidate]
    files = [item for item in sorted(candidate.rglob("*")) if item.is_file()]
    if not files:
        raise ArgumentError(f"{field} directory contains no files")
    if len(files) > MAX_READER_RECORDS:
        raise ArgumentError(f"{field} directory contains more than {MAX_READER_RECORDS} files")
    for item in files:
        if item.stat().st_size > MAX_READER_FILE_BYTES:
            raise ArgumentError(f"{field} member exceeds the {MAX_READER_FILE_BYTES}-byte reader limit: {item}")
    return files


def _dicom_value(dataset: Any, name: str, default: Any = None) -> Any:
    value = getattr(dataset, name, default)
    if value is None:
        return default
    if isinstance(value, (str, int, float, bool)):
        return value
    if hasattr(value, "tolist"):
        return value.tolist()
    try:
        return list(value)
    except TypeError:
        return str(value)


def read_dicom_projection(
    path: str,
    *,
    source_id: str,
    provenance: Mapping[str, Any] | None = None,
    max_items: int = 1_000,
) -> Mapping[str, Any]:
    """Read DICOM metadata with pydicom ``stop_before_pixels`` and audit the projection."""

    pydicom = _import("pydicom")
    files = _input_files(path, field="dicom path")
    projections: list[dict[str, Any]] = []
    try:
        for item in files:
            dataset = pydicom.dcmread(str(item), stop_before_pixels=True, force=False)
            file_meta = getattr(dataset, "file_meta", None)
            transfer_syntax = getattr(file_meta, "TransferSyntaxUID", None) if file_meta is not None else None
            frames = int(_dicom_value(dataset, "NumberOfFrames", 1) or 1)
            projection: dict[str, Any] = {
                "instance_id": str(item),
                "study_uid": _dicom_value(dataset, "StudyInstanceUID"),
                "series_uid": _dicom_value(dataset, "SeriesInstanceUID"),
                "sop_instance_uid": _dicom_value(dataset, "SOPInstanceUID"),
                "sop_class_uid": _dicom_value(dataset, "SOPClassUID"),
                "frame_of_reference_uid": _dicom_value(dataset, "FrameOfReferenceUID"),
                "transfer_syntax_uid": str(transfer_syntax) if transfer_syntax is not None else None,
                "modality": _dicom_value(dataset, "Modality"),
                "rows": _dicom_value(dataset, "Rows"),
                "columns": _dicom_value(dataset, "Columns"),
                "number_of_frames": frames,
                "instance_number": _dicom_value(dataset, "InstanceNumber"),
                "pixel_spacing": _dicom_value(dataset, "PixelSpacing"),
                "image_orientation_patient": _dicom_value(dataset, "ImageOrientationPatient"),
                "image_position_patient": _dicom_value(dataset, "ImagePositionPatient"),
                "spacing_between_slices": _dicom_value(dataset, "SpacingBetweenSlices"),
                "tags": {},
            }
            per_frame = getattr(dataset, "PerFrameFunctionalGroupsSequence", None)
            if per_frame is not None and frames > 1:
                positions: list[Any] = []
                for frame_group in per_frame:
                    plane_position = getattr(frame_group, "PlanePositionSequence", None)
                    if plane_position:
                        positions.append(_dicom_value(plane_position[0], "ImagePositionPatient"))
                if len(positions) == frames:
                    projection["per_frame_positions"] = positions
            projections.append(projection)
    except Exception as error:  # noqa: BLE001 - turn reader failures into a bounded SDK error
        raise ArgumentError(f"DICOM metadata inspection failed for {path!r}: {error}") from error
    return audit_dicom(projections, source_id=source_id, provenance=provenance, max_items=max_items).to_wire()


def read_indexed_vcf(
    path: str,
    *,
    source_id: str,
    reference_build: str | None = None,
    provenance: Mapping[str, Any] | None = None,
    max_records: int = MAX_VCF_RECORDS,
    max_items: int = MAX_VCF_ITEMS,
) -> Mapping[str, Any]:
    """Read indexed/compressed VCF or BCF records with pysam and audit the bounded text projection."""

    if isinstance(max_records, bool) or not isinstance(max_records, int) or not 1 <= max_records <= MAX_VCF_RECORDS:
        raise ArgumentError(f"max_records must be between 1 and {MAX_VCF_RECORDS}")
    if isinstance(max_items, bool) or not isinstance(max_items, int) or not 1 <= max_items <= MAX_VCF_ITEMS:
        raise ArgumentError(f"max_items must be between 1 and {MAX_VCF_ITEMS}")
    candidate = _path(path, field="indexed VCF path")
    pysam = _import("pysam")
    variant_file = None
    try:
        variant_file = pysam.VariantFile(str(candidate), "r")
        header_text = str(variant_file.header).rstrip("\n")
        lines: list[str] = []
        for index, record in enumerate(variant_file.fetch()):
            if index >= max_records:
                raise ArgumentError(f"indexed VCF contains more than the max_records limit of {max_records}")
            lines.append(str(record).rstrip("\n"))
        text = header_text + ("\n" if header_text else "") + "\n".join(lines) + "\n"
        document = parse_vcf(
            text,
            source_id=source_id,
            reference_build=reference_build,
            provenance=provenance,
            max_records=max_records,
            max_items=max_items,
        ).to_wire()
        manifest = dict(document["manifest"])
        manifest.update({"indexed": True, "reader": "pysam", "representation_digest": content_digest({"header": header_text, "record_count": len(lines)})})
        document["manifest"] = manifest
        document["document_digest"] = content_digest(document)
        return document
    except ArgumentError:
        raise
    except Exception as error:  # noqa: BLE001
        raise ArgumentError(f"indexed VCF inspection failed for {path!r}: {error}") from error
    finally:
        if variant_file is not None:
            variant_file.close()


def read_alignment_file(
    path: str,
    *,
    source_id: str,
    reference_build: str | None = None,
    provenance: Mapping[str, Any] | None = None,
    reference_fasta: str | None = None,
    require_index: bool = True,
    max_records: int = MAX_READER_RECORDS,
    max_items: int = MAX_ALIGNMENT_ITEMS,
) -> Mapping[str, Any]:
    """Read bounded BAM/CRAM alignment metadata with pysam, requiring an index by default."""

    if isinstance(max_records, bool) or not isinstance(max_records, int) or not 1 <= max_records <= MAX_READER_RECORDS:
        raise ArgumentError(f"max_records must be between 1 and {MAX_READER_RECORDS}")
    candidate = _path(path, field="alignment path")
    pysam = _import("pysam")
    alignment_file = None
    try:
        kwargs: dict[str, Any] = {}
        if reference_fasta is not None:
            kwargs["reference_filename"] = reference_fasta
        alignment_file = pysam.AlignmentFile(str(candidate), "rb", **kwargs)
        if require_index and hasattr(alignment_file, "has_index") and not alignment_file.has_index():
            raise ArgumentError("alignment route requires an index for coordinate-bounded iteration")
        references = {name: int(length) for name, length in zip(alignment_file.references, alignment_file.lengths)}
        records: list[dict[str, Any]] = []
        iterator = alignment_file.fetch(until_eof=not require_index)
        for index, segment in enumerate(iterator):
            if index >= max_records:
                raise ArgumentError(f"alignment contains more than the max_records limit of {max_records}")
            read_group = None
            try:
                read_group = segment.get_tag("RG")
            except (KeyError, ValueError):
                pass
            mate_reference = segment.next_reference_name
            if mate_reference in {None, "*"}:
                mate_reference = None
            records.append(
                {
                    "record_id": f"record-{index}",
                    "read_id": segment.query_name or f"read-{index}",
                    "reference_name": segment.reference_name,
                    "start": segment.reference_start if segment.reference_start >= 0 else None,
                    "reference_end": segment.reference_end,
                    "cigar": segment.cigarstring,
                    "flags": int(segment.flag),
                    "mapping_quality": int(segment.mapping_quality),
                    "sequence_length": segment.query_length,
                    "mate_reference_name": mate_reference,
                    "mate_start": segment.next_reference_start if segment.next_reference_start >= 0 else None,
                    "template_length": int(segment.template_length),
                    "read_group": read_group,
                }
            )
        return audit_alignments(
            references,
            records,
            source_id=source_id,
            reference_build=reference_build,
            provenance=provenance,
            max_records=max_records,
            max_items=max_items,
        ).to_wire()
    except ArgumentError:
        raise
    except Exception as error:  # noqa: BLE001
        raise ArgumentError(f"alignment inspection failed for {path!r}: {error}") from error
    finally:
        if alignment_file is not None:
            alignment_file.close()


def read_fhir_json(
    path: str,
    *,
    source_id: str,
    provenance: Mapping[str, Any] | None = None,
    max_items: int = 1_000,
) -> Mapping[str, Any]:
    """Read bounded UTF-8 FHIR JSON and delegate to the dependency-free FHIR auditor."""

    candidate = _path(path, field="FHIR JSON path")
    if candidate.stat().st_size > MAX_FHIR_BYTES:
        raise ArgumentError(f"FHIR JSON path exceeds the {MAX_FHIR_BYTES}-byte reader limit")
    try:
        return parse_fhir_json(
            candidate.read_bytes(),
            source_id=source_id,
            provenance=provenance,
            max_items=max_items,
        ).to_wire()
    except ArgumentError:
        raise
    except OSError as error:
        raise ArgumentError(f"FHIR JSON read failed for {str(candidate)!r}: {error}") from error


def read_fhir_ndjson(
    path: str,
    *,
    source_id: str,
    provenance: Mapping[str, Any] | None = None,
    max_records: int = 100_000,
    max_items: int = 1_000,
) -> Mapping[str, Any]:
    """Read bounded UTF-8 FHIR Bulk Data NDJSON and audit all resource records."""

    candidate = _path(path, field="FHIR NDJSON path")
    if candidate.stat().st_size > MAX_FHIR_BYTES:
        raise ArgumentError(f"FHIR NDJSON path exceeds the {MAX_FHIR_BYTES}-byte reader limit")
    try:
        return parse_fhir_ndjson(
            candidate.read_bytes(),
            source_id=source_id,
            provenance=provenance,
            max_records=max_records,
            max_items=max_items,
        ).to_wire()
    except ArgumentError:
        raise
    except OSError as error:
        raise ArgumentError(f"FHIR NDJSON read failed for {str(candidate)!r}: {error}") from error


def read_ome_zarr(
    path: str,
    *,
    source_id: str,
    provenance: Mapping[str, Any] | None = None,
    max_items: int = 1_000,
) -> Mapping[str, Any]:
    """Read only OME-Zarr group attributes and array metadata through zarr."""

    candidate = _path(path, field="OME-Zarr path", directory=True)
    zarr = _import("zarr")
    try:
        group = zarr.open_group(str(candidate), mode="r")
        attrs = dict(group.attrs)
        multiscales = attrs.get("multiscales", [])
        if not isinstance(multiscales, list):
            multiscales = list(multiscales) if isinstance(multiscales, tuple) else []
        projection: dict[str, Any] = {"multiscales": [], "omero": attrs.get("omero"), "labels": attrs.get("labels", {})}
        for multiscale in multiscales:
            if not isinstance(multiscale, Mapping):
                projection["multiscales"].append(multiscale)
                continue
            datasets: list[dict[str, Any]] = []
            for dataset in multiscale.get("datasets", []):
                if not isinstance(dataset, Mapping):
                    datasets.append(dataset)
                    continue
                dataset_path = dataset.get("path")
                array = group[dataset_path] if isinstance(dataset_path, str) else None
                if array is None:
                    datasets.append(dict(dataset))
                    continue
                if hasattr(array, "shape") and hasattr(array, "chunks"):
                    row = dict(dataset)
                    row.update({"shape": [int(item) for item in array.shape], "chunks": [int(item) for item in array.chunks], "dtype": str(array.dtype)})
                    datasets.append(row)
                else:
                    datasets.append(dict(dataset))
            row = dict(multiscale)
            row["datasets"] = datasets
            projection["multiscales"].append(row)
        return audit_ome_zarr(projection, source_id=source_id, provenance=provenance, max_items=max_items).to_wire()
    except ArgumentError:
        raise
    except Exception as error:  # noqa: BLE001
        raise ArgumentError(f"OME-Zarr metadata inspection failed for {path!r}: {error}") from error


__all__ = [
    "MAX_READER_CATEGORIES",
    "MAX_READER_FILE_BYTES",
    "MAX_READER_INDEX_VALUES",
    "MAX_READER_RECORDS",
    "OptionalDependencyUnavailable",
    "read_alignment_file",
    "read_anndata_projection",
    "read_dicom_projection",
    "read_fhir_json",
    "read_fhir_ndjson",
    "read_indexed_vcf",
    "read_nifti_header",
    "read_ome_zarr",
]
