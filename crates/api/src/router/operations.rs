//! Operational snapshots, evidence gates, and review routes.
//!
//! The handlers share the router state but keep this route family reviewable on its own.

use super::*;

impl ApiRouter {
    pub(super) fn operations_snapshot(
        &self,
        request: &HttpRequest,
        request_id: &str,
    ) -> HttpResponse {
        let query = match request.query() {
            Ok(query) => query,
            Err(error) => return self.error(400, "invalid_query", &error.to_string(), request_id),
        };
        for key in query.keys() {
            if key != "after" && key != "limit" {
                return self.error(
                    400,
                    "invalid_query",
                    "operations snapshot accepts only after and limit",
                    request_id,
                );
            }
        }
        let after = match query_u64(&query, "after", 0) {
            Ok(value) => value,
            Err(error) => return self.error(400, "invalid_query", &error, request_id),
        };
        let limit = match query_usize(&query, "limit", 100) {
            Ok(value) if (1..=MAX_OPERATIONS_SNAPSHOT_LIMIT).contains(&value) => value,
            Ok(_) => {
                return self.error(
                    422,
                    "invalid_query",
                    &format!("limit must be between 1 and {MAX_OPERATIONS_SNAPSHOT_LIMIT}"),
                    request_id,
                )
            }
            Err(error) => return self.error(400, "invalid_query", &error, request_id),
        };

        let recent_events = match self.events.lock() {
            Ok(events) => match events.events(after, limit) {
                Ok(page) => serde_json::to_value(page).unwrap_or_else(|_| json!({})),
                Err(error) => return self.error(422, "invalid_query", &error, request_id),
            },
            Err(_) => {
                return self.error(
                    500,
                    "event_log_unavailable",
                    "event log is unavailable",
                    request_id,
                )
            }
        };
        let metrics = self.event_metrics();
        let mission_summary = match self.operations_mission_summary() {
            Ok(summary) => summary,
            Err(code) => {
                return self.error(500, code, "mission registry is unavailable", request_id)
            }
        };
        let mission_persistence = response_value(self.mission_persistence_status());
        let mission_queue_persistence = response_value(self.mission_queue_persistence_status());
        let event_persistence = response_value(self.event_persistence_status());
        let evidence_persistence = response_value(self.evidence_persistence_status());
        let reconciliation_persistence = response_value(self.reconciliation_persistence_status());
        let artifact_persistence = response_value(self.artifact_persistence_status());
        let ci_provider_evidence_persistence =
            response_value(self.ci_provider_evidence_persistence_status());
        let reconciliation_summary = match self.reconciliation_registry.lock() {
            Ok(registry) => registry.operator_summary(),
            Err(_) => {
                return self.error(
                    500,
                    "reconciliation_registry_unavailable",
                    "workflow reconciliation registry is unavailable",
                    request_id,
                )
            }
        };
        let recovery = response_value(self.recovery_matrix());
        let mut operator_actions = vec![
            "advance the event cursor with recent_events.next_after and inspect gap before claiming continuity".to_string(),
            "use recovery.boundaries and persistence digests before treating restart state as authoritative".to_string(),
            "treat tool completion as local evidence; verify any external delivery or scientific claim independently".to_string(),
        ];
        if metrics.pending_deliveries > 0 {
            operator_actions.push(
                "inspect pending webhook deliveries and correlated delivery attempts through the outbox routes".to_string(),
            );
        }
        if self.config.event_state_path.is_none() {
            operator_actions.push(
                "configure event_state_path if event rows, subscriptions, outbox rows, and attempt provenance must survive restart".to_string(),
            );
        }
        if self.config.mission_state_path.is_none() {
            operator_actions.push(
                "configure mission_state_path if terminal mission snapshots must survive restart"
                    .to_string(),
            );
        }
        if self.config.mission_queue_state_path.is_none() {
            operator_actions.push(
                "configure mission_queue_state_path if mission lease, idempotency, and restart recovery state must survive an API restart".to_string(),
            );
        }
        if self.config.evidence_state_path.is_none() {
            operator_actions.push(
                "configure evidence_state_path if independently verified evidence bundles must survive an API restart"
                    .to_string(),
            );
        }
        if self.config.reconciliation_state_path.is_none() {
            operator_actions.push(
                "configure reconciliation_state_path if digest-valid workflow reconciliation audit records must survive an API restart"
                    .to_string(),
            );
        }
        if self.config.artifact_state_path.is_none() {
            operator_actions.push(
                "configure artifact_state_path if cross-domain artifact records and parent-lineage inspection must survive an API restart".to_string(),
            );
        }
        if self.config.ci_provider_evidence_state_path.is_none() {
            operator_actions.push(
                "configure ci_provider_evidence_state_path if re-audited provider CI evidence and its artifact/log/attestation joins must survive an API restart".to_string(),
            );
        }

        HttpResponse::json(
            200,
            &json!({
                "ok": true,
                "schema": "bioprism-operations-snapshot/0.1",
                "service": SERVER_NAME,
                "api_version": API_VERSION,
                "protocol_version": PROTOCOL_VERSION,
                "after": after,
                "limit": limit,
                "recent_events": recent_events,
                "event_metrics": metrics,
                "mission_summary": mission_summary,
                "persistence": {
                    "missions": mission_persistence,
                    "mission_queue": mission_queue_persistence,
                    "events": event_persistence,
                    "evidence_bundles": evidence_persistence,
                    "workflow_reconciliations": reconciliation_persistence,
                    "artifacts": artifact_persistence,
                    "ci_provider_evidence": ci_provider_evidence_persistence
                },
                "reconciliation_summary": reconciliation_summary,
                "recovery": recovery,
                "domain_coverage": operations_domain_coverage(),
                "consistency": {
                    "read_model": "bounded composition of process-local stores",
                    "cross_store_atomic": false,
                    "event_cursor_authoritative": true,
                    "clock_free": true,
                    "underlying_routes_remain_authoritative": true
                },
                "capabilities": {
                    "tool_count": bioprism_mcp::tool_definitions().len(),
                    "resource_count": bioprism_mcp::resource_definitions().len(),
                    "rest_tools": true,
                    "json_rpc": true,
                    "event_cursor": true,
                    "async_missions": true,
                    "mission_inventory": true,
                    "mission_execution_queue": true,
                    "mission_queue_persistence": self.config.mission_queue_state_path.is_some(),
                    "mission_execution_provenance": true,
                    "mission_claim_lineage": true,
                    "mission_evaluator_replay_query": true,
                    "mission_evaluator_replay_compare": true,
                    "mission_evidence_bundle_export": true,
                    "mission_evidence_bundle_verify": true,
                    "mission_evidence_bundle_registry": true,
                    "mission_evidence_bundle_import": true,
                    "mission_evidence_bundle_query": true,
                    "mission_evidence_bundle_persistence": self.config.evidence_state_path.is_some(),
                    "workflow_reconciliation_registry": true,
                    "workflow_reconciliation_persistence": self.config.reconciliation_state_path.is_some(),
                    "ci_provider_evidence_registry": true,
                    "ci_provider_evidence_persistence": self.config.ci_provider_evidence_state_path.is_some(),
                    "operations_snapshot": true,
                    "domain_coverage": true,
                    "operations_domains": true,
                    "operations_gates": true,
                    "operations_gate_reviews": true,
                    "operations_handoff": true,
                    "delivery_attempt_provenance": true,
                    "external_delivery_worker": false
                },
                "operator_actions": operator_actions,
                "guarantees": [
                    "the event page is bounded by the caller-supplied cursor and the server limit",
                    "mission counts come from the process-local authoritative registry without returning terminal reports",
                    "persistence and recovery views retain their existing digest, integrity, and non-claim semantics",
                    "reconciliation_summary counts only stored digest-valid reports and keeps completion, integrity, and evidence posture separate",
                    "the snapshot reports local evidence and capability boundaries; it does not execute tools or external effects"
                ],
                "non_claims": [
                    "scientific validity or clinical safety",
                    "network delivery or receiver acceptance",
                    "automatic mission resumption",
                    "distributed ordering, consensus, or cross-instance state"
                ],
                "links": {
                    "events": "/v1/events",
                    "missions": "/v1/missions",
                    "recovery": "/v1/recovery",
                    "mission_persistence": "/v1/missions/persistence",
                    "event_persistence": "/v1/events/persistence",
                    "mission_provenance": "/v1/missions/{mission_id}/provenance",
                    "mission_claims": "/v1/missions/{mission_id}/claims",
                    "mission_evaluator_replay": "/v1/missions/{mission_id}/evaluator-replay",
                    "mission_evaluator_replay_compare": "/v1/missions/{mission_id}/evaluator-replay/compare",
                    "mission_evidence_bundle": "/v1/missions/{mission_id}/evidence-bundle",
                    "mission_evidence_bundle_verify": "/v1/evidence-bundles/verify",
                    "evidence_bundles": "/v1/evidence-bundles",
                    "evidence_bundle_persistence": "/v1/evidence-bundles/persistence",
                    "evidence_bundle_persistence_flush": "/v1/evidence-bundles/persistence/flush",
                    "workflow_reconciliations": "/v1/domain-workflows/reconciliations",
                    "workflow_reconciliation_persistence": "/v1/domain-workflows/reconciliations/persistence",
                    "workflow_reconciliation_persistence_flush": "/v1/domain-workflows/reconciliations/persistence/flush",
                    "ci_provider_evidence": "/v1/ci/provider-evidence",
                    "ci_provider_evidence_persistence": "/v1/ci/provider-evidence/persistence",
                    "ci_provider_evidence_persistence_flush": "/v1/ci/provider-evidence/persistence/flush",
                    "capabilities": "/v1/capabilities",
                    "delivery_attempts": "/v1/webhooks/subscriptions/{id}/attempts"
                }
            }),
        )
    }

    pub(super) fn operations_handoff(
        &self,
        request: &HttpRequest,
        request_id: &str,
    ) -> HttpResponse {
        let arguments = match self.json_object(request) {
            Ok(arguments) => arguments,
            Err(error) => return self.error(400, "invalid_json", &error, request_id),
        };
        match operations_handoff_value(&arguments) {
            Ok(value) => HttpResponse::json(200, &value),
            Err(error) => self.error(422, "invalid_operations_handoff", &error, request_id),
        }
    }

    pub(super) fn operations_domain_activity(
        &self,
        request: &HttpRequest,
        request_id: &str,
    ) -> HttpResponse {
        let query = match request.query() {
            Ok(query) => query,
            Err(error) => return self.error(400, "invalid_query", &error.to_string(), request_id),
        };
        for key in query.keys() {
            if key != "after" && key != "limit" {
                return self.error(
                    400,
                    "invalid_query",
                    "operations domain activity accepts only after and limit",
                    request_id,
                );
            }
        }
        let after = match query_u64(&query, "after", 0) {
            Ok(value) => value,
            Err(error) => return self.error(400, "invalid_query", &error, request_id),
        };
        let limit = match query_usize(&query, "limit", 100) {
            Ok(value) if (1..=MAX_OPERATIONS_SNAPSHOT_LIMIT).contains(&value) => value,
            Ok(_) => {
                return self.error(
                    422,
                    "invalid_query",
                    &format!("limit must be between 1 and {MAX_OPERATIONS_SNAPSHOT_LIMIT}"),
                    request_id,
                )
            }
            Err(error) => return self.error(400, "invalid_query", &error, request_id),
        };
        let page = match self.events.lock() {
            Ok(events) => match events.events(after, limit) {
                Ok(page) => page,
                Err(error) => return self.error(422, "invalid_query", &error, request_id),
            },
            Err(_) => {
                return self.error(
                    500,
                    "event_log_unavailable",
                    "event log is unavailable",
                    request_id,
                )
            }
        };
        let coverage = operations_domain_coverage();
        let groups = coverage
            .get("groups")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let capability_groups = bioprism_mcp::workspace_capabilities();
        let mut tools_by_group = BTreeMap::<String, BTreeSet<String>>::new();
        if let Some(capability_groups) = capability_groups.as_array() {
            for group in capability_groups {
                let Some(id) = group.get("id").and_then(Value::as_str) else {
                    continue;
                };
                let tools = group
                    .get("mcp_tools")
                    .and_then(Value::as_array)
                    .map(|values| {
                        values
                            .iter()
                            .filter_map(Value::as_str)
                            .map(str::to_owned)
                            .collect::<BTreeSet<_>>()
                    })
                    .unwrap_or_default();
                tools_by_group.insert(id.to_string(), tools);
            }
        }
        let advertised_tools = bioprism_mcp::tool_definitions()
            .into_iter()
            .filter_map(|tool| tool.get("name").and_then(Value::as_str).map(str::to_owned))
            .collect::<BTreeSet<_>>();
        let tool_name = |event: &crate::events::ApiEvent| -> Option<String> {
            event
                .payload
                .get("tool")
                .and_then(Value::as_str)
                .filter(|name| advertised_tools.contains(*name))
                .map(str::to_owned)
                .or_else(|| {
                    advertised_tools
                        .contains(&event.subject)
                        .then_some(event.subject.clone())
                })
        };
        let tool_events_scanned = page
            .events
            .iter()
            .filter(|event| tool_name(event).is_some())
            .count();
        let mut attributed_event_ids = BTreeSet::new();
        let mut groups_with_gaps = 0usize;
        let mut groups_with_observed_activity = 0usize;
        let mut catalogued_unobserved_tool_count = 0usize;
        let mut domain_rows = Vec::new();
        for group in groups {
            let id = group.get("id").and_then(Value::as_str).unwrap_or("unknown");
            let declared_tools = tools_by_group.get(id).cloned().unwrap_or_default();
            let advertised_group_tools = declared_tools
                .iter()
                .filter(|tool| advertised_tools.contains(*tool))
                .cloned()
                .collect::<BTreeSet<_>>();
            let mut observed_tools = BTreeSet::new();
            let mut observed_event_count = 0usize;
            let mut last_event_id = None;
            for event in &page.events {
                let Some(tool) = tool_name(event) else {
                    continue;
                };
                if !declared_tools.contains(&tool) {
                    continue;
                }
                observed_event_count += 1;
                observed_tools.insert(tool);
                last_event_id = Some(event.id);
                attributed_event_ids.insert(event.id);
            }
            let missing_tool_count = group
                .get("missing_tool_count")
                .and_then(Value::as_u64)
                .map(|value| usize::try_from(value).unwrap_or(usize::MAX))
                .unwrap_or(0);
            if missing_tool_count > 0 {
                groups_with_gaps += 1;
            }
            if observed_event_count > 0 {
                groups_with_observed_activity += 1;
            }
            let unobserved_tool_count = advertised_group_tools
                .len()
                .saturating_sub(observed_tools.intersection(&advertised_group_tools).count());
            catalogued_unobserved_tool_count += unobserved_tool_count;
            let activity_state = if missing_tool_count > 0 {
                "catalogue_gap"
            } else if observed_event_count > 0 {
                "observed_in_page"
            } else {
                "catalogued_unobserved_in_page"
            };
            let mut row = group;
            row["observed_event_count"] = json!(observed_event_count);
            row["observed_tool_count"] = json!(observed_tools.len());
            row["observed_tools"] = json!(observed_tools.into_iter().collect::<Vec<_>>());
            row["unobserved_advertised_tool_count"] = json!(unobserved_tool_count);
            row["last_event_id"] = json!(last_event_id);
            row["activity_state"] = json!(activity_state);
            row["observation_scope"] = json!("requested_event_page_only");
            domain_rows.push(row);
        }
        let unmatched_tool_events = tool_events_scanned.saturating_sub(attributed_event_ids.len());
        let event_cursor = json!({
            "after": page.after,
            "next_after": page.next_after,
            "oldest": page.oldest,
            "newest": page.newest,
            "gap": page.gap,
            "dropped_events": page.dropped_events,
            "returned_events": page.events.len()
        });
        HttpResponse::json(
            200,
            &json!({
                "ok": true,
                "workflow": "operations_domain_activity",
                "schema": "bioprism-operations-domain-activity/0.1",
                "event_cursor": event_cursor,
                "groups": domain_rows,
                "summary": {
                    "group_count": coverage.get("group_count").and_then(Value::as_u64).unwrap_or(0),
                    "returned_groups": coverage.get("returned_groups").and_then(Value::as_u64).unwrap_or(0),
                    "tool_events_scanned": tool_events_scanned,
                    "attributed_tool_events": attributed_event_ids.len(),
                    "unattributed_tool_events": unmatched_tool_events,
                    "groups_with_catalogue_gaps": groups_with_gaps,
                    "groups_with_observed_activity": groups_with_observed_activity,
                    "catalogued_unobserved_tool_count": catalogued_unobserved_tool_count
                },
                "observation_policy": {
                    "event_matching": "exact advertised tool name from event payload or subject",
                    "scope": "only the bounded event page requested by the caller",
                    "control_plane_evidence_scope": "completed evaluation, safety, and release tools are pooled across the page and applied to each matched domain group",
                    "cross_group_membership": "one tool event may contribute to multiple groups",
                    "readiness_claimed": false
                },
                "guarantees": [
                    "catalogue coverage and observed local activity remain separate fields",
                    "event cursor gaps and the observation window are explicit",
                    "no tool is invoked by this projection"
                ],
                "non_claims": [
                    "runtime health or successful execution for unobserved tools",
                    "scientific, clinical, safety, or release readiness",
                    "complete historical activity when retention gaps or a bounded page apply"
                ],
                "links": {
                    "operations_snapshot": "/v1/operations/snapshot",
                    "operations_domains": "/v1/operations/domains",
                    "operations_gates": "/v1/operations/gates",
                    "operations_gate_reviews": "/v1/operations/gate-reviews",
                    "operations_handoff": "/v1/operations/handoff",
                    "events": "/v1/events",
                    "capabilities": "/v1/capabilities"
                }
            }),
        )
    }

    pub(super) fn operations_domain_gates(
        &self,
        request: &HttpRequest,
        request_id: &str,
    ) -> HttpResponse {
        let query = match request.query() {
            Ok(query) => query,
            Err(error) => return self.error(400, "invalid_query", &error.to_string(), request_id),
        };
        for key in query.keys() {
            if key != "after" && key != "limit" {
                return self.error(
                    400,
                    "invalid_query",
                    "operations domain gates accepts only after and limit",
                    request_id,
                );
            }
        }
        let after = match query_u64(&query, "after", 0) {
            Ok(value) => value,
            Err(error) => return self.error(400, "invalid_query", &error, request_id),
        };
        let limit = match query_usize(&query, "limit", 100) {
            Ok(value) if (1..=MAX_OPERATIONS_SNAPSHOT_LIMIT).contains(&value) => value,
            Ok(_) => {
                return self.error(
                    422,
                    "invalid_query",
                    &format!("limit must be between 1 and {MAX_OPERATIONS_SNAPSHOT_LIMIT}"),
                    request_id,
                )
            }
            Err(error) => return self.error(400, "invalid_query", &error, request_id),
        };
        let page = match self.events.lock() {
            Ok(events) => match events.events(after, limit) {
                Ok(page) => page,
                Err(error) => return self.error(422, "invalid_query", &error, request_id),
            },
            Err(_) => {
                return self.error(
                    500,
                    "event_log_unavailable",
                    "event log is unavailable",
                    request_id,
                )
            }
        };
        let coverage = operations_domain_coverage();
        let groups = coverage
            .get("groups")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let reconciliation_registry = match self.reconciliation_registry.lock() {
            Ok(registry) => registry,
            Err(_) => {
                return self.error(
                    500,
                    "reconciliation_registry_unavailable",
                    "workflow reconciliation registry is unavailable",
                    request_id,
                )
            }
        };
        let artifact_registry = match self.artifact_registry.lock() {
            Ok(registry) => registry,
            Err(_) => {
                return self.error(
                    500,
                    "artifact_registry_unavailable",
                    "artifact registry is unavailable",
                    request_id,
                )
            }
        };
        let artifact_registry_generation = artifact_registry.generation();
        let artifact_registry_size = artifact_registry.len();
        let capability_groups = bioprism_mcp::workspace_capabilities();
        let mut tools_by_group = BTreeMap::<String, BTreeSet<String>>::new();
        if let Some(capability_groups) = capability_groups.as_array() {
            for group in capability_groups {
                let Some(id) = group.get("id").and_then(Value::as_str) else {
                    continue;
                };
                let tools = group
                    .get("mcp_tools")
                    .and_then(Value::as_array)
                    .map(|values| {
                        values
                            .iter()
                            .filter_map(Value::as_str)
                            .map(str::to_owned)
                            .collect::<BTreeSet<_>>()
                    })
                    .unwrap_or_default();
                tools_by_group.insert(id.to_string(), tools);
            }
        }
        let advertised_tools = bioprism_mcp::tool_definitions()
            .into_iter()
            .filter_map(|tool| tool.get("name").and_then(Value::as_str).map(str::to_owned))
            .collect::<BTreeSet<_>>();
        let tool_name = |event: &crate::events::ApiEvent| -> Option<String> {
            event
                .payload
                .get("tool")
                .and_then(Value::as_str)
                .filter(|name| advertised_tools.contains(*name))
                .map(str::to_owned)
                .or_else(|| {
                    advertised_tools
                        .contains(&event.subject)
                        .then_some(event.subject.clone())
                })
        };
        let tool_events = page
            .events
            .iter()
            .filter_map(|event| tool_name(event).map(|tool| (event, tool)))
            .collect::<Vec<_>>();
        let tool_events_scanned = tool_events.len();
        let completed_tool_events = tool_events
            .iter()
            .filter(|(event, _)| event.event_type == "tool.completed")
            .count();
        let refused_tool_events = tool_events
            .iter()
            .filter(|(event, _)| {
                matches!(event.event_type.as_str(), "tool.refused" | "tool.rpc_error")
            })
            .count();
        let mut global_channel_events = BTreeMap::<String, usize>::new();
        let mut global_channel_tools = BTreeMap::<String, BTreeSet<String>>::new();
        let mut evaluator_bindings_by_group = BTreeMap::<String, Vec<Value>>::new();
        for (event, tool) in &tool_events {
            if event.event_type != "tool.completed" {
                continue;
            }
            for channel in operations_evidence_channels(tool) {
                *global_channel_events
                    .entry((*channel).to_string())
                    .or_default() += 1;
                global_channel_tools
                    .entry((*channel).to_string())
                    .or_default()
                    .insert(tool.clone());
            }
            if operations_evidence_channels(tool).contains(&"evaluation") {
                for group_id in operations_evaluator_group_bindings(tool) {
                    evaluator_bindings_by_group
                        .entry((*group_id).to_string())
                        .or_default()
                        .push(json!({
                            "tool": tool,
                            "event_id": event.id,
                            "binding": "catalogue_group_alias"
                        }));
                }
            }
        }
        let mut attributed_event_ids = BTreeSet::new();
        let mut groups_blocked_catalogue = 0usize;
        let mut groups_insufficient_evidence = 0usize;
        let mut groups_review_required = 0usize;
        let mut groups_reconciliation_blocked = 0usize;
        let mut groups_with_artifact_evidence = 0usize;
        let mut artifact_evidence_records = 0usize;
        let mut rows = Vec::new();
        for group in groups {
            let id = group.get("id").and_then(Value::as_str).unwrap_or("unknown");
            let group_domains = group
                .get("domains")
                .and_then(Value::as_array)
                .map(|values| {
                    values
                        .iter()
                        .filter_map(Value::as_str)
                        .map(str::to_owned)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let artifact_evidence = artifact_registry.domain_evidence_posture(id, &group_domains);
            let matching_artifact_records = artifact_evidence
                .get("matching_record_count")
                .and_then(Value::as_u64)
                .map(|value| usize::try_from(value).unwrap_or(usize::MAX))
                .unwrap_or(0);
            if matching_artifact_records > 0 {
                groups_with_artifact_evidence += 1;
                artifact_evidence_records =
                    artifact_evidence_records.saturating_add(matching_artifact_records);
            }
            let reconciliation_posture = reconciliation_registry.workflow_posture(id);
            let reconciliation_state = reconciliation_posture
                .get("state")
                .and_then(Value::as_str)
                .unwrap_or("missing");
            let reconciliation_blocks = matches!(reconciliation_state, "incomplete" | "invalid");
            if reconciliation_blocks {
                groups_reconciliation_blocked += 1;
            }
            let declared_tools = tools_by_group.get(id).cloned().unwrap_or_default();
            let missing_tool_count = group
                .get("missing_tool_count")
                .and_then(Value::as_u64)
                .map(|value| usize::try_from(value).unwrap_or(usize::MAX))
                .unwrap_or(0);
            let mut observed_tools = BTreeSet::new();
            let mut completed_tools = BTreeSet::new();
            let mut refused_tools = BTreeSet::new();
            let mut observed_event_count = 0usize;
            let mut completed_event_count = 0usize;
            let mut refused_event_count = 0usize;
            let channel_tools = global_channel_tools.clone();
            let channel_events = global_channel_events.clone();
            let evaluator_bindings = evaluator_bindings_by_group
                .get(id)
                .cloned()
                .unwrap_or_default();
            let evaluator_tools = evaluator_bindings
                .iter()
                .filter_map(|binding| binding.get("tool").and_then(Value::as_str))
                .map(str::to_owned)
                .collect::<BTreeSet<_>>();
            let mut last_event_id = None;
            for (event, tool) in &tool_events {
                if !declared_tools.contains(tool) {
                    continue;
                }
                observed_event_count += 1;
                observed_tools.insert(tool.clone());
                last_event_id = Some(event.id);
                attributed_event_ids.insert(event.id);
                match event.event_type.as_str() {
                    "tool.completed" => {
                        completed_event_count += 1;
                        completed_tools.insert(tool.clone());
                    }
                    "tool.refused" | "tool.rpc_error" => {
                        refused_event_count += 1;
                        refused_tools.insert(tool.clone());
                    }
                    _ => {}
                }
            }
            let channel_gate = |name: &str| {
                let tools = channel_tools.get(name).cloned().unwrap_or_default();
                let events = channel_events.get(name).copied().unwrap_or(0);
                json!({
                    "state": if events > 0 { "observed" } else { "missing" },
                    "scope": "cross_domain_control_plane_event_page",
                    "event_count": events,
                    "tool_count": tools.len(),
                    "tools": tools.into_iter().collect::<Vec<_>>()
                })
            };
            let catalogue_state = if missing_tool_count == 0 {
                "pass"
            } else {
                "blocked"
            };
            let activity_state = if observed_event_count > 0 {
                "observed"
            } else {
                "missing"
            };
            let transport_state = if completed_event_count > 0 {
                "observed"
            } else if refused_event_count > 0 {
                "refused_or_failed"
            } else {
                "missing"
            };
            let domain_evaluator_state = if evaluator_bindings.is_empty() {
                "missing"
            } else {
                "observed"
            };
            let gate_state = if missing_tool_count > 0 {
                groups_blocked_catalogue += 1;
                "catalogue_blocked"
            } else if observed_event_count == 0
                || completed_event_count == 0
                || channel_events.get("evaluation").copied().unwrap_or(0) == 0
                || evaluator_bindings.is_empty()
                || channel_events.get("safety").copied().unwrap_or(0) == 0
                || channel_events.get("release").copied().unwrap_or(0) == 0
                || reconciliation_blocks
            {
                groups_insufficient_evidence += 1;
                "insufficient_evidence"
            } else {
                groups_review_required += 1;
                "review_required"
            };
            rows.push(json!({
                "id": id,
                "status": group.get("status").and_then(Value::as_str).unwrap_or("unknown"),
                "domains": group.get("domains").cloned().unwrap_or_else(|| json!([])),
                "declared_tool_count": group.get("declared_tool_count").cloned().unwrap_or_else(|| json!(0)),
                "advertised_tool_count": group.get("advertised_tool_count").cloned().unwrap_or_else(|| json!(0)),
                "missing_tool_count": missing_tool_count,
                "missing_tools": group.get("missing_tools").cloned().unwrap_or_else(|| json!([])),
                "gate_state": gate_state,
                "readiness_claimed": false,
                "gates": {
                    "catalogue": { "state": catalogue_state, "missing_tool_count": missing_tool_count },
                    "observed_activity": { "state": activity_state, "event_count": observed_event_count, "tool_count": observed_tools.len(), "tools": observed_tools },
                    "transport_completion": { "state": transport_state, "event_count": completed_event_count, "tool_count": completed_tools.len(), "tools": completed_tools, "refused_event_count": refused_event_count, "refused_tool_count": refused_tools.len(), "refused_tools": refused_tools },
                    "evaluation_evidence": channel_gate("evaluation"),
                    "domain_evaluator_evidence": {
                        "state": domain_evaluator_state,
                        "scope": "completed_evaluator_tool_exact_or_catalogue_group_binding",
                        "event_count": evaluator_bindings.len(),
                        "tool_count": evaluator_tools.len(),
                        "tools": evaluator_tools,
                        "bindings": evaluator_bindings,
                        "readiness_claimed": false
                    },
                    "safety_evidence": channel_gate("safety"),
                    "release_evidence": channel_gate("release"),
                    "reconciliation_evidence": reconciliation_posture,
                    "artifact_evidence": artifact_evidence
                },
                "last_event_id": last_event_id,
                "evidence_scope": "requested_event_page_only",
                "artifact_evidence_scope": "current_digest_verified_artifact_registry_exact_declared_matches"
            }));
        }
        let unmatched_tool_events = tool_events_scanned.saturating_sub(attributed_event_ids.len());
        let mut body = json!({
            "ok": true,
            "workflow": "operations_domain_gates",
            "schema": "bioprism-operations-domain-gates/0.1",
            "event_cursor": {
                "after": page.after,
                "next_after": page.next_after,
                "oldest": page.oldest,
                "newest": page.newest,
                "gap": page.gap,
                "dropped_events": page.dropped_events,
                "returned_events": page.events.len()
            },
            "artifact_evidence_scope": "current_digest_verified_artifact_registry_exact_declared_matches",
            "groups": rows,
            "summary": {
                "group_count": coverage.get("group_count").and_then(Value::as_u64).unwrap_or(0),
                "returned_groups": coverage.get("returned_groups").and_then(Value::as_u64).unwrap_or(0),
                "tool_events_scanned": tool_events_scanned,
                "attributed_tool_events": attributed_event_ids.len(),
                "unattributed_tool_events": unmatched_tool_events,
                "completed_tool_events": completed_tool_events,
                "refused_tool_events": refused_tool_events,
                "evaluation_evidence_events": global_channel_events.get("evaluation").copied().unwrap_or(0),
                "domain_evaluator_evidence_events": evaluator_bindings_by_group.values().map(Vec::len).sum::<usize>(),
                "safety_evidence_events": global_channel_events.get("safety").copied().unwrap_or(0),
                "release_evidence_events": global_channel_events.get("release").copied().unwrap_or(0),
                "groups_blocked_catalogue": groups_blocked_catalogue,
                "groups_insufficient_evidence": groups_insufficient_evidence,
                "groups_review_required": groups_review_required,
                "groups_reconciliation_blocked": groups_reconciliation_blocked,
                "groups_with_artifact_evidence": groups_with_artifact_evidence,
                "artifact_evidence_records": artifact_evidence_records,
                "artifact_registry_generation": artifact_registry_generation,
                "artifact_registry_size": artifact_registry_size,
                "readiness_claimed": false
            },
            "gate_policy": {
                "required_gates": operations_required_gates(),
                "optional_evidence_gates": ["artifact_evidence"],
                "decision_rule": "all gates need observed evidence; domain evaluator evidence must bind a completed evaluator tool to the selected capability group; a complete evidence set still requires human or domain authority review",
                "event_matching": "exact advertised tool name from event payload or subject",
                "scope": "only the bounded event page requested by the caller",
                "artifact_evidence_scope": "exact declared artifact registration domain intersection or explicit artifact.group_id for the selected capability group",
                "artifact_evidence_policy": "advisory; missing artifact evidence is visible and never promoted to readiness or used to pass a required gate",
                "control_plane_evidence_scope": "completed evaluation, safety, and release tools are pooled across the page and applied to each matched domain group",
                "domain_evaluator_binding_scope": "only completed evaluation-channel tools with an exact or catalogue-declared capability-group binding",
                "reconciliation_evidence_scope": "matching workflow_id rows from the bounded digest-valid reconciliation registry",
                "reconciliation_evidence_policy": "missing is explicit and does not pass; incomplete or invalid retained posture blocks review; structurally_ready remains review-required evidence",
                "cross_group_membership": "one tool event may contribute to multiple groups",
                "readiness_claimed": false
            },
            "guarantees": [
                "catalogue, activity, transport, pooled evaluation, domain evaluator, safety, and release evidence remain separate",
                "reconciliation evidence is joined by exact capability-group workflow_id and cannot be inferred from a different domain",
                "domain evaluator evidence is a bounded catalogue binding and does not assert scientific validity or evaluator adequacy",
                "missing evidence is represented as a gate state instead of inferred readiness",
                "artifact evidence is joined from the current digest-verified registry for every returned capability group",
                "no tool is invoked by this projection"
            ],
            "non_claims": [
                "scientific validity, clinical safety, or deployment authorization",
                "successful tool transport proves only a completed local call, not semantic correctness",
                "complete historical evidence when retention gaps or a bounded page apply"
            ],
            "links": {
                "operations_snapshot": "/v1/operations/snapshot",
                "operations_domains": "/v1/operations/domains",
                "operations_gates": "/v1/operations/gates",
                "operations_gate_reviews": "/v1/operations/gate-reviews",
                "operations_handoff": "/v1/operations/handoff",
                "events": "/v1/events",
                "capabilities": "/v1/capabilities"
            }
        });
        let mut digest_body = body.clone();
        if let Some(cursor) = digest_body
            .get_mut("event_cursor")
            .and_then(Value::as_object_mut)
        {
            cursor.insert(
                "oldest".into(),
                tool_events
                    .first()
                    .map(|(event, _)| json!(event.id))
                    .unwrap_or(Value::Null),
            );
            cursor.insert(
                "newest".into(),
                tool_events
                    .last()
                    .map(|(event, _)| json!(event.id))
                    .unwrap_or(Value::Null),
            );
            cursor.insert(
                "next_after".into(),
                tool_events
                    .last()
                    .map(|(event, _)| json!(event.id))
                    .unwrap_or_else(|| json!(page.after)),
            );
            cursor.insert("returned_events".into(), json!(tool_events_scanned));
        }
        let gate_digest = serde_json::to_vec(&digest_body)
            .map(|bytes| hex_digest(&Sha256::digest(&bytes)))
            .unwrap_or_default();
        body["gate_digest"] = json!(gate_digest);
        body["gate_digest_scope"] =
            json!("operations_evidence_and_reconciliation_projection_without_gate_digest");
        HttpResponse::json(200, &body)
    }

    pub(super) fn operations_gate_snapshot(&self) -> Value {
        let gate_request = HttpRequest {
            method: "GET".into(),
            target: "/v1/operations/gates?after=0&limit=256".into(),
            version: "HTTP/1.1".into(),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        response_value(self.operations_domain_gates(&gate_request, "internal-operations-gates"))
    }

    pub(super) fn operations_gate_reviews(
        &self,
        request: &HttpRequest,
        request_id: &str,
    ) -> HttpResponse {
        let query = match request.query() {
            Ok(query) => query,
            Err(error) => return self.error(400, "invalid_query", &error.to_string(), request_id),
        };
        for key in query.keys() {
            if !matches!(key.as_str(), "after" | "limit" | "review_id") {
                return self.error(
                    400,
                    "invalid_query",
                    "operations gate reviews accepts only after, limit, and review_id",
                    request_id,
                );
            }
        }
        let after = match query_u64(&query, "after", 0) {
            Ok(value) => value,
            Err(error) => return self.error(400, "invalid_query", &error, request_id),
        };
        let limit = match query_usize(&query, "limit", 100) {
            Ok(value) => value,
            Err(error) => return self.error(400, "invalid_query", &error, request_id),
        };
        let review_id = query.get("review_id").map(String::as_str);
        let events = match self.events.lock() {
            Ok(events) => events,
            Err(_) => {
                return self.error(
                    500,
                    "event_log_unavailable",
                    "event log is unavailable",
                    request_id,
                )
            }
        };
        let page = match events.operations_gate_reviews(after, limit, review_id) {
            Ok(page) => page,
            Err(error) => return self.error(400, "invalid_query", &error, request_id),
        };
        let reviews = page
            .events
            .iter()
            .filter_map(|event| {
                let payload = event.payload.as_object()?;
                let review_id = payload.get("review_id")?.as_str()?;
                Some(json!({
                    "review_id": review_id,
                    "event_id": event.id,
                    "request_id": event.request_id,
                    "acceptance": payload.get("acceptance").cloned().unwrap_or_else(|| json!({})),
                    "gate_digest": payload.get("gate_digest").cloned().unwrap_or(Value::Null),
                    "group_ids": payload.get("group_ids").cloned().unwrap_or_else(|| json!([])),
                    "evidence": payload.get("evidence").cloned().unwrap_or_else(|| json!([])),
                    "replay": format!("/v1/operations/gate-reviews?review_id={review_id}"),
                    "readiness_claimed": false
                }))
            })
            .collect::<Vec<_>>();
        HttpResponse::json(
            200,
            &json!({
                "ok": true,
                "workflow": "operations_gate_reviews",
                "schema": "bioprism-operations-gate-reviews/0.1",
                "review_id": review_id,
                "found": !reviews.is_empty(),
                "page": page,
                "reviews": reviews,
                "review_count": reviews.len(),
                "durability": {
                    "event_checkpoint_configured": self.config.event_state_path.is_some(),
                    "durable_across_restart": self.config.event_state_path.is_some(),
                    "persistence_endpoint": "/v1/events/persistence"
                },
                "readiness_claimed": false,
                "guarantees": [
                    "review records are read from the bounded retained event log",
                    "content-addressed review IDs and event cursors make the acceptance replayable",
                    "retention gaps remain visible instead of being presented as complete history",
                    "cross-restart durability is true only when the event checkpoint is configured"
                ],
                "non_claims": [
                    "a retained operator review is not scientific, clinical, regulatory, or deployment approval"
                ]
            }),
        )
    }

    pub(super) fn create_operations_gate_review(
        &self,
        request: &HttpRequest,
        request_id: &str,
    ) -> HttpResponse {
        let object = match self.json_object(request) {
            Ok(object) => object,
            Err(error) => return self.error(400, "invalid_json", &error, request_id),
        };
        let acceptance = Value::Object(object.clone());
        if let Err(error) = validate_operations_gate_acceptance_value(&acceptance, false) {
            return self.error(422, "invalid_operations_gate_review", &error, request_id);
        }
        let snapshot = self.operations_gate_snapshot();
        let group_ids = object
            .get("group_ids")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let rows = operations_gate_review_rows(&snapshot, &group_ids);
        if rows.len() != group_ids.len() {
            return self.error(
                422,
                "operations_gate_review_unresolved_group",
                "every reviewed group_id must exist in the current operations gate catalogue",
                request_id,
            );
        }
        let gate_digest = snapshot.get("gate_digest").cloned().unwrap_or(Value::Null);
        let match_arguments = json!({ "operations_gate_acceptance": acceptance.clone() });
        if !operations_gate_acceptance_matches(&match_arguments, &group_ids, &gate_digest, &rows) {
            return self.error(
                422,
                "operations_gate_review_not_ready",
                "a review requires a current gate digest and every selected group to be review_required with all required accepted gates",
                request_id,
            );
        }
        let canonical = operations_gate_acceptance_canonical(&acceptance)
            .ok_or_else(|| "operations gate review could not be canonicalized".to_string());
        let canonical = match canonical {
            Ok(value) => value,
            Err(error) => return self.error(500, "review_digest_failed", &error, request_id),
        };
        let canonical_bytes = match serde_json::to_vec(&canonical) {
            Ok(bytes) => bytes,
            Err(error) => {
                return self.error(
                    500,
                    "review_digest_failed",
                    &format!("operations gate review could not be digested: {error}"),
                    request_id,
                )
            }
        };
        let review_id = hex_digest(&Sha256::digest(&canonical_bytes));
        let mut recorded_acceptance = object;
        recorded_acceptance.insert("review_id".into(), json!(review_id));
        let payload = json!({
            "workflow": "operations_gate_review",
            "schema": "bioprism-operations-gate-review/0.1",
            "review_id": review_id,
            "acceptance": recorded_acceptance,
            "gate_digest": gate_digest,
            "group_ids": group_ids,
            "evidence": rows,
            "readiness_claimed": false
        });
        let event = {
            let mut events = match self.events.lock() {
                Ok(events) => events,
                Err(_) => {
                    return self.error(
                        500,
                        "event_log_unavailable",
                        "event log is unavailable",
                        request_id,
                    )
                }
            };
            match events.emit(
                "operations.gate_review.accepted",
                "operations_gate_review",
                request_id,
                payload.clone(),
            ) {
                Ok(event) => event,
                Err(error) => return self.error(500, "event_emit_failed", &error, request_id),
            }
        };
        if let Err(error) = self.event_persistence.persist() {
            return self.error(503, "event_persistence_unavailable", &error, request_id);
        }
        HttpResponse::json(
            201,
            &json!({
                "ok": true,
                "workflow": "operations_gate_review",
                "schema": "bioprism-operations-gate-review/0.1",
                "review_id": review_id,
                "event_id": event.id,
                "request_id": request_id,
                "acceptance": payload["acceptance"],
                "gate_digest": payload["gate_digest"],
                "group_ids": payload["group_ids"],
                "evidence": payload["evidence"],
                "replay": format!("/v1/operations/gate-reviews?review_id={review_id}"),
                "durability": {
                    "event_checkpoint_configured": self.config.event_state_path.is_some(),
                    "durable_across_restart": self.config.event_state_path.is_some(),
                    "persistence_endpoint": "/v1/events/persistence"
                },
                "readiness_claimed": false,
                "guarantees": [
                    "the acceptance is appended to the retained event log before the success response",
                    "the review ID is a content hash of the normalized acceptance",
                    "missions must present this retained review ID and matching acceptance before execution",
                    "cross-restart durability is true only when the event checkpoint is configured"
                ],
                "non_claims": [
                    "the review is not scientific, clinical, regulatory, or deployment approval"
                ]
            }),
        )
    }

    pub(super) fn operations_gate_review_matches(
        &self,
        arguments: &Value,
        gate_digest: &Value,
    ) -> Option<u64> {
        let acceptance = arguments
            .get("operations_gate_acceptance")
            .and_then(Value::as_object)?;
        let review_id = acceptance.get("review_id").and_then(Value::as_str)?;
        let current_fingerprint =
            operations_gate_acceptance_canonical(&Value::Object(acceptance.clone()))?;
        let events = match self.events.lock() {
            Ok(events) => events,
            Err(_) => return None,
        };
        let page = match events.events_for_operations_gate_review(0, 1000, review_id) {
            Ok(page) => page,
            Err(_) => return None,
        };
        page.events.iter().find_map(|event| {
            let matches = event.payload.get("review_id").and_then(Value::as_str) == Some(review_id)
                && event.payload.get("gate_digest") == Some(gate_digest)
                && event
                    .payload
                    .get("acceptance")
                    .and_then(operations_gate_acceptance_canonical)
                    .is_some_and(|fingerprint| fingerprint == current_fingerprint);
            matches.then_some(event.id)
        })
    }

    pub(super) fn operations_gate_projection(&self, arguments: &Value) -> Value {
        let requirements = mission_domain_group_requirements(arguments);
        let snapshot = self.operations_gate_snapshot();
        let group_ids = requirements
            .get("group_ids")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let mut rows = Vec::new();
        let mut all_review_required = !group_ids.is_empty();
        for group_id in &group_ids {
            let group_id = group_id.as_str().unwrap_or("unknown");
            let group = snapshot
                .get("groups")
                .and_then(Value::as_array)
                .and_then(|groups| {
                    groups
                        .iter()
                        .find(|group| group.get("id").and_then(Value::as_str) == Some(group_id))
                });
            let Some(group) = group else {
                all_review_required = false;
                rows.push(json!({
                    "group_id": group_id,
                    "gate_state": "unresolved_group",
                    "missing_gates": operations_required_gates(),
                    "readiness_claimed": false
                }));
                continue;
            };
            let gate_state = group
                .get("gate_state")
                .and_then(Value::as_str)
                .unwrap_or("insufficient_evidence");
            let gates = group.get("gates").and_then(Value::as_object);
            let missing_gates = operations_required_gates()
                .iter()
                .filter(|gate| {
                    let expected = if **gate == "catalogue" {
                        "pass"
                    } else {
                        "observed"
                    };
                    gates
                        .and_then(|gates| gates.get(**gate))
                        .and_then(|gate| gate.get("state"))
                        .and_then(Value::as_str)
                        != Some(expected)
                })
                .map(|gate| (*gate).to_string())
                .collect::<Vec<_>>();
            if gate_state != "review_required" || !missing_gates.is_empty() {
                all_review_required = false;
            }
            rows.push(json!({
                "group_id": group_id,
                "gate_state": gate_state,
                "missing_gates": missing_gates,
                "gates": group.get("gates").cloned().unwrap_or_else(|| json!({})),
                "last_event_id": group.get("last_event_id").cloned().unwrap_or(Value::Null),
                "readiness_claimed": false
            }));
        }
        let unresolved_steps = requirements
            .get("unresolved_steps")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let decision = if !unresolved_steps.is_empty()
            || rows
                .iter()
                .any(|row| row["gate_state"] == json!("unresolved_group"))
        {
            "unresolved_domain"
        } else if rows
            .iter()
            .any(|row| row["gate_state"] == json!("catalogue_blocked"))
        {
            "catalogue_blocked"
        } else if !all_review_required {
            "insufficient_evidence"
        } else {
            "review_required"
        };
        let gate_digest = snapshot.get("gate_digest").cloned().unwrap_or(Value::Null);
        let acceptance_matches =
            operations_gate_acceptance_matches(arguments, &group_ids, &gate_digest, &rows);
        let review_event_id = self.operations_gate_review_matches(arguments, &gate_digest);
        let review_present = review_event_id.is_some();
        let acceptance_valid = all_review_required && acceptance_matches && review_present;
        let review_id = arguments
            .pointer("/operations_gate_acceptance/review_id")
            .cloned()
            .unwrap_or(Value::Null);
        json!({
            "schema": "bioprism-operations-preflight-evidence/0.1",
            "gate_digest": gate_digest,
            "gate_digest_scope": "operations_evidence_and_reconciliation_projection_without_gate_digest",
            "group_ids": group_ids,
            "groups": rows,
            "unresolved_steps": unresolved_steps,
            "required_gates": operations_required_gates(),
            "decision": decision,
            "acceptance_required": mission_execution_requested(arguments),
            "acceptance_present": arguments.get("operations_gate_acceptance").is_some(),
            "review_id": review_id,
            "review_event_id": review_event_id,
            "review_present": review_present,
            "acceptance_matches_current_gates": acceptance_matches,
            "acceptance_valid": acceptance_valid,
            "dispatch_prerequisite": if acceptance_valid { "satisfied" } else { "acceptance_required" },
            "gate_endpoint": "/v1/operations/gates?after=0&limit=256",
            "review_endpoint": "/v1/operations/gate-reviews",
            "readiness_claimed": false,
            "guarantees": [
                "mission steps are mapped to every matching workspace capability group by exact tool name",
                "all seven evidence gates remain separate from the mission execution policy",
                "execution acceptance is bound to the current gate digest, selected group set, and retained review event"
            ],
            "non_claims": [
                "scientific validity, clinical safety, or deployment authorization",
                "an operator acceptance is not a domain-authority or regulatory approval",
                "complete historical evidence beyond the bounded retained event page"
            ]
        })
    }

    pub(super) fn operations_mission_summary(&self) -> Result<Value, &'static str> {
        let jobs = self
            .mission_jobs
            .lock()
            .map_err(|_| "mission_registry_unavailable")?;
        let mut status_counts = BTreeMap::<String, usize>::new();
        let mut recovered_after_restart = 0usize;
        let mut cancel_requested = 0usize;
        for job in jobs.values() {
            let state = job_state(job).map_err(|_| "mission_state_unavailable")?;
            *status_counts.entry(state.status).or_default() += 1;
            if state.recovered_after_restart {
                recovered_after_restart += 1;
            }
            if state.cancel_requested {
                cancel_requested += 1;
            }
        }
        Ok(json!({
            "total": jobs.len(),
            "status_counts": status_counts,
            "recovered_after_restart": recovered_after_restart,
            "cancel_requested": cancel_requested,
            "registry_capacity": MAX_MISSION_JOBS
        }))
    }
}
