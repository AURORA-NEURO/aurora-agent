//! Typed failures for every compiler stage.
//!
//! Blueprint 06.01 asks each transformation to "emit an actionable diagnostic rather than silently
//! repairing or discarding state". Every error here names the thing that went wrong and carries the
//! evidence a reviewer would ask for next; none of them are recoverable by retrying.
//!
//! The errors are grouped by stage rather than merged into one enum. A caller minimizing a cell
//! cannot receive an oracle error, and expressing that in the type is cheaper than documenting it.

use crate::minimize::InterestSignature;
use thiserror::Error;

/// Failures of state and context minimization (06.07).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum MinimizeError {
    #[error("nothing to minimize: the candidate context is empty")]
    NothingToMinimize,

    /// Minimizing a state that is not interesting to begin with produces a smaller uninteresting
    /// state, which is worse than no reduction because it looks like progress.
    #[error(
        "the starting context does not exhibit the property to preserve: expected {expected}, observed {observed}"
    )]
    NotInterestingToBeginWith { expected: String, observed: String },

    /// Delta debugging assumes the probe is a function of the subset. A probe that answers
    /// differently for the same input makes every subsequent removal decision arbitrary, and the
    /// resulting "minimal" set is a record of coin flips.
    #[error(
        "the interest probe is not deterministic: the same {size}-item subset observed {first} then {second}"
    )]
    NondeterministicProbe {
        size: usize,
        first: String,
        second: String,
    },

    /// The independent re-check after minimization disagreed with what was preserved during it.
    #[error(
        "minimization lost the preserved property: the reduced context observes {observed}, not {expected}"
    )]
    PropertyLost { expected: String, observed: String },

    /// The 1-minimality proof failed: some remaining unit turned out to be removable after all.
    #[error(
        "result is not 1-minimal: removing unit {unit} alone still observes the preserved property"
    )]
    NotOneMinimal { unit: String },

    #[error("minimization exceeded its evaluation budget of {budget} after {spent} probes")]
    BudgetExhausted { budget: usize, spent: usize },

    #[error("context item {id} names parent {parent}, which is not present in the candidate")]
    DanglingParent { id: String, parent: String },

    #[error("context item {id} is its own ancestor; the containment graph must be a forest")]
    CyclicContainment { id: String },
}

impl MinimizeError {
    pub(crate) fn property_lost(expected: &InterestSignature, observed: &InterestSignature) -> Self {
        MinimizeError::PropertyLost {
            expected: expected.describe(),
            observed: observed.describe(),
        }
    }
}

/// Failures of candidate action set reconstruction (06.04).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ActionError {
    /// The hindsight firewall. An option justified by what happened later cannot be presented as
    /// something the agent could have weighed at the time; doing so makes every failure look
    /// obvious in retrospect and inflates localization accuracy.
    #[error(
        "candidate {action} was derived from step {from_step}, which is after the decision at step {decision_step}"
    )]
    HindsightLeak {
        action: String,
        from_step: usize,
        decision_step: usize,
    },

    #[error("step {step} is not present in the trace")]
    StepNotInTrace { step: usize },

    /// 06.03: observations and results are things that happened *to* the agent.
    #[error("step {step} is a {kind}; the agent had no alternative there, so it hosts no action set")]
    NotDecisionBearing { step: usize, kind: &'static str },
}

/// Failures of first causal divergence analysis (06.05) and attribution (06.06).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CausalError {
    #[error("a causal divergence needs at least one decision-bearing step; this trace has none")]
    NoDecisionBearingStep,

    #[error("the failing trace is empty, so there is no terminal failure to explain")]
    NoTerminalFailure,

    /// The caller asked for the failure to be pinned to a step the agent did not control.
    #[error(
        "step {step} is a {kind}: the environment produced it, so a failure cannot be attributed there"
    )]
    NotAgentControlled { step: usize, kind: &'static str },
}

/// Failures of oracle synthesis and review (06.08).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum OracleError {
    #[error("an oracle cannot be approved without a named reviewer")]
    UnattributedReview,

    /// 06.08: "Successful attacks block publication." An oracle that a recorded exploit satisfies
    /// without fulfilling task intent is a scoring bug with a review stamp on it.
    #[error("exploit {attack} scores as a pass without fulfilling task intent; review cannot clear it")]
    UnrebuttedExploit { attack: String },

    /// 06.08 asks each proposal to list what it cannot see. An empty list is almost always an
    /// unexamined oracle rather than an omniscient one.
    #[error("oracle {oracle} declares no blind spots; gap analysis is mandatory before review")]
    NoGapAnalysis { oracle: String },

    #[error("oracle {oracle} accepts no verdict at all, so nothing can ever pass it")]
    EmptyAcceptanceSet { oracle: String },

    /// A model judge or statistical tolerance may not be the only thing standing between a
    /// candidate and a passing grade; 35.08 quality gate 3 wants a non-model oracle on the primary
    /// defect wherever one is feasible.
    #[error(
        "oracle {oracle} is {strength}, which cannot stand alone: pair it with a deterministic check"
    )]
    WeakOracleAlone {
        oracle: String,
        strength: &'static str,
    },
}

/// Failures of counterfactual cell generation (06.09).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CounterfactualError {
    /// 06.09's matched design: everything except the intervened factor is held constant. A pair
    /// that moved two things measures neither.
    #[error("counterfactual pair is not matched: {fields:?} differ in addition to the intervention")]
    UnmatchedPair { fields: Vec<String> },

    #[error("intervention on {factor} changed nothing; the pair is a duplicate, not a contrast")]
    NullIntervention { factor: String },

    /// 06.09 realism check. An unreachable state tests an agent against a world that cannot exist.
    #[error("intervention on {factor} produces an incoherent state: {reason}")]
    IncoherentState { factor: String, reason: String },

    #[error("the source cell and the follow-up cell carry the same id {cell_id}")]
    CollidingCellIds { cell_id: String },
}

/// Failures of the assembled pipeline (06.01).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CompileError {
    #[error("causal analysis failed: {0}")]
    Causal(#[from] CausalError),

    #[error("minimization failed: {0}")]
    Minimize(#[from] MinimizeError),

    #[error("action reconstruction failed: {0}")]
    Action(#[from] ActionError),

    #[error("oracle synthesis failed: {0}")]
    Oracle(#[from] OracleError),

    /// The divergence landed somewhere no cell can sit. Reported rather than nudged to a nearby
    /// step, because a cell at a step the agent did not control measures nothing.
    #[error("no compilable decision found in trace {trace_id}: {reason}")]
    NotCompilable { trace_id: String, reason: String },
}
