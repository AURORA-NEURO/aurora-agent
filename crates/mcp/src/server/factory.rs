//! Deterministic factory lifecycle and authority verification handlers.

use super::*;

impl Server {
    pub(super) fn factory_lifecycle_simulate(&self, arguments: &Value) -> Result<Value, String> {
        let raw_jobs = arguments
            .get("jobs")
            .and_then(Value::as_array)
            .ok_or("jobs is required and must be an array of serialized Job values")?;
        let raw_workers = arguments.get("workers").and_then(Value::as_array).ok_or(
            "workers is required and must be an array of serialized WorkerCapability values",
        )?;
        let raw_actions = arguments
            .get("actions")
            .and_then(Value::as_array)
            .ok_or("actions is required and must be an array of lifecycle operations")?;
        if raw_jobs.is_empty() || raw_jobs.len() > 256 {
            return Err("jobs must contain between 1 and 256 entries".into());
        }
        if raw_workers.is_empty() || raw_workers.len() > 256 {
            return Err("workers must contain between 1 and 256 entries".into());
        }
        if raw_actions.len() > 2_000 {
            return Err("actions are bounded at 2000 entries".into());
        }
        let encoded = serde_json::to_vec(&json!({
            "jobs": raw_jobs,
            "workers": raw_workers,
            "actions": raw_actions,
        }))
        .map_err(|error| format!("cannot measure factory simulation input: {error}"))?;
        if encoded.len() > 20_000_000 {
            return Err("factory simulation input exceeds the 20000000-byte safety bound".into());
        }

        let jobs = raw_jobs
            .iter()
            .cloned()
            .map(serde_json::from_value::<FactoryJob>)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("invalid factory job: {error}"))?;
        let workers = raw_workers
            .iter()
            .cloned()
            .map(serde_json::from_value::<WorkerCapability>)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("invalid worker capability: {error}"))?;
        let mut worker_by_id = BTreeMap::new();
        for worker in workers {
            if worker_by_id
                .insert(worker.worker_id.clone(), worker)
                .is_some()
            {
                return Err("workers must have unique worker_id values".into());
            }
        }

        let mut store = JobStore::new();
        let mut job_ids = BTreeSet::new();
        for job in jobs {
            let requested_id = job.id.clone();
            let enqueued = store
                .enqueue(job)
                .map_err(|error| format!("initial enqueue of {requested_id:?} refused: {error}"))?;
            job_ids.insert(enqueued);
        }

        let mut trace = Vec::with_capacity(raw_actions.len());
        let mut failures = 0usize;
        for (index, raw_action) in raw_actions.iter().enumerate() {
            let kind = raw_action
                .get("kind")
                .and_then(Value::as_str)
                .unwrap_or("<missing>")
                .to_string();
            let action_result: Result<Value, String> = (|| match kind.as_str() {
                "enqueue" => {
                    let raw_job = raw_action
                        .get("job")
                        .cloned()
                        .ok_or("enqueue requires a serialized job in `job`")?;
                    let job: FactoryJob = serde_json::from_value(raw_job)
                        .map_err(|error| format!("invalid enqueue job: {error}"))?;
                    let requested_id = job.id.clone();
                    let id = store.enqueue(job).map_err(|error| {
                        format!("enqueue refused for {requested_id:?}: {error}")
                    })?;
                    job_ids.insert(id.clone());
                    Ok(json!({ "job_id": id }))
                }
                "lease" => {
                    let worker_id = raw_action
                        .get("worker_id")
                        .and_then(Value::as_str)
                        .ok_or("lease requires worker_id")?;
                    let worker = worker_by_id
                        .get(worker_id)
                        .ok_or_else(|| format!("unknown worker {worker_id:?}"))?;
                    let now = FactoryTimestamp::from_nanos_utc(json_i128(
                        raw_action.get("now_nanos"),
                        "now_nanos",
                    )?);
                    let lease = store
                        .lease(worker, now)
                        .map_err(|error| format!("lease refused: {error}"))?;
                    if let Some(lease) = &lease {
                        job_ids.insert(lease.job_id.clone());
                    }
                    serde_json::to_value(lease)
                        .map_err(|error| format!("cannot serialize lease result: {error}"))
                }
                "heartbeat" => {
                    let job_id = raw_action
                        .get("job_id")
                        .and_then(Value::as_str)
                        .ok_or("heartbeat requires job_id")?;
                    let worker_id = raw_action
                        .get("worker_id")
                        .and_then(Value::as_str)
                        .ok_or("heartbeat requires worker_id")?;
                    let attempt = json_u32(raw_action.get("attempt"), "attempt")?;
                    let now = FactoryTimestamp::from_nanos_utc(json_i128(
                        raw_action.get("now_nanos"),
                        "now_nanos",
                    )?);
                    let duration = json_i128(raw_action.get("duration_nanos"), "duration_nanos")?;
                    store
                        .heartbeat(job_id, worker_id, attempt, now, duration)
                        .map_err(|error| format!("heartbeat refused: {error}"))?;
                    Ok(json!({ "job_id": job_id, "worker_id": worker_id }))
                }
                "stage" => {
                    let job_id = raw_action
                        .get("job_id")
                        .and_then(Value::as_str)
                        .ok_or("stage requires job_id")?;
                    let worker_id = raw_action
                        .get("worker_id")
                        .and_then(Value::as_str)
                        .ok_or("stage requires worker_id")?;
                    let attempt = json_u32(raw_action.get("attempt"), "attempt")?;
                    let now = FactoryTimestamp::from_nanos_utc(json_i128(
                        raw_action.get("now_nanos"),
                        "now_nanos",
                    )?);
                    let output = raw_action
                        .get("output")
                        .cloned()
                        .ok_or("stage requires output")?;
                    store
                        .stage(job_id, worker_id, attempt, now, output)
                        .map_err(|error| format!("stage refused: {error}"))?;
                    Ok(json!({ "job_id": job_id, "visible_before_commit": false }))
                }
                "commit" => {
                    let job_id = raw_action
                        .get("job_id")
                        .and_then(Value::as_str)
                        .ok_or("commit requires job_id")?;
                    let worker_id = raw_action
                        .get("worker_id")
                        .and_then(Value::as_str)
                        .ok_or("commit requires worker_id")?;
                    let attempt = json_u32(raw_action.get("attempt"), "attempt")?;
                    let now = FactoryTimestamp::from_nanos_utc(json_i128(
                        raw_action.get("now_nanos"),
                        "now_nanos",
                    )?);
                    store
                        .commit(job_id, worker_id, attempt, now)
                        .map_err(|error| format!("commit refused: {error}"))?;
                    Ok(json!({ "job_id": job_id, "committed": true }))
                }
                "fail" => {
                    let job_id = raw_action
                        .get("job_id")
                        .and_then(Value::as_str)
                        .ok_or("fail requires job_id")?;
                    let worker_id = raw_action
                        .get("worker_id")
                        .and_then(Value::as_str)
                        .ok_or("fail requires worker_id")?;
                    let attempt = json_u32(raw_action.get("attempt"), "attempt")?;
                    let now = FactoryTimestamp::from_nanos_utc(json_i128(
                        raw_action.get("now_nanos"),
                        "now_nanos",
                    )?);
                    let reason = raw_action
                        .get("reason")
                        .and_then(Value::as_str)
                        .ok_or("fail requires reason")?;
                    let recovery = store
                        .fail(job_id, worker_id, attempt, now, reason)
                        .map_err(|error| format!("failure report refused: {error}"))?;
                    serde_json::to_value(recovery)
                        .map_err(|error| format!("cannot serialize recovery: {error}"))
                }
                "recover_expired" => {
                    let now = FactoryTimestamp::from_nanos_utc(json_i128(
                        raw_action.get("now_nanos"),
                        "now_nanos",
                    )?);
                    let recoveries = store.recover_expired(now);
                    serde_json::to_value(recoveries)
                        .map_err(|error| format!("cannot serialize recoveries: {error}"))
                }
                "compensate" => {
                    let job_id = raw_action
                        .get("job_id")
                        .and_then(Value::as_str)
                        .ok_or("compensate requires job_id")?;
                    store
                        .compensate(job_id)
                        .map_err(|error| format!("compensation refused: {error}"))?;
                    Ok(json!({ "job_id": job_id, "compensated": true }))
                }
                "release_quarantine" => {
                    let job_id = raw_action
                        .get("job_id")
                        .and_then(Value::as_str)
                        .ok_or("release_quarantine requires job_id")?;
                    let operator = raw_action
                        .get("operator")
                        .and_then(Value::as_str)
                        .ok_or("release_quarantine requires operator")?;
                    store
                        .release_quarantine(job_id, operator)
                        .map_err(|error| format!("quarantine release refused: {error}"))?;
                    Ok(json!({ "job_id": job_id, "operator": operator, "released": true }))
                }
                "cancel" => {
                    let job_id = raw_action
                        .get("job_id")
                        .and_then(Value::as_str)
                        .ok_or("cancel requires job_id")?;
                    let reason = raw_action
                        .get("reason")
                        .and_then(Value::as_str)
                        .ok_or("cancel requires reason")?;
                    store
                        .cancel(job_id, reason)
                        .map_err(|error| format!("cancellation refused: {error}"))?;
                    Ok(json!({ "job_id": job_id, "cancelled": true }))
                }
                other => Err(format!("unknown factory action {other:?}")),
            })();
            match action_result {
                Ok(result) => trace.push(json!({
                    "index": index,
                    "kind": kind,
                    "ok": true,
                    "result": result,
                })),
                Err(refusal) => {
                    failures += 1;
                    trace.push(json!({
                        "index": index,
                        "kind": kind,
                        "ok": false,
                        "refusal": refusal,
                        "fail_closed": true,
                    }));
                }
            }
        }

        let jobs = job_ids
            .iter()
            .filter_map(|id| {
                store.job(id).map(|job| {
                    json!({
                        "id": id,
                        "job": job,
                        "committed_result": store.result(id),
                    })
                })
            })
            .collect::<Vec<_>>();
        Ok(json!({
            "ok": failures == 0,
            "action_count": raw_actions.len(),
            "action_failures": failures,
            "trace": trace,
            "jobs": jobs,
            "quarantined": store.quarantined(),
            "dead_lettered": store.dead_lettered(),
            "counts_by_class": store.counts_by_class(),
            "guarantees": [
                "the simulation delegates every lifecycle transition to the typed in-memory JobStore",
                "lease expiry branches on idempotency and never treats non-idempotent ambiguity as a safe retry",
                "staged outputs are reported invisible until atomic commit",
                "compensation, quarantine release, cancellation, and every refusal remain explicit in the replay trace",
                "no worker process, queue, clock, filesystem, network, or external side effect is created",
            ],
        }))
    }

    /// Verify an execution-authority envelope without trusting its claimed digests or replaying
    /// any worker effect. This is the MCP-side audit seam for API operators and offline tooling.
    pub(super) fn factory_authority_verify(&self, arguments: &Value) -> Result<Value, String> {
        let checkpoint = arguments
            .get("checkpoint")
            .cloned()
            .ok_or("checkpoint is required and must be a serialized execution authority object")?;
        let encoded = serde_json::to_vec(&checkpoint)
            .map_err(|error| format!("cannot measure authority checkpoint: {error}"))?;
        if encoded.len() > 64 * 1024 * 1024 {
            return Err("authority checkpoint exceeds the 64 MiB safety bound".into());
        }
        let snapshot = ExecutionAuthoritySnapshot::from_json_bytes(&encoded)
            .map_err(|error| format!("execution authority verification refused: {error}"))?;
        let include_events = arguments
            .get("include_events")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let max_events = arguments
            .get("max_events")
            .and_then(Value::as_u64)
            .unwrap_or(64)
            .min(256) as usize;
        let events = if include_events {
            snapshot
                .events
                .iter()
                .rev()
                .take(max_events)
                .map(|event| {
                    json!({
                        "sequence": event.sequence,
                        "operation": event.operation,
                        "idempotency_key": event.idempotency_key,
                        "job_id": event.job_id,
                        "worker_id": event.worker_id,
                        "attempt": event.attempt,
                        "at": event.at,
                        "digest": event.digest,
                        "previous_digest": event.previous_digest,
                    })
                })
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        Ok(json!({
            "schema": "bioprism-factory-execution-authority/0.1",
            "valid": true,
            "schema_version": snapshot.schema_version,
            "revision": snapshot.revision,
            "authority_epoch": snapshot.authority_epoch,
            "state_digest": snapshot.state_digest,
            "queue_state_digest": snapshot.queue.state_digest,
            "job_count": snapshot.queue.jobs.len(),
            "active_lease_count": snapshot.queue.leases.len(),
            "event_count": snapshot.events.len(),
            "events": events,
            "guarantees": [
                "the queue snapshot and transition journal verify under one content digest",
                "transition sequence and previous-digest links were checked before projection",
                "legacy bare queue checkpoints are accepted only after their queue digest verifies"
            ],
            "does_not_claim": [
                "multi-host consensus or network-partition tolerance",
                "worker liveness",
                "external provider effect completion"
            ]
        }))
    }
}
