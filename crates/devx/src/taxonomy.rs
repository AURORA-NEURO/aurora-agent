//! The retryability taxonomy: nine classes, three retry decisions, one required change kind.
//!
//! Blueprint 40.36 (error taxonomy, exit codes and retry semantics) is the normative source, and
//! 11.01's failure clause — *emit an actionable diagnostic rather than silently repairing or
//! discarding state* — is the reason a developer-platform crate owns a copy of it at all. An
//! actionable diagnostic has to answer two different questions, and a boolean answers neither
//! well:
//!
//! 1. **May I re-send this?** [`Retryability`], three-valued because "never" and "not until you
//!    change something" are different instructions to a client. A client that collapses them
//!    re-sends a request that will be refused identically forever.
//! 2. **What would I have to change?** [`ChangeRequired`]. This is the part 40.36 does not
//!    specify and the part that makes a retry decision usable: "retry after caller change" with no
//!    statement of *which* surface must change is a shrug with a type.
//!
//! # This is a second transcription of 40.36, and that is stated rather than hidden
//!
//! `bioprism-services` also transcribes 40.36 into nine classes, and reaches the same three-way
//! retry mapping. This crate cannot depend on it — `bioprism-services` is not in this crate's
//! dependency set — so the agreement is not enforced by the compiler here. What is enforced is
//! that the mapping is *total* ([`DiagnosticClass::ALL`] plus an exhaustive `match`), so a tenth
//! class cannot be added without deciding what a caller should do with it.
//!
//! The classes are 40.36's, not either crate's invention. Where this crate adds something it is
//! [`ChangeRequired`] and the [`DiagnosticClass::remedy_is_mandatory`] rule, not a different
//! retry decision; inventing a divergent decision to look original would have been a worse
//! outcome than an admitted duplicate.
//!
//! # Not implemented
//!
//! No HTTP status mapping, no `problem+json`, no backoff schedule and no clock. A retry *decision*
//! is data; scheduling a retry is an effect, and this crate has none.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Whether a caller may re-send the identical request.
///
/// The three cases are the ones 40.36's "retry classification" input needs to stay honest. The
/// names are stated from the caller's point of view — the caller is the one holding the request —
/// rather than from the service's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Retryability {
    /// Re-sending is pointless and so is waiting. The request is dead as written; the caller must
    /// change the request itself.
    Terminal,
    /// Re-sending the identical request is pointless, but a *different* request may succeed:
    /// raise the budget, widen the boundary, obtain the grant.
    RetryableAfterChange,
    /// The identical logical request may succeed later. Re-sending is safe because the contract
    /// that refused it is idempotent under the request.
    RetryableAsIs,
}

impl Retryability {
    pub const ALL: [Retryability; 3] = [
        Retryability::Terminal,
        Retryability::RetryableAfterChange,
        Retryability::RetryableAsIs,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Retryability::Terminal => "terminal",
            Retryability::RetryableAfterChange => "retryable_after_change",
            Retryability::RetryableAsIs => "retryable_as_is",
        }
    }

    /// Whether a client loop may re-send without a human or an agent editing the request.
    pub fn permits_automatic_retry(self) -> bool {
        self == Retryability::RetryableAsIs
    }
}

impl fmt::Display for Retryability {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Which surface a caller must touch before a retry can differ from the refusal.
///
/// [`ChangeRequired::Unknown`] exists so that an unclassified diagnostic is *loud* rather than
/// silently indistinguishable from [`ChangeRequired::Nothing`]. This is the same argument
/// `bioprism-section` makes for `InfluenceClass::Unknown`: the dangerous default is the one that
/// looks like a benign answer. [`crate::lint`](mod@crate::lint) rejects any catalogue entry that
/// leaves it `Unknown`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeRequired {
    /// Nothing. Re-send the identical bytes.
    Nothing,
    /// Re-read the snapshot, digest or version the request asserted, then re-send. The logical
    /// request is unchanged; only the precondition token is refreshed.
    Precondition,
    /// The invocation: a subcommand, a flag, an argument.
    Invocation,
    /// The payload: the world, query or document the request carries.
    Payload,
    /// The declared contract: a budget, a protected closure, a boundary, a schema range.
    Contract,
    /// Something outside the caller entirely: a grant, a policy decision, a dependency that is
    /// down, a maintainer fixing a bug.
    Environment,
    /// Not classified. Never emitted by [`DiagnosticClass::change_required`]; reserved for
    /// diagnostics arriving from outside this crate, so that "nobody decided" is visible.
    Unknown,
}

impl ChangeRequired {
    pub const ALL: [ChangeRequired; 7] = [
        ChangeRequired::Nothing,
        ChangeRequired::Precondition,
        ChangeRequired::Invocation,
        ChangeRequired::Payload,
        ChangeRequired::Contract,
        ChangeRequired::Environment,
        ChangeRequired::Unknown,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            ChangeRequired::Nothing => "nothing",
            ChangeRequired::Precondition => "precondition",
            ChangeRequired::Invocation => "invocation",
            ChangeRequired::Payload => "payload",
            ChangeRequired::Contract => "contract",
            ChangeRequired::Environment => "environment",
            ChangeRequired::Unknown => "unknown",
        }
    }

    /// Whether the request itself has to be rewritten.
    ///
    /// This is the axis that ties [`ChangeRequired`] to [`Retryability`], in two one-directional
    /// implications rather than an equivalence:
    ///
    /// - [`Retryability::Terminal`] ⟹ the request must be rewritten. "Terminal" means the request
    ///   as written is dead, so if nothing in the request had to change the class would not be
    ///   terminal.
    /// - [`Retryability::RetryableAsIs`] ⟹ the request must *not* be rewritten. If the bytes have
    ///   to change, re-sending them unchanged cannot help.
    ///
    /// [`Retryability::RetryableAfterChange`] is constrained by neither, and that is the point:
    /// `ContractViolation` needs the request's declared contract widened while `PolicyDenied`
    /// needs a grant nobody in the request can issue. Both must be retried only after something
    /// changes, and *what* changes is exactly what this axis adds.
    ///
    /// [`ChangeRequired::Unknown`] returns `false` so that an unclassified diagnostic is not
    /// silently treated as one demanding a rewrite; it is caught by name in
    /// [`crate::lint`](mod@crate::lint) instead.
    pub fn touches_the_request(self) -> bool {
        matches!(
            self,
            ChangeRequired::Invocation | ChangeRequired::Payload | ChangeRequired::Contract
        )
    }
}

impl fmt::Display for ChangeRequired {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The nine classes every diagnostic maps to, exactly one each.
///
/// Declaration order carries no severity claim. Two classes with different retry decisions are not
/// comparable on one axis and a `max()` over them would quietly pick the wrong one; the order
/// exists so iteration and serialisation are stable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticClass {
    /// The invocation was malformed: an unknown subcommand, a missing required flag.
    Usage,
    /// The payload parsed but did not satisfy its schema or descriptor.
    InvalidInput,
    /// A precondition on a version, digest or snapshot no longer holds. The caller re-reads.
    Stale,
    /// The request contradicts committed state: an identity rebound to different content.
    Conflict,
    /// Policy refused. 40.36 invariant 3: not a system failure, and not something to page on.
    PolicyDenied,
    /// The tool cannot produce a result satisfying its own declared invariants: a budget below the
    /// mandatory closure, a minimisation that would change the defect under test.
    ContractViolation,
    /// The science is unknown, not the execution. 40.36 invariant 4: an unstable oracle, an
    /// abstention, a posterior that does not fit.
    Indeterminate,
    /// A declared dependency could not be reached or read.
    Unavailable,
    /// Unclassified failure of the tool itself. Present so the unknown-failure rate is measurable
    /// rather than hidden inside a neighbouring class.
    Internal,
}

impl DiagnosticClass {
    /// The whole taxonomy. It is closed.
    pub const ALL: [DiagnosticClass; 9] = [
        DiagnosticClass::Usage,
        DiagnosticClass::InvalidInput,
        DiagnosticClass::Stale,
        DiagnosticClass::Conflict,
        DiagnosticClass::PolicyDenied,
        DiagnosticClass::ContractViolation,
        DiagnosticClass::Indeterminate,
        DiagnosticClass::Unavailable,
        DiagnosticClass::Internal,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            DiagnosticClass::Usage => "usage",
            DiagnosticClass::InvalidInput => "invalid_input",
            DiagnosticClass::Stale => "stale",
            DiagnosticClass::Conflict => "conflict",
            DiagnosticClass::PolicyDenied => "policy_denied",
            DiagnosticClass::ContractViolation => "contract_violation",
            DiagnosticClass::Indeterminate => "indeterminate",
            DiagnosticClass::Unavailable => "unavailable",
            DiagnosticClass::Internal => "internal",
        }
    }

    /// The retry decision. Total over the enum by construction.
    pub fn retryability(self) -> Retryability {
        match self {
            DiagnosticClass::Usage | DiagnosticClass::InvalidInput | DiagnosticClass::Conflict => {
                Retryability::Terminal
            }
            DiagnosticClass::PolicyDenied
            | DiagnosticClass::ContractViolation
            | DiagnosticClass::Indeterminate => Retryability::RetryableAfterChange,
            DiagnosticClass::Stale | DiagnosticClass::Unavailable | DiagnosticClass::Internal => {
                Retryability::RetryableAsIs
            }
        }
    }

    /// The surface a caller must change. Total over the enum, and never
    /// [`ChangeRequired::Unknown`].
    ///
    /// [`Internal`](DiagnosticClass::Internal) maps to
    /// [`Environment`](ChangeRequired::Environment) rather than `Nothing`: an internal fault is
    /// safe to re-send once, but the thing that would actually fix it is a maintainer, and saying
    /// "change nothing" would advertise a retry loop as a repair.
    pub fn change_required(self) -> ChangeRequired {
        match self {
            DiagnosticClass::Usage => ChangeRequired::Invocation,
            DiagnosticClass::InvalidInput => ChangeRequired::Payload,
            DiagnosticClass::Stale => ChangeRequired::Precondition,
            DiagnosticClass::Conflict => ChangeRequired::Payload,
            DiagnosticClass::PolicyDenied => ChangeRequired::Environment,
            DiagnosticClass::ContractViolation => ChangeRequired::Contract,
            DiagnosticClass::Indeterminate => ChangeRequired::Contract,
            DiagnosticClass::Unavailable => ChangeRequired::Environment,
            DiagnosticClass::Internal => ChangeRequired::Environment,
        }
    }

    /// Whether an operator should read this as the platform being broken.
    ///
    /// False for [`PolicyDenied`](DiagnosticClass::PolicyDenied) and
    /// [`Indeterminate`](DiagnosticClass::Indeterminate) by 40.36 invariants 3 and 4, and false
    /// for every class whose cause is the caller's own request. A platform that pages on an oracle
    /// returning "unknown" has taught its operators to ignore the pager.
    pub fn is_system_failure(self) -> bool {
        matches!(
            self,
            DiagnosticClass::Unavailable | DiagnosticClass::Internal | DiagnosticClass::Stale
        )
    }

    /// Whether a diagnostic in this class must carry at least one remedy.
    ///
    /// True for every class, which is the whole thesis of this crate: a diagnostic that names a
    /// violated invariant and stops has moved the diagnosis back into a debugger. Even
    /// [`Internal`](DiagnosticClass::Internal) — where the remedy is "report it with this record
    /// attached" — has a next action, and a class-shaped exemption is how "no remedy" becomes
    /// normal.
    ///
    /// It is a method rather than a constant so that a future class arguing for an exemption has
    /// to write the argument down here, in a place a reviewer reads.
    pub fn remedy_is_mandatory(self) -> bool {
        match self {
            DiagnosticClass::Usage
            | DiagnosticClass::InvalidInput
            | DiagnosticClass::Stale
            | DiagnosticClass::Conflict
            | DiagnosticClass::PolicyDenied
            | DiagnosticClass::ContractViolation
            | DiagnosticClass::Indeterminate
            | DiagnosticClass::Unavailable
            | DiagnosticClass::Internal => true,
        }
    }

    /// One sentence a developer can read to check the classification was right.
    pub fn gloss(self) -> &'static str {
        match self {
            DiagnosticClass::Usage => "the command as invoked does not exist in the shape it was invoked",
            DiagnosticClass::InvalidInput => "the payload was read and did not satisfy its schema",
            DiagnosticClass::Stale => "a precondition the request asserted has been superseded since it was read",
            DiagnosticClass::Conflict => "the request contradicts state already committed under the same identity",
            DiagnosticClass::PolicyDenied => "policy refused the request; the platform behaved correctly",
            DiagnosticClass::ContractViolation => "no result exists that satisfies the tool's own declared invariants under this request",
            DiagnosticClass::Indeterminate => "the tool ran correctly and the answer is not determined by the evidence",
            DiagnosticClass::Unavailable => "a declared dependency could not be reached or read",
            DiagnosticClass::Internal => "the tool failed in a way it has no classification for",
        }
    }
}

impl fmt::Display for DiagnosticClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// One row of the taxonomy, flattened for reporting.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaxonomyRow {
    pub class: DiagnosticClass,
    pub retryability: Retryability,
    pub change_required: ChangeRequired,
    pub is_system_failure: bool,
    pub gloss: String,
}

/// The taxonomy as data, in declaration order.
///
/// Emitting the table rather than only exposing the methods is what lets a consumer in another
/// language check its own copy against this one without linking to Rust.
pub fn taxonomy_table() -> Vec<TaxonomyRow> {
    DiagnosticClass::ALL
        .iter()
        .map(|class| TaxonomyRow {
            class: *class,
            retryability: class.retryability(),
            change_required: class.change_required(),
            is_system_failure: class.is_system_failure(),
            gloss: class.gloss().to_string(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn every_class_has_exactly_one_retry_decision_and_the_mapping_is_total() {
        let mut seen = BTreeSet::new();
        for class in DiagnosticClass::ALL {
            assert!(seen.insert(class), "{class} appears twice in ALL");
            let decision = class.retryability();
            assert!(
                Retryability::ALL.contains(&decision),
                "{class} produced a decision outside the three-valued axis"
            );
        }
        assert_eq!(seen.len(), 9);
    }

    #[test]
    fn change_required_is_never_unknown_for_a_classified_diagnostic() {
        for class in DiagnosticClass::ALL {
            assert_ne!(
                class.change_required(),
                ChangeRequired::Unknown,
                "{class} left the required change unclassified"
            );
        }
    }

    #[test]
    fn a_terminal_class_always_demands_the_request_itself_be_rewritten() {
        for class in DiagnosticClass::ALL {
            if class.retryability() == Retryability::Terminal {
                assert!(
                    class.change_required().touches_the_request(),
                    "{class} is terminal but names no change to the request, which leaves the \
                     caller with nothing to do"
                );
            }
        }
    }

    #[test]
    fn a_retryable_as_is_class_never_demands_the_request_be_rewritten() {
        for class in DiagnosticClass::ALL {
            if class.retryability() == Retryability::RetryableAsIs {
                assert!(
                    !class.change_required().touches_the_request(),
                    "{class} says re-send the identical request and also says rewrite it"
                );
            }
        }
    }

    #[test]
    fn retryable_after_change_is_the_only_decision_spanning_both_sides_of_the_request_boundary() {
        let spanning: Vec<bool> = DiagnosticClass::ALL
            .into_iter()
            .filter(|c| c.retryability() == Retryability::RetryableAfterChange)
            .map(|c| c.change_required().touches_the_request())
            .collect();
        assert!(
            spanning.contains(&true) && spanning.contains(&false),
            "if every retryable-after-change class landed on one side, the change axis would add \
             nothing to the retry axis"
        );
    }

    #[test]
    fn every_class_demands_a_remedy() {
        for class in DiagnosticClass::ALL {
            assert!(class.remedy_is_mandatory(), "{class} exempted itself");
        }
    }

    #[test]
    fn only_the_three_infrastructure_classes_page_an_operator() {
        let paging: Vec<_> = DiagnosticClass::ALL
            .iter()
            .filter(|c| c.is_system_failure())
            .copied()
            .collect();
        assert_eq!(
            paging,
            vec![
                DiagnosticClass::Stale,
                DiagnosticClass::Unavailable,
                DiagnosticClass::Internal
            ]
        );
    }

    #[test]
    fn the_taxonomy_table_round_trips_through_json() {
        let table = taxonomy_table();
        let encoded = serde_json::to_string(&table).expect("table serialises");
        let decoded: Vec<TaxonomyRow> = serde_json::from_str(&encoded).expect("table parses back");
        assert_eq!(table, decoded);
        assert_eq!(table.len(), 9);
    }
}
