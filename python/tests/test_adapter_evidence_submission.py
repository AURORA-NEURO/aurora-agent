from __future__ import annotations

import asyncio

import pytest

from prism_sdk import (
    AdapterExecutionEvidenceReport,
    AdapterEvidenceSubmission,
    AdapterRuntime,
    ArgumentError,
    ProjectionBatchRequest,
    ProjectionRequest,
    TransportError,
    execute_and_submit_projection,
    execute_projection_batch,
    submit_adapter_execution_evidence,
    submit_adapter_execution_evidence_async,
    submit_projection_batch_evidence,
)


VCF = """##fileformat=VCFv4.3
##reference=GRCh38
#CHROM\tPOS\tID\tREF\tALT\tQUAL\tFILTER\tINFO
chr1\t10\t.\tA\tG\t50\tPASS\tDP=4
"""


def report_for(request) -> AdapterExecutionEvidenceReport:
    evidence = request.to_mcp_arguments()
    evidence.update(
        {
            "schema": "bioprism-devplat-adapter-execution-evidence/0.1",
            "workflow": "adapter_execution_evidence",
            "attestation_posture": "caller_asserted",
            "evidence_digest": "f" * 64,
        }
    )
    return AdapterExecutionEvidenceReport.from_wire(
        {
            "ok": True,
            "schema": "bioprism-devplat-adapter-execution-evidence/0.1",
            "workflow": "adapter_execution_evidence",
            "evidence": evidence,
            "adapter": {
                "id": request.adapter_id,
                "version": request.adapter_version,
                "execution": "python_delegated",
                "conformance_level": "normalize",
                "declared_loss_kinds": [],
                "scope_dimensions": ["subject", "sample"],
            },
            "evidence_digest": "f" * 64,
            "attestation_posture": "caller_asserted",
            "artifact_registry": {"indexed": True, "created": True},
            "execution": "not_started",
            "readiness_claimed": False,
        }
    )


class FakeSink:
    def __init__(self, failing_sources: tuple[str, ...] = ()) -> None:
        self.failing_sources = set(failing_sources)
        self.requests = []

    def adapter_execution_evidence_report(self, request):
        self.requests.append(request)
        if request.source_id in self.failing_sources:
            raise TransportError(f"sink unavailable for {request.source_id}")
        return report_for(request)


class AsyncFakeSink(FakeSink):
    async def adapter_execution_evidence_report(self, request):
        return super().adapter_execution_evidence_report(request)


def runtime_result(source_id: str = "submit-vcf"):
    return AdapterRuntime().execute(
        ProjectionRequest(
            "bioprism.python.vcf_text",
            source_id,
            {"text": VCF},
        )
    )


def test_single_submission_runs_only_local_runtime_then_retains_report() -> None:
    sink = FakeSink()
    submission = submit_adapter_execution_evidence(
        sink,
        runtime_result(),
        "biological_domains",
        ("genomics",),
        subject_id="submission-subject",
        input_digest="a" * 64,
        attempt_id="attempt-1",
    )
    assert isinstance(submission, AdapterEvidenceSubmission)
    assert submission.retained is True
    assert submission.remote_execution_status == "partial"
    assert sink.requests[0].subject_id == "submission-subject"
    assert sink.requests[0].attempt_id == "attempt-1"


def test_batch_submission_preserves_transport_failure_per_member() -> None:
    batch = execute_projection_batch(
        (
            ProjectionRequest("bioprism.python.vcf_text", "submit-vcf", {"text": VCF}, max_items=10),
            ProjectionRequest(
                "bioprism.python.fhir_manifest",
                "submit-fhir",
                {"document": {"resourceType": "Patient", "id": "patient-1"}},
                max_items=10,
            ),
        ),
        max_total_items=20,
    )
    sink = FakeSink(("submit-fhir",))
    submission = submit_projection_batch_evidence(
        sink,
        batch,
        "biological_domains",
        ("genomics",),
        subject_id="batch-subject",
        input_digests={"submit-vcf": "a" * 64, "submit-fhir": "b" * 64},
        continue_on_error=True,
    )
    assert submission.attempted_count == 2
    assert submission.retained_count == 1
    assert submission.transport_error_count == 1
    assert submission.complete is False
    assert submission.submissions[1].error["kind"] == "TransportError"
    assert [request.source_id for request in sink.requests] == ["submit-vcf", "submit-fhir"]

    with pytest.raises(TransportError):
        submit_projection_batch_evidence(
            sink,
            batch,
            "biological_domains",
            ("genomics",),
            subject_id="batch-subject",
            input_digests={"submit-vcf": "a" * 64, "submit-fhir": "b" * 64},
        )


def test_execute_and_submit_validates_batch_inputs_before_network_calls() -> None:
    sink = FakeSink()
    result = execute_and_submit_projection(
        sink,
        ProjectionRequest("bioprism.python.vcf_text", "execute-submit", {"text": VCF}),
        "biological_domains",
        ("genomics",),
        subject_id="execute-submit-subject",
        input_digest="c" * 64,
    )
    assert result.retained is True

    batch = ProjectionBatchRequest(
        (ProjectionRequest("bioprism.python.vcf_text", "only-member", {"text": VCF}),)
    )
    with pytest.raises(ArgumentError):
        submit_projection_batch_evidence(
            sink,
            execute_projection_batch(batch.requests),
            "biological_domains",
            ("genomics",),
            subject_id="batch-subject",
            input_digests={},
        )


def test_async_submission_uses_the_same_typed_handoff() -> None:
    async def run() -> None:
        sink = AsyncFakeSink()
        submission = await submit_adapter_execution_evidence_async(
            sink,
            runtime_result("async-submit-vcf"),
            "biological_domains",
            ("genomics",),
            subject_id="async-subject",
            input_digest="d" * 64,
        )
        assert submission.retained is True
        assert submission.request.source_id == "async-submit-vcf"

    asyncio.run(run())
