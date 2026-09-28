//! Mission submission, execution, inspection, and evidence routes.
//!
//! Keeping this lifecycle together makes its dispatch, recovery, and evidence boundaries
//! reviewable without mixing them into transport and registry routing.

use super::operations::{
    mission_execution_provenance, mission_execution_requested, validate_operations_gate_acceptance,
};
use super::*;

impl ApiRouter {
    pub(super) fn preflight_mission(
        &self,
        request: &HttpRequest,
        request_id: &str,
    ) -> HttpResponse {
        let arguments = match self.json_object(request) {
            Ok(arguments) => arguments,
            Err(error) => return self.error(400, "invalid_json", &error, request_id),
        };
        let raw_arguments = Value::Object(arguments.clone());
        if let Err(error) = validate_operations_gate_acceptance(&raw_arguments) {
            return self.error(
                422,
                "invalid_operations_gate_acceptance",
                &error,
                request_id,
            );
        }
        let evidence = self.operations_gate_projection(&raw_arguments);
        let mut report = match self
            .mission_executor
            .preflight_agent_mission(&raw_arguments)
        {
            Ok(report) => report,
            Err(error) => return self.error(422, "invalid_mission", &error, request_id),
        };
        report["request_id"] = json!(request_id);
        report["operations_evidence"] = evidence;
        HttpResponse::json(200, &report)
    }

    pub(super) fn submit_mission(&self, request: &HttpRequest, request_id: &str) -> HttpResponse {
        let arguments = match self.json_object(request) {
            Ok(arguments) => arguments,
            Err(error) => return self.error(400, "invalid_json", &error, request_id),
        };
        let mission_id = match arguments.get("mission_id").and_then(Value::as_str) {
            Some(value) if !value.trim().is_empty() && value.len() <= 256 => value.to_string(),
            _ => {
                return self.error(
                    422,
                    "invalid_mission",
                    "mission_id must be a non-empty string of at most 256 bytes",
                    request_id,
                )
            }
        };
        let arguments = Value::Object(arguments);
        if let Err(error) = validate_operations_gate_acceptance(&arguments) {
            return self.error(
                422,
                "invalid_operations_gate_acceptance",
                &error,
                request_id,
            );
        }
        if let Err(error) = self.mission_executor.validate_agent_mission(&arguments) {
            return self.error(422, "invalid_mission", &error, request_id);
        }
        let mut execution_provenance = None;
        if mission_execution_requested(&arguments) {
            let evidence = self.operations_gate_projection(&arguments);
            if evidence["acceptance_valid"] != json!(true)
                || evidence["decision"] != json!("review_required")
            {
                return self.error(
                    422,
                    "operations_gate_acceptance_required",
                    "execution requires a current operations_gate_acceptance covering every selected domain evidence gate",
                    request_id,
                );
            }
            let Some(provenance) = mission_execution_provenance(&mission_id, &arguments, &evidence)
            else {
                return self.error(
                    500,
                    "execution_provenance_unavailable",
                    "the validated execution acceptance could not be converted into bounded provenance",
                    request_id,
                );
            };
            execution_provenance = Some(provenance);
        }
        let total_steps = arguments
            .get("steps")
            .and_then(Value::as_array)
            .map_or(0, Vec::len);

        let mission_already_exists = match self.mission_jobs.lock() {
            Ok(jobs) => jobs.contains_key(&mission_id),
            Err(_) => {
                return self.error(
                    500,
                    "mission_registry_unavailable",
                    "mission job registry is unavailable",
                    request_id,
                )
            }
        };
        if mission_already_exists {
            return self.error(
                409,
                "mission_exists",
                "a mission with this mission_id already exists",
                request_id,
            );
        }

        let queue_now = match current_timestamp() {
            Ok(now) => now,
            Err(error) => {
                return self.error(500, "mission_queue_clock_unavailable", &error, request_id)
            }
        };
        let queue_idempotency = if mission_execution_requested(&arguments) {
            FactoryIdempotency::NonIdempotent
        } else {
            FactoryIdempotency::Idempotent
        };
        let queue_job = FactoryJob::new(
            mission_id.clone(),
            ResourceClass::Evaluate,
            queue_idempotency,
            arguments.clone(),
        )
        .with_priority(8);
        let queue_lease = match self
            .mission_queue_persistence
            .enqueue_and_lease(queue_job, queue_now)
        {
            Ok(lease) => lease,
            Err(error) if error.contains("duplicate work") || error.contains("already present") => {
                return self.error(409, "mission_duplicate_work", &error, request_id)
            }
            Err(error) if error.contains("admission limit") => {
                return self.error(429, "mission_queue_backpressure", &error, request_id)
            }
            Err(error) => return self.error(503, "mission_queue_unavailable", &error, request_id),
        };

        let queue_attempt = queue_lease.attempt;
        let route_review_provenance = mission_route_review_provenance(&arguments);
        let cancellation = Arc::new(AtomicBool::new(false));
        let state = Arc::new(Mutex::new(MissionJobState {
            total_steps,
            trace: Vec::new(),
            progress: MissionProgressState::new(total_steps),
            status: "queued".into(),
            cancel_requested: false,
            cancel_reason: None,
            result: None,
            result_omitted: None,
            evaluator_replay_summary: None,
            route_review_provenance: route_review_provenance.clone(),
            error: None,
            recovered_after_restart: false,
            execution_provenance: execution_provenance.clone(),
        }));
        let job = Arc::new(MissionJob {
            cancellation: Arc::clone(&cancellation),
            state: Arc::clone(&state),
        });
        {
            let mut jobs = match self.mission_jobs.lock() {
                Ok(jobs) => jobs,
                Err(_) => {
                    return self.error(
                        500,
                        "mission_registry_unavailable",
                        "mission job registry is unavailable",
                        request_id,
                    )
                }
            };
            if jobs.contains_key(&mission_id) {
                let _ = self
                    .mission_queue_persistence
                    .cancel(&mission_id, "mission id already exists");
                return self.error(
                    409,
                    "mission_exists",
                    "a mission with this mission_id already exists",
                    request_id,
                );
            }
            if jobs.len() >= MAX_MISSION_JOBS {
                let _ = self
                    .mission_queue_persistence
                    .cancel(&mission_id, "mission registry capacity exhausted");
                return self.error(
                    429,
                    "mission_capacity_exhausted",
                    "the in-memory mission registry has reached its safety bound",
                    request_id,
                );
            }
            jobs.insert(mission_id.clone(), Arc::clone(&job));
        }
        if let Err(error) = self.persist_mission_registry() {
            if let Ok(mut jobs) = self.mission_jobs.lock() {
                jobs.remove(&mission_id);
            }
            let _ = self
                .mission_queue_persistence
                .cancel(&mission_id, "mission checkpoint unavailable");
            return self.error(503, "mission_persistence_unavailable", &error, request_id);
        }

        if let Some(provenance) = execution_provenance.as_mut() {
            let event = {
                let mut events = match self.events.lock() {
                    Ok(events) => events,
                    Err(_) => {
                        if let Ok(mut jobs) = self.mission_jobs.lock() {
                            jobs.remove(&mission_id);
                        }
                        let _ = self.persist_mission_registry();
                        let _ = self
                            .mission_queue_persistence
                            .cancel(&mission_id, "event log unavailable");
                        return self.error(
                            500,
                            "event_log_unavailable",
                            "event log is unavailable",
                            request_id,
                        );
                    }
                };
                match events.emit(
                    "mission.execution.accepted",
                    &mission_id,
                    request_id,
                    json!({
                        "workflow": "mission_execution",
                        "schema": "bioprism-mission-execution-provenance/0.1",
                        "mission_id": mission_id,
                        "provenance": provenance,
                        "readiness_claimed": false
                    }),
                ) {
                    Ok(event) => event,
                    Err(error) => {
                        if let Ok(mut jobs) = self.mission_jobs.lock() {
                            jobs.remove(&mission_id);
                        }
                        let _ = self.persist_mission_registry();
                        let _ = self
                            .mission_queue_persistence
                            .cancel(&mission_id, "mission acceptance event failed");
                        return self.error(500, "event_emit_failed", &error, request_id);
                    }
                }
            };
            if let Err(error) = self.event_persistence.persist() {
                if let Ok(mut jobs) = self.mission_jobs.lock() {
                    jobs.remove(&mission_id);
                }
                let _ = self.persist_mission_registry();
                let _ = self
                    .mission_queue_persistence
                    .cancel(&mission_id, "event checkpoint unavailable");
                return self.error(503, "event_persistence_unavailable", &error, request_id);
            }
            provenance["accepted_event_id"] = json!(event.id);
            if let Ok(mut current) = state.lock() {
                current.execution_provenance = Some(provenance.clone());
            }
            if let Err(error) = self.persist_mission_registry() {
                if let Ok(mut jobs) = self.mission_jobs.lock() {
                    jobs.remove(&mission_id);
                }
                let _ = self.persist_mission_registry();
                let _ = self
                    .mission_queue_persistence
                    .cancel(&mission_id, "mission checkpoint unavailable");
                return self.error(503, "mission_persistence_unavailable", &error, request_id);
            }
        }

        let progress_state = Arc::clone(&state);
        let mission_events = Arc::clone(&self.events);
        let persistence = Arc::clone(&self.mission_persistence);
        let event_persistence = Arc::clone(&self.event_persistence);
        let mission_queue_persistence = Arc::clone(&self.mission_queue_persistence);
        let mission_subject = mission_id.clone();
        let mission_request_id = request_id.to_string();
        let observer = Arc::new(move |event: Value| {
            if let Ok(mut current) = progress_state.lock() {
                current.record_trace(event.clone());
            }
            if let Ok(now) = current_timestamp() {
                let _ = mission_queue_persistence.heartbeat(&mission_subject, queue_attempt, now);
            }
            let _ = persistence.persist();
            if let Ok(mut events) = mission_events.lock() {
                let _ = events.emit(
                    "mission.trace",
                    &mission_subject,
                    &mission_request_id,
                    json!({ "mission_id": mission_subject.clone(), "trace": event }),
                );
            }
            let _ = event_persistence.persist();
        });
        let executor = Arc::new(self.mission_executor.with_mission_trace_observer(observer));
        let worker_persistence = Arc::clone(&self.mission_persistence);
        let worker_reconciliation_persistence = Arc::clone(&self.reconciliation_persistence);
        let worker_artifact_persistence = Arc::clone(&self.artifact_persistence);
        let worker_workflow_execution_evidence_persistence =
            Arc::clone(&self.workflow_execution_evidence_persistence);
        let worker_queue_persistence = Arc::clone(&self.mission_queue_persistence);
        let worker_id = mission_id.clone();
        let worker_mission_id = mission_id.clone();
        let worker_arguments = arguments;
        let spawn = thread::Builder::new()
            .name(format!("mission-{worker_id}"))
            .spawn(move || {
                if let Ok(mut current) = state.lock() {
                    current.status = "running".into();
                    current.progress.phase = "running".into();
                }
                let _ = worker_persistence.persist();
                let outcome = executor
                    .execute_agent_mission_with_cancellation(&worker_arguments, &cancellation);
                // The MCP executor has already imported any workflow reconciliation. Checkpoint
                // it before publishing the terminal mission state to the in-memory job registry.
                let _ = worker_reconciliation_persistence.persist();
                let _ = worker_artifact_persistence.persist();
                let _ = worker_workflow_execution_evidence_persistence.persist();
                if let Ok(mut current) = job.state.lock() {
                    match outcome {
                        Ok(result) => {
                            let queue_commit = current_timestamp().and_then(|now| {
                                worker_queue_persistence
                                    .commit_success(
                                        &worker_mission_id,
                                        queue_attempt,
                                        result.clone(),
                                        now,
                                    )
                            });
                            if let Err(queue_error) = queue_commit {
                                current.status = "failed".into();
                                current.progress.phase = "failed".into();
                                current.progress.active_steps = 0;
                                current.error = Some(format!(
                                    "mission produced a report but the durable execution lease could not commit: {queue_error}"
                                ));
                                current.result = Some(result);
                                current.result_omitted = None;
                            } else {
                                current.progress.reconcile(&result);
                                current.status = result
                                    .get("mission_status")
                                    .and_then(Value::as_str)
                                    .unwrap_or("succeeded")
                                    .into();
                                current.evaluator_replay_summary =
                                    evaluator_replay_summary(&result, &worker_mission_id);
                                current.result = Some(result);
                                current.result_omitted = None;
                            }
                        }
                        Err(error) => {
                            let queue_error = current_timestamp()
                                .and_then(|now| {
                                    worker_queue_persistence.record_failure(
                                        &worker_mission_id,
                                        queue_attempt,
                                        error.clone(),
                                        now,
                                    )
                                })
                                .err();
                            current.status = "failed".into();
                            current.progress.phase = "failed".into();
                            current.progress.active_steps = 0;
                            current.error = Some(match queue_error {
                                Some(queue_error) => format!("{error}; queue transition failed: {queue_error}"),
                                None => error,
                            });
                        }
                    }
                }
                let _ = worker_persistence.persist();
            });
        if spawn.is_err() {
            if let Ok(mut jobs) = self.mission_jobs.lock() {
                jobs.remove(&mission_id);
            }
            let _ = self.persist_mission_registry();
            let _ = self
                .mission_queue_persistence
                .cancel(&mission_id, "mission worker could not be started");
            return self.error(
                503,
                "mission_worker_unavailable",
                "the mission worker could not be started",
                request_id,
            );
        }

        HttpResponse::json(
            202,
            &json!({
                "ok": true,
                "mission_id": mission_id,
                "status": "queued",
                "queue": {
                    "state": "leased",
                    "attempt": queue_lease.attempt,
                    "worker_id": queue_lease.worker_id,
                    "expires_at": queue_lease.expires_at,
                    "automatic_resume": false
                },
                "cancel_requested": false,
                "progress": mission_progress_json(&MissionProgressState::new(total_steps)),
                "route_review_provenance": route_review_provenance,
                "execution_provenance": execution_provenance,
                "poll": format!("/v1/missions/{mission_id}"),
                "cancel": format!("/v1/missions/{mission_id}/cancel"),
                "trace": format!("/v1/missions/{mission_id}/trace"),
                "guarantees": [
                    "mission validation completed before acceptance",
                    "execution is cooperative and preserves the authoritative mission report",
                    "in-flight nested tool calls are allowed to return before future dispatch stops",
                ],
            }),
        )
    }

    pub(super) fn mission_inventory(
        &self,
        request: &HttpRequest,
        request_id: &str,
    ) -> HttpResponse {
        let query = match request.query() {
            Ok(query) => query,
            Err(error) => return self.error(400, "invalid_query", &error.to_string(), request_id),
        };
        for key in query.keys() {
            if key != "limit" && key != "status" {
                return self.error(
                    400,
                    "invalid_query",
                    "mission inventory accepts only limit and status",
                    request_id,
                );
            }
        }
        let limit = match query.get("limit") {
            None => 100,
            Some(value) => match value.parse::<usize>() {
                Ok(value) if (1..=MAX_MISSION_LIST_LIMIT).contains(&value) => value,
                _ => {
                    return self.error(
                        422,
                        "invalid_query",
                        &format!("limit must be between 1 and {MAX_MISSION_LIST_LIMIT}"),
                        request_id,
                    )
                }
            },
        };
        let status_filter = match query.get("status") {
            None => None,
            Some(status) if is_known_mission_status(status) => Some(status.as_str()),
            Some(_) => {
                return self.error(
                    422,
                    "invalid_query",
                    "status is not a recognized mission status",
                    request_id,
                )
            }
        };
        let jobs = match self.mission_jobs.lock() {
            Ok(jobs) => jobs,
            Err(_) => {
                return self.error(
                    500,
                    "mission_registry_unavailable",
                    "mission job registry is unavailable",
                    request_id,
                )
            }
        };
        let mut entries = Vec::new();
        for (mission_id, job) in jobs.iter() {
            let state = match job_state(job) {
                Ok(state) => state,
                Err(_) => {
                    return self.error(
                        500,
                        "mission_state_unavailable",
                        "mission state is unavailable",
                        request_id,
                    )
                }
            };
            if status_filter.is_some_and(|status| status != state.status) {
                continue;
            }
            let queue = match self.mission_queue_persistence.projection(mission_id) {
                Ok(queue) => queue,
                Err(_) => {
                    return self.error(
                        500,
                        "mission_queue_unavailable",
                        "mission queue is unavailable",
                        request_id,
                    )
                }
            };
            entries.push(json!({
                "mission_id": mission_id,
                "status": state.status,
                "cancel_requested": state.cancel_requested,
                "cancel_reason": state.cancel_reason,
                "recovered_after_restart": state.recovered_after_restart,
                "route_review_provenance": state.route_review_provenance,
                "execution_provenance": state.execution_provenance,
                "queue": queue,
                "progress": mission_progress_json(&state.progress),
                "summary": mission_summary(&state),
                "poll": format!("/v1/missions/{mission_id}"),
                "cancel": format!("/v1/missions/{mission_id}/cancel"),
                "trace": format!("/v1/missions/{mission_id}/trace"),
            }));
        }
        let total = entries.len();
        let missions = entries.into_iter().take(limit).collect::<Vec<_>>();
        HttpResponse::json(
            200,
            &json!({
                "ok": true,
                "missions": missions,
                "returned": total.min(limit),
                "total_matching": total,
                "limit": limit,
                "truncated": total > limit,
                "status_filter": status_filter,
                "guarantees": [
                    "inventory order is deterministic by mission_id",
                    "inventory entries expose summaries and links, not unbounded terminal reports",
                    "status filters are evaluated against the process-local authoritative registry"
                ]
            }),
        )
    }

    pub(super) fn mission_status(&self, request: &HttpRequest, request_id: &str) -> HttpResponse {
        let Some(mission_id) = mission_id(&request.path_segments(), None) else {
            return self.error(404, "not_found", "mission route does not exist", request_id);
        };
        let job = match self.mission_jobs.lock() {
            Ok(jobs) => jobs.get(&mission_id).cloned(),
            Err(_) => {
                return self.error(
                    500,
                    "mission_registry_unavailable",
                    "mission job registry is unavailable",
                    request_id,
                )
            }
        };
        let Some(job) = job else {
            return self.error(404, "not_found", "mission does not exist", request_id);
        };
        let current = match job_state(&job) {
            Ok(state) => state,
            Err(_) => {
                return self.error(
                    500,
                    "mission_state_unavailable",
                    "mission state is unavailable",
                    request_id,
                )
            }
        };
        let queue = match self.mission_queue_persistence.projection(&mission_id) {
            Ok(queue) => queue,
            Err(error) => return self.error(500, "mission_queue_unavailable", &error, request_id),
        };
        HttpResponse::json(
            200,
            &json!({
                "ok": true,
                "mission_id": mission_id,
                "status": current.status,
                "cancel_requested": current.cancel_requested,
                "cancel_reason": current.cancel_reason,
                "recovered_after_restart": current.recovered_after_restart,
                "execution_provenance": current.execution_provenance,
                "queue": queue,
                "progress": mission_progress_json(&current.progress),
                "route_review_provenance": current.route_review_provenance,
                "result": current.result,
                "result_omitted": current.result_omitted,
                "error": current.error,
                "poll": format!("/v1/missions/{mission_id}"),
                "cancel": format!("/v1/missions/{mission_id}/cancel"),
                "trace": format!("/v1/missions/{mission_id}/trace"),
                "evaluator_replay": format!("/v1/missions/{mission_id}/evaluator-replay"),
                "evaluator_replay_compare": format!("/v1/missions/{mission_id}/evaluator-replay/compare"),
                "evidence_bundle": format!("/v1/missions/{mission_id}/evidence-bundle"),
            }),
        )
    }

    pub(super) fn mission_provenance(
        &self,
        request: &HttpRequest,
        request_id: &str,
    ) -> HttpResponse {
        let Some(mission_id) = mission_id(&request.path_segments(), Some("provenance")) else {
            return self.error(
                404,
                "not_found",
                "mission provenance route does not exist",
                request_id,
            );
        };
        if request
            .query()
            .map(|query| !query.is_empty())
            .unwrap_or(true)
        {
            return self.error(
                400,
                "invalid_query",
                "mission provenance does not accept query parameters",
                request_id,
            );
        }
        let job = match self.mission_jobs.lock() {
            Ok(jobs) => jobs.get(&mission_id).cloned(),
            Err(_) => {
                return self.error(
                    500,
                    "mission_registry_unavailable",
                    "mission job registry is unavailable",
                    request_id,
                )
            }
        };
        let Some(job) = job else {
            return self.error(404, "not_found", "mission does not exist", request_id);
        };
        let state = match job_state(&job) {
            Ok(state) => state,
            Err(_) => {
                return self.error(
                    500,
                    "mission_state_unavailable",
                    "mission state is unavailable",
                    request_id,
                )
            }
        };
        let Some(provenance) = state.execution_provenance else {
            return self.error(
                404,
                "provenance_unavailable",
                "mission has no execution provenance because it was preview-only or predates this contract",
                request_id,
            );
        };
        HttpResponse::json(
            200,
            &json!({
                "ok": true,
                "schema": "bioprism-mission-execution-provenance/0.1",
                "mission_id": mission_id,
                "provenance": provenance,
                "readiness_claimed": false,
                "guarantees": [
                    "the projection is retained with the mission checkpoint when mission_state_path is configured",
                    "review, gate digest, evaluator binding, and accepted-dispatch event identifiers remain correlated",
                    "the underlying gate review and event routes remain authoritative for replay"
                ],
                "non_claims": [
                    "preview-only missions have no dispatch provenance",
                    "provenance is not scientific, clinical, regulatory, or deployment approval"
                ],
                "links": {
                    "mission": format!("/v1/missions/{mission_id}"),
                    "mission_trace": format!("/v1/missions/{mission_id}/trace"),
                    "operations_gates": "/v1/operations/gates?after=0&limit=256",
                    "events": "/v1/events?after=0&limit=256"
                }
            }),
        )
    }

    pub(super) fn mission_evaluator_replay_compare(
        &self,
        request: &HttpRequest,
        request_id: &str,
    ) -> HttpResponse {
        let Some(mission_id) =
            mission_id_nested(&request.path_segments(), "evaluator-replay", "compare")
        else {
            return self.error(
                404,
                "not_found",
                "mission evaluator replay comparison route does not exist",
                request_id,
            );
        };
        let query = match request.query() {
            Ok(query) => query,
            Err(error) => return self.error(400, "invalid_query", &error.to_string(), request_id),
        };
        for key in query.keys() {
            if key != "include_fixtures" && key != "max_items" {
                return self.error(
                    400,
                    "invalid_query",
                    "mission evaluator replay comparison accepts only include_fixtures and max_items",
                    request_id,
                );
            }
        }
        let include_fixtures = match query_bool(&query, "include_fixtures", false) {
            Ok(value) => value,
            Err(error) => return self.error(422, "invalid_query", &error, request_id),
        };
        let max_items = match query_usize(&query, "max_items", 128) {
            Ok(value) if (1..=512).contains(&value) => value,
            Ok(_) | Err(_) => {
                return self.error(
                    422,
                    "invalid_query",
                    "max_items must be between 1 and 512",
                    request_id,
                )
            }
        };
        let job = match self.mission_jobs.lock() {
            Ok(jobs) => jobs.get(&mission_id).cloned(),
            Err(_) => {
                return self.error(
                    500,
                    "mission_registry_unavailable",
                    "mission job registry is unavailable",
                    request_id,
                )
            }
        };
        let Some(job) = job else {
            return self.error(404, "not_found", "mission does not exist", request_id);
        };
        let state = match job_state(&job) {
            Ok(state) => state,
            Err(_) => {
                return self.error(
                    500,
                    "mission_state_unavailable",
                    "mission state is unavailable",
                    request_id,
                )
            }
        };
        let query_value = json!({
            "include_fixtures": include_fixtures,
            "max_items": max_items
        });
        let catalogue = MissionEvaluatorCatalogue::standard();
        let comparison = if let Some(result) = state.result.clone() {
            catalogue.compare(&MissionEvaluatorReplayCompareRequest {
                mission: result,
                include_fixtures,
                max_items,
            })
        } else if let Some(summary) = state.evaluator_replay_summary.clone() {
            catalogue.compare_summary(&summary)
        } else if let Some(omitted) = state.result_omitted.clone() {
            return self.error(
                410,
                "mission_evaluator_replay_omitted",
                &format!(
                    "mission result and evaluator replay summary were omitted from the bounded registry snapshot ({} bytes, sha256 {})",
                    omitted["bytes"], omitted["sha256"]
                ),
                request_id,
            );
        } else {
            return self.error(
                409,
                "evaluator_replay_unavailable",
                "mission evaluator replay comparison is available after a terminal mission report is retained",
                request_id,
            );
        };
        let comparison = match comparison {
            Ok(comparison) => comparison,
            Err(error) => {
                return self.error(
                    422,
                    "evaluator_replay_comparison_invalid",
                    &error.to_string(),
                    request_id,
                )
            }
        };
        let result_retained = state.result.is_some();
        HttpResponse::json(
            200,
            &json!({
                "ok": true,
                "schema": "bioprism-api/mission-evaluator-replay-compare/0.1",
                "workflow": "mission_evaluator_replay_compare",
                "mission_id": mission_id,
                "query": query_value,
                "retention": {
                    "mode": if result_retained { "full" } else { "summary_only" },
                    "result_retained": result_retained,
                    "summary_retained": state.evaluator_replay_summary.is_some(),
                    "result_omitted": state.result_omitted.clone()
                },
                "replay": comparison["replay"].clone(),
                "catalog_drift": comparison["catalog_drift"].clone(),
                "execution": "not_started",
                "guarantees": comparison["guarantees"].clone(),
                "limitations": comparison["limitations"].clone(),
                "links": {
                    "mission": format!("/v1/missions/{mission_id}"),
                    "replay": format!("/v1/missions/{mission_id}/evaluator-replay"),
                    "compare": format!("/v1/missions/{mission_id}/evaluator-replay/compare"),
                    "evidence_bundle": format!("/v1/missions/{mission_id}/evidence-bundle")
                }
            }),
        )
    }

    pub(super) fn mission_evidence_bundle(
        &self,
        request: &HttpRequest,
        request_id: &str,
    ) -> HttpResponse {
        let Some(mission_id) = mission_id(&request.path_segments(), Some("evidence-bundle")) else {
            return self.error(
                404,
                "not_found",
                "mission evidence bundle route does not exist",
                request_id,
            );
        };
        let query = match request.query() {
            Ok(query) => query,
            Err(error) => return self.error(400, "invalid_query", &error.to_string(), request_id),
        };
        for key in query.keys() {
            if !matches!(
                key.as_str(),
                "include_result" | "include_trace" | "include_fixtures" | "max_items"
            ) {
                return self.error(
                    400,
                    "invalid_query",
                    "mission evidence bundle accepts only include_result, include_trace, include_fixtures, and max_items",
                    request_id,
                );
            }
        }
        let include_result = match query_bool(&query, "include_result", false) {
            Ok(value) => value,
            Err(error) => return self.error(422, "invalid_query", &error, request_id),
        };
        let include_trace = match query_bool(&query, "include_trace", true) {
            Ok(value) => value,
            Err(error) => return self.error(422, "invalid_query", &error, request_id),
        };
        let include_fixtures = match query_bool(&query, "include_fixtures", false) {
            Ok(value) => value,
            Err(error) => return self.error(422, "invalid_query", &error, request_id),
        };
        let max_items = match query_usize(&query, "max_items", 128) {
            Ok(value) if (1..=512).contains(&value) => value,
            Ok(_) | Err(_) => {
                return self.error(
                    422,
                    "invalid_query",
                    "max_items must be between 1 and 512",
                    request_id,
                )
            }
        };
        let job = match self.mission_jobs.lock() {
            Ok(jobs) => jobs.get(&mission_id).cloned(),
            Err(_) => {
                return self.error(
                    500,
                    "mission_registry_unavailable",
                    "mission job registry is unavailable",
                    request_id,
                )
            }
        };
        let Some(job) = job else {
            return self.error(404, "not_found", "mission does not exist", request_id);
        };
        let state = match job_state(&job) {
            Ok(state) => state,
            Err(_) => {
                return self.error(
                    500,
                    "mission_state_unavailable",
                    "mission state is unavailable",
                    request_id,
                )
            }
        };
        if !is_terminal_mission_status(&state.status)
            && state.result.is_none()
            && state.evaluator_replay_summary.is_none()
        {
            return self.error(
                409,
                "evidence_bundle_unavailable",
                "mission evidence bundle is available after a terminal mission report is retained",
                request_id,
            );
        }
        let catalogue = MissionEvaluatorCatalogue::standard();
        let (replay, comparison) = if let Some(result) = state.result.clone() {
            let replay = catalogue
                .replay(&MissionEvaluatorReplayRequest {
                    mission: result.clone(),
                    include_fixtures,
                    max_items,
                })
                .map_err(|error| error.to_string());
            let comparison = catalogue
                .compare(&MissionEvaluatorReplayCompareRequest {
                    mission: result,
                    include_fixtures: false,
                    max_items,
                })
                .map_err(|error| error.to_string());
            (replay, comparison)
        } else if let Some(summary) = state.evaluator_replay_summary.clone() {
            let comparison = catalogue
                .compare_summary(&summary)
                .map_err(|error| error.to_string());
            (Ok(summary), comparison)
        } else {
            (Ok(Value::Null), Ok(Value::Null))
        };
        let replay = match replay {
            Ok(replay) => replay,
            Err(error) => {
                return self.error(422, "evidence_bundle_replay_invalid", &error, request_id)
            }
        };
        let comparison = match comparison {
            Ok(comparison) => comparison,
            Err(error) => {
                return self.error(
                    422,
                    "evidence_bundle_comparison_invalid",
                    &error,
                    request_id,
                )
            }
        };
        let result_digest = state
            .result
            .as_ref()
            .and_then(|result| ContentHash::of_value(result).ok())
            .map(|digest| digest.to_string());
        let mut bundle = json!({
            "schema": "bioprism-api/mission-evidence-bundle/0.1",
            "workflow": "mission_evidence_bundle_export",
            "mission_id": mission_id,
            "retention": {
                "mode": if state.result.is_some() { "full" } else { "summary_only" },
                "result_retained": state.result.is_some(),
                "result_included": include_result && state.result.is_some(),
                "summary_retained": state.evaluator_replay_summary.is_some(),
                "result_omitted": state.result_omitted.clone()
            },
            "mission": {
                "status": state.status,
                "cancel_requested": state.cancel_requested,
                "cancel_reason": state.cancel_reason,
                "recovered_after_restart": state.recovered_after_restart,
                "error": state.error,
                "progress": mission_progress_json(&state.progress)
            },
            "result": if include_result {
                state.result.clone().unwrap_or(Value::Null)
            } else {
                Value::Null
            },
            "result_digest": result_digest,
            "execution_provenance": state.execution_provenance,
            "claim_lineage": state
                .result
                .as_ref()
                .and_then(|result| result.get("claim_lineage"))
                .cloned()
                .unwrap_or(Value::Null),
            "evaluator_replay": replay,
            "catalog_drift": comparison.get("catalog_drift").cloned().unwrap_or(Value::Null),
            "trace": if include_trace {
                json!(state.trace)
            } else {
                json!([])
            },
            "export": {
                "format": "json",
                "include_result": include_result,
                "include_trace": include_trace,
                "trace_included": include_trace,
                "include_fixtures": include_fixtures,
                "max_items": max_items,
                "execution": "not_started",
                "digest_algorithm": "sha256"
            },
            "guarantees": [
                "the bundle is assembled from the bounded local mission registry and does not execute a tool",
                "every included result, replay, trace, provenance, and omission field remains separately inspectable",
                "bundle_digest covers the canonical bundle object before the digest field is added"
            ],
            "limitations": [
                "the bundle is a local evidence export, not a signature or proof of external storage",
                "summary-only bundles cannot include omitted raw mission output or reconstruct historical catalogue rows",
                "included evidence does not establish scientific, clinical, causal, operational, regulatory, or release truth"
            ],
            "links": {
                "mission": format!("/v1/missions/{mission_id}"),
                "claims": format!("/v1/missions/{mission_id}/claims"),
                "replay": format!("/v1/missions/{mission_id}/evaluator-replay"),
                "compare": format!("/v1/missions/{mission_id}/evaluator-replay/compare"),
                "bundle": format!("/v1/missions/{mission_id}/evidence-bundle")
            }
        });
        let bundle_digest = match ContentHash::of_value(&bundle) {
            Ok(digest) => digest.to_string(),
            Err(error) => {
                return self.error(
                    500,
                    "evidence_bundle_digest_failed",
                    &error.to_string(),
                    request_id,
                )
            }
        };
        bundle["bundle_digest"] = Value::String(bundle_digest);
        let bundle_bytes = match serde_json::to_vec(&bundle) {
            Ok(bytes) => bytes,
            Err(error) => {
                return self.error(
                    500,
                    "evidence_bundle_serialize_failed",
                    &error.to_string(),
                    request_id,
                )
            }
        };
        if bundle_bytes.len() > MAX_MISSION_EVIDENCE_BUNDLE_BYTES {
            return self.error(
                413,
                "evidence_bundle_too_large",
                &format!(
                    "evidence bundle is {} bytes; disable include_result or narrow the export below the {}-byte bound",
                    bundle_bytes.len(),
                    MAX_MISSION_EVIDENCE_BUNDLE_BYTES
                ),
                request_id,
            );
        }
        HttpResponse::json(200, &bundle)
    }

    pub(super) fn mission_evaluator_replay(
        &self,
        request: &HttpRequest,
        request_id: &str,
    ) -> HttpResponse {
        let Some(mission_id) = mission_id(&request.path_segments(), Some("evaluator-replay"))
        else {
            return self.error(
                404,
                "not_found",
                "mission evaluator replay route does not exist",
                request_id,
            );
        };
        let query = match request.query() {
            Ok(query) => query,
            Err(error) => return self.error(400, "invalid_query", &error.to_string(), request_id),
        };
        for key in query.keys() {
            if key != "include_fixtures" && key != "max_items" {
                return self.error(
                    400,
                    "invalid_query",
                    "mission evaluator replay accepts only include_fixtures and max_items",
                    request_id,
                );
            }
        }
        let include_fixtures = match query_bool(&query, "include_fixtures", false) {
            Ok(value) => value,
            Err(error) => return self.error(422, "invalid_query", &error, request_id),
        };
        let max_items = match query_usize(&query, "max_items", 128) {
            Ok(value) if (1..=512).contains(&value) => value,
            Ok(_) | Err(_) => {
                return self.error(
                    422,
                    "invalid_query",
                    "max_items must be between 1 and 512",
                    request_id,
                )
            }
        };
        let job = match self.mission_jobs.lock() {
            Ok(jobs) => jobs.get(&mission_id).cloned(),
            Err(_) => {
                return self.error(
                    500,
                    "mission_registry_unavailable",
                    "mission job registry is unavailable",
                    request_id,
                )
            }
        };
        let Some(job) = job else {
            return self.error(404, "not_found", "mission does not exist", request_id);
        };
        let state = match job_state(&job) {
            Ok(state) => state,
            Err(_) => {
                return self.error(
                    500,
                    "mission_state_unavailable",
                    "mission state is unavailable",
                    request_id,
                )
            }
        };
        let query_value = json!({
            "include_fixtures": include_fixtures,
            "max_items": max_items
        });
        let retention = json!({
            "mode": "full",
            "result_retained": state.result.is_some(),
            "summary_retained": state.evaluator_replay_summary.is_some(),
            "result_omitted": state.result_omitted.clone()
        });
        let replay = if let Some(result) = state.result.clone() {
            let replay_request = MissionEvaluatorReplayRequest {
                mission: result,
                include_fixtures,
                max_items,
            };
            match MissionEvaluatorCatalogue::standard().replay(&replay_request) {
                Ok(mut replay) => {
                    if let Some(object) = replay.as_object_mut() {
                        object.insert("source".into(), json!("durable_mission_result"));
                        object.insert("query".into(), query_value.clone());
                    }
                    replay
                }
                Err(error) => {
                    return self.error(
                        422,
                        "evaluator_replay_invalid",
                        &error.to_string(),
                        request_id,
                    )
                }
            }
        } else if let Some(summary) = state.evaluator_replay_summary.clone() {
            return HttpResponse::json(
                200,
                &json!({
                    "ok": true,
                    "schema": "bioprism-api/mission-evaluator-replay-query/0.1",
                    "workflow": "mission_evaluator_replay_query",
                    "mission_id": mission_id,
                    "query": query_value,
                    "retention": {
                        "mode": "summary_only",
                        "result_retained": false,
                        "summary_retained": true,
                        "result_omitted": state.result_omitted.clone()
                    },
                    "replay": summary,
                    "execution": "not_started",
                    "guarantees": [
                        "the compact evaluator summary is restored from the mission checkpoint",
                        "result omission remains explicit and is never represented as replay success",
                        "no evaluator or domain tool is executed by this query"
                    ],
                    "limitations": [
                        "full bindings, retained outputs, and fixture rows require the original mission result",
                        "summary-only replay cannot revalidate omitted raw output against a changed catalogue"
                    ],
                    "links": {
                        "mission": format!("/v1/missions/{mission_id}"),
                        "claims": format!("/v1/missions/{mission_id}/claims"),
                        "replay": format!("/v1/missions/{mission_id}/evaluator-replay")
                    }
                }),
            );
        } else if let Some(omitted) = state.result_omitted.clone() {
            return self.error(
                410,
                "mission_evaluator_replay_omitted",
                &format!(
                    "mission result and evaluator replay summary were omitted from the bounded registry snapshot ({} bytes, sha256 {})",
                    omitted["bytes"], omitted["sha256"]
                ),
                request_id,
            );
        } else {
            return self.error(
                409,
                "evaluator_replay_unavailable",
                "mission evaluator replay is available after a terminal mission report is retained",
                request_id,
            );
        };
        HttpResponse::json(
            200,
            &json!({
                "ok": true,
                "schema": "bioprism-api/mission-evaluator-replay-query/0.1",
                "workflow": "mission_evaluator_replay_query",
                "mission_id": mission_id,
                "query": query_value,
                "retention": retention,
                "replay": replay,
                "execution": "not_started",
                "guarantees": [
                    "the replay input is read from the bounded durable mission registry",
                    "full replay rechecks the current evaluator catalogue without dispatch",
                    "the retention mode and omitted-result metadata remain explicit"
                ],
                "limitations": [
                    "replay does not rerun an evaluator or validate domain semantics",
                    "a summary-only response cannot expose omitted raw evaluator output"
                ],
                "links": {
                    "mission": format!("/v1/missions/{mission_id}"),
                    "claims": format!("/v1/missions/{mission_id}/claims"),
                    "replay": format!("/v1/missions/{mission_id}/evaluator-replay")
                }
            }),
        )
    }

    pub(super) fn mission_claims(&self, request: &HttpRequest, request_id: &str) -> HttpResponse {
        let Some(mission_id) = mission_id(&request.path_segments(), Some("claims")) else {
            return self.error(
                404,
                "not_found",
                "mission claim lineage route does not exist",
                request_id,
            );
        };
        if request
            .query()
            .map(|query| !query.is_empty())
            .unwrap_or(true)
        {
            return self.error(
                400,
                "invalid_query",
                "mission claim lineage does not accept query parameters",
                request_id,
            );
        }
        let job = match self.mission_jobs.lock() {
            Ok(jobs) => jobs.get(&mission_id).cloned(),
            Err(_) => {
                return self.error(
                    500,
                    "mission_registry_unavailable",
                    "mission job registry is unavailable",
                    request_id,
                )
            }
        };
        let Some(job) = job else {
            return self.error(404, "not_found", "mission does not exist", request_id);
        };
        let state = match job_state(&job) {
            Ok(state) => state,
            Err(_) => {
                return self.error(
                    500,
                    "mission_state_unavailable",
                    "mission state is unavailable",
                    request_id,
                )
            }
        };
        let Some(result) = state.result else {
            if let Some(omitted) = state.result_omitted {
                return self.error(
                    410,
                    "mission_result_omitted",
                    &format!(
                        "mission result was omitted from the bounded registry snapshot ({} bytes, sha256 {})",
                        omitted["bytes"], omitted["sha256"]
                    ),
                    request_id,
                );
            }
            return self.error(
                409,
                "claim_lineage_unavailable",
                "mission claim lineage is available after a terminal report is retained",
                request_id,
            );
        };
        let Some(claim_lineage) = result.get("claim_lineage") else {
            return self.error(
                404,
                "claim_lineage_unavailable",
                "mission report predates the claim lineage contract or did not request claims",
                request_id,
            );
        };
        if !claim_lineage.is_object() {
            return self.error(
                500,
                "invalid_claim_lineage",
                "mission report claim_lineage is not an object",
                request_id,
            );
        }
        HttpResponse::json(
            200,
            &json!({
                "ok": true,
                "schema": "bioprism-mission-claim-lineage-response/0.1",
                "mission_id": mission_id,
                "claim_lineage": claim_lineage,
                "guarantees": [
                    "claim rows are correlated only to the explicitly requested mission steps",
                    "omitted outputs and non-successful steps remain visible as non-claimable evidence states",
                    "the projection preserves non-claim posture and does not interpret claim truth"
                ],
                "non_claims": [
                    "claimable means the requested evidence was retained, not that the claim is true",
                    "this route is not scientific, clinical, regulatory, or deployment approval"
                ],
                "links": {
                    "mission": format!("/v1/missions/{mission_id}"),
                    "mission_trace": format!("/v1/missions/{mission_id}/trace"),
                    "execution_provenance": format!("/v1/missions/{mission_id}/provenance")
                }
            }),
        )
    }

    pub(super) fn mission_trace(&self, request: &HttpRequest, request_id: &str) -> HttpResponse {
        let Some(mission_id) = mission_id(&request.path_segments(), Some("trace")) else {
            return self.error(
                404,
                "not_found",
                "mission trace route does not exist",
                request_id,
            );
        };
        let query = match request.query() {
            Ok(query) => query,
            Err(error) => return self.error(400, "invalid_query", &error.to_string(), request_id),
        };
        for key in query.keys() {
            if key != "after" && key != "limit" {
                return self.error(
                    400,
                    "invalid_query",
                    "mission trace accepts only after and limit",
                    request_id,
                );
            }
        }
        let after = match query_u64(&query, "after", 0) {
            Ok(value) => value,
            Err(error) => return self.error(400, "invalid_query", &error, request_id),
        };
        let limit = match query_usize(&query, "limit", 100) {
            Ok(value) if (1..=1000).contains(&value) => value,
            Ok(_) => {
                return self.error(
                    400,
                    "invalid_query",
                    "limit must be between 1 and 1000",
                    request_id,
                )
            }
            Err(error) => return self.error(400, "invalid_query", &error, request_id),
        };
        let jobs = match self.mission_jobs.lock() {
            Ok(jobs) => jobs,
            Err(_) => {
                return self.error(
                    500,
                    "mission_registry_unavailable",
                    "mission job registry is unavailable",
                    request_id,
                )
            }
        };
        let Some(job) = jobs.get(&mission_id).cloned() else {
            return self.error(404, "not_found", "mission does not exist", request_id);
        };
        let state = match job_state(&job) {
            Ok(state) => state,
            Err(_) => {
                return self.error(
                    500,
                    "mission_state_unavailable",
                    "mission state is unavailable",
                    request_id,
                )
            }
        };
        let oldest = state
            .trace
            .first()
            .and_then(|event| event.get("sequence"))
            .and_then(Value::as_u64);
        let newest = state
            .trace
            .last()
            .and_then(|event| event.get("sequence"))
            .and_then(Value::as_u64);
        let events = state
            .trace
            .iter()
            .filter(|event| {
                event
                    .get("sequence")
                    .and_then(Value::as_u64)
                    .is_some_and(|sequence| sequence >= after)
            })
            .take(limit)
            .cloned()
            .collect::<Vec<_>>();
        let next_after = events
            .last()
            .and_then(|event| event.get("sequence"))
            .and_then(Value::as_u64)
            .map_or(after, |sequence| sequence.saturating_add(1));
        let dropped_events = oldest.map_or(0, |sequence| sequence.saturating_sub(after));
        HttpResponse::json(
            200,
            &json!({
                "ok": true,
                "mission_id": mission_id,
                "trace_schema_version": bioprism_mcp::MISSION_TRACE_SCHEMA_VERSION,
                "events": events,
                "after": after,
                "next_after": next_after,
                "oldest": oldest,
                "newest": newest,
                "gap": dropped_events > 0,
                "dropped_events": dropped_events,
                "terminal": is_terminal_mission_status(&state.status),
                "limit": limit,
                "truncated": next_after < newest.map_or(next_after, |sequence| sequence.saturating_add(1)),
                "guarantees": [
                    "events are ordered by the authoritative clock-free mission sequence",
                    "after is an inclusive sequence cursor for the first page and next_after is exclusive",
                    "retention gaps are reported instead of silently presented as complete history"
                ]
            }),
        )
    }

    pub(super) fn mission_control(&self, request: &HttpRequest, request_id: &str) -> HttpResponse {
        let Some(mission_id) = mission_id(&request.path_segments(), Some("cancel")) else {
            return self.error(
                404,
                "not_found",
                "mission control route does not exist",
                request_id,
            );
        };
        let job = match self.mission_jobs.lock() {
            Ok(jobs) => jobs.get(&mission_id).cloned(),
            Err(_) => {
                return self.error(
                    500,
                    "mission_registry_unavailable",
                    "mission job registry is unavailable",
                    request_id,
                )
            }
        };
        let Some(job) = job else {
            return self.error(404, "not_found", "mission does not exist", request_id);
        };
        let reason = if request.body.is_empty() {
            "cancellation requested by API caller".to_string()
        } else {
            let body = match self.json_object(request) {
                Ok(body) => body,
                Err(error) => return self.error(400, "invalid_json", &error, request_id),
            };
            match body.get("reason") {
                None => "cancellation requested by API caller".to_string(),
                Some(Value::String(value)) if !value.trim().is_empty() && value.len() <= 2_048 => {
                    value.clone()
                }
                Some(_) => {
                    return self.error(
                        422,
                        "invalid_cancellation",
                        "reason must be a non-empty string of at most 2048 bytes",
                        request_id,
                    )
                }
            }
        };
        let mut current = match job_state(&job) {
            Ok(state) => state,
            Err(_) => {
                return self.error(
                    500,
                    "mission_state_unavailable",
                    "mission state is unavailable",
                    request_id,
                )
            }
        };
        if is_terminal_mission_status(&current.status) {
            if current.status == "cancelled" {
                return HttpResponse::json(
                    200,
                    &json!({ "ok": true, "mission_id": mission_id, "status": current.status, "cancel_requested": true, "idempotent": true }),
                );
            }
            return self.error(
                409,
                "mission_terminal",
                "mission has already reached a terminal state",
                request_id,
            );
        }
        job.cancellation.store(true, Ordering::Release);
        current.cancel_requested = true;
        current.cancel_reason = Some(reason.clone());
        current.progress.request_cancel();
        let Ok(mut state) = job.state.lock() else {
            return self.error(
                500,
                "mission_state_unavailable",
                "mission state is unavailable",
                request_id,
            );
        };
        state.cancel_requested = current.cancel_requested;
        state.cancel_reason = current.cancel_reason.clone();
        state.progress.request_cancel();
        drop(state);
        if let Err(error) = self.mission_queue_persistence.cancel(&mission_id, &reason) {
            if !error.contains("already terminal") {
                return self.error(503, "mission_queue_unavailable", &error, request_id);
            }
        }
        let _ = self.persist_mission_registry();
        HttpResponse::json(
            202,
            &json!({
                "ok": true,
                "mission_id": mission_id,
                "status": current.status,
                "cancel_requested": true,
                "cancel_reason": current.cancel_reason,
                "progress": mission_progress_json(&current.progress),
                "reason": reason,
                "poll": format!("/v1/missions/{mission_id}"),
                "trace": format!("/v1/missions/{mission_id}/trace"),
            }),
        )
    }

    pub(super) fn delete_mission(&self, request: &HttpRequest, request_id: &str) -> HttpResponse {
        let Some(mission_id) = mission_id(&request.path_segments(), None) else {
            return self.error(404, "not_found", "mission route does not exist", request_id);
        };
        let mut jobs = match self.mission_jobs.lock() {
            Ok(jobs) => jobs,
            Err(_) => {
                return self.error(
                    500,
                    "mission_registry_unavailable",
                    "mission job registry is unavailable",
                    request_id,
                )
            }
        };
        let Some(job) = jobs.remove(&mission_id) else {
            return self.error(404, "not_found", "mission does not exist", request_id);
        };
        let state = match job_state(&job) {
            Ok(state) => state,
            Err(_) => {
                jobs.insert(mission_id, job);
                return self.error(
                    500,
                    "mission_state_unavailable",
                    "mission state is unavailable",
                    request_id,
                );
            }
        };
        if !is_terminal_mission_status(&state.status) {
            jobs.insert(mission_id, job);
            return self.error(
                409,
                "mission_running",
                "only terminal missions may be removed",
                request_id,
            );
        }
        drop(jobs);
        let queue_projection = match self.mission_queue_persistence.projection(&mission_id) {
            Ok(projection) => projection,
            Err(error) => {
                if let Ok(mut jobs) = self.mission_jobs.lock() {
                    jobs.insert(mission_id.clone(), job);
                }
                return self.error(503, "mission_queue_unavailable", &error, request_id);
            }
        };
        if !queue_projection.is_null() {
            if let Err(error) = self
                .mission_queue_persistence
                .cancel(&mission_id, "mission deleted by operator")
            {
                if !error.contains("already terminal") {
                    if let Ok(mut jobs) = self.mission_jobs.lock() {
                        jobs.insert(mission_id.clone(), job);
                    }
                    return self.error(503, "mission_queue_unavailable", &error, request_id);
                }
            }
        }
        if let Err(error) = self.persist_mission_registry() {
            if let Ok(mut jobs) = self.mission_jobs.lock() {
                jobs.insert(mission_id.clone(), job);
            }
            return self.error(503, "mission_persistence_unavailable", &error, request_id);
        }
        HttpResponse::json(
            200,
            &json!({ "ok": true, "mission_id": mission_id, "deleted": true }),
        )
    }
}
