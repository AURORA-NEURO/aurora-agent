from __future__ import annotations

from dataclasses import replace

import pytest

from prism_sdk import (
    AdapterConformanceReport,
    AdapterRuntime,
    ArgumentError,
    ProjectionRequest,
    RuntimeStatus,
    adapter_conformance_profile,
    adapter_conformance_profiles,
    evaluate_adapter_conformance,
)


VCF = """##fileformat=VCFv4.3
##reference=GRCh38
#CHROM\tPOS\tID\tREF\tALT\tQUAL\tFILTER\tINFO
chr1\t10\t.\tA\tG\t50\tPASS\tDP=4
"""


def test_profiles_cover_every_concrete_and_catalogued_route() -> None:
    runtime = AdapterRuntime()
    profiles = adapter_conformance_profiles()
    profile_ids = {profile.adapter_id for profile in profiles}
    assert set(runtime.concrete_adapter_ids) <= profile_ids
    assert {"bioprism.inventory", "bioprism.tabular"} <= profile_ids
    assert len(profile_ids) == len(profiles)
    assert adapter_conformance_profile("bioprism.python.fhir_manifest").family == "clinical_interoperability"


def test_verified_lossy_variant_result_has_verified_structure_without_readiness_claim() -> None:
    result = AdapterRuntime().execute(
        ProjectionRequest("bioprism.python.vcf_text", "profile-vcf", {"text": VCF})
    )
    report = evaluate_adapter_conformance(result)
    assert isinstance(report, AdapterConformanceReport)
    assert result.status is RuntimeStatus.LOSSY
    assert report.status == "verified"
    assert report.verified is True
    assert report.failed_checks == ()
    assert report.to_wire()["report_digest"] == report.report_digest
    evidence = report.to_adapter_execution_evidence_request(
        result,
        "biological_domains",
        ("genomics",),
        subject_id="profile-subject",
        input_digest="a" * 64,
    )
    assert report.report_digest in evidence.parent_digests


def test_missing_or_failed_profile_checks_are_partial_and_visible() -> None:
    result = AdapterRuntime().execute(
        ProjectionRequest("bioprism.python.vcf_text", "profile-vcf", {"text": VCF})
    )
    document = dict(result.document)
    conformance = dict(document["conformance"])
    checks = dict(conformance["checks"])
    checks.pop("typed_value_projection")
    checks["record_structure"] = "fail"
    conformance["checks"] = checks
    document["conformance"] = conformance
    report = evaluate_adapter_conformance(replace(result, document=document))
    assert report.status == "partial"
    assert report.missing_checks == ("typed_value_projection",)
    assert report.failed_checks == ("record_structure",)


def test_clinical_and_catalogue_boundaries_do_not_become_verified_by_default() -> None:
    fhir = AdapterRuntime().execute(
        ProjectionRequest(
            "bioprism.python.fhir_manifest",
            "profile-fhir",
            {"document": {"resourceType": "Patient", "id": "patient-1"}},
        )
    )
    fhir_report = evaluate_adapter_conformance(fhir)
    assert fhir_report.profile.family == "clinical_interoperability"
    assert fhir_report.status == "refused"
    unsupported = AdapterRuntime().execute(
        ProjectionRequest("bioprism.tabular", "profile-tabular", {"columns": []})
    )
    unsupported_report = evaluate_adapter_conformance(unsupported)
    assert unsupported_report.status == "unsupported"
    assert unsupported_report.missing_checks == ("route_declared",)


def test_unknown_profile_and_mismatched_profile_fail_closed() -> None:
    with pytest.raises(ArgumentError):
        adapter_conformance_profile("unknown.adapter")
    result = AdapterRuntime().execute(
        ProjectionRequest("bioprism.python.vcf_text", "profile-vcf", {"text": VCF})
    )
    with pytest.raises(ArgumentError):
        evaluate_adapter_conformance(result, adapter_conformance_profile("bioprism.python.fhir_manifest"))
