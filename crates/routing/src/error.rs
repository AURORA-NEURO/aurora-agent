//! Typed routing failures.
//!
//! Blueprint 09.01 requires the Inference Lab to "fail closed where integrity or safety is
//! affected, preserve the underlying evidence, and emit an actionable diagnostic rather than
//! silently repairing or discarding state". Every variant below is a condition under which a
//! routing number would be *misleading* if produced anyway.
//!
//! [`RoutingError::EvidenceLeak`] is the one that matters most. A router evaluated on evidence
//! that contains the task it is routing is not a router at all — it is the oracle retrospective
//! selector wearing a router's name — and that mistake produces a flattering number rather than
//! a visible crash. It is therefore a hard error, not a warning.

use thiserror::Error;

#[derive(Debug, Error, PartialEq)]
pub enum RoutingError {
    #[error("the approved architecture set is empty; there is nothing to route between")]
    NoApprovedArchitectures,

    #[error(
        "architecture `{architecture}` is not in the approved set; blueprint 09.08 forbids the \
         router from selecting configurations that were never reviewed"
    )]
    UnapprovedArchitecture { architecture: String },

    #[error("evidence holds two observations for task `{task}` and architecture `{architecture}`")]
    DuplicateObservation { task: String, architecture: String },

    #[error(
        "the evidence offered for routing task `{task}` contains that task's own outcome; that is \
         retrospective selection, not routing, and it would silently inflate the router's score"
    )]
    EvidenceLeak { task: String },

    #[error(
        "evidence has no observation of architecture `{architecture}` on task `{task}`; the four \
         comparators must be scored on exactly the same tasks or their means are not comparable"
    )]
    IncompleteEvidence { task: String, architecture: String },

    #[error("a task identifier must not be empty")]
    EmptyTaskId,

    #[error("no tasks were supplied; a routing report over zero tasks would report nothing")]
    NoTasks,

    #[error("policy field `{field}` must be finite and in range, got {value}")]
    InvalidParameter { field: &'static str, value: f64 },

    #[error("a calibration curve needs at least one bin")]
    NoCalibrationBins,

    #[error(
        "architecture `{architecture}` reports baseline strategy name `{actual}` but its own \
         label is `{expected}`; evidence rows would be attributed to the wrong architecture"
    )]
    ArchitectureLabelMismatch {
        architecture: String,
        expected: String,
        actual: String,
    },

    /// The comparison harness had no reference verdict to measure this task against.
    ///
    /// Nothing about the task's outcomes is observable, so no row of the ledger can be filled. The
    /// alternative — recording every architecture as failing — would teach the router that they all
    /// lose on this structural regime, which is a claim the evidence does not make.
    #[error(
        "task `{task}` has no full-context reference verdict, so none of its architectures can be \
         observed: {detail}"
    )]
    TaskNotComparable { task: String, detail: String },

    /// The oracle refused one architecture's selection on an otherwise comparable task.
    ///
    /// A ledger row is a *measured* outcome, and there is no honest value for
    /// [`crate::evidence::Observation::verdict_preserving`] here. `false` would enter the router's
    /// training evidence as a demonstrated failure of an architecture nobody evaluated, which is
    /// exactly the "unmeasured is not zero" collapse the utility encoding exists to prevent;
    /// dropping the row instead would surface later as [`RoutingError::IncompleteEvidence`] with
    /// the cause erased.
    #[error(
        "the oracle refused architecture `{architecture}`'s selection on task `{task}`, and an \
         unobserved outcome must not enter the ledger as a measured one: {detail}"
    )]
    UnjudgedObservation {
        task: String,
        architecture: String,
        detail: String,
    },
}
