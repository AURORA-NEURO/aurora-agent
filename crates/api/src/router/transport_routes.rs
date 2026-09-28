//! HTTP discovery and protocol-ingress routes.
//!
//! These endpoints expose the transport contract and proxy calls to the same MCP dispatcher;
//! they do not create a second domain-execution path.

use super::*;

impl ApiRouter {
    pub(super) fn health(&self, _ready: bool) -> HttpResponse {
        let metrics = self.event_metrics();
        let payload = json!({
            "ok": true,
            "ready": true,
            "service": SERVER_NAME,
            "api_version": API_VERSION,
            "protocol_version": PROTOCOL_VERSION,
            "event_metrics": metrics,
            "guarantees": [
                "HTTP requests are bounded before JSON parsing",
                "domain calls delegate to the same MCP server implementation",
                "event cursors expose retention gaps instead of silently skipping history"
            ],
        });
        HttpResponse::json(200, &payload)
    }

    pub(super) fn index(&self) -> HttpResponse {
        HttpResponse::json(
            200,
            &json!({
                "service": SERVER_NAME,
                "api_version": API_VERSION,
                "links": {
                    "health": "/healthz",
                    "ready": "/readyz",
                    "openapi": "/v1/openapi.json",
                    "capabilities": "/v1/capabilities",
                    "capability_dashboard": "/v1/capabilities/dashboard",
                    "capability_route": "/v1/capabilities/route",
                    "capability_route_review": "/v1/capabilities/route/review",
                    "capability_route_plan": "/v1/capabilities/route/plan",
                    "capability_route_plan_verify": "/v1/capabilities/route/plan/verify",
                    "recovery": "/v1/recovery",
                    "operations_snapshot": "/v1/operations/snapshot",
                    "operations_domains": "/v1/operations/domains",
                    "operations_gates": "/v1/operations/gates",
                    "operations_gate_reviews": "/v1/operations/gate-reviews",
                    "operations_handoff": "/v1/operations/handoff",
                    "domain_workflows": "/v1/domain-workflows",
                    "domain_reports": "/v1/domain-reports",
                    "domain_report_coverage": "/v1/domain-reports/coverage",
                    "domain_evidence_harmonize": "/v1/domain-evidence/harmonize",
                    "domain_evidence_harmonization_coverage": "/v1/domain-evidence/harmonization/coverage",
                    "domain_evidence_lineage": "/v1/domain-evidence/lineage",
                    "domain_evidence_intake": "/v1/domain-evidence/intake",
                    "domain_evidence_source_plan": "/v1/domain-evidence/sources",
                    "domain_evidence_source_execute": "/v1/domain-evidence/sources/execute",
                    "domain_evidence_coverage": "/v1/domain-evidence/coverage",
                    "domain_decision_readiness": "/v1/domain-decision-readiness",
                    "control_plane_readiness": "/v1/control-plane-readiness",
                    "control_plane_readiness_compare": "/v1/control-plane-readiness/compare",
                    "control_plane_readiness_compare_retained": "/v1/control-plane-readiness/compare-retained",
                    "domain_workflow_scaffold": "/v1/domain-workflows/scaffold",
                    "domain_workflow_instantiate": "/v1/domain-workflows/instantiate",
                    "domain_workflow_portfolio": "/v1/domain-workflows/portfolio",
                    "domain_workflow_portfolio_verify": "/v1/domain-workflows/portfolio/verify",
                    "developer_workbench_verify": "/v1/developer-workbench/verify",
                    "developer_workbench_reports": "/v1/developer-workbench/reports",
                    "developer_workbench_report_persistence": "/v1/developer-workbench/reports/persistence",
                    "developer_workbench_report_persistence_flush": "/v1/developer-workbench/reports/persistence/flush",
                    "ci_provider_evidence": "/v1/ci/provider-evidence",
                    "ci_provider_evidence_persistence": "/v1/ci/provider-evidence/persistence",
                    "ci_provider_evidence_persistence_flush": "/v1/ci/provider-evidence/persistence/flush",
                    "domain_workflow_verify": "/v1/domain-workflows/verify",
                    "domain_workflow_reconcile": "/v1/domain-workflows/reconcile",
                    "domain_workflow_reconciliations": "/v1/domain-workflows/reconciliations",
                    "domain_workflow_reconciliation_persistence": "/v1/domain-workflows/reconciliations/persistence",
                    "domain_workflow_reconciliation_persistence_flush": "/v1/domain-workflows/reconciliations/persistence/flush",
                    "tools": "/v1/tools",
                    "missions": "/v1/missions",
                     "mission_provenance": "/v1/missions/{mission_id}/provenance",
                     "mission_claims": "/v1/missions/{mission_id}/claims",
                     "mission_evaluator_replay": "/v1/missions/{mission_id}/evaluator-replay",
                     "mission_evaluator_replay_compare": "/v1/missions/{mission_id}/evaluator-replay/compare",
                     "mission_evidence_bundle": "/v1/missions/{mission_id}/evidence-bundle",
                     "mission_evidence_bundle_verify": "/v1/evidence-bundles/verify",
                     "evidence_bundles": "/v1/evidence-bundles",
                     "evidence_bundle_persistence": "/v1/evidence-bundles/persistence",
                     "evidence_bundle_persistence_flush": "/v1/evidence-bundles/persistence/flush",
                    "artifacts": "/v1/artifacts",
                    "domain_decision_readiness_query": "/v1/domain-decision-readiness",
                    "control_plane_readiness_query": "/v1/control-plane-readiness",
                    "control_plane_readiness_compare_retained": "/v1/control-plane-readiness/compare-retained",
                     "artifact_persistence": "/v1/artifacts/persistence",
                     "artifact_persistence_flush": "/v1/artifacts/persistence/flush",
                    "mission_persistence": "/v1/missions/persistence",
                    "mission_queue": "/v1/missions/queue",
                     "mission_queue_persistence": "/v1/missions/queue/persistence",
                     "mission_queue_persistence_flush": "/v1/missions/queue/persistence/flush",
                     "mission_queue_authority_release_lock": "/v1/missions/queue/authority/release-lock",
                    "mission_preflight": "/v1/missions/preflight",
                    "events": "/v1/events",
                    "delivery_receipt_events": "/v1/delivery-receipts/{receipt_id}/events",
                    "delivery_receipt_attempts": "/v1/delivery-receipts/{receipt_id}/attempts",
                    "route_review_evidence": "/v1/route-reviews/{review_id}/evidence",
                    "event_persistence": "/v1/events/persistence",
                    "webhooks": "/v1/webhooks/subscriptions",
                    "delivery_attempts": "/v1/webhooks/subscriptions/{id}/attempts"
                }
            }),
        )
    }

    pub(super) fn capabilities(&self) -> HttpResponse {
        HttpResponse::json(
            200,
            &json!({
                "api_version": API_VERSION,
                "mcp_protocol_version": PROTOCOL_VERSION,
                "tool_count": bioprism_mcp::tool_definitions().len(),
                "resource_count": bioprism_mcp::resource_definitions().len(),
                "workspace": bioprism_mcp::workspace_capabilities(),
                "transport": {
                    "rest_tools": true,
                    "json_rpc": true,
                    "event_cursor": true,
                    "server_sent_events_snapshot": true,
                    "async_missions": true,
                    "mission_preflight": true,
                    "mission_inventory": true,
                    "mission_execution_provenance": true,
                    "mission_claim_lineage": true,
                    "mission_trace": true,
                    "delivery_receipt_events": true,
                    "delivery_receipt_attempt_provenance": true,
                    "route_review_evidence": true,
                    "mission_evidence_bundle_registry": true,
                    "mission_evidence_bundle_import": true,
                    "mission_evidence_bundle_query": true,
                    "mission_evidence_bundle_persistence": self.config.evidence_state_path.is_some(),
                    "artifact_registry": true,
                    "artifact_registry_lineage": true,
                    "artifact_registry_persistence": self.config.artifact_state_path.is_some(),
                    "ci_provider_evidence_registry": true,
                    "ci_provider_evidence_lineage": true,
                    "ci_provider_evidence_persistence": self.config.ci_provider_evidence_state_path.is_some(),
                    "domain_report_projection": true,
                    "domain_report_coverage": true,
                    "domain_evidence_harmonization": true,
                    "domain_evidence_harmonization_coverage": true,
                    "domain_evidence_lineage": true,
                    "domain_evidence_intake": true,
                    "domain_evidence_source_plan": true,
                    "domain_evidence_source_execute": true,
                    "domain_evidence_coverage": true,
                    "domain_decision_readiness_query": true,
                    "control_plane_readiness_audit": true,
                    "control_plane_readiness_compare": true,
                    "control_plane_readiness_compare_retained": true,
                    "control_plane_readiness_query": true,
                    "capability_dashboard": true,
                    "capability_route": true,
                    "capability_route_review": true,
                    "capability_route_plan": true,
                    "capability_route_plan_verify": true,
                    "recovery_matrix": true,
                    "operations_snapshot": true,
                    "domain_coverage": true,
                    "operations_domains": true,
                    "operations_gates": true,
                    "operations_gate_reviews": true,
                    "operations_handoff": true,
                    "domain_workflow_catalogue": true,
                    "domain_workflow_scaffold": true,
                    "domain_workflow_instantiate": true,
                    "domain_workflow_portfolio": true,
                    "domain_workflow_portfolio_verify": true,
                    "developer_workbench_verify": true,
                    "developer_workbench_report_registry": true,
                    "developer_workbench_report_persistence": self.config.workbench_state_path.is_some(),
                    "domain_workflow_verify": true,
                    "domain_workflow_reconcile": true,
                    "domain_workflow_reconciliation_registry": true,
                    "domain_workflow_reconciliation_persistence": self.config.reconciliation_state_path.is_some(),
                    "max_mission_trace_events": MAX_MISSION_TRACE_EVENTS,
                    "cooperative_mission_cancellation": true,
                    "durable_mission_snapshots": self.config.mission_state_path.is_some(),
                    "durable_mission_queue_snapshots": self.config.mission_queue_state_path.is_some(),
                    "durable_event_snapshots": self.config.event_state_path.is_some(),
                    "signed_webhook_outbox": true,
                    "delivery_failure_inspection": true,
                    "bounded_delivery_replay": true,
                    "delivery_attempt_provenance": true,
                    "restart_aware_webhook_metadata": true,
                    "explicit_secret_rebind": true,
                    "grpc": false,
                    "tls": false,
                    "external_delivery_worker": false
                },
                "limits": {
                    "max_header_bytes": self.config.max_header_bytes,
                    "max_body_bytes": self.config.max_body_bytes,
                    "event_capacity": self.config.event_capacity,
                    "mission_state_file_bytes": MAX_MISSION_STATE_FILE_BYTES,
                    "persisted_mission_result_bytes": MAX_PERSISTED_MISSION_RESULT_BYTES,
                    "persisted_mission_provenance_bytes": MAX_PERSISTED_MISSION_PROVENANCE_BYTES,
                    "event_state_file_bytes": MAX_EVENT_STATE_FILE_BYTES,
                    "evidence_registry_file_bytes": MAX_EVIDENCE_REGISTRY_BYTES,
                    "evidence_registry_max_bundles": bioprism_devplat::MAX_EVIDENCE_REGISTRY_BUNDLES,
                    "evidence_registry_max_query_items": bioprism_devplat::MAX_EVIDENCE_REGISTRY_QUERY_ITEMS,
                    "workflow_reconciliation_file_bytes": MAX_WORKFLOW_RECONCILIATION_STATE_BYTES,
                    "workflow_reconciliation_max_records": bioprism_devplat::MAX_DOMAIN_WORKFLOW_RECONCILIATIONS,
                    "workflow_reconciliation_max_query_items": bioprism_devplat::MAX_DOMAIN_WORKFLOW_RECONCILIATION_QUERY_ITEMS,
                    "artifact_registry_file_bytes": MAX_ARTIFACT_REGISTRY_BYTES,
                    "artifact_registry_max_records": bioprism_devplat::MAX_ARTIFACT_REGISTRY_RECORDS,
                    "artifact_registry_max_query_items": bioprism_devplat::MAX_ARTIFACT_REGISTRY_QUERY_ITEMS,
                    "ci_provider_evidence_registry_file_bytes": MAX_CI_PROVIDER_EVIDENCE_REGISTRY_STATE_BYTES,
                    "ci_provider_evidence_max_records": bioprism_devplat::MAX_CI_PROVIDER_EVIDENCE_RECORDS,
                    "ci_provider_evidence_max_query_items": bioprism_devplat::MAX_CI_PROVIDER_EVIDENCE_QUERY_ITEMS,
                    "delivery_error_bytes": crate::events::MAX_DELIVERY_ERROR_BYTES,
                    "webhook_filters": MAX_FILTERS
                }
            }),
        )
    }

    pub(super) fn tools(&self) -> HttpResponse {
        HttpResponse::json(
            200,
            &json!({
                "api_version": API_VERSION,
                "tools": bioprism_mcp::tool_definitions(),
                "call_shape": "POST /v1/tools/{name} with a JSON object body"
            }),
        )
    }

    pub(super) fn bounded_evolution_admit(
        &self,
        request: &HttpRequest,
        request_id: &str,
    ) -> HttpResponse {
        let arguments = match self.json_object(request) {
            Ok(arguments) => arguments,
            Err(error) => return self.error(400, "invalid_json", &error, request_id),
        };
        let call = Request {
            id: Some(Value::String(request_id.to_string())),
            method: "tools/call".into(),
            params: json!({
                "name": "adapter_bounded_evolution",
                "arguments": arguments,
            }),
        };
        let mut server = self.server.clone();
        let Some(response) = server.handle(&call) else {
            return self.error(
                500,
                "dispatch_failed",
                "bounded evolution dispatch produced no response",
                request_id,
            );
        };
        let wire = response.to_json();
        self.record_tool_event(request_id, "adapter_bounded_evolution", &wire);
        let transport_ok = wire.get("error").is_none();
        HttpResponse::json(
            if transport_ok {
                200
            } else {
                response_status(&wire)
            },
            &json!({
                "ok": transport_ok,
                "api_version": API_VERSION,
                "feature_id": bioprism_adapter::BOUNDED_EVOLUTION_FEATURE_ID,
                "tool": "adapter_bounded_evolution",
                "request_id": request_id,
                "mcp": wire,
                "guarantees": [
                    "REST and MCP use the same bounded evolution dispatcher and receipt contract",
                    "replay, evidence, safety, policy, budget, protected-closure, and preclinical gates remain explicit",
                    "the endpoint admits receipt metadata only and never mutates or deploys candidate artifacts"
                ],
                "limitations": [
                    "sandbox execution, independent review, signing, and release governance remain outside this transport endpoint"
                ]
            }),
        )
    }

    pub(super) fn rpc(&self, request: &HttpRequest, request_id: &str) -> HttpResponse {
        let text = match std::str::from_utf8(&request.body) {
            Ok(text) => text,
            Err(_) => return self.error(400, "invalid_json", "body is not UTF-8", request_id),
        };
        let parsed = match Request::parse(text) {
            Ok(request) => request,
            Err(error) => return HttpResponse::json(400, &error.to_json()),
        };
        if parsed.method == "initialize" {
            return HttpResponse::json(
                200,
                &Response::result(
                    parsed.id.clone(),
                    json!({
                        "protocolVersion": PROTOCOL_VERSION,
                        "capabilities": {
                            "tools": { "listChanged": false },
                            "resources": { "subscribe": false, "listChanged": false }
                        },
                        "serverInfo": { "name": SERVER_NAME, "version": env!("CARGO_PKG_VERSION") },
                        "instructions": "Use the REST routes for ordinary calls, or continue with JSON-RPC tools/list, tools/call, resources/list, and resources/read."
                    }),
                )
                .to_json(),
            );
        }
        if parsed.is_notification() {
            return HttpResponse::empty(204);
        }
        let method = parsed.method.clone();
        let tool = parsed
            .params
            .get("name")
            .and_then(Value::as_str)
            .map(str::to_string);
        let mut server = self.server.clone();
        let Some(response) = server.handle(&parsed) else {
            return HttpResponse::empty(204);
        };
        let wire = response.to_json();
        if method == "tools/call" {
            if let Some(tool) = tool {
                self.record_tool_event(request_id, &tool, &wire);
            }
            // A synchronous MCP call may execute a workflow-bound mission directly rather than
            // through the asynchronous mission worker. The server writes the shared registry;
            // checkpoint it before returning the transport response when durability is enabled.
            let _ = self.reconciliation_persistence.persist();
            let _ = self.artifact_persistence.persist();
            let _ = self.workflow_execution_evidence_persistence.persist();
            let _ = self.ci_provider_evidence_persistence.persist();
        }
        HttpResponse::json(response_status(&wire), &wire)
    }

    pub(super) fn rest_tool(&self, request: &HttpRequest, request_id: &str) -> HttpResponse {
        let segments = match request.path_segments() {
            Ok(segments) => segments,
            Err(error) => return self.error(400, "invalid_path", &error.to_string(), request_id),
        };
        if segments.len() != 3 || segments[0] != "v1" || segments[1] != "tools" {
            return self.error(404, "not_found", "tool route does not exist", request_id);
        }
        let arguments = match self.json_object(request) {
            Ok(arguments) => arguments,
            Err(error) => return self.error(400, "invalid_json", &error, request_id),
        };
        let tool = &segments[2];
        let call = Request {
            id: Some(Value::String(request_id.to_string())),
            method: "tools/call".into(),
            params: json!({ "name": tool, "arguments": arguments }),
        };
        let mut server = self.server.clone();
        let Some(response) = server.handle(&call) else {
            return self.error(
                500,
                "dispatch_failed",
                "tool dispatch produced no response",
                request_id,
            );
        };
        let wire = response.to_json();
        self.record_tool_event(request_id, tool, &wire);
        let _ = self.reconciliation_persistence.persist();
        let _ = self.artifact_persistence.persist();
        let _ = self.workflow_execution_evidence_persistence.persist();
        let _ = self.ci_provider_evidence_persistence.persist();
        let transport_ok = wire.get("error").is_none();
        HttpResponse::json(
            if transport_ok {
                200
            } else {
                response_status(&wire)
            },
            &json!({
                "ok": transport_ok,
                "tool": tool,
                "request_id": request_id,
                "mcp": wire,
                "guarantee": "REST and MCP calls share the same in-process tool dispatcher"
            }),
        )
    }
}
