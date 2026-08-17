"""Submit caller-owned adapter execution evidence without moving execution into the core.

The local runtime and the MCP/HTTP evidence endpoint are deliberately separate planes. This
module composes them for applications that want one explicit handoff while preserving member-level
remote refusals and transport failures in heterogeneous batches.
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Mapping, Protocol, Sequence

from .adapter_execution_evidence import (
    AdapterExecutionEvidenceReport,
    AdapterExecutionEvidenceRequest,
)
from .adapter_runtime import (
    AdapterExecutionResult,
    AdapterRuntime,
    ProjectionBatchRequest,
    ProjectionBatchResult,
    ProjectionRequest,
)
from .errors import ArgumentError

MAX_SUBMISSION_ERROR_DETAIL_BYTES = 512


class AdapterEvidenceSink(Protocol):
    """The sync portion shared by ``ApiClient`` and ``Workspace``."""

    def adapter_execution_evidence_report(
        self,
        request: AdapterExecutionEvidenceRequest,
    ) -> AdapterExecutionEvidenceReport:
        ...


class AsyncAdapterEvidenceSink(Protocol):
    """The async portion shared by ``AsyncApiClient`` and ``AsyncWorkspace``."""

    async def adapter_execution_evidence_report(
        self,
        request: AdapterExecutionEvidenceRequest,
    ) -> AdapterExecutionEvidenceReport:
        ...


def _error_payload(error: Exception) -> dict[str, str]:
    detail = str(error)
    if len(detail.encode("utf-8")) > MAX_SUBMISSION_ERROR_DETAIL_BYTES:
        detail = detail.encode("utf-8")[:MAX_SUBMISSION_ERROR_DETAIL_BYTES].decode("utf-8", "ignore")
    payload = {"kind": type(error).__name__, "detail": detail}
    status = getattr(error, "status", None)
    if isinstance(status, int):
        payload["status"] = str(status)
    code = getattr(error, "code", None)
    if isinstance(code, int):
        payload["code"] = str(code)
    return payload


@dataclass(frozen=True)
class AdapterEvidenceSubmission:
    """One attempted retention call and its typed report or bounded transport error."""

    request: AdapterExecutionEvidenceRequest
    report: AdapterExecutionEvidenceReport | None = None
    error: Mapping[str, str] | None = None

    def __post_init__(self) -> None:
        if (self.report is None) == (self.error is None):
            raise ArgumentError("submission must contain exactly one report or error")

    @property
    def retained(self) -> bool:
        return self.report is not None

    @property
    def remote_execution_status(self) -> str | None:
        return self.report.execution_status if self.report is not None else None


@dataclass(frozen=True)
class ProjectionBatchEvidenceSubmission:
    """Ordered batch handoff with member-level retention and transport outcomes."""

    batch: ProjectionBatchResult
    submissions: tuple[AdapterEvidenceSubmission, ...]

    def __post_init__(self) -> None:
        if len(self.submissions) != len(self.batch.results):
            raise ArgumentError("batch submission count must match executed result count")

    @property
    def attempted_count(self) -> int:
        return len(self.submissions)

    @property
    def retained_count(self) -> int:
        return sum(submission.retained for submission in self.submissions)

    @property
    def transport_error_count(self) -> int:
        return sum(submission.error is not None for submission in self.submissions)

    @property
    def refused_count(self) -> int:
        return sum(
            submission.remote_execution_status == "refused"
            for submission in self.submissions
        )

    @property
    def complete(self) -> bool:
        return self.batch.omitted_requests == 0 and self.transport_error_count == 0


def submit_adapter_execution_evidence(
    sink: AdapterEvidenceSink,
    result: AdapterExecutionResult,
    group_id: str,
    domains: Sequence[str],
    *,
    subject_id: str,
    input_digest: str,
    parent_digests: Sequence[str] = (),
    attempt_id: str | None = None,
) -> AdapterEvidenceSubmission:
    """Convert and retain one runtime result through an HTTP or MCP facade."""

    request = result.to_adapter_execution_evidence_request(
        group_id,
        domains,
        subject_id=subject_id,
        input_digest=input_digest,
        parent_digests=parent_digests,
        attempt_id=attempt_id,
    )
    return AdapterEvidenceSubmission(request=request, report=sink.adapter_execution_evidence_report(request))


def submit_projection_batch_evidence(
    sink: AdapterEvidenceSink,
    batch: ProjectionBatchResult,
    group_id: str,
    domains: Sequence[str],
    *,
    subject_id: str,
    input_digests: Mapping[str, str],
    parent_digests_by_source: Mapping[str, Sequence[str]] | None = None,
    attempt_ids_by_source: Mapping[str, str] | None = None,
    continue_on_error: bool = False,
) -> ProjectionBatchEvidenceSubmission:
    """Retain every executed batch member, optionally continuing after transport failures.

    Request conversion happens before the first network call, so missing source digests and
    undeclared adapters fail closed without leaving an accidentally partial retention set.
    ``continue_on_error`` only applies to transport/facade exceptions; a successfully returned
    refused report remains a retained member and is never retried or relabeled.
    """

    if not isinstance(continue_on_error, bool):
        raise ArgumentError("continue_on_error must be a boolean")
    requests = batch.to_adapter_execution_evidence_requests(
        group_id,
        domains,
        subject_id=subject_id,
        input_digests=input_digests,
        parent_digests_by_source=parent_digests_by_source,
        attempt_ids_by_source=attempt_ids_by_source,
    )
    submissions: list[AdapterEvidenceSubmission] = []
    for request in requests:
        try:
            report = sink.adapter_execution_evidence_report(request)
        except Exception as error:  # noqa: BLE001 - preserve bounded member transport state
            if not continue_on_error:
                raise
            submissions.append(AdapterEvidenceSubmission(request=request, error=_error_payload(error)))
        else:
            submissions.append(AdapterEvidenceSubmission(request=request, report=report))
    return ProjectionBatchEvidenceSubmission(batch=batch, submissions=tuple(submissions))


def execute_and_submit_projection(
    sink: AdapterEvidenceSink,
    request: ProjectionRequest,
    group_id: str,
    domains: Sequence[str],
    *,
    subject_id: str,
    input_digest: str,
    parent_digests: Sequence[str] = (),
    attempt_id: str | None = None,
    runtime: AdapterRuntime | None = None,
) -> AdapterEvidenceSubmission:
    """Run one concrete local adapter, then retain its caller-owned evidence."""

    if not isinstance(request, ProjectionRequest):
        raise ArgumentError("request must be a ProjectionRequest")
    result = (runtime or AdapterRuntime()).execute(request)
    return submit_adapter_execution_evidence(
        sink,
        result,
        group_id,
        domains,
        subject_id=subject_id,
        input_digest=input_digest,
        parent_digests=parent_digests,
        attempt_id=attempt_id,
    )


def execute_and_submit_projection_batch(
    sink: AdapterEvidenceSink,
    batch: ProjectionBatchRequest,
    group_id: str,
    domains: Sequence[str],
    *,
    subject_id: str,
    input_digests: Mapping[str, str],
    parent_digests_by_source: Mapping[str, Sequence[str]] | None = None,
    attempt_ids_by_source: Mapping[str, str] | None = None,
    continue_on_error: bool = False,
    runtime: AdapterRuntime | None = None,
) -> ProjectionBatchEvidenceSubmission:
    """Run a bounded batch locally, then retain its members in deterministic order."""

    if not isinstance(batch, ProjectionBatchRequest):
        raise ArgumentError("batch must be a ProjectionBatchRequest")
    result = (runtime or AdapterRuntime()).execute_batch(batch)
    return submit_projection_batch_evidence(
        sink,
        result,
        group_id,
        domains,
        subject_id=subject_id,
        input_digests=input_digests,
        parent_digests_by_source=parent_digests_by_source,
        attempt_ids_by_source=attempt_ids_by_source,
        continue_on_error=continue_on_error,
    )


async def submit_adapter_execution_evidence_async(
    sink: AsyncAdapterEvidenceSink,
    result: AdapterExecutionResult,
    group_id: str,
    domains: Sequence[str],
    *,
    subject_id: str,
    input_digest: str,
    parent_digests: Sequence[str] = (),
    attempt_id: str | None = None,
) -> AdapterEvidenceSubmission:
    """Async counterpart to :func:`submit_adapter_execution_evidence`."""

    request = result.to_adapter_execution_evidence_request(
        group_id,
        domains,
        subject_id=subject_id,
        input_digest=input_digest,
        parent_digests=parent_digests,
        attempt_id=attempt_id,
    )
    report = await sink.adapter_execution_evidence_report(request)
    return AdapterEvidenceSubmission(request=request, report=report)


async def submit_projection_batch_evidence_async(
    sink: AsyncAdapterEvidenceSink,
    batch: ProjectionBatchResult,
    group_id: str,
    domains: Sequence[str],
    *,
    subject_id: str,
    input_digests: Mapping[str, str],
    parent_digests_by_source: Mapping[str, Sequence[str]] | None = None,
    attempt_ids_by_source: Mapping[str, str] | None = None,
    continue_on_error: bool = False,
) -> ProjectionBatchEvidenceSubmission:
    """Async batch handoff with the same fail-closed conversion and member accounting."""

    if not isinstance(continue_on_error, bool):
        raise ArgumentError("continue_on_error must be a boolean")
    requests = batch.to_adapter_execution_evidence_requests(
        group_id,
        domains,
        subject_id=subject_id,
        input_digests=input_digests,
        parent_digests_by_source=parent_digests_by_source,
        attempt_ids_by_source=attempt_ids_by_source,
    )
    submissions: list[AdapterEvidenceSubmission] = []
    for request in requests:
        try:
            report = await sink.adapter_execution_evidence_report(request)
        except Exception as error:  # noqa: BLE001 - preserve bounded member transport state
            if not continue_on_error:
                raise
            submissions.append(AdapterEvidenceSubmission(request=request, error=_error_payload(error)))
        else:
            submissions.append(AdapterEvidenceSubmission(request=request, report=report))
    return ProjectionBatchEvidenceSubmission(batch=batch, submissions=tuple(submissions))


async def execute_and_submit_projection_async(
    sink: AsyncAdapterEvidenceSink,
    request: ProjectionRequest,
    group_id: str,
    domains: Sequence[str],
    *,
    subject_id: str,
    input_digest: str,
    parent_digests: Sequence[str] = (),
    attempt_id: str | None = None,
    runtime: AdapterRuntime | None = None,
) -> AdapterEvidenceSubmission:
    """Async local execution followed by async evidence retention."""

    if not isinstance(request, ProjectionRequest):
        raise ArgumentError("request must be a ProjectionRequest")
    result = (runtime or AdapterRuntime()).execute(request)
    return await submit_adapter_execution_evidence_async(
        sink,
        result,
        group_id,
        domains,
        subject_id=subject_id,
        input_digest=input_digest,
        parent_digests=parent_digests,
        attempt_id=attempt_id,
    )


async def execute_and_submit_projection_batch_async(
    sink: AsyncAdapterEvidenceSink,
    batch: ProjectionBatchRequest,
    group_id: str,
    domains: Sequence[str],
    *,
    subject_id: str,
    input_digests: Mapping[str, str],
    parent_digests_by_source: Mapping[str, Sequence[str]] | None = None,
    attempt_ids_by_source: Mapping[str, str] | None = None,
    continue_on_error: bool = False,
    runtime: AdapterRuntime | None = None,
) -> ProjectionBatchEvidenceSubmission:
    """Async bounded batch execution followed by ordered evidence retention."""

    if not isinstance(batch, ProjectionBatchRequest):
        raise ArgumentError("batch must be a ProjectionBatchRequest")
    result = (runtime or AdapterRuntime()).execute_batch(batch)
    return await submit_projection_batch_evidence_async(
        sink,
        result,
        group_id,
        domains,
        subject_id=subject_id,
        input_digests=input_digests,
        parent_digests_by_source=parent_digests_by_source,
        attempt_ids_by_source=attempt_ids_by_source,
        continue_on_error=continue_on_error,
    )


__all__ = [
    "MAX_SUBMISSION_ERROR_DETAIL_BYTES",
    "AdapterEvidenceSink",
    "AsyncAdapterEvidenceSink",
    "AdapterEvidenceSubmission",
    "ProjectionBatchEvidenceSubmission",
    "submit_adapter_execution_evidence",
    "submit_projection_batch_evidence",
    "execute_and_submit_projection",
    "execute_and_submit_projection_batch",
    "submit_adapter_execution_evidence_async",
    "submit_projection_batch_evidence_async",
    "execute_and_submit_projection_async",
    "execute_and_submit_projection_batch_async",
]
