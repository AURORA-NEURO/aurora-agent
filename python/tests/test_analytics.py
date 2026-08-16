from __future__ import annotations

import json
from pathlib import Path
import sys
import unittest
from unittest.mock import AsyncMock, patch

from prism_sdk import (
    AdapterDescriptorReport,
    AdapterPlanRequest,
    AdapterPlanCandidateReport,
    AdapterPlanProjection,
    AdapterPlanReport,
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
    ConformanceCaseReport,
    ConformancePyramidReport,
    ConformanceReleaseDecisionReport,
    ConformanceRunArgs,
    ConformanceRunReport,
    ConformanceSuiteReport,
    CiExecutionEvidenceRequest,
    DeliveryReadinessReport,
    DeveloperDeliveryAuditReport,
    DeveloperPlatformStatusArgs,
    DeveloperPlatformStatusReport,
    developer_delivery_audit_report,
    developer_platform_status_report,
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
    adapter_plan_report,
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
    TabularCheckReport,
    TabularConformanceReport,
    TabularIngestReport,
    TabularIngestRequest,
    TabularManifestReport,
    TabularSemanticLossReport,
    TokenContextPlanArgs,
    TokenContextPlanningReport,
    TokenContextRequest,
    TokenEstimate,
    TokenPlanCandidate,
    TokenPolicyComparisonReport,
    token_context_plan_report,
    WeaveLangCompileArgs,
    WeaveLangCompileReport,
    WeaveLangExecutionReport,
    WeaveLangLivenessReport,
    weavelang_compile_report,
    analytics_request,
    tabular_ingest_report,
    conformance_run_report,
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
            "ci_execution_evidence_ready": False,
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


def developer_platform_status_payload(*, include_details: bool = False) -> dict:
    walkthroughs = [
        {
            "id": "foreign",
            "goal": "verify a foreign SDK",
            "standing": {"standing": "entirely_outside", "claims": 2},
            "standing_text": "entirely outside",
            "steps": 2,
            "claims": 2,
            "guarded_claims": 0,
            "unguarded_claims": 2,
            "documents_absent_artifact": True,
            "refuted_claims": 0,
            "narration_permille": 0,
        },
        {
            "id": "mixed",
            "goal": "compare local and foreign evidence",
            "standing": {"standing": "partly_outside", "here": 1, "outside": 1},
            "standing_text": "partly outside",
            "steps": 3,
            "claims": 2,
            "guarded_claims": 1,
            "unguarded_claims": 1,
            "documents_absent_artifact": False,
            "refuted_claims": 0,
            "narration_permille": 333,
        },
        {
            "id": "local",
            "goal": "check a local contract",
            "standing": {"standing": "checkable_here", "claims": 1},
            "standing_text": "checkable here",
            "steps": 1,
            "claims": 1,
            "guarded_claims": 1,
            "unguarded_claims": 0,
            "documents_absent_artifact": False,
            "refuted_claims": 0,
            "narration_permille": 0,
        },
    ]
    payload = {
        "ok": True,
        "root": "workspace",
        "detail_mode": "full" if include_details else "summary",
        "max_items": 100,
        "devplat": {
            "digest": "d" * 64,
            "verdict_counts": [1, 1, 1, 1],
            "modules_classified": 4,
            "implemented_count": 1,
            "not_implemented_count": 3,
            "foreign_subject_count": 1,
            "walkthrough_count": 3,
            "guarded_claims": 2,
            "unguarded_claims": 3,
        },
        "walkthroughs": walkthroughs,
        "cookbook": {
            "recipes": 2,
            "anti_recipes": 1,
            "crates": ["bioprism-cookbook"],
            "enforcing_tests": 3,
            "quotes": 1,
            "verification": {
                "clean": True,
                "crates_checked": 1,
                "entry_points_checked": 2,
                "tests_checked": 3,
                "quotes_checked": 1,
                "defect_count": 1,
                "defects_returned": [{"kind": "missing_test"}],
                "omitted_defects": 0,
            },
        },
        "developer_contract": {
            "surface_count": 1,
            "surfaces_returned": [
                {
                    "id": "canonical",
                    "owns_count": 1,
                    "invalidates_count": 2,
                    "rationale": "digests depend on canonical bytes",
                }
            ],
            "omitted_surfaces": 0,
        },
        "diagnostic_catalogue": {
            "clean": False,
            "checked": 2,
            "errors": 1,
            "warnings": 0,
            "finding_count": 1,
            "findings_returned": [{"code": "DEVX-0001"}],
            "omitted_findings": 0,
        },
        "exit_code_audit": {
            "clean": False,
            "retry_decision_recoverable_from_code_alone": False,
            "divergence_count": 1,
            "divergences_returned": [{"kind": "class_collision"}],
            "omitted_divergences": 0,
        },
        "limitations": ["foreign SDK and CI remain outside this check"],
    }
    if include_details:
        payload["details"] = {
            "devplat": {
                "digest": "d" * 64,
                "verdict_counts": [1, 1, 1, 1],
                "implemented": ["local"],
                "not_implemented": [["foreign", "foreign artifact"]],
                "foreign_subjects": ["foreign SDK"],
                "walkthroughs": [],
                "guarded_claims": 2,
                "unguarded_claims": 3,
            },
            "cookbook_verification": {},
            "developer_contract": [{"id": "canonical"}],
            "diagnostic_findings": [{"code": "DEVX-0001"}],
            "exit_code_divergences": [{"kind": "class_collision"}],
        }
    return payload


def token_context_plan_payload(*, include_comparison: bool = True) -> dict:
    def plan(
        request_digest: str,
        plan_digest: str,
        candidates: list[str],
        mandatory: list[str],
        handles: list[str],
        mandatory_tokens: int,
        optional_tokens: int,
    ) -> dict:
        return {
            "request_digest": request_digest,
            "plan_digest": plan_digest,
            "candidates": candidates,
            "mandatory": mandatory,
            "handles": handles,
            "mandatory_estimate": {
                "tokens": mandatory_tokens,
                "method": {"method": "declared_by_caller"},
            },
            "optional_estimate": {
                "tokens": optional_tokens,
                "method": {"method": "declared_by_caller"},
            },
            "envelope": {"total": 100},
        }

    baseline = plan(
        "a" * 64,
        "b" * 64,
        ["invariant/identity", "evidence/summary"],
        ["invariant/identity"],
        [],
        20,
        30,
    )
    variant = plan(
        "c" * 64,
        "e" * 64,
        ["invariant/identity", "invariant/uncertainty"],
        ["invariant/identity", "invariant/uncertainty"],
        [],
        35,
        0,
    )
    payload = {
        "ok": True,
        "plan": baseline,
        "comparison": {
            "comparison_id": "mcp-token-policy-comparison",
            "mode": "policy_only",
            "baseline_policy": "policy/minimal",
            "variant_policy": "policy/strict",
            "baseline_plan": baseline,
            "variant_plan": variant,
        }
        if include_comparison
        else None,
        "guarantees": [
            "mandatory closure is checked before a plan is returned",
            "token counts retain their estimation method",
        ],
    }
    return payload


def weavelang_compile_payload(*, status: str = "not_requested", include_ir: bool = False) -> dict:
    execution = {
        "status": status,
        "mode": "replay",
        "state": "start",
        "liveness": {
            "messages_left_unconsumed": 0,
            "commitments_left_open": [],
            "states_without_exit": [],
            "unreachable_states": [],
            "deadlock_freedom_proven": False,
        },
        "invariant_violations": [],
    }
    if status == "completed":
        execution.update({"event_count": 2, "trace_digest": "t" * 64, "trace": {"events": []}})
    if status == "refused":
        execution.update({"error": "replay refused a world mutation", "fail_closed": True})
    return {
        "ok": True,
        "program": {
            "program_id": "urn:weave:program:demo@sha256:" + "p" * 64,
            "digest": "d" * 64,
            "semantic_digest": "s" * 64,
            "weave_ir_version": "0.1.0",
            "roles": 2,
            "participants": 2,
            "interfaces": 1,
            "policies": 1,
            "state_nodes": 3,
            "transitions": 2,
            "monitors": 0,
            "initial_state": "start",
            "terminal_states": ["done"],
        },
        "execution": execution,
        "ir": {"weave_ir_version": "0.1.0"} if include_ir else None,
        "guarantees": [
            "compilation is deterministic and returns both whole-document and semantic digests",
            "replay is the default execution mode and refuses world-mutating transitions",
            "execution is a local semantic trace; it performs no network, model, or tool call",
        ],
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


def adapter_plan_payload() -> dict:
    descriptor = {
        "id": "bioprism.tabular",
        "version": "0.1.0",
        "execution": "native",
        "accepted_formats": ["text/csv"],
        "accepts_undeclared_format": True,
        "source_kinds": ["bytes"],
        "conformance_level": "normalize",
        "declared_loss_kinds": ["precision_reduced"],
        "scope_dimensions": ["subject"],
        "optional_dependency": None,
        "description": "bounded tabular adapter",
    }
    return {
        "ok": True,
        "workflow": "adapter_plan",
        "plan_id": "p" * 64,
        "registry": "bioprism-adapter-registry/0.1",
        "executable": True,
        "selected_adapter": {
            "id": descriptor["id"],
            "execution": descriptor["execution"],
            "version": descriptor["version"],
            "conformance_level": descriptor["conformance_level"],
            "optional_dependency": descriptor["optional_dependency"],
            "declared_loss_kinds": descriptor["declared_loss_kinds"],
            "scope_dimensions": descriptor["scope_dimensions"],
        },
        "plan": {
            "schema": "bioprism-adapter-registry/0.1",
            "request": {
                "source_id": "table-1",
                "source_kind": "bytes",
                "declared_format": "text/csv",
            },
            "selected_adapter": descriptor,
            "executable": True,
            "candidates": [
                {
                    "adapter": descriptor,
                    "status": "ready",
                    "reasons": ["native adapter is available in this runtime"],
                }
            ],
            "limitations": ["source-specific conformance remains required"],
        },
        "execution": "not_started",
        "guarantees": ["format matching is explicit"],
        "limitations": ["does not execute adapters"],
    }


def tabular_ingest_payload() -> dict:
    return {
        "ok": True,
        "source_id": "cohort.csv",
        "fact_count": 2,
        "ingestion_sha256": "sha256:ingestion",
        "manifest": {
            "source_id": "cohort.csv",
            "declared_format": "text/csv",
            "source_digest": "sha256:source",
            "byte_length": 42,
            "adapter": "bioprism.tabular",
            "adapter_version": "0.1.0",
            "profile_digest": "sha256:profile",
            "provenance": {"accession": "RG-DEMO-001", "version": "v1"},
        },
        "semantic_loss": {
            "audit": "lossy",
            "mapped": [{"source_id": "cohort.csv", "column": "subject"}],
            "lost": [
                {
                    "kind": "unmapped_column",
                    "severity": "degrading",
                    "location": {"source_id": "cohort.csv", "column": "comment"},
                    "detail": "comment was not mapped",
                }
            ],
        },
        "conformance": {
            "report": {
                "adapter": "bioprism.tabular",
                "adapter_version": "0.1.0",
                "source_id": "cohort.csv",
                "checks": [
                    {"check": "determinism", "status": "pass", "detail": "three trials matched"},
                    {"check": "loss_completeness", "status": "pass", "detail": "all fields accounted for"},
                    {"check": "fact_integrity", "status": "not_applicable", "detail": "no independent fact verifier"},
                ],
            },
            "passed": True,
            "verified": True,
            "summary": "bioprism.tabular 0.1.0 on cohort.csv: 3 checks, 0 failed",
        },
        "max_items": 1,
        "facts": [{"id": "fact-1", "provides": "age", "value": 41}],
        "omitted_facts": 1,
        "limitations": ["conformance verifies mapping accounting, not source truth"],
    }


def conformance_run_payload() -> dict:
    return {
        "ok": True,
        "suite": {
            "id": "fiber-compiler-conformance",
            "version": "0.1.0",
            "digest": "d" * 64,
            "fixture_manifest_id": "fixture-manifest-1",
            "fixture_count": 2,
            "synthetic_fixture_count": 1,
            "case_count": 4,
            "passed": 3,
            "failed": 1,
            "unsupported": 0,
            "errored": 0,
            "fixture_drift": [],
            "pyramid": {"counts": {"unit": 1, "property": 1, "golden": 1, "conformance": 1, "end_to_end": 0}},
            "fully_conformant": False,
        },
        "release_decision": {
            "decision": "blocked",
            "suite_id": "fiber-compiler-conformance",
            "suite_version": "0.1.0",
            "met": ["no_fixture_drift"],
            "unmet": [{"gate": "required_requirements_pass", "because": "one mandatory requirement failed", "evidence": ["case-2 (typed refusal) is failed"]}],
        },
        "summary": "fiber-compiler-conformance 0.1.0 against reference 0.1.0: 3 passed, 1 failed, 0 unsupported, 0 errored",
        "results": [
            {"case_id": "case-1", "title": "deterministic compile", "layer": "unit", "requirement": "must", "enforces": ["determinism"], "invariant": "same input has same digest", "expectations": ["embedded_digest_verifies"], "outcome": {"outcome": "passed"}},
            {"case_id": "case-2", "title": "typed refusal", "layer": "conformance", "requirement": "must", "enforces": ["typed_failure"], "invariant": "refusal is structured", "expectations": ["fails_with"], "outcome": {"outcome": "failed", "expectation": "fails_with", "detail": "missing error kind"}},
        ],
        "guarantees": ["fixture digests are verified before case results are trusted"],
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

    def test_adapter_plan_report_preserves_route_loss_and_dependency_surface(self) -> None:
        report = AdapterPlanReport.from_wire(adapter_plan_payload())
        self.assertTrue(report.executable)
        self.assertEqual(report.selected_adapter_id, "bioprism.tabular")
        self.assertEqual(report.candidate_count, 1)
        self.assertIsInstance(report.plan.selected_adapter, AdapterDescriptorReport)
        self.assertIsInstance(report.plan.candidates[0], AdapterPlanCandidateReport)
        self.assertEqual(report.plan.candidates[0].adapter.declared_loss_kinds, ("precision_reduced",))
        self.assertFalse(report.plan.dependency_blocked)
        self.assertIsInstance(AdapterPlanProjection.from_wire(adapter_plan_payload()["plan"]), AdapterPlanProjection)

    def test_adapter_plan_report_extracts_http_projection(self) -> None:
        report = adapter_plan_report({"ok": True, "mcp": {"result": {"structuredContent": adapter_plan_payload()}}})
        self.assertEqual(report.plan.request["declared_format"], "text/csv")
        self.assertEqual(report.plan.candidates[0].status, "ready")

    def test_adapter_plan_report_rejects_outer_inner_selection_drift(self) -> None:
        payload = adapter_plan_payload()
        payload["selected_adapter"] = dict(payload["selected_adapter"])
        payload["selected_adapter"]["id"] = "bioprism.other"
        with self.assertRaises(ArgumentError):
            AdapterPlanReport.from_wire(payload)

    def test_tabular_request_and_report_preserve_conformance_loss_and_omissions(self) -> None:
        request = TabularIngestRequest(
            "cohort.csv",
            {"profile_id": "RG-DEMO-001", "columns": {"age": {"type": "integer"}}},
            csv="subject,age,comment\nS1,41,ok\n",
            format="text/csv",
            provenance={"accession": "RG-DEMO-001"},
            include_facts=True,
            max_items=1,
        )
        arguments = request.to_mcp_arguments()
        self.assertEqual(arguments["source_id"], "cohort.csv")
        self.assertNotIn("document", arguments)
        self.assertEqual(arguments["profile"]["profile_id"], "RG-DEMO-001")

        report = TabularIngestReport.from_wire(tabular_ingest_payload())
        self.assertIsInstance(report.manifest, TabularManifestReport)
        self.assertIsInstance(report.semantic_loss, TabularSemanticLossReport)
        self.assertIsInstance(report.conformance, TabularConformanceReport)
        self.assertIsInstance(report.conformance.checks[0], TabularCheckReport)
        self.assertTrue(report.conformance_verified)
        self.assertFalse(report.publishable_candidate)
        self.assertEqual(report.fact_count, len(report.facts) + report.omitted_facts)
        self.assertEqual(report.conformance.failed_checks, ())

    def test_tabular_report_extracts_http_projection_and_preserves_unverified_conformance(self) -> None:
        report = tabular_ingest_report({"ok": True, "mcp": {"result": {"structuredContent": tabular_ingest_payload()}}})
        self.assertEqual(report.manifest.provenance["accession"], "RG-DEMO-001")
        self.assertEqual(report.semantic_loss.lost[0]["kind"], "unmapped_column")
        payload = tabular_ingest_payload()
        payload["conformance"] = dict(payload["conformance"])
        payload["conformance"]["verified"] = False
        self.assertFalse(TabularIngestReport.from_wire(payload).conformance_verified)

    def test_conformance_run_report_preserves_pyramid_cases_and_blocking_gates(self) -> None:
        request = ConformanceRunArgs(include_details=True, max_items=2)
        self.assertEqual(request.to_mcp_arguments(), {"include_details": True, "max_items": 2})
        report = ConformanceRunReport.from_wire(conformance_run_payload())
        self.assertIsInstance(report.suite, ConformanceSuiteReport)
        self.assertIsInstance(report.suite.pyramid, ConformancePyramidReport)
        self.assertIsInstance(report.results[0], ConformanceCaseReport)
        self.assertIsInstance(report.release_decision, ConformanceReleaseDecisionReport)
        self.assertFalse(report.release_ready)
        self.assertEqual(report.release_decision.blocking_gates, ("required_requirements_pass",))
        self.assertEqual(report.suite.pyramid.total, 4)

    def test_conformance_run_report_extracts_http_projection_and_reconciles_decision(self) -> None:
        report = conformance_run_report({"ok": True, "mcp": {"result": {"structuredContent": conformance_run_payload()}}})
        self.assertTrue(report.details_included)
        self.assertEqual(report.summary.split()[0], "fiber-compiler-conformance")
        payload = conformance_run_payload()
        payload["release_decision"] = dict(payload["release_decision"])
        payload["release_decision"]["suite_id"] = "other-suite"
        with self.assertRaises(ArgumentError):
            ConformanceRunReport.from_wire(payload)

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

    def test_developer_platform_status_report_reconciles_nested_contracts(self) -> None:
        report = DeveloperPlatformStatusReport.from_wire(developer_platform_status_payload())
        self.assertTrue(report.platform_checks_clean is False)
        self.assertFalse(report.claims_guarded)
        self.assertTrue(report.foreign_artifacts_present)
        self.assertTrue(report.complete_summary)
        self.assertEqual(report.devplat.modules_classified, 4)
        self.assertEqual(report.walkthroughs[1].standing, "partly_outside")
        self.assertEqual(report.cookbook.verification.defect_count, 1)
        self.assertEqual(report.diagnostic_catalogue.findings_returned[0]["code"], "DEVX-0001")

    def test_developer_platform_status_report_preserves_full_details_and_http_envelope(self) -> None:
        payload = developer_platform_status_payload(include_details=True)
        report = developer_platform_status_report(
            {"ok": True, "mcp": {"result": {"structuredContent": payload}}}
        )
        self.assertTrue(report.details_available)
        self.assertEqual(report.details.devplat["digest"], "d" * 64)
        self.assertEqual(len(report.details.developer_contract), 1)

    def test_developer_platform_status_report_rejects_unreconciled_claims(self) -> None:
        payload = developer_platform_status_payload()
        payload["walkthroughs"][0]["unguarded_claims"] = 1
        with self.assertRaises(ArgumentError):
            DeveloperPlatformStatusReport.from_wire(payload)

    def test_developer_platform_status_args_enforce_protocol_bounds(self) -> None:
        self.assertEqual(
            DeveloperPlatformStatusArgs.from_wire({"include_details": True, "max_items": 7}).to_mcp_arguments(),
            {"include_details": True, "max_items": 7},
        )
        with self.assertRaises(ArgumentError):
            DeveloperPlatformStatusArgs(max_items=0)
        with self.assertRaises(ArgumentError):
            DeveloperPlatformStatusArgs(max_items=1_001)

    def test_token_context_plan_report_preserves_estimates_and_policy_delta(self) -> None:
        report = TokenContextPlanningReport.from_wire(token_context_plan_payload())
        self.assertIsInstance(report.comparison, TokenPolicyComparisonReport)
        self.assertTrue(report.mandatory_closure_affordable)
        self.assertEqual(report.plan.discretionary_tokens, 80)
        self.assertEqual(report.comparison.mandatory_difference, 15)
        self.assertEqual(report.comparison.mandatory_added, ("invariant/uncertainty",))
        self.assertEqual(report.comparison.mandatory_removed, ())
        self.assertFalse(report.estimates_are_measured)

    def test_token_context_plan_report_extracts_http_text_and_rejects_bad_candidates(self) -> None:
        payload = token_context_plan_payload(include_comparison=False)
        report = token_context_plan_report(
            {
                "ok": True,
                "mcp": {
                    "result": {
                        "content": [{"type": "text", "text": json.dumps(payload)}]
                    }
                },
            }
        )
        self.assertFalse(report.has_comparison)
        self.assertEqual(report.plan.mandatory_estimate.method.label, "declared-by-caller")
        with self.assertRaises(ArgumentError):
            TokenContextPlanArgs.from_wire(
                {
                    "request": {
                        "world_ref": "world",
                        "decision_ref": "decision",
                        "role": "researcher",
                        "policy_id": "policy",
                        "envelope": {"total": 10},
                        "depth": "l1",
                        "compiler_version": "compiler/1",
                    },
                    "candidates": [
                        {
                            "node_id": "same",
                            "kind": "evidence",
                            "estimate": {"tokens": 1, "method": {"method": "declared_by_caller"}},
                        },
                        {
                            "node_id": "same",
                            "kind": "summary",
                            "estimate": {"tokens": 1, "method": {"method": "declared_by_caller"}},
                        },
                    ],
                }
            )

    def test_token_context_request_and_candidate_types_round_trip(self) -> None:
        request = TokenContextRequest("world", "decision", "researcher", "policy", 50, "dry_run", "compiler/1")
        estimate = TokenEstimate.from_wire({"tokens": 5, "method": {"method": "provider_tokenizer", "name": "cl100k"}})
        candidate = TokenPlanCandidate("evidence/one", "evidence", estimate, restricted=True)
        args = TokenContextPlanArgs(request, [candidate])
        self.assertEqual(args.to_mcp_arguments()["request"]["depth"], "dry_run")
        self.assertTrue(args.to_mcp_arguments()["candidates"][0]["restricted"])
        self.assertTrue(estimate.method.measured)

    def test_weavelang_compile_report_separates_compilation_and_replay(self) -> None:
        report = WeaveLangCompileReport.from_wire(weavelang_compile_payload())
        self.assertTrue(report.compiled)
        self.assertFalse(report.execution_requested)
        self.assertTrue(report.execution_local_only)
        self.assertTrue(report.replay_defaulted)
        self.assertFalse(report.disclosure_includes_ir)
        self.assertTrue(report.execution.invariant_clean)
        self.assertIsInstance(report.execution.liveness, WeaveLangLivenessReport)
        completed = WeaveLangCompileReport.from_wire(
            weavelang_compile_payload(status="completed", include_ir=True)
        )
        self.assertTrue(completed.execution.completed)
        self.assertEqual(completed.execution.event_count, 2)
        self.assertTrue(completed.disclosure_includes_ir)

    def test_weavelang_compile_report_preserves_fail_closed_replay_refusal_and_http_json_text(self) -> None:
        refused = weavelang_compile_payload(status="refused")
        report = weavelang_compile_report(
            {
                "ok": True,
                "mcp": {
                    "result": {
                        "content": [{"type": "text", "text": json.dumps(refused)}]
                    }
                },
            }
        )
        self.assertTrue(report.execution.refused)
        self.assertTrue(report.execution.fail_closed)
        self.assertTrue(report.execution.replay_safe)
        self.assertIn("replay refused", report.execution.error)

    def test_weavelang_compile_args_enforce_source_mode_and_disclosure_bounds(self) -> None:
        args = WeaveLangCompileArgs("package demo", execute=True, mode="live", thread_id="worker-1", include_ir=True)
        self.assertEqual(args.to_mcp_arguments()["mode"], "live")
        self.assertTrue(args.to_mcp_arguments()["include_ir"])
        with self.assertRaises(ArgumentError):
            WeaveLangCompileArgs("package demo", mode="unknown")
        with self.assertRaises(ArgumentError):
            WeaveLangCompileArgs("package demo", thread_id="")

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
            ci_evidence=None,
            execution_provenance=None,
        )

    def test_delivery_audit_report_preserves_explicit_ci_evidence_target(self) -> None:
        payload = developer_delivery_audit_payload()
        payload["ci_evidence"] = {"ci_evidence_ready": True, "audit": {"verification": "structural_only"}}
        payload["readiness"]["ci_execution_evidence_ready"] = True
        payload["release_request"] = {
            "present": True,
            "id": "delivery-ci-1",
            "targets": [
                {
                    "target": "ci_execution_evidence",
                    "available": True,
                    "eligible": True,
                    "blockers": [],
                    "notes": ["structural CI evidence reconciled"],
                }
            ],
            "ready": True,
            "fail_closed": False,
            "no_implicit_release": True,
            "available_target_count": 11,
        }
        report = DeveloperDeliveryAuditReport.from_wire(payload)
        self.assertTrue(report.readiness.ci_execution_evidence_ready)
        self.assertEqual(report.checks["ci_evidence"]["ci_evidence_ready"], True)
        self.assertTrue(report.ready_for_requested_release)

    def test_delivery_audit_report_preserves_explicit_execution_provenance_target(self) -> None:
        payload = developer_delivery_audit_payload()
        payload["execution_provenance"] = {
            "provenance_ready": True,
            "audit": {"verification": "structural_only"},
        }
        payload["readiness"]["execution_provenance_ready"] = True
        payload["release_request"] = {
            "present": True,
            "id": "delivery-provenance-1",
            "targets": [
                {
                    "target": "execution_provenance",
                    "available": True,
                    "eligible": True,
                    "blockers": [],
                    "notes": ["mission provenance structurally reconciled"],
                }
            ],
            "ready": True,
            "fail_closed": False,
            "no_implicit_release": True,
            "available_target_count": 12,
        }
        report = DeveloperDeliveryAuditReport.from_wire(payload)
        self.assertTrue(report.readiness.execution_provenance_ready)
        self.assertEqual(report.checks["execution_provenance"]["provenance_ready"], True)
        self.assertTrue(report.ready_for_requested_release)

    def test_sync_workspace_typed_developer_platform_report(self) -> None:
        with patch.object(
            Workspace,
            "developer_platform_status",
            return_value=developer_platform_status_payload(),
        ) as status:
            report = Workspace(None).developer_platform_status_report()  # type: ignore[arg-type]
        self.assertIsInstance(report, DeveloperPlatformStatusReport)
        self.assertTrue(report.foreign_artifacts_present)
        status.assert_called_once_with(None, include_details=False, max_items=100)

    def test_sync_workspace_typed_token_context_report(self) -> None:
        request = TokenContextPlanArgs.from_wire(
            {
                "request": {
                    "world_ref": "world",
                    "decision_ref": "decision",
                    "role": "researcher",
                    "policy_id": "policy",
                    "envelope": {"total": 100},
                    "depth": "l1",
                    "compiler_version": "compiler/1",
                },
                "candidates": [
                    {"node_id": "evidence/one", "kind": "evidence", "estimate": {"tokens": 5, "method": {"method": "declared_by_caller"}}}
                ],
            }
        )
        with patch.object(
            Workspace,
            "token_context_plan",
            return_value=token_context_plan_payload(include_comparison=False),
        ) as plan:
            report = Workspace(None).token_context_plan_report(request)  # type: ignore[arg-type]
        self.assertFalse(report.has_comparison)
        plan.assert_called_once_with(request)

    def test_sync_workspace_typed_weavelang_report(self) -> None:
        with patch.object(
            Workspace,
            "weavelang_compile",
            return_value=weavelang_compile_payload(status="completed"),
        ) as compile_tool:
            report = Workspace(None).weavelang_compile_report("package demo")  # type: ignore[arg-type]
        self.assertTrue(report.execution.completed)
        compile_tool.assert_called_once_with("package demo")

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

    def test_sync_workspace_typed_adapter_plan_report_delegates_to_raw_plan(self) -> None:
        with patch.object(Workspace, "adapter_plan", return_value=adapter_plan_payload()) as plan:
            report = Workspace(None).adapter_plan_report(  # type: ignore[arg-type]
                "table-1",
                "bytes",
                declared_format="text/csv",
                available_dependencies=["pandas"],
            )
        self.assertEqual(report.selected_adapter_id, "bioprism.tabular")
        plan.assert_called_once_with(
            "table-1",
            "bytes",
            declared_format="text/csv",
            required_conformance=None,
            available_dependencies=["pandas"],
        )

    def test_sync_workspace_typed_tabular_ingest_report_delegates_to_raw_ingest(self) -> None:
        request = TabularIngestRequest("cohort.csv", {"profile_id": "RG-DEMO-001"}, csv="subject\nS1\n")
        with patch.object(Workspace, "tabular_ingest", return_value=tabular_ingest_payload()) as ingest:
            report = Workspace(None).tabular_ingest_report(request)  # type: ignore[arg-type]
        self.assertEqual(report.source_id, "cohort.csv")
        ingest.assert_called_once_with(request)

    def test_sync_workspace_typed_conformance_report_delegates_to_raw_run(self) -> None:
        with patch.object(Workspace, "conformance_run", return_value=conformance_run_payload()) as run:
            report = Workspace(None).conformance_run_report(include_details=True, max_items=2)  # type: ignore[arg-type]
        self.assertFalse(report.release_ready)
        run.assert_called_once_with(include_details=True, max_items=2)


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
            ci_evidence=None,
            execution_provenance=None,
        )

    async def test_async_workspace_typed_developer_platform_report(self) -> None:
        with patch.object(
            AsyncWorkspace,
            "developer_platform_status",
            new_callable=AsyncMock,
            return_value=developer_platform_status_payload(include_details=True),
        ) as status:
            report = await AsyncWorkspace(None).developer_platform_status_report(
                include_details=True
            )  # type: ignore[arg-type]
        self.assertTrue(report.details_available)
        status.assert_awaited_once_with(None, include_details=True, max_items=100)

    async def test_async_workspace_typed_token_context_report(self) -> None:
        with patch.object(
            AsyncWorkspace,
            "token_context_plan",
            new_callable=AsyncMock,
            return_value=token_context_plan_payload(include_comparison=False),
        ) as plan:
            report = await AsyncWorkspace(None).token_context_plan_report(  # type: ignore[arg-type]
                {"request": {}, "candidates": []}
            )
        self.assertFalse(report.has_comparison)
        plan.assert_awaited_once_with({"request": {}, "candidates": []})

    async def test_async_workspace_typed_weavelang_report(self) -> None:
        with patch.object(
            AsyncWorkspace,
            "weavelang_compile",
            new_callable=AsyncMock,
            return_value=weavelang_compile_payload(status="refused"),
        ) as compile_tool:
            report = await AsyncWorkspace(None).weavelang_compile_report("package demo")  # type: ignore[arg-type]
        self.assertTrue(report.execution.refused)
        compile_tool.assert_awaited_once_with("package demo")

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

    async def test_async_workspace_typed_adapter_plan_report_delegates_to_raw_plan(self) -> None:
        with patch.object(
            AsyncWorkspace,
            "adapter_plan",
            new_callable=AsyncMock,
            return_value=adapter_plan_payload(),
        ) as plan:
            report = await AsyncWorkspace(None).adapter_plan_report(  # type: ignore[arg-type]
                "table-1",
                "bytes",
                declared_format="text/csv",
                available_dependencies=["pandas"],
            )
        self.assertTrue(report.executable)
        plan.assert_awaited_once_with(
            "table-1",
            "bytes",
            declared_format="text/csv",
            required_conformance=None,
            available_dependencies=["pandas"],
        )

    async def test_async_workspace_typed_tabular_ingest_report_delegates_to_raw_ingest(self) -> None:
        request = TabularIngestRequest("cohort.csv", {"profile_id": "RG-DEMO-001"}, csv="subject\nS1\n")
        with patch.object(
            AsyncWorkspace,
            "tabular_ingest",
            new_callable=AsyncMock,
            return_value=tabular_ingest_payload(),
        ) as ingest:
            report = await AsyncWorkspace(None).tabular_ingest_report(request)  # type: ignore[arg-type]
        self.assertEqual(report.omitted_facts, 1)
        ingest.assert_awaited_once_with(request)

    async def test_async_workspace_typed_conformance_report_delegates_to_raw_run(self) -> None:
        with patch.object(
            AsyncWorkspace,
            "conformance_run",
            new_callable=AsyncMock,
            return_value=conformance_run_payload(),
        ) as run:
            report = await AsyncWorkspace(None).conformance_run_report(include_details=True, max_items=2)  # type: ignore[arg-type]
        self.assertEqual(report.release_decision.blocking_gates, ("required_requirements_pass",))
        run.assert_awaited_once_with(include_details=True, max_items=2)


if __name__ == "__main__":
    unittest.main()
