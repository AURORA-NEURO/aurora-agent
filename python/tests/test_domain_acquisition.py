from __future__ import annotations

import pytest

from prism_sdk import (
    DOMAIN_ACQUISITION_SCHEMA,
    DomainAcquisitionQuery,
    ArgumentError,
    domain_acquisition_report,
)


def payload() -> dict:
    route = {
        "group_id": "biological_domains",
        "domain": "oncology",
        "declared_tool_count": 8,
        "transport": {
            "status": "bounded_file_http",
            "tools": ["domain_evidence_source_plan", "domain_evidence_source_execute", "domain_evidence_intake"],
            "bounded_connector_kinds": ["file", "generic_http"],
            "caller_managed_connector_kinds": ["fhir"],
            "limitations": ["not source authenticity"],
        },
        "interpretation": {
            "status": "python_delegated",
            "adapter_ids": ["bioprism.python.dicom"],
            "match_basis": ["declared scope overlap"],
            "declared_conformance": ["normalize"],
            "limitations": ["not semantic validation"],
        },
        "adapters": [{"id": "bioprism.python.dicom"}],
        "guarantees": ["catalogue-bound"],
        "limitations": ["source-specific audit required"],
    }
    return {
        "ok": True,
        "schema": DOMAIN_ACQUISITION_SCHEMA,
        "workflow": "domain_acquisition_catalogue",
        "catalogue": {
            "schema": DOMAIN_ACQUISITION_SCHEMA,
            "workflow": "domain_acquisition_catalogue",
            "digest": "a" * 64,
            "complete": True,
            "truncated": False,
            "selected_domain_count": 1,
            "routes": [route],
        },
        "execution": "not_started",
        "guarantees": ["separate planes"],
        "does_not_claim": ["scientific truth"],
    }


def test_query_is_bounded_and_serializes_explicit_filters() -> None:
    query = DomainAcquisitionQuery(group_id="biological", domain="oncology", include_adapters=True, max_domains=2)
    assert query.to_mcp_arguments()["group_id"] == "biological"
    assert query.to_mcp_arguments()["include_adapters"] is True
    with pytest.raises(ArgumentError):
        DomainAcquisitionQuery(max_domains=0)


def test_report_preserves_two_plane_route_and_digest() -> None:
    report = domain_acquisition_report(payload())
    assert report.complete is True
    assert report.selected_domain_count == 1
    assert report.routes[0].transport_status == "bounded_file_http"
    assert report.routes[0].interpretation_status == "python_delegated"
    assert report.routes[0].adapter_ids == ("bioprism.python.dicom",)
    assert report.digest == "a" * 64


def test_report_refuses_execution_or_schema_drift() -> None:
    invalid = payload()
    invalid["execution"] = "completed"
    with pytest.raises(ArgumentError):
        domain_acquisition_report(invalid)
