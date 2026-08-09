//! Every refusal this crate can make, in one place.
//!
//! Each variant exists because some caller wanted a number and the evidence did not support one.
//! The variants carry the *reason* rather than a code, because a refusal a caller cannot explain to
//! a reviewer is a refusal that will be worked around.

use thiserror::Error;

/// Refusals from [`crate::plane`], the scoring plane (26.17, 07.05).
#[derive(Debug, Clone, PartialEq, Error)]
pub enum PlaneError {
    #[error("dimension `{0}` is already present on this plane")]
    DuplicateDimension(String),
    #[error("dimension `{0}` is not on this plane")]
    UnknownDimension(String),
    #[error(
        "cannot fold a plane with unscored dimensions: {unscored:?} were never measured; \
         name a policy that says what to do with them"
    )]
    UnscoredDimensions { unscored: Vec<String> },
    #[error(
        "dimension `{dimension}` is out of tier for this system: it declared tier `{declared}` \
         and the dimension requires `{required}`"
    )]
    OutOfTier {
        dimension: String,
        declared: String,
        required: String,
    },
    #[error("weight for dimension `{0}` is not finite and positive")]
    BadWeight(String),
    #[error("a fold needs at least one dimension")]
    Empty,
    #[error("score for dimension `{dimension}` is {value}, outside the unit interval")]
    ScoreOutOfRange { dimension: String, value: f64 },
}

/// Refusals from [`crate::mesh`], evaluator independence (26.01).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum MeshError {
    #[error("evaluator `{0}` is declared twice")]
    DuplicateEvaluator(String),
    #[error("evaluator `{0}` is not declared in this mesh")]
    UnknownEvaluator(String),
    #[error(
        "evaluator `{evaluator}` derives from the system under evaluation via `{artifact}`: \
         a verdict from it would be the system grading itself"
    )]
    CircularOracle { evaluator: String, artifact: String },
    #[error("a mesh needs at least one evaluator")]
    Empty,
    #[error(
        "evaluators {class:?} share an evidence source and called {positions:?}: they are one \
         vote, and one vote cannot hold two positions. Resolve the evaluator defect rather than \
         letting the split become a distribution"
    )]
    ClassSplit {
        class: Vec<String>,
        positions: Vec<String>,
    },
}

/// Refusals from [`crate::grounding`], claim-to-evidence resolution (26.03).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum GroundingError {
    #[error("claim `{0}` is declared twice")]
    DuplicateClaim(String),
    #[error("support edge references claim `{0}`, which is not in the claim set")]
    UnknownClaim(String),
    #[error("support edge references evidence `{0}`, which is not in the evidence set")]
    UnknownEvidence(String),
    #[error("evidence `{0}` is declared twice")]
    DuplicateEvidence(String),
}

/// Refusals from [`crate::acquisition`], information-acquisition accounting (26.05).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum AcquisitionError {
    #[error("action `{0}` appears twice in one acquisition trace")]
    DuplicateAction(String),
    #[error("action `{action}` closes obligation `{obligation}`, which was not open")]
    UnopenedObligation { action: String, obligation: String },
    #[error(
        "regret needs a reference acquisition policy; none was supplied, and 26.05 defines \
         no default policy to regret against"
    )]
    NoReferencePolicy,
}

/// Refusals from [`crate::burden`], the nonrenewable-resource ledger (26.06).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum BurdenError {
    #[error("resource `{0}` is declared twice")]
    DuplicateResource(String),
    #[error("resource `{0}` was never declared")]
    UnknownResource(String),
    #[error(
        "fork `{fork}` draws {requested} of `{resource}` but only {remaining} remains on the \
         branch it inherited"
    )]
    Overdraw {
        fork: String,
        resource: String,
        requested: u64,
        remaining: u64,
    },
    #[error(
        "fork `{fork}` and fork `{other}` both consume `{resource}`; a specimen aliquot spent on \
         one branch is not available on the other"
    )]
    ForkDoubleSpend {
        fork: String,
        other: String,
        resource: String,
    },
    #[error("resource `{resource}` is quoted in `{left}` here and `{right}` there")]
    UnitMismatch {
        resource: String,
        left: String,
        right: String,
    },
}

/// Refusals from [`crate::worldline`], the availability audit (26.07).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum WorldlineError {
    #[error("observation `{0}` is declared twice")]
    DuplicateObservation(String),
    #[error(
        "observation `{observation}` was recorded at {recorded} but measured at {measured}: \
         a record cannot precede the measurement it records"
    )]
    RecordedBeforeMeasured {
        observation: String,
        measured: String,
        recorded: String,
    },
    #[error(
        "observation `{observation}` was measured at {measured} but occurred at {occurred}: \
         a measurement cannot precede the biology it measures"
    )]
    MeasuredBeforeOccurred {
        observation: String,
        occurred: String,
        measured: String,
    },
    #[error(
        "observation `{observation}` became accessible at {accessible}, before it was recorded \
         at {recorded}"
    )]
    AccessibleBeforeRecorded {
        observation: String,
        recorded: String,
        accessible: String,
    },
}

/// Refusals from [`crate::estimand`], causal declaration (26.09).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum EstimandError {
    #[error("estimand is missing a declared {0}; 26.09 step 1 requires all five")]
    MissingElement(&'static str),
    #[error(
        "a model-conditional finding cannot be promoted to `{target}`: 26.09 says simulator \
         conclusions are \"never upgraded automatically to real-world truth\""
    )]
    NoAutomaticPromotion { target: String },
    #[error("transport to `{to}` was requested but the estimand's scope is `{from}`")]
    OutOfScope { from: String, to: String },
}

/// Refusals from [`crate::repro`], reproducibility certification (26.11).
#[derive(Debug, Clone, PartialEq, Error)]
pub enum ReproError {
    #[error("re-execution produced no comparable outputs, so there is nothing to certify")]
    NothingCompared,
    #[error("output `{0}` appears twice in one comparison set")]
    DuplicateOutput(String),
    #[error(
        "a reproducibility certificate is not a validity claim: `{0}` asked this certificate to \
         support a conclusion about the biology"
    )]
    NotAValidityClaim(String),
    #[error("tolerance for `{output}` is {tolerance}, which is not finite and non-negative")]
    BadTolerance { output: String, tolerance: f64 },
}

/// Refusals from [`crate::metamorphic`], mutation-response scoring (26.12).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum MetamorphicError {
    #[error("trial `{0}` appears twice in one family")]
    DuplicateTrial(String),
    #[error("a metamorphic family needs at least one trial")]
    EmptyFamily,
    #[error(
        "trial `{trial}` declares relation `{relation}` but the family declares `{family}`; \
         consistency across a family means one relation, not an average of several"
    )]
    RelationMismatch {
        trial: String,
        relation: String,
        family: String,
    },
}

/// Refusals from [`crate::reveal`], sealed prospective evaluation (26.16).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum RevealError {
    #[error("this registration is already sealed; commitments cannot be added after the seal")]
    AlreadySealed,
    #[error("the outcome has already been revealed")]
    AlreadyRevealed,
    #[error("nothing was committed before the seal, so there is nothing to score")]
    NothingCommitted,
    #[error(
        "the rubric presented at scoring hashes to {presented}, but {sealed} was sealed before \
         the reveal; 26.16 forbids scoring \"with retrospective rubric changes\""
    )]
    RubricChanged { sealed: String, presented: String },
    #[error("commitment `{0}` was made twice")]
    DuplicateCommitment(String),
    #[error("no commitment named `{0}` was sealed, so the reveal has nothing to score it against")]
    UncommittedOutcome(String),
}

/// Refusals from [`crate::design`], matched counterfactual designs (26.18).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DesignError {
    #[error("arm `{0}` is declared twice")]
    DuplicateArm(String),
    #[error("a design needs a baseline arm and at least one other")]
    TooFewArms,
    #[error("arm `{arm}` does not assign factor `{factor}`, which the design declares")]
    UnassignedFactor { arm: String, factor: String },
    #[error("arm `{arm}` assigns factor `{factor}`, which the design does not declare")]
    UndeclaredFactor { arm: String, factor: String },
    #[error("arm `{arm}` is identical to arm `{other}` on every declared factor")]
    DuplicateCell { arm: String, other: String },
}

/// Refusals from [`crate::evaluator`], evaluator health (07.02).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum EvaluatorError {
    #[error(
        "evaluator `{evaluator}` was {health}, so its outcome is not evidence about the task; \
         reading it as a task failure would blame the system for the harness"
    )]
    NotTaskEvidence { evaluator: String, health: String },
    #[error("evaluator `{0}` reported no diagnostic; 07.02 requires evidence-bearing diagnostics")]
    NoDiagnostic(String),
}

/// Refusals from [`crate::trajectory`], path evaluation (07.03).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum TrajectoryError {
    #[error("step index {0} is past the end of the trajectory")]
    StepOutOfRange(usize),
    #[error("property `{0}` is declared twice")]
    DuplicateProperty(String),
    #[error(
        "a bounded-suffix score needs a declared horizon; scoring to the end of the trajectory \
         makes the number depend on how long the run happened to be"
    )]
    NoHorizon,
}

/// Refusals from [`crate::boundary`], contextual integrity (07.09).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum BoundaryError {
    #[error("policy `{0}` is declared twice")]
    DuplicatePolicy(String),
    #[error(
        "a utility-and-safety composite is refused: {violations} violation(s) stand, and 07.09 \
         requires reporting a Pareto curve rather than \"a combined score that allows high task \
         success to erase privacy violations\""
    )]
    CompositeRefused { violations: usize },
    #[error("flow `{0}` names no transmission principle, so no policy can be checked against it")]
    NoTransmissionPrinciple(String),
}

/// Refusals from [`crate::waiver`], release-gate waivers (07.13).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum WaiverError {
    #[error("a waiver must name an authorising party")]
    NoAuthoriser,
    #[error("a waiver must carry a rationale")]
    NoRationale,
    #[error("a waiver must name at least one affected version")]
    NoAffectedVersion,
    #[error("a waiver must state required follow-up")]
    NoFollowUp,
    #[error(
        "gate `{gate}` is a safety veto and cannot be waived; 07.09's rule is that a materialized \
         forbidden action is a veto, and a veto that can be signed away is a warning"
    )]
    VetoNotWaivable { gate: String },
    #[error("waiver for gate `{gate}` expired at {expiry}; the gate is in force again")]
    Expired { gate: String, expiry: String },
    #[error("gate `{0}` was not blocking, so there is nothing to waive")]
    NotBlocking(String),
}
