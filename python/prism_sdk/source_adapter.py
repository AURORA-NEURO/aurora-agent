"""Bridge bounded source execution envelopes into concrete Python adapter audits.

The Rust source connector owns confinement, network policy, byte limits, and transport digests.
The Python runtime owns format-specific parsing and semantic-loss reporting. This module is the
explicit seam between those planes. It never reads the locator a second time, never guesses a
format, and never hides a truncated or binary-only response behind a parser refusal.
"""

from __future__ import annotations

from dataclasses import dataclass
from enum import Enum
from typing import Any, Mapping

from .adapter_runtime import AdapterExecutionResult, AdapterRuntime, ProjectionRequest, RuntimeStatus
from .authoring import content_digest
from .errors import ArgumentError

SOURCE_ADAPTER_PROJECTION_SCHEMA = "bioprism-python-domain-evidence-source-adapter/0.1"
SOURCE_ADAPTER_PROJECTION_WORKFLOW = "domain_evidence_source_project"
MAX_SOURCE_ADAPTER_ID_BYTES = 256
MAX_SOURCE_ADAPTER_SOURCE_ID_BYTES = 512
MAX_SOURCE_ADAPTER_PROVENANCE_ITEMS = 64


class SourceAdapterProjectionStatus(str, Enum):
    PROJECTED = "projected"
    SOURCE_PARTIAL = "source_partial"
    INVALID = "invalid"
    LOSSY = "lossy"
    BLOCKED = "blocked"
    REFUSED = "refused"


def _text(name: str, value: str, maximum: int) -> str:
    if not isinstance(value, str) or not value.strip():
        raise ArgumentError(f"{name} must be a non-empty string")
    if len(value.encode("utf-8")) > maximum:
        raise ArgumentError(f"{name} exceeds the {maximum}-byte bound")
    if any(ord(character) < 0x20 and character not in "\t " for character in value):
        raise ArgumentError(f"{name} must not contain control characters")
    return value


def _digest(name: str, value: Any) -> str:
    value = _text(name, value, 128)
    if len(value) != 64 or any(character not in "0123456789abcdef" for character in value):
        raise ArgumentError(f"{name} must be a lowercase SHA-256 digest")
    return value


def _mapping(name: str, value: Any) -> Mapping[str, Any]:
    if not isinstance(value, Mapping):
        raise ArgumentError(f"{name} must be an object")
    return value


def _body_payload(adapter_id: str, retrieval: Mapping[str, Any], options: Mapping[str, Any]) -> dict[str, Any]:
    encoding = retrieval.get("body_encoding")
    if encoding in {"omitted", "binary", "utf8_preview"} or retrieval.get("body_truncated") is True:
        raise ArgumentError(
            "source response does not contain a complete parser input; omitted, binary, and truncated bodies are refused"
        )
    if encoding == "empty":
        body: Any = ""
    elif encoding == "utf8":
        body = retrieval.get("body")
        if not isinstance(body, str):
            raise ArgumentError("source UTF-8 response body is missing")
    elif encoding == "json":
        body = retrieval.get("body")
    else:
        raise ArgumentError(f"unsupported source response body encoding {encoding!r}")

    payload = dict(options)
    if any(key in payload for key in ("text", "document", "dataset", "instances", "images", "files", "references", "records")):
        raise ArgumentError("adapter options must not override the source-derived parser body")

    if adapter_id in {
        "bioprism.python.vcf_text",
        "bioprism.python.fasta_text",
        "bioprism.python.fastq_text",
        "bioprism.python.gff3_text",
        "bioprism.python.bed_text",
        "bioprism.python.mzml_text",
        "bioprism.python.pdb_text",
        "bioprism.python.sdf_text",
        "bioprism.python.sam_text",
    }:
        if not isinstance(body, str):
            raise ArgumentError(f"{adapter_id} requires a complete UTF-8 source body")
        payload["text"] = body
    elif adapter_id == "bioprism.python.bids_manifest":
        payload["files"] = body.get("files") if isinstance(body, Mapping) and "files" in body else body
        if not isinstance(payload["files"], list):
            raise ArgumentError("bids_manifest source JSON must be an array or an object with a files array")
    elif adapter_id == "bioprism.python.dicom_metadata":
        payload["instances"] = body.get("instances") if isinstance(body, Mapping) and "instances" in body else body
        if not isinstance(payload["instances"], list):
            raise ArgumentError("dicom_metadata source JSON must be an array or an object with an instances array")
    elif adapter_id == "bioprism.python.nifti_metadata":
        payload["images"] = body.get("images") if isinstance(body, Mapping) and "images" in body else body
        if not isinstance(payload["images"], list):
            raise ArgumentError("nifti_metadata source JSON must be an array or an object with an images array")
    elif adapter_id == "bioprism.python.anndata_metadata":
        payload["dataset"] = body
        if not isinstance(body, Mapping):
            raise ArgumentError("anndata_metadata source JSON must be an object")
    elif adapter_id == "bioprism.python.alignment_metadata":
        if not isinstance(body, Mapping):
            raise ArgumentError("alignment_metadata source JSON must be an object")
        for key in ("references", "records"):
            if key not in body:
                raise ArgumentError(f"alignment_metadata source JSON is missing {key!r}")
            payload[key] = body[key]
    elif adapter_id == "bioprism.python.fhir_manifest":
        payload["document"] = body
        if not isinstance(body, Mapping):
            raise ArgumentError("fhir_manifest source JSON must be an object")
    else:
        raise ArgumentError(
            f"adapter {adapter_id!r} has no safe inline source-body binding; use its explicit path or dependency-gated route"
        )
    return payload


def _source_provenance(execution: Mapping[str, Any], caller: Mapping[str, Any] | None) -> dict[str, Any]:
    fields = {
        "source_plan_digest": _digest("source plan digest", execution.get("source_plan_digest")),
        "source_response_digest": _digest("source response digest", execution.get("response_digest")),
        "source_outcome": _text("source outcome", execution.get("outcome"), 64),
        "source_group_id": _text("source group_id", execution.get("group_id"), 512),
        "source_subject_id": _text("source subject_id", execution.get("subject_id"), 512),
    }
    raw_digest = execution.get("raw_content_digest")
    if raw_digest is not None:
        fields["source_raw_content_digest"] = _digest("source raw content digest", raw_digest)
    domains = execution.get("domains")
    if isinstance(domains, list):
        fields["source_domains"] = tuple(_text("source domain", domain, 512) for domain in domains)
    if caller is not None:
        if not isinstance(caller, Mapping):
            raise ArgumentError("provenance must be an object")
        if len(caller) > MAX_SOURCE_ADAPTER_PROVENANCE_ITEMS:
            raise ArgumentError("provenance contains too many fields")
        collisions = set(fields).intersection(caller)
        if collisions:
            raise ArgumentError(f"caller provenance cannot override source-bound fields: {sorted(collisions)}")
        fields.update(caller)
    return fields


@dataclass(frozen=True)
class SourceAdapterProjectionRequest:
    """Explicit adapter selection and bounded options for one source response."""

    adapter_id: str
    source_id: str
    adapter_options: Mapping[str, Any] | None = None
    provenance: Mapping[str, Any] | None = None
    max_items: int = 1_000
    expected_raw_content_digest: str | None = None

    def __post_init__(self) -> None:
        _text("adapter_id", self.adapter_id, MAX_SOURCE_ADAPTER_ID_BYTES)
        _text("source_id", self.source_id, MAX_SOURCE_ADAPTER_SOURCE_ID_BYTES)
        if self.adapter_options is None:
            object.__setattr__(self, "adapter_options", {})
        elif not isinstance(self.adapter_options, Mapping):
            raise ArgumentError("adapter_options must be an object")
        if self.provenance is not None and not isinstance(self.provenance, Mapping):
            raise ArgumentError("provenance must be an object")
        if isinstance(self.max_items, bool) or not isinstance(self.max_items, int) or not 1 <= self.max_items <= 1_000:
            raise ArgumentError("max_items must be between 1 and 1000")
        if self.expected_raw_content_digest is not None:
            _digest("expected raw content digest", self.expected_raw_content_digest)

    def to_wire(self) -> dict[str, Any]:
        return {
            "adapter_id": self.adapter_id,
            "source_id": self.source_id,
            "adapter_option_keys": sorted(str(key) for key in self.adapter_options),
            "provenance_present": self.provenance is not None,
            "max_items": self.max_items,
            "expected_raw_content_digest": self.expected_raw_content_digest,
        }


@dataclass(frozen=True)
class SourceAdapterProjectionResult:
    """Source-bound adapter outcome preserving transport and parser evidence separately."""

    request: SourceAdapterProjectionRequest
    status: SourceAdapterProjectionStatus
    source_outcome: str
    source_plan_digest: str
    raw_content_digest: str | None
    response_digest: str
    adapter_result: AdapterExecutionResult | None = None
    error: Mapping[str, Any] | None = None

    @property
    def projected(self) -> bool:
        return self.adapter_result is not None and self.status in {
            SourceAdapterProjectionStatus.PROJECTED,
            SourceAdapterProjectionStatus.SOURCE_PARTIAL,
            SourceAdapterProjectionStatus.LOSSY,
        }

    @property
    def projection_digest(self) -> str:
        return content_digest(
            {
                "schema": SOURCE_ADAPTER_PROJECTION_SCHEMA,
                "request": self.request.to_wire(),
                "status": self.status.value,
                "source_outcome": self.source_outcome,
                "source_plan_digest": self.source_plan_digest,
                "raw_content_digest": self.raw_content_digest,
                "response_digest": self.response_digest,
                "adapter": self.adapter_result.to_wire() if self.adapter_result else None,
                "error": self.error,
            }
        )

    def to_wire(self) -> dict[str, Any]:
        return {
            "schema": SOURCE_ADAPTER_PROJECTION_SCHEMA,
            "workflow": SOURCE_ADAPTER_PROJECTION_WORKFLOW,
            "request": self.request.to_wire(),
            "status": self.status.value,
            "projected": self.projected,
            "source_outcome": self.source_outcome,
            "source_plan_digest": self.source_plan_digest,
            "raw_content_digest": self.raw_content_digest,
            "response_digest": self.response_digest,
            "projection_digest": self.projection_digest,
            "adapter_result": self.adapter_result.to_wire() if self.adapter_result else None,
            "error": dict(self.error) if self.error else None,
        }


def project_source_execution(
    execution: Mapping[str, Any],
    request: SourceAdapterProjectionRequest,
    *,
    runtime: AdapterRuntime | None = None,
) -> SourceAdapterProjectionResult:
    """Project one complete source-execution body through one explicit Python adapter.

    The function returns typed refusals for transport outcomes that cannot supply a complete body;
    malformed execution identity is an argument error because it cannot safely be retained as an
    adapter result. Adapter parse/validation failures remain inside ``adapter_result``.
    """

    if not isinstance(execution, Mapping):
        raise ArgumentError("source execution must be an object")
    if not isinstance(request, SourceAdapterProjectionRequest):
        raise ArgumentError("request must be a SourceAdapterProjectionRequest")
    source_plan_digest = _digest("source plan digest", execution.get("source_plan_digest"))
    response_digest = _digest("source response digest", execution.get("response_digest"))
    source_outcome = _text("source outcome", execution.get("outcome"), 64)
    raw_content_digest = execution.get("raw_content_digest")
    if raw_content_digest is not None:
        raw_content_digest = _digest("source raw content digest", raw_content_digest)
    if request.expected_raw_content_digest is not None and raw_content_digest != request.expected_raw_content_digest:
        return SourceAdapterProjectionResult(
            request,
            SourceAdapterProjectionStatus.REFUSED,
            source_outcome,
            source_plan_digest,
            raw_content_digest,
            response_digest,
            error={"kind": "raw_content_digest_mismatch", "expected": request.expected_raw_content_digest},
        )
    if source_outcome not in {"observed", "partial"}:
        return SourceAdapterProjectionResult(
            request,
            SourceAdapterProjectionStatus.REFUSED,
            source_outcome,
            source_plan_digest,
            raw_content_digest,
            response_digest,
            error={"kind": "source_outcome_not_projectable", "detail": "only observed or partial source bodies may reach an adapter"},
        )
    execution_result = _mapping("source execution result", execution.get("execution_result"))
    response = _mapping("source execution response", execution_result.get("response"))
    retrieval = _mapping("source execution retrieval", response.get("retrieval"))
    try:
        payload = _body_payload(request.adapter_id, retrieval, request.adapter_options)
        provenance = _source_provenance(execution, request.provenance)
    except ArgumentError as error:
        return SourceAdapterProjectionResult(
            request,
            SourceAdapterProjectionStatus.REFUSED,
            source_outcome,
            source_plan_digest,
            raw_content_digest,
            response_digest,
            error={"kind": "source_body_refused", "detail": str(error)},
        )
    adapter_result = (runtime or AdapterRuntime()).execute(
        ProjectionRequest(
            request.adapter_id,
            request.source_id,
            payload,
            request.provenance,
            request.max_items,
            source_context=provenance,
        )
    )
    if source_outcome == "partial" and adapter_result.status in {
        RuntimeStatus.SUCCEEDED,
        RuntimeStatus.LOSSY,
    }:
        # Preserve transport incompleteness as the outer status. The nested adapter result still
        # retains whether parsing itself was lossless or lossy.
        status = SourceAdapterProjectionStatus.SOURCE_PARTIAL
    elif adapter_result.status is RuntimeStatus.INVALID:
        status = SourceAdapterProjectionStatus.INVALID
    elif adapter_result.status is RuntimeStatus.BLOCKED:
        status = SourceAdapterProjectionStatus.BLOCKED
    elif adapter_result.status is RuntimeStatus.LOSSY:
        status = SourceAdapterProjectionStatus.LOSSY
    elif adapter_result.status in {RuntimeStatus.REJECTED, RuntimeStatus.UNSUPPORTED}:
        status = SourceAdapterProjectionStatus.REFUSED
    else:
        status = SourceAdapterProjectionStatus.PROJECTED
    return SourceAdapterProjectionResult(
        request,
        status,
        source_outcome,
        source_plan_digest,
        raw_content_digest,
        response_digest,
        adapter_result=adapter_result,
        error=adapter_result.error,
    )


__all__ = [
    "MAX_SOURCE_ADAPTER_ID_BYTES",
    "MAX_SOURCE_ADAPTER_PROVENANCE_ITEMS",
    "MAX_SOURCE_ADAPTER_SOURCE_ID_BYTES",
    "SOURCE_ADAPTER_PROJECTION_SCHEMA",
    "SOURCE_ADAPTER_PROJECTION_WORKFLOW",
    "SourceAdapterProjectionRequest",
    "SourceAdapterProjectionResult",
    "SourceAdapterProjectionStatus",
    "project_source_execution",
]
