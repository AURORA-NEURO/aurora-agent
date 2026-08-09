//! Typed refusals for the public hub.
//!
//! Every variant here is a refusal the hub owes an explanation for, so each one names the thing
//! that was wrong rather than the layer that noticed. Blueprint 34.15 requires the public surface
//! to "explicitly support unavailable, controlled, stale, under-review, disputed, withdrawn,
//! non-reproducible, and not-comparable states" and forbids replacing them "with zero, empty, or
//! hidden values". An untyped `bool` return would be exactly that substitution, so refusals are
//! carried as data.
//!
//! The enum derives `PartialEq`/`Eq` deliberately: a test asserting *which* refusal fired is the
//! only way to check that a refusal is the intended one and not an incidental one.

use crate::leaderboard::ConditionDifference;
use crate::moderation::ModerationState;
use bioprism_ids::{CanonicalError, IdError};
use thiserror::Error;

/// Everything the hub can refuse, and why.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum HubError {
    /// The submission contract of 34.16 is a floor, not a suggestion. The field is named so the
    /// submitter can fix it without guessing.
    #[error("submission is missing required field `{field}`: {why}")]
    MissingRequiredField {
        field: &'static str,
        why: &'static str,
    },

    /// 43.43 and 36.12: a research submission must state what it does not establish. Clinical
    /// validity is the non-claim the hub will not accept a submission without.
    #[error("submission does not disclaim `{kind}`; a research result establishes no such thing and must say so")]
    MissingRequiredNonClaim { kind: &'static str },

    #[error("submitter `{submitter}` is suspended and may not submit: {reason}")]
    SubmitterSuspended { submitter: String, reason: String },

    /// 36.16 (expert conflicts, sponsorship and independence).
    #[error("submitter `{submitter}` has not declared conflicts of interest")]
    ConflictsUndeclared { submitter: String },

    #[error("draft names submitter `{declared}` but was presented by `{presenting}`")]
    SubmitterMismatch {
        declared: String,
        presenting: String,
    },

    /// Verification is awarded by a reviewer, never claimed by the author.
    #[error("submitter claimed verification status `{claimed}`; only `self-reported` may be self-asserted")]
    SelfAssertedVerification { claimed: &'static str },

    #[error("actor `{actor}` may not attest their own submission `{submission}`")]
    SelfReview { actor: String, submission: String },

    #[error("unknown submission `{0}`")]
    UnknownSubmission(String),

    #[error("submission `{0}` is already on the ledger; a resubmission needs a new identifier")]
    DuplicateSubmission(String),

    #[error("illegal moderation transition {from} -> {to} for submission `{submission}`")]
    IllegalTransition {
        submission: String,
        from: ModerationState,
        to: ModerationState,
    },

    /// No silent rejection. 34.16 lists "appeal fairness" as a product metric, and an appeal
    /// against an unstated reason is not possible.
    #[error("transition to {to} requires a recorded reason")]
    MissingReason { to: ModerationState },

    #[error("transition to superseded requires the identifier of the superseding submission")]
    MissingSupersedingSubmission,

    #[error("a submission cannot supersede itself: `{0}`")]
    SelfSupersession(String),

    /// An audit log that accepts backdated entries is not an audit log.
    #[error("event at epoch {proposed} precedes the last recorded event at epoch {last}")]
    NonMonotonicEpoch { proposed: u64, last: u64 },

    /// 43.43: no claim of universal superiority. Two results produced under different conditions
    /// are not a ranking, and the hub will not manufacture one.
    #[error("entries `{left}` and `{right}` were evaluated under different conditions and cannot be ordered ({} differing dimension(s))", .differences.len())]
    NotComparable {
        left: String,
        right: String,
        differences: Vec<ConditionDifference>,
    },

    /// A lint, not a proof. See [`crate::leaderboard::lint_claim`].
    #[error("claim text contains forbidden phrasing `{phrase}`: {why}")]
    ForbiddenClaimPhrase {
        phrase: &'static str,
        why: &'static str,
    },

    /// Executive-summary rule: instance count is not benchmark count.
    #[error("evidence scale headlines {instances} instance(s) with {classes} independent equivalence class(es); report the classes")]
    InstanceCountWithoutClasses { instances: usize, classes: usize },

    /// A pack whose disclosure state was never recorded is not thereby held out.
    #[error("pack `{pack}` has no recorded disclosure state; absence of a leak report is not evidence of a held-out pack")]
    DisclosureUnrecorded { pack: String },

    #[error("pack `{pack}` is contaminated ({kind}: {detail}); no headline score may be published on it")]
    ContaminatedPack {
        pack: String,
        kind: &'static str,
        detail: String,
    },

    #[error("entry scores pack `{pack}`, disclosed at epoch {disclosed_at}, and does not acknowledge the disclosure")]
    UnacknowledgedDisclosure { pack: String, disclosed_at: u64 },

    /// Disclosure is a ratchet: a leaked pack does not become secret again.
    #[error("disclosure state of pack `{pack}` cannot move from {from} back to {to}")]
    DisclosureRegression {
        pack: String,
        from: &'static str,
        to: &'static str,
    },

    /// 36.14: dropping an ancestor's attribution is an error, not a warning.
    #[error("ancestor `{ancestor}` requires attribution to `{holder}` and the submission does not carry it")]
    MissingAttribution { ancestor: String, holder: String },

    #[error("ancestor `{ancestor}` requires attribution but names no rights holder")]
    AttributionUnspecified { ancestor: String },

    #[error("ancestor `{ancestor}` is licensed no-derivatives; a derived submission may not be published")]
    DerivativeForbidden { ancestor: String },

    /// Declaring a looser licence than an ancestor permits is how licence obligations get lost.
    #[error("declared licence `{declared}` ({declared_terms}) is less restrictive than ancestor `{ancestor}` ({ancestor_terms}); declare at least `{required_terms}`")]
    LicenceEscalation {
        declared: String,
        declared_terms: &'static str,
        ancestor: String,
        ancestor_terms: &'static str,
        required_terms: &'static str,
    },

    #[error("declared access tier `{declared}` is more open than ancestor `{ancestor}` tier `{required}`")]
    AccessTierEscalation {
        declared: &'static str,
        ancestor: String,
        required: &'static str,
    },

    /// 34.15 again: a card that is not publishable shows a withheld marker, never a number.
    #[error("card in state {state} may not carry a score; publication states are not zeroes")]
    ScoreOnNonPublishableCard { state: &'static str },

    #[error("identifier: {0}")]
    Id(#[from] IdError),

    #[error("canonical encoding: {0}")]
    Canonical(#[from] CanonicalError),
}
