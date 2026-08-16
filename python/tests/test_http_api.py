from __future__ import annotations

import asyncio
import json
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import threading
import unittest
from unittest.mock import AsyncMock, patch

from prism_sdk import AdapterPlanReport, ApiClient, ApiError, ArgumentError, AsyncApiClient, BioAtlasPublicationAuditReport, BioCapabilityEvidenceAuditReport, BioCapabilityEvidenceAuditRequest, BioQlCompileRequest, CapabilityAuditReport, ClaimRequest, ConformanceRunReport, DeliveryPage, DeveloperDeliveryAuditReport, DeveloperPlatformStatusReport, EventPage, EventPersistenceStatus, EvidenceItem, HubLockArgs, HubResolveArgs, HubSearchArgs, InfluenceAnalyzeArgs, LabPlanRequest, MedicalBoundaryRequest, MeasurementCompareArgs, MissionInventoryPage, MissionPersistenceStatus, MissionRequest, MissionStep, MissionWaitTimeout, ObservedWorldDeclareArgs, OperationsCatalogReport, OpsAcceptanceReport, ProviderCapabilityGateArgs, ReleaseAuditArgs, ReleaseAuditReport, ReleaseAuditCheckRequest, RiskAssessmentRequest, RouteReviewEvidence, RoutingDecisionRequest, SdkRegistryCheckArgs, SseSnapshot, StressProfileArgs, StressReportArgs, TabularIngestReport, TabularIngestRequest, TokenContextPlanArgs, TokenContextPlanningReport, WeaveLangCompileArgs, WeaveLangCompileReport, WorldClaimCheckRequest


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
        "selected_adapter": {"id": "bioprism.tabular", "execution": "native", "version": "0.1.0", "conformance_level": "normalize", "optional_dependency": None, "declared_loss_kinds": ["precision_reduced"], "scope_dimensions": ["subject"]},
        "plan": {
            "schema": "bioprism-adapter-registry/0.1",
            "request": {"source_id": "table-1", "source_kind": "bytes", "declared_format": "text/csv"},
            "selected_adapter": descriptor,
            "executable": True,
            "candidates": [{"adapter": descriptor, "status": "ready", "reasons": ["native adapter is available in this runtime"]}],
            "limitations": ["source-specific conformance remains required"],
        },
        "execution": "not_started",
        "guarantees": ["format matching is explicit"],
        "limitations": ["does not execute adapters"],
    }


def developer_platform_status_payload() -> dict:
    return {
        "ok": True,
        "root": "workspace",
        "detail_mode": "summary",
        "max_items": 100,
        "devplat": {
            "digest": "d" * 64,
            "verdict_counts": [1, 1, 1, 1],
            "modules_classified": 4,
            "implemented_count": 1,
            "not_implemented_count": 3,
            "foreign_subject_count": 1,
            "walkthrough_count": 0,
            "guarded_claims": 0,
            "unguarded_claims": 0,
        },
        "walkthroughs": [],
        "cookbook": {"recipes": 0, "anti_recipes": 0, "crates": [], "enforcing_tests": 0, "quotes": 0, "verification": {"clean": True, "crates_checked": 0, "entry_points_checked": 0, "tests_checked": 0, "quotes_checked": 0, "defect_count": 0, "defects_returned": [], "omitted_defects": 0}},
        "developer_contract": {"surface_count": 0, "surfaces_returned": [], "omitted_surfaces": 0},
        "diagnostic_catalogue": {"clean": True, "checked": 0, "errors": 0, "warnings": 0, "finding_count": 0, "findings_returned": [], "omitted_findings": 0},
        "exit_code_audit": {"clean": True, "retry_decision_recoverable_from_code_alone": True, "divergence_count": 0, "divergences_returned": [], "omitted_divergences": 0},
        "limitations": ["foreign artifacts remain explicit"],
    }


def token_context_plan_payload() -> dict:
    plan = {
        "request_digest": "a" * 64,
        "plan_digest": "b" * 64,
        "candidates": ["invariant/identity"],
        "mandatory": ["invariant/identity"],
        "handles": [],
        "mandatory_estimate": {"tokens": 20, "method": {"method": "declared_by_caller"}},
        "optional_estimate": {"tokens": 0, "method": {"method": "declared_by_caller"}},
        "envelope": {"total": 100},
    }
    return {
        "ok": True,
        "plan": plan,
        "comparison": None,
        "guarantees": ["mandatory closure is checked before a plan is returned"],
    }


def weavelang_compile_payload() -> dict:
    return {
        "ok": True,
        "program": {
            "program_id": "urn:weave:program:demo@sha256:" + "p" * 64,
            "digest": "d" * 64,
            "semantic_digest": "s" * 64,
            "weave_ir_version": "0.1.0",
            "roles": 1,
            "participants": 1,
            "interfaces": 0,
            "policies": 1,
            "state_nodes": 1,
            "transitions": 0,
            "monitors": 0,
            "initial_state": "start",
            "terminal_states": ["done"],
        },
        "execution": {
            "status": "not_requested",
            "mode": "replay",
            "state": "start",
            "liveness": {"messages_left_unconsumed": 0, "commitments_left_open": [], "states_without_exit": [], "unreachable_states": [], "deadlock_freedom_proven": False},
            "invariant_violations": [],
        },
        "ir": None,
        "guarantees": ["execution is a local semantic trace; it performs no network, model, or tool call"],
    }


def tabular_ingest_payload() -> dict:
    return {
        "ok": True,
        "source_id": "cohort.csv",
        "fact_count": 1,
        "ingestion_sha256": "sha256:ingestion",
        "manifest": {"source_id": "cohort.csv", "declared_format": "text/csv", "source_digest": "sha256:source", "byte_length": 20, "adapter": "bioprism.tabular", "adapter_version": "0.1.0", "profile_digest": "sha256:profile", "provenance": {"accession": "RG-DEMO-001"}},
        "semantic_loss": {"audit": "lossless", "mapped": [{"source_id": "cohort.csv", "column": "subject"}]},
        "conformance": {"report": {"adapter": "bioprism.tabular", "adapter_version": "0.1.0", "source_id": "cohort.csv", "checks": [{"check": "determinism", "status": "pass", "detail": "stable"}]}, "passed": True, "verified": True, "summary": "verified"},
        "max_items": 100,
        "facts": [{"id": "fact-1", "provides": "subject", "value": "S1"}],
        "omitted_facts": 0,
        "limitations": ["source truth remains caller-owned"],
    }


def conformance_run_payload() -> dict:
    return {
        "ok": True,
        "suite": {"id": "fiber-compiler-conformance", "version": "0.1.0", "digest": "d" * 64, "fixture_manifest_id": "fixture-manifest-1", "fixture_count": 1, "synthetic_fixture_count": 0, "case_count": 1, "passed": 1, "failed": 0, "unsupported": 0, "errored": 0, "fixture_drift": [], "pyramid": {"counts": {"unit": 1}}, "fully_conformant": True},
        "release_decision": {"decision": "release", "suite_id": "fiber-compiler-conformance", "suite_version": "0.1.0", "suite_digest": "d" * 64, "implementation": "reference 0.1.0", "gates": ["no_fixture_drift"]},
        "summary": "fiber-compiler-conformance 0.1.0 against reference 0.1.0: 1 passed, 0 failed, 0 unsupported, 0 errored",
        "results": None,
        "guarantees": ["fixture digests are verified"],
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
            "foreign_subject_count": 0,
            "foreign_artifacts_present": False,
            "foreign_artifacts_are_not_inferred": True,
            "local_integration_foundations": [],
            "unverified_surface_families": [],
        },
        "release_request": {
            "present": True,
            "id": "delivery-1",
            "targets": [{"target": "local_delivery", "available": True, "eligible": True, "blockers": [], "notes": []}],
            "ready": True,
            "fail_closed": False,
            "no_implicit_release": True,
            "available_target_count": 10,
        },
        "guarantees": [],
        "limitations": [],
    }


def biocapability_evidence_audit_payload() -> dict:
    return {
        "ok": True,
        "workflow": "biocapability_evidence_conditioned_profile",
        "metrics": {"ok": True},
        "metrics_ok": True,
        "evidence": {
            "items": [],
            "omitted_items": 0,
            "item_count": 0,
            "invalid_item_count": 0,
            "dimensions": [],
            "domains": {},
        },
        "claim_requests": {
            "rows": [],
            "omitted_rows": 0,
            "requested": 0,
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
            "requires_explicit_claim_request": True,
            "numeric_scores_are_not_claims_without_evidence": True,
            "declared_evidence_is_visible_but_not_measured_support": True,
        },
        "guarantees": [],
        "limitations": [],
    }


def bioatlas_publication_audit_payload() -> dict:
    return {
        "ok": True,
        "workflow": "bioatlas_publication_audit",
        "atlas": {"ok": True},
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
            "leaderboard_ranked_count": 1,
            "leaderboard_unranked_count": 0,
            "unranked_leaderboard_entries_remain_visible": True,
            "withheld_scores_are_not_zeroes": True,
        },
        "guarantees": [],
        "limitations": [],
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
        "groups": [],
    }


class FakeApiHandler(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def log_message(self, _format: str, *_args: object) -> None:
        return

    def _send(self, status: int, value: dict) -> None:
        body = json.dumps(value, separators=(",", ":")).encode("utf-8")
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self) -> None:  # noqa: N802
        if self.path == "/healthz":
            self._send(200, {"ok": True, "ready": True})
        elif self.path == "/v1/tools":
            self._send(
                200,
                {
                    "tools": [
                        {
                            "name": "echo",
                            "inputSchema": {
                                "type": "object",
                                "properties": {"value": {"type": "integer"}},
                            },
                        }
                    ]
                },
            )
        elif self.path.startswith("/v1/route-reviews/"):
            review_id = "a" * 64
            self._send(200, {"ok": True, "workflow": "capability_route_review_evidence", "review_id": review_id, "found": True, "page": {"events": [{"id": 1, "event_type": "tool.completed", "subject": "capability_route_review", "request_id": "req-1", "payload": {}}], "after": 0, "next_after": 1, "oldest": 1, "newest": 1, "gap": False, "dropped_events": 0}})
        elif self.path.startswith("/v1/events/stream"):
            body = b'id: 1\nevent: mission.trace\ndata: {"mission_id":"async-1"}\n\n'
            self.send_response(200)
            self.send_header("Content-Type", "text/event-stream; charset=utf-8")
            self.send_header("X-Next-After", "1")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
        elif self.path == "/v1/events/persistence":
            self._send(200, {"ok": True, "enabled": True, "file_present": True, "file_bytes": 128, "schema_version": 1, "max_file_bytes": 64 * 1024 * 1024, "retained_events": 2, "next_event_id": 3, "dropped_events": 0, "subscriptions_durable": False, "webhook_deliveries_durable": False, "recovery_policy": "events restore with cursor continuity; subscriptions and deliveries must be re-established", "flush": "/v1/events/persistence/flush"})
        elif self.path.startswith("/v1/events"):
            self._send(200, {"ok": True, "page": {"events": [], "after": 0, "next_after": 0, "oldest": None, "newest": None, "gap": False, "dropped_events": 0}})
        elif self.path.startswith("/v1/missions?"):
            self._send(200, {"ok": True, "missions": [{"mission_id": "async-1", "status": "succeeded", "cancel_requested": False, "progress": {"phase": "succeeded", "current_wave": 0, "total_steps": 1, "completed_steps": 1, "active_steps": 0, "succeeded": 1, "refused": 0, "blocked": 0, "cancelled": 0, "required_failures": 0, "returned_bytes": 14, "trace_sequence": 4, "last_event": "mission.completed"}, "summary": {"total_steps": 1, "completed_steps": 1, "succeeded": 1, "refused": 0, "blocked": 0, "cancelled": 0, "required_failures": 0, "returned_bytes": 14, "result_available": True}, "poll": "/v1/missions/async-1", "cancel": "/v1/missions/async-1/cancel", "trace": "/v1/missions/async-1/trace"}], "returned": 1, "total_matching": 1, "limit": 5, "truncated": False, "status_filter": "succeeded"})
        elif self.path == "/v1/missions/persistence":
            self._send(200, {"ok": True, "enabled": True, "file_present": True, "file_bytes": 128, "schema_version": 1, "max_file_bytes": 64 * 1024 * 1024, "max_result_bytes": 256 * 1024, "registry_size": 1, "event_log_durable": False, "webhook_deliveries_durable": False, "recovery_policy": "terminal snapshots restore; queued and running jobs fail explicitly after restart", "flush": "/v1/missions/persistence/flush"})
        elif self.path.startswith("/v1/missions/async-1/trace"):
            self._send(200, {"ok": True, "mission_id": "async-1", "trace_schema_version": "bioprism-devplat-mission-trace/0.1", "events": [{"sequence": 0, "event": "mission.started", "wave": None, "step_id": None, "tool": None, "status": "running", "arguments_digest": None, "bytes": 0, "detail": None}, {"sequence": 1, "event": "mission.completed", "wave": None, "step_id": None, "tool": None, "status": "succeeded", "arguments_digest": None, "bytes": 14, "detail": None}], "after": 0, "next_after": 2, "oldest": 0, "newest": 1, "gap": False, "dropped_events": 0, "terminal": True, "limit": 100, "truncated": False})
        elif self.path == "/v1/missions/async-1":
            self._send(200, {"ok": True, "mission_id": "async-1", "status": "succeeded", "cancel_requested": False, "progress": {"phase": "succeeded", "current_wave": 0, "total_steps": 1, "completed_steps": 1, "active_steps": 0, "succeeded": 1, "refused": 0, "blocked": 0, "cancelled": 0, "required_failures": 0, "returned_bytes": 14, "trace_sequence": 4, "last_event": "mission.completed"}, "result": {"mission_status": "succeeded"}})
        elif self.path == "/v1/missions/slow":
            self._send(200, {"ok": True, "mission_id": "slow", "status": "running", "cancel_requested": False, "progress": {"phase": "running", "current_wave": 0, "total_steps": 1, "completed_steps": 0, "active_steps": 1, "succeeded": 0, "refused": 0, "blocked": 0, "cancelled": 0, "required_failures": 0, "returned_bytes": 0, "trace_sequence": 1, "last_event": "step.started"}})
        else:
            self._send(404, {"ok": False, "error": {"code": "not_found"}})

    def do_POST(self) -> None:  # noqa: N802
        size = int(self.headers.get("Content-Length", "0"))
        body = json.loads(self.rfile.read(size) or b"{}")
        if self.path == "/v1/missions/preflight":
            self._send(200, {"ok": True, "workflow": "agent_mission", "execution": "planned", "mission_status": "planned", "preflight": True, "dispatch": "not_started", "results": []})
        elif self.path == "/v1/missions":
            self._send(202, {"ok": True, "mission_id": "async-1", "status": "queued", "cancel_requested": False})
        elif self.path == "/v1/missions/async-1/cancel":
            self._send(202, {"ok": True, "mission_id": "async-1", "status": "running", "cancel_requested": True, "cancel_reason": body.get("reason")})
        elif self.path in {"/v1/missions/persistence/flush", "/v1/events/persistence/flush"}:
            self._send(200, {"ok": True, "enabled": True, "file_present": True, "file_bytes": 128, "schema_version": 1, "max_file_bytes": 64 * 1024 * 1024, "max_result_bytes": 256 * 1024, "registry_size": 1, "retained_events": 2, "next_event_id": 3, "dropped_events": 0, "event_log_durable": False, "subscriptions_durable": False, "webhook_deliveries_durable": False, "recovery_policy": "events restore with cursor continuity; subscriptions and deliveries must be re-established", "flush": self.path})
        elif self.path == "/v1/tools/echo":
            self._send(200, {"ok": True, "tool": "echo", "mcp": {"result": body}})
        elif self.path.startswith("/v1/tools/capability_") or self.path in {"/v1/tools/developer_platform_status", "/v1/tools/token_context_plan", "/v1/tools/weavelang_compile", "/v1/tools/developer_delivery_audit", "/v1/tools/biocapability_evidence_audit", "/v1/tools/bioatlas_publication_audit", "/v1/tools/bioql_compile", "/v1/tools/world_claim_check", "/v1/tools/observed_world_declare", "/v1/tools/lineage_audit", "/v1/tools/preanalytic_apply", "/v1/tools/contradiction_review", "/v1/tools/onco_boundary_check", "/v1/tools/onco_response_assess", "/v1/tools/onco_worldline_view", "/v1/tools/onco_classification_check", "/v1/tools/oncoworlds_identity_join", "/v1/tools/oncoworlds_model_transport", "/v1/tools/oncoworlds_methylation_classify", "/v1/tools/oncoworlds_methylation_compare", "/v1/tools/oncoworlds_radiogenomic_check", "/v1/tools/oncoworlds_clonal_history_check", "/v1/tools/oncoworlds_clonal_evidence_check", "/v1/tools/oncoworlds_era_shift_check", "/v1/tools/oncoworlds_equity_check", "/v1/tools/oncoworlds_entity_world_check", "/v1/tools/literature_bind_check", "/v1/tools/modality_support_check", "/v1/tools/modality_transport_check", "/v1/tools/modality_comparability_check", "/v1/tools/obligation_gate_check", "/v1/tools/onco_outcome_analyze", "/v1/tools/oracle_combine", "/v1/tools/oracle_reference_panel", "/v1/tools/oracle_missingness", "/v1/tools/bioeval_reference_audit", "/v1/tools/evaluation_worldline_audit", "/v1/tools/evaluation_reproduction_check", "/v1/tools/evaluation_trajectory_check", "/v1/tools/runtime_effect_check", "/v1/tools/runtime_tape_verify", "/v1/tools/runtime_execution_simulate", "/v1/tools/bioethics_action_review", "/v1/tools/bioethics_human_subject_screen", "/v1/tools/bioethics_dual_use_review", "/v1/tools/bioethics_validation_check", "/v1/tools/bioethics_representation_audit", "/v1/tools/stress_profile", "/v1/tools/stress_report", "/v1/tools/influence_analyze", "/v1/tools/lab_plan", "/v1/tools/routing_decide", "/v1/tools/provider_capability_gate", "/v1/tools/sdk_registry_check", "/v1/tools/fiber_compile", "/v1/tools/fiber_refine", "/v1/tools/fiber_explain", "/v1/tools/fiber_verify", "/v1/tools/projection_bundle", "/v1/tools/repository_catalog", "/v1/tools/repository_bundle", "/v1/tools/repository_impact", "/v1/tools/telemetry_project", "/v1/tools/tabular_ingest", "/v1/tools/conformance_run", "/v1/tools/release_audit", "/v1/tools/operations_catalog", "/v1/tools/ops_acceptance", "/v1/tools/safety_release_gate", "/v1/tools/medical_boundary_check", "/v1/tools/safety_posture", "/v1/tools/measurement_compare", "/v1/tools/hub_search", "/v1/tools/hub_resolve", "/v1/tools/hub_lock"}:
            self._send(200, {"ok": True, "tool": self.path.rsplit("/", 1)[-1], "mcp": {"result": body}})
        elif self.path == "/v1/tools/adapter_plan":
            self._send(200, {"ok": True, "tool": "adapter_plan", "mcp": {"result": body}})
        elif self.path.endswith("/replay"):
            self._send(200, {"ok": True, "replayed": [{"delivery_id": 1, "subscription_id": "sub", "attempt": 1, "state": "pending", "last_error": None, "last_error_retryable": None, "event_id": 2, "event_type": "tool.completed", "signature": "sha256=x", "envelope": {}}]})
        else:
            self._send(422, {"ok": False, "error": {"code": "refused"}})


class HttpApiClientTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.server = ThreadingHTTPServer(("127.0.0.1", 0), FakeApiHandler)
        cls.thread = threading.Thread(target=cls.server.serve_forever, daemon=True)
        cls.thread.start()
        host, port = cls.server.server_address
        cls.base_url = f"http://{host}:{port}"

    @classmethod
    def tearDownClass(cls) -> None:
        cls.server.shutdown()
        cls.server.server_close()
        cls.thread.join(timeout=2)

    def test_delivery_failure_state_is_typed_and_replay_is_explicit(self) -> None:
        page = DeliveryPage.from_wire({
            "ok": True,
            "page": {
                "deliveries": [{
                    "delivery_id": 1,
                    "subscription_id": "sub",
                    "attempt": 1,
                    "state": "failed",
                    "last_error": "blocked",
                    "last_error_retryable": False,
                    "event_id": 2,
                    "event_type": "tool.completed",
                    "signature": "sha256=x",
                    "envelope": {},
                }],
                "after": 0,
                "next_after": 1,
                "pending_count": 1,
                "dropped_deliveries": 0,
            },
        })
        self.assertEqual(page.deliveries[0].state, "failed")
        self.assertEqual(page.deliveries[0].last_error, "blocked")
        self.assertIs(page.deliveries[0].last_error_retryable, False)
        replayed = ApiClient(self.base_url).replay("sub", [1])
        self.assertEqual(replayed["replayed"][0]["state"], "pending")

    def test_http_health_tools_events_and_structured_errors(self) -> None:
        client = ApiClient(self.base_url, bearer_token="0123456789abcdef")
        self.assertTrue(client.health()["ready"])
        self.assertEqual(client.tools()[0]["name"], "echo")
        catalogue = client.tool_catalogue()
        self.assertEqual(client.plan_tool("echo", {"value": 3}, catalogue=catalogue).tool, "echo")
        self.assertEqual(client.tool_checked("echo", {"value": 3}, catalogue=catalogue)["mcp"]["result"]["value"], 3)
        with self.assertRaises(ArgumentError):
            client.plan_tool("echo", {"value": "not-an-integer"}, catalogue=catalogue)
        mission = client.mission_preflight(
            MissionRequest(
                "mission-http",
                "check",
                [MissionStep("one", "data", "read", "check", "echo", {"value": 3})],
            ),
            catalogue=catalogue,
        )
        self.assertTrue(mission.ok)
        remote_preflight = client.preflight_mission(
            MissionRequest(
                "mission-http-remote-preflight",
                "check",
                [MissionStep("one", "data", "read", "check", "echo", {"value": 3})],
            )
        )
        self.assertTrue(remote_preflight["preflight"])
        self.assertEqual(remote_preflight["dispatch"], "not_started")
        self.assertEqual(client.call_tool("echo", {"value": 3})["mcp"]["result"]["value"], 3)
        submitted = client.submit_mission(MissionRequest("async-1", "run", [MissionStep("one", "data", "read", "run", "echo", {"value": 1})]))
        self.assertEqual(submitted.status, "queued")
        status = client.mission_status("async-1")
        self.assertEqual(status.result["mission_status"], "succeeded")
        self.assertIsNotNone(status.progress)
        self.assertEqual(status.progress.phase, "succeeded")
        self.assertEqual(status.progress.completed_steps, 1)
        self.assertEqual(status.progress.last_event, "mission.completed")
        waited = client.wait_mission("async-1", timeout=1.0, poll_interval=0.01)
        self.assertEqual(waited.status, "succeeded")
        trace = client.mission_trace("async-1")
        self.assertEqual(trace.events[0].event, "mission.started")
        self.assertEqual(trace.events[-1].event, "mission.completed")
        self.assertEqual(trace.next_after, 2)
        with self.assertRaises(ArgumentError):
            client.mission_trace("async-1", after=-1)
        inventory = client.missions(status="succeeded", limit=5)
        self.assertEqual(inventory["missions"][0]["mission_id"], "async-1")
        typed_inventory = client.mission_inventory(status="succeeded", limit=5)
        self.assertIsInstance(typed_inventory, MissionInventoryPage)
        self.assertTrue(typed_inventory.missions[0].terminal)
        self.assertEqual(typed_inventory.missions[0].progress.completed_steps, 1)
        self.assertIsInstance(client.mission_persistence(), MissionPersistenceStatus)
        self.assertIsInstance(client.flush_mission_persistence(), MissionPersistenceStatus)
        with self.assertRaises(ArgumentError):
            client.wait_mission("async-1", timeout=0)
        with self.assertRaises(MissionWaitTimeout) as wait_error:
            client.wait_mission("slow", timeout=0.01, poll_interval=0.01)
        self.assertEqual(wait_error.exception.last_job.status, "running")
        self.assertTrue(client.cancel_mission("async-1", "operator stop").cancel_requested)
        self.assertEqual(
            client.capability_discover(query="oncology")["mcp"]["result"]["query"],
            "oncology",
        )
        self.assertEqual(
            client.capability_audit(include_groups=False)["mcp"]["result"]["include_groups"],
            False,
        )
        self.assertEqual(
            client.developer_delivery_audit(
                request_id="delivery-1", targets=["local_delivery"]
            )["mcp"]["result"]["release_request"]["id"],
            "delivery-1",
        )
        self.assertEqual(
            client.capability_route("compose evidence", [{"id": "oncology", "query": "oncology"}])["mcp"]["result"]["goal"],
            "compose evidence",
        )
        self.assertEqual(
            client.adapter_plan("scan-1", "bytes", declared_format="application/dicom")["mcp"]["result"]["source_id"],
            "scan-1",
        )
        tabular_request = TabularIngestRequest("cohort.csv", {"profile_id": "RG-DEMO-001"}, csv="subject\nS1\n")
        self.assertEqual(
            client.tabular_ingest(tabular_request)["mcp"]["result"]["source_id"],
            "cohort.csv",
        )
        self.assertEqual(
            client.conformance_run(include_details=True, max_items=2)["mcp"]["result"]["max_items"],
            2,
        )
        release_request = ReleaseAuditArgs([ReleaseAuditCheckRequest("conformance_run", {})], include_details=True)
        self.assertEqual(
            client.release_audit(release_request)["mcp"]["result"]["checks"][0]["kind"],
            "conformance_run",
        )
        self.assertEqual(
            client.operations_catalog(max_items=2)["mcp"]["result"]["max_items"],
            2,
        )
        self.assertEqual(
            client.ops_acceptance(max_items=3)["mcp"]["result"]["max_items"],
            3,
        )
        safety_request = RiskAssessmentRequest("subject", {"capability_uplift": "low"})
        self.assertEqual(
            client.safety_release_gate(safety_request)["mcp"]["result"]["assessment"]["subject"],
            "subject",
        )
        medical_request = MedicalBoundaryRequest({"side": "research", "use_case": "provenance", "label": "trace"})
        self.assertEqual(
            client.medical_boundary_check(medical_request)["mcp"]["result"]["output"]["use_case"],
            "provenance",
        )
        self.assertTrue(
            client.safety_posture(include_threats=True)["mcp"]["result"]["include_threats"]
        )
        measurement = MeasurementCompareArgs({"label": "left"}, {"label": "right"}, require_bound_terms=True)
        self.assertTrue(
            client.measurement_compare(measurement)["mcp"]["result"]["require_bound_terms"]
        )
        hub_request = HubSearchArgs({"members": {}}, [{"releases": {}}], {"facets": []}, max_items=3)
        self.assertEqual(
            client.hub_search(hub_request)["mcp"]["result"]["max_items"],
            3,
        )
        hub_resolve_request = HubResolveArgs({}, [], {"name": "bioprism/root"})
        self.assertEqual(
            client.hub_resolve(hub_resolve_request)["mcp"]["result"]["request"]["name"],
            "bioprism/root",
        )
        hub_lock_request = HubLockArgs({}, [], {"name": "bioprism/root"}, max_items=2)
        self.assertEqual(
            client.hub_lock(hub_lock_request)["mcp"]["result"]["max_items"],
            2,
        )
        self.assertEqual(
            client.lineage_audit({"registry": {"nodes": {}, "artifacts": {}}, "max_items": 2})["mcp"]["result"]["max_items"],
            2,
        )
        self.assertEqual(
            client.preanalytic_apply({"specimen": {"id": "sp-1"}, "mutation": {"id": "m-1"}})["mcp"]["result"]["mutation"]["id"],
            "m-1",
        )
        self.assertEqual(
            client.contradiction_review({"left": {}, "right": {}, "intent": "resolvable", "hypotheses": [{"id": "h-1", "account": {}}]})["mcp"]["result"]["intent"],
            "resolvable",
        )
        self.assertEqual(
            client.onco_boundary_check({"request": {"requested_uses": ["cohort_analysis"]}})["mcp"]["result"]["request"]["requested_uses"],
            ["cohort_analysis"],
        )
        self.assertEqual(
            client.onco_response_assess({"criterion": {}, "baseline": {}, "current": {}, "current_acquired": "2026-01-01T00:00:00Z", "baseline_clinical": {}, "current_clinical": {}, "treatment": {}})["mcp"]["result"]["current_acquired"],
            "2026-01-01T00:00:00Z",
        )
        self.assertEqual(
            client.onco_worldline_view({"worldline": {}, "visible_at": "2026-01-02T00:00:00Z"})["mcp"]["result"]["visible_at"],
            "2026-01-02T00:00:00Z",
        )
        self.assertEqual(
            client.onco_classification_check({"histology": "diffuse_glioma", "panel": {}})["mcp"]["result"]["histology"],
            "diffuse_glioma",
        )
        self.assertEqual(
            client.oncoworlds_identity_join({"left": {}, "right": {}, "unit": "specimen"})["mcp"]["result"]["unit"],
            "specimen",
        )
        self.assertEqual(
            client.onco_outcome_analyze({"follow_up": {}, "estimand": {}})["mcp"]["result"]["estimand"],
            {},
        )
        evidence_request = BioCapabilityEvidenceAuditRequest(
            [EvidenceItem("grounding", "evidence_grounding", "observed", support={"source": "ledger", "scope": "pack/1"})],
            [ClaimRequest("claim", "grounded profile", ("evidence_grounding",))],
            vectors=({"system": "a"}, {"system": "b"}),
        )
        self.assertEqual(
            client.biocapability_evidence_audit(evidence_request)["mcp"]["result"]["claim_requests"][0]["id"],
            "claim",
        )
        self.assertEqual(
            client.bioql_compile(BioQlCompileRequest("SELECT sample.id", {"schema_version": "v1"}))["mcp"]["result"]["query"],
            "SELECT sample.id",
        )
        self.assertEqual(
            client.world_claim_check(WorldClaimCheckRequest({"top": "observed"}, {"kind": "biology"}))["mcp"]["result"]["provenance"]["top"],
            "observed",
        )
        observed_request = ObservedWorldDeclareArgs("observed-demo", [], {"cohort_size": 0}, [])
        self.assertEqual(
            client.observed_world_declare(observed_request)["mcp"]["result"]["id"],
            "observed-demo",
        )
        self.assertEqual(
            client.lab_plan(LabPlanRequest({"obligations": []}, [{"id": "assay"}], {"tokens": 1}))["mcp"]["result"]["actions"][0]["id"],
            "assay",
        )
        self.assertEqual(
            client.oracle_combine("subject", "2026-01-01T00:00:00Z", [{}])["mcp"]["result"]["subject"],
            "subject",
        )
        self.assertEqual(
            client.oracle_reference_panel({"reads": []})["mcp"]["result"]["panel"]["reads"],
            [],
        )
        self.assertEqual(
            client.oracle_missingness({}, {}, {}, 5)["mcp"]["result"]["small_cell_floor"],
            5,
        )
        self.assertEqual(
            client.bioeval_reference_audit({"standard": "unresolved"})["mcp"]["result"]["reference"]["standard"],
            "unresolved",
        )
        self.assertEqual(
            client.evaluation_worldline_audit({"decisions": []})["mcp"]["result"]["worldline"]["decisions"],
            [],
        )
        self.assertEqual(
            client.evaluation_reproduction_check({"specs": []})["mcp"]["result"]["reexecution"]["specs"],
            [],
        )
        self.assertEqual(
            client.evaluation_trajectory_check({"steps": []})["mcp"]["result"]["trajectory"]["steps"],
            [],
        )
        self.assertEqual(
            client.runtime_effect_check({"policy": {}, "request": {"kind": "clock_now"}})["mcp"]["result"]["request"]["kind"],
            "clock_now",
        )
        self.assertEqual(
            client.runtime_tape_verify({"tape": {}})["mcp"]["result"]["tape"],
            {},
        )
        self.assertEqual(
            client.runtime_execution_simulate({"policy": {}, "requests": []})["mcp"]["result"]["requests"],
            [],
        )
        self.assertEqual(
            client.bioethics_action_review({"plan": {}})["mcp"]["result"]["plan"],
            {},
        )
        self.assertEqual(
            client.human_subject_screen({"study": {}})["mcp"]["result"]["study"],
            {},
        )
        self.assertEqual(
            client.bioethics_dual_use_review({"release": {}, "risk": {}})["mcp"]["result"]["risk"],
            {},
        )
        self.assertEqual(
            client.bioethics_validation_check({"dossier": {}})["mcp"]["result"]["dossier"],
            {},
        )
        self.assertEqual(
            client.bioethics_representation_audit({"subject": "study", "observations": []})["mcp"]["result"]["observations"],
            [],
        )
        self.assertEqual(
            client.oncoworlds_model_transport({"result": {}, "establishment": {}, "claimed_n": 1, "transport": {}})["mcp"]["result"]["claimed_n"],
            1,
        )
        self.assertEqual(
            client.oncoworlds_methylation_classify({"classifier": {}, "scores": {}, "context": {}})["mcp"]["result"]["scores"],
            {},
        )
        self.assertEqual(
            client.oncoworlds_methylation_compare({"left": {}, "right": {}})["mcp"]["result"]["right"],
            {},
        )
        self.assertEqual(
            client.oncoworlds_radiogenomic_check({"claim": {}, "design": {}, "observation": {}, "transport": {}})["mcp"]["result"]["claim"],
            {},
        )
        self.assertEqual(
            client.oncoworlds_clonal_history_check({"population": {}, "candidates": []})["mcp"]["result"]["candidates"],
            [],
        )
        self.assertEqual(
            client.oncoworlds_clonal_evidence_check({"promotion": {}})["mcp"]["result"]["promotion"],
            {},
        )
        self.assertEqual(
            client.oncoworlds_era_shift_check({"left": {}, "right": {}})["mcp"]["result"]["left"],
            {},
        )
        self.assertEqual(
            client.oncoworlds_equity_check({"pooled": {}})["mcp"]["result"]["pooled"],
            {},
        )
        self.assertEqual(
            client.oncoworlds_entity_world_check({"provenance": {}})["mcp"]["result"]["provenance"],
            {},
        )
        self.assertEqual(
            client.literature_bind_check({"claim": {}, "target": {}, "at_tier": "review", "horizon": {}})["mcp"]["result"]["claim"],
            {},
        )
        self.assertEqual(
            client.modality_support_check({"modality": "single_cell", "claim": "cell_composition"})["mcp"]["result"]["claim"],
            "cell_composition",
        )
        self.assertEqual(
            client.modality_transport_check({"from": "single_cell", "to": "bulk_transcriptomics", "axis": "cell", "transport": {"kind": "aggregation", "operator": "mean"}})["mcp"]["result"]["from"],
            "single_cell",
        )
        self.assertEqual(
            client.obligation_gate_check({"graph": {}, "action": {}})["mcp"]["result"]["action"],
            {},
        )
        self.assertEqual(
            client.stress_profile(StressProfileArgs({"id": "cohort"}, {"id": "stress"}))["mcp"]["result"]["stress"]["id"],
            "stress",
        )
        self.assertEqual(
            client.stress_report(StressReportArgs({"id": "cohort"}, ({"id": "stress"},)))["mcp"]["result"]["stresses"][0]["id"],
            "stress",
        )
        self.assertEqual(
            client.influence_analyze(InfluenceAnalyzeArgs("region", {"a": 2}, ({"id": "f.a", "scope": ["a"]},), ("a",), {"class": "removal"}, factor="f.a"))["mcp"]["result"]["label"],
            "region",
        )
        self.assertEqual(
            client.routing_decide(RoutingDecisionRequest({"features": {}}, [{"task_id": "other"}], {"safe_default": "abstain"}))["mcp"]["result"]["policy"]["safe_default"],
            "abstain",
        )
        self.assertEqual(
            client.provider_capability_gate(ProviderCapabilityGateArgs({"provider": "runtime-a", "states": {}, "measurements": []}, ("host_escape",)))["mcp"]["result"]["card"]["provider"],
            "runtime-a",
        )
        self.assertEqual(
            client.sdk_registry_check(SdkRegistryCheckArgs(({"id": "plugin"},)))["mcp"]["result"]["manifests"][0]["id"],
            "plugin",
        )
        self.assertEqual(
            client.fiber_compile("world.json", "query.json", layer="l1")["mcp"]["result"]["layer"],
            "l1",
        )
        self.assertEqual(
            client.fiber_refine("l2", handle={"digest": "compiled"})["mcp"]["result"]["handle"]["digest"],
            "compiled",
        )
        self.assertEqual(
            client.fiber_explain("world.json", "query.json")["mcp"]["result"]["query"],
            "query.json",
        )
        self.assertEqual(
            client.fiber_verify("certificate.json")["mcp"]["result"]["certificate"],
            "certificate.json",
        )
        self.assertTrue(
            client.projection_bundle("world.json", "query.json", include_views=True)["mcp"]["result"]["include_views"]
        )
        self.assertEqual(
            client.repository_catalog(prefix="docs/", limit=3)["mcp"]["result"]["prefix"],
            "docs/",
        )
        self.assertEqual(
            client.repository_bundle({"id": "route-1"}, policy="exhaustive")["mcp"]["result"]["policy"],
            "exhaustive",
        )
        self.assertEqual(
            client.repository_impact("docs/README")["mcp"]["result"]["changed"],
            "docs/README",
        )
        self.assertEqual(
            client.telemetry_project({"kind": "event"}, {"treatments": {}}, "trace-http")["mcp"]["result"]["trace"],
            "trace-http",
        )
        self.assertEqual(client.events()["page"]["events"], [])
        event_page = client.event_page()
        self.assertIsInstance(event_page, EventPage)
        self.assertFalse(event_page.gap)
        stream = client.event_stream()
        self.assertIsInstance(stream, SseSnapshot)
        self.assertEqual(stream.next_after, 1)
        self.assertEqual(stream.events[0].event, "mission.trace")
        evidence = client.route_review_evidence("a" * 64)
        self.assertIsInstance(evidence, RouteReviewEvidence)
        self.assertTrue(evidence.found)
        self.assertEqual(evidence.page.events[0].subject, "capability_route_review")
        with self.assertRaises(ArgumentError):
            client.route_review_evidence("invalid")
        self.assertIsInstance(client.event_persistence(), EventPersistenceStatus)
        self.assertIsInstance(client.flush_event_persistence(), EventPersistenceStatus)
        with self.assertRaises(ArgumentError):
            client.event_page(after=True)
        with self.assertRaises(ApiError) as error:
            client.request("POST", "/v1/tools/refuse", {})
        self.assertEqual(error.exception.status, 422)

    def test_http_typed_delivery_audit_report_delegates_to_raw_helper(self) -> None:
        with patch.object(
            ApiClient,
            "developer_delivery_audit",
            return_value=developer_delivery_audit_payload(),
        ) as audit:
            report = ApiClient(self.base_url).developer_delivery_audit_report(
                request_id="delivery-1", targets=["local_delivery"]
            )
        self.assertIsInstance(report, DeveloperDeliveryAuditReport)
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

    def test_http_developer_platform_status_round_trips_bounded_arguments(self) -> None:
        result = ApiClient(self.base_url).developer_platform_status(
            include_details=True, max_items=7
        )
        self.assertTrue(result["mcp"]["result"]["include_details"])
        self.assertEqual(result["mcp"]["result"]["max_items"], 7)

    def test_http_typed_developer_platform_status_report_delegates_to_raw_helper(self) -> None:
        with patch.object(
            ApiClient,
            "developer_platform_status",
            return_value=developer_platform_status_payload(),
        ) as status:
            report = ApiClient(self.base_url).developer_platform_status_report(max_items=7)
        self.assertIsInstance(report, DeveloperPlatformStatusReport)
        self.assertEqual(report.devplat.modules_classified, 4)
        status.assert_called_once_with(include_details=False, max_items=7)

    def test_http_token_context_plan_round_trips_typed_arguments(self) -> None:
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
                    {
                        "node_id": "invariant/identity",
                        "kind": "invariant",
                        "mandatory": True,
                        "estimate": {"tokens": 20, "method": {"method": "declared_by_caller"}},
                    }
                ],
            }
        )
        result = ApiClient(self.base_url).token_context_plan(request)
        self.assertEqual(result["mcp"]["result"]["request"]["depth"], "l1")
        self.assertEqual(result["mcp"]["result"]["candidates"][0]["node_id"], "invariant/identity")

    def test_http_typed_token_context_report_delegates_to_raw_helper(self) -> None:
        with patch.object(
            ApiClient,
            "token_context_plan",
            return_value=token_context_plan_payload(),
        ) as plan:
            report = ApiClient(self.base_url).token_context_plan_report(
                TokenContextPlanArgs.from_wire(
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
                            {"node_id": "evidence/one", "kind": "evidence", "estimate": {"tokens": 1, "method": {"method": "declared_by_caller"}}}
                        ],
                    }
                )
            )
        self.assertIsInstance(report, TokenContextPlanningReport)
        self.assertEqual(report.plan.mandatory_estimate.tokens, 20)
        plan.assert_called_once()

    def test_http_weavelang_compile_round_trips_explicit_replay_controls(self) -> None:
        request = WeaveLangCompileArgs("package demo", execute=False, mode="replay", include_ir=True)
        result = ApiClient(self.base_url).weavelang_compile(request)
        echoed = result["mcp"]["result"]
        self.assertEqual(echoed["source"], "package demo")
        self.assertEqual(echoed["mode"], "replay")
        self.assertTrue(echoed["include_ir"])

    def test_http_typed_weavelang_report_delegates_to_raw_helper(self) -> None:
        with patch.object(
            ApiClient,
            "weavelang_compile",
            return_value=weavelang_compile_payload(),
        ) as compile_tool:
            report = ApiClient(self.base_url).weavelang_compile_report("package demo")
        self.assertIsInstance(report, WeaveLangCompileReport)
        self.assertTrue(report.replay_defaulted)
        compile_tool.assert_called_once_with("package demo")

    def test_http_typed_biocapability_evidence_report_delegates_to_raw_helper(self) -> None:
        request = BioCapabilityEvidenceAuditRequest(
            evidence=[EvidenceItem("evidence-1", "evidence_grounding", "observed")],
            claim_requests=[ClaimRequest("claim-1", "profile", ["evidence_grounding"])],
            metrics={"observations": []},
        )
        with patch.object(
            ApiClient,
            "biocapability_evidence_audit",
            return_value=biocapability_evidence_audit_payload(),
        ) as audit:
            report = ApiClient(self.base_url).biocapability_evidence_audit_report(request)
        self.assertIsInstance(report, BioCapabilityEvidenceAuditReport)
        self.assertTrue(report.release_posture.requires_explicit_claim_request)
        audit.assert_called_once_with(request)

    def test_http_typed_bioatlas_publication_report_delegates_to_raw_helper(self) -> None:
        with patch.object(
            ApiClient,
            "bioatlas_publication_audit",
            return_value=bioatlas_publication_audit_payload(),
        ) as audit:
            report = ApiClient(self.base_url).bioatlas_publication_audit_report(
                {"atlas_id": "atlas-1"},
                release_request={"id": "publication-1", "targets": ["atlas_profile"]},
            )
        self.assertIsInstance(report, BioAtlasPublicationAuditReport)
        self.assertTrue(report.ready_for_requested_publication)
        audit.assert_called_once_with(
            {"atlas_id": "atlas-1"},
            release_request={"id": "publication-1", "targets": ["atlas_profile"]},
        )

    def test_http_typed_capability_audit_report_delegates_to_raw_helper(self) -> None:
        with patch.object(ApiClient, "capability_audit", return_value=capability_audit_payload()) as audit:
            report = ApiClient(self.base_url).capability_audit_report(include_groups=False)
        self.assertIsInstance(report, CapabilityAuditReport)
        self.assertTrue(report.schema_quality.fully_valid)
        audit.assert_called_once_with(include_groups=False)

    def test_http_typed_adapter_plan_report_delegates_to_raw_helper(self) -> None:
        with patch.object(ApiClient, "adapter_plan", return_value=adapter_plan_payload()) as plan:
            report = ApiClient(self.base_url).adapter_plan_report(
                "table-1", "bytes", declared_format="text/csv"
            )
        self.assertIsInstance(report, AdapterPlanReport)
        self.assertEqual(report.selected_adapter_id, "bioprism.tabular")
        plan.assert_called_once_with(
            "table-1",
            "bytes",
            declared_format="text/csv",
            required_conformance=None,
            available_dependencies=None,
        )

    def test_http_typed_tabular_ingest_report_delegates_to_raw_helper(self) -> None:
        request = TabularIngestRequest("cohort.csv", {"profile_id": "RG-DEMO-001"}, csv="subject\nS1\n")
        with patch.object(ApiClient, "tabular_ingest", return_value=tabular_ingest_payload()) as ingest:
            report = ApiClient(self.base_url).tabular_ingest_report(request)
        self.assertIsInstance(report, TabularIngestReport)
        self.assertTrue(report.conformance_verified)
        ingest.assert_called_once_with(request)

    def test_http_typed_conformance_report_delegates_to_raw_helper(self) -> None:
        with patch.object(ApiClient, "conformance_run", return_value=conformance_run_payload()) as run:
            report = ApiClient(self.base_url).conformance_run_report(include_details=False, max_items=100)
        self.assertIsInstance(report, ConformanceRunReport)
        self.assertTrue(report.release_ready)
        self.assertFalse(report.details_included)
        run.assert_called_once_with(include_details=False, max_items=100)

    def test_http_typed_release_report_delegates_to_raw_helper(self) -> None:
        payload = {
            "ok": True,
            "release_ready": True,
            "required_check_count": 1,
            "check_count": 1,
            "invocation_failures": 0,
            "blocking_count": 0,
            "blockers": [],
            "checks": [{
                "index": 0,
                "kind": "conformance_run",
                "required": True,
                "advisory": False,
                "evaluated": True,
                "gate": True,
                "passed": True,
                "result_digest": "d" * 64,
            }],
            "guarantees": [],
            "limitations": [],
        }
        request = ReleaseAuditArgs([ReleaseAuditCheckRequest("conformance_run", {})])
        with patch.object(ApiClient, "release_audit", return_value=payload) as audit:
            report = ApiClient(self.base_url).release_audit_report(request)
        self.assertIsInstance(report, ReleaseAuditReport)
        self.assertTrue(report.release_ready)
        audit.assert_called_once_with(request)

    def test_http_arguments_and_async_facade(self) -> None:
        with self.assertRaises(ArgumentError):
            ApiClient(self.base_url, bearer_token="short")
        client = AsyncApiClient(ApiClient(self.base_url))

        async def run() -> None:
            self.assertTrue((await client.health())["ok"])
            self.assertEqual((await client.replay("sub", [1]))["replayed"][0]["state"], "pending")
            catalogue = await client.tool_catalogue()
            self.assertEqual((await client.plan_tool("echo", {"value": 5}, catalogue=catalogue)).tool, "echo")
            self.assertEqual((await client.tool_checked("echo", {"value": 5}, catalogue=catalogue))["mcp"]["result"]["value"], 5)
            mission = await client.mission_preflight(
                MissionRequest(
                    "mission-http-async",
                    "check",
                    [MissionStep("one", "data", "read", "check", "echo", {"value": 5})],
                ),
                catalogue=catalogue,
            )
            self.assertTrue(mission.fully_checked)
            remote_preflight = await client.preflight_mission(
                MissionRequest(
                    "mission-http-async-remote-preflight",
                    "check",
                    [MissionStep("one", "data", "read", "check", "echo", {"value": 5})],
                )
            )
            self.assertTrue(remote_preflight["preflight"])
            self.assertEqual(remote_preflight["dispatch"], "not_started")
            self.assertEqual((await client.call_tool("echo", {"async": True}))["tool"], "echo")
            self.assertEqual((await client.submit_mission(MissionRequest("async-1", "run", [MissionStep("one", "data", "read", "run", "echo", {"value": 1})]))).status, "queued")
            status = await client.mission_status("async-1")
            self.assertEqual(status.status, "succeeded")
            self.assertIsNotNone(status.progress)
            self.assertEqual(status.progress.phase, "succeeded")
            waited = await client.wait_mission("async-1", timeout=1.0, poll_interval=0.01)
            self.assertEqual(waited.status, "succeeded")
            trace = await client.mission_trace("async-1")
            self.assertEqual(trace.events[-1].event, "mission.completed")
            inventory = await client.missions(status="succeeded", limit=5)
            self.assertEqual(inventory["missions"][0]["mission_id"], "async-1")
            typed_inventory = await client.mission_inventory(status="succeeded", limit=5)
            self.assertIsInstance(typed_inventory, MissionInventoryPage)
            self.assertTrue(typed_inventory.missions[0].terminal)
            self.assertTrue((await client.cancel_mission("async-1", "operator stop")).cancel_requested)
            self.assertEqual(
                (await client.capability_route("async route", [{"id": "release", "tool": "bundle_verify"}]))["mcp"]["result"]["goal"],
                "async route",
            )
            self.assertEqual(
                (await client.adapter_plan("variants", "bytes", declared_format="text/vcf"))["mcp"]["result"]["declared_format"],
                "text/vcf",
            )
            release_request = ReleaseAuditArgs([ReleaseAuditCheckRequest("conformance_run", {})])
            self.assertEqual(
                (await client.release_audit(release_request))["mcp"]["result"]["checks"][0]["kind"],
                "conformance_run",
            )
            self.assertEqual(
                (await client.operations_catalog(max_items=2))["mcp"]["result"]["max_items"],
                2,
            )
            self.assertEqual(
                (await client.ops_acceptance(max_items=3))["mcp"]["result"]["max_items"],
                3,
            )
            safety_request = RiskAssessmentRequest("async-subject", {"scale": "moderate"})
            self.assertEqual(
                (await client.safety_release_gate(safety_request))["mcp"]["result"]["assessment"]["subject"],
                "async-subject",
            )
            medical_request = MedicalBoundaryRequest({"side": "clinical", "category": "treatment_selection", "label": "treatment"})
            self.assertEqual(
                (await client.medical_boundary_check(medical_request))["mcp"]["result"]["output"]["category"],
                "treatment_selection",
            )
            self.assertTrue(
                (await client.safety_posture(include_threats=True))["mcp"]["result"]["include_threats"]
            )
            measurement = MeasurementCompareArgs({"label": "left"}, {"label": "right"})
            self.assertEqual(
                (await client.measurement_compare(measurement))["mcp"]["result"]["left"]["label"],
                "left",
            )
            hub_request = HubSearchArgs({"members": {}}, [], {"facets": []})
            self.assertEqual(
                (await client.hub_search(hub_request))["mcp"]["result"]["query"]["facets"],
                [],
            )
            hub_resolve_request = HubResolveArgs({}, [], {"name": "bioprism/root"})
            self.assertEqual(
                (await client.hub_resolve(hub_resolve_request))["mcp"]["result"]["request"]["name"],
                "bioprism/root",
            )
            hub_lock_request = HubLockArgs({}, [], {"name": "bioprism/root"}, max_items=2)
            self.assertEqual(
                (await client.hub_lock(hub_lock_request))["mcp"]["result"]["max_items"],
                2,
            )
            observed_request = ObservedWorldDeclareArgs("observed-demo", [], {"cohort_size": 0}, [])
            self.assertEqual(
                (await client.observed_world_declare(observed_request))["mcp"]["result"]["id"],
                "observed-demo",
            )
            with patch.object(
                AsyncApiClient,
                "adapter_plan",
                new_callable=AsyncMock,
                return_value=adapter_plan_payload(),
            ) as plan:
                report = await client.adapter_plan_report("table-1", "bytes", declared_format="text/csv")
            self.assertIsInstance(report, AdapterPlanReport)
            self.assertEqual(report.plan.candidates[0].status, "ready")
            plan.assert_awaited_once_with(
                "table-1",
                "bytes",
                declared_format="text/csv",
                required_conformance=None,
                available_dependencies=None,
            )
            request = TabularIngestRequest("cohort.csv", {"profile_id": "RG-DEMO-001"}, csv="subject\nS1\n")
            with patch.object(
                AsyncApiClient,
                "tabular_ingest",
                new_callable=AsyncMock,
                return_value=tabular_ingest_payload(),
            ) as ingest:
                report = await client.tabular_ingest_report(request)
            self.assertIsInstance(report, TabularIngestReport)
            self.assertEqual(report.facts[0]["value"], "S1")
            ingest.assert_awaited_once_with(request)
            with patch.object(
                AsyncApiClient,
                "conformance_run",
                new_callable=AsyncMock,
                return_value=conformance_run_payload(),
            ) as run:
                report = await client.conformance_run_report(include_details=False, max_items=100)
            self.assertTrue(report.release_ready)
            run.assert_awaited_once_with(include_details=False, max_items=100)
            evidence_request = BioCapabilityEvidenceAuditRequest(
                [EvidenceItem("grounding", "evidence_grounding", "observed", support={"source": "ledger", "scope": "pack/1"})],
                [ClaimRequest("claim", "grounded profile", ("evidence_grounding",))],
                vectors=({"system": "a"}, {"system": "b"}),
            )
            self.assertEqual(
                (await client.biocapability_evidence_audit(evidence_request))["mcp"]["result"]["max_items"],
                100,
            )
            self.assertEqual(
                (await client.bioql_compile("SELECT sample.id", {"schema_version": "v1"}))["mcp"]["result"]["schema"]["schema_version"],
                "v1",
            )
            self.assertEqual(
                (await client.routing_decide({"features": {}}, [{"task_id": "other"}], {"safe_default": "abstain"}, task_id="new"))["mcp"]["result"]["task_id"],
                "new",
            )
            self.assertEqual(
                (await client.fiber_compile("world.json", "query.json"))["mcp"]["result"]["layer"],
                "l0",
            )
            self.assertEqual(
                (await client.fiber_refine("l1", handle={"digest": "async"}))["mcp"]["result"]["handle"]["digest"],
                "async",
            )
            self.assertEqual(
                (await client.fiber_explain("world.json", "query.json"))["mcp"]["result"]["world"],
                "world.json",
            )
            self.assertEqual(
                (await client.fiber_verify("certificate.json"))["mcp"]["result"]["certificate"],
                "certificate.json",
            )
            self.assertFalse(
                (await client.projection_bundle("world.json", "query.json"))["mcp"]["result"]["include_views"]
            )
            self.assertEqual(
                (await client.repository_catalog(limit=2))["mcp"]["result"]["limit"],
                2,
            )
            self.assertEqual(
                (await client.repository_bundle({"id": "route-async"}))["mcp"]["result"]["route"]["id"],
                "route-async",
            )
            self.assertEqual(
                (await client.repository_impact("docs/README"))["mcp"]["result"]["changed"],
                "docs/README",
            )
            self.assertEqual(
                (await client.telemetry_project({"kind": "event"}, {"treatments": {}}, "trace-async"))["mcp"]["result"]["trace"],
                "trace-async",
            )
            self.assertFalse((await client.event_page(review_id="a" * 64)).gap)
            self.assertEqual((await client.event_stream()).events[0].data, '{"mission_id":"async-1"}')
            self.assertTrue((await client.route_review_evidence("a" * 64)).found)
            with patch.object(
                AsyncApiClient,
                "developer_delivery_audit",
                new_callable=AsyncMock,
                return_value=developer_delivery_audit_payload(),
            ) as audit:
                report = await client.developer_delivery_audit_report(
                    request_id="delivery-1", targets=["local_delivery"]
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
            with patch.object(
                AsyncApiClient,
                "developer_platform_status",
                new_callable=AsyncMock,
                return_value=developer_platform_status_payload(),
            ) as platform:
                report = await client.developer_platform_status_report(max_items=7)
            self.assertIsInstance(report, DeveloperPlatformStatusReport)
            self.assertEqual(report.devplat.foreign_subject_count, 1)
            platform.assert_awaited_once_with(include_details=False, max_items=7)
            with patch.object(
                AsyncApiClient,
                "token_context_plan",
                new_callable=AsyncMock,
                return_value=token_context_plan_payload(),
            ) as token_plan:
                report = await client.token_context_plan_report(
                    TokenContextPlanArgs.from_wire(
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
                                {"node_id": "evidence/one", "kind": "evidence", "estimate": {"tokens": 1, "method": {"method": "declared_by_caller"}}}
                            ],
                        }
                    )
                )
            self.assertEqual(report.plan.plan_digest, "b" * 64)
            token_plan.assert_awaited_once()
            with patch.object(
                AsyncApiClient,
                "weavelang_compile",
                new_callable=AsyncMock,
                return_value=weavelang_compile_payload(),
            ) as compile_tool:
                report = await client.weavelang_compile_report("package demo")
            self.assertTrue(report.execution_local_only)
            compile_tool.assert_awaited_once_with("package demo")
            request = BioCapabilityEvidenceAuditRequest(
                evidence=[EvidenceItem("evidence-1", "evidence_grounding", "observed")],
                claim_requests=[ClaimRequest("claim-1", "profile", ["evidence_grounding"])],
                metrics={"observations": []},
            )
            with patch.object(
                AsyncApiClient,
                "biocapability_evidence_audit",
                new_callable=AsyncMock,
                return_value=biocapability_evidence_audit_payload(),
            ) as evidence_audit:
                report = await client.biocapability_evidence_audit_report(request)
            self.assertFalse(report.ready_for_requested_claims)
            evidence_audit.assert_awaited_once_with(request)
            with patch.object(
                AsyncApiClient,
                "bioatlas_publication_audit",
                new_callable=AsyncMock,
                return_value=bioatlas_publication_audit_payload(),
            ) as publication_audit:
                report = await client.bioatlas_publication_audit_report(
                    {"atlas_id": "atlas-1"},
                    release_request={"id": "publication-1", "targets": ["atlas_profile"]},
                )
            self.assertTrue(report.cross_layer.atlas_aggregation_ready)
            publication_audit.assert_awaited_once_with(
                {"atlas_id": "atlas-1"},
                release_request={"id": "publication-1", "targets": ["atlas_profile"]},
            )
            with patch.object(
                AsyncApiClient,
                "capability_audit",
                new_callable=AsyncMock,
                return_value=capability_audit_payload(),
            ) as audit:
                report = await client.capability_audit_report(include_groups=True)
            self.assertTrue(report.catalogue_complete)
            audit.assert_awaited_once_with(include_groups=True)

        asyncio.run(run())


if __name__ == "__main__":
    unittest.main()
