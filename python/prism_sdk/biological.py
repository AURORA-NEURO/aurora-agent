"""Cross-domain biological adapter planning.

The Rust adapter crate owns the stable contract and semantic-loss vocabulary. This module gives
Python callers the same planning model plus an optional local dependency check, without importing
heavy scientific packages merely to discover whether they exist. It intentionally does not parse
DICOM, NIfTI, AnnData, VCF, BAM/CRAM, OME-Zarr, or FHIR: those readers belong behind the selected
adapter and must produce a source-specific loss audit before their output is publishable.
"""

from __future__ import annotations

import importlib.util
from dataclasses import dataclass
from enum import Enum
from typing import Any, Iterable, Mapping, Sequence

from .capability import _route_mapping, _route_strings, _route_text, _tool_payload
from .errors import ArgumentError


MAX_SOURCE_ID_BYTES = 512
MAX_FORMAT_BYTES = 256
MAX_DEPENDENCIES = 128
MAX_CANDIDATES = 64
REGISTRY_SCHEMA = "bioprism-adapter-registry/0.1"


class SourceKind(str, Enum):
    BYTES = "bytes"
    DIRECTORY = "directory"


class AdapterExecution(str, Enum):
    NATIVE = "native"
    PYTHON_DELEGATED = "python_delegated"


class PlanStatus(str, Enum):
    READY = "ready"
    DEPENDENCY_UNKNOWN = "dependency_unknown"
    DEPENDENCY_MISSING = "dependency_missing"
    UNSUPPORTED_FORMAT = "unsupported_format"
    UNSUPPORTED_SOURCE_KIND = "unsupported_source_kind"
    UNSUPPORTED_CONFORMANCE = "unsupported_conformance"

    @property
    def executable(self) -> bool:
        return self is PlanStatus.READY


class ConformanceLevel(str, Enum):
    PARSE = "parse"
    NORMALIZE = "normalize"
    EXECUTE = "execute"
    STREAM = "stream"
    REPLAY = "replay"


def _text(name: str, value: str, maximum: int) -> None:
    if not isinstance(value, str) or not value.strip():
        raise ArgumentError(f"{name} must be a non-empty string")
    if len(value.encode("utf-8")) > maximum:
        raise ArgumentError(f"{name} exceeds the {maximum}-byte limit")
    if any(character.isspace() and character not in " \t" for character in value):
        raise ArgumentError(f"{name} must not contain control whitespace")


def _format(value: str) -> str:
    return value.strip().lower()


def _level(value: ConformanceLevel | str) -> ConformanceLevel:
    if isinstance(value, ConformanceLevel):
        return value
    try:
        return ConformanceLevel(value)
    except ValueError as error:
        raise ArgumentError(f"unsupported conformance level: {value!r}") from error


@dataclass(frozen=True)
class AdapterDescriptor:
    """One stable adapter route and the loss surface it promises to audit."""

    id: str
    version: str
    execution: AdapterExecution
    accepted_formats: tuple[str, ...]
    accepts_undeclared_format: bool
    source_kinds: frozenset[SourceKind]
    conformance_level: ConformanceLevel
    declared_loss_kinds: frozenset[str]
    scope_dimensions: frozenset[str]
    optional_dependency: str | None
    description: str

    def to_wire(self) -> dict[str, Any]:
        result: dict[str, Any] = {
            "id": self.id,
            "version": self.version,
            "execution": self.execution.value,
            "accepted_formats": list(self.accepted_formats),
            "accepts_undeclared_format": self.accepts_undeclared_format,
            "source_kinds": sorted(kind.value for kind in self.source_kinds),
            "conformance_level": self.conformance_level.value,
            "declared_loss_kinds": sorted(self.declared_loss_kinds),
            "scope_dimensions": sorted(self.scope_dimensions),
            "description": self.description,
        }
        if self.optional_dependency is not None:
            result["optional_dependency"] = self.optional_dependency
        return result


@dataclass(frozen=True)
class AdapterPlanRequest:
    """Bounded source description used by both local planning and MCP transport."""

    source_id: str
    source_kind: SourceKind | str
    declared_format: str | None = None
    required_conformance: ConformanceLevel | str | None = None
    available_dependencies: Sequence[str] | None = None

    def __post_init__(self) -> None:
        _text("source_id", self.source_id, MAX_SOURCE_ID_BYTES)
        try:
            kind = self.source_kind if isinstance(self.source_kind, SourceKind) else SourceKind(self.source_kind)
        except ValueError as error:
            raise ArgumentError(f"unsupported source kind: {self.source_kind!r}") from error
        object.__setattr__(self, "source_kind", kind)
        if self.declared_format is not None:
            _text("declared_format", self.declared_format, MAX_FORMAT_BYTES)
            object.__setattr__(self, "declared_format", _format(self.declared_format))
        if self.required_conformance is not None:
            object.__setattr__(self, "required_conformance", _level(self.required_conformance))
        if self.available_dependencies is not None:
            if isinstance(self.available_dependencies, (str, bytes)):
                raise ArgumentError("available_dependencies must be a sequence of names")
            if len(self.available_dependencies) > MAX_DEPENDENCIES:
                raise ArgumentError(f"available_dependencies must contain at most {MAX_DEPENDENCIES} names")
            dependencies = set()
            for dependency in self.available_dependencies:
                _text("available_dependencies item", dependency, MAX_FORMAT_BYTES)
                dependencies.add(dependency.strip())
            object.__setattr__(self, "available_dependencies", tuple(sorted(dependencies)))

    def to_mcp_arguments(self) -> dict[str, Any]:
        result: dict[str, Any] = {
            "source_id": self.source_id,
            "source_kind": self.source_kind.value,
        }
        if self.declared_format is not None:
            result["declared_format"] = self.declared_format
        if self.required_conformance is not None:
            result["required_conformance"] = self.required_conformance.value
        if self.available_dependencies is not None:
            result["available_dependencies"] = list(self.available_dependencies)
        return result


@dataclass(frozen=True)
class AdapterPlanCandidate:
    adapter: AdapterDescriptor
    status: PlanStatus
    reasons: tuple[str, ...]

    def to_wire(self) -> dict[str, Any]:
        return {
            "adapter": self.adapter.to_wire(),
            "status": self.status.value,
            "reasons": list(self.reasons),
        }


@dataclass(frozen=True)
class AdapterPlan:
    request: AdapterPlanRequest
    selected_adapter: AdapterDescriptor | None
    candidates: tuple[AdapterPlanCandidate, ...]
    limitations: tuple[str, ...]

    @property
    def executable(self) -> bool:
        return self.selected_adapter is not None

    def to_wire(self) -> dict[str, Any]:
        return {
            "schema": REGISTRY_SCHEMA,
            "request": self.request.to_mcp_arguments(),
            "selected_adapter": self.selected_adapter.to_wire() if self.selected_adapter else None,
            "executable": self.executable,
            "candidates": [candidate.to_wire() for candidate in self.candidates],
            "limitations": list(self.limitations),
        }


def _adapter_report_bool(name: str, value: Any) -> bool:
    if not isinstance(value, bool):
        raise ArgumentError(f"{name} must be a boolean")
    return value


def _validate_adapter_summary(value: Mapping[str, Any]) -> dict[str, Any]:
    """Validate the compact selected-adapter summary in the outer envelope."""

    raw = _route_mapping("adapter plan selected_adapter", value)
    execution = _route_text("adapter selected execution", raw.get("execution"))
    if execution not in {item.value for item in AdapterExecution}:
        raise ArgumentError(f"unknown adapter selected execution: {execution!r}")
    conformance = _route_text("adapter selected conformance_level", raw.get("conformance_level"))
    if conformance not in {item.value for item in ConformanceLevel}:
        raise ArgumentError(f"unknown adapter selected conformance level: {conformance!r}")
    dependency = raw.get("optional_dependency")
    if dependency is not None:
        _route_text("adapter selected optional_dependency", dependency)
    _route_text("adapter selected id", raw.get("id"))
    _route_text("adapter selected version", raw.get("version"))
    _route_strings("adapter selected declared_loss_kinds", raw.get("declared_loss_kinds", []))
    _route_strings("adapter selected scope_dimensions", raw.get("scope_dimensions", []))
    return raw


@dataclass(frozen=True)
class AdapterDescriptorReport:
    """Transport projection of one adapter route and its semantic-loss boundary."""

    raw: dict[str, Any]
    id: str
    version: str
    execution: str
    accepted_formats: tuple[str, ...]
    accepts_undeclared_format: bool
    source_kinds: tuple[str, ...]
    conformance_level: str
    declared_loss_kinds: tuple[str, ...]
    scope_dimensions: tuple[str, ...]
    optional_dependency: str | None
    description: str

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "AdapterDescriptorReport":
        raw = _route_mapping("adapter descriptor", value)
        execution = _route_text("adapter execution", raw.get("execution"))
        if execution not in {item.value for item in AdapterExecution}:
            raise ArgumentError(f"unknown adapter execution: {execution!r}")
        source_kinds = _route_strings("adapter source_kinds", raw.get("source_kinds", []))
        if any(kind not in {item.value for item in SourceKind} for kind in source_kinds):
            raise ArgumentError("adapter source_kinds contains an unknown source kind")
        conformance_level = _route_text("adapter conformance_level", raw.get("conformance_level"))
        if conformance_level not in {item.value for item in ConformanceLevel}:
            raise ArgumentError(f"unknown adapter conformance level: {conformance_level!r}")
        raw_dependency = raw.get("optional_dependency")
        optional_dependency = None if raw_dependency is None else _route_text(
            "adapter optional_dependency", raw_dependency
        )
        return cls(
            raw=raw,
            id=_route_text("adapter id", raw.get("id")),
            version=_route_text("adapter version", raw.get("version")),
            execution=execution,
            accepted_formats=_route_strings("adapter accepted_formats", raw.get("accepted_formats", [])),
            accepts_undeclared_format=_adapter_report_bool(
                "adapter accepts_undeclared_format", raw.get("accepts_undeclared_format")
            ),
            source_kinds=source_kinds,
            conformance_level=conformance_level,
            declared_loss_kinds=_route_strings(
                "adapter declared_loss_kinds", raw.get("declared_loss_kinds", [])
            ),
            scope_dimensions=_route_strings("adapter scope_dimensions", raw.get("scope_dimensions", [])),
            optional_dependency=optional_dependency,
            description=_route_text("adapter description", raw.get("description")),
        )

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


@dataclass(frozen=True)
class AdapterPlanCandidateReport:
    """One candidate route with explicit status and refusal reasons."""

    raw: dict[str, Any]
    adapter: AdapterDescriptorReport
    status: str
    reasons: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "AdapterPlanCandidateReport":
        raw = _route_mapping("adapter plan candidate", value)
        status = _route_text("adapter candidate status", raw.get("status"))
        if status not in {item.value for item in PlanStatus}:
            raise ArgumentError(f"unknown adapter plan status: {status!r}")
        return cls(
            raw=raw,
            adapter=AdapterDescriptorReport.from_wire(raw.get("adapter")),
            status=status,
            reasons=_route_strings("adapter candidate reasons", raw.get("reasons", [])),
        )

    @property
    def executable(self) -> bool:
        return self.status == PlanStatus.READY.value

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


@dataclass(frozen=True)
class AdapterPlanProjection:
    """Full serialized plan with candidates, selected route, and limitations."""

    raw: dict[str, Any]
    request: dict[str, Any]
    selected_adapter: AdapterDescriptorReport | None
    executable: bool
    candidates: tuple[AdapterPlanCandidateReport, ...]
    limitations: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "AdapterPlanProjection":
        raw = _route_mapping("adapter plan projection", value)
        raw_selected = raw.get("selected_adapter")
        selected_adapter = None if raw_selected is None else AdapterDescriptorReport.from_wire(raw_selected)
        executable = _adapter_report_bool("adapter plan executable", raw.get("executable"))
        raw_candidates = raw.get("candidates", [])
        if not isinstance(raw_candidates, Sequence) or isinstance(raw_candidates, (str, bytes)):
            raise ArgumentError("adapter plan candidates must be an array")
        candidates = tuple(AdapterPlanCandidateReport.from_wire(candidate) for candidate in raw_candidates)
        if executable != (selected_adapter is not None):
            raise ArgumentError("adapter plan executable state does not reconcile with selected_adapter")
        return cls(
            raw=raw,
            request=_route_mapping("adapter plan request", raw.get("request", {})),
            selected_adapter=selected_adapter,
            executable=executable,
            candidates=candidates,
            limitations=_route_strings("adapter plan limitations", raw.get("limitations", [])),
        )

    @property
    def dependency_blocked(self) -> bool:
        return any(candidate.status in {
            PlanStatus.DEPENDENCY_MISSING.value,
            PlanStatus.DEPENDENCY_UNKNOWN.value,
        } for candidate in self.candidates)

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


@dataclass(frozen=True)
class AdapterPlanReport:
    """Authoritative adapter-plan envelope with typed route and loss evidence."""

    raw: dict[str, Any]
    plan_id: str
    registry: str
    executable: bool
    selected_adapter: dict[str, Any] | None
    plan: AdapterPlanProjection
    execution: str
    guarantees: tuple[str, ...]
    limitations: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "AdapterPlanReport":
        raw = _route_mapping("adapter plan report", value)
        if raw.get("ok") is False:
            raise ArgumentError("adapter plan report is not successful")
        if raw.get("workflow") != "adapter_plan":
            raise ArgumentError("adapter plan workflow is invalid")
        executable = _adapter_report_bool("adapter plan report executable", raw.get("executable"))
        raw_selected = raw.get("selected_adapter")
        selected_adapter = None if raw_selected is None else _validate_adapter_summary(raw_selected)
        plan = AdapterPlanProjection.from_wire(raw.get("plan", {}))
        if executable != plan.executable or (selected_adapter is not None) != executable:
            raise ArgumentError("adapter plan envelope does not reconcile with the nested plan")
        if executable and selected_adapter is not None and plan.selected_adapter is not None:
            if selected_adapter["id"] != plan.selected_adapter.id:
                raise ArgumentError("adapter plan selected adapter ids do not reconcile")
        execution = _route_text("adapter plan execution", raw.get("execution"))
        if execution != "not_started":
            raise ArgumentError("adapter plan execution must remain not_started")
        return cls(
            raw=raw,
            plan_id=_route_text("adapter plan id", raw.get("plan_id")),
            registry=_route_text("adapter plan registry", raw.get("registry")),
            executable=executable,
            selected_adapter=selected_adapter,
            plan=plan,
            execution=execution,
            guarantees=_route_strings("adapter plan guarantees", raw.get("guarantees", [])),
            limitations=_route_strings("adapter plan report limitations", raw.get("limitations", [])),
        )

    @property
    def selected_adapter_id(self) -> str | None:
        return None if self.selected_adapter is None else self.selected_adapter.get("id")

    @property
    def candidate_count(self) -> int:
        return len(self.plan.candidates)

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


def adapter_plan_report(value: Mapping[str, Any]) -> AdapterPlanReport:
    """Parse a direct adapter-plan result or an HTTP tool envelope."""

    return AdapterPlanReport.from_wire(_tool_payload(value, "adapter_plan"))


def _descriptor(
    id: str,
    execution: AdapterExecution,
    formats: Iterable[str],
    undeclared: bool,
    source_kinds: Iterable[SourceKind],
    dependency: str | None,
    losses: Iterable[str],
    dimensions: Iterable[str],
    description: str,
) -> AdapterDescriptor:
    return AdapterDescriptor(
        id=id,
        version="0.1.0",
        execution=execution,
        accepted_formats=tuple(sorted({_format(value) for value in formats})),
        accepts_undeclared_format=undeclared,
        source_kinds=frozenset(source_kinds),
        conformance_level=ConformanceLevel.NORMALIZE,
        declared_loss_kinds=frozenset(losses),
        scope_dimensions=frozenset(dimensions),
        optional_dependency=dependency,
        description=description,
    )


def _builtin_descriptors() -> tuple[AdapterDescriptor, ...]:
    common_image_losses = (
        "coordinate_frame_not_carried",
        "precision_reduced",
        "provenance_unavailable",
        "ontology_term_unmapped",
        "content_uninterpreted",
    )
    descriptors = (
        _descriptor(
            "bioprism.tabular",
            AdapterExecution.NATIVE,
            ("text/csv", "text/tab-separated-values", "text/tsv"),
            True,
            (SourceKind.BYTES,),
            None,
            ("unmapped_column", "unpreserved_unit", "coordinate_frame_not_carried", "precision_reduced", "provenance_unavailable", "ontology_term_unmapped", "type_undetermined"),
            ("subject", "specimen", "observation"),
            "Validated CSV/TSV normalization under an explicit mapping profile.",
        ),
        _descriptor(
            "bioprism.inventory",
            AdapterExecution.NATIVE,
            ("application/x-directory", "inode/directory"),
            True,
            (SourceKind.DIRECTORY,),
            None,
            ("content_uninterpreted", "provenance_unavailable"),
            ("repository", "artifact"),
            "Deterministic artifact inventory with hashes and explicit unread-content loss.",
        ),
        _descriptor(
            "bioprism.python.dicom",
            AdapterExecution.PYTHON_DELEGATED,
            ("application/dicom", "application/dicom+json"),
            False,
            (SourceKind.BYTES, SourceKind.DIRECTORY),
            "pydicom",
            ("unpreserved_unit", *common_image_losses),
            ("subject", "specimen", "acquisition", "image"),
            "Python-owned DICOM adapter route; the Rust layer plans and audits the boundary.",
        ),
        _descriptor(
            "bioprism.python.dicom_metadata",
            AdapterExecution.PYTHON_DELEGATED,
            ("application/dicom-manifest",),
            False,
            (SourceKind.BYTES,),
            None,
            ("coordinate_frame_not_carried", "provenance_unavailable", "content_uninterpreted", "type_undetermined"),
            ("subject", "specimen", "acquisition", "image"),
            "Dependency-free audit of parsed DICOM identity, study/series hierarchy, frame geometry, and provenance; pixels remain uninterpreted.",
        ),
        _descriptor(
            "bioprism.python.bids_manifest",
            AdapterExecution.PYTHON_DELEGATED,
            ("application/bids-manifest",),
            False,
            (SourceKind.BYTES,),
            None,
            ("coordinate_frame_not_carried", "provenance_unavailable", "content_uninterpreted", "type_undetermined"),
            ("subject", "session", "acquisition", "image", "event"),
            "Dependency-free BIDS manifest, entity, sidecar-inheritance, and participant audit; binary image bytes remain uninterpreted.",
        ),
        _descriptor(
            "bioprism.python.nifti_bids",
            AdapterExecution.PYTHON_DELEGATED,
            ("application/nifti", "application/x-nifti", "application/bids"),
            False,
            (SourceKind.BYTES, SourceKind.DIRECTORY),
            "nibabel",
            common_image_losses,
            ("subject", "session", "acquisition", "image"),
            "Python-owned NIfTI/BIDS adapter route with affine and sidecar provenance checks.",
        ),
        _descriptor(
            "bioprism.python.nifti_metadata",
            AdapterExecution.PYTHON_DELEGATED,
            ("application/nifti-manifest",),
            False,
            (SourceKind.BYTES,),
            None,
            ("coordinate_frame_not_carried", "provenance_unavailable", "content_uninterpreted", "type_undetermined"),
            ("subject", "session", "acquisition", "image", "voxel"),
            "Dependency-free audit of parsed NIfTI shape, datatype, affine, qform/sform, units, and coordinate-frame metadata; arrays remain uninterpreted.",
        ),
        _descriptor(
            "bioprism.python.anndata",
            AdapterExecution.PYTHON_DELEGATED,
            ("application/anndata", "application/h5ad", "application/zarr"),
            False,
            (SourceKind.BYTES, SourceKind.DIRECTORY),
            "anndata",
            ("unmapped_column", *common_image_losses),
            ("subject", "cell", "feature", "assay"),
            "Python-owned AnnData/Zarr adapter route preserving obs/var/uns provenance.",
        ),
        _descriptor(
            "bioprism.python.anndata_metadata",
            AdapterExecution.PYTHON_DELEGATED,
            ("application/anndata-manifest",),
            False,
            (SourceKind.BYTES,),
            None,
            ("coordinate_frame_not_carried", "provenance_unavailable", "content_uninterpreted", "type_undetermined"),
            ("subject", "cell", "feature", "assay", "embedding"),
            "Dependency-free audit of parsed AnnData/Zarr dimensions, indices, annotations, layers, embeddings, and sparse matrix metadata; payloads remain uninterpreted.",
        ),
        _descriptor(
            "bioprism.python.vcf_text",
            AdapterExecution.PYTHON_DELEGATED,
            ("text/vcf", "text/x-vcf", "application/vcf"),
            False,
            (SourceKind.BYTES,),
            None,
            ("coordinate_frame_not_carried", "precision_reduced", "provenance_unavailable", "ontology_term_unmapped", "type_undetermined", "content_uninterpreted"),
            ("subject", "sample", "variant", "genome"),
            "Dependency-free bounded text VCF adapter route requiring reference-build and sample identity checks.",
        ),
        _descriptor(
            "bioprism.python.vcf_indexed",
            AdapterExecution.PYTHON_DELEGATED,
            ("application/bcf", "application/vcf+bgzip", "application/vcf+gzip"),
            False,
            (SourceKind.BYTES,),
            "pysam",
            ("coordinate_frame_not_carried", "precision_reduced", "provenance_unavailable", "ontology_term_unmapped", "type_undetermined", "content_uninterpreted"),
            ("subject", "sample", "variant", "genome"),
            "Python-owned indexed/compressed VCF and BCF route using pysam with reference-build and sample identity checks.",
        ),
        _descriptor(
            "bioprism.python.bam_cram",
            AdapterExecution.PYTHON_DELEGATED,
            ("application/bam", "application/cram"),
            False,
            (SourceKind.BYTES,),
            "pysam",
            ("coordinate_frame_not_carried", "precision_reduced", "provenance_unavailable", "content_uninterpreted"),
            ("subject", "sample", "read", "reference"),
            "Python-owned BAM/CRAM adapter route preserving reference and alignment metadata.",
        ),
        _descriptor(
            "bioprism.python.alignment_metadata",
            AdapterExecution.PYTHON_DELEGATED,
            ("application/alignment-manifest",),
            False,
            (SourceKind.BYTES,),
            None,
            ("coordinate_frame_not_carried", "provenance_unavailable", "content_uninterpreted", "type_undetermined"),
            ("subject", "sample", "read", "reference", "locus"),
            "Dependency-free audit of parsed BAM/CRAM records, CIGAR accounting, coordinates, flags, pairing, sort order, and coverage; read payloads remain uninterpreted.",
        ),
        _descriptor(
            "bioprism.python.fastq_text",
            AdapterExecution.PYTHON_DELEGATED,
            ("application/fastq", "text/fastq", "text/x-fastq"),
            False,
            (SourceKind.BYTES,),
            None,
            ("provenance_unavailable", "content_uninterpreted", "type_undetermined"),
            ("subject", "sample", "read", "sequence", "quality"),
            "Dependency-free bounded FASTQ reader validating complete records, quality lengths, and paired-read evidence without disclosing read content.",
        ),
        _descriptor(
            "bioprism.python.sam_text",
            AdapterExecution.PYTHON_DELEGATED,
            ("application/sam", "text/sam", "text/x-sam"),
            False,
            (SourceKind.BYTES,),
            None,
            ("content_uninterpreted", "coordinate_frame_not_carried", "type_undetermined", "provenance_unavailable"),
            ("subject", "sample", "reference", "read", "alignment", "assay"),
            "Dependency-free bounded SAM reader validating headers, CIGAR semantics, coordinate bounds, mate flags, optional-tag types, and sort order without disclosing raw alignment content.",
        ),
        _descriptor(
            "bioprism.python.fasta_text",
            AdapterExecution.PYTHON_DELEGATED,
            ("application/fasta", "text/fasta", "text/x-fasta"),
            False,
            (SourceKind.BYTES,),
            None,
            ("provenance_unavailable", "content_uninterpreted", "type_undetermined"),
            ("subject", "sample", "reference", "sequence"),
            "Dependency-free bounded FASTA reader validating complete records, optional nucleotide/protein alphabets, and duplicate identifiers without disclosing sequence content.",
        ),
        _descriptor(
            "bioprism.python.gff3_text",
            AdapterExecution.PYTHON_DELEGATED,
            ("application/gff3", "text/gff3", "application/gtf", "text/x-gtf"),
            False,
            (SourceKind.BYTES,),
            None,
            ("content_uninterpreted", "coordinate_frame_not_carried", "ontology_term_unmapped", "provenance_unavailable"),
            ("subject", "sample", "reference", "feature", "interval"),
            "Dependency-free bounded GFF3/GTF reader validating coordinates, attributes, parent references, and feature hierarchy without disclosing attribute values.",
        ),
        _descriptor(
            "bioprism.python.bed_text",
            AdapterExecution.PYTHON_DELEGATED,
            ("application/bed", "text/bed", "text/x-bed"),
            False,
            (SourceKind.BYTES,),
            None,
            ("content_uninterpreted", "coordinate_frame_not_carried", "ontology_term_unmapped", "provenance_unavailable"),
            ("subject", "sample", "reference", "feature", "interval", "transcript"),
            "Dependency-free bounded BED3-BED12 reader validating zero-based intervals, thick bounds, RGB fields, block geometry, and ordering without disclosing chromosome or item labels.",
        ),
        _descriptor(
            "bioprism.python.pdb_text",
            AdapterExecution.PYTHON_DELEGATED,
            ("application/pdb", "chemical/x-pdb", "text/pdb"),
            False,
            (SourceKind.BYTES,),
            None,
            ("content_uninterpreted", "coordinate_frame_not_carried", "ontology_term_unmapped", "provenance_unavailable"),
            ("subject", "sample", "structure", "chain", "residue", "atom"),
            "Dependency-free bounded PDB fixed-column reader validating models, coordinates, chains, residues, and connectivity without disclosing raw structure records.",
        ),
        _descriptor(
            "bioprism.python.sdf_text",
            AdapterExecution.PYTHON_DELEGATED,
            ("chemical/x-mdl-sdfile", "chemical/x-mdl-molfile", "text/sdf"),
            False,
            (SourceKind.BYTES,),
            None,
            ("content_uninterpreted", "coordinate_frame_not_carried", "ontology_term_unmapped", "provenance_unavailable"),
            ("subject", "sample", "molecule", "atom", "bond", "assay"),
            "Dependency-free bounded SDF/MOL V2000 reader validating molecular graph counts, properties, connectivity, and coordinates without disclosing raw records.",
        ),
        _descriptor(
            "bioprism.python.fhir_ndjson",
            AdapterExecution.PYTHON_DELEGATED,
            ("application/fhir+ndjson",),
            False,
            (SourceKind.BYTES,),
            None,
            ("provenance_unavailable", "ontology_term_unmapped", "content_uninterpreted", "type_undetermined"),
            ("subject", "encounter", "resource", "terminology", "time"),
            "Dependency-free bounded FHIR Bulk Data NDJSON reader with complete-record validation and privacy-safe reference projection.",
        ),
        _descriptor(
            "bioprism.python.fhir_json",
            AdapterExecution.PYTHON_DELEGATED,
            ("application/fhir+json",),
            False,
            (SourceKind.BYTES,),
            None,
            ("provenance_unavailable", "ontology_term_unmapped", "content_uninterpreted", "type_undetermined"),
            ("subject", "encounter", "resource", "terminology", "time"),
            "Dependency-free bounded FHIR JSON resource and Bundle reader with privacy-safe reference projection.",
        ),
        _descriptor(
            "bioprism.python.fhir_manifest",
            AdapterExecution.PYTHON_DELEGATED,
            ("application/fhir-manifest",),
            False,
            (SourceKind.BYTES,),
            None,
            ("provenance_unavailable", "ontology_term_unmapped", "content_uninterpreted", "type_undetermined"),
            ("subject", "encounter", "resource", "terminology", "time"),
            "Dependency-free audit of parsed FHIR structure, resource identity, references, profiles, and provenance; clinical values remain uninterpreted.",
        ),
        _descriptor(
            "bioprism.python.mzml_text",
            AdapterExecution.PYTHON_DELEGATED,
            ("application/mzml", "application/xml+mass-spectrometry", "text/mzml"),
            False,
            (SourceKind.BYTES,),
            None,
            ("provenance_unavailable", "ontology_term_unmapped", "content_uninterpreted", "type_undetermined"),
            ("subject", "sample", "assay", "spectrum", "ion"),
            "Dependency-free bounded mzML XML metadata reader that audits spectra and binary-array declarations without decoding payloads.",
        ),
        _descriptor(
            "bioprism.python.ome_zarr",
            AdapterExecution.PYTHON_DELEGATED,
            ("application/ome-zarr", "application/x-zarr"),
            False,
            (SourceKind.DIRECTORY,),
            "zarr",
            common_image_losses,
            ("subject", "specimen", "image", "tile"),
            "Python-owned OME-Zarr adapter route preserving multiscale and spatial metadata.",
        ),
    )
    return tuple(sorted(descriptors, key=lambda descriptor: descriptor.id))


class AdapterRegistry:
    """Built-in adapter catalogue with optional local dependency discovery."""

    def __init__(self, descriptors: Sequence[AdapterDescriptor] | None = None) -> None:
        self._descriptors = tuple(sorted(descriptors or _builtin_descriptors(), key=lambda item: item.id))

    @property
    def descriptors(self) -> tuple[AdapterDescriptor, ...]:
        return self._descriptors

    @staticmethod
    def _installed_dependencies(descriptors: Sequence[AdapterDescriptor]) -> set[str]:
        installed: set[str] = set()
        for descriptor in descriptors:
            dependency = descriptor.optional_dependency
            if dependency is None:
                continue
            try:
                found = importlib.util.find_spec(dependency)
            except (ImportError, ModuleNotFoundError, ValueError):
                found = None
            if found is not None:
                installed.add(dependency)
        return installed

    def plan(
        self,
        request: AdapterPlanRequest,
        *,
        check_environment: bool = True,
    ) -> AdapterPlan:
        if not isinstance(request, AdapterPlanRequest):
            raise ArgumentError("request must be an AdapterPlanRequest")
        if request.available_dependencies is not None:
            dependencies: set[str] | None = set(request.available_dependencies)
        elif check_environment:
            dependencies = self._installed_dependencies(self._descriptors)
        else:
            dependencies = None

        candidates = [self._candidate(descriptor, request, dependencies) for descriptor in self._descriptors]
        order = {
            PlanStatus.READY: 0,
            PlanStatus.DEPENDENCY_UNKNOWN: 1,
            PlanStatus.DEPENDENCY_MISSING: 2,
            PlanStatus.UNSUPPORTED_FORMAT: 3,
            PlanStatus.UNSUPPORTED_SOURCE_KIND: 4,
            PlanStatus.UNSUPPORTED_CONFORMANCE: 5,
        }
        candidates.sort(key=lambda candidate: (order[candidate.status], candidate.adapter.id))
        candidates = candidates[:MAX_CANDIDATES]
        selected = next((candidate.adapter for candidate in candidates if candidate.status.executable), None)
        limitations = (
            "format matching is explicit; the planner never sniffs source bytes",
            "planning does not fetch, parse, execute, or grant credentials",
            "semantic-loss declarations describe the adapter surface; source-specific loss is only known after conformance",
        )
        if selected is not None and selected.execution is AdapterExecution.PYTHON_DELEGATED:
            limitations += ("the selected implementation is delegated to a Python adapter and must run an independent source-specific conformance audit",)
        if selected is None:
            limitations += ("no executable adapter is available for this request; change the declared format, source shape, conformance requirement, or dependency inventory",)
        return AdapterPlan(request, selected, tuple(candidates), limitations)

    @staticmethod
    def _candidate(
        descriptor: AdapterDescriptor,
        request: AdapterPlanRequest,
        dependencies: set[str] | None,
    ) -> AdapterPlanCandidate:
        reasons: list[str] = []
        if request.declared_format is None:
            format_ok = descriptor.accepts_undeclared_format
        else:
            format_ok = request.declared_format in descriptor.accepted_formats
        if not format_ok:
            reasons.append(
                "this adapter requires an explicit declared format"
                if request.declared_format is None
                else f"declared format {request.declared_format!r} is not accepted by this adapter"
            )
            return AdapterPlanCandidate(descriptor, PlanStatus.UNSUPPORTED_FORMAT, tuple(reasons))
        if request.source_kind not in descriptor.source_kinds:
            reasons.append(f"source kind {request.source_kind.value} is not supported by this adapter")
            return AdapterPlanCandidate(descriptor, PlanStatus.UNSUPPORTED_SOURCE_KIND, tuple(reasons))
        if request.required_conformance is not None and list(ConformanceLevel).index(request.required_conformance) > list(ConformanceLevel).index(descriptor.conformance_level):
            reasons.append(f"requested conformance exceeds this adapter's {descriptor.conformance_level.value} level")
            return AdapterPlanCandidate(descriptor, PlanStatus.UNSUPPORTED_CONFORMANCE, tuple(reasons))
        dependency = descriptor.optional_dependency
        if dependency is None:
            reasons.append("native adapter is available in this runtime")
            return AdapterPlanCandidate(descriptor, PlanStatus.READY, tuple(reasons))
        if dependencies is None:
            reasons.append(f"optional dependency {dependency!r} was not checked by the caller")
            return AdapterPlanCandidate(descriptor, PlanStatus.DEPENDENCY_UNKNOWN, tuple(reasons))
        if dependency not in dependencies:
            reasons.append(f"optional dependency {dependency!r} is absent from the caller inventory")
            return AdapterPlanCandidate(descriptor, PlanStatus.DEPENDENCY_MISSING, tuple(reasons))
        reasons.append(f"optional dependency {dependency!r} is present in the caller inventory")
        return AdapterPlanCandidate(descriptor, PlanStatus.READY, tuple(reasons))


def adapter_plan(
    source_id: str,
    source_kind: SourceKind | str,
    *,
    declared_format: str | None = None,
    required_conformance: ConformanceLevel | str | None = None,
    available_dependencies: Sequence[str] | None = None,
    check_environment: bool = True,
) -> AdapterPlan:
    """Plan the default cross-domain adapter registry for a caller-owned source."""

    request = AdapterPlanRequest(
        source_id=source_id,
        source_kind=source_kind,
        declared_format=declared_format,
        required_conformance=required_conformance,
        available_dependencies=available_dependencies,
    )
    return AdapterRegistry().plan(request, check_environment=check_environment)


__all__ = [
    "AdapterDescriptor",
    "AdapterDescriptorReport",
    "AdapterExecution",
    "AdapterPlan",
    "AdapterPlanCandidateReport",
    "AdapterPlanCandidate",
    "AdapterPlanProjection",
    "AdapterPlanRequest",
    "AdapterPlanReport",
    "AdapterRegistry",
    "ConformanceLevel",
    "PlanStatus",
    "REGISTRY_SCHEMA",
    "SourceKind",
    "adapter_plan",
    "adapter_plan_report",
]
