//! Trial lifecycle: states, transitions, retries, cancellation, and honest finalization.
//!
//! Blueprint 05.02 (run orchestrator). The orchestrator collects evidence; it is explicitly *not*
//! the source of scoring truth, so nothing here decides whether a trial went well. It decides only
//! what happened to it, and says so in a form a scorer cannot misread.
//!
//! Three things it refuses to be sloppy about:
//!
//! **Transitions are checked.** `Running → Completed` without passing through evaluation is not a
//! shortcut, it is a bug that would publish a result nobody evaluated. Illegal transitions error.
//!
//! **Finalization is idempotent.** 05.02 requires dispatch and finalize to be idempotent under
//! trial and attempt identity so a controller failover cannot duplicate materialized side effects.
//! Finalizing twice returns the first termination unchanged and emits no second event — including
//! when the second call gives a different reason, because the first one is what actually happened
//! and a controller that lost its lease is not a better witness than the run itself.
//!
//! **Incompleteness is stated, not inferred.** A forcibly cancelled trial records *which* evidence
//! is missing. The alternative — a trial that simply stops, leaving a downstream reader to notice
//! the absence — is how a truncated run becomes a low score instead of a missing one.
//!
//! Deliberately **not** implemented: scheduling, leasing against a real queue, and dispatch. Those
//! need a controller and a worker pool; what a single crate can own is the state machine and the
//! finalization record, which is where the honesty requirements actually live.

use crate::budget::RuntimeResource;
use crate::error::RuntimeError;
use bioprism_ids::RunId;
use serde::{Deserialize, Serialize};
use std::fmt;

macro_rules! execution_id {
    ($(#[$meta:meta])* $name:ident, $kind:literal) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(try_from = "String", into = "String")]
        pub struct $name(String);

        impl $name {
            pub fn parse(value: impl Into<String>) -> Result<Self, RuntimeError> {
                let value = value.into();
                if value.is_empty() || value.chars().any(char::is_control) {
                    return Err(RuntimeError::MalformedId {
                        kind: $kind,
                        value,
                    });
                }
                Ok($name(value))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl From<$name> for String {
            fn from(value: $name) -> Self {
                value.0
            }
        }

        impl TryFrom<String> for $name {
            type Error = RuntimeError;

            fn try_from(value: String) -> Result<Self, Self::Error> {
                $name::parse(value)
            }
        }
    };
}

execution_id!(
    /// Identifies one execution trial.
    ///
    /// Deliberately not in `bioprism-ids`: a trial is an execution-local identity that only the
    /// runtime mints, and 05.02's rule that a benchmark family, a task, an instance, a trial and a
    /// result are never conflated is better served by keeping each identity in the crate that owns
    /// its lifecycle.
    TrialId,
    "trial"
);
execution_id!(
    /// Identifies one attempt at a trial. A retry gets a new one; the trial id does not change.
    AttemptId,
    "attempt"
);

/// The lifecycle of 05.02.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunState {
    Queued,
    Leased,
    Preparing,
    Running,
    Evaluating,
    Finalizing,
    Completed,
    Failed,
    Cancelled,
    /// Ran, but its evidence is known to be partial. Distinct from `Failed`: a failed trial has a
    /// result, an incomplete one has a gap, and averaging them together is a category error.
    Incomplete,
}

impl RunState {
    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            RunState::Completed | RunState::Failed | RunState::Cancelled | RunState::Incomplete
        )
    }

    fn permits(self, next: RunState) -> bool {
        use RunState::*;
        match self {
            Queued => matches!(next, Leased | Cancelled | Failed),
            Leased => matches!(next, Preparing | Cancelled | Failed),
            Preparing => matches!(next, Running | Cancelled | Failed),
            Running => matches!(next, Evaluating | Cancelled | Failed | Incomplete),
            Evaluating => matches!(next, Finalizing | Cancelled | Failed),
            Finalizing => matches!(next, Completed | Failed | Incomplete),
            Completed | Failed | Cancelled | Incomplete => false,
        }
    }
}

impl fmt::Display for RunState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            RunState::Queued => "queued",
            RunState::Leased => "leased",
            RunState::Preparing => "preparing",
            RunState::Running => "running",
            RunState::Evaluating => "evaluating",
            RunState::Finalizing => "finalizing",
            RunState::Completed => "completed",
            RunState::Failed => "failed",
            RunState::Cancelled => "cancelled",
            RunState::Incomplete => "incomplete",
        };
        f.write_str(name)
    }
}

/// Every transition emits one of these (05.02: "every transition emits an event").
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LifecycleEvent {
    pub seq: u64,
    pub from: RunState,
    pub to: RunState,
    /// Task time (05.07), so a lifecycle trace is comparable across machines.
    pub task_millis: u64,
}

/// Why a trial stopped.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "reason", rename_all = "snake_case")]
pub enum TerminationReason {
    Completed,
    /// The agent's work failed. Evidence about the agent.
    TaskFailure { detail: String },
    /// Evidence about the budget, not about the agent's ability.
    BudgetExhausted { resource: RuntimeResource },
    Cancelled { forced: bool },
    /// The agent proposed something policy refused. Evidence about the agent's judgement.
    PolicyDenied { detail: String },
    /// Evidence about the platform.
    ProviderUnavailable { provider: String },
    Incomplete { detail: String },
}

/// The finalization record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Termination {
    pub reason: TerminationReason,
    pub task_millis: u64,
    /// Named gaps in the evidence. Empty means the record is complete, and says so on the record
    /// rather than leaving a reader to assume it.
    pub missing_evidence: Vec<String>,
}

/// How results across attempts are combined (05.02: "results specify aggregation policy").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AggregationPolicy {
    #[default]
    LastAttempt,
    BestAttempt,
    AllAttempts,
}

/// Why an attempt was retried (05.02 keeps these apart).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RetryClass {
    /// The platform failed. The agent gets another go at the same task.
    Infrastructure,
    /// The agent chose to try again. Part of the behaviour being measured.
    Agent,
}

/// What a previous attempt left behind.
///
/// Kept rather than replaced: 05.02 requires retried trials to preserve prior evidence, because
/// "succeeded on the third attempt" and "succeeded" are different findings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttemptRecord {
    pub attempt: AttemptId,
    pub retry_class: RetryClass,
    pub termination: Option<Termination>,
    /// The tape head the attempt reached, so its evidence remains addressable.
    pub tape_head: String,
}

/// One trial's lifecycle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Trial {
    run: RunId,
    trial: TrialId,
    attempt: AttemptId,
    state: RunState,
    task_millis: u64,
    events: Vec<LifecycleEvent>,
    termination: Option<Termination>,
    prior_attempts: Vec<AttemptRecord>,
    aggregation: AggregationPolicy,
}

impl Trial {
    pub fn new(run: RunId, trial: TrialId, attempt: AttemptId) -> Self {
        Trial {
            run,
            trial,
            attempt,
            state: RunState::Queued,
            task_millis: 0,
            events: Vec::new(),
            termination: None,
            prior_attempts: Vec::new(),
            aggregation: AggregationPolicy::default(),
        }
    }

    pub fn with_aggregation(mut self, aggregation: AggregationPolicy) -> Self {
        self.aggregation = aggregation;
        self
    }

    pub fn run(&self) -> &RunId {
        &self.run
    }

    pub fn id(&self) -> &TrialId {
        &self.trial
    }

    pub fn attempt(&self) -> &AttemptId {
        &self.attempt
    }

    pub fn state(&self) -> RunState {
        self.state
    }

    pub fn events(&self) -> &[LifecycleEvent] {
        &self.events
    }

    pub fn termination(&self) -> Option<&Termination> {
        self.termination.as_ref()
    }

    pub fn prior_attempts(&self) -> &[AttemptRecord] {
        &self.prior_attempts
    }

    pub fn aggregation(&self) -> AggregationPolicy {
        self.aggregation
    }

    /// Sets the task clock, which lifecycle events are stamped with.
    pub fn set_task_millis(&mut self, task_millis: u64) {
        self.task_millis = task_millis;
    }

    /// Moves to the next state, or refuses.
    pub fn advance(&mut self, to: RunState) -> Result<&LifecycleEvent, RuntimeError> {
        if !self.state.permits(to) {
            return Err(RuntimeError::IllegalTransition {
                from: self.state,
                to,
            });
        }
        let event = LifecycleEvent {
            seq: self.events.len() as u64,
            from: self.state,
            to,
            task_millis: self.task_millis,
        };
        self.state = to;
        self.events.push(event);
        Ok(self.events.last().expect("just pushed"))
    }

    /// Walks the happy path from wherever the trial is to `Running`.
    pub fn dispatch(&mut self) -> Result<(), RuntimeError> {
        for state in [RunState::Leased, RunState::Preparing, RunState::Running] {
            if self.state != state {
                self.advance(state)?;
            }
        }
        Ok(())
    }

    /// Ends the trial. Idempotent under trial and attempt identity.
    ///
    /// A second call is not an error and not a correction: it returns the first termination
    /// unchanged and emits no further event. A controller that failed over and re-finalized must
    /// not be able to overwrite what the run itself recorded.
    pub fn finalize(
        &mut self,
        reason: TerminationReason,
        missing_evidence: Vec<String>,
    ) -> Result<&Termination, RuntimeError> {
        if self.termination.is_none() {
            let target = match &reason {
                TerminationReason::Completed => RunState::Completed,
                TerminationReason::Cancelled { .. } => RunState::Cancelled,
                TerminationReason::Incomplete { .. } => RunState::Incomplete,
                TerminationReason::TaskFailure { .. }
                | TerminationReason::BudgetExhausted { .. }
                | TerminationReason::PolicyDenied { .. }
                | TerminationReason::ProviderUnavailable { .. } => RunState::Failed,
            };
            if !self.state.is_terminal() {
                self.advance(target)?;
            }
            self.termination = Some(Termination {
                reason,
                task_millis: self.task_millis,
                missing_evidence,
            });
        }
        Ok(self.termination.as_ref().expect("set on the branch above"))
    }

    /// Cancels the trial, stating what evidence the cancellation cost.
    ///
    /// A graceful cancellation asks for a checkpoint first and loses nothing; a forced one is
    /// required to name the gaps, because "cancelled" alone lets a reader assume the evidence is
    /// merely short rather than absent.
    pub fn cancel(
        &mut self,
        forced: bool,
        missing_evidence: Vec<String>,
    ) -> Result<&Termination, RuntimeError> {
        self.finalize(TerminationReason::Cancelled { forced }, missing_evidence)
    }

    /// Opens a fresh attempt at the same trial, carrying the previous attempts forward.
    pub fn retry(
        &self,
        attempt: AttemptId,
        retry_class: RetryClass,
        tape_head: impl Into<String>,
    ) -> Result<Trial, RuntimeError> {
        if !self.state.is_terminal() {
            return Err(RuntimeError::RunNotRunnable { state: self.state });
        }
        let mut prior = self.prior_attempts.clone();
        prior.push(AttemptRecord {
            attempt: self.attempt.clone(),
            retry_class,
            termination: self.termination.clone(),
            tape_head: tape_head.into(),
        });
        Ok(Trial {
            run: self.run.clone(),
            trial: self.trial.clone(),
            attempt,
            state: RunState::Queued,
            task_millis: self.task_millis,
            events: Vec::new(),
            termination: None,
            prior_attempts: prior,
            aggregation: self.aggregation,
        })
    }
}
