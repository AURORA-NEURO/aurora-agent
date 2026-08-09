//! Typed failures, and the typed *absence* of an answer.
//!
//! Two kinds of thing go wrong here and they are deliberately not the same type.
//!
//! [`InfluenceError`] is a caller bug or a malformed input: a ratio range whose lower endpoint
//! exceeds its upper one, a factor id that is not in the region, a region whose answer has zero
//! total mass and therefore has no normalised form. These are conditions under which no method
//! could have been asked a meaningful question.
//!
//! [`UnknownReason`] is the opposite: a perfectly well-formed question that no implemented method
//! is entitled to answer. Blueprint 43.28 lists "no validated influence method yields
//! `ABSTAIN_UNKNOWN_OMISSION`" among its failure modes, and `AGENTS.md` makes the same point in
//! stronger terms — "provably cannot matter" and "nobody checked" must never share a
//! representation. An `UnknownReason` is therefore a *successful* outcome of analysis that
//! reports non-analysis, and it carries the precondition that failed so a caller can tell what
//! would have to change.

use thiserror::Error;

/// A malformed request. Never used to represent "no bound is available".
#[derive(Debug, Clone, PartialEq, Error)]
pub enum InfluenceError {
    #[error("ratio range [{lo}, {hi}] is inverted; the lower endpoint must not exceed the upper")]
    InvertedRatioRange { lo: f64, hi: f64 },

    #[error("ratio range endpoint {value} is not a finite non-negative real")]
    InadmissibleRatioEndpoint { value: f64 },

    #[error("factor {factor:?} is not present in region {region:?}")]
    UnknownFactor { region: String, factor: String },

    #[error("factor {factor:?} carries no table, so no perturbation of its entries is defined")]
    UntabledFactor { factor: String },

    #[error(
        "the region's answer has total mass {mass}; a distribution cannot be normalised out of it"
    )]
    DegenerateAnswer { mass: f64 },

    #[error("answers over scopes {left:?} and {right:?} are not comparable")]
    IncomparableScopes { left: Vec<String>, right: Vec<String> },

    #[error("rebuilding the perturbed region failed: {message}")]
    PerturbedRegionRejected { message: String },

    #[error("a bound of {value} is not a total-variation distance; bounds must lie in [0, 1]")]
    BoundOutOfRange { value: f64 },

    #[error("brute force over {entries} perturbation vertices exceeds the cap of {cap}; a soundness check that silently samples instead of enumerating would not be a soundness check")]
    BruteForceTooLarge { entries: u64, cap: u64 },

    #[error("the brute-force executor declined a fixture it must be able to run: {detail}")]
    BruteForceDeclined { detail: String },

    #[error("the shipped reference world could not be read: {message}")]
    ReferenceWorldUnreadable { message: String },

    /// A misuse of the abstract-domain machinery of 43.11.
    ///
    /// Transparent rather than wrapped in a sentence of its own: a caller who passed one domain's
    /// abstraction to another's transformer needs [`crate::DomainError`]'s words, not a paraphrase
    /// of them.
    #[error(transparent)]
    Domain(#[from] crate::domain::DomainError),
}

/// Why no implemented method produced a bound.
///
/// Every variant names a precondition, not an effort level. A reader should be able to answer
/// "what would have to be true for this to become `Bounded`?" from the variant alone.
#[derive(Debug, Clone, PartialEq, Eq, Error, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case", tag = "reason")]
pub enum UnknownReason {
    /// The `fiber-world/0.1` wire schema declares a factor's signature and never its potential.
    /// This is the ordinary state of a region sliced straight out of a world, and it is why the
    /// reference-world measurement in [`crate::reference`] comes out the way it does.
    #[error("factor {factor:?} carries no potential; every implemented method needs a valuation, and inventing one would make the bound a statement about the invention")]
    NoFactorTable { factor: String },

    /// A method exists for this perturbation class but not for this region's structure.
    ///
    /// `method` and `handles` are owned strings rather than `&'static str` so the whole enum can
    /// round-trip through serde: a reason that cannot be deserialised cannot be replayed from a
    /// stored certificate, and replayability is the point of writing it down.
    #[error("{method} handles {handles}, and this region is not of that class: {detail}")]
    RegionOutsideMethodClass {
        method: String,
        handles: String,
        detail: String,
    },

    /// The exact method needs to run the query twice and the backend refused.
    #[error("the exact method requires executing the region and the backend declined: {detail}")]
    BackendDeclined { detail: String },

    /// The caller asked about a perturbation no method models.
    #[error("no implemented method covers perturbation class {class}")]
    PerturbationClassUnsupported { class: String },

    /// Nothing was asked. Distinct from every variant above, which record that something was.
    #[error("no influence method was run against this group")]
    NotAnalysed,
}
