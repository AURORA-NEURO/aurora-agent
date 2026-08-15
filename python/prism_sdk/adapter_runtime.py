"""Typed execution gateway for the concrete Python biological adapter contracts.

The registry answers *which* route could handle a source. This module answers the next practical
question: *run the selected bounded projection audit*. It deliberately does not sniff, import
optional binary readers, or silently fall back between formats. Concrete routes return their full
audit document; catalogued raw-byte routes return an explicit unsupported execution result until
their optional reader binding is installed and implemented.
"""

from __future__ import annotations

from dataclasses import dataclass
from enum import Enum
from typing import Any, Mapping, Sequence

from .alignment import audit_alignments
from .anndata import audit_anndata
from .bids import audit_bids
from .biological import AdapterDescriptor, AdapterRegistry
from .dicom import audit_dicom
from .errors import ArgumentError
from .nifti import audit_nifti
from .optional_readers import (
    OptionalDependencyUnavailable,
    read_alignment_file,
    read_anndata_projection,
    read_dicom_projection,
    read_indexed_vcf,
    read_nifti_header,
)
from .vcf import parse_vcf


RUNTIME_SCHEMA = "bioprism-python-adapter-runtime/0.1"
MAX_RUNTIME_ADAPTER_ID_BYTES = 256
MAX_RUNTIME_SOURCE_ID_BYTES = 512
MAX_RUNTIME_ITEMS = 1_000


class RuntimeStatus(str, Enum):
    SUCCEEDED = "succeeded"
    LOSSY = "lossy"
    INVALID = "invalid"
    BLOCKED = "blocked"
    REJECTED = "rejected"
    UNSUPPORTED = "unsupported"


def _text(name: str, value: str, maximum: int) -> None:
    if not isinstance(value, str) or not value.strip():
        raise ArgumentError(f"{name} must be a non-empty string")
    if len(value.encode("utf-8")) > maximum:
        raise ArgumentError(f"{name} exceeds the {maximum}-byte limit")
    if any(ord(character) < 0x20 for character in value):
        raise ArgumentError(f"{name} must not contain control characters")


@dataclass(frozen=True)
class ProjectionRequest:
    """A bounded request whose payload shape is owned by the selected adapter route."""

    adapter_id: str
    source_id: str
    payload: Mapping[str, Any]
    provenance: Mapping[str, Any] | None = None
    max_items: int = MAX_RUNTIME_ITEMS

    def __post_init__(self) -> None:
        _text("adapter_id", self.adapter_id, MAX_RUNTIME_ADAPTER_ID_BYTES)
        _text("source_id", self.source_id, MAX_RUNTIME_SOURCE_ID_BYTES)
        if not isinstance(self.payload, Mapping):
            raise ArgumentError("payload must be a mapping; its schema is selected by adapter_id")
        if self.provenance is not None and not isinstance(self.provenance, Mapping):
            raise ArgumentError("provenance must be a mapping when supplied")
        if isinstance(self.max_items, bool) or not isinstance(self.max_items, int) or not 1 <= self.max_items <= MAX_RUNTIME_ITEMS:
            raise ArgumentError(f"max_items must be between 1 and {MAX_RUNTIME_ITEMS}")

    def to_wire(self) -> dict[str, Any]:
        return {
            "adapter_id": self.adapter_id,
            "source_id": self.source_id,
            "payload_keys": sorted(str(key) for key in self.payload.keys()),
            "provenance_present": self.provenance is not None,
            "max_items": self.max_items,
        }


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
            "bioprism.python.vcf_indexed",
            "bioprism.python.bam_cram",
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


__all__ = [
    "AdapterExecutionResult",
    "AdapterRuntime",
    "MAX_RUNTIME_ITEMS",
    "ProjectionRequest",
    "RUNTIME_SCHEMA",
    "RuntimeStatus",
    "execute_projection",
]
