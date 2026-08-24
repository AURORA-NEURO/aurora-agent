//! Metadata-only restart state for the grant-authorised autopilot.
//!
//! A drive history contains the mission and report JSON needed by the pure planner, but those
//! values may contain private task text, arguments, provider output, or evidence.  This module
//! therefore separates *checkpoint identity* from *rehydration material*: the checkpoint stores
//! only content digests, bounded statuses, counts, and reconciliation posture.  A restarted caller
//! must supply the original mission and freshly rehydrated [`AttemptRecord`] values; the restore
//! function proves that they match the checkpoint before another dispatch can be planned.

use crate::error::AutopilotError;
use crate::grant::AutonomyGrant;
use crate::history::{AttemptRecord, DriveHistory};
use bioprism_ids::{to_canonical_string, ContentHash};
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;

pub const AUTOPILOT_CHECKPOINT_SCHEMA: &str = "bioprism-autopilot-checkpoint/0.1";
pub const AUTOPILOT_CHECKPOINT_RETENTION: &str = "metadata_only_autopilot;missions_arguments_provider_output_credentials_and_evidence_not_retained";
pub const AUTOPILOT_CHECKPOINT_MAX_ATTEMPTS: usize = 16;
pub const AUTOPILOT_CHECKPOINT_MAX_BYTES: usize = 2_000_000;
pub const AUTOPILOT_CHECKPOINT_MAX_STEP_IDS: usize = 128;

const CHECKPOINT_KEYS: &[&str] = &[
    "schema",
    "grant_digest",
    "base_mission_id",
    "base_mission_digest",
    "base_step_count",
    "attempts",
    "attempts_used",
    "max_attempts",
    "history_digest",
    "generation",
    "previous_snapshot_digest",
    "retention",
    "secret_material",
];
const ATTEMPT_KEYS: &[&str] = &[
    "attempt_index",
    "kind",
    "mission_digest",
    "report_digest",
    "step_count",
    "step_ids_digest",
    "result_count",
    "result_status_counts",
    "result_metadata_digest",
    "reconciliation",
    "dispatch_error_digest",
];
const RECONCILIATION_KEYS: &[&str] = &[
    "present",
    "digest",
    "completion_status",
    "integrity_valid",
    "scope",
];

fn invalid(reason: impl Into<String>) -> AutopilotError {
    AutopilotError::InvalidCheckpoint {
        reason: reason.into(),
    }
}

fn digest_value(value: &Value) -> Result<String, AutopilotError> {
    ContentHash::of_value(value)
        .map(|digest| digest.to_string())
        .map_err(|error| AutopilotError::Canonicalisation {
            reason: error.to_string(),
        })
}

fn require_digest(object: &Map<String, Value>, field: &str) -> Result<String, AutopilotError> {
    let value = object
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| invalid(format!("{field} must be a lowercase SHA-256 digest")))?;
    ContentHash::parse(value.to_owned())
        .map(|_| value.to_owned())
        .map_err(|_| invalid(format!("{field} must be a lowercase SHA-256 digest")))
}

fn optional_digest(
    object: &Map<String, Value>,
    field: &str,
) -> Result<Option<String>, AutopilotError> {
    match object.get(field) {
        Some(Value::Null) | None => Ok(None),
        Some(Value::String(value)) => ContentHash::parse(value.clone())
            .map(|_| Some(value.clone()))
            .map_err(|_| {
                invalid(format!(
                    "{field} must be null or a lowercase SHA-256 digest"
                ))
            }),
        _ => Err(invalid(format!("{field} must be null or a digest string"))),
    }
}

fn exact_keys(
    object: &Map<String, Value>,
    expected: &[&str],
    name: &str,
) -> Result<(), AutopilotError> {
    if object.len() != expected.len() || expected.iter().any(|key| !object.contains_key(*key)) {
        return Err(invalid(format!("{name} has unsupported or missing fields")));
    }
    Ok(())
}

fn bounded_text(value: &Value, field: &str, maximum: usize) -> Result<String, AutopilotError> {
    let text = value
        .as_str()
        .filter(|text| !text.trim().is_empty())
        .ok_or_else(|| invalid(format!("{field} must be non-empty text")))?;
    if text.contains('\0') || text.len() > maximum {
        return Err(invalid(format!("{field} is outside its text bound")));
    }
    Ok(text.to_owned())
}

fn status_counts(value: &Value) -> Result<BTreeMap<String, usize>, AutopilotError> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid("result_status_counts must be an object"))?;
    if object.len() > 16 {
        return Err(invalid("result_status_counts has too many statuses"));
    }
    let mut result = BTreeMap::new();
    for (status, count) in object {
        bounded_text(&Value::String(status.clone()), "result status", 64)?;
        let count = count
            .as_u64()
            .and_then(|count| usize::try_from(count).ok())
            .ok_or_else(|| invalid("result status counts must be non-negative integers"))?;
        if count > AUTOPILOT_CHECKPOINT_MAX_STEP_IDS {
            return Err(invalid("result status count exceeds the step bound"));
        }
        result.insert(status.clone(), count);
    }
    Ok(result)
}

fn reconciliation_projection(attempt: &AttemptRecord) -> Value {
    match attempt.reconciliation_summary() {
        Some((status, integrity_valid, digest)) => json!({
            "present": true,
            "digest": digest,
            "completion_status": status,
            "integrity_valid": integrity_valid,
            "scope": attempt.kind().reconciliation_scope(),
        }),
        None => json!({
            "present": false,
            "digest": Value::Null,
            "completion_status": Value::Null,
            "integrity_valid": false,
            "scope": Value::Null,
        }),
    }
}

/// Produce the metadata retained for one private attempt.
pub fn attempt_checkpoint_projection(attempt: &AttemptRecord) -> Result<Value, AutopilotError> {
    let step_ids = attempt.dispatched_step_ids();
    if step_ids.is_empty() || step_ids.len() > AUTOPILOT_CHECKPOINT_MAX_STEP_IDS {
        return Err(invalid("attempt step count is outside its bound"));
    }
    let mut result_status_counts = BTreeMap::<String, usize>::new();
    let mut result_rows = Vec::new();
    if let Some(report) = attempt.parsed_report() {
        for result in &report.results {
            *result_status_counts
                .entry(result.status.clone())
                .or_default() += 1;
            result_rows.push(json!({
                "status": result.status,
                "required": result.required,
                "arguments_digest": result.arguments_digest,
                "wire_digest": result.wire.as_ref().map(digest_value).transpose()?,
            }));
        }
    }
    let result_count = result_rows.len();
    let result_metadata_digest = digest_value(&json!({ "rows": result_rows }))?;
    let dispatch_error_digest = attempt
        .dispatch_error()
        .map(|error| digest_value(&Value::String(error.to_owned())))
        .transpose()?;
    Ok(json!({
        "attempt_index": 0,
        "kind": attempt.kind().as_str(),
        "mission_digest": attempt.mission_digest(),
        "report_digest": attempt.report_digest(),
        "step_count": step_ids.len(),
        "step_ids_digest": digest_value(&json!(step_ids))?,
        "result_count": result_count,
        "result_status_counts": result_status_counts,
        "result_metadata_digest": result_metadata_digest,
        "reconciliation": reconciliation_projection(attempt),
        "dispatch_error_digest": dispatch_error_digest,
    }))
}

fn history_projection(
    base_mission_digest: &str,
    attempts: &[Value],
) -> Result<String, AutopilotError> {
    digest_value(&json!({
        "base_mission_digest": base_mission_digest,
        "attempts": attempts,
    }))
}

/// Seal the current private drive history into a bounded, digest-only checkpoint.
pub fn seal_autopilot_checkpoint(
    grant: &AutonomyGrant,
    history: &DriveHistory,
    generation: u64,
    previous_snapshot_digest: Option<&str>,
) -> Result<Value, AutopilotError> {
    if generation == 0 {
        return Err(invalid("generation must be positive"));
    }
    if history.dispatches_used() > AUTOPILOT_CHECKPOINT_MAX_ATTEMPTS {
        return Err(invalid("history exceeds the checkpoint attempt bound"));
    }
    if let Some(previous) = previous_snapshot_digest {
        ContentHash::parse(previous.to_owned())
            .map_err(|_| invalid("previous_snapshot_digest is malformed"))?;
    }
    let grant_digest = grant.digest()?;
    let base_mission_digest = digest_value(history.base_mission())?;
    let attempts = history
        .attempts()
        .iter()
        .map(attempt_checkpoint_projection)
        .collect::<Result<Vec<_>, _>>()?;
    let attempts = attempts
        .into_iter()
        .enumerate()
        .map(|(index, mut attempt)| {
            attempt["attempt_index"] = json!(index + 1);
            attempt
        })
        .collect::<Vec<_>>();
    let body = json!({
        "schema": AUTOPILOT_CHECKPOINT_SCHEMA,
        "grant_digest": grant_digest,
        "base_mission_id": history.parsed_base().mission_id,
        "base_mission_digest": base_mission_digest,
        "base_step_count": history.parsed_base().steps.len(),
        "attempts": attempts,
        "attempts_used": history.dispatches_used(),
        "max_attempts": grant.max_attempts(),
        "history_digest": history_projection(&base_mission_digest, &attempts)?,
        "generation": generation,
        "previous_snapshot_digest": previous_snapshot_digest,
        "retention": AUTOPILOT_CHECKPOINT_RETENTION,
        "secret_material": "never_returned",
    });
    let snapshot = json!({
        "schema": AUTOPILOT_CHECKPOINT_SCHEMA,
        "grant_digest": body["grant_digest"].clone(),
        "base_mission_id": body["base_mission_id"].clone(),
        "base_mission_digest": body["base_mission_digest"].clone(),
        "base_step_count": body["base_step_count"].clone(),
        "attempts": body["attempts"].clone(),
        "attempts_used": body["attempts_used"].clone(),
        "max_attempts": body["max_attempts"].clone(),
        "history_digest": body["history_digest"].clone(),
        "generation": body["generation"].clone(),
        "previous_snapshot_digest": body["previous_snapshot_digest"].clone(),
        "retention": body["retention"].clone(),
        "secret_material": body["secret_material"].clone(),
        "snapshot_digest": digest_value(&body)?,
    });
    validate_autopilot_checkpoint(&snapshot)
}

fn validate_reconciliation(value: &Value) -> Result<(), AutopilotError> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid("attempt reconciliation must be an object"))?;
    exact_keys(object, RECONCILIATION_KEYS, "reconciliation")?;
    let present = object
        .get("present")
        .and_then(Value::as_bool)
        .ok_or_else(|| invalid("reconciliation.present must be boolean"))?;
    let digest = optional_digest(object, "digest")?;
    let status = match object.get("completion_status") {
        Some(Value::Null) | None => None,
        Some(value) => Some(bounded_text(value, "completion_status", 64)?),
    };
    let scope = match object.get("scope") {
        Some(Value::Null) | None => None,
        Some(value) => Some(bounded_text(value, "reconciliation.scope", 64)?),
    };
    let integrity = object
        .get("integrity_valid")
        .and_then(Value::as_bool)
        .ok_or_else(|| invalid("reconciliation.integrity_valid must be boolean"))?;
    if present != (status.is_some() && scope.is_some()) {
        return Err(invalid("reconciliation presence does not match its fields"));
    }
    if !present && (digest.is_some() || status.is_some() || scope.is_some() || integrity) {
        return Err(invalid(
            "absent reconciliation cannot carry completion, digest, scope, or integrity",
        ));
    }
    Ok(())
}

fn validate_attempt(value: &Value, expected_index: usize) -> Result<(), AutopilotError> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid("checkpoint attempt must be an object"))?;
    exact_keys(object, ATTEMPT_KEYS, "checkpoint attempt")?;
    if object
        .get("attempt_index")
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
        != Some(expected_index)
    {
        return Err(invalid("checkpoint attempt indexes are not contiguous"));
    }
    let kind = bounded_text(object.get("kind").unwrap(), "attempt.kind", 16)?;
    if kind != "full" && kind != "repair" {
        return Err(invalid("attempt.kind is invalid"));
    }
    require_digest(object, "mission_digest")?;
    optional_digest(object, "report_digest")?;
    let step_count = object
        .get("step_count")
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
        .ok_or_else(|| invalid("attempt.step_count must be an integer"))?;
    if !(1..=AUTOPILOT_CHECKPOINT_MAX_STEP_IDS).contains(&step_count) {
        return Err(invalid("attempt.step_count is outside its bound"));
    }
    require_digest(object, "step_ids_digest")?;
    let result_count = object
        .get("result_count")
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
        .ok_or_else(|| invalid("attempt.result_count must be an integer"))?;
    if result_count > AUTOPILOT_CHECKPOINT_MAX_STEP_IDS {
        return Err(invalid("attempt.result_count is outside its bound"));
    }
    let status_counts = status_counts(object.get("result_status_counts").unwrap())?;
    if status_counts.values().sum::<usize>() != result_count {
        return Err(invalid(
            "result status counts do not add up to result_count",
        ));
    }
    require_digest(object, "result_metadata_digest")?;
    validate_reconciliation(object.get("reconciliation").unwrap())?;
    optional_digest(object, "dispatch_error_digest")?;
    Ok(())
}

/// Strictly validate a checkpoint and its content digest.
pub fn validate_autopilot_checkpoint(value: &Value) -> Result<Value, AutopilotError> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid("checkpoint must be a JSON object"))?;
    let mut checkpoint_keys = CHECKPOINT_KEYS.to_vec();
    checkpoint_keys.push("snapshot_digest");
    exact_keys(object, &checkpoint_keys, "checkpoint")?;
    if object.get("schema").and_then(Value::as_str) != Some(AUTOPILOT_CHECKPOINT_SCHEMA)
        || object.get("retention").and_then(Value::as_str) != Some(AUTOPILOT_CHECKPOINT_RETENTION)
        || object.get("secret_material").and_then(Value::as_str) != Some("never_returned")
    {
        return Err(invalid("checkpoint retention markers are invalid"));
    }
    let grant_digest = require_digest(object, "grant_digest")?;
    let base_mission_id = bounded_text(
        object.get("base_mission_id").unwrap(),
        "base_mission_id",
        256,
    )?;
    let base_mission_digest = require_digest(object, "base_mission_digest")?;
    let base_step_count = object
        .get("base_step_count")
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
        .ok_or_else(|| invalid("base_step_count must be an integer"))?;
    if !(1..=AUTOPILOT_CHECKPOINT_MAX_STEP_IDS).contains(&base_step_count) {
        return Err(invalid("base_step_count is outside its bound"));
    }
    let attempts = object
        .get("attempts")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("attempts must be an array"))?;
    if attempts.len() > AUTOPILOT_CHECKPOINT_MAX_ATTEMPTS {
        return Err(invalid("attempts exceed the checkpoint bound"));
    }
    for (index, attempt) in attempts.iter().enumerate() {
        validate_attempt(attempt, index + 1)?;
    }
    let attempts_used = object
        .get("attempts_used")
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
        .ok_or_else(|| invalid("attempts_used must be an integer"))?;
    let max_attempts = object
        .get("max_attempts")
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
        .ok_or_else(|| invalid("max_attempts must be an integer"))?;
    if attempts_used != attempts.len()
        || !(1..=AUTOPILOT_CHECKPOINT_MAX_ATTEMPTS).contains(&max_attempts)
        || attempts_used > max_attempts
    {
        return Err(invalid("checkpoint attempt accounting is inconsistent"));
    }
    let history_digest = require_digest(object, "history_digest")?;
    if history_digest != history_projection(&base_mission_digest, attempts)? {
        return Err(invalid(
            "history_digest does not match the retained attempt projections",
        ));
    }
    let generation = object
        .get("generation")
        .and_then(Value::as_u64)
        .ok_or_else(|| invalid("generation must be a positive integer"))?;
    if generation == 0 {
        return Err(invalid("generation must be positive"));
    }
    optional_digest(object, "previous_snapshot_digest")?;
    let snapshot_digest = require_digest(object, "snapshot_digest")?;
    let mut without_digest = value.clone();
    without_digest
        .as_object_mut()
        .expect("object checked above")
        .remove("snapshot_digest");
    if snapshot_digest != digest_value(&without_digest)? {
        return Err(invalid(
            "snapshot_digest does not match the checkpoint body",
        ));
    }
    let canonical =
        to_canonical_string(value).map_err(|error| AutopilotError::Canonicalisation {
            reason: error.to_string(),
        })?;
    if canonical.as_bytes().len() > AUTOPILOT_CHECKPOINT_MAX_BYTES {
        return Err(invalid("checkpoint exceeds its byte bound"));
    }
    let _ = (grant_digest, base_mission_id);
    Ok(value.clone())
}

/// Rehydrate private attempts and prove that they match a validated checkpoint.
pub fn restore_drive_history(
    grant: &AutonomyGrant,
    checkpoint: &Value,
    base_mission: Value,
    attempts: Vec<AttemptRecord>,
) -> Result<DriveHistory, AutopilotError> {
    let checkpoint = validate_autopilot_checkpoint(checkpoint)?;
    let object = checkpoint.as_object().expect("validated checkpoint object");
    if require_digest(object, "grant_digest")? != grant.digest()? {
        return Err(invalid(
            "checkpoint grant digest does not match the supplied grant",
        ));
    }
    let history = DriveHistory::from_attempts(base_mission, attempts)?;
    let base_digest = digest_value(history.base_mission())?;
    if require_digest(object, "base_mission_digest")? != base_digest
        || object.get("base_mission_id").and_then(Value::as_str)
            != Some(history.parsed_base().mission_id.as_str())
        || object.get("base_step_count").and_then(Value::as_u64)
            != Some(history.parsed_base().steps.len() as u64)
    {
        return Err(invalid(
            "checkpoint does not match the supplied base mission",
        ));
    }
    let rows = object
        .get("attempts")
        .and_then(Value::as_array)
        .expect("validated attempts array");
    if rows.len() != history.attempts().len() {
        return Err(invalid(
            "checkpoint attempt count does not match rehydrated attempts",
        ));
    }
    for (row, attempt) in rows.iter().zip(history.attempts()) {
        let mut actual = attempt_checkpoint_projection(attempt)?;
        let index = row
            .get("attempt_index")
            .cloned()
            .ok_or_else(|| invalid("checkpoint attempt index is missing"))?;
        actual["attempt_index"] = index;
        if actual != *row {
            return Err(invalid(
                "rehydrated attempt metadata does not match the checkpoint",
            ));
        }
    }
    Ok(history)
}

/// Minimal caller-owned text storage contract.
pub trait AutopilotCheckpointStore {
    fn read(&mut self) -> Result<Option<String>, String>;
    fn write(&mut self, value: String) -> Result<(), String>;
}

/// Text storage with stale-writer compare-and-swap.
pub trait TransactionalAutopilotCheckpointStore: AutopilotCheckpointStore {
    fn write_if_unchanged(
        &mut self,
        expected_snapshot_digest: Option<&str>,
        value: String,
    ) -> Result<bool, String>;
}

pub trait AutopilotCheckpointPersistence {
    fn read_snapshot(&mut self) -> Result<Option<Value>, AutopilotError>;
    fn write_snapshot(&mut self, snapshot: &Value) -> Result<(), AutopilotError>;
}

pub trait TransactionalAutopilotCheckpointPersistence: AutopilotCheckpointPersistence {
    fn write_snapshot_if_unchanged(
        &mut self,
        expected_snapshot_digest: Option<&str>,
        snapshot: &Value,
    ) -> Result<bool, AutopilotError>;
}

fn store_error(error: String) -> AutopilotError {
    AutopilotError::Persistence { reason: error }
}

fn canonical_snapshot(snapshot: &Value, max_bytes: usize) -> Result<String, AutopilotError> {
    let normalized = validate_autopilot_checkpoint(snapshot)?;
    let canonical =
        to_canonical_string(&normalized).map_err(|error| AutopilotError::Canonicalisation {
            reason: error.to_string(),
        })?;
    if canonical.as_bytes().len() > max_bytes {
        return Err(invalid("checkpoint exceeds the configured byte bound"));
    }
    Ok(canonical)
}

fn read_json_snapshot<S: AutopilotCheckpointStore>(
    store: &mut S,
    max_bytes: usize,
) -> Result<Option<Value>, AutopilotError> {
    let encoded = store.read().map_err(store_error)?;
    let Some(encoded) = encoded else {
        return Ok(None);
    };
    if encoded.as_bytes().len() > max_bytes {
        return Err(invalid("stored checkpoint exceeds its byte bound"));
    }
    let raw: Value = serde_json::from_str(&encoded)
        .map_err(|error| invalid(format!("stored checkpoint is invalid JSON: {error}")))?;
    let normalized = validate_autopilot_checkpoint(&raw)?;
    let canonical =
        to_canonical_string(&normalized).map_err(|error| AutopilotError::Canonicalisation {
            reason: error.to_string(),
        })?;
    if canonical != encoded {
        return Err(invalid("stored checkpoint is not canonical JSON"));
    }
    Ok(Some(normalized))
}

fn write_json_snapshot<S: AutopilotCheckpointStore>(
    store: &mut S,
    max_bytes: usize,
    snapshot: &Value,
) -> Result<(), AutopilotError> {
    let canonical = canonical_snapshot(snapshot, max_bytes)?;
    store.write(canonical).map_err(store_error)
}

pub struct JsonAutopilotCheckpointPersistence<S> {
    pub store: S,
    pub max_bytes: usize,
}

impl<S> JsonAutopilotCheckpointPersistence<S> {
    pub fn new(store: S) -> Self {
        Self {
            store,
            max_bytes: AUTOPILOT_CHECKPOINT_MAX_BYTES,
        }
    }
}

impl<S: AutopilotCheckpointStore> AutopilotCheckpointPersistence
    for JsonAutopilotCheckpointPersistence<S>
{
    fn read_snapshot(&mut self) -> Result<Option<Value>, AutopilotError> {
        read_json_snapshot(&mut self.store, self.max_bytes)
    }

    fn write_snapshot(&mut self, snapshot: &Value) -> Result<(), AutopilotError> {
        write_json_snapshot(&mut self.store, self.max_bytes, snapshot)
    }
}

pub struct TransactionalJsonAutopilotCheckpointPersistence<S> {
    pub store: S,
    pub max_bytes: usize,
}

impl<S> TransactionalJsonAutopilotCheckpointPersistence<S> {
    pub fn new(store: S) -> Self {
        Self {
            store,
            max_bytes: AUTOPILOT_CHECKPOINT_MAX_BYTES,
        }
    }
}

impl<S: TransactionalAutopilotCheckpointStore> AutopilotCheckpointPersistence
    for TransactionalJsonAutopilotCheckpointPersistence<S>
{
    fn read_snapshot(&mut self) -> Result<Option<Value>, AutopilotError> {
        read_json_snapshot(&mut self.store, self.max_bytes)
    }

    fn write_snapshot(&mut self, snapshot: &Value) -> Result<(), AutopilotError> {
        write_json_snapshot(&mut self.store, self.max_bytes, snapshot)
    }
}

impl<S: TransactionalAutopilotCheckpointStore> TransactionalAutopilotCheckpointPersistence
    for TransactionalJsonAutopilotCheckpointPersistence<S>
{
    fn write_snapshot_if_unchanged(
        &mut self,
        expected_snapshot_digest: Option<&str>,
        snapshot: &Value,
    ) -> Result<bool, AutopilotError> {
        let normalized = validate_autopilot_checkpoint(snapshot)?;
        let canonical = canonical_snapshot(&normalized, self.max_bytes)?;
        self.store
            .write_if_unchanged(expected_snapshot_digest, canonical)
            .map_err(store_error)
    }
}

pub struct AutopilotCheckpointPersistenceCoordinator<P> {
    pub persistence: P,
    expected_snapshot_digest: Option<String>,
    expected_generation: u64,
}

impl<P: AutopilotCheckpointPersistence> AutopilotCheckpointPersistenceCoordinator<P> {
    pub fn new(persistence: P) -> Self {
        Self {
            persistence,
            expected_snapshot_digest: None,
            expected_generation: 0,
        }
    }

    pub fn restore(&mut self) -> Result<Option<Value>, AutopilotError> {
        let snapshot = self.persistence.read_snapshot()?;
        self.expected_snapshot_digest = snapshot
            .as_ref()
            .and_then(|value| value.get("snapshot_digest"))
            .and_then(Value::as_str)
            .map(str::to_owned);
        self.expected_generation = snapshot
            .as_ref()
            .and_then(|value| value.get("generation"))
            .and_then(Value::as_u64)
            .unwrap_or(0);
        Ok(snapshot)
    }

    pub fn flush(&mut self, snapshot: &Value) -> Result<Value, AutopilotError> {
        let normalized = validate_autopilot_checkpoint(snapshot)?;
        let generation = normalized
            .get("generation")
            .and_then(Value::as_u64)
            .ok_or_else(|| invalid("checkpoint generation is missing"))?;
        if generation != self.expected_generation + 1 {
            return Err(invalid("checkpoint generation is not contiguous"));
        }
        if normalized
            .get("previous_snapshot_digest")
            .and_then(Value::as_str)
            != self.expected_snapshot_digest.as_deref()
        {
            return Err(invalid(
                "checkpoint predecessor does not match the restored head",
            ));
        }
        self.persistence.write_snapshot(&normalized)?;
        self.expected_generation = generation;
        self.expected_snapshot_digest = normalized
            .get("snapshot_digest")
            .and_then(Value::as_str)
            .map(str::to_owned);
        Ok(normalized)
    }
}

pub struct TransactionalAutopilotCheckpointPersistenceCoordinator<P> {
    pub persistence: P,
    expected_snapshot_digest: Option<String>,
    expected_generation: u64,
}

impl<P: TransactionalAutopilotCheckpointPersistence>
    TransactionalAutopilotCheckpointPersistenceCoordinator<P>
{
    pub fn new(persistence: P) -> Self {
        Self {
            persistence,
            expected_snapshot_digest: None,
            expected_generation: 0,
        }
    }

    pub fn restore(&mut self) -> Result<Option<Value>, AutopilotError> {
        let snapshot = self.persistence.read_snapshot()?;
        self.expected_snapshot_digest = snapshot
            .as_ref()
            .and_then(|value| value.get("snapshot_digest"))
            .and_then(Value::as_str)
            .map(str::to_owned);
        self.expected_generation = snapshot
            .as_ref()
            .and_then(|value| value.get("generation"))
            .and_then(Value::as_u64)
            .unwrap_or(0);
        Ok(snapshot)
    }

    pub fn flush(&mut self, snapshot: &Value) -> Result<Value, AutopilotError> {
        let normalized = validate_autopilot_checkpoint(snapshot)?;
        let generation = normalized
            .get("generation")
            .and_then(Value::as_u64)
            .ok_or_else(|| invalid("checkpoint generation is missing"))?;
        if generation != self.expected_generation + 1 {
            return Err(invalid("checkpoint generation is not contiguous"));
        }
        if normalized
            .get("previous_snapshot_digest")
            .and_then(Value::as_str)
            != self.expected_snapshot_digest.as_deref()
        {
            return Err(invalid(
                "checkpoint predecessor does not match the restored head",
            ));
        }
        if !self
            .persistence
            .write_snapshot_if_unchanged(self.expected_snapshot_digest.as_deref(), &normalized)?
        {
            return Err(AutopilotError::CompareAndSwapConflict);
        }
        self.expected_generation = generation;
        self.expected_snapshot_digest = normalized
            .get("snapshot_digest")
            .and_then(Value::as_str)
            .map(str::to_owned);
        Ok(normalized)
    }
}
