//! Mission checkpoint serialization, recovery projections, and bounded state helpers.
//!
//! These helpers keep durable mission representation consistent across restart, polling, and
//! operator evidence routes.
use super::*;

pub(super) fn durable_mission_state_json(mission_id: &str, state: &MissionJobState) -> Value {
    let persisted_trace = state
        .trace
        .iter()
        .map(durable_trace_event)
        .collect::<Vec<_>>();
    let (result, generated_omission) = match state.result.as_ref() {
        Some(result) => match serde_json::to_vec(result) {
            Ok(bytes) if bytes.len() <= MAX_PERSISTED_MISSION_RESULT_BYTES => {
                (result.clone(), None)
            }
            Ok(bytes) => (Value::Null, Some(value_omission(&bytes))),
            Err(_) => (Value::Null, None),
        },
        None => (Value::Null, None),
    };
    let (execution_provenance, generated_provenance_omission) =
        match state.execution_provenance.as_ref() {
            Some(provenance) => match serde_json::to_vec(provenance) {
                Ok(bytes) if bytes.len() <= MAX_PERSISTED_MISSION_PROVENANCE_BYTES => {
                    (provenance.clone(), None)
                }
                Ok(bytes) => (Value::Null, Some(value_omission(&bytes))),
                Err(_) => (Value::Null, None),
            },
            None => (Value::Null, None),
        };
    json!({
        "mission_id": mission_id,
        "total_steps": state.total_steps,
        "status": state.status,
        "cancel_requested": state.cancel_requested,
        "cancel_reason": state.cancel_reason,
        "progress": mission_progress_json(&state.progress),
        "trace": persisted_trace,
        "result": result,
        "result_omitted": state.result_omitted.clone().or(generated_omission),
        "evaluator_replay_summary": state.evaluator_replay_summary.clone(),
        "route_review_provenance": state.route_review_provenance.clone(),
        "execution_provenance": execution_provenance,
        "execution_provenance_omitted": generated_provenance_omission,
        "error": state.error,
        "recovered_after_restart": state.recovered_after_restart,
    })
}

pub(super) fn durable_trace_event(event: &Value) -> Value {
    let Ok(bytes) = serde_json::to_vec(event) else {
        return json!({ "event": "trace.event_omitted", "detail_omitted": true });
    };
    if bytes.len() <= MAX_PERSISTED_MISSION_TRACE_EVENT_BYTES {
        return event.clone();
    }
    json!({
        "sequence": event.get("sequence"),
        "event": event.get("event"),
        "wave": event.get("wave"),
        "step_id": event.get("step_id"),
        "tool": event.get("tool"),
        "status": event.get("status"),
        "arguments_digest": event.get("arguments_digest"),
        "bytes": event.get("bytes"),
        "detail": Value::Null,
        "detail_omitted": value_omission(&bytes),
    })
}

/// Retain a compact evaluator replay index independently of the full terminal report. The index is
/// intentionally non-executing and non-semantic: it preserves enough accounting to explain what
/// remains queryable after the bounded mission result itself has been omitted from a checkpoint.
pub(super) fn evaluator_replay_summary(report: &Value, mission_id: &str) -> Option<Value> {
    if report.get("workflow").and_then(Value::as_str) != Some("agent_mission") {
        return None;
    }
    let result_bytes = serde_json::to_vec(report).ok()?;
    let result_digest = hex_digest(&Sha256::digest(&result_bytes));
    let request = MissionEvaluatorReplayRequest {
        mission: report.clone(),
        include_fixtures: false,
        max_items: 512,
    };
    let replay = MissionEvaluatorCatalogue::standard()
        .replay(&request)
        .ok()?;
    let claims = replay
        .get("claims")
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .filter_map(|row| {
                    Some(json!({
                        "claim_id": row.get("claim_id")?,
                        "binding_count": row.get("binding_count")?,
                        "returned_binding_count": row.get("returned_binding_count")?,
                        "outcome_counts": row.get("outcome_counts")?,
                        "distinct_output_digests": row.get("distinct_output_digests")?,
                        "disagreement_posture": row.get("disagreement_posture")?,
                        "replayed_disagreement_posture": row.get("replayed_disagreement_posture")?
                    }))
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let referenced_adapter_ids = mission_evaluator_binding_adapter_ids(report);
    let review_provenance = replay
        .get("review_provenance")
        .cloned()
        .unwrap_or(Value::Null);
    Some(json!({
        "schema": "bioprism-devplat-mission-evaluator-replay-summary/0.1",
        "workflow": "mission_evaluator_replay_summary",
        "mission_id": mission_id,
        "mission_digest": replay.get("mission_digest")?,
        "mission_status": replay.get("mission_status").cloned().unwrap_or(Value::Null),
        "catalog_digest": replay.get("catalog_digest")?,
        "historical_catalog_digest": review_provenance.get("catalog_digest").cloned().unwrap_or(Value::Null),
        "historical_review_id": review_provenance.get("review_id").cloned().unwrap_or(Value::Null),
        "historical_discovery_digest": review_provenance.get("discovery_digest").cloned().unwrap_or(Value::Null),
        "historical_catalogue_snapshot": review_provenance.get("catalogue_snapshot").cloned().unwrap_or(Value::Null),
        "route_review_provenance": replay.get("route_review_provenance").cloned().unwrap_or(Value::Null),
        "route_review_status": replay.get("route_review_status").cloned().unwrap_or(json!("absent")),
        "referenced_adapter_ids": referenced_adapter_ids,
        "binding_count": replay.get("binding_count")?,
        "omitted_bindings": replay.get("omitted_bindings")?,
        "state_counts": replay.get("state_counts")?,
        "claim_count": claims.len(),
        "claims": claims,
        "coverage": replay.get("coverage")?,
        "findings": replay.get("findings")?,
        "replay_status": replay.get("replay_status")?,
        "execution": "not_started",
        "result_retained": true,
        "result_bytes": result_bytes.len(),
        "result_digest": result_digest,
        "guarantees": [
            "the summary is derived from the retained terminal mission report",
            "the summary remains persisted when the full report exceeds the result retention bound",
            "no evaluator or domain tool is executed while building the summary"
        ],
        "limitations": [
            "summary-only recovery cannot expose omitted raw evaluator output or rerun replay against it",
            "the summary is structural evidence and not scientific, clinical, causal, or release truth"
        ]
    }))
}

pub(super) fn mission_evaluator_binding_adapter_ids(report: &Value) -> Vec<String> {
    report
        .get("claim_lineage")
        .and_then(|lineage| lineage.get("claims"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|claim| claim.get("evaluator_bindings"))
        .filter_map(Value::as_array)
        .flatten()
        .filter_map(|binding| binding.get("adapter_id"))
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

pub(super) fn value_omission(bytes: &[u8]) -> Value {
    let mut digest = Sha256::new();
    digest.update(bytes);
    json!({ "bytes": bytes.len(), "sha256": hex_digest(&digest.finalize()) })
}

pub(super) fn trim_mission_snapshot_to_bound(missions: &mut [Value]) -> Result<(), String> {
    loop {
        let mut document = json!({
            "schema_version": MISSION_STATE_SCHEMA_VERSION,
            "missions": missions,
            "guarantees": [
                "terminal reports are restored only when their bounded JSON was retained",
                "queued and running jobs are marked failed after a process restart",
                "event cursors and webhook deliveries remain process-local"
            ]
        });
        let state_digest = mission_checkpoint_digest(&document)?;
        let Some(document_object) = document.as_object_mut() else {
            return Err("mission checkpoint document is not an object".into());
        };
        document_object.insert("state_digest".into(), Value::String(state_digest));
        let size = serde_json::to_vec(&document)
            .map_err(|error| format!("mission state could not be sized: {error}"))?
            .len();
        if size <= MAX_MISSION_STATE_FILE_BYTES {
            return Ok(());
        }
        if let Some(object) = missions.iter_mut().find_map(Value::as_object_mut) {
            if let Some(result) = object.get_mut("result") {
                if !result.is_null() {
                    let bytes = serde_json::to_vec(result)
                        .map_err(|error| format!("mission result could not be sized: {error}"))?;
                    let omission = value_omission(&bytes);
                    *result = Value::Null;
                    object.insert("result_omitted".into(), omission);
                    continue;
                }
            }
        }
        if let Some(trace) = missions.iter_mut().find_map(|mission| {
            mission
                .get_mut("trace")
                .and_then(Value::as_array_mut)
                .filter(|trace| !trace.is_empty())
        }) {
            trace.remove(0);
            continue;
        }
        return Err(format!(
            "mission state snapshot cannot fit within the {}-byte bound",
            MAX_MISSION_STATE_FILE_BYTES
        ));
    }
}

pub(super) fn mission_progress_from_json(
    value: Option<&Value>,
    total_steps: usize,
) -> MissionProgressState {
    let mut progress = MissionProgressState::new(total_steps);
    let Some(object) = value.and_then(Value::as_object) else {
        return progress;
    };
    if let Some(phase) = object.get("phase").and_then(Value::as_str) {
        progress.phase = phase.to_string();
    }
    progress.current_wave = object.get("current_wave").and_then(value_usize);
    progress.total_steps = object
        .get("total_steps")
        .and_then(value_usize)
        .unwrap_or(total_steps);
    for (key, target) in [
        ("completed_steps", &mut progress.completed_steps),
        ("active_steps", &mut progress.active_steps),
        ("succeeded", &mut progress.succeeded),
        ("refused", &mut progress.refused),
        ("blocked", &mut progress.blocked),
        ("cancelled", &mut progress.cancelled),
        ("required_failures", &mut progress.required_failures),
        ("returned_bytes", &mut progress.returned_bytes),
    ] {
        if let Some(value) = object.get(key).and_then(value_usize) {
            *target = value;
        }
    }
    progress.trace_sequence = object.get("trace_sequence").and_then(value_usize);
    progress.last_event = object
        .get("last_event")
        .and_then(Value::as_str)
        .map(str::to_string);
    progress
}

pub(super) fn value_usize(value: &Value) -> Option<usize> {
    value.as_u64().and_then(|value| usize::try_from(value).ok())
}

pub(super) fn unavailable_event_metrics() -> EventMetrics {
    EventMetrics {
        retained_events: 0,
        dropped_events: 0,
        subscriptions: 0,
        active_subscriptions: 0,
        pending_deliveries: 0,
        dropped_deliveries: 0,
        next_event_id: 0,
        next_delivery_id: 0,
        retained_delivery_attempts: 0,
        dropped_delivery_attempts: 0,
        next_attempt_id: 0,
    }
}

pub(super) fn progress_count(report: &Value, key: &str) -> usize {
    report
        .get(key)
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
        .unwrap_or(0)
}

pub(super) fn mission_progress_json(progress: &MissionProgressState) -> Value {
    json!({
        "phase": progress.phase,
        "current_wave": progress.current_wave,
        "total_steps": progress.total_steps,
        "completed_steps": progress.completed_steps,
        "active_steps": progress.active_steps,
        "succeeded": progress.succeeded,
        "refused": progress.refused,
        "blocked": progress.blocked,
        "cancelled": progress.cancelled,
        "required_failures": progress.required_failures,
        "returned_bytes": progress.returned_bytes,
        "trace_sequence": progress.trace_sequence,
        "last_event": progress.last_event,
    })
}

pub(super) fn mission_summary(state: &MissionJobState) -> Value {
    let report = state.result.as_ref();
    let completed_steps = report
        .and_then(|report| report.get("results"))
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    let total_steps = report
        .and_then(|report| report.pointer("/plan/ordered_steps"))
        .and_then(Value::as_array)
        .map_or(state.total_steps, Vec::len);
    let count = |key: &str| {
        report
            .and_then(|report| report.get(key))
            .and_then(Value::as_u64)
            .unwrap_or(0)
    };
    json!({
        "total_steps": total_steps,
        "completed_steps": completed_steps,
        "succeeded": count("succeeded"),
        "refused": count("refused"),
        "blocked": count("blocked"),
        "cancelled": count("cancelled"),
        "required_failures": count("required_failures"),
        "returned_bytes": count("returned_bytes"),
        "result_available": report.is_some(),
        "result_omitted": state.result_omitted,
        "recovered_after_restart": state.recovered_after_restart,
    })
}

pub(super) fn is_known_mission_status(status: &str) -> bool {
    matches!(
        status,
        "queued" | "running" | "planned" | "succeeded" | "partial" | "failed" | "cancelled"
    )
}

pub(super) fn is_terminal_mission_status(status: &str) -> bool {
    matches!(
        status,
        "planned" | "succeeded" | "partial" | "failed" | "cancelled"
    )
}
