//! Value-only autonomous brain control-plane state.
//!
//! The provider runtime and the durable Python job store remain the authoritative places for
//! secrets and restart-safe execution. This module is the transport-facing admission and
//! observation layer used by the MCP server (and therefore by the HTTP adapter's generic tool
//! route). It deliberately retains only bounded metadata in process memory:
//!
//! * job submissions are rehydratable identities, never task or prompt payloads;
//! * approvals carry a caller-supplied authorization proof digest but are not authenticated by
//!   this crate; and
//! * health and replay contain only value-free provider posture and normalized [0, 1] signals.
//!
//! A deployment that needs restart durability should persist the same metadata through its
//! application-owned job store and recreate this projection on startup. The process boundary is
//! explicit in every response so an in-memory MCP session cannot be mistaken for a durable queue.

use bioprism_brain::{record_brain_outcome, BrainOutcomeRecordRequest};
use bioprism_ids::ContentHash;
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) const CONTROL_SCHEMA: &str = "bioprism-brain-control-plane/0.1";
pub(crate) const JOB_SCHEMA: &str = "bioprism-brain-job/0.1";
pub(crate) const EVENT_SCHEMA: &str = "bioprism-brain-job-event/0.1";
pub(crate) const HEALTH_SCHEMA: &str = "bioprism-brain-provider-health/0.1";
pub(crate) const REPLAY_SCHEMA: &str = "bioprism-brain-replay/0.1";
const DOMAIN_EVALUATOR_SCHEMA: &str = "bioprism-brain-domain-evaluator/0.1";

const MAX_JOBS: usize = 1_024;
const MAX_EVENTS: usize = 8_192;
const MAX_HEALTH_ROWS: usize = 1_024;
const MAX_ID_BYTES: usize = 256;
const MAX_REASON_BYTES: usize = 2_048;
const MAX_SIGNAL_COUNT: usize = 64;
const MAX_REFERENCE_COUNT: usize = 64;
const MAX_LIMITATION_COUNT: usize = 32;
const MAX_OUTCOME_RECORDS: usize = 4096;

#[derive(Clone, Default)]
pub(crate) struct BrainControlState {
    jobs: BTreeMap<String, JobRecord>,
    idempotency: BTreeMap<String, String>,
    events: Vec<Value>,
    head_digest: String,
    next_sequence: u64,
    health: BTreeMap<(String, String), HealthRecord>,
    outcome_records: BTreeMap<String, OutcomeRecord>,
}

#[derive(Clone)]
struct JobRecord {
    job_id: String,
    idempotency_key: String,
    spec_digest: String,
    domain: String,
    capability: String,
    risk_class: String,
    priority: u64,
    max_attempts: u64,
    state: String,
    attempts: u64,
    side_effect_boundary: String,
    checkpoint_digest: Option<String>,
    reason_digest: Option<String>,
    created_sequence: u64,
    updated_sequence: u64,
    record_digest: String,
    submission_event: Value,
}

#[derive(Clone, Default)]
struct HealthRecord {
    observations: u64,
    successes: u64,
    failures: u64,
    consecutive_failures: u64,
    total_latency_ms: u64,
    last_latency_ms: u64,
    quality_sum: f64,
    quality_observations: u64,
    last_status: String,
    last_sequence: u64,
    registered: bool,
    credential_ready: bool,
    eligible: bool,
}

#[derive(Clone)]
struct OutcomeRecord {
    contract_digest: String,
    report: Value,
}

impl BrainControlState {
    /// Record one evaluator outcome with an in-process idempotency barrier.
    ///
    /// The durable bandit state remains caller-owned. This bounded cache only closes the common
    /// retry window while an MCP process is alive; the credited outcome digest in the returned
    /// state is what makes the update replay-safe after the caller or server restarts.
    pub(crate) fn outcome_record(&mut self, arguments: &Value) -> Result<Value, String> {
        let object = object(arguments, "brain_outcome_record")?;
        reject_unknown(
            object,
            &[
                "run",
                "assessment",
                "bandit_state",
                "arm_id",
                "context_digest",
                "context",
                "idempotency_key",
            ],
        )?;
        let idempotency_key = object
            .get("idempotency_key")
            .map(|value| bounded_text(value, "idempotency_key", MAX_ID_BYTES))
            .transpose()?;
        let request: BrainOutcomeRecordRequest = serde_json::from_value(arguments.clone())
            .map_err(|error| format!("invalid brain outcome record request: {error}"))?;
        let contract_digest = digest_value(&json!({
            "run": request.run.clone(),
            "assessment": request.assessment.clone(),
            "arm_id": request.arm_id.clone(),
            "context_digest": request.context_digest.clone(),
            "context": request.context.clone(),
        }))?;
        let idempotency_key_digest = idempotency_key
            .as_ref()
            .map(|key| digest_value(&json!(key)))
            .transpose()?;

        if let Some(key) = &idempotency_key {
            if let Some(existing) = self.outcome_records.get(key) {
                if existing.contract_digest != contract_digest {
                    return Err(
                        "idempotency_key is already bound to a different evaluator outcome contract"
                            .into(),
                    );
                }
                let mut replay = existing.report.clone();
                replay["idempotent"] = json!(true);
                replay["idempotency_key_digest"] = json!(digest_value(&json!(key))?);
                return Ok(replay);
            }
            if self.outcome_records.len() >= MAX_OUTCOME_RECORDS {
                return Err(format!(
                    "brain outcome idempotency capacity is exhausted at {MAX_OUTCOME_RECORDS} records"
                ));
            }
        }

        let report = record_brain_outcome(&request)
            .map_err(|error| format!("brain outcome record refused: {error}"))?;
        let mut value = serde_json::to_value(report)
            .map_err(|error| format!("cannot encode brain learning evidence: {error}"))?;
        value["idempotent"] = json!(false);
        value["idempotency_key_digest"] = idempotency_key_digest
            .map(|digest| json!(digest))
            .unwrap_or(Value::Null);
        if let Some(key) = idempotency_key {
            self.outcome_records.insert(
                key,
                OutcomeRecord {
                    contract_digest,
                    report: value.clone(),
                },
            );
        }
        Ok(value)
    }

    pub(crate) fn submit_job(&mut self, arguments: &Value) -> Result<Value, String> {
        let object = object(arguments, "brain_job_submit")?;
        reject_unknown(
            object,
            &[
                "job_id",
                "idempotency_key",
                "spec_digest",
                "domain",
                "capability",
                "risk_class",
                "priority",
                "max_attempts",
                "checkpoint_digest",
            ],
        )?;
        let idempotency_key = text(object, "idempotency_key", MAX_ID_BYTES)?;
        let spec_digest = digest(object, "spec_digest")?;
        let domain = text(object, "domain", MAX_ID_BYTES)?;
        let capability = text(object, "capability", MAX_ID_BYTES)?;
        let risk_class = text(object, "risk_class", MAX_ID_BYTES)?;
        let priority = bounded_u64(object, "priority", 0, 255, 0)?;
        let max_attempts = bounded_u64(object, "max_attempts", 1, 8, 3)?;
        let checkpoint_digest = optional_digest(object, "checkpoint_digest")?;
        let job_id = match object.get("job_id") {
            Some(value) => bounded_text(value, "job_id", MAX_ID_BYTES)?,
            None => format!(
                "job-{}",
                digest_value(&json!({
                    "idempotency_key": idempotency_key,
                    "spec_digest": spec_digest,
                }))?
                .chars()
                .take(32)
                .collect::<String>()
            ),
        };

        if let Some(existing_id) = self.idempotency.get(&idempotency_key).cloned() {
            let existing = self
                .jobs
                .get(&existing_id)
                .ok_or_else(|| "idempotency index is inconsistent".to_string())?;
            if existing.spec_digest != spec_digest
                || existing.domain != domain
                || existing.capability != capability
                || existing.risk_class != risk_class
            {
                return Err(
                    "idempotency_key is already bound to a different brain job contract".into(),
                );
            }
            return Ok(json!({
                "schema": CONTROL_SCHEMA,
                "ok": true,
                "created": false,
                "idempotent": true,
                "job": existing.to_value(),
                "event": existing.submission_event,
                "retention": "metadata_only_hash_chained",
                "durability": durability_posture(),
            }));
        }
        if self.jobs.len() >= MAX_JOBS {
            return Err(format!(
                "brain job capacity is exhausted at {MAX_JOBS} jobs"
            ));
        }
        if self.jobs.contains_key(&job_id) {
            return Err("job_id is already registered".into());
        }

        let mut job = JobRecord {
            job_id: job_id.clone(),
            idempotency_key: idempotency_key.clone(),
            spec_digest,
            domain,
            capability,
            risk_class,
            priority,
            max_attempts,
            state: "queued".into(),
            attempts: 0,
            side_effect_boundary: "not_started".into(),
            checkpoint_digest,
            reason_digest: None,
            created_sequence: 0,
            updated_sequence: 0,
            record_digest: String::new(),
            submission_event: Value::Null,
        };
        let event = self.append_event(
            &job_id,
            "job_submitted",
            json!({
                "idempotency_key_digest": digest_value(&json!(idempotency_key))?,
                "spec_digest": job.spec_digest,
                "domain": job.domain,
                "capability": job.capability,
                "risk_class": job.risk_class,
                "priority": job.priority,
                "max_attempts": job.max_attempts,
                "checkpoint_digest": job.checkpoint_digest,
            }),
        )?;
        let sequence = event
            .get("sequence")
            .and_then(Value::as_u64)
            .ok_or_else(|| "submission event has no sequence".to_string())?;
        job.created_sequence = sequence;
        job.updated_sequence = sequence;
        job.record_digest = event
            .get("event_digest")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        job.submission_event = event.clone();
        let result = json!({
            "schema": CONTROL_SCHEMA,
            "ok": true,
            "created": true,
            "idempotent": false,
            "job": job.to_value(),
            "event": event,
            "retention": "metadata_only_hash_chained",
            "durability": durability_posture(),
        });
        self.idempotency.insert(idempotency_key, job_id.clone());
        self.jobs.insert(job_id, job);
        Ok(result)
    }

    pub(crate) fn job_status(&self, arguments: &Value) -> Result<Value, String> {
        let object = object(arguments, "brain_job_status")?;
        reject_unknown(object, &["job_id"])?;
        let job_id = text(object, "job_id", MAX_ID_BYTES)?;
        let job = self
            .jobs
            .get(&job_id)
            .ok_or_else(|| format!("unknown brain job_id {job_id:?}"))?;
        Ok(json!({
            "schema": CONTROL_SCHEMA,
            "ok": true,
            "job": job.to_value(),
            "head_digest": self.head_digest,
            "durability": durability_posture(),
        }))
    }

    pub(crate) fn job_events(&self, arguments: &Value) -> Result<Value, String> {
        let object = object(arguments, "brain_job_events")?;
        reject_unknown(object, &["job_id", "after", "limit"])?;
        let job_id = object
            .get("job_id")
            .map(|value| bounded_text(value, "job_id", MAX_ID_BYTES))
            .transpose()?;
        if let Some(job_id) = &job_id {
            if !self.jobs.contains_key(job_id) {
                return Err(format!("unknown brain job_id {job_id:?}"));
            }
        }
        let after = bounded_u64(object, "after", 0, u64::MAX, 0)?;
        let limit = bounded_u64(object, "limit", 1, 256, 100)? as usize;
        let events = self
            .events
            .iter()
            .filter(|event| {
                event.get("sequence").and_then(Value::as_u64).unwrap_or(0) > after
                    && job_id
                        .as_deref()
                        .is_none_or(|id| event.get("job_id").and_then(Value::as_str) == Some(id))
            })
            .take(limit)
            .cloned()
            .collect::<Vec<_>>();
        let next_after = events
            .last()
            .and_then(|event| event.get("sequence"))
            .and_then(Value::as_u64)
            .unwrap_or(after);
        Ok(json!({
            "schema": CONTROL_SCHEMA,
            "ok": true,
            "events": events,
            "after": after,
            "next_after": next_after,
            "head_digest": self.head_digest,
            "chain": "sha256_prev_digest",
            "retention": "metadata_only_hash_chained",
            "durability": durability_posture(),
        }))
    }

    pub(crate) fn job_approval(&mut self, arguments: &Value) -> Result<Value, String> {
        let object = object(arguments, "brain_job_approval")?;
        reject_unknown(
            object,
            &["job_id", "action", "reason", "authorization_digest"],
        )?;
        let job_id = text(object, "job_id", MAX_ID_BYTES)?;
        let action = text(object, "action", 32)?;
        if !matches!(action.as_str(), "request" | "approve" | "deny") {
            return Err("action must be request, approve, or deny".into());
        }
        let reason_digest = object
            .get("reason")
            .map(|value| {
                let reason = bounded_text(value, "reason", MAX_REASON_BYTES)?;
                digest_value(&json!(reason))
            })
            .transpose()?;
        let authorization_digest = optional_digest(object, "authorization_digest")?;
        let existing = self
            .jobs
            .get(&job_id)
            .ok_or_else(|| format!("unknown brain job_id {job_id:?}"))?;
        if matches!(action.as_str(), "approve" | "deny") && authorization_digest.is_none() {
            return Err(
                "approval decisions require authorization_digest from a caller-authenticated boundary"
                    .into(),
            );
        }
        let (next_state, event_type) = match (action.as_str(), existing.state.as_str()) {
            ("request", "queued") => ("waiting_approval", "job_approval_requested"),
            ("request", "waiting_approval") => ("waiting_approval", "job_approval_requested"),
            ("approve", "waiting_approval") => ("queued", "job_approval_granted"),
            ("deny", "waiting_approval") => ("cancelled", "job_approval_denied"),
            (_, state) => {
                return Err(format!(
                    "cannot apply approval action {action:?} while job is in state {state:?}"
                ))
            }
        };
        let mut details = json!({
            "action": action,
            "reason_digest": reason_digest,
            "authorization_digest": authorization_digest,
            "authorization_posture": "caller_authenticated_out_of_band; this server does not verify identity",
        });
        if event_type == "job_approval_granted" {
            details["execution"] = json!("not_started");
        }
        let event = self.append_event(&job_id, event_type, details)?;
        let sequence = event.get("sequence").and_then(Value::as_u64).unwrap_or(0);
        let job = self
            .jobs
            .get_mut(&job_id)
            .ok_or_else(|| "job disappeared during approval transition".to_string())?;
        job.state = next_state.into();
        job.updated_sequence = sequence;
        job.reason_digest = reason_digest;
        job.record_digest = event
            .get("event_digest")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        Ok(json!({
            "schema": CONTROL_SCHEMA,
            "ok": true,
            "job": job.to_value(),
            "event": event,
            "authorization": {
                "posture": "caller_authenticated_out_of_band",
                "verified_by_server": false,
                "execution": "not_started",
            },
            "durability": durability_posture(),
        }))
    }

    pub(crate) fn model_health(&mut self, arguments: &Value) -> Result<Value, String> {
        let object = object(arguments, "brain_model_health")?;
        reject_unknown(
            object,
            &[
                "operation",
                "provider",
                "model",
                "status",
                "latency_ms",
                "quality",
                "tokens",
                "registered",
                "credential_ready",
                "eligible",
            ],
        )?;
        let operation = text_with_default(object, "operation", "snapshot", 32)?;
        match operation.as_str() {
            "snapshot" => self.health_snapshot(object),
            "record" => self.health_record(object),
            _ => Err("operation must be snapshot or record".into()),
        }
    }

    pub(crate) fn replay_evaluate(&self, arguments: &Value) -> Result<Value, String> {
        let object = object(arguments, "brain_replay_evaluate")?;
        reject_unknown(
            object,
            &[
                "case_id",
                "domain",
                "capability",
                "risk_class",
                "evidence_digest",
                "signals",
                "references",
                "limitations",
                "required_signals",
                "signal_weights",
                "pass_threshold",
            ],
        )?;
        let case_id = text(object, "case_id", MAX_ID_BYTES)?;
        let domain = text(object, "domain", MAX_ID_BYTES)?;
        let capability = text(object, "capability", MAX_ID_BYTES)?;
        let risk_class = text(object, "risk_class", MAX_ID_BYTES)?;
        let supplied_digest = digest(object, "evidence_digest")?;
        let signals = normalized_signals(object.get("signals"))?;
        let references =
            bounded_digest_list(object.get("references"), "references", MAX_REFERENCE_COUNT)?;
        let limitations = bounded_text_list(
            object.get("limitations"),
            "limitations",
            MAX_LIMITATION_COUNT,
        )?;
        let profile = evaluator_profile(
            &domain,
            object.get("required_signals"),
            object.get("signal_weights"),
            object.get("pass_threshold"),
        )?;
        let evidence = json!({
            "schema": DOMAIN_EVALUATOR_SCHEMA,
            "domain": domain,
            "capability": capability,
            "risk_class": risk_class,
            "signals": signals,
            "references": references,
            "limitations": limitations,
            "retention": "value_only_digests_and_signal_scores",
        });
        let actual_digest = digest_value(&evidence)?;
        if supplied_digest != actual_digest {
            return Err(format!(
                "evidence_digest does not match normalized signals and metadata (expected {actual_digest})"
            ));
        }
        let mut weighted_total = 0.0;
        let mut observed_weight = 0.0;
        let mut missing = Vec::new();
        let mut below_threshold = Vec::new();
        for required in &profile.required_signals {
            match signals.get(required) {
                Some(value) if *value >= profile.pass_threshold => {}
                Some(_) => below_threshold.push(required.clone()),
                None => missing.push(required.clone()),
            }
        }
        for (signal, weight) in &profile.weights {
            if let Some(value) = signals.get(signal) {
                weighted_total += value * weight;
                observed_weight += weight;
            }
        }
        let reward = if observed_weight == 0.0 {
            0.0
        } else {
            weighted_total / observed_weight
        };
        let failed =
            !missing.is_empty() || !below_threshold.is_empty() || reward < profile.pass_threshold;
        let feedback_digest = digest_value(&json!({
            "case_id": case_id,
            "domain": domain,
            "evidence_digest": actual_digest,
            "missing": missing,
            "below_threshold": below_threshold,
            "reward": reward,
        }))?;
        Ok(json!({
            "schema": REPLAY_SCHEMA,
            "ok": true,
            "case_id": case_id,
            "domain": domain,
            "evaluator_id": profile.evaluator_id,
            "evaluator_version": "1",
            "evidence_digest": actual_digest,
            "reward": reward,
            "passed": !failed,
            "failed": failed,
            "failure_class": if failed { Some("domain_evidence_gate") } else { None::<&str> },
            "feedback_digest": feedback_digest,
            "replan_requested": failed,
            "replan_instruction": if failed { Some("Address the bounded domain evaluation gaps before retrying.") } else { None::<&str> },
            "execution": "offline_value_only_replay; no provider or domain tool invocation",
            "truth_authority": "caller_declared_normalized_signals",
            "retention": "digest_bound_metadata_only",
        }))
    }

    fn append_event(
        &mut self,
        job_id: &str,
        event_type: &str,
        details: Value,
    ) -> Result<Value, String> {
        if self.events.len() >= MAX_EVENTS {
            return Err(format!(
                "brain event capacity is exhausted at {MAX_EVENTS} events"
            ));
        }
        self.next_sequence = self.next_sequence.saturating_add(1);
        let sequence = self.next_sequence;
        let previous_digest = self.head_digest.clone();
        let payload = json!({
            "schema": EVENT_SCHEMA,
            "event": event_type,
            "job_id": job_id,
            "details": details,
        });
        let event_digest = digest_value(&json!({
            "schema": EVENT_SCHEMA,
            "event_type": event_type,
            "job_id": job_id,
            "payload": payload,
            "previous_digest": previous_digest,
            "sequence": sequence,
            "created_ns": sequence,
        }))?;
        let event = json!({
            "schema": EVENT_SCHEMA,
            "sequence": sequence,
            "event_type": event_type,
            "job_id": job_id,
            "payload": payload,
            "previous_digest": previous_digest,
            "event_digest": event_digest,
            "head_digest": event_digest,
            "created_ns": sequence,
            "retention": "metadata_only_hash_chained",
        });
        self.head_digest = event_digest.clone();
        self.events.push(event.clone());
        Ok(event)
    }

    fn health_record(&mut self, object: &Map<String, Value>) -> Result<Value, String> {
        let provider = text(object, "provider", MAX_ID_BYTES)?;
        let model = text(object, "model", MAX_ID_BYTES)?;
        let status = text(object, "status", 32)?;
        if !matches!(
            status.as_str(),
            "success" | "failure" | "timeout" | "rate_limited" | "circuit_open" | "unknown"
        ) {
            return Err(
                "status must be success, failure, timeout, rate_limited, circuit_open, or unknown"
                    .into(),
            );
        }
        let latency_ms = bounded_u64(object, "latency_ms", 0, 600_000, 0)?;
        let quality = bounded_f64(object, "quality", 0.0, 1.0)?;
        let tokens = bounded_u64(object, "tokens", 0, 1_000_000_000, 0)?;
        let registered = bool_with_default(object, "registered", true)?;
        let credential_ready = bool_with_default(object, "credential_ready", false)?;
        let eligible = bool_with_default(object, "eligible", registered && credential_ready)?;
        if self.health.len() >= MAX_HEALTH_ROWS
            && !self.health.contains_key(&(provider.clone(), model.clone()))
        {
            return Err(format!(
                "brain model health capacity is exhausted at {MAX_HEALTH_ROWS} rows"
            ));
        }
        let event = self.append_event(
            &format!("health:{provider}:{model}"),
            "model_health_observed",
            json!({
                "provider": provider,
                "model": model,
                "status": status,
                "latency_ms": latency_ms,
                "quality": quality,
                "tokens": tokens,
                "registered": registered,
                "credential_ready": credential_ready,
                "eligible": eligible,
            }),
        )?;
        let sequence = event.get("sequence").and_then(Value::as_u64).unwrap_or(0);
        let record = self
            .health
            .entry((provider.clone(), model.clone()))
            .or_default();
        record.observations = record.observations.saturating_add(1);
        record.total_latency_ms = record.total_latency_ms.saturating_add(latency_ms);
        record.last_latency_ms = latency_ms;
        record.last_status = status.clone();
        record.last_sequence = sequence;
        record.registered = registered;
        record.credential_ready = credential_ready;
        record.eligible = eligible;
        if let Some(quality) = quality {
            record.quality_sum += quality;
            record.quality_observations = record.quality_observations.saturating_add(1);
        }
        if status == "success" {
            record.successes = record.successes.saturating_add(1);
            record.consecutive_failures = 0;
        } else if status != "unknown" {
            record.failures = record.failures.saturating_add(1);
            record.consecutive_failures = record.consecutive_failures.saturating_add(1);
        }
        Ok(json!({
            "schema": CONTROL_SCHEMA,
            "ok": true,
            "operation": "record",
            "provider": provider,
            "model": model,
            "observed_tokens": tokens,
            "health": self.health_projection(),
            "retention": "value_only_provider_model_health",
            "durability": durability_posture(),
        }))
    }

    fn health_snapshot(&self, object: &Map<String, Value>) -> Result<Value, String> {
        let provider = object
            .get("provider")
            .map(|value| bounded_text(value, "provider", MAX_ID_BYTES))
            .transpose()?;
        let models = self
            .health_projection()
            .into_iter()
            .filter(|row| {
                provider
                    .as_deref()
                    .is_none_or(|name| row.get("provider").and_then(Value::as_str) == Some(name))
            })
            .collect::<Vec<_>>();
        let provider_health = self
            .provider_health_projection()
            .into_iter()
            .filter(|(name, _)| provider.as_deref().is_none_or(|selected| selected == name))
            .collect::<BTreeMap<_, _>>();
        Ok(json!({
            "schema": HEALTH_SCHEMA,
            "ok": true,
            "operation": "snapshot",
            "provider_health": provider_health,
            "models": models,
            "model_health": self.model_health_projection(),
            "retention": "value_only_provider_model_health",
            "durability": durability_posture(),
        }))
    }

    fn health_projection(&self) -> Vec<Value> {
        self.health
            .iter()
            .map(|((provider, model), record)| {
                json!({
                    "provider": provider,
                    "model": model,
                    "observations": record.observations,
                    "attempts": record.observations,
                    "successes": record.successes,
                    "failures": record.failures,
                    "consecutive_failures": record.consecutive_failures,
                    "average_latency_ms": if record.observations == 0 { 0.0 } else { record.total_latency_ms as f64 / record.observations as f64 },
                    "mean_latency_ms": if record.observations == 0 { 0.0 } else { record.total_latency_ms as f64 / record.observations as f64 },
                    "last_latency_ms": record.last_latency_ms,
                    "success_rate": if record.observations == 0 { 0.0 } else { record.successes as f64 / record.observations as f64 },
                    "average_quality": if record.quality_observations == 0 { Value::Null } else { json!(record.quality_sum / record.quality_observations as f64) },
                    "quality_observations": record.quality_observations,
                    "last_status": record.last_status,
                    "last_sequence": record.last_sequence,
                    "registered": record.registered,
                    "credential_ready": record.credential_ready,
                    "eligible": record.eligible,
                })
            })
            .collect()
    }

    fn model_health_projection(&self) -> BTreeMap<String, Value> {
        self.health
            .iter()
            .map(|((provider, model), record)| {
                (
                    format!("{provider}/{model}"),
                    json!({
                        "provider": provider,
                        "model": model,
                        "attempts": record.observations,
                        "successes": record.successes,
                        "failures": record.failures,
                        "success_rate": if record.observations == 0 { 0.0 } else { record.successes as f64 / record.observations as f64 },
                        "mean_latency_ms": if record.observations == 0 { 0.0 } else { record.total_latency_ms as f64 / record.observations as f64 },
                        "last_latency_ms": record.last_latency_ms,
                        "last_status": record.last_status,
                        "registered": record.registered,
                        "credential_ready": record.credential_ready,
                        "eligible": record.eligible,
                        "quality_mean": if record.quality_observations == 0 { Value::Null } else { json!(record.quality_sum / record.quality_observations as f64) },
                        "quality_observations": record.quality_observations,
                    }),
                )
            })
            .collect()
    }

    fn provider_health_projection(&self) -> BTreeMap<String, Value> {
        let mut result = BTreeMap::new();
        for ((provider, _model), record) in &self.health {
            let entry = result.entry(provider.clone()).or_insert_with(|| {
                json!({
                    "registered": record.registered,
                    "circuit": "closed",
                    "consecutive_failures": 0,
                    "attempts": 0,
                    "successes": 0,
                    "failures": 0,
                    "success_rate": 0.0,
                    "mean_latency_ms": 0.0,
                    "last_latency_ms": 0.0,
                    "credential_ready": record.credential_ready,
                    "eligible": record.eligible,
                })
            });
            let current_failures = entry
                .get("consecutive_failures")
                .and_then(Value::as_u64)
                .unwrap_or(0)
                .max(record.consecutive_failures);
            entry["registered"] = json!(
                entry
                    .get("registered")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
                    && record.registered
            );
            entry["credential_ready"] = json!(
                entry
                    .get("credential_ready")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
                    && record.credential_ready
            );
            entry["eligible"] = json!(
                entry
                    .get("eligible")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
                    && record.eligible
            );
            entry["consecutive_failures"] = json!(current_failures);
            let attempts = entry
                .get("attempts")
                .and_then(Value::as_u64)
                .unwrap_or(0)
                .saturating_add(record.observations);
            let successes = entry
                .get("successes")
                .and_then(Value::as_u64)
                .unwrap_or(0)
                .saturating_add(record.successes);
            let failures = entry
                .get("failures")
                .and_then(Value::as_u64)
                .unwrap_or(0)
                .saturating_add(record.failures);
            let previous_mean = entry
                .get("mean_latency_ms")
                .and_then(Value::as_f64)
                .unwrap_or(0.0);
            let previous_attempts = attempts.saturating_sub(record.observations);
            let row_mean = if record.observations == 0 {
                0.0
            } else {
                record.total_latency_ms as f64 / record.observations as f64
            };
            let weighted_mean = if attempts == 0 {
                0.0
            } else {
                (previous_mean * previous_attempts as f64 + row_mean * record.observations as f64)
                    / attempts as f64
            };
            entry["attempts"] = json!(attempts);
            entry["successes"] = json!(successes);
            entry["failures"] = json!(failures);
            entry["success_rate"] = json!(if attempts == 0 {
                0.0
            } else {
                successes as f64 / attempts as f64
            });
            entry["mean_latency_ms"] = json!(weighted_mean);
            entry["last_latency_ms"] = json!(record.last_latency_ms);
            if record.last_status == "circuit_open" || current_failures >= 3 {
                entry["circuit"] = json!("open");
                entry["eligible"] = json!(false);
            }
        }
        result
    }
}

impl JobRecord {
    fn to_value(&self) -> Value {
        json!({
            "schema": JOB_SCHEMA,
            "job_id": self.job_id,
            "idempotency_key_digest": digest_value(&json!(self.idempotency_key)).unwrap_or_default(),
            "spec_digest": self.spec_digest,
            "domain": self.domain,
            "capability": self.capability,
            "risk_class": self.risk_class,
            "priority": self.priority,
            "max_attempts": self.max_attempts,
            "state": self.state,
            "attempts": self.attempts,
            "lease_owner": Value::Null,
            "lease_expires_ns": Value::Null,
            "checkpoint_digest": self.checkpoint_digest,
            "side_effect_boundary": self.side_effect_boundary,
            "recovered_after_restart": false,
            "reason_digest": self.reason_digest,
            "created_sequence": self.created_sequence,
            "updated_sequence": self.updated_sequence,
            "record_digest": self.record_digest,
            "spec": "not_returned; caller resolver owns rehydration",
            "retention": "metadata_only_hash_chained",
        })
    }
}

struct EvaluatorProfile {
    evaluator_id: String,
    required_signals: Vec<String>,
    weights: BTreeMap<String, f64>,
    pass_threshold: f64,
}

fn evaluator_profile(
    domain: &str,
    required: Option<&Value>,
    weights: Option<&Value>,
    threshold: Option<&Value>,
) -> Result<EvaluatorProfile, String> {
    let (default_required, default_weights) = match domain {
        "engineering" => (
            vec!["schema_valid", "tests_passed", "evidence_complete"],
            vec![
                ("schema_valid", 1.0),
                ("tests_passed", 2.0),
                ("evidence_complete", 1.0),
            ],
        ),
        "research" => (
            vec![
                "evidence_traceable",
                "uncertainty_reported",
                "claim_scope_respected",
            ],
            vec![
                ("evidence_traceable", 2.0),
                ("uncertainty_reported", 1.0),
                ("claim_scope_respected", 2.0),
            ],
        ),
        "operations" => (
            vec![
                "safety_gate_passed",
                "approval_complete",
                "rollback_plan_present",
            ],
            vec![
                ("safety_gate_passed", 3.0),
                ("approval_complete", 2.0),
                ("rollback_plan_present", 1.0),
            ],
        ),
        "data" => (
            vec!["schema_valid", "lineage_complete", "quality_gate_passed"],
            vec![
                ("schema_valid", 1.0),
                ("lineage_complete", 2.0),
                ("quality_gate_passed", 2.0),
            ],
        ),
        "biomedical" => (
            vec![
                "boundary_compliant",
                "provenance_complete",
                "human_review_ready",
            ],
            vec![
                ("boundary_compliant", 3.0),
                ("provenance_complete", 2.0),
                ("human_review_ready", 2.0),
            ],
        ),
        _ => (Vec::new(), Vec::new()),
    };
    let required_signals = if let Some(value) = required {
        let values = value
            .as_array()
            .ok_or("required_signals must be an array")?;
        if values.is_empty() || values.len() > MAX_SIGNAL_COUNT {
            return Err(format!(
                "required_signals must contain 1..{MAX_SIGNAL_COUNT} entries"
            ));
        }
        values
            .iter()
            .map(|value| bounded_text(value, "required_signal", 128))
            .collect::<Result<Vec<_>, _>>()?
    } else if !default_required.is_empty() {
        default_required.into_iter().map(String::from).collect()
    } else {
        Vec::new()
    };
    let weights = if let Some(value) = weights {
        let values = value
            .as_object()
            .ok_or("signal_weights must be an object")?;
        if values.is_empty() || values.len() > MAX_SIGNAL_COUNT {
            return Err(format!(
                "signal_weights must contain 1..{MAX_SIGNAL_COUNT} entries"
            ));
        }
        values
            .iter()
            .map(|(name, value)| {
                let weight = finite_number(value, "signal weight")?;
                if weight <= 0.0 {
                    return Err("signal weights must be positive".into());
                }
                validate_safe_identifier(name, "signal weight")?;
                Ok((name.clone(), weight))
            })
            .collect::<Result<BTreeMap<_, _>, String>>()?
    } else if !default_weights.is_empty() {
        default_weights
            .into_iter()
            .map(|(name, weight)| (name.into(), weight))
            .collect()
    } else {
        if required_signals.is_empty() {
            return Err("unknown domains require required_signals and signal_weights".into());
        }
        required_signals
            .iter()
            .map(|name| (name.clone(), 1.0))
            .collect()
    };
    let pass_threshold = threshold
        .map(|value| finite_number(value, "pass_threshold"))
        .transpose()?
        .unwrap_or(1.0);
    if !(0.0..=1.0).contains(&pass_threshold) {
        return Err("pass_threshold must be within [0, 1]".into());
    }
    if required_signals.is_empty() {
        return Err("evaluator profile requires at least one required signal".into());
    }
    Ok(EvaluatorProfile {
        evaluator_id: format!("domain-{domain}-quality"),
        required_signals,
        weights,
        pass_threshold,
    })
}

fn normalized_signals(value: Option<&Value>) -> Result<BTreeMap<String, f64>, String> {
    let object = value
        .and_then(Value::as_object)
        .ok_or("signals must be an object")?;
    if object.is_empty() || object.len() > MAX_SIGNAL_COUNT {
        return Err(format!(
            "signals must contain 1..{MAX_SIGNAL_COUNT} entries"
        ));
    }
    object
        .iter()
        .map(|(name, value)| {
            validate_safe_identifier(name, "signal")?;
            let number = if let Some(boolean) = value.as_bool() {
                if boolean {
                    1.0
                } else {
                    0.0
                }
            } else {
                finite_number(value, "signal value")?
            };
            if !(0.0..=1.0).contains(&number) {
                return Err("signal values must be within [0, 1]".into());
            }
            Ok((name.clone(), number))
        })
        .collect()
}

fn bounded_digest_list(
    value: Option<&Value>,
    field: &str,
    maximum: usize,
) -> Result<Vec<String>, String> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let values = value
        .as_array()
        .ok_or_else(|| format!("{field} must be an array"))?;
    if values.len() > maximum {
        return Err(format!("{field} exceeds its {maximum}-item bound"));
    }
    values
        .iter()
        .map(|value| digest_value_text(value, field))
        .collect()
}

fn bounded_text_list(
    value: Option<&Value>,
    field: &str,
    maximum: usize,
) -> Result<Vec<String>, String> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let values = value
        .as_array()
        .ok_or_else(|| format!("{field} must be an array"))?;
    if values.len() > maximum {
        return Err(format!("{field} exceeds its {maximum}-item bound"));
    }
    values
        .iter()
        .map(|value| bounded_text(value, field, MAX_REASON_BYTES))
        .collect()
}

fn object<'a>(value: &'a Value, tool: &str) -> Result<&'a Map<String, Value>, String> {
    value
        .as_object()
        .ok_or_else(|| format!("{tool} arguments must be an object"))
}

fn reject_unknown(object: &Map<String, Value>, allowed: &[&str]) -> Result<(), String> {
    let allowed = allowed.iter().copied().collect::<BTreeSet<_>>();
    if let Some(name) = object.keys().find(|name| !allowed.contains(name.as_str())) {
        return Err(format!(
            "unsupported field {name:?}; secrets, prompts, task payloads, and provider responses are not accepted"
        ));
    }
    Ok(())
}

fn text(object: &Map<String, Value>, field: &str, maximum: usize) -> Result<String, String> {
    object
        .get(field)
        .ok_or_else(|| format!("{field} is required"))
        .and_then(|value| bounded_text(value, field, maximum))
}

fn text_with_default(
    object: &Map<String, Value>,
    field: &str,
    default: &str,
    maximum: usize,
) -> Result<String, String> {
    object.get(field).map_or_else(
        || Ok(default.to_string()),
        |value| bounded_text(value, field, maximum),
    )
}

fn bounded_text(value: &Value, field: &str, maximum: usize) -> Result<String, String> {
    let text = value
        .as_str()
        .ok_or_else(|| format!("{field} must be a string"))?;
    if text.trim().is_empty() || text.contains('\0') {
        return Err(format!("{field} must be non-empty and NUL-free"));
    }
    if text.len() > maximum {
        return Err(format!("{field} exceeds its {maximum}-byte bound"));
    }
    Ok(text.to_string())
}

fn digest(object: &Map<String, Value>, field: &str) -> Result<String, String> {
    object
        .get(field)
        .ok_or_else(|| format!("{field} is required"))
        .and_then(|value| digest_value_text(value, field))
}

fn optional_digest(object: &Map<String, Value>, field: &str) -> Result<Option<String>, String> {
    object
        .get(field)
        .map(|value| digest_value_text(value, field))
        .transpose()
}

fn digest_value_text(value: &Value, field: &str) -> Result<String, String> {
    let digest = bounded_text(value, field, 64)?;
    ContentHash::parse(digest.clone())
        .map_err(|_| format!("{field} must be a lowercase SHA-256 digest"))?;
    Ok(digest)
}

fn digest_value(value: &Value) -> Result<String, String> {
    ContentHash::of_value(value)
        .map(|digest| digest.to_string())
        .map_err(|error| format!("cannot hash control-plane value: {error}"))
}

fn bounded_u64(
    object: &Map<String, Value>,
    field: &str,
    minimum: u64,
    maximum: u64,
    default: u64,
) -> Result<u64, String> {
    let value = object.get(field).and_then(Value::as_u64).unwrap_or(default);
    if object.contains_key(field) && object.get(field).and_then(Value::as_u64).is_none() {
        return Err(format!("{field} must be an unsigned integer"));
    }
    if value < minimum || value > maximum {
        return Err(format!("{field} must be within [{minimum}, {maximum}]"));
    }
    Ok(value)
}

fn bounded_f64(
    object: &Map<String, Value>,
    field: &str,
    minimum: f64,
    maximum: f64,
) -> Result<Option<f64>, String> {
    object
        .get(field)
        .map(|value| {
            let number = finite_number(value, field)?;
            if number < minimum || number > maximum {
                return Err(format!("{field} must be within [{minimum}, {maximum}]"));
            }
            Ok(number)
        })
        .transpose()
}

fn finite_number(value: &Value, field: &str) -> Result<f64, String> {
    let number = value
        .as_f64()
        .ok_or_else(|| format!("{field} must be a finite number"))?;
    if !number.is_finite() {
        return Err(format!("{field} must be a finite number"));
    }
    Ok(number)
}

fn bool_with_default(
    object: &Map<String, Value>,
    field: &str,
    default: bool,
) -> Result<bool, String> {
    object.get(field).map_or_else(
        || Ok(default),
        |value| {
            value
                .as_bool()
                .ok_or_else(|| format!("{field} must be a boolean"))
        },
    )
}

fn validate_safe_identifier(value: &str, field: &str) -> Result<(), String> {
    if value.is_empty()
        || value.len() > 128
        || !value.as_bytes()[0].is_ascii_alphabetic()
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b'-'))
    {
        return Err(format!("{field} must be a safe bounded identifier"));
    }
    Ok(())
}

fn durability_posture() -> Value {
    json!({
        "scope": "mcp_process",
        "restart": "caller_must_rehydrate_from_durable_job_store",
        "secrets": "never_retained",
    })
}

pub(crate) fn tool_definitions() -> Vec<Value> {
    vec![
        json!({
            "name": "brain_job_submit",
            "description": "Admit a rehydratable autonomous-brain job identity into the bounded MCP control plane. Accepts only metadata and digests; never accepts a prompt, task payload, provider response, credential, or API key. Idempotency is bound to the spec digest.",
            "inputSchema": {
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "job_id": {"type": "string", "maxLength": 256},
                    "idempotency_key": {"type": "string", "minLength": 1, "maxLength": 256},
                    "spec_digest": {"type": "string", "pattern": "^[0-9a-f]{64}$"},
                    "domain": {"type": "string", "maxLength": 256},
                    "capability": {"type": "string", "maxLength": 256},
                    "risk_class": {"type": "string", "maxLength": 256},
                    "priority": {"type": "integer", "minimum": 0, "maximum": 255},
                    "max_attempts": {"type": "integer", "minimum": 1, "maximum": 8},
                    "checkpoint_digest": {"type": ["string", "null"], "pattern": "^[0-9a-f]{64}$"}
                },
                "required": ["idempotency_key", "spec_digest", "domain", "capability", "risk_class"]
            }
        }),
        json!({
            "name": "brain_job_status",
            "description": "Read one value-only autonomous-brain job status. The task, prompt, plan, provider response, credential, and lease secret are never returned.",
            "inputSchema": {"type": "object", "additionalProperties": false, "properties": {"job_id": {"type": "string", "maxLength": 256}}, "required": ["job_id"]}
        }),
        json!({
            "name": "brain_job_events",
            "description": "Read a bounded cursor page from the metadata-only hash-chained brain journal. Events contain digests and state transitions, not raw work or secrets.",
            "inputSchema": {"type": "object", "additionalProperties": false, "properties": {"job_id": {"type": "string", "maxLength": 256}, "after": {"type": "integer", "minimum": 0}, "limit": {"type": "integer", "minimum": 1, "maximum": 256}}, "required": []}
        }),
        json!({
            "name": "brain_job_approval",
            "description": "Request, approve, or deny a job's approval checkpoint. Approve and deny require a caller-authenticated authorization proof digest; this transport does not verify identity and never dispatches execution.",
            "inputSchema": {"type": "object", "additionalProperties": false, "properties": {"job_id": {"type": "string", "maxLength": 256}, "action": {"type": "string", "enum": ["request", "approve", "deny"]}, "reason": {"type": "string", "maxLength": 2048}, "authorization_digest": {"type": "string", "pattern": "^[0-9a-f]{64}$"}}, "required": ["job_id", "action"]}
        }),
        json!({
            "name": "brain_model_health",
            "description": "Record or inspect bounded provider/model health. Only status, latency, quality, usage counts, registration posture, and credential readiness booleans are accepted; no credential material or provider payload is accepted.",
            "inputSchema": {"type": "object", "additionalProperties": false, "properties": {"operation": {"type": "string", "enum": ["snapshot", "record"]}, "provider": {"type": "string", "maxLength": 256}, "model": {"type": "string", "maxLength": 256}, "status": {"type": "string", "enum": ["success", "failure", "timeout", "rate_limited", "circuit_open", "unknown"]}, "latency_ms": {"type": "integer", "minimum": 0, "maximum": 600000}, "quality": {"type": "number", "minimum": 0, "maximum": 1}, "tokens": {"type": "integer", "minimum": 0, "maximum": 1000000000}, "registered": {"type": "boolean"}, "credential_ready": {"type": "boolean"}, "eligible": {"type": "boolean"}}, "required": []}
        }),
        json!({
            "name": "brain_replay_evaluate",
            "description": "Run a deterministic offline evaluator over caller-normalized bounded signals for engineering, research, operations, data, biomedical, or an explicitly supplied domain profile. The evidence digest must bind the exact signal packet. No provider, task, prompt, raw evidence, credential, or domain tool is invoked.",
            "inputSchema": {"type": "object", "additionalProperties": false, "properties": {"case_id": {"type": "string", "maxLength": 256}, "domain": {"type": "string", "maxLength": 256}, "capability": {"type": "string", "maxLength": 256}, "risk_class": {"type": "string", "maxLength": 256}, "evidence_digest": {"type": "string", "pattern": "^[0-9a-f]{64}$"}, "signals": {"type": "object", "maxProperties": 64}, "references": {"type": "array", "maxItems": 64, "items": {"type": "string", "pattern": "^[0-9a-f]{64}$"}}, "limitations": {"type": "array", "maxItems": 32, "items": {"type": "string", "maxLength": 2048}}, "required_signals": {"type": "array", "maxItems": 64, "items": {"type": "string", "maxLength": 128}}, "signal_weights": {"type": "object", "maxProperties": 64}, "pass_threshold": {"type": "number", "minimum": 0, "maximum": 1}}, "required": ["case_id", "domain", "capability", "risk_class", "evidence_digest", "signals"]}
        }),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn submission_is_idempotent_and_approval_requires_external_proof() {
        let mut state = BrainControlState::default();
        let arguments = json!({
            "idempotency_key": "request-001",
            "spec_digest": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "domain": "engineering",
            "capability": "code_change",
            "risk_class": "reversible",
        });
        let first = state.submit_job(&arguments).unwrap();
        assert_eq!(first["created"], json!(true));
        let job_id = first["job"]["job_id"].as_str().unwrap().to_string();
        assert!(first["job"].get("prompt").is_none());
        let second = state.submit_job(&arguments).unwrap();
        assert_eq!(second["idempotent"], json!(true));
        assert_eq!(second["job"]["job_id"], json!(job_id.clone()));

        let requested = state
            .job_approval(&json!({"job_id": job_id, "action": "request"}))
            .unwrap();
        assert_eq!(requested["job"]["state"], json!("waiting_approval"));
        assert!(state
            .job_approval(&json!({"job_id": job_id, "action": "approve"}))
            .is_err());
    }

    #[test]
    fn replay_digest_matches_python_canonical_value_and_rejects_unknown_fields() {
        let mut signals = BTreeMap::new();
        signals.insert("schema_valid", 1.0);
        signals.insert("tests_passed", 1.0);
        signals.insert("evidence_complete", 1.0);
        let evidence = json!({
            "schema": DOMAIN_EVALUATOR_SCHEMA,
            "domain": "engineering",
            "capability": "code_change",
            "risk_class": "reversible",
            "signals": signals,
            "references": [],
            "limitations": [],
            "retention": "value_only_digests_and_signal_scores",
        });
        let evidence_digest = digest_value(&evidence).unwrap();
        let replay = BrainControlState::default()
            .replay_evaluate(&json!({
                "case_id": "case-001",
                "domain": "engineering",
                "capability": "code_change",
                "risk_class": "reversible",
                "evidence_digest": evidence_digest,
                "signals": {
                    "schema_valid": true,
                    "tests_passed": true,
                    "evidence_complete": true,
                },
            }))
            .unwrap();
        assert_eq!(replay["passed"], json!(true));
        assert!(BrainControlState::default()
            .replay_evaluate(&json!({
                "case_id": "case-002",
                "domain": "engineering",
                "capability": "code_change",
                "risk_class": "reversible",
                "evidence_digest": evidence_digest,
                "signals": {
                    "schema_valid": true,
                    "tests_passed": true,
                    "evidence_complete": true,
                },
                "api_key": "refused",
            }))
            .is_err());
    }

    #[test]
    fn health_observations_are_hash_chained_and_project_provider_posture() {
        let mut state = BrainControlState::default();
        let result = state
            .model_health(&json!({
                "operation": "record",
                "provider": "openai",
                "model": "gpt-test",
                "status": "success",
                "latency_ms": 100,
                "quality": 0.9,
                "credential_ready": true,
            }))
            .unwrap();
        assert_eq!(result["health"][0]["provider"], json!("openai"));
        let events = state.job_events(&json!({"limit": 4})).unwrap();
        assert_eq!(events["events"].as_array().unwrap().len(), 1);
        assert_eq!(
            events["events"][0]["event_type"],
            json!("model_health_observed")
        );
        assert_eq!(events["chain"], json!("sha256_prev_digest"));
        assert_eq!(
            state
                .health_snapshot(json!({"operation": "snapshot"}).as_object().unwrap())
                .unwrap()["provider_health"]["openai"]["credential_ready"],
            json!(true)
        );
    }

    #[test]
    fn keyed_outcome_record_replays_without_double_credit_and_rejects_contract_changes() {
        let mut state = BrainControlState::default();
        let arguments = json!({
            "run": {
                "run_id": "run-001",
                "selection_digest": "a".repeat(64),
                "prompt_digest": "b".repeat(64),
                "plan_digest": "c".repeat(64),
                "provider": "openai",
                "model": "test-model",
                "outcome_digest": "d".repeat(64)
            },
            "assessment": {
                "evaluator_id": "quality",
                "evaluator_version": "1",
                "reward": 0.8,
                "passed": true,
                "failed": false
            },
            "bandit_state": {
                "schema": "bioprism-brain-bandit/0.1",
                "generation": 0,
                "arms": [{
                    "arm_id": "openai/test-model",
                    "pulls": 0,
                    "reward_sum": 0.0,
                    "failures": 0,
                    "disabled": false
                }]
            },
            "arm_id": "openai/test-model",
            "idempotency_key": "episode:run-001"
        });
        let first = state.outcome_record(&arguments).unwrap();
        assert_eq!(first["idempotent"], json!(false));
        let mut retry = arguments.clone();
        retry["bandit_state"] = first["next_state"].clone();
        let replay = state.outcome_record(&retry).unwrap();
        assert_eq!(replay["idempotent"], json!(true));
        assert_eq!(replay["next_state"], first["next_state"]);

        retry["assessment"]["reward"] = json!(0.2);
        assert!(state.outcome_record(&retry).is_err());
    }

    #[test]
    fn first_outcome_record_hydrates_an_unseen_arm_at_the_mcp_boundary() {
        let mut state = BrainControlState::default();
        let arguments = json!({
            "run": {
                "run_id": "run-first-seen",
                "selection_digest": "a".repeat(64),
                "prompt_digest": "b".repeat(64),
                "plan_digest": "c".repeat(64),
                "provider": "anthropic",
                "model": "new-model",
                "outcome_digest": "d".repeat(64)
            },
            "assessment": {
                "evaluator_id": "quality",
                "evaluator_version": "1",
                "reward": 0.6,
                "passed": true,
                "failed": false
            },
            "bandit_state": {
                "schema": "bioprism-brain-bandit/0.1",
                "generation": 0,
                "arms": []
            },
            "arm_id": "anthropic/new-model"
        });

        let result = state.outcome_record(&arguments).unwrap();
        assert_eq!(result["learning_evidence"]["next_generation"], json!(1));
        assert_eq!(result["next_state"]["generation"], json!(1));
        assert_eq!(
            result["next_state"]["arms"][0]["arm_id"],
            json!("anthropic/new-model")
        );
        assert_eq!(result["next_state"]["arms"][0]["pulls"], json!(1));
        assert_eq!(result["next_state"]["arms"][0]["reward_sum"], json!(0.6));
    }
}
