//! Typed refusals.
//!
//! Every variant here is a thing a browsing surface would otherwise have done quietly: divide by a
//! denominator it does not have, add two readings that were taken under different conditions, or
//! accept a stored count that its own contents contradict. Each is an error rather than a warning
//! because the alternative rendering — a zero, a blank, a plausible percentage — is indistinguishable
//! from a real one once it reaches a page.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// The error type of this crate.
#[derive(Debug, Clone, PartialEq, Eq, Error, Serialize, Deserialize)]
#[serde(tag = "error", rename_all = "snake_case")]
pub enum AtlasxError {
    /// A fraction over nothing.
    ///
    /// `0 / 0` is not zero percent and it is not a hundred percent; it is the absence of a
    /// measurement, which is [`crate::SurfaceCell::Hole`]'s job rather than a number's.
    #[error("a share of {numerator} needs a denominator, and zero is not one")]
    EmptyDenominator { numerator: usize },

    /// More in the numerator than was ever counted.
    #[error("a share of {numerator} over {denominator} exceeds one; a surface may not report more than it aggregated")]
    ShareAboveOne {
        numerator: usize,
        denominator: usize,
    },

    /// A stored debt naming more holes than the grid it claims to describe has capabilities.
    #[error("debt for `{subject}` claims {claimed} capabilities but names {named} unmeasured")]
    DebtExceedsGrid {
        subject: String,
        claimed: usize,
        named: usize,
    },

    /// A stored debt whose summary count disagrees with the holes it carries.
    ///
    /// The summary is redundant on purpose: it is recomputed on the way in, so a hand-edited
    /// statement fails to load instead of loading with a friendlier number.
    #[error("debt for `{subject}` asserts {asserted} declaration-closed holes; its own holes yield {derived}")]
    DebtNotDerivable {
        subject: String,
        asserted: usize,
        derived: usize,
    },

    /// The same capability named twice in one hole list, which would inflate the debt.
    #[error("debt for `{subject}` names capability `{capability}` twice")]
    DuplicateHole { subject: String, capability: String },

    /// Two debts about different readings.
    ///
    /// A grid restricted to a pack and the same grid restricted to a site are different subjects
    /// in `bioprism-metrics`, and their debts do not add for the same reason their scores do not
    /// compare.
    #[error("debt for `{left}` and debt for `{right}` are statements about different readings and do not add")]
    DebtSubjectsDiffer { left: String, right: String },

    /// Failure records spanning taxonomy versions.
    ///
    /// The failure taxonomy is a closed set, and its usefulness depends on being closed
    /// *comparably* across releases. Counting a mechanism under two versions of the set produces a
    /// bar chart whose bars mean different things.
    #[error("failure records span taxonomy versions `{left}` and `{right}`; a browse across them is not a browse")]
    MixedTaxonomyVersions { left: String, right: String },

    /// The same failure appearing twice in a browsed set, which would double-count it.
    #[error("failure `{failure_id}` appears twice in the browsed set")]
    DuplicateRecord { failure_id: String },

    /// Two visibility states for one record.
    #[error("failure `{failure_id}` was given a visibility state twice")]
    DuplicateVisibility { failure_id: String },

    /// A visibility state for a record that is not being browsed.
    ///
    /// Refused rather than ignored: a caller who withholds a record that is not in the set has a
    /// stale identifier, and silently accepting it hides the staleness.
    #[error("visibility declared for `{failure_id}`, which is not in the browsed set")]
    VisibilityForAbsentRecord { failure_id: String },
}
