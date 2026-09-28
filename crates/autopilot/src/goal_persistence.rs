//! Caller-owned persistence for safe-stop goal-control checkpoints.
//!
//! A goal report can contain enough bounded metadata to resume through `resume_goal`, but a report
//! digest is neither durable storage nor a rollback fence. This module wraps only a verified safe
//! stop in a generation-linked checkpoint and provides canonical JSON and compare-and-swap seams.
//! Stores own atomicity, access control, backup policy, and anti-rollback guarantees. The last
//! private Autopilot report and controller state are deliberately not copied into the checkpoint;
//! callers rehydrate them and `resume_goal` verifies the report against its retained digest.

use crate::goal_control::{verify_goal_control_report, GoalControlError};
use bioprism_ids::{to_canonical_string, ContentHash};
use serde_json::{json, Map, Value};
use thiserror::Error;

pub const GOAL_CONTROL_CHECKPOINT_SCHEMA: &str = "bioprism-autopilot-goal-checkpoint/0.1";
pub const GOAL_CONTROL_CHECKPOINT_MAX_BYTES: usize = 2_000_000;
pub const GOAL_CONTROL_CHECKPOINT_RETENTION: &str =
    "goal_metadata_only;private_mission_reports_and_controller_state_are_caller_owned";

const CHECKPOINT_FIELDS: &[&str] = &[
    "schema",
    "goal_control_report",
    "generation",
    "previous_snapshot_digest",
    "retention",
    "snapshot_digest",
];

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum GoalControlCheckpointError {
    #[error("invalid goal-control checkpoint: {reason}")]
    Invalid { reason: String },
    #[error("goal-control report is invalid: {0}")]
    GoalControl(#[from] GoalControlError),
    #[error("goal-control checkpoint storage failed: {reason}")]
    Persistence { reason: String },
    #[error("goal-control checkpoint canonicalisation failed: {reason}")]
    Canonicalisation { reason: String },
    #[error("goal-control checkpoint compare-and-swap conflict")]
    CompareAndSwapConflict,
}

fn invalid(reason: impl Into<String>) -> GoalControlCheckpointError {
    GoalControlCheckpointError::Invalid {
        reason: reason.into(),
    }
}

fn digest(value: &Value) -> Result<String, GoalControlCheckpointError> {
    ContentHash::of_value(value)
        .map(|hash| hash.to_string())
        .map_err(|error| GoalControlCheckpointError::Canonicalisation {
            reason: error.to_string(),
        })
}

fn is_digest(value: &str) -> bool {
    ContentHash::parse(value.to_owned()).is_ok()
}

fn exact_fields(
    object: &Map<String, Value>,
    fields: &[&str],
) -> Result<(), GoalControlCheckpointError> {
    if object.len() != fields.len() || fields.iter().any(|field| !object.contains_key(*field)) {
        return Err(invalid("checkpoint has missing or undeclared fields"));
    }
    Ok(())
}

fn safe_stop(report: &Value) -> Result<(), GoalControlCheckpointError> {
    let projection = verify_goal_control_report(report)?;
    if projection.get("valid").and_then(Value::as_bool) != Some(true)
        || report.get("final_status").and_then(Value::as_str) != Some("stopped")
        || !matches!(
            report
                .pointer("/disposition/reason")
                .and_then(Value::as_str),
            Some("no_admissible_work" | "needs_review")
        )
    {
        return Err(invalid(
            "only a verified no_admissible_work or needs_review stop can be checkpointed",
        ));
    }
    if report
        .pointer("/cycles")
        .and_then(Value::as_array)
        .is_some_and(|cycles| {
            cycles.iter().any(|cycle| {
                !matches!(
                    cycle.get("final_status").and_then(Value::as_str),
                    Some("succeeded" | "exhausted")
                )
            })
        })
    {
        return Err(invalid(
            "checkpoint contains a non-continuable mission cycle",
        ));
    }
    Ok(())
}

/// Seal an explicit safe-stop report into a bounded, metadata-only checkpoint.
pub fn seal_goal_control_checkpoint(
    goal_control_report: &Value,
    generation: u64,
    previous_snapshot_digest: Option<&str>,
) -> Result<Value, GoalControlCheckpointError> {
    if generation == 0 {
        return Err(invalid("generation must be positive"));
    }
    if previous_snapshot_digest.is_some_and(|value| !is_digest(value)) {
        return Err(invalid("previous_snapshot_digest is malformed"));
    }
    safe_stop(goal_control_report)?;

    let body = json!({
        "schema": GOAL_CONTROL_CHECKPOINT_SCHEMA,
        "goal_control_report": goal_control_report,
        "generation": generation,
        "previous_snapshot_digest": previous_snapshot_digest,
        "retention": GOAL_CONTROL_CHECKPOINT_RETENTION,
    });
    let mut checkpoint = body.clone();
    checkpoint["snapshot_digest"] = Value::String(digest(&body)?);
    validate_goal_control_checkpoint(&checkpoint)
}

/// Validate the hash chain and return a normalized metadata-only checkpoint.
pub fn validate_goal_control_checkpoint(
    checkpoint: &Value,
) -> Result<Value, GoalControlCheckpointError> {
    let object = checkpoint
        .as_object()
        .ok_or_else(|| invalid("checkpoint must be a JSON object"))?;
    exact_fields(object, CHECKPOINT_FIELDS)?;
    if object.get("schema").and_then(Value::as_str) != Some(GOAL_CONTROL_CHECKPOINT_SCHEMA) {
        return Err(invalid("schema is missing or unsupported"));
    }
    if object.get("retention").and_then(Value::as_str) != Some(GOAL_CONTROL_CHECKPOINT_RETENTION) {
        return Err(invalid("retention marker is missing or unsupported"));
    }
    let generation = object
        .get("generation")
        .and_then(Value::as_u64)
        .filter(|generation| *generation > 0)
        .ok_or_else(|| invalid("generation must be a positive integer"))?;
    let previous = match object.get("previous_snapshot_digest") {
        Some(Value::Null) => None,
        Some(Value::String(value)) if is_digest(value) => Some(value.clone()),
        _ => return Err(invalid("previous_snapshot_digest must be null or a digest")),
    };
    if (generation == 1) != previous.is_none() {
        return Err(invalid(
            "generation one must have no predecessor and later generations must name one",
        ));
    }
    let report = object
        .get("goal_control_report")
        .ok_or_else(|| invalid("goal_control_report is missing"))?;
    safe_stop(report)?;
    let claimed = object
        .get("snapshot_digest")
        .and_then(Value::as_str)
        .filter(|value| is_digest(value))
        .ok_or_else(|| invalid("snapshot_digest is malformed"))?;
    let mut unsigned = checkpoint.clone();
    unsigned
        .as_object_mut()
        .expect("checkpoint object was checked")
        .remove("snapshot_digest");
    if claimed != digest(&unsigned)? {
        return Err(invalid(
            "snapshot_digest does not match the checkpoint body",
        ));
    }
    let canonical = to_canonical_string(checkpoint).map_err(|error| {
        GoalControlCheckpointError::Canonicalisation {
            reason: error.to_string(),
        }
    })?;
    if canonical.len() > GOAL_CONTROL_CHECKPOINT_MAX_BYTES {
        return Err(invalid("checkpoint exceeds the configured byte bound"));
    }
    Ok(json!({
        "schema": GOAL_CONTROL_CHECKPOINT_SCHEMA,
        "goal_control_report": report,
        "generation": generation,
        "previous_snapshot_digest": previous,
        "retention": GOAL_CONTROL_CHECKPOINT_RETENTION,
        "snapshot_digest": claimed,
    }))
}

pub fn resume_goal_from_checkpoint<D, C>(
    checkpoint: &Value,
    previous_autopilot_report: Option<Value>,
    grant: &crate::grant::AutonomyGrant,
    controller: &mut C,
    dispatcher: &mut D,
) -> Result<crate::goal_control::GoalControlOutcome, GoalControlCheckpointError>
where
    D: crate::drive::MissionDispatch,
    C: crate::goal_control::GoalController,
{
    let normalized = validate_goal_control_checkpoint(checkpoint)?;
    crate::goal_control::resume_goal(
        &normalized["goal_control_report"],
        previous_autopilot_report,
        grant,
        controller,
        dispatcher,
    )
    .map_err(Into::into)
}

pub trait GoalControlCheckpointStore {
    fn read(&mut self) -> Result<Option<String>, String>;
    fn write(&mut self, value: String) -> Result<(), String>;
}

/// The implementation must compare and replace the value atomically.
pub trait TransactionalGoalControlCheckpointStore: GoalControlCheckpointStore {
    fn write_if_unchanged(
        &mut self,
        expected_snapshot_digest: Option<&str>,
        value: String,
    ) -> Result<bool, String>;
}

pub trait GoalControlCheckpointPersistence {
    fn read_snapshot(&mut self) -> Result<Option<Value>, GoalControlCheckpointError>;
    fn write_snapshot(&mut self, snapshot: &Value) -> Result<(), GoalControlCheckpointError>;
}

pub trait TransactionalGoalControlCheckpointPersistence: GoalControlCheckpointPersistence {
    fn write_snapshot_if_unchanged(
        &mut self,
        expected_snapshot_digest: Option<&str>,
        snapshot: &Value,
    ) -> Result<bool, GoalControlCheckpointError>;
}

fn canonical_snapshot(
    snapshot: &Value,
    max_bytes: usize,
) -> Result<String, GoalControlCheckpointError> {
    let normalized = validate_goal_control_checkpoint(snapshot)?;
    let canonical = to_canonical_string(&normalized).map_err(|error| {
        GoalControlCheckpointError::Canonicalisation {
            reason: error.to_string(),
        }
    })?;
    if canonical.len() > max_bytes {
        return Err(invalid("checkpoint exceeds the configured byte bound"));
    }
    Ok(canonical)
}

fn read_json_snapshot<S: GoalControlCheckpointStore>(
    store: &mut S,
    max_bytes: usize,
) -> Result<Option<Value>, GoalControlCheckpointError> {
    let Some(encoded) = store
        .read()
        .map_err(|reason| GoalControlCheckpointError::Persistence { reason })?
    else {
        return Ok(None);
    };
    if encoded.len() > max_bytes {
        return Err(invalid("stored checkpoint exceeds its byte bound"));
    }
    let raw: Value = serde_json::from_str(&encoded)
        .map_err(|error| invalid(format!("stored checkpoint is invalid JSON: {error}")))?;
    let normalized = validate_goal_control_checkpoint(&raw)?;
    let canonical = to_canonical_string(&normalized).map_err(|error| {
        GoalControlCheckpointError::Canonicalisation {
            reason: error.to_string(),
        }
    })?;
    if canonical != encoded {
        return Err(invalid("stored checkpoint is not canonical JSON"));
    }
    Ok(Some(normalized))
}

fn write_json_snapshot<S: GoalControlCheckpointStore>(
    store: &mut S,
    max_bytes: usize,
    snapshot: &Value,
) -> Result<(), GoalControlCheckpointError> {
    let canonical = canonical_snapshot(snapshot, max_bytes)?;
    store
        .write(canonical)
        .map_err(|reason| GoalControlCheckpointError::Persistence { reason })
}

pub struct JsonGoalControlCheckpointPersistence<S> {
    pub store: S,
    pub max_bytes: usize,
}

impl<S> JsonGoalControlCheckpointPersistence<S> {
    pub fn new(store: S) -> Self {
        Self {
            store,
            max_bytes: GOAL_CONTROL_CHECKPOINT_MAX_BYTES,
        }
    }
}

impl<S: GoalControlCheckpointStore> GoalControlCheckpointPersistence
    for JsonGoalControlCheckpointPersistence<S>
{
    fn read_snapshot(&mut self) -> Result<Option<Value>, GoalControlCheckpointError> {
        read_json_snapshot(&mut self.store, self.max_bytes)
    }

    fn write_snapshot(&mut self, snapshot: &Value) -> Result<(), GoalControlCheckpointError> {
        write_json_snapshot(&mut self.store, self.max_bytes, snapshot)
    }
}

pub struct TransactionalJsonGoalControlCheckpointPersistence<S> {
    pub store: S,
    pub max_bytes: usize,
}

impl<S> TransactionalJsonGoalControlCheckpointPersistence<S> {
    pub fn new(store: S) -> Self {
        Self {
            store,
            max_bytes: GOAL_CONTROL_CHECKPOINT_MAX_BYTES,
        }
    }
}

impl<S: TransactionalGoalControlCheckpointStore> GoalControlCheckpointPersistence
    for TransactionalJsonGoalControlCheckpointPersistence<S>
{
    fn read_snapshot(&mut self) -> Result<Option<Value>, GoalControlCheckpointError> {
        read_json_snapshot(&mut self.store, self.max_bytes)
    }

    fn write_snapshot(&mut self, snapshot: &Value) -> Result<(), GoalControlCheckpointError> {
        write_json_snapshot(&mut self.store, self.max_bytes, snapshot)
    }
}

impl<S: TransactionalGoalControlCheckpointStore> TransactionalGoalControlCheckpointPersistence
    for TransactionalJsonGoalControlCheckpointPersistence<S>
{
    fn write_snapshot_if_unchanged(
        &mut self,
        expected_snapshot_digest: Option<&str>,
        snapshot: &Value,
    ) -> Result<bool, GoalControlCheckpointError> {
        let canonical = canonical_snapshot(snapshot, self.max_bytes)?;
        self.store
            .write_if_unchanged(expected_snapshot_digest, canonical)
            .map_err(|reason| GoalControlCheckpointError::Persistence { reason })
    }
}

pub struct GoalControlCheckpointPersistenceCoordinator<P> {
    pub persistence: P,
    expected_snapshot_digest: Option<String>,
    expected_generation: u64,
}

impl<P: GoalControlCheckpointPersistence> GoalControlCheckpointPersistenceCoordinator<P> {
    pub fn new(persistence: P) -> Self {
        Self {
            persistence,
            expected_snapshot_digest: None,
            expected_generation: 0,
        }
    }

    pub fn restore(&mut self) -> Result<Option<Value>, GoalControlCheckpointError> {
        let checkpoint = self
            .persistence
            .read_snapshot()?
            .map(|value| validate_goal_control_checkpoint(&value))
            .transpose()?;
        self.expected_snapshot_digest = checkpoint
            .as_ref()
            .and_then(|value| value.get("snapshot_digest"))
            .and_then(Value::as_str)
            .map(str::to_owned);
        self.expected_generation = checkpoint
            .as_ref()
            .and_then(|value| value.get("generation"))
            .and_then(Value::as_u64)
            .unwrap_or(0);
        Ok(checkpoint)
    }

    pub fn flush(&mut self, checkpoint: &Value) -> Result<Value, GoalControlCheckpointError> {
        let normalized = validate_goal_control_checkpoint(checkpoint)?;
        let generation = normalized["generation"]
            .as_u64()
            .ok_or_else(|| invalid("checkpoint generation is missing"))?;
        let next_generation = self
            .expected_generation
            .checked_add(1)
            .ok_or_else(|| invalid("checkpoint generation overflowed"))?;
        if generation != next_generation {
            return Err(invalid("checkpoint generation is not contiguous"));
        }
        if normalized["previous_snapshot_digest"].as_str()
            != self.expected_snapshot_digest.as_deref()
        {
            return Err(invalid(
                "checkpoint predecessor does not match the restored head",
            ));
        }
        self.persistence.write_snapshot(&normalized)?;
        self.advance(&normalized, generation);
        Ok(normalized)
    }

    fn advance(&mut self, checkpoint: &Value, generation: u64) {
        self.expected_generation = generation;
        self.expected_snapshot_digest = checkpoint["snapshot_digest"].as_str().map(str::to_owned);
    }
}

pub struct TransactionalGoalControlCheckpointPersistenceCoordinator<P> {
    pub persistence: P,
    expected_snapshot_digest: Option<String>,
    expected_generation: u64,
}

impl<P: TransactionalGoalControlCheckpointPersistence>
    TransactionalGoalControlCheckpointPersistenceCoordinator<P>
{
    pub fn new(persistence: P) -> Self {
        Self {
            persistence,
            expected_snapshot_digest: None,
            expected_generation: 0,
        }
    }

    pub fn restore(&mut self) -> Result<Option<Value>, GoalControlCheckpointError> {
        let checkpoint = self
            .persistence
            .read_snapshot()?
            .map(|value| validate_goal_control_checkpoint(&value))
            .transpose()?;
        self.expected_snapshot_digest = checkpoint
            .as_ref()
            .and_then(|value| value.get("snapshot_digest"))
            .and_then(Value::as_str)
            .map(str::to_owned);
        self.expected_generation = checkpoint
            .as_ref()
            .and_then(|value| value.get("generation"))
            .and_then(Value::as_u64)
            .unwrap_or(0);
        Ok(checkpoint)
    }

    pub fn flush(&mut self, checkpoint: &Value) -> Result<Value, GoalControlCheckpointError> {
        let normalized = validate_goal_control_checkpoint(checkpoint)?;
        let generation = normalized["generation"]
            .as_u64()
            .ok_or_else(|| invalid("checkpoint generation is missing"))?;
        let next_generation = self
            .expected_generation
            .checked_add(1)
            .ok_or_else(|| invalid("checkpoint generation overflowed"))?;
        if generation != next_generation {
            return Err(invalid("checkpoint generation is not contiguous"));
        }
        if normalized["previous_snapshot_digest"].as_str()
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
            return Err(GoalControlCheckpointError::CompareAndSwapConflict);
        }
        self.expected_generation = generation;
        self.expected_snapshot_digest = normalized["snapshot_digest"].as_str().map(str::to_owned);
        Ok(normalized)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::goal_control::{
        drive_goal, GoalControlBudget, GoalControlOutcome, GoalDecision, GoalStopReason,
    };
    use crate::grant::AutonomyGrant;
    use crate::MissionDispatch;
    use bioprism_devplat::{
        plan_mission, MissionReport, MissionRequest, MissionStepResult, MISSION_SCHEMA_VERSION,
        MISSION_TRACE_SCHEMA_VERSION,
    };
    use serde_json::json;
    use std::sync::{Arc, Mutex};

    fn grant() -> AutonomyGrant {
        serde_json::from_value(json!({
            "allowed_tools": ["observe"],
            "max_attempts": 4,
            "require_reconciliation_complete": false,
        }))
        .expect("test grant is valid")
    }

    fn safe_goal_outcome() -> GoalControlOutcome {
        let mut controller = |context: &crate::goal_control::GoalControlContext<'_>| {
            if context.completed_cycles() == 0 {
                Ok(GoalDecision::RunMission(json!({
                    "mission_id": "private-mission-id",
                    "goal": "private-task-payload",
                    "steps": [{
                        "id": "step-1",
                        "domain": "metrics",
                        "capability": "analytics",
                        "objective": "observe the declared fixture",
                        "tool": "observe",
                        "arguments": {"private_argument": "secret-value"},
                        "depends_on": [],
                        "bindings": [],
                        "required": true,
                    }],
                })))
            } else {
                Ok(GoalDecision::Stop(GoalStopReason::NoAdmissibleWork))
            }
        };
        let mut dispatcher = SuccessfulDispatcher;
        drive_goal(
            "goal-persisted",
            &grant(),
            GoalControlBudget::new(4, 16).unwrap(),
            &mut controller,
            &mut dispatcher,
        )
        .unwrap()
    }

    fn safe_goal_report() -> Value {
        safe_goal_outcome().report
    }

    fn successful_report(mission: &Value) -> Value {
        let request: MissionRequest = serde_json::from_value(mission.clone()).unwrap();
        let plan = plan_mission(&request).unwrap();
        let results = vec![MissionStepResult {
            id: "step-1".into(),
            tool: "observe".into(),
            status: "succeeded".into(),
            required: true,
            arguments_digest: Some("1".repeat(64)),
            bytes: 2,
            wire: Some(json!({
                "jsonrpc": "2.0",
                "id": "step-1",
                "result": {"content": [{"type": "text", "text": "{}"}]},
            })),
            error: None,
        }];
        let claim_lineage = bioprism_devplat::mission_claim_lineage_with_review(
            &request.claim_requests,
            &results,
            request.evaluator_review.as_ref(),
        );
        serde_json::to_value(MissionReport {
            schema_version: MISSION_SCHEMA_VERSION.into(),
            plan,
            execution: "executed".into(),
            mission_status: "succeeded".into(),
            succeeded: 1,
            refused: 0,
            blocked: 0,
            cancelled: 0,
            required_failures: 0,
            returned_bytes: 2,
            results,
            execution_trace_schema_version: MISSION_TRACE_SCHEMA_VERSION.into(),
            execution_trace: Vec::new(),
            claim_requests: request.claim_requests.clone(),
            evaluator_review: request.evaluator_review.clone(),
            claim_lineage,
            trace_observer: None,
            guarantees: Vec::new(),
            limitations: Vec::new(),
        })
        .unwrap()
    }

    struct NoCallDispatcher;

    impl MissionDispatch for NoCallDispatcher {
        fn dispatch(&mut self, _: &Value) -> Result<Value, String> {
            Err("the stop decision must not dispatch".into())
        }
    }

    struct SuccessfulDispatcher;

    impl MissionDispatch for SuccessfulDispatcher {
        fn dispatch(&mut self, mission: &Value) -> Result<Value, String> {
            Ok(successful_report(mission))
        }
    }

    #[derive(Clone, Default)]
    struct SharedTextStore(Arc<Mutex<Option<String>>>);

    impl GoalControlCheckpointStore for SharedTextStore {
        fn read(&mut self) -> Result<Option<String>, String> {
            self.0
                .lock()
                .map(|value| value.clone())
                .map_err(|e| e.to_string())
        }

        fn write(&mut self, value: String) -> Result<(), String> {
            *self.0.lock().map_err(|e| e.to_string())? = Some(value);
            Ok(())
        }
    }

    impl TransactionalGoalControlCheckpointStore for SharedTextStore {
        fn write_if_unchanged(
            &mut self,
            expected_snapshot_digest: Option<&str>,
            value: String,
        ) -> Result<bool, String> {
            let mut current = self.0.lock().map_err(|e| e.to_string())?;
            let current_digest = current
                .as_ref()
                .and_then(|raw| serde_json::from_str::<Value>(raw).ok())
                .and_then(|snapshot| {
                    snapshot
                        .get("snapshot_digest")
                        .and_then(Value::as_str)
                        .map(str::to_owned)
                });
            if current_digest.as_deref() != expected_snapshot_digest {
                return Ok(false);
            }
            *current = Some(value);
            Ok(true)
        }
    }

    #[test]
    fn safe_goal_checkpoint_is_digest_sealed_metadata_only_and_resumable() {
        let outcome = safe_goal_outcome();
        let checkpoint = seal_goal_control_checkpoint(&outcome.report, 1, None).unwrap();
        let normalized = validate_goal_control_checkpoint(&checkpoint).unwrap();
        assert_eq!(normalized, checkpoint);
        assert_eq!(checkpoint["generation"], Value::from(1));
        assert_eq!(
            checkpoint["retention"],
            Value::from(GOAL_CONTROL_CHECKPOINT_RETENTION)
        );
        let encoded = checkpoint.to_string();
        assert!(!encoded.contains("private-mission-id"));
        assert!(!encoded.contains("private-task-payload"));
        assert!(!encoded.contains("secret-value"));

        let mut controller = |_: &crate::goal_control::GoalControlContext<'_>| {
            Ok(GoalDecision::Stop(GoalStopReason::NoAdmissibleWork))
        };
        let mut dispatcher = NoCallDispatcher;
        let resumed = resume_goal_from_checkpoint(
            &checkpoint,
            outcome.cycle_reports.last().cloned(),
            &grant(),
            &mut controller,
            &mut dispatcher,
        )
        .unwrap();
        assert_eq!(resumed.final_status.as_str(), "stopped");
    }

    #[test]
    fn goal_checkpoint_rejects_tampering_unknown_fields_and_unsafe_stop_states() {
        let mut checkpoint = seal_goal_control_checkpoint(&safe_goal_report(), 1, None).unwrap();
        checkpoint["generation"] = Value::from(2);
        assert!(validate_goal_control_checkpoint(&checkpoint).is_err());

        let mut checkpoint = seal_goal_control_checkpoint(&safe_goal_report(), 1, None).unwrap();
        checkpoint["unreviewed"] = Value::Bool(true);
        assert!(validate_goal_control_checkpoint(&checkpoint).is_err());

        let mut controller = |_: &crate::goal_control::GoalControlContext<'_>| {
            Ok(GoalDecision::Stop(GoalStopReason::Cancelled))
        };
        let mut dispatcher = NoCallDispatcher;
        let cancelled = drive_goal(
            "goal-cancelled",
            &grant(),
            GoalControlBudget::new(2, 4).unwrap(),
            &mut controller,
            &mut dispatcher,
        )
        .unwrap();
        assert!(seal_goal_control_checkpoint(&cancelled.report, 1, None).is_err());
    }

    #[test]
    fn transactional_goal_checkpoint_coordinator_rejects_stale_writers_and_links_generations() {
        let store = SharedTextStore::default();
        let mut first_writer = TransactionalGoalControlCheckpointPersistenceCoordinator::new(
            TransactionalJsonGoalControlCheckpointPersistence::new(store.clone()),
        );
        let mut stale_writer = TransactionalGoalControlCheckpointPersistenceCoordinator::new(
            TransactionalJsonGoalControlCheckpointPersistence::new(store.clone()),
        );
        assert!(first_writer.restore().unwrap().is_none());
        assert!(stale_writer.restore().unwrap().is_none());

        let first = seal_goal_control_checkpoint(&safe_goal_report(), 1, None).unwrap();
        first_writer.flush(&first).unwrap();
        assert!(matches!(
            stale_writer.flush(&first),
            Err(GoalControlCheckpointError::CompareAndSwapConflict)
        ));

        let second =
            seal_goal_control_checkpoint(&safe_goal_report(), 2, first["snapshot_digest"].as_str())
                .unwrap();
        first_writer.flush(&second).unwrap();

        let mut reader = JsonGoalControlCheckpointPersistence::new(store);
        let restored = reader.read_snapshot().unwrap().unwrap();
        assert_eq!(restored["generation"], Value::from(2));
        assert_eq!(
            restored["previous_snapshot_digest"],
            first["snapshot_digest"]
        );
    }

    #[test]
    fn json_goal_checkpoint_coordinator_enforces_generation_and_byte_bounds() {
        let mut coordinator = GoalControlCheckpointPersistenceCoordinator::new(
            JsonGoalControlCheckpointPersistence::new(SharedTextStore::default()),
        );
        assert!(coordinator.restore().unwrap().is_none());
        let first = seal_goal_control_checkpoint(&safe_goal_report(), 1, None).unwrap();
        let mut oversized = JsonGoalControlCheckpointPersistence::new(SharedTextStore::default());
        oversized.max_bytes = 1;
        assert!(oversized.write_snapshot(&first).is_err());

        coordinator.flush(&first).unwrap();
        let skipped =
            seal_goal_control_checkpoint(&safe_goal_report(), 3, first["snapshot_digest"].as_str())
                .unwrap();
        assert!(coordinator.flush(&skipped).is_err());
        assert_eq!(coordinator.restore().unwrap().unwrap(), first);
    }
}
