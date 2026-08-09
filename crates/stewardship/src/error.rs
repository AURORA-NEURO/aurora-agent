//! Typed refusals.
//!
//! Every error here is a governance rule that was about to be broken, named as the rule rather than
//! as a symptom. `InvalidInput { field }` would tell a caller where the compiler stopped;
//! [`ReviewError::SelfReview`] tells them which sentence of section 14 they walked into.
//!
//! The errors are grouped by the module that raises them rather than gathered into one enum,
//! because the groups have genuinely different audiences: a reviewer sees [`ReviewError`], a
//! statistician [`PredeclarationError`], a communications lead [`ClaimError`], a data steward
//! [`AccessError`]. [`StewardshipError`] exists only for callers that want one return type.

use crate::id::{ActorId, Epoch, Role};
use thiserror::Error;

/// Refusals from the oracle and evaluator review process of 14.09, and 14.08's rule that an
/// approval names dimensions.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ReviewError {
    /// 14.03: "Author cannot provide the only approval for scoring semantics."
    #[error("{reviewer} authored the evaluator under review; an approval needs a reviewer who did not build it")]
    SelfReview { reviewer: ActorId },

    /// The reviewer's declared role is one that may not approve alone.
    #[error("a party acting as {role} cannot be the sole approver")]
    NotIndependent { role: Role },

    /// 14.09 makes gap analysis mandatory: a dimension nobody checked is not a dimension that
    /// passed, and the mandatory ones cannot be left unchecked at all.
    #[error("mandatory review dimension `{dimension}` was not checked: {why}")]
    MandatoryDimensionNotChecked {
        dimension: &'static str,
        why: String,
    },

    /// An approval that names a failing dimension is a contradiction, so it is not issued.
    #[error("review dimension `{dimension}` failed: {defect}")]
    DimensionFailed {
        dimension: &'static str,
        defect: String,
    },

    /// A dimension recorded as passed when the corpus held no material to test it with.
    #[error("dimension `{dimension}` was recorded as passed but the corpus contains no `{missing}` cases")]
    PassedWithoutCorpus {
        dimension: &'static str,
        missing: &'static str,
    },

    /// 14.09's test corpus is the evidence base of the whole review.
    #[error("the review corpus is empty; a review with no test cases is an opinion")]
    EmptyCorpus,

    /// 14.09: "Any scoring-semantic change creates a new evaluator revision."
    #[error("scoring semantics changed from {from} to {to} but the version bump was {observed} where {required} was required")]
    ScoringSemanticsUnderVersioned {
        from: String,
        to: String,
        observed: String,
        required: String,
    },

    /// A revision that claims to succeed another but shares its scoring digest is not a revision.
    #[error("revision {revision} declares a scoring change but carries the predecessor's scoring digest")]
    RevisionWithoutChange { revision: String },

    /// Two dimensions recorded for the same review.
    #[error("dimension `{dimension}` was recorded twice; a review has one finding per dimension")]
    DuplicateDimension { dimension: &'static str },

    /// 14.09: historical results are not silently recomputed under new meaning.
    #[error("this approval was issued against scoring digest {approved} and does not carry to {current}")]
    ApprovalDoesNotCarry { approved: String, current: String },
}

/// Refusals from the metric, score and statistical governance of 14.10.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PredeclarationError {
    /// The confirmatory claim's whole content: this metric was named before the results existed.
    #[error("metric `{metric}` was not predeclared in the sealed plan; it can support an exploratory finding and nothing more")]
    MetricNotPredeclared { metric: String },

    /// A plan sealed after the results were observed is a description of them.
    #[error("the plan was sealed at {sealed} but the results were observed at {observed}")]
    PlanSealedAfterResults { sealed: Epoch, observed: Epoch },

    /// An exclusion that was applied but never declared.
    #[error("trials with disposition `{disposition}` were excluded under no declared rule")]
    UndeclaredExclusion { disposition: &'static str },

    /// The one exclusion that cannot be declared: dropping trials that produced a score.
    #[error("an exclusion rule cannot target scored trials; that is selective reporting with a rule attached")]
    ExclusionOfScoredTrials,

    /// A metric definition changed under a version that already existed.
    #[error(
        "metric `{metric}` version {version} is already registered with a different construct"
    )]
    MetricRedefinedInPlace { metric: String, version: String },

    /// A plan with no primary metric declares nothing.
    #[error("the analysis plan declares no primary metric")]
    NoPrimaryMetric,

    /// A result for a metric that was not measured at all.
    #[error("the results carry no value for metric `{metric}`")]
    MetricNotMeasured { metric: String },

    /// 14.10: generated siblings and repeated trials are not independent.
    #[error("the plan declares {declared} independent units but the results report {observed} trials with no clustering unit")]
    ClusteringUnitAbsent { declared: usize, observed: usize },

    /// A sealed plan whose digest does not match its content.
    #[error("the sealed plan's digest {stated} does not match its content {computed}")]
    SealBroken { stated: String, computed: String },

    /// Canonicalisation of the plan failed.
    #[error("the analysis plan could not be canonicalised: {detail}")]
    NotCanonical { detail: String },
}

/// Refusals from result-claim and leaderboard governance, 14.11.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ClaimError {
    /// The evidence ladder of 43.40 caps what may be said; the class asked for more.
    #[error(
        "a {class} claim needs evidence at tier {required} but the atlas licenses only {available}"
    )]
    EvidenceBelowClass {
        class: &'static str,
        required: String,
        available: String,
    },

    /// 14.11's eligibility rule: a stable pack.
    #[error("a {class} claim needs a pack at trust tier {required} but the pack earned {earned}")]
    PackTierBelowClass {
        class: &'static str,
        required: String,
        earned: String,
    },

    /// A sentence that omits something 14.11 requires it to name.
    #[error("a {class} claim must state its {missing}")]
    SentenceIncomplete {
        class: &'static str,
        missing: &'static str,
    },

    /// 14.11: "'Best' requires defined comparison set and date."
    #[error("a superlative claim needs a named comparison set and an as-of epoch")]
    SuperlativeWithoutComparisonSet,

    /// 14.11: "'general' requires broad validated coverage."
    #[error("a generality claim needs at least {required} validated domains; {stated} were named")]
    GeneralityWithoutBreadth { required: usize, stated: usize },

    /// A causal claim resting on a metric chosen after the fact.
    #[error("a research causal claim requires a confirmatory finding from a predeclared analysis")]
    CausalClaimWithoutPredeclaration,

    /// 14.11: an independently reproduced result reproduced by its own publisher is not one.
    #[error("{party} both published and reproduced this result")]
    SelfReproduction { party: ActorId },

    /// A claim whose subject is already disputed.
    #[error("claim {claim} is under dispute and cannot be promoted or restated")]
    ClaimDisputed { claim: String },

    /// 14.11's correction rule: nothing is erased.
    #[error("claim {claim} is already withdrawn; a withdrawn claim is superseded, never revived")]
    ClaimWithdrawn { claim: String },

    /// A dispute closed by the party it was raised against.
    #[error("{party} raised or published this claim and cannot also resolve the dispute")]
    SelfResolvedDispute { party: ActorId },

    /// 14.11's promotion rule.
    #[error("promotion on {basis} alone is not a permitted basis for a highlight")]
    PromotionBasisNotPermitted { basis: &'static str },

    /// An identifier used twice.
    #[error("claim {claim} is already registered")]
    ClaimAlreadyRegistered { claim: String },

    /// A reference to a claim nobody registered.
    #[error("claim {claim} is not registered")]
    ClaimUnknown { claim: String },
}

/// Refusals from the medical and neuroscience boundary governance of 14.14.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum BoundaryError {
    /// 14.14: "Neuroscience pack cards state data source, modality, population, intended research
    /// capability, limitations, and clinician/scientist review."
    #[error("the domain pack card does not state its {field}")]
    MissingDomainEvidence { field: &'static str },

    /// A card with no domain sign-off at all.
    #[error("the domain pack card carries no review by a clinician or domain scientist")]
    NoDomainReview,

    /// The only domain review is by the card's own author.
    #[error("{party} authored this pack and cannot be its only domain reviewer")]
    SelfDomainReview { party: ActorId },

    /// A pack that declares a use the boundary excludes outright.
    #[error("`{use_case}` is outside the research boundary and no evidence admits it")]
    ExcludedUseDeclared { use_case: &'static str },

    /// A pack card with no permitted research scope declares nothing it may do.
    #[error("the domain pack card names no permitted research scope")]
    NoPermittedScope,

    /// 14.14's clinical-transition gate, still open.
    #[error("clinical transition obligation `{obligation}` is not discharged")]
    ObligationOutstanding { obligation: &'static str },

    /// The refusal that has no override.
    #[error("{party} refused this crossing at {epoch}; a human refusal is terminal")]
    HumanRefusalIsFinal { party: ActorId, epoch: Epoch },

    /// Only a domain expert disposes of a crossing.
    #[error("a party acting as {role} cannot dispose of a boundary crossing")]
    NotQualifiedToDispose { role: Role },

    /// A crossing decided twice.
    #[error("crossing {docket} is already decided")]
    CrossingAlreadyDecided { docket: String },

    /// A crossing that nobody has decided, used as though it were decided.
    #[error("crossing {docket} is open; no output may be released while a crossing is open")]
    CrossingOpen { docket: String },

    /// An obligation discharged with no responsible party named.
    #[error("obligation `{obligation}` was discharged by nobody")]
    ObligationUnattributed { obligation: &'static str },
}

/// Refusals from data governance, federation and access, 14.15.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum AccessError {
    /// A contract that permits and prohibits the same purpose decides nothing.
    #[error("purpose `{purpose}` is both permitted and prohibited by this contract")]
    PurposeBothPermittedAndProhibited { purpose: String },

    /// A grant for a purpose the contract never permitted.
    #[error("purpose `{purpose}` is not among the contract's permitted purposes")]
    PurposeNotPermitted { purpose: String },

    /// A grant for a purpose the contract prohibits.
    #[error("purpose `{purpose}` is prohibited by this contract")]
    PurposeProhibited { purpose: String },

    /// Least privilege: a grant cannot exceed its contract.
    #[error("field `{field}` is not releasable under this contract")]
    FieldNotReleasable { field: String },

    /// 14.15: grants are time-bound. There is no indefinite grant.
    #[error("a grant must expire after the epoch it was made at; {granted} to {expires} does not")]
    GrantWithoutExpiry { granted: Epoch, expires: Epoch },

    /// An access recorded outside the grant that was supposed to authorise it.
    #[error("access at {epoch} falls outside grant {grant}, which runs to {expires}")]
    AccessOutsideGrant {
        grant: String,
        epoch: Epoch,
        expires: Epoch,
    },

    /// Derivative and training rights default to denied.
    #[error("this contract grants no {right} right")]
    RightNotGranted { right: &'static str },

    /// A withdrawn contract issues no further grants.
    #[error("contract {contract} was withdrawn at {epoch}")]
    ContractWithdrawn { contract: String, epoch: Epoch },

    /// A federated site attestation missing one of the five things 14.15 requires it to sign.
    #[error("the site attestation does not cover {missing}")]
    IncompleteAttestation { missing: &'static str },

    /// A model or provider the contract's restriction excludes.
    #[error("model `{model}` is not permitted to process data under this contract")]
    ModelNotPermitted { model: String },

    /// A reference to a grant nobody issued.
    #[error("grant {grant} is not held by this ledger")]
    GrantUnknown { grant: String },

    /// Retention past the contract's ceiling.
    #[error("contract {contract} retains until {until}; access at {epoch} is past retention")]
    PastRetention {
        contract: String,
        until: Epoch,
        epoch: Epoch,
    },
}

/// One return type for callers that span modules.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum StewardshipError {
    #[error(transparent)]
    Review(#[from] ReviewError),
    #[error(transparent)]
    Predeclaration(#[from] PredeclarationError),
    #[error(transparent)]
    Claim(#[from] ClaimError),
    #[error(transparent)]
    Boundary(#[from] BoundaryError),
    #[error(transparent)]
    Access(#[from] AccessError),
}
