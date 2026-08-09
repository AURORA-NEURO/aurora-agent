//! Jobs and their idempotency class.
//!
//! Blueprint 40.30. The field that does the work here is [`Idempotency`], because it is what
//! decides whether a job may be retried after a lease expires. That is the second non-negotiable
//! invariant of the contract and the one a naive queue gets wrong:
//!
//! > Lease expiry does not imply safe retry for non-idempotent effects.
//!
//! A lease expiring means the worker stopped reporting. It does **not** mean the work did not
//! happen — the worker may have completed the effect and died before committing. Retrying a
//! non-idempotent job on that evidence double-applies it.

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Whether a job may be re-run after an ambiguous failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Idempotency {
    /// Re-running produces the same result and no additional effect. Safe to retry blindly.
    Idempotent,
    /// Re-running would repeat an external effect. After an ambiguous failure this job is
    /// **quarantined for a human**, never auto-retried.
    NonIdempotent,
    /// Re-running is unsafe, but a compensating action can undo the first attempt. Retry is
    /// permitted only after compensation succeeds.
    Compensable,
}

impl Idempotency {
    /// Whether an ambiguous outcome — lease expiry, worker crash — permits an automatic retry.
    ///
    /// Only [`Idempotency::Idempotent`] does. `Compensable` requires compensation first, which is
    /// a separate decision, and `NonIdempotent` requires a person.
    pub fn safe_to_retry_after_ambiguity(self) -> bool {
        matches!(self, Idempotency::Idempotent)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceClass {
    Compile,
    Ingest,
    Sandbox,
    Evaluate,
    Mutate,
    Index,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobState {
    Queued,
    Leased,
    /// Outputs staged, not yet committed.
    Staged,
    Succeeded,
    Failed,
    /// Ambiguous outcome on a job that cannot be safely retried. Awaits a human.
    Quarantined,
    /// Exhausted its retry budget.
    DeadLettered,
    Cancelled,
}

impl JobState {
    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            JobState::Succeeded | JobState::DeadLettered | JobState::Cancelled
        )
    }

    /// Whether a worker may take this job.
    pub fn is_claimable(self) -> bool {
        self == JobState::Queued
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Job {
    pub id: String,
    pub resource_class: ResourceClass,
    pub idempotency: Idempotency,
    pub priority: u8,
    pub max_attempts: u32,
    pub spec: Value,
    pub state: JobState,
    pub attempts: u32,
    /// Why the job is in its current state, when that needs explaining.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

impl Job {
    pub fn new(
        id: impl Into<String>,
        resource_class: ResourceClass,
        idempotency: Idempotency,
        spec: Value,
    ) -> Self {
        Job {
            id: id.into(),
            resource_class,
            idempotency,
            priority: 5,
            max_attempts: 3,
            spec,
            state: JobState::Queued,
            attempts: 0,
            reason: None,
        }
    }

    pub fn with_priority(mut self, priority: u8) -> Self {
        self.priority = priority;
        self
    }

    pub fn with_max_attempts(mut self, max_attempts: u32) -> Self {
        self.max_attempts = max_attempts;
        self
    }

    /// A stable key for deduplicating enqueues.
    ///
    /// Derived from the specification rather than the id, so the same work submitted twice under
    /// different names is recognised as the same work.
    pub fn idempotency_key(&self) -> ContentHash {
        let body = serde_json::json!({
            "resource_class": self.resource_class,
            "spec": self.spec,
        });
        ContentHash::of_value(&body).expect("spec is finite JSON")
    }

    pub fn attempts_remaining(&self) -> u32 {
        self.max_attempts.saturating_sub(self.attempts)
    }
}
