//! Typed failures, and the typed *absence* of a guarantee.
//!
//! Two things go wrong in this crate and they are deliberately different types, for the same
//! reason `bioprism-influence` separates them: a malformed input and an unearned guarantee are
//! not the same state, and collapsing them lets a caller read "no error" as "guaranteed".
//!
//! [`EpistemicError`] is a caller bug or an input the calculus cannot accept — a loss matrix with
//! a non-finite entry, a belief with zero total mass, an acquisition whose outcome likelihoods do
//! not sum to one under some model, a ground set larger than the exhaustive checks will enumerate.
//!
//! [`crate::theorem::Applicability`] is the opposite: a well-formed request for which a named
//! approximation guarantee does not hold. That is a *successful* outcome that reports the absence
//! of a bound, and it carries the precondition that failed.
//!
//! Nothing here refuses by returning a sentinel number. Blueprint 43.14 requires that
//! "approximation guarantees are reported only under verified assumptions"; a function that
//! returned `0.0` for "no guarantee" would make an unverified assumption indistinguishable from a
//! verified guarantee of zero quality.

use thiserror::Error;

/// A malformed request. Never used to represent "this guarantee does not apply".
#[derive(Debug, Clone, PartialEq, Error)]
pub enum EpistemicError {
    #[error("a decision problem needs at least one action and at least one model; got {actions} actions and {models} models")]
    EmptyDecisionProblem { actions: usize, models: usize },

    #[error("loss matrix has {got} entries but {actions} actions x {models} models needs {want}")]
    LossMatrixShape {
        got: usize,
        want: usize,
        actions: usize,
        models: usize,
    },

    #[error("loss entry for action {action} under model {model} is {value}, which is not a finite real; a decision loss that is NaN or infinite has no minimiser")]
    NonFiniteLoss {
        action: usize,
        model: usize,
        value: f64,
    },

    #[error("a belief over {models} models was given {got} masses")]
    BeliefShape { models: usize, got: usize },

    #[error("belief mass {value} at model {model} is negative or non-finite")]
    InadmissibleBeliefMass { model: usize, value: f64 },

    #[error("the belief has total mass {mass}; it cannot be normalised, and a belief of total mass zero is not the uniform belief")]
    DegenerateBelief { mass: f64 },

    #[error("likelihood {value} for model {model} of evidence {item:?} is negative or non-finite")]
    InadmissibleLikelihood {
        item: String,
        model: usize,
        value: f64,
    },

    #[error("evidence {item:?} declares {got} likelihoods for a problem with {models} models")]
    LikelihoodShape {
        item: String,
        got: usize,
        models: usize,
    },

    #[error("evidence {item:?} assigns zero likelihood to every model; conditioning on it annihilates the posterior, which is a contradiction rather than an observation")]
    AnnihilatingEvidence { item: String },

    #[error("acquisition {action:?} has outcome likelihoods summing to {sum} under model {model}; an acquisition must produce exactly one of its outcomes, so the column must sum to 1")]
    ImproperAcquisition {
        action: String,
        model: usize,
        sum: f64,
    },

    #[error("acquisition {action:?} declares no outcomes; an evidence action with no possible result cannot be valued")]
    OutcomelessAcquisition { action: String },

    #[error("cost {value} for {item:?} is negative or non-finite")]
    InadmissibleCost { item: String, value: f64 },

    #[error("duplicate identifier {id:?} in {collection}; identity has to be a key for the result to be replayable")]
    DuplicateIdentifier { collection: String, id: String },

    #[error("no member of {collection} is named {id:?}")]
    UnknownIdentifier { collection: String, id: String },

    #[error("the decision quotient needs at least one permitted action; an empty decision boundary would make every model vacuously equivalent")]
    EmptyPermittedActionSet,

    #[error("element {element} is outside a ground set of size {ground}")]
    ElementOutOfRange { element: usize, ground: usize },

    #[error("exhaustive enumeration over a ground set of {ground} elements needs {needed} evaluations, above the cap of {cap}; a check that silently switched to sampling here would be weakest exactly where the guarantee is most interesting")]
    ExhaustiveCapExceeded {
        ground: usize,
        needed: u64,
        cap: u64,
    },

    #[error("the protected closure costs {protected} and the budget is {budget}; blueprint 43.14 forbids trimming a mandatory closure to fit, so this is a refusal rather than a smaller selection")]
    ProtectedClosureExceedsBudget { protected: f64, budget: f64 },

    #[error("a cardinality constraint of {cardinality} cannot hold the protected closure of {protected} elements")]
    ProtectedClosureExceedsCardinality {
        cardinality: usize,
        protected: usize,
    },

    #[error("no constraint was given; an unconstrained selection problem has the whole ground set as its optimum and measures nothing")]
    UnconstrainedSelection,

    #[error("message from {from:?} to {to:?} mentions variable {variable:?}, which is not in their separator {separator:?}; sending it would leak a private local variable")]
    VariableOutsideSeparator {
        from: String,
        to: String,
        variable: String,
        separator: Vec<String>,
    },

    #[error("factor {factor:?} declares scope {scope:?} and carries {got} table entries; a binary factor over {arity} variables needs {want}")]
    FactorTableShape {
        factor: String,
        scope: Vec<String>,
        got: usize,
        want: usize,
        arity: usize,
    },

    #[error("factor {factor:?} repeats variable {variable:?} in its scope")]
    RepeatedVariableInScope { factor: String, variable: String },

    #[error(
        "factor {factor:?} entry {index} is {value}, which is not a finite non-negative potential"
    )]
    InadmissiblePotential {
        factor: String,
        index: usize,
        value: f64,
    },

    #[error("agent {agent:?} was assigned factor {factor:?}, which no other agent may also hold; a factor counted twice is counted twice in the product")]
    FactorAssignedTwice { agent: String, factor: String },

    #[error("the partition assigns no factors to agent {agent:?}")]
    EmptyAgent { agent: String },

    #[error("lens {lens:?} cannot focus this document: {detail}")]
    FocusFailed { lens: String, detail: String },

    #[error("lens {lens:?} is a {kind} and has no lawful put; it is a one-way read, and 43.49 says a non-lawful update optic becomes a request API rather than a silent write")]
    NoLawfulPut { lens: String, kind: &'static str },

    #[error("lens {lens:?} focuses {foci} values and was given {values} to put back")]
    PutArity {
        lens: String,
        foci: usize,
        values: usize,
    },

    #[error("cannot compose a view indexed by {left:?} with one indexed by {right:?}; no transform is registered between them, and erasing the index would preserve a claim that depends on it")]
    UnregisteredIndexTransform { left: String, right: String },

    #[error("the query document is not valid against {schema}: {detail}")]
    QueryRejected { schema: String, detail: String },

    #[error("distortion tolerance {value} is negative or non-finite")]
    InadmissibleTolerance { value: f64 },
}
