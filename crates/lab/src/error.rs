//! Typed failures for the inference lab.
//!
//! Blueprint section 09 states the same mitigation eleven times: *fail closed where integrity or
//! safety is affected, preserve the underlying evidence, and emit an actionable diagnostic rather
//! than silently repairing or discarding state.* Every variant here is one instance of that
//! sentence, and the ones that matter most are in [`HoldoutError`], because a contaminated
//! measurement is the failure this crate exists to make unrepresentable.
//!
//! The errors derive `Serialize` and `Deserialize` because an [`crate::evolution::EvolutionCard`]
//! that failed cleanliness must record *which* refusal it hit. A card that says only "not clean"
//! is a card that will be argued with.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Failures in the architecture search space (09.04).
#[derive(Debug, Clone, PartialEq, Eq, Error, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "error")]
pub enum SpaceError {
    #[error("configuration `{0}` is already registered; bundles are immutable versions")]
    DuplicateConfiguration(String),

    #[error("configuration `{0}` is not registered")]
    UnknownConfiguration(String),

    #[error("component `{component}` is wired to `{target}`, which is not in the candidate")]
    DanglingEdge { component: String, target: String },

    #[error("the candidate's component graph contains a cycle through `{0}`")]
    ComponentCycle(String),

    #[error("component `{component}` is duplicated in the candidate")]
    DuplicateComponent { component: String },

    #[error("candidate `{candidate}` declares `{surface}`, which is a protected surface and may not be evolved")]
    ProtectedSurfaceTouched { candidate: String, surface: String },

    #[error("candidate `{candidate}` omits a required component kind: {kind}")]
    MissingRequiredComponent { candidate: String, kind: String },

    #[error("lineage of `{0}` is cyclic; a configuration cannot be its own ancestor")]
    LineageCycle(String),

    #[error("candidate `{candidate}` declares parent `{parent}`, which is not registered")]
    UnregisteredParent { candidate: String, parent: String },
}

/// Failures in hypothesis separation (09.03).
#[derive(Debug, Clone, PartialEq, Eq, Error, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "error")]
pub enum SeparationError {
    #[error("hypothesis `{0}` is already in the set")]
    DuplicateHypothesis(String),

    #[error("hypothesis `{incoming}` has the same assumption signature as `{existing}`; 09.03 deduplicates by assumptions, not wording")]
    DuplicateByAssumptions { incoming: String, existing: String },

    #[error("hypothesis `{0}` is not in the set")]
    UnknownHypothesis(String),

    #[error("separation needs at least two live hypotheses; the set has {0}")]
    TooFewHypotheses(usize),

    #[error("the evidence would retire every live hypothesis; that refutes the hypothesis set, not one member of it")]
    WouldRetireAll,

    #[error("obligation `{0}` names a discriminator but is absent from the obligation graph")]
    ObligationMissing(String),

    #[error("the obligation graph is malformed: {0}")]
    ObligationGraph(String),
}

/// Failures in holdout accounting (09.11). The centre of the crate.
#[derive(Debug, Clone, PartialEq, Eq, Error, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "error")]
pub enum HoldoutError {
    #[error("holdout `{holdout}` was used to select configuration `{configuration}` at exposure event {event}; a later score on it is not a measurement")]
    SelectedUsingThisHoldout {
        holdout: String,
        configuration: String,
        event: usize,
    },

    #[error("holdout `{holdout}` was already queried for configuration `{configuration}`; a repeat query adds selection pressure and no information")]
    AlreadyQueried {
        holdout: String,
        configuration: String,
    },

    #[error("holdout `{holdout}` was exposed to `{ancestor}`, an ancestor of `{configuration}`; the descendant inherits the contamination")]
    AncestorExposed {
        holdout: String,
        configuration: String,
        ancestor: String,
    },

    #[error("holdout `{holdout}` has spent its query budget ({used}/{budget}); repeated optimization has retired it as a holdout")]
    Retired {
        holdout: String,
        used: u32,
        budget: u32,
    },

    #[error("partition `{partition}` is a development surface; a score on it is never a certification measurement")]
    NotACertificationSurface { partition: String },

    #[error("holdout `{0}` is not registered in this ledger")]
    UnknownHoldout(String),

    #[error("holdout `{0}` is already registered")]
    DuplicateHoldout(String),

    #[error("metric `{metric}` on holdout `{holdout}` was given the non-finite value {value}")]
    NonFiniteValue {
        holdout: String,
        metric: String,
        value: String,
    },

    #[error("the two measurements compare `{left}` against `{right}`; a delta needs one metric")]
    MetricMismatch { left: String, right: String },

    #[error("the two measurements come from holdouts `{left}` and `{right}`; a delta needs one surface")]
    SurfaceMismatch { left: String, right: String },
}

/// Failures in rollback (09.11).
#[derive(Debug, Clone, PartialEq, Eq, Error, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "error")]
pub enum RollbackError {
    #[error("checkpoint names configuration `{0}`, which is not in the bundle registry; rollback restores a whole known-good bundle or nothing")]
    BundleMissing(String),

    #[error("checkpoint covers holdouts {covered:?} but the ledger now holds {present:?}; a checkpoint cannot be applied across a changed holdout set")]
    HoldoutSetChanged {
        covered: Vec<String>,
        present: Vec<String>,
    },

    #[error("checkpoint watermark for holdout `{holdout}` is {watermark}, ahead of the ledger's {current}; exposure history is append-only and cannot run backwards")]
    WatermarkAhead {
        holdout: String,
        watermark: usize,
        current: usize,
    },
}

/// Failures in Pareto accounting (09.09).
#[derive(Debug, Clone, PartialEq, Eq, Error, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "error")]
pub enum ParetoError {
    #[error("front has no objectives; dominance over an empty axis set would make everything equivalent")]
    NoObjectives,

    #[error("objective `{0}` is declared twice")]
    DuplicateObjective(String),

    #[error("candidate `{candidate}` carries value for `{axis}`, which is not an objective of this front")]
    UnknownAxis { candidate: String, axis: String },

    #[error("candidate `{candidate}` says nothing at all about objective `{axis}`; an absent axis must be recorded as unmeasured with a reason, not left out")]
    AxisAbsent { candidate: String, axis: String },

    #[error("candidate `{candidate}` is already on the front")]
    DuplicateCandidate { candidate: String },

    #[error("objective `{axis}` of candidate `{candidate}` is the non-finite value {value}")]
    NonFiniteValue {
        candidate: String,
        axis: String,
        value: String,
    },
}

/// Failures in benchmark-gated self-improvement (09.10).
#[derive(Debug, Clone, PartialEq, Eq, Error, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "error")]
pub enum EvolutionError {
    #[error("card `{card}` was measured on a contaminated surface: {reason}")]
    ContaminatedSurface { card: String, reason: String },

    #[error("card `{0}` states no defeater; a self-improvement record that cannot be wrong is not a record")]
    NoDefeaterStated(String),

    #[error("card `{card}` names no changed artifact")]
    NoChangedArtifacts { card: String },

    #[error("card `{card}` proposes to change `{surface}`, which 09.10 protects from evolution")]
    ProtectedSurface { card: String, surface: String },

    #[error("card `{card}` reports a delta of {delta} on `{metric}`, which is not an improvement")]
    NotAnImprovement {
        card: String,
        metric: String,
        delta: String,
    },

    #[error("card `{card}` has no rollback handle; 09.10 requires one on every accepted change")]
    NoRollbackHandle { card: String },

    #[error(transparent)]
    Holdout(#[from] HoldoutError),
}

/// Failures in risk-triggered branching (09.07) and context-value expansion (09.02).
#[derive(Debug, Clone, PartialEq, Eq, Error, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "error")]
pub enum LabError {
    #[error("branch policy would spend {requested} branches against a hard ceiling of {ceiling}")]
    BranchCeilingExceeded { requested: u32, ceiling: u32 },

    #[error("branch policy would spend {requested} verifier calls against a hard ceiling of {ceiling}")]
    VerifierCeilingExceeded { requested: u32, ceiling: u32 },

    #[error("branch rule `{0}` states no trigger predicate; an unconditional escalation is not risk-triggered")]
    UntriggeredRule(String),

    #[error("acquisition action `{action}` targets obligation `{obligation}`, which is not in the graph")]
    ActionTargetsUnknownObligation { action: String, obligation: String },

    #[error("acquisition action `{0}` targets no obligation; expansion by expected decision value needs a decision to serve")]
    ActionTargetsNoObligation(String),

    #[error("acquisition action `{action}` declares the non-finite value {value}")]
    NonFiniteValue { action: String, value: String },

    #[error("acquisition action `{action}` declares zero token and latency cost; a free action would sort ahead of every real one")]
    CostlessAction { action: String },

    #[error(transparent)]
    Separation(#[from] SeparationError),

    #[error(transparent)]
    Space(#[from] SpaceError),

    #[error(transparent)]
    Holdout(#[from] HoldoutError),

    #[error(transparent)]
    Rollback(#[from] RollbackError),

    #[error(transparent)]
    Pareto(#[from] ParetoError),

    #[error(transparent)]
    Evolution(#[from] EvolutionError),
}
