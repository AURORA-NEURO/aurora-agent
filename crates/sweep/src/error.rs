//! Every way the remainders refuse.
//!
//! One error type for the whole crate. The variants are grouped by the shape of the refusal rather
//! than by module, because the same refusal recurs across sections: an undeclared gap
//! ([`SweepError::Undeclared`]), a claim without the evidence that would earn it
//! ([`SweepError::Unproven`]), and an approval that no longer describes what is about to happen
//! ([`SweepError::ApprovalStale`]) are the three that appear in §03, §04, §05, §08, §10 and §13
//! alike.

use thiserror::Error;

/// A refusal from one of the remainder modules.
#[derive(Debug, Error, PartialEq, Eq, Clone)]
pub enum SweepError {
    /// A justification was required and was empty.
    ///
    /// Blueprint 03.03 asks each component to "declare restoration confidence" and 04.04 to
    /// "declare capture omissions". A declaration whose reason is the empty string discharges the
    /// letter of that and none of its purpose, so it is refused at construction.
    #[error("{what} requires a non-empty {field}")]
    EmptyJustification { what: &'static str, field: &'static str },

    /// Something that had to be accounted for was neither included nor declared missing.
    ///
    /// This is the crate's central refusal. The undeclared items are named because a count is not
    /// actionable.
    #[error("{what}: {items:?} were neither included nor declared omitted")]
    Undeclared { what: &'static str, items: Vec<String> },

    /// A required field of a record that the blueprint enumerates exhaustively was absent.
    #[error("{what} is missing required field(s) {fields:?}")]
    MissingField { what: &'static str, fields: Vec<String> },

    /// A declaration was moved upward.
    ///
    /// Fidelity is monotone downward by construction (see [`crate::fidelity`]): a fallback lowers
    /// it (05.11), an unpinned dependency lowers it (04.04), and nothing raises it. An attempt to
    /// raise is a bug in the caller, not a value to clamp.
    #[error("cannot raise {what} from {from} to {to}")]
    FidelityRaised { what: &'static str, from: &'static str, to: &'static str },

    /// A capability, equivalence or support level was asserted without the evidence that earns it.
    #[error("{subject} claims {claim} but its evidence is {state}")]
    Unproven { subject: String, claim: String, state: &'static str },

    /// The plan changed after it was approved.
    ///
    /// Blueprint 13.11: "Revalidate resource and policy immediately before effect. Changes to
    /// target or arguments invalidate approval."
    #[error("approval is stale: approved plan {approved} but current plan is {current}")]
    ApprovalStale { approved: String, current: String },

    /// An action was attempted on a source that failed validation and was quarantined.
    ///
    /// Blueprint 04.01: the platform never "best-effort executes" an untrusted pack during import.
    #[error("source {source_id} is quarantined: {diagnostic}")]
    Quarantined { source_id: String, diagnostic: String },

    /// A published result was asked to change its meaning after the fact.
    ///
    /// Blueprint 03.11 invariant: "Changes to schemas and scoring semantics are versioned and
    /// cannot retroactively rewrite already published results."
    #[error("result {result} is published under {published}; cannot rewrite it as {proposed}")]
    RetroactiveRewrite { result: String, published: String, proposed: String },

    /// An operation was permitted only under a declaration that was not supplied.
    ///
    /// Covers 05.10's provider substitution without a comparability declaration, 05.11's merge
    /// outside a task that tests merging, and 08.06's plan with no declared sentinel floor.
    #[error("{operation} requires {declaration}, which was not declared")]
    UndeclaredPrecondition { operation: &'static str, declaration: &'static str },

    /// A structural rule of the world-state IR was violated.
    #[error("{what}: {detail}")]
    Malformed { what: &'static str, detail: String },

    /// Canonical serialisation failed, so no digest could be taken.
    #[error("canonicalisation failed: {0}")]
    Canonical(String),
}

impl SweepError {
    pub(crate) fn empty(what: &'static str, field: &'static str) -> Self {
        SweepError::EmptyJustification { what, field }
    }

    pub(crate) fn malformed(what: &'static str, detail: impl Into<String>) -> Self {
        SweepError::Malformed { what, detail: detail.into() }
    }
}

impl From<bioprism_ids::CanonicalError> for SweepError {
    fn from(value: bioprism_ids::CanonicalError) -> Self {
        SweepError::Canonical(value.to_string())
    }
}

pub(crate) fn require_nonempty(
    value: &str,
    what: &'static str,
    field: &'static str,
) -> Result<(), SweepError> {
    if value.trim().is_empty() {
        Err(SweepError::empty(what, field))
    } else {
        Ok(())
    }
}
