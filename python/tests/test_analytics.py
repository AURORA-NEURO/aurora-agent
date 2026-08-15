from __future__ import annotations

import json
from pathlib import Path
import sys
import unittest
from unittest.mock import AsyncMock, patch

from prism_sdk import (
    AdapterPlanRequest,
    AdapterRegistry,
    AnalyticsDirection,
    AnalyticsEvidence,
    AsyncClient,
    AsyncWorkspace,
    CalibrationObservation,
    CapabilityAuditGroupReport,
    CapabilityAuditReport,
    CapabilityGroupReport,
    CapabilityMatchReport,
    CapabilityQuery,
    CapabilitySearchReport,
    CapabilityRouteReport,
    CapabilityRouteNeed,
    CapabilityRouteRequest,
    CapabilityRouteReviewReport,
    CapabilityRouteReviewRequest,
    CapabilitySchemaQualityReport,
    DeliveryReadinessReport,
    DeveloperDeliveryAuditReport,
    developer_delivery_audit_report,
    BioCapabilityEvidenceAuditReport,
    BioCapabilityEvidenceAuditRequest,
    ClaimRequest,
    EvidenceItem,
    EvidenceDimensionReport,
    biocapability_evidence_audit_report,
    BioAtlasPublicationAuditReport,
    bioatlas_publication_audit_report,
    capability_audit_report,
    capability_discover_report,
    capability_route_report,
    capability_route_review_report,
    Client,
    MetricObservation,
    MissionBinding,
    MissionPolicy,
    MissionRequest,
    MissionStep,
    PairedObservation,
    WorkbenchRequest,
    Workspace,
    PlanStatus,
    SourceKind,
    analytics_request,
)
from prism_sdk.errors import ArgumentError


ROOT = Path(__file__).parent
FAKE = ROOT / "fake_mcp_server.py"


def command() -> list[str]:
    return [sys.executable, "-u", str(FAKE)]


def observation() -> MetricObservation:
    return MetricObservation(
        id="verification-1",
        dimension="verification",
        domain="oncology",
        system="agent-a",
        value=0.8,
        direction=AnalyticsDirection.HIGHER_IS_BETTER,
        unit="fraction",
        condition="pack/4",
        replicate_group="world-1",
        cost=4.0,
        latency_ms=20.0,
        evidence=AnalyticsEvidence.REPRODUCED,
    )


def route_report_payload() -> dict:
    return {
        "ok": True,
        "workflow": "capability_route",
        "route_id": "r" * 64,
        "catalog_digest": "c" * 64,
        "goal": "compose evidence",
        "needs": [
            {
                "id": "oncology",
                "resolution": "ranked_candidates",
                "candidate_groups": ["oncology"],
                "candidate_domains": ["oncology"],
                "candidate_tools": ["oncology_search"],
                "search": {"matches": [{"group": {"id": "oncology"}}]},
            }
        ],
        "unresolved_needs": [],
        "recommended_tools": ["oncology_search"],
        "recommended_tool_count": 1,
        "recommended_tool_overflow": 0,
        "route_coverage": {
            "needs_total": 1,
            "needs_resolved": 1,
            "needs_unresolved": 0,
            "candidate_group_count": 1,
            "candidate_groups": ["oncology"],
            "candidate_domain_count": 1,
            "candidate_domains": ["oncology"],
            "candidate_tool_count": 1,
            "posture": "routing evidence only",
        },
        "schema_attachment": {
            "requested": True,
            "returned": 1,
            "missing": [],
        },
        "execution": "not_started",
        "guarantees": ["no routed tool was executed"],
        "limitations": ["candidate ranking is not authorization"],
    }


def route_review_payload() -> dict:
    return {
        "ok": True,
        "workflow": "capability_route_review",
        "review_id": "v" * 64,
        "route_id": "r" * 64,
        "catalog_digest": "c" * 64,
        "goal": "compose evidence",
        "need_count": 1,
        "selection_count": 1,
        "missing_needs": [],
        "selected_tools": ["oncology_search"],
        "selected_domains": ["oncology"],
        "dependency_waves": [["oncology"]],
        "findings": [],
        "review_status": "ready",
        "handoff_status": "mission_preflight_required",
        "mission_draft": {
            "goal": "compose evidence",
            "steps": [{"id": "oncology", "tool": "oncology_search"}],
            "dependency_waves": [["oncology"]],
        },
        "execution": "not_started",
        "route_coverage": {"needs_total": 1, "needs_resolved": 1},
        "schema_review": {
            "requested": True,
            "checked": 1,
            "valid": True,
            "fully_checked": True,
            "reports": [],
        },
    }


def capability_discover_payload() -> dict:
    return {
        "ok": True,
        "workflow": "capability_discover",
        "capability_schema_version": "bioprism-devplat-capability/0.1",
        "schema_version": "bioprism-devplat-capability/0.1",
        "catalog_digest": "c" * 64,
        "total_groups": 1,
        "query": {"query": "oncology", "max_items": 5, "include_tools": True},
        "result_count": 1,
        "matches": [
            {
                "group": {
                    "id": "oncology",
                    "domains": ["oncology"],
                    "crates": ["bioprism-onco"],
                    "mcp_tools": ["onco_response_assess"],
                    "cli_entrypoints": ["bioprism onco"],
                    "python_artifacts": ["prism_sdk.onco"],
                    "status": "implemented",
                },
                "score": 640,
                "matched_fields": ["domains", "mcp_tools"],
                "matched_tools": ["onco_response_assess"],
                "tool_schemas": [
                    {"name": "onco_response_assess", "inputSchema": {"type": "object"}}
                ],
            }
        ],
        "schema_attachment": {
            "requested": True,
            "returned": 1,
            "missing": [],
            "authoritative_source": "tools/list definition set",
        },
    }


def capability_audit_payload() -> dict:
    return {
        "ok": True,
        "workflow": "capability_audit",
        "capability_schema_version": "bioprism-devplat-capability/0.1",
        "catalog_digest": "c" * 64,
        "healthy": True,
        "total_groups": 1,
        "catalog_tool_memberships": 1,
        "unique_catalog_tools": 1,
        "advertised_tool_count": 1,
        "catalog_only_tools": [],
        "advertised_only_tools": [],
        "duplicate_schema_names": [],
        "duplicate_group_memberships": [],
        "schema_quality": {
            "checked": 1,
            "valid": 1,
            "total_bytes": 128,
            "maximum_schema_bytes": 1_000_000,
            "findings": [],
        },
        "invariants": {
            "every_catalog_tool_has_authoritative_schema": True,
            "every_advertised_tool_is_catalogued": True,
            "schema_names_are_unique": True,
            "all_input_schemas_are_well_formed": True,
            "multi_group_membership_is_allowed": True,
        },
        "groups": [
            {
                "id": "oncology",
                "domains": ["oncology"],
                "status": "implemented",
                "declared_tool_memberships": 1,
                "unique_tools": 1,
                "schemas_found": 1,
                "missing_schemas": [],
            }
        ],
    }


def developer_delivery_audit_payload() -> dict:
    return {
        "ok": True,
        "workflow": "developer_delivery_audit",
        "platform": {},
        "repository": {},
        "repository_impact": None,
        "sdk": {},
        "conformance": {},
        "provider": {},
        "governance": {},
        "release": {},
        "readiness": {
            "platform_checks_clean": True,
            "unguarded_claims": 0,
            "developer_claims_ready": True,
            "repository_scope_clean": True,
            "repository_impact_clean": False,
            "sdk_admission_clean": True,
            "conformance_release": True,
            "provider_capability_gate_cleared": True,
            "governance_document_clean": True,
            "release_audit_ready": True,
            "local_delivery_ready": True,
        },
        "external_surface_posture": {
            "foreign_subject_count": 2,
            "foreign_artifacts_present": True,
            "foreign_artifacts_are_not_inferred": True,
            "local_integration_foundations": [{"artifact": "prism_sdk", "kind": "client"}],
            "unverified_surface_families": ["typescript_sdk"],
        },
        "release_request": {
            "present": True,
            "id": "delivery-1",
            "targets": [
                {
                    "target": "local_delivery",
                    "available": True,
                    "eligible": True,
                    "blockers": [],
                    "notes": ["bounded local delivery"],
                }
            ],
            "ready": True,
            "fail_closed": False,
            "no_implicit_release": True,
            "available_target_count": 10,
        },
        "guarantees": ["no implicit release"],
        "limitations": ["external execution remains outside the workflow"],
    }


def biocapability_evidence_audit_payload() -> dict:
    return {
        "ok": True,
        "workflow": "biocapability_evidence_conditioned_profile",
        "metrics": {"ok": True, "coverage": {"measured": 1}},
        "metrics_ok": True,
        "evidence": {
            "items": [
                {
                    "index": 0,
                    "ok": True,
                    "id": "evidence-1",
                    "dimension": "evidence_grounding",
                    "domain": "oncology",
                    "declared_status": "observed",
                    "effective_status": "observed",
                    "issues": [],
                    "support": {"source": "ledger", "scope": "pack/1"},
                    "fail_closed": False,
                }
            ],
            "omitted_items": 0,
            "item_count": 1,
            "invalid_item_count": 0,
            "dimensions": [
                {
                    "dimension": "evidence_grounding",
                    "state": "observed",
                    "evidence_count": 1,
                    "measured_count": 1,
                    "declared_count": 0,
                    "blocked_count": 0,
                    "missing": False,
                    "measured": True,
                }
            ],
            "domains": {"oncology": 1},
        },
        "claim_requests": {
            "rows": [
                {
                    "index": 0,
                    "ok": True,
                    "id": "claim-1",
                    "claim": "grounded profile",
                    "requires": ["temporal_validity"],
                    "allow_declared": False,
                    "eligible": False,
                    "blockers": [
                        {
                            "dimension": "temporal_validity",
                            "state": "missing",
                            "reason": "missing, blocked, or non-applicable evidence",
                        }
                    ],
                    "explicit_assumptions": [],
                    "fail_closed": True,
                }
            ],
            "omitted_rows": 0,
            "requested": 1,
            "eligible": 0,
            "all_requested_claims_eligible": False,
        },
        "subaudits": {
            "information_value": None,
            "reference_quality": None,
            "temporal_validity": None,
            "reproducibility": None,
        },
        "release_posture": {
            "ready_for_requested_claims": False,
            "requires_explicit_claim_request": False,
            "numeric_scores_are_not_claims_without_evidence": True,
            "declared_evidence_is_visible_but_not_measured_support": True,
        },
        "guarantees": ["declared evidence is not measured support"],
        "limitations": ["no external dataset was inspected"],
    }


def biocapability_request() -> BioCapabilityEvidenceAuditRequest:
    return BioCapabilityEvidenceAuditRequest(
        evidence=[EvidenceItem("evidence-1", "evidence_grounding", "observed", support={"source": "ledger"})],
        claim_requests=[ClaimRequest("claim-1", "grounded profile", ["evidence_grounding"])],
        metrics={"observations": []},
    )


def bioatlas_publication_audit_payload() -> dict:
    return {
        "ok": True,
        "workflow": "bioatlas_publication_audit",
        "atlas": {"ok": True, "summary": {"coverage_supports_aggregation": True}},
        "evidence_audit": None,
        "card": None,
        "leaderboard": None,
        "release_request": {
            "present": True,
            "id": "publication-1",
            "targets": [{"target": "atlas_profile", "eligible": True, "blockers": [], "notes": []}],
            "ready": True,
            "fail_closed": False,
            "no_implicit_release": True,
        },
        "cross_layer": {
            "numeric_score_requires_evidence_audit": True,
            "numeric_score_evidence_ready": False,
            "atlas_aggregation_ready": True,
            "leaderboard_ranked_count": 3,
            "leaderboard_unranked_count": 1,
            "unranked_leaderboard_entries_remain_visible": True,
            "withheld_scores_are_not_zeroes": True,
        },
        "guarantees": ["publication targets are explicit"],
        "limitations": ["no network publisher"],
    }


class AnalyticsModelTests(unittest.TestCase):
    def test_biological_adapter_registry_distinguishes_dependency_states(self) -> None:
        request = AdapterPlanRequest(
            "scan-1",
            SourceKind.BYTES,
            declared_format="APPLICATION/DICOM",
            available_dependencies=["pydicom"],
        )
        plan = AdapterRegistry().plan(request, check_environment=False)
        self.assertTrue(plan.executable)
        self.assertEqual(plan.selected_adapter.id, "bioprism.python.dicom")

        unknown = AdapterRegistry().plan(
            AdapterPlanRequest("scan-1", SourceKind.BYTES, declared_format="application/dicom"),
            check_environment=False,
        )
        self.assertEqual(unknown.candidates[0].status, PlanStatus.DEPENDENCY_UNKNOWN)

        text_vcf = AdapterRegistry().plan(
            AdapterPlanRequest("variants", SourceKind.BYTES, declared_format="text/vcf"),
            check_environment=False,
        )
        self.assertTrue(text_vcf.executable)
        self.assertEqual(text_vcf.selected_adapter.id, "bioprism.python.vcf_text")

        bids = AdapterRegistry().plan(
            AdapterPlanRequest("bids-demo", SourceKind.BYTES, declared_format="application/bids-manifest"),
            check_environment=False,
        )
        self.assertTrue(bids.executable)
        self.assertEqual(bids.selected_adapter.id, "bioprism.python.bids_manifest")

        dicom_metadata = AdapterRegistry().plan(
            AdapterPlanRequest("ct-metadata", SourceKind.BYTES, declared_format="application/dicom-manifest"),
            check_environment=False,
        )
        self.assertTrue(dicom_metadata.executable)
        self.assertEqual(dicom_metadata.selected_adapter.id, "bioprism.python.dicom_metadata")

        nifti_metadata = AdapterRegistry().plan(
            AdapterPlanRequest("bold-header", SourceKind.BYTES, declared_format="application/nifti-manifest"),
            check_environment=False,
        )
        self.assertTrue(nifti_metadata.executable)
        self.assertEqual(nifti_metadata.selected_adapter.id, "bioprism.python.nifti_metadata")

        anndata_metadata = AdapterRegistry().plan(
            AdapterPlanRequest("cells", SourceKind.BYTES, declared_format="application/anndata-manifest"),
            check_environment=False,
        )
        self.assertTrue(anndata_metadata.executable)
        self.assertEqual(anndata_metadata.selected_adapter.id, "bioprism.python.anndata_metadata")

        alignment_metadata = AdapterRegistry().plan(
            AdapterPlanRequest("reads", SourceKind.BYTES, declared_format="application/alignment-manifest"),
            check_environment=False,
        )
        self.assertTrue(alignment_metadata.executable)
        self.assertEqual(alignment_metadata.selected_adapter.id, "bioprism.python.alignment_metadata")

        fhir_manifest = AdapterRegistry().plan(
            AdapterPlanRequest("clinical", SourceKind.BYTES, declared_format="application/fhir-manifest"),
            check_environment=False,
        )
        self.assertTrue(fhir_manifest.executable)
        self.assertEqual(fhir_manifest.selected_adapter.id, "bioprism.python.fhir_manifest")

        fhir_json = AdapterRegistry().plan(
            AdapterPlanRequest("clinical-json", SourceKind.BYTES, declared_format="application/fhir+json"),
            check_environment=False,
        )
        self.assertTrue(fhir_json.executable)
        self.assertEqual(fhir_json.selected_adapter.id, "bioprism.python.fhir_json")

        fhir_ndjson = AdapterRegistry().plan(
            AdapterPlanRequest("clinical-bulk", SourceKind.BYTES, declared_format="application/fhir+ndjson"),
            check_environment=False,
        )
        self.assertTrue(fhir_ndjson.executable)
        self.assertEqual(fhir_ndjson.selected_adapter.id, "bioprism.python.fhir_ndjson")

        fastq = AdapterRegistry().plan(
            AdapterPlanRequest("sequencing", SourceKind.BYTES, declared_format="text/fastq"),
            check_environment=False,
        )
        self.assertTrue(fastq.executable)
        self.assertEqual(fastq.selected_adapter.id, "bioprism.python.fastq_text")

        sam = AdapterRegistry().plan(
            AdapterPlanRequest("alignments", SourceKind.BYTES, declared_format="text/sam"),
            check_environment=False,
        )
        self.assertTrue(sam.executable)
        self.assertEqual(sam.selected_adapter.id, "bioprism.python.sam_text")

        mzml = AdapterRegistry().plan(
            AdapterPlanRequest("proteomics", SourceKind.BYTES, declared_format="application/mzml"),
            check_environment=False,
        )
        self.assertTrue(mzml.executable)
        self.assertEqual(mzml.selected_adapter.id, "bioprism.python.mzml_text")

        fasta = AdapterRegistry().plan(
            AdapterPlanRequest("reference", SourceKind.BYTES, declared_format="text/fasta"),
            check_environment=False,
        )
        self.assertTrue(fasta.executable)
        self.assertEqual(fasta.selected_adapter.id, "bioprism.python.fasta_text")

        gff3 = AdapterRegistry().plan(
            AdapterPlanRequest("annotation", SourceKind.BYTES, declared_format="text/gff3"),
            check_environment=False,
        )
        self.assertTrue(gff3.executable)
        self.assertEqual(gff3.selected_adapter.id, "bioprism.python.gff3_text")

        bed = AdapterRegistry().plan(
            AdapterPlanRequest("intervals", SourceKind.BYTES, declared_format="text/bed"),
            check_environment=False,
        )
        self.assertTrue(bed.executable)
        self.assertEqual(bed.selected_adapter.id, "bioprism.python.bed_text")

        pdb = AdapterRegistry().plan(
            AdapterPlanRequest("structure", SourceKind.BYTES, declared_format="chemical/x-pdb"),
            check_environment=False,
        )
        self.assertTrue(pdb.executable)
        self.assertEqual(pdb.selected_adapter.id, "bioprism.python.pdb_text")

        sdf = AdapterRegistry().plan(
            AdapterPlanRequest("molecules", SourceKind.BYTES, declared_format="chemical/x-mdl-sdfile"),
            check_environment=False,
        )
        self.assertTrue(sdf.executable)
        self.assertEqual(sdf.selected_adapter.id, "bioprism.python.sdf_text")

    def test_biological_adapter_request_refuses_implicit_format_sniffing(self) -> None:
        with self.assertRaises(ArgumentError):
            AdapterPlanRequest("", SourceKind.BYTES)
        plan = AdapterRegistry().plan(
            AdapterPlanRequest("variants", SourceKind.BYTES, declared_format="application/octet-stream"),
            check_environment=False,
        )
        self.assertFalse(plan.executable)

    def test_models_emit_the_exact_rust_wire_shape(self) -> None:
        request = analytics_request(
            [observation()],
            pairs=[
                PairedObservation(
                    "robustness-1",
                    "robustness",
                    "oncology",
                    0.9,
                    0.72,
                    AnalyticsDirection.HIGHER_IS_BETTER,
                    0.2,
                )
            ],
            calibration=[CalibrationObservation("forecast-1", "oncology", 0.9, 1.0)],
            calibration_bins=5,
        )
        arguments = request.to_mcp_arguments()
        self.assertEqual(arguments["observations"][0]["evidence"], "reproduced")
        self.assertEqual(arguments["pairs"][0]["direction"], "higher_is_better")
        self.assertEqual(arguments["calibration"][0]["predicted"], 0.9)
        self.assertEqual(arguments["calibration_bins"], 5)

    def test_models_fail_closed_on_probability_and_bin_bounds(self) -> None:
        with self.assertRaises(ArgumentError):
            CalibrationObservation("bad", "domain", 1.1, 0.5)
        with self.assertRaises(ArgumentError):
            analytics_request([], calibration_bins=1)

    def test_workbench_request_preserves_nested_contracts(self) -> None:
        request = WorkbenchRequest(
            {"session_id": "studio-1", "artifacts": [], "cells": [], "changes": []},
            dashboard={"include_holes": True},
            ci={"offline": True},
        )
        arguments = request.to_mcp_arguments()
        self.assertEqual(arguments["dashboard"]["include_holes"], True)
        self.assertEqual(arguments["ci"]["offline"], True)
        with self.assertRaises(ArgumentError):
            WorkbenchRequest({})

    def test_mission_request_builds_dependency_bound_wire_contract(self) -> None:
        request = MissionRequest(
            "mission-1",
            "compose evidence",
            [
                MissionStep("catalog", "workspace", "discovery", "discover routes", "workspace_capabilities"),
                MissionStep(
                    "metrics",
                    "metrics",
                    "analytics",
                    "prepare measurements",
                    "metrics_analytics_audit",
                    {"observations": [], "inputs": [None]},
                    ("catalog",),
                    True,
                    (MissionBinding("catalog", "/value", "/inputs/0"),),
                ),
            ],
            MissionPolicy(execute=True, allowed_tools=("workspace_capabilities", "metrics_analytics_audit")),
        )
        arguments = request.to_mcp_arguments()
        self.assertEqual(arguments["steps"][1]["depends_on"], ["catalog"])
        self.assertEqual(arguments["steps"][1]["bindings"][0]["target_pointer"], "/inputs/0")
        self.assertEqual(arguments["policy"]["allowed_tools"], ["workspace_capabilities", "metrics_analytics_audit"])
        with self.assertRaises(ArgumentError):
            MissionRequest("", "goal", [MissionStep("s", "d", "c", "o", "tool")])

    def test_capability_query_builds_bounded_cross_domain_wire_contract(self) -> None:
        query = CapabilityQuery(query="oncology evidence", max_items=3, include_tools=True)
        self.assertEqual(
            query.to_mcp_arguments(),
            {"query": "oncology evidence", "max_items": 3, "include_tools": True},
        )
        with self.assertRaises(ArgumentError):
            CapabilityQuery(max_items=0)

    def test_capability_route_request_batches_named_needs_without_execution(self) -> None:
        request = CapabilityRouteRequest(
            "compose evidence",
            [
                CapabilityRouteNeed("oncology", CapabilityQuery(query="oncology")),
                {"id": "release", "tool": "bundle_verify"},
            ],
            max_candidates_per_need=2,
            max_tools=4,
            include_tools=True,
        )
        arguments = request.to_mcp_arguments()
        self.assertEqual(arguments["needs"][1]["tool"], "bundle_verify")
        self.assertEqual(arguments["max_tools"], 4)
        with self.assertRaises(ArgumentError):
            CapabilityRouteRequest("goal", [{"id": "same"}, {"id": "same"}])

    def test_capability_route_report_validates_direct_projection_and_coverage(self) -> None:
        report = CapabilityRouteReport.from_wire(route_report_payload())
        self.assertTrue(report.route_coverage.fully_resolved)
        self.assertEqual(report.resolved_needs[0].candidate_domains, ("oncology",))
        self.assertEqual(report.candidate_domains, ("oncology",))
        self.assertEqual(report.to_dict()["route_id"], "r" * 64)

    def test_capability_discover_report_preserves_cross_domain_context(self) -> None:
        report = CapabilitySearchReport.from_wire(capability_discover_payload())
        self.assertIsInstance(report.matches[0], CapabilityMatchReport)
        self.assertIsInstance(report.matches[0].group, CapabilityGroupReport)
        self.assertEqual(report.domains, ("oncology",))
        self.assertEqual(report.tools, ("onco_response_assess",))
        self.assertEqual(len(report.matches[0].tool_schemas), 1)

    def test_capability_discover_report_extracts_http_projection(self) -> None:
        report = capability_discover_report(
            {"ok": True, "mcp": {"result": {"structuredContent": capability_discover_payload()}}}
        )
        self.assertEqual(report.catalog_digest, "c" * 64)

    def test_capability_audit_report_reconciles_parity_and_quality(self) -> None:
        report = CapabilityAuditReport.from_wire(capability_audit_payload())
        self.assertTrue(report.healthy)
        self.assertTrue(report.catalogue_complete)
        self.assertTrue(report.schema_quality.fully_valid)
        self.assertIsInstance(report.schema_quality, CapabilitySchemaQualityReport)
        self.assertIsInstance(report.groups[0], CapabilityAuditGroupReport)
        self.assertEqual(report.groups[0].schemas_found, 1)

    def test_capability_audit_report_extracts_http_projection(self) -> None:
        report = capability_audit_report(
            {"ok": True, "mcp": {"result": {"structuredContent": capability_audit_payload()}}}
        )
        self.assertEqual(report.catalog_digest, "c" * 64)

    def test_developer_delivery_audit_report_preserves_fail_closed_readiness(self) -> None:
        report = DeveloperDeliveryAuditReport.from_wire(developer_delivery_audit_payload())
        self.assertIsInstance(report.readiness, DeliveryReadinessReport)
        self.assertTrue(report.readiness.claims_guarded)
        self.assertTrue(report.ready_for_requested_release)
        self.assertFalse(report.evidence_complete)
        self.assertEqual(report.release_request.targets[0].target, "local_delivery")
        self.assertTrue(report.external_surface_posture.foreign_posture_explicit)

    def test_developer_delivery_audit_report_extracts_http_projection(self) -> None:
        report = developer_delivery_audit_report(
            {"ok": True, "mcp": {"result": {"structuredContent": developer_delivery_audit_payload()}}}
        )
        self.assertEqual(report.release_request.request_id, "delivery-1")

    def test_developer_delivery_audit_report_rejects_duplicate_targets(self) -> None:
        payload = developer_delivery_audit_payload()
        payload["release_request"]["targets"].append(
            dict(payload["release_request"]["targets"][0])
        )
        with self.assertRaises(ArgumentError):
            DeveloperDeliveryAuditReport.from_wire(payload)

    def test_biocapability_evidence_audit_report_preserves_claim_blockers(self) -> None:
        report = BioCapabilityEvidenceAuditReport.from_wire(
            biocapability_evidence_audit_payload()
        )
        self.assertFalse(report.ready_for_requested_claims)
        self.assertEqual(report.domains, ("oncology",))
        self.assertIsInstance(report.evidence.dimensions[0], EvidenceDimensionReport)
        self.assertEqual(report.claim_requests.rows[0].blockers[0]["dimension"], "temporal_validity")
        self.assertTrue(report.claim_requests.rows[0].fail_closed)

    def test_biocapability_evidence_audit_report_extracts_http_projection(self) -> None:
        report = biocapability_evidence_audit_report(
            {
                "ok": True,
                "mcp": {
                    "result": {
                        "structuredContent": biocapability_evidence_audit_payload()
                    }
                },
            }
        )
        self.assertEqual(report.evidence.item_count, 1)

    def test_bioatlas_publication_audit_report_preserves_cross_layer_gates(self) -> None:
        report = BioAtlasPublicationAuditReport.from_wire(bioatlas_publication_audit_payload())
        self.assertTrue(report.ready_for_requested_publication)
        self.assertTrue(report.cross_layer.atlas_aggregation_ready)
        self.assertFalse(report.cross_layer.fully_ranked)
        self.assertFalse(report.has_evidence_audit)
        self.assertEqual(report.release_request.targets[0].target, "atlas_profile")

    def test_bioatlas_publication_audit_report_extracts_http_projection(self) -> None:
        report = bioatlas_publication_audit_report(
            {"ok": True, "mcp": {"result": {"structuredContent": bioatlas_publication_audit_payload()}}}
        )
        self.assertEqual(report.cross_layer.leaderboard_ranked_count, 3)

    def test_capability_route_report_extracts_http_json_text_projection(self) -> None:
        envelope = {
            "ok": True,
            "mcp": {
                "result": {
                    "content": [{"type": "text", "text": json.dumps(route_report_payload())}]
                }
            },
        }
        report = capability_route_report(envelope)
        self.assertEqual(report.goal, "compose evidence")
        self.assertEqual(report.route_coverage.candidate_domain_count, 1)

    def test_capability_route_report_rejects_inconsistent_coverage(self) -> None:
        payload = route_report_payload()
        payload["route_coverage"]["needs_resolved"] = 0
        with self.assertRaises(ArgumentError):
            CapabilityRouteReport.from_wire(payload)

    def test_capability_route_review_request_and_report_preserve_handoff_contract(self) -> None:
        route = route_report_payload()
        request = CapabilityRouteReviewRequest(
            route,
            [
                {
                    "need_id": "oncology",
                    "tool": "oncology_search",
                    "domain": "oncology",
                    "capability": "evidence",
                    "objective": "review evidence",
                    "arguments": {},
                }
            ],
            validate_schemas=True,
        )
        self.assertEqual(request.to_mcp_arguments()["selections"][0]["need_id"], "oncology")
        self.assertTrue(request.to_mcp_arguments()["validate_schemas"])
        report = CapabilityRouteReviewReport.from_wire(route_review_payload())
        self.assertTrue(report.ready)
        self.assertEqual(len(report.review_id), 64)
        self.assertEqual(report.dependency_waves, (("oncology",),))
        self.assertTrue(report.schema_review["valid"])

    def test_capability_route_review_report_extracts_http_structured_projection(self) -> None:
        envelope = {
            "ok": True,
            "mcp": {"result": {"structuredContent": route_review_payload()}},
        }
        report = capability_route_review_report(envelope)
        self.assertEqual(report.handoff_status, "mission_preflight_required")


class AnalyticsWorkspaceTests(unittest.TestCase):
    def test_sync_workspace_sends_typed_analytics_request(self) -> None:
        with Client(command(), timeout=2) as client:
            result = Workspace(client).metrics_analytics_audit(
                [observation()],
                calibration=[CalibrationObservation("forecast-1", "oncology", 0.9, 1.0)],
            )
        self.assertEqual(result["echo"]["observations"][0]["dimension"], "verification")
        self.assertEqual(result["echo"]["calibration"][0]["observed"], 1.0)

    def test_sync_workspace_exposes_workbench_composition(self) -> None:
        with Client(command(), timeout=2) as client:
            result = Workspace(client).developer_workbench(
                {"session_id": "studio-1", "artifacts": [], "cells": [], "changes": []},
                dashboard={"include_holes": True},
                ci={"offline": True},
            )
        self.assertEqual(result["echo"]["session"]["session_id"], "studio-1")
        self.assertEqual(result["echo"]["ci"]["offline"], True)

    def test_sync_workspace_typed_biocapability_evidence_report(self) -> None:
        request = biocapability_request()
        with patch.object(
            Workspace,
            "biocapability_evidence_audit",
            return_value=biocapability_evidence_audit_payload(),
        ) as audit:
            report = Workspace(None).biocapability_evidence_audit_report(request)  # type: ignore[arg-type]
        self.assertFalse(report.ready_for_requested_claims)
        audit.assert_called_once_with(request)

    def test_sync_workspace_typed_bioatlas_publication_report(self) -> None:
        with patch.object(
            Workspace,
            "bioatlas_publication_audit",
            return_value=bioatlas_publication_audit_payload(),
        ) as audit:
            report = Workspace(None).bioatlas_publication_audit_report(  # type: ignore[arg-type]
                {"atlas_id": "atlas-1"}, request_id="publication-1", targets=["atlas_profile"]
            )
        self.assertTrue(report.ready_for_requested_publication)
        audit.assert_called_once_with(
            {"atlas_id": "atlas-1"}, request_id="publication-1", targets=["atlas_profile"]
        )

    def test_sync_workspace_typed_delivery_audit_report(self) -> None:
        with patch.object(
            Workspace,
            "developer_delivery_audit",
            return_value=developer_delivery_audit_payload(),
        ) as audit:
            report = Workspace(None).developer_delivery_audit_report(  # type: ignore[arg-type]
                request_id="delivery-1",
                targets=["local_delivery"],
            )
        self.assertTrue(report.ready_for_requested_release)
        audit.assert_called_once_with(
            request_id="delivery-1",
            targets=["local_delivery"],
            platform=None,
            repository=None,
            repository_impact=None,
            sdk=None,
            conformance=None,
            provider=None,
            governance=None,
            release=None,
        )

    def test_sync_workspace_exposes_agent_mission(self) -> None:
        with Client(command(), timeout=2) as client:
            result = Workspace(client).agent_mission(
                "mission-sync",
                "discover capabilities",
                [MissionStep("catalog", "workspace", "discovery", "discover routes", "workspace_capabilities")],
            )
        self.assertEqual(result["echo"]["mission_id"], "mission-sync")
        self.assertEqual(result["echo"]["steps"][0]["tool"], "workspace_capabilities")

    def test_sync_workspace_exposes_capability_discovery(self) -> None:
        with Client(command(), timeout=2) as client:
            with self.assertRaises(ArgumentError):
                Workspace(client).capability_discover(query=object())  # type: ignore[arg-type]
            result = Workspace(client).capability_discover(query="oncology")
        self.assertEqual(result["echo"]["query"], "oncology")
        self.assertEqual(result["echo"]["include_tools"], False)

    def test_sync_workspace_typed_capability_discovery_report(self) -> None:
        with patch.object(
            Workspace, "capability_discover", return_value=capability_discover_payload()
        ) as discover:
            report = Workspace(None).capability_discover_report(query="oncology")  # type: ignore[arg-type]
        self.assertEqual(report.domains, ("oncology",))
        discover.assert_called_once_with(
            query="oncology",
            text=None,
            domain=None,
            tool=None,
            group_id=None,
            max_items=50,
            include_tools=False,
        )

    def test_sync_workspace_exposes_capability_audit(self) -> None:
        with Client(command(), timeout=2) as client:
            result = Workspace(client).capability_audit(include_groups=False)
        self.assertEqual(result["echo"], {"include_groups": False})

    def test_sync_workspace_typed_capability_audit_report(self) -> None:
        with patch.object(Workspace, "capability_audit", return_value=capability_audit_payload()) as audit:
            report = Workspace(None).capability_audit_report(include_groups=False)  # type: ignore[arg-type]
        self.assertTrue(report.catalogue_complete)
        audit.assert_called_once_with(include_groups=False)

    def test_sync_workspace_exposes_capability_route(self) -> None:
        with Client(command(), timeout=2) as client:
            result = Workspace(client).capability_route(
                "compose evidence",
                [{"id": "oncology", "query": "oncology"}],
                max_tools=4,
            )
        self.assertEqual(result["echo"]["goal"], "compose evidence")
        self.assertEqual(result["echo"]["needs"][0]["id"], "oncology")

    def test_sync_workspace_typed_capability_route_report_delegates_to_raw_route(self) -> None:
        with patch.object(Workspace, "capability_route", return_value=route_report_payload()) as route:
            report = Workspace(None).capability_route_report("compose evidence", [{"id": "oncology"}])  # type: ignore[arg-type]
        self.assertTrue(report.route_coverage.fully_resolved)
        route.assert_called_once_with(
            "compose evidence",
            [{"id": "oncology"}],
            max_candidates_per_need=10,
            max_tools=128,
            include_tools=False,
        )

    def test_sync_workspace_exposes_adapter_planning(self) -> None:
        with Client(command(), timeout=2) as client:
            result = Workspace(client).adapter_plan(
                "scan-1",
                "bytes",
                declared_format="application/dicom",
                available_dependencies=["pydicom"],
            )
        self.assertEqual(result["echo"]["source_id"], "scan-1")
        self.assertEqual(result["echo"]["available_dependencies"], ["pydicom"])


class AsyncAnalyticsWorkspaceTests(unittest.IsolatedAsyncioTestCase):
    async def test_async_workspace_matches_sync_surface(self) -> None:
        async with AsyncClient(command(), timeout=2) as client:
            result = await AsyncWorkspace(client).metrics_analytics_audit(
                [observation()], calibration_bins=7
            )
        self.assertEqual(result["echo"]["calibration_bins"], 7)

    async def test_async_workspace_exposes_workbench(self) -> None:
        async with AsyncClient(command(), timeout=2) as client:
            result = await AsyncWorkspace(client).developer_workbench(
                {"session_id": "studio-async", "artifacts": [], "cells": [], "changes": []}
            )
        self.assertEqual(result["echo"]["session"]["session_id"], "studio-async")

    async def test_async_workspace_typed_biocapability_evidence_report(self) -> None:
        request = biocapability_request()
        with patch.object(
            AsyncWorkspace,
            "biocapability_evidence_audit",
            new_callable=AsyncMock,
            return_value=biocapability_evidence_audit_payload(),
        ) as audit:
            report = await AsyncWorkspace(None).biocapability_evidence_audit_report(  # type: ignore[arg-type]
                request
            )
        self.assertEqual(report.claim_requests.requested, 1)
        audit.assert_awaited_once_with(request)

    async def test_async_workspace_typed_bioatlas_publication_report(self) -> None:
        with patch.object(
            AsyncWorkspace,
            "bioatlas_publication_audit",
            new_callable=AsyncMock,
            return_value=bioatlas_publication_audit_payload(),
        ) as audit:
            report = await AsyncWorkspace(None).bioatlas_publication_audit_report(  # type: ignore[arg-type]
                {"atlas_id": "atlas-1"}, request_id="publication-1", targets=["atlas_profile"]
            )
        self.assertTrue(report.cross_layer.atlas_aggregation_ready)
        audit.assert_awaited_once_with(
            {"atlas_id": "atlas-1"}, request_id="publication-1", targets=["atlas_profile"]
        )

    async def test_async_workspace_typed_delivery_audit_report(self) -> None:
        with patch.object(
            AsyncWorkspace,
            "developer_delivery_audit",
            new_callable=AsyncMock,
            return_value=developer_delivery_audit_payload(),
        ) as audit:
            report = await AsyncWorkspace(None).developer_delivery_audit_report(  # type: ignore[arg-type]
                request_id="delivery-1",
                targets=["local_delivery"],
            )
        self.assertTrue(report.readiness.local_delivery_ready)
        audit.assert_awaited_once_with(
            request_id="delivery-1",
            targets=["local_delivery"],
            platform=None,
            repository=None,
            repository_impact=None,
            sdk=None,
            conformance=None,
            provider=None,
            governance=None,
            release=None,
        )

    async def test_async_workspace_exposes_agent_mission(self) -> None:
        async with AsyncClient(command(), timeout=2) as client:
            result = await AsyncWorkspace(client).agent_mission(
                "mission-async",
                "discover capabilities",
                [MissionStep("catalog", "workspace", "discovery", "discover routes", "workspace_capabilities")],
            )
        self.assertEqual(result["echo"]["mission_id"], "mission-async")

    async def test_async_workspace_exposes_capability_discovery(self) -> None:
        async with AsyncClient(command(), timeout=2) as client:
            result = await AsyncWorkspace(client).capability_discover(domain="release", max_items=2)
        self.assertEqual(result["echo"]["domain"], "release")
        self.assertEqual(result["echo"]["max_items"], 2)

    async def test_async_workspace_typed_capability_discovery_report(self) -> None:
        with patch.object(
            AsyncWorkspace,
            "capability_discover",
            new_callable=AsyncMock,
            return_value=capability_discover_payload(),
        ) as discover:
            report = await AsyncWorkspace(None).capability_discover_report(  # type: ignore[arg-type]
                query="oncology"
            )
        self.assertEqual(report.tools, ("onco_response_assess",))
        discover.assert_awaited_once_with(
            query="oncology",
            text=None,
            domain=None,
            tool=None,
            group_id=None,
            max_items=50,
            include_tools=False,
        )

    async def test_async_workspace_exposes_capability_audit(self) -> None:
        async with AsyncClient(command(), timeout=2) as client:
            result = await AsyncWorkspace(client).capability_audit()
        self.assertEqual(result["echo"], {"include_groups": True})

    async def test_async_workspace_typed_capability_audit_report(self) -> None:
        with patch.object(
            AsyncWorkspace,
            "capability_audit",
            new_callable=AsyncMock,
            return_value=capability_audit_payload(),
        ) as audit:
            report = await AsyncWorkspace(None).capability_audit_report(  # type: ignore[arg-type]
                include_groups=True
            )
        self.assertTrue(report.schema_quality.fully_valid)
        audit.assert_awaited_once_with(include_groups=True)

    async def test_async_workspace_exposes_capability_route(self) -> None:
        async with AsyncClient(command(), timeout=2) as client:
            result = await AsyncWorkspace(client).capability_route(
                "compose evidence",
                [CapabilityRouteNeed("release", CapabilityQuery(tool="bundle_verify"))],
            )
        self.assertEqual(result["echo"]["needs"][0]["tool"], "bundle_verify")

    async def test_async_workspace_typed_capability_route_report_delegates_to_raw_route(self) -> None:
        with patch.object(
            AsyncWorkspace,
            "capability_route",
            new_callable=AsyncMock,
            return_value=route_report_payload(),
        ) as route:
            report = await AsyncWorkspace(None).capability_route_report(  # type: ignore[arg-type]
                "compose evidence", [{"id": "oncology"}]
            )
        self.assertEqual(report.recommended_tools, ("oncology_search",))
        route.assert_awaited_once_with(
            "compose evidence",
            [{"id": "oncology"}],
            max_candidates_per_need=10,
            max_tools=128,
            include_tools=False,
        )

    async def test_async_workspace_exposes_adapter_planning(self) -> None:
        async with AsyncClient(command(), timeout=2) as client:
            result = await AsyncWorkspace(client).adapter_plan(
                "variants-1",
                "bytes",
                declared_format="text/vcf",
                available_dependencies=["pysam"],
            )
        self.assertEqual(result["echo"]["declared_format"], "text/vcf")


if __name__ == "__main__":
    unittest.main()
