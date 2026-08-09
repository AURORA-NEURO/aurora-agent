//! Structured refusals.
//!
//! Section 34's shared failure-states paragraph is the closest thing it has to an error model:
//! *"The surface must explicitly support unavailable, controlled, stale, under-review, disputed,
//! withdrawn, non-reproducible, and not-comparable states. It must not replace them with zero,
//! empty, or hidden values."* Every variant below exists so that one of those states has somewhere
//! to be other than a blank.
//!
//! The naming rule throughout: an error says what was refused and names the specific thing that
//! caused the refusal, never just the category. A caller that receives
//! [`CardError::UndeclaredUnsuitability`] learns which claim kind it failed to disclaim, not that
//! "validation failed".

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Refusals when building or publishing a world card (34.03).
#[derive(Debug, Clone, PartialEq, Eq, Error, Serialize, Deserialize)]
#[serde(tag = "error", rename_all = "snake_case")]
pub enum CardError {
    /// A world card with no ancestry has no rung, and a card with no rung cannot say what the
    /// world is unsuitable for. This is the refusal `bioprism-worldfactory` and `bioprism-scale`
    /// both arrived at, restated at the publication boundary.
    #[error("world card for {world} declares no provenance: a card without a construction rung cannot state what the world is unsuitable for")]
    NoProvenanceRung { world: String },

    /// The card claims a suitability its deepest rung cannot support, or fails to disclaim one.
    #[error("world card for {world} stands on {rung} but does not declare that it is unsuitable for {claim}")]
    UndeclaredUnsuitability {
        world: String,
        rung: String,
        claim: String,
    },

    /// An observed world has no injected structure and therefore no known latent truth. A card
    /// that offers latent state on an observed world is describing a different world.
    #[error("world card for {world} stands only on observed data but advertises latent state as {offered}")]
    LatentStateWithoutConstruction { world: String, offered: String },

    /// A scope with no bound dimension makes every question in range, which is the same as making
    /// the card say nothing.
    #[error("world card for {world} binds no scope dimension: an unscoped card cannot be checked against a request")]
    UnscopedCard { world: String },

    /// 34.03's `links` object exists to make results reachable from the world that produced them.
    /// A link to something the caller cannot name is worse than no link.
    #[error("world card for {world} links to {kind} '{target}', which does not resolve")]
    UnresolvableLink {
        world: String,
        kind: String,
        target: String,
    },

    /// A card whose health is not active may still be read; it may not be presented as current.
    #[error("world card for {world} is {health} and may not be offered for a new run")]
    NotOfferable { world: String, health: String },

    /// Staleness is judged against a caller-supplied epoch. Without one the answer is undetermined,
    /// never "fresh".
    #[error("world card for {world} has no reference epoch, so its staleness is undetermined rather than fresh")]
    StalenessUndetermined { world: String },
}

/// Refusals in the value-of-experiment lab (34.10).
#[derive(Debug, Clone, PartialEq, Eq, Error, Serialize, Deserialize)]
#[serde(tag = "error", rename_all = "snake_case")]
pub enum ValueError {
    /// The honest answer when the numerator has no estimator. `bioprism-lab` recorded the same gap
    /// at 09.02; this is that finding applied to a public ranking rather than to a private plan.
    #[error("experiment '{experiment}' has no declared value, so no ranking is possible; a missing value is not a value of zero")]
    ValueUndetermined { experiment: String },

    /// Ranking across incommensurable budget axes requires an exchange rate, and section 34 has
    /// none. The caller may supply one; they may not omit one and get a number anyway.
    #[error("cannot scalarise cost across {axes} axes without an exchange rate; use the Pareto front, or supply one and own it")]
    NoExchangeRate { axes: usize },

    /// An exchange rate that prices an axis at zero silently deletes it from the comparison.
    #[error("exchange rate prices the '{axis}' axis at zero, which removes it from the comparison rather than valuing it")]
    ZeroWeight { axis: String },

    /// Nothing to choose between.
    #[error("no candidate experiments were supplied")]
    NoCandidates,

    /// An experiment must be attached to a live question. Value assigned to an experiment that
    /// resolves nothing is value assigned to activity.
    #[error("experiment '{experiment}' addresses no hypothesis that is still open")]
    AddressesNothingOpen { experiment: String },

    /// A hypothesis an experiment claims to address must exist.
    #[error("experiment '{experiment}' addresses unknown hypothesis '{hypothesis}'")]
    UnknownHypothesis {
        experiment: String,
        hypothesis: String,
    },

    /// Declared values must be finite and non-negative; NaN sorts arbitrarily and would make the
    /// ranking non-deterministic.
    #[error("declared value for '{experiment}' is not a finite non-negative number")]
    NonFiniteValue { experiment: String },
}

/// Refusals in the data connector registry (34.12).
#[derive(Debug, Clone, PartialEq, Eq, Error, Serialize, Deserialize)]
#[serde(tag = "error", rename_all = "snake_case")]
pub enum ConnectorError {
    /// A conformance record with a zero denominator asserts nothing while looking like evidence.
    #[error("conformance record for connector '{connector}' has a zero denominator")]
    EmptyConformance { connector: String },

    /// The composition point with `bioprism-adapter`: a mapping that coarsens or reinterprets must
    /// report what it discarded. "Lossless coarsening" is the claim this refuses.
    #[error("connector '{connector}' declares a {kind} mapping with an empty loss ledger: a mapping that changes granularity always discards something")]
    UndeclaredSemanticLoss { connector: String, kind: String },

    /// The composition point with `bioprism-sdk`: no conformance evidence, no load-bearing use.
    #[error("connector '{connector}' has no conformance evidence and cannot be selected for load-bearing work")]
    NoConformanceEvidence { connector: String },

    /// Evidence gathered inside a narrower scope does not license a wider request. This is
    /// `bioprism-scope`'s refinement order used as an admission check.
    #[error("connector '{connector}' has conformance evidence only within a narrower scope than the request")]
    ConformanceOutOfScope { connector: String },

    /// Egress is a boundary, not a cost. There is no discount that makes crossing it acceptable.
    #[error("connector '{connector}' permits {permitted} egress; the request needs {requested}")]
    EgressRefused {
        connector: String,
        permitted: String,
        requested: String,
    },

    /// A quarantined connector is not a slow connector.
    #[error("connector '{connector}' is {health} and is not selectable")]
    NotSelectable { connector: String, health: String },

    /// A connector that declares no modality cannot be matched against any request.
    #[error("connector '{connector}' declares no modality it can fetch")]
    FetchesNothing { connector: String },

    /// The request asked for something the connector never claimed.
    #[error("connector '{connector}' does not declare modality '{modality}'")]
    ModalityNotDeclared {
        connector: String,
        modality: String,
    },
}

/// Refusals in federated and bring-your-own-data evaluation (34.17).
///
/// Named `FederatedError` rather than `FederationError` because
/// [`bioprism_hubapi::federation::FederationError`] already exists and is about a different thing:
/// trust crossing a registry boundary. This one is about evidence not crossing a site boundary.
#[derive(Debug, Clone, PartialEq, Eq, Error, Serialize, Deserialize)]
#[serde(tag = "error", rename_all = "snake_case")]
pub enum FederatedError {
    /// The central invariant. When the data does not move, something is always unverifiable; a
    /// federated result asserting otherwise is asserting that it was centralised.
    #[error("a federated result must name at least one thing it could not check")]
    ClaimsNothingUnchecked,

    /// `bioprism-hubapi`'s rule that trust does not transit, applied to sites: a standing earned at
    /// one registry does not authorise a run at another.
    #[error("trust standing holds in registry '{holds_in}' and does not authorise site '{site}'")]
    StandingDoesNotHoldAtSite { holds_in: String, site: String },

    /// Two sites that ran different pack content produced results about different questions.
    #[error("site '{site}' ran pack digest {theirs}, not {ours}; the results are not comparable")]
    NotComparable {
        site: String,
        ours: String,
        theirs: String,
    },

    /// A suppressed cell has no value. Pooling over it would require inventing one.
    #[error("site '{site}' suppressed its cell under the small-cell policy; it cannot contribute a point estimate")]
    SuppressedCellCannotPool { site: String },

    /// Meta-analysis over nothing.
    #[error("no site results were supplied")]
    NoSites,

    /// A policy whose threshold is zero suppresses nothing and is therefore not a policy.
    #[error("small-cell threshold of zero protects nothing")]
    VacuousSmallCellPolicy,

    /// Two sites reporting under the same identifier makes the union of unchecked aspects
    /// ambiguous and lets one site's participation be counted twice.
    #[error("site '{site}' appears more than once in the same pooled result")]
    DuplicateSite { site: String },
}

/// Refusals in research CI (34.20).
#[derive(Debug, Clone, PartialEq, Eq, Error, Serialize, Deserialize)]
#[serde(tag = "error", rename_all = "snake_case")]
pub enum CiError {
    /// A suite with no checks passes everything.
    #[error("a CI suite with no checks would pass every result")]
    EmptySuite,

    /// The same check twice with different outcomes has no defined report.
    #[error("check '{check}' was recorded twice")]
    DuplicateCheck { check: String },

    /// A report is publishable only when every check passed. Asking for the publication decision
    /// before every check has an outcome is the mistake this refuses.
    #[error("check '{check}' has no recorded outcome, so publishability is not yet decidable")]
    IncompleteReport { check: String },
}
