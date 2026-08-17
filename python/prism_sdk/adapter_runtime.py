"""Typed execution gateway for the concrete Python biological adapter contracts.

The registry answers *which* route could handle a source. This module answers the next practical
question: *run the selected bounded projection audit*. It deliberately does not sniff, import
optional binary readers, or silently fall back between formats. Concrete routes return their full
audit document; routes without an installed or implemented reader return an explicit typed
unsupported result.
"""

from __future__ import annotations

from collections import Counter
from dataclasses import dataclass
from enum import Enum
from typing import Any, Mapping, Sequence

from .alignment import audit_alignments
from .anndata import audit_anndata
from .authoring import content_digest
from .bed import parse_bed
from .bids import audit_bids
from .biological import AdapterDescriptor, AdapterRegistry
from .dicom import audit_dicom
from .errors import ArgumentError
from .fasta import parse_fasta
from .fastq import parse_fastq
from .fhir import audit_fhir
from .gff3 import parse_gff3
from .mzml import parse_mzml
from .pdb import parse_pdb
from .sam import parse_sam
from .sdf import parse_sdf
from .nifti import audit_nifti
from .ome_zarr import audit_ome_zarr
from .optional_readers import (
    OptionalDependencyUnavailable,
    read_alignment_file,
    read_anndata_projection,
    read_bed,
    read_dicom_projection,
    read_fasta,
    read_fastq,
    read_fhir_json,
    read_fhir_ndjson,
    read_gff3,
    read_indexed_vcf,
    read_mzml,
    read_pdb,
    read_sam,
    read_sdf,
    read_nifti_header,
    read_ome_zarr,
)
from .vcf import parse_vcf


RUNTIME_SCHEMA = "bioprism-python-adapter-runtime/0.1"
MAX_RUNTIME_ADAPTER_ID_BYTES = 256
MAX_RUNTIME_SOURCE_ID_BYTES = 512
MAX_RUNTIME_ITEMS = 1_000
MAX_RUNTIME_BATCH_REQUESTS = 64
MAX_RUNTIME_BATCH_ITEMS = 10_000
RUNTIME_BATCH_SCHEMA = "bioprism-python-adapter-batch/0.1"


class RuntimeStatus(str, Enum):
    SUCCEEDED = "succeeded"
    LOSSY = "lossy"
    INVALID = "invalid"
    BLOCKED = "blocked"
    REJECTED = "rejected"
    UNSUPPORTED = "unsupported"


class BatchStatus(str, Enum):
    """Aggregate state for a bounded heterogeneous projection batch."""

    SUCCEEDED = "succeeded"
    PARTIAL = "partial"
    BLOCKED = "blocked"
    REJECTED = "rejected"


def _text(name: str, value: str, maximum: int) -> None:
    if not isinstance(value, str) or not value.strip():
        raise ArgumentError(f"{name} must be a non-empty string")
    if len(value.encode("utf-8")) > maximum:
        raise ArgumentError(f"{name} exceeds the {maximum}-byte limit")
    if any(ord(character) < 0x20 for character in value):
        raise ArgumentError(f"{name} must not contain control characters")


@dataclass(frozen=True)
class ProjectionRequest:
    """A bounded request whose payload shape is owned by the selected adapter route.

    ``provenance`` is passed to the selected format audit and follows that adapter's accepted
    provenance schema. ``source_context`` is a separate transport-bound envelope for connector
    identity and digests; it is retained in the request identity but never passed to a parser as
    if it were scientific provenance.
    """

    adapter_id: str
    source_id: str
    payload: Mapping[str, Any]
    provenance: Mapping[str, Any] | None = None
    max_items: int = MAX_RUNTIME_ITEMS
    source_context: Mapping[str, Any] | None = None

    def __post_init__(self) -> None:
        _text("adapter_id", self.adapter_id, MAX_RUNTIME_ADAPTER_ID_BYTES)
        _text("source_id", self.source_id, MAX_RUNTIME_SOURCE_ID_BYTES)
        if not isinstance(self.payload, Mapping):
            raise ArgumentError("payload must be a mapping; its schema is selected by adapter_id")
        if self.provenance is not None and not isinstance(self.provenance, Mapping):
            raise ArgumentError("provenance must be a mapping when supplied")
        if self.source_context is not None:
            if not isinstance(self.source_context, Mapping):
                raise ArgumentError("source_context must be a mapping when supplied")
            try:
                content_digest(dict(self.source_context))
            except Exception as error:
                raise ArgumentError(f"source_context must be canonical JSON-safe: {error}") from error
        if isinstance(self.max_items, bool) or not isinstance(self.max_items, int) or not 1 <= self.max_items <= MAX_RUNTIME_ITEMS:
            raise ArgumentError(f"max_items must be between 1 and {MAX_RUNTIME_ITEMS}")

    def to_wire(self) -> dict[str, Any]:
        result = {
            "adapter_id": self.adapter_id,
            "source_id": self.source_id,
            "payload_keys": sorted(str(key) for key in self.payload.keys()),
            "provenance_present": self.provenance is not None,
            "max_items": self.max_items,
        }
        if self.source_context is not None:
            result["source_context_present"] = True
            result["source_context_digest"] = content_digest(dict(self.source_context))
        else:
            result["source_context_present"] = False
        return result


@dataclass(frozen=True)
class AdapterExecutionResult:
    """Normalized result envelope for successful audits, refusals, and invalid projections."""

    request: ProjectionRequest
    status: RuntimeStatus
    executable: bool
    adapter: AdapterDescriptor | None
    document: Mapping[str, Any] | None = None
    error: Mapping[str, Any] | None = None

    @property
    def accepted(self) -> bool:
        return self.adapter is not None

    @property
    def document_digest(self) -> str | None:
        if self.document is None:
            return None
        digest = self.document.get("document_digest")
        return digest if isinstance(digest, str) else None

    def to_wire(self) -> dict[str, Any]:
        result: dict[str, Any] = {
            "schema": RUNTIME_SCHEMA,
            "request": self.request.to_wire(),
            "status": self.status.value,
            "accepted": self.accepted,
            "executable": self.executable,
            "adapter": self.adapter.to_wire() if self.adapter else None,
            "document_digest": self.document_digest,
        }
        if self.document is not None:
            result["document"] = dict(self.document)
        if self.error is not None:
            result["error"] = dict(self.error)
        return result


@dataclass(frozen=True)
class ProjectionBatchRequest:
    """A bounded ordered set of explicit projection requests.

    The batch envelope preserves request order for reproducibility, but each member retains its
    own adapter contract, provenance, refusal state, and document digest. Request payload values
    are never echoed by ``to_wire``.
    """

    requests: tuple[ProjectionRequest, ...]
    stop_on_error: bool = False
    max_total_items: int = MAX_RUNTIME_BATCH_ITEMS

    def __post_init__(self) -> None:
        if isinstance(self.requests, (str, bytes)) or not isinstance(self.requests, Sequence):
            raise ArgumentError("requests must be a sequence of ProjectionRequest values")
        normalized = tuple(self.requests)
        if not 1 <= len(normalized) <= MAX_RUNTIME_BATCH_REQUESTS:
            raise ArgumentError(f"requests must contain between 1 and {MAX_RUNTIME_BATCH_REQUESTS} items")
        if any(not isinstance(request, ProjectionRequest) for request in normalized):
            raise ArgumentError("requests must contain only ProjectionRequest values")
        if not isinstance(self.stop_on_error, bool):
            raise ArgumentError("stop_on_error must be a boolean")
        if isinstance(self.max_total_items, bool) or not isinstance(self.max_total_items, int) or not 1 <= self.max_total_items <= MAX_RUNTIME_BATCH_ITEMS:
            raise ArgumentError(f"max_total_items must be between 1 and {MAX_RUNTIME_BATCH_ITEMS}")
        requested_items = sum(request.max_items for request in normalized)
        if requested_items > self.max_total_items:
            raise ArgumentError(
                f"sum of request max_items ({requested_items}) exceeds max_total_items ({self.max_total_items})"
            )
        object.__setattr__(self, "requests", normalized)

    def to_wire(self) -> dict[str, Any]:
        return {
            "requests": [request.to_wire() for request in self.requests],
            "stop_on_error": self.stop_on_error,
            "max_total_items": self.max_total_items,
        }


@dataclass(frozen=True)
class ProjectionBatchResult:
    """Evidence-bearing aggregate for one bounded heterogeneous projection batch."""

    request: ProjectionBatchRequest
    status: BatchStatus
    results: tuple[AdapterExecutionResult, ...]
    omitted_requests: int = 0
    stopped_on_error: bool = False

    @property
    def accepted(self) -> bool:
        return self.omitted_requests == 0 and bool(self.results) and all(result.accepted for result in self.results)

    @property
    def document_digests(self) -> tuple[str, ...]:
        return tuple(digest for result in self.results if (digest := result.document_digest) is not None)

    @property
    def batch_digest(self) -> str:
        return content_digest(self._digest_input())

    def _digest_input(self) -> dict[str, Any]:
        return {
            "schema": RUNTIME_BATCH_SCHEMA,
            "request": self.request.to_wire(),
            "status": self.status.value,
            "omitted_requests": self.omitted_requests,
            "stopped_on_error": self.stopped_on_error,
            "results": [
                {
                    "request": result.request.to_wire(),
                    "status": result.status.value,
                    "accepted": result.accepted,
                    "executable": result.executable,
                    "adapter_id": result.adapter.id if result.adapter else None,
                    "document_digest": result.document_digest,
                    "error": result.error,
                }
                for result in self.results
            ],
        }

    def _aggregate(self) -> dict[str, Any]:
        status_counts: Counter[str] = Counter(result.status.value for result in self.results)
        adapter_counts: Counter[str] = Counter()
        error_kind_counts: Counter[str] = Counter()
        loss_kind_counts: Counter[str] = Counter()
        scope_dimensions: set[str] = set()
        valid_count = 0
        publishable_count = 0
        document_count = 0
        executable_count = 0
        declared_loss_count = 0
        visible_loss_entry_count = 0
        omitted_loss_entries = 0
        max_loss_severity: str | None = None
        severity_order = {"advisory": 0, "degrading": 1, "major": 2, "blocking": 3}
        for result in self.results:
            if result.adapter is not None:
                adapter_counts[result.adapter.id] += 1
                scope_dimensions.update(result.adapter.scope_dimensions)
            if result.error is not None:
                kind = result.error.get("kind")
                if isinstance(kind, str):
                    error_kind_counts[kind] += 1
            if result.executable:
                executable_count += 1
            document = result.document
            if not isinstance(document, Mapping):
                continue
            document_count += 1
            conformance = document.get("conformance")
            conformance_passed = isinstance(conformance, Mapping) and conformance.get("passed") is True
            if document.get("valid") is True or conformance_passed:
                valid_count += 1
            semantic_loss = document.get("semantic_loss")
            loss_is_blocking = isinstance(semantic_loss, Mapping) and semantic_loss.get("max_severity") == "blocking"
            conformance_publishable = isinstance(conformance, Mapping) and conformance.get("publishable") is True
            if document.get("publishable") is True or conformance_publishable or (
                conformance_passed and not loss_is_blocking and result.status in {RuntimeStatus.SUCCEEDED, RuntimeStatus.LOSSY}
            ):
                publishable_count += 1
            if not isinstance(semantic_loss, Mapping):
                continue
            loss_count = semantic_loss.get("lost_count")
            if isinstance(loss_count, int) and loss_count >= 0:
                declared_loss_count += loss_count
            lost = semantic_loss.get("lost")
            if isinstance(lost, Sequence) and not isinstance(lost, (str, bytes)):
                for loss in lost:
                    if not isinstance(loss, Mapping):
                        continue
                    visible_loss_entry_count += 1
                    kind = loss.get("kind")
                    if isinstance(kind, str):
                        loss_kind_counts[kind] += 1
            omitted = semantic_loss.get("omitted_lost")
            if isinstance(omitted, int) and omitted >= 0:
                omitted_loss_entries += omitted
            severity = semantic_loss.get("max_severity")
            if isinstance(severity, str) and (
                max_loss_severity is None or severity_order.get(severity, -1) > severity_order.get(max_loss_severity, -1)
            ):
                max_loss_severity = severity
        return {
            "status_counts": dict(sorted(status_counts.items())),
            "adapter_counts": dict(sorted(adapter_counts.items())),
            "error_kind_counts": dict(sorted(error_kind_counts.items())),
            "scope_dimensions": sorted(scope_dimensions),
            "valid_count": valid_count,
            "publishable_count": publishable_count,
            "document_count": document_count,
            "executable_count": executable_count,
            "semantic_loss": {
                "declared_loss_count": declared_loss_count,
                "visible_loss_entry_count": visible_loss_entry_count,
                "omitted_loss_entries": omitted_loss_entries,
                "max_severity": max_loss_severity,
                "kind_counts": dict(sorted(loss_kind_counts.items())),
            },
        }

    def to_wire(self) -> dict[str, Any]:
        aggregate = self._aggregate()
        return {
            "schema": RUNTIME_BATCH_SCHEMA,
            "request": self.request.to_wire(),
            "status": self.status.value,
            "accepted": self.accepted,
            "result_count": len(self.results),
            "omitted_requests": self.omitted_requests,
            "stopped_on_error": self.stopped_on_error,
            "status_counts": aggregate["status_counts"],
            "adapter_counts": aggregate["adapter_counts"],
            "error_kind_counts": aggregate["error_kind_counts"],
            "scope_dimensions": aggregate["scope_dimensions"],
            "valid_count": aggregate["valid_count"],
            "publishable_count": aggregate["publishable_count"],
            "document_count": aggregate["document_count"],
            "executable_count": aggregate["executable_count"],
            "semantic_loss": aggregate["semantic_loss"],
            "batch_digest": self.batch_digest,
            "results": [result.to_wire() for result in self.results],
        }


class AdapterRuntime:
    """Dispatch concrete projection routes without content sniffing or optional imports."""

    _DISPATCHED = frozenset(
        {
            "bioprism.python.vcf_text",
            "bioprism.python.bids_manifest",
            "bioprism.python.dicom_metadata",
            "bioprism.python.nifti_metadata",
            "bioprism.python.anndata_metadata",
            "bioprism.python.alignment_metadata",
            "bioprism.python.nifti_bids",
            "bioprism.python.anndata",
            "bioprism.python.dicom",
            "bioprism.python.fasta_text",
            "bioprism.python.fastq_text",
            "bioprism.python.sam_text",
            "bioprism.python.bed_text",
            "bioprism.python.fhir_manifest",
            "bioprism.python.fhir_json",
            "bioprism.python.fhir_ndjson",
            "bioprism.python.gff3_text",
            "bioprism.python.mzml_text",
            "bioprism.python.pdb_text",
            "bioprism.python.sdf_text",
            "bioprism.python.vcf_indexed",
            "bioprism.python.bam_cram",
            "bioprism.python.ome_zarr",
        }
    )

    def __init__(self, registry: AdapterRegistry | None = None) -> None:
        self.registry = registry or AdapterRegistry()
        self._descriptors = {descriptor.id: descriptor for descriptor in self.registry.descriptors}

    @property
    def concrete_adapter_ids(self) -> tuple[str, ...]:
        return tuple(sorted(self._DISPATCHED.intersection(self._descriptors)))

    def execute(self, request: ProjectionRequest) -> AdapterExecutionResult:
        if not isinstance(request, ProjectionRequest):
            raise ArgumentError("request must be a ProjectionRequest")
        descriptor = self._descriptors.get(request.adapter_id)
        if descriptor is None:
            return AdapterExecutionResult(
                request,
                RuntimeStatus.UNSUPPORTED,
                False,
                None,
                error={"kind": "unknown_adapter", "detail": f"adapter {request.adapter_id!r} is not in the local registry"},
            )
        if request.adapter_id not in self._DISPATCHED:
            dependency = descriptor.optional_dependency
            detail = "the route is catalogued but has no concrete local binary execution binding"
            if dependency is not None:
                detail += f"; optional dependency {dependency!r} remains an explicit implementation boundary"
            return AdapterExecutionResult(
                request,
                RuntimeStatus.UNSUPPORTED,
                False,
                descriptor,
                error={"kind": "binary_reader_unavailable", "detail": detail},
            )
        try:
            document = self._dispatch(request)
        except OptionalDependencyUnavailable as error:
            return AdapterExecutionResult(
                request,
                RuntimeStatus.UNSUPPORTED,
                True,
                descriptor,
                error={"kind": "optional_dependency_missing", "dependency": error.dependency, "detail": str(error)},
            )
        except ArgumentError as error:
            return AdapterExecutionResult(
                request,
                RuntimeStatus.REJECTED,
                True,
                descriptor,
                error={"kind": "argument_error", "detail": str(error)},
            )
        except Exception as error:  # noqa: BLE001 - keep the gateway evidence-bearing on adapter faults
            return AdapterExecutionResult(
                request,
                RuntimeStatus.BLOCKED,
                True,
                descriptor,
                error={"kind": "adapter_execution_error", "detail": str(error)},
            )
        return AdapterExecutionResult(request, self._status(document), True, descriptor, document=document)

    def execute_batch(self, batch: ProjectionBatchRequest) -> ProjectionBatchResult:
        """Execute an ordered heterogeneous batch without hiding member-level outcomes."""

        if not isinstance(batch, ProjectionBatchRequest):
            raise ArgumentError("batch must be a ProjectionBatchRequest")
        results: list[AdapterExecutionResult] = []
        for request in batch.requests:
            result = self.execute(request)
            results.append(result)
            if batch.stop_on_error and result.status in {
                RuntimeStatus.INVALID,
                RuntimeStatus.BLOCKED,
                RuntimeStatus.REJECTED,
                RuntimeStatus.UNSUPPORTED,
            }:
                break
        omitted = len(batch.requests) - len(results)
        statuses = {result.status for result in results}
        successful = statuses.intersection({RuntimeStatus.SUCCEEDED, RuntimeStatus.LOSSY})
        failures = statuses.difference({RuntimeStatus.SUCCEEDED, RuntimeStatus.LOSSY})
        if not failures and omitted == 0:
            status = BatchStatus.SUCCEEDED
        elif successful or omitted:
            status = BatchStatus.PARTIAL
        elif RuntimeStatus.BLOCKED in failures or RuntimeStatus.INVALID in failures:
            status = BatchStatus.BLOCKED
        else:
            status = BatchStatus.REJECTED
        return ProjectionBatchResult(batch, status, tuple(results), omitted, omitted > 0)

    def _dispatch(self, request: ProjectionRequest) -> Mapping[str, Any]:
        payload = request.payload
        adapter_id = request.adapter_id
        if adapter_id == "bioprism.python.vcf_text":
            text = payload.get("text")
            if not isinstance(text, str):
                raise ArgumentError("vcf_text payload requires a string 'text'")
            return parse_vcf(
                text,
                source_id=request.source_id,
                reference_build=payload.get("reference_build"),
                provenance=request.provenance,
                max_records=payload.get("max_records", 100_000),
                max_items=min(request.max_items, 1_000),
            ).to_wire()
        if adapter_id == "bioprism.python.bids_manifest":
            files = payload.get("files")
            if not isinstance(files, Sequence) or isinstance(files, (str, bytes)):
                raise ArgumentError("bids_manifest payload requires a sequence 'files'")
            return audit_bids(
                files,
                source_id=request.source_id,
                metadata=payload.get("metadata"),
                participants_tsv=payload.get("participants_tsv"),
                max_files=payload.get("max_files", 50_000),
                max_items=request.max_items,
            ).to_wire()
        if adapter_id == "bioprism.python.dicom_metadata":
            instances = payload.get("instances")
            if not isinstance(instances, Sequence) or isinstance(instances, (str, bytes)):
                raise ArgumentError("dicom_metadata payload requires a sequence 'instances'")
            return audit_dicom(
                instances,
                source_id=request.source_id,
                provenance=request.provenance,
                max_instances=payload.get("max_instances", 100_000),
                max_items=request.max_items,
            ).to_wire()
        if adapter_id == "bioprism.python.dicom":
            path = payload.get("path")
            if not isinstance(path, str):
                raise ArgumentError("dicom payload requires a string 'path'")
            return read_dicom_projection(path, source_id=request.source_id, provenance=request.provenance, max_items=request.max_items)
        if adapter_id == "bioprism.python.fasta_text":
            sequence_type = payload.get("sequence_type", "unknown")
            if "text" in payload:
                text = payload.get("text")
                if not isinstance(text, (str, bytes)):
                    raise ArgumentError("fasta_text payload requires text or bytes under 'text'")
                return parse_fasta(
                    text,
                    source_id=request.source_id,
                    provenance=request.provenance,
                    sequence_type=sequence_type,
                    max_records=payload.get("max_records", 100_000),
                    max_items=request.max_items,
                ).to_wire()
            path = payload.get("path")
            if not isinstance(path, str):
                raise ArgumentError("fasta_text payload requires 'text' or a string 'path'")
            return read_fasta(
                path,
                source_id=request.source_id,
                provenance=request.provenance,
                sequence_type=sequence_type,
                max_records=payload.get("max_records", 100_000),
                max_items=request.max_items,
            )
        if adapter_id == "bioprism.python.fastq_text":
            if "text" in payload:
                text = payload.get("text")
                if not isinstance(text, (str, bytes)):
                    raise ArgumentError("fastq_text payload requires text or bytes under 'text'")
                return parse_fastq(
                    text,
                    source_id=request.source_id,
                    provenance=request.provenance,
                    max_records=payload.get("max_records", 100_000),
                    max_items=request.max_items,
                ).to_wire()
            path = payload.get("path")
            if not isinstance(path, str):
                raise ArgumentError("fastq_text payload requires 'text' or a string 'path'")
            return read_fastq(
                path,
                source_id=request.source_id,
                provenance=request.provenance,
                max_records=payload.get("max_records", 100_000),
                max_items=request.max_items,
            )
        if adapter_id == "bioprism.python.fhir_manifest":
            document = payload.get("document")
            if not isinstance(document, Mapping):
                raise ArgumentError("fhir_manifest payload requires a mapping 'document'")
            return audit_fhir(document, source_id=request.source_id, provenance=request.provenance, max_items=request.max_items).to_wire()
        if adapter_id == "bioprism.python.fhir_json":
            path = payload.get("path")
            if not isinstance(path, str):
                raise ArgumentError("fhir_json payload requires a string 'path'")
            return read_fhir_json(path, source_id=request.source_id, provenance=request.provenance, max_items=request.max_items)
        if adapter_id == "bioprism.python.fhir_ndjson":
            path = payload.get("path")
            if not isinstance(path, str):
                raise ArgumentError("fhir_ndjson payload requires a string 'path'")
            return read_fhir_ndjson(
                path,
                source_id=request.source_id,
                provenance=request.provenance,
                max_records=payload.get("max_records", 100_000),
                max_items=request.max_items,
            )
        if adapter_id == "bioprism.python.gff3_text":
            annotation_format = payload.get("annotation_format", "gff3")
            if "text" in payload:
                text = payload.get("text")
                if not isinstance(text, (str, bytes)):
                    raise ArgumentError("gff3_text payload requires text or bytes under 'text'")
                return parse_gff3(
                    text,
                    source_id=request.source_id,
                    provenance=request.provenance,
                    annotation_format=annotation_format,
                    max_features=payload.get("max_features", 500_000),
                    max_items=request.max_items,
                ).to_wire()
            path = payload.get("path")
            if not isinstance(path, str):
                raise ArgumentError("gff3_text payload requires 'text' or a string 'path'")
            return read_gff3(
                path,
                source_id=request.source_id,
                provenance=request.provenance,
                annotation_format=annotation_format,
                max_features=payload.get("max_features", 500_000),
                max_items=request.max_items,
            )
        if adapter_id == "bioprism.python.bed_text":
            if "text" in payload:
                text = payload.get("text")
                if not isinstance(text, (str, bytes)):
                    raise ArgumentError("bed_text payload requires text or bytes under 'text'")
                return parse_bed(
                    text,
                    source_id=request.source_id,
                    provenance=request.provenance,
                    max_features=payload.get("max_features", 500_000),
                    max_items=request.max_items,
                ).to_wire()
            path = payload.get("path")
            if not isinstance(path, str):
                raise ArgumentError("bed_text payload requires 'text' or a string 'path'")
            return read_bed(
                path,
                source_id=request.source_id,
                provenance=request.provenance,
                max_features=payload.get("max_features", 500_000),
                max_items=request.max_items,
            )
        if adapter_id == "bioprism.python.mzml_text":
            if "text" in payload:
                text = payload.get("text")
                if not isinstance(text, (str, bytes)):
                    raise ArgumentError("mzml_text payload requires text or bytes under 'text'")
                return parse_mzml(
                    text,
                    source_id=request.source_id,
                    provenance=request.provenance,
                    max_spectra=payload.get("max_spectra", 100_000),
                    max_items=request.max_items,
                ).to_wire()
            path = payload.get("path")
            if not isinstance(path, str):
                raise ArgumentError("mzml_text payload requires 'text' or a string 'path'")
            return read_mzml(
                path,
                source_id=request.source_id,
                provenance=request.provenance,
                max_spectra=payload.get("max_spectra", 100_000),
                max_items=request.max_items,
            )
        if adapter_id == "bioprism.python.pdb_text":
            if "text" in payload:
                text = payload.get("text")
                if not isinstance(text, (str, bytes)):
                    raise ArgumentError("pdb_text payload requires text or bytes under 'text'")
                return parse_pdb(
                    text,
                    source_id=request.source_id,
                    provenance=request.provenance,
                    max_atoms=payload.get("max_atoms", 1_000_000),
                    max_items=request.max_items,
                ).to_wire()
            path = payload.get("path")
            if not isinstance(path, str):
                raise ArgumentError("pdb_text payload requires 'text' or a string 'path'")
            return read_pdb(
                path,
                source_id=request.source_id,
                provenance=request.provenance,
                max_atoms=payload.get("max_atoms", 1_000_000),
                max_items=request.max_items,
            )
        if adapter_id == "bioprism.python.sdf_text":
            if "text" in payload:
                text = payload.get("text")
                if not isinstance(text, (str, bytes)):
                    raise ArgumentError("sdf_text payload requires text or bytes under 'text'")
                return parse_sdf(
                    text,
                    source_id=request.source_id,
                    provenance=request.provenance,
                    max_molecules=payload.get("max_molecules", 100_000),
                    max_items=request.max_items,
                ).to_wire()
            path = payload.get("path")
            if not isinstance(path, str):
                raise ArgumentError("sdf_text payload requires 'text' or a string 'path'")
            return read_sdf(
                path,
                source_id=request.source_id,
                provenance=request.provenance,
                max_molecules=payload.get("max_molecules", 100_000),
                max_items=request.max_items,
            )
        if adapter_id == "bioprism.python.sam_text":
            if "text" in payload:
                text = payload.get("text")
                if not isinstance(text, (str, bytes)):
                    raise ArgumentError("sam_text payload requires text or bytes under 'text'")
                return parse_sam(
                    text,
                    source_id=request.source_id,
                    provenance=request.provenance,
                    max_records=payload.get("max_records", 1_000_000),
                    max_headers=payload.get("max_headers", 100_000),
                    max_items=request.max_items,
                    max_tags=payload.get("max_tags", 100_000),
                ).to_wire()
            path = payload.get("path")
            if not isinstance(path, str):
                raise ArgumentError("sam_text payload requires 'text' or a string 'path'")
            return read_sam(
                path,
                source_id=request.source_id,
                provenance=request.provenance,
                max_records=payload.get("max_records", 1_000_000),
                max_headers=payload.get("max_headers", 100_000),
                max_items=request.max_items,
                max_tags=payload.get("max_tags", 100_000),
            )
        if adapter_id == "bioprism.python.nifti_metadata":
            images = payload.get("images")
            if not isinstance(images, Sequence) or isinstance(images, (str, bytes)):
                raise ArgumentError("nifti_metadata payload requires a sequence 'images'")
            return audit_nifti(
                images,
                source_id=request.source_id,
                provenance=request.provenance,
                max_images=payload.get("max_images", 10_000),
                max_items=request.max_items,
            ).to_wire()
        if adapter_id == "bioprism.python.nifti_bids":
            path = payload.get("path")
            if not isinstance(path, str):
                raise ArgumentError("nifti_bids payload requires a string 'path'")
            return read_nifti_header(
                path,
                source_id=request.source_id,
                provenance=request.provenance,
                reference_space=payload.get("reference_space"),
                max_items=request.max_items,
            )
        if adapter_id == "bioprism.python.anndata_metadata":
            dataset = payload.get("dataset")
            if not isinstance(dataset, Mapping):
                raise ArgumentError("anndata_metadata payload requires a mapping 'dataset'")
            return audit_anndata(dataset, source_id=request.source_id, provenance=request.provenance, max_items=request.max_items).to_wire()
        if adapter_id == "bioprism.python.anndata":
            path = payload.get("path")
            if not isinstance(path, str):
                raise ArgumentError("anndata payload requires a string 'path'")
            return read_anndata_projection(
                path,
                source_id=request.source_id,
                provenance=request.provenance,
                storage_format=payload.get("storage_format", "auto"),
                max_items=request.max_items,
            )
        if adapter_id == "bioprism.python.alignment_metadata":
            references = payload.get("references")
            records = payload.get("records")
            if not isinstance(references, Mapping):
                raise ArgumentError("alignment_metadata payload requires a mapping 'references'")
            if not isinstance(records, Sequence) or isinstance(records, (str, bytes)):
                raise ArgumentError("alignment_metadata payload requires a sequence 'records'")
            return audit_alignments(
                references,
                records,
                source_id=request.source_id,
                reference_build=payload.get("reference_build"),
                provenance=request.provenance,
                max_records=payload.get("max_records", 100_000),
                max_items=request.max_items,
            ).to_wire()
        if adapter_id == "bioprism.python.vcf_indexed":
            path = payload.get("path")
            if not isinstance(path, str):
                raise ArgumentError("vcf_indexed payload requires a string 'path'")
            return read_indexed_vcf(
                path,
                source_id=request.source_id,
                reference_build=payload.get("reference_build"),
                provenance=request.provenance,
                max_records=payload.get("max_records", 100_000),
                max_items=request.max_items,
            )
        if adapter_id == "bioprism.python.bam_cram":
            path = payload.get("path")
            if not isinstance(path, str):
                raise ArgumentError("bam_cram payload requires a string 'path'")
            return read_alignment_file(
                path,
                source_id=request.source_id,
                reference_build=payload.get("reference_build"),
                provenance=request.provenance,
                reference_fasta=payload.get("reference_fasta"),
                require_index=payload.get("require_index", True),
                max_records=payload.get("max_records", 100_000),
                max_items=request.max_items,
            )
        if adapter_id == "bioprism.python.ome_zarr":
            path = payload.get("path")
            if not isinstance(path, str):
                raise ArgumentError("ome_zarr payload requires a string 'path'")
            return read_ome_zarr(path, source_id=request.source_id, provenance=request.provenance, max_items=request.max_items)
        raise ArgumentError(f"no dispatch binding exists for {adapter_id!r}")

    @staticmethod
    def _status(document: Mapping[str, Any]) -> RuntimeStatus:
        if document.get("valid") is False or document.get("conformance", {}).get("passed") is False:
            return RuntimeStatus.INVALID
        semantic_loss = document.get("semantic_loss")
        if isinstance(semantic_loss, Mapping):
            max_severity = semantic_loss.get("max_severity")
            if max_severity == "blocking":
                return RuntimeStatus.BLOCKED
            if max_severity in {"major", "degrading"}:
                return RuntimeStatus.LOSSY
        if document.get("publishable") is False:
            return RuntimeStatus.BLOCKED
        return RuntimeStatus.SUCCEEDED


def execute_projection(
    adapter_id: str,
    source_id: str,
    payload: Mapping[str, Any],
    *,
    provenance: Mapping[str, Any] | None = None,
    max_items: int = MAX_RUNTIME_ITEMS,
    runtime: AdapterRuntime | None = None,
) -> AdapterExecutionResult:
    """Execute one explicit concrete adapter projection through the normalized runtime envelope."""

    request = ProjectionRequest(adapter_id, source_id, payload, provenance, max_items)
    return (runtime or AdapterRuntime()).execute(request)


def execute_projection_batch(
    requests: Sequence[ProjectionRequest],
    *,
    stop_on_error: bool = False,
    max_total_items: int = MAX_RUNTIME_BATCH_ITEMS,
    runtime: AdapterRuntime | None = None,
) -> ProjectionBatchResult:
    """Execute a bounded ordered batch across heterogeneous explicit adapter requests."""

    batch = ProjectionBatchRequest(tuple(requests), stop_on_error=stop_on_error, max_total_items=max_total_items)
    return (runtime or AdapterRuntime()).execute_batch(batch)


__all__ = [
    "AdapterExecutionResult",
    "AdapterRuntime",
    "BatchStatus",
    "MAX_RUNTIME_BATCH_ITEMS",
    "MAX_RUNTIME_BATCH_REQUESTS",
    "MAX_RUNTIME_ITEMS",
    "ProjectionRequest",
    "ProjectionBatchRequest",
    "ProjectionBatchResult",
    "RUNTIME_SCHEMA",
    "RUNTIME_BATCH_SCHEMA",
    "RuntimeStatus",
    "execute_projection",
    "execute_projection_batch",
]
