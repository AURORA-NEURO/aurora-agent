//! Evaluator health: telling a broken harness from a failing system (07.02).
//!
//! 07.02's fourth responsibility is one line — "Distinguish task failure from evaluator failure" —
//! and it is the only part of that module a library in this workspace can hold. The rest of 07.02
//! is a sandbox: run commands in a separate evaluator image, mount grader files after agent
//! execution, deny ambient credentials. `bioprism-safety` already states the position this
//! workspace takes on that — a library of plain Rust types cannot isolate anything and must not
//! claim to — and nothing here spawns a process.
//!
//! # Why the distinction needs a type
//!
//! A grader that times out, throws on a malformed fixture, or cannot reach a hidden expectation
//! produces the same shape of output as a grader that ran and found the answer wrong: a
//! non-passing result. Aggregated, they are indistinguishable, and an evaluation whose harness
//! degrades looks exactly like a system that got worse. That is a live risk in a benchmark that
//! ships regressions: the harness breaks, the numbers drop, and the drop gets attributed to the
//! model.
//!
//! [`EvaluatorRun::task_outcome`] returns `Result`. An unhealthy run cannot yield a task outcome
//! at all — [`EvaluatorError::NotTaskEvidence`] names the health state — so the only way to get a
//! number out is to have a healthy evaluator, and the only way to record an unhealthy one is as an
//! [`crate::plane::UnscoredReason::EvaluatorUnhealthy`] cell, which the plane then refuses to fold.
//! The two invariants meet: an evaluator failure becomes an unscored dimension, and an unscored
//! dimension has no path to becoming a zero.
//!
//! # Diagnostics are required, not optional
//!
//! 07.02: "Failures include command, exit state, relevant diff, logs, hidden-data access evidence,
//! and whether the evaluator itself was healthy." [`EvaluatorRun::task_outcome`] refuses a failing
//! run with no [`Diagnostic`], because a bare `false` is exactly the output 07.02's responsibility
//! list is written against.
//!
//! # Not implemented
//!
//! No sandbox, no process execution, no filesystem or network mediation, no grader-image
//! mounting, no hidden-fixture protection. Those are 07.02's core and they are infrastructure, not
//! a predicate over an artifact; this module models their *outcome* and claims none of them. No
//! property evaluators over output sets (invariance, monotonicity, conservation) — the metamorphic
//! subset of that lives in [`crate::metamorphic`] and the rest needs the outputs themselves.

use serde::{Deserialize, Serialize};

use crate::error::EvaluatorError;
use crate::plane::UnscoredReason;

/// Whether the evaluator itself worked.
///
/// Four states, and three of them are ways of being unable to say anything about the task.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "health")]
pub enum Health {
    /// The evaluator ran to completion and its own preconditions held.
    Healthy,
    /// The evaluator exceeded its own time budget. Says nothing about the task.
    TimedOut { after: String },
    /// The evaluator crashed or errored internally.
    Errored { detail: String },
    /// The evaluator's fixtures, expectations or environment were not in the state it requires.
    FixtureBroken { detail: String },
}

impl Health {
    /// Whether this run can say anything about the task.
    pub fn is_healthy(&self) -> bool {
        matches!(self, Health::Healthy)
    }

    /// A short label, used in the refusal message so a reader learns which failure occurred.
    pub fn label(&self) -> &'static str {
        match self {
            Health::Healthy => "healthy",
            Health::TimedOut { .. } => "timed out",
            Health::Errored { .. } => "errored",
            Health::FixtureBroken { .. } => "running on broken fixtures",
        }
    }
}

/// The evidence 07.02 requires alongside a failure.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Diagnostic {
    /// What was run. Opaque to this crate, which does not execute it.
    pub command: String,
    pub exit_state: String,
    /// The difference that made the evaluator conclude what it did.
    pub diff: String,
    #[serde(default)]
    pub logs: Vec<String>,
    /// Evidence that the system reached data it was not meant to see. Retained separately from the
    /// task verdict, because a run that passed while reading hidden data is not a pass.
    #[serde(default)]
    pub hidden_data_access: Vec<String>,
}

impl Diagnostic {
    /// A diagnostic with a command and an exit state.
    pub fn new(command: impl Into<String>, exit_state: impl Into<String>) -> Self {
        Diagnostic {
            command: command.into(),
            exit_state: exit_state.into(),
            ..Diagnostic::default()
        }
    }

    /// Attach the difference the evaluator observed.
    pub fn showing(mut self, diff: impl Into<String>) -> Self {
        self.diff = diff.into();
        self
    }

    /// Record that the system reached data it should not have.
    pub fn with_hidden_access(mut self, evidence: impl Into<String>) -> Self {
        self.hidden_data_access.push(evidence.into());
        self
    }

    /// Whether this diagnostic carries anything at all.
    pub fn is_empty(&self) -> bool {
        self.command.is_empty()
            && self.exit_state.is_empty()
            && self.diff.is_empty()
            && self.logs.is_empty()
    }
}

/// What a healthy evaluator concluded about the task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskOutcome {
    Met,
    NotMet,
    /// The evaluator ran, was healthy, and the task's own predicate was inapplicable to what the
    /// system produced. Distinct from `NotMet`: the system did not fail the check, there was no
    /// check to fail.
    Inapplicable,
}

/// One evaluator's run: its health, what it concluded, and its evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvaluatorRun {
    pub evaluator: String,
    pub health: Health,
    /// Present only when the evaluator was healthy enough to reach one. A caller can set it on an
    /// unhealthy run and [`EvaluatorRun::task_outcome`] will still refuse — the field is data, the
    /// method is the gate.
    pub reached: Option<TaskOutcome>,
    #[serde(default)]
    pub diagnostic: Diagnostic,
}

impl EvaluatorRun {
    /// A healthy run that reached an outcome.
    pub fn healthy(
        evaluator: impl Into<String>,
        reached: TaskOutcome,
        diagnostic: Diagnostic,
    ) -> Self {
        EvaluatorRun {
            evaluator: evaluator.into(),
            health: Health::Healthy,
            reached: Some(reached),
            diagnostic,
        }
    }

    /// A run that failed for reasons of its own.
    pub fn unhealthy(evaluator: impl Into<String>, health: Health) -> Self {
        EvaluatorRun {
            evaluator: evaluator.into(),
            health,
            reached: None,
            diagnostic: Diagnostic::default(),
        }
    }

    /// The task outcome, or a refusal.
    ///
    /// Refuses twice over: once when the evaluator was not healthy, and once when a healthy
    /// evaluator reports a non-pass with no diagnostic to justify it.
    pub fn task_outcome(&self) -> Result<TaskOutcome, EvaluatorError> {
        if !self.health.is_healthy() {
            return Err(EvaluatorError::NotTaskEvidence {
                evaluator: self.evaluator.clone(),
                health: self.health.label().to_string(),
            });
        }
        let outcome = self.reached.ok_or_else(|| EvaluatorError::NotTaskEvidence {
            evaluator: self.evaluator.clone(),
            health: "healthy but reported no outcome".to_string(),
        })?;
        if outcome == TaskOutcome::NotMet && self.diagnostic.is_empty() {
            return Err(EvaluatorError::NoDiagnostic(self.evaluator.clone()));
        }
        Ok(outcome)
    }

    /// The reason a scoring plane should record when this run cannot produce an outcome.
    ///
    /// The bridge between the two invariants: an unhealthy evaluator becomes an unscored
    /// dimension, and [`crate::plane::ScorePlane::fold`] then refuses until somebody deals with it.
    pub fn unscored_reason(&self) -> Option<UnscoredReason> {
        if self.health.is_healthy() {
            None
        } else {
            Some(UnscoredReason::EvaluatorUnhealthy {
                evaluator: self.evaluator.clone(),
            })
        }
    }

    /// Whether this run recorded the system reaching data it should not have.
    ///
    /// Reported separately from the outcome because a pass obtained this way is 07.02's first
    /// failure mode — "a superficially successful run receives credit despite violating the
    /// intended task semantics" — and folding it into the outcome would hide which of the two
    /// happened.
    pub fn hidden_data_touched(&self) -> bool {
        !self.diagnostic.hidden_data_access.is_empty()
    }
}

/// A whole panel of evaluator runs for one result.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Panel {
    runs: Vec<EvaluatorRun>,
}

impl Panel {
    /// An empty panel.
    pub fn new() -> Self {
        Panel::default()
    }

    /// Add a run.
    pub fn record(&mut self, run: EvaluatorRun) {
        self.runs.push(run);
    }

    /// Runs whose evaluator was not healthy.
    pub fn unhealthy(&self) -> Vec<&EvaluatorRun> {
        self.runs.iter().filter(|r| !r.health.is_healthy()).collect()
    }

    /// Outcomes from the healthy runs only.
    pub fn task_outcomes(&self) -> Vec<(&str, TaskOutcome)> {
        self.runs
            .iter()
            .filter_map(|r| r.task_outcome().ok().map(|o| (r.evaluator.as_str(), o)))
            .collect()
    }

    /// Whether the panel says anything at all about the task.
    ///
    /// False when every evaluator was unhealthy. A panel in that state has measured its own
    /// harness, and reporting its result as a task score would be reporting the harness's health
    /// as the system's capability.
    pub fn says_anything(&self) -> bool {
        !self.task_outcomes().is_empty()
    }

    /// The runs, in record order.
    pub fn runs(&self) -> &[EvaluatorRun] {
        &self.runs
    }
}
