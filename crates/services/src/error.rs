//! The one failure vocabulary the nine contracts share.
//!
//! Implements blueprint 40.36 (Error Taxonomy, Exit Codes and Retry Semantics), which every §40
//! module leans on through the sentence it repeats verbatim: *a failure must be emitted as a typed
//! event with the module ID, world/run/cell identity where applicable, causal parent event, retry
//! classification, evidence references, and whether the failure invalidates only the current
//! projection or the underlying result.* That sentence is the field list of [`ServiceFault`].
//!
//! # Retryability is a property, not a comment
//!
//! `bioprism-cli` already ships six exit codes whose doc comments carry the retry decision in
//! prose — "Never retryable", "Possibly retryable". Prose does not survive a refactor and cannot be
//! matched on. Here [`ErrorClass::retryability`] is total over the enum, so a new class cannot be
//! added without deciding what a caller should do with it.
//!
//! 40.36's second invariant — *retryability and idempotency are distinct* — is why
//! [`Retryability`] has three cases rather than a bool. "Safe to retry as-is" and "retry only after
//! the caller changes something" are both non-`Never`, and collapsing them is how a client ends up
//! in a loop re-sending a request that will be refused identically forever.
//!
//! # Two classes that are not failures of the system
//!
//! 40.36 invariants 3 and 4 carve out [`ErrorClass::PolicyDenied`] (*policy denial is not system
//! failure*) and [`ErrorClass::Indeterminate`] (*scientific unknown is not execution error*). They
//! are in the taxonomy because a caller must be able to tell them apart from a crash, and
//! [`ErrorClass::is_system_failure`] is the predicate an operator's alerting would use. A platform
//! that pages on an oracle returning "unknown" has taught its operators to ignore the pager.
//!
//! # What is deliberately not implemented
//!
//! No transport and no HTTP status mapping. 40.36 names `problem+json`, and a mapping to it would
//! be a claim that something here serves traffic. Nothing in this crate does. The CLI exit-code
//! mapping is here because that registry exists in this workspace and can therefore be checked;
//! see [`ErrorClass::cli_exit_code`] and the test that reports what it cannot express.
//!
//! No clock, no retry executor, no backoff schedule. [`Retryability`] is the decision; the
//! scheduling of a retry belongs to the caller that owns the effect.

use serde::{Deserialize, Serialize};
use std::fmt;

/// What a caller may do with a failure.
///
/// Blueprint 40.36 input 4, "retry/effect classification".
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Retryability {
    /// Retrying the identical request produces the identical failure. The caller must stop.
    Never,
    /// Retrying the identical request produces the identical failure, but a different request
    /// might succeed: raise the budget, widen the boundary, pin the dependency.
    OnlyAfterCallerChange,
    /// The identical request may succeed later, and re-sending it is safe because the contract
    /// that raised the failure is idempotent under it.
    Safe,
}

impl Retryability {
    pub fn as_str(self) -> &'static str {
        match self {
            Retryability::Never => "never",
            Retryability::OnlyAfterCallerChange => "only-after-caller-change",
            Retryability::Safe => "safe",
        }
    }
}

impl fmt::Display for Retryability {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Whether a failure poisons the answer or only the rendering of it.
///
/// 40.36 and every §40 failure clause require this distinction: a summariser that produced an
/// inconsistent view has invalidated the projection, while a mutation whose postcondition failed
/// has invalidated the descendant itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Invalidates {
    /// The underlying result stands; only this view of it must be discarded and recomputed.
    Projection,
    /// The result itself is void. Anything derived from it is void with it.
    Result,
}

/// The nine classes every contract failure maps to, exactly one each.
///
/// The order is declaration order and carries no severity claim: two classes with different retry
/// decisions are not comparable on a single axis, and pretending otherwise would invite a
/// `max()` that quietly picks the wrong one. It exists so that iteration, serialisation and test
/// assertions are stable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorClass {
    /// The invocation itself was malformed: an unknown operation, a missing required argument.
    Usage,
    /// The payload parsed but did not satisfy the request descriptor.
    InvalidInput,
    /// A precondition on a version, digest or graph snapshot no longer holds. The caller re-reads
    /// and re-sends.
    Stale,
    /// The request contradicts state already committed: an idempotency key reused with a different
    /// body, a version name already bound to different content.
    Conflict,
    /// Policy refused the request. 40.36 invariant 3: not a system failure, and an operator must
    /// not be paged for it.
    PolicyDenied,
    /// The service cannot produce a result that satisfies its own declared invariants: a budget
    /// below the mandatory closure, a minimisation that would change the tested defect.
    ContractViolation,
    /// The science is unknown, not the execution. 40.36 invariant 4: an unstable oracle, an
    /// ambiguous decision boundary, a posterior that does not fit.
    Indeterminate,
    /// A declared dependency could not be reached or read.
    Unavailable,
    /// Unclassified. Present so the unknown-error rate 40.36 asks for is measurable rather than
    /// hidden inside another class.
    Internal,
}

impl ErrorClass {
    /// Every class, in declaration order. The taxonomy is closed; this is the whole of it.
    pub const ALL: [ErrorClass; 9] = [
        ErrorClass::Usage,
        ErrorClass::InvalidInput,
        ErrorClass::Stale,
        ErrorClass::Conflict,
        ErrorClass::PolicyDenied,
        ErrorClass::ContractViolation,
        ErrorClass::Indeterminate,
        ErrorClass::Unavailable,
        ErrorClass::Internal,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            ErrorClass::Usage => "usage",
            ErrorClass::InvalidInput => "invalid_input",
            ErrorClass::Stale => "stale",
            ErrorClass::Conflict => "conflict",
            ErrorClass::PolicyDenied => "policy_denied",
            ErrorClass::ContractViolation => "contract_violation",
            ErrorClass::Indeterminate => "indeterminate",
            ErrorClass::Unavailable => "unavailable",
            ErrorClass::Internal => "internal",
        }
    }

    /// The retry decision, total over the enum.
    pub fn retryability(self) -> Retryability {
        match self {
            ErrorClass::Usage | ErrorClass::InvalidInput | ErrorClass::Conflict => {
                Retryability::Never
            }
            ErrorClass::PolicyDenied
            | ErrorClass::ContractViolation
            | ErrorClass::Indeterminate => Retryability::OnlyAfterCallerChange,
            ErrorClass::Stale | ErrorClass::Unavailable | ErrorClass::Internal => {
                Retryability::Safe
            }
        }
    }

    /// Whether an operator should treat this as the platform being broken.
    ///
    /// False for [`ErrorClass::PolicyDenied`] and [`ErrorClass::Indeterminate`] by 40.36
    /// invariants 3 and 4, and false for every class whose cause is the caller's request.
    pub fn is_system_failure(self) -> bool {
        matches!(
            self,
            ErrorClass::Unavailable | ErrorClass::Internal | ErrorClass::Stale
        )
    }

    /// The exit code `bioprism-cli` would return for this class today.
    ///
    /// This is a description of the registry that exists, not a proposal. Four classes collide
    /// onto codes that mean something narrower than they do; see
    /// [`tests::the_shipped_exit_code_registry_cannot_distinguish_four_of_the_nine_classes`].
    pub fn cli_exit_code(self) -> u8 {
        match self {
            ErrorClass::Usage => 2,
            ErrorClass::InvalidInput => 3,
            ErrorClass::Stale
            | ErrorClass::Conflict
            | ErrorClass::PolicyDenied
            | ErrorClass::ContractViolation
            | ErrorClass::Indeterminate => 4,
            ErrorClass::Unavailable | ErrorClass::Internal => 5,
        }
    }

    /// Whether the shipped exit code preserves this class's retry decision.
    ///
    /// `bioprism-cli` treats code 5 alone as retryable. A class whose retryability is `Safe` and
    /// whose code is not 5 would be advertised as terminal when it is not, and the reverse would
    /// invite a retry loop.
    pub fn exit_code_preserves_retryability(self) -> bool {
        (self.cli_exit_code() == 5) == (self.retryability() == Retryability::Safe)
    }
}

impl fmt::Display for ErrorClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// One entry from a §40 module's "Failure semantics" list, bound to exactly one class.
///
/// The blueprint states these as bare noun phrases — "budget below closure", "oracle unstable",
/// "private metadata leak". A phrase is not a taxonomy. Binding each to an [`ErrorClass`] is what
/// makes the taxonomy total, and the binding is the part a reader may disagree with, so it is
/// written where it can be read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FailureMode {
    /// The blueprint's own phrase, so the mapping can be checked against the source.
    pub label: String,
    pub class: ErrorClass,
    /// What the failure voids.
    pub invalidates: Invalidates,
}

impl FailureMode {
    pub fn new(label: impl Into<String>, class: ErrorClass, invalidates: Invalidates) -> Self {
        FailureMode {
            label: label.into(),
            class,
            invalidates,
        }
    }

    /// A failure that voids only the view of an intact result.
    pub fn projection(label: impl Into<String>, class: ErrorClass) -> Self {
        FailureMode::new(label, class, Invalidates::Projection)
    }

    /// A failure that voids the result.
    pub fn result(label: impl Into<String>, class: ErrorClass) -> Self {
        FailureMode::new(label, class, Invalidates::Result)
    }

    pub fn retryability(&self) -> Retryability {
        self.class.retryability()
    }
}

/// The typed error object 40.36 requires, as a value.
///
/// Carries the module id, the identity the failure is about, the causal parent, evidence, the
/// retry classification and the invalidation scope. It is a value rather than a `std::error::Error`
/// on purpose: 40.36's first invariant is *errors are not strings only*, and the thing a worker
/// hands to a ledger is a record, not a `Display` impl.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServiceFault {
    /// The blueprint module the raising contract implements, for example `40.25`.
    pub module_id: String,
    /// The contract that raised it, as a stable slug.
    pub contract: String,
    pub class: ErrorClass,
    pub retryability: Retryability,
    pub invalidates: Invalidates,
    /// The world, run or cell the failure is about, where one applies.
    pub subject: Option<String>,
    /// The fault this one descends from. 40.36's "cause lost across worker" failure mode is what
    /// this field exists to prevent.
    pub caused_by: Option<Box<ServiceFault>>,
    /// Stable locators for the evidence, never the evidence itself. 40.36 lists "sensitive data in
    /// error" as a failure mode, and a locator cannot leak what it does not carry.
    pub evidence: Vec<String>,
    /// The blueprint's phrase for the failure mode, for a human reader.
    pub detail: String,
}

impl ServiceFault {
    /// Build a fault from a declared failure mode, so the retry class and invalidation scope
    /// cannot disagree with the taxonomy.
    pub fn from_mode(
        module_id: impl Into<String>,
        contract: impl Into<String>,
        mode: &FailureMode,
    ) -> Self {
        ServiceFault {
            module_id: module_id.into(),
            contract: contract.into(),
            class: mode.class,
            retryability: mode.class.retryability(),
            invalidates: mode.invalidates,
            subject: None,
            caused_by: None,
            evidence: Vec::new(),
            detail: mode.label.clone(),
        }
    }

    pub fn about(mut self, subject: impl Into<String>) -> Self {
        self.subject = Some(subject.into());
        self
    }

    pub fn caused_by(mut self, parent: ServiceFault) -> Self {
        self.caused_by = Some(Box::new(parent));
        self
    }

    pub fn with_evidence(mut self, locator: impl Into<String>) -> Self {
        self.evidence.push(locator.into());
        self
    }

    /// The chain from this fault to its root cause, nearest first.
    pub fn chain(&self) -> Vec<&ServiceFault> {
        let mut chain = vec![self];
        let mut cursor = self;
        while let Some(parent) = cursor.caused_by.as_deref() {
            chain.push(parent);
            cursor = parent;
        }
        chain
    }

    /// The class of the root cause, which is the one a caller can act on.
    ///
    /// A wrapper reporting `Unavailable` over a root `PolicyDenied` would tell an operator to
    /// retry a request policy will refuse forever. 40.36 lists "cause lost across worker" as a
    /// failure mode; this is the accessor that makes keeping the cause worth the field.
    pub fn root_class(&self) -> ErrorClass {
        self.chain()
            .last()
            .expect("a chain contains at least this fault")
            .class
    }
}

/// Failures of this crate itself.
///
/// Distinct from [`ServiceFault`], which describes a failure of a *described* service. These are
/// raised when a contract, a graph or a conformance report is malformed as a document.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ServicesError {
    #[error("contract {contract} declares failure mode {label:?} twice with different classes: {first} and {second}")]
    ContradictoryFailureMode {
        contract: String,
        label: String,
        first: ErrorClass,
        second: ErrorClass,
    },

    #[error("contract {contract} declares no failure mode for error class {class}, so the class is unreachable from it")]
    UnreachableErrorClass { contract: String, class: ErrorClass },

    #[error("contract {contract} has a malformed {part} descriptor: {detail}")]
    MalformedDescriptor {
        contract: String,
        part: &'static str,
        detail: String,
    },

    #[error("contract {contract} cannot be compared with {other}: {detail}")]
    Incomparable {
        contract: String,
        other: String,
        detail: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_error_class_has_exactly_one_retry_decision() {
        for class in ErrorClass::ALL {
            let decision = class.retryability();
            assert_eq!(
                decision,
                class.retryability(),
                "{class} must not depend on when it is asked"
            );
        }
        assert_eq!(ErrorClass::ALL.len(), 9);
    }

    #[test]
    fn a_policy_denial_is_not_a_system_failure() {
        assert!(!ErrorClass::PolicyDenied.is_system_failure());
        assert!(!ErrorClass::Indeterminate.is_system_failure());
        assert!(ErrorClass::Unavailable.is_system_failure());
    }

    #[test]
    fn a_scientific_unknown_is_never_reported_as_safe_to_retry_unchanged() {
        assert_eq!(
            ErrorClass::Indeterminate.retryability(),
            Retryability::OnlyAfterCallerChange,
            "re-running an unstable oracle with the same inputs is how an unstable oracle gets \
             mistaken for a flaky one"
        );
    }

    #[test]
    fn a_conflict_is_never_retryable_because_the_committed_state_will_not_move() {
        assert_eq!(ErrorClass::Conflict.retryability(), Retryability::Never);
        assert_eq!(ErrorClass::Stale.retryability(), Retryability::Safe);
    }

    #[test]
    fn the_shipped_exit_code_registry_cannot_distinguish_four_of_the_nine_classes() {
        let collapsed: Vec<ErrorClass> = ErrorClass::ALL
            .into_iter()
            .filter(|class| class.cli_exit_code() == 4)
            .collect();
        assert_eq!(
            collapsed,
            [
                ErrorClass::Stale,
                ErrorClass::Conflict,
                ErrorClass::PolicyDenied,
                ErrorClass::ContractViolation,
                ErrorClass::Indeterminate
            ],
            "five classes share exit code 4, which the CLI documents as \"compilation could not \
             produce a sound result\"; four of them are not compilation failures at all"
        );
    }

    #[test]
    fn the_exit_code_registry_misreports_retryability_for_exactly_one_class() {
        let wrong: Vec<ErrorClass> = ErrorClass::ALL
            .into_iter()
            .filter(|class| !class.exit_code_preserves_retryability())
            .collect();
        assert_eq!(
            wrong,
            [ErrorClass::Stale],
            "a stale precondition is safe to retry after a re-read, and code 4 advertises it as \
             terminal"
        );
    }

    #[test]
    fn a_fault_chain_reports_the_root_cause_class_not_the_outermost_one() {
        let root = ServiceFault::from_mode(
            "40.12",
            "policy",
            &FailureMode::result("policy conflict", ErrorClass::PolicyDenied),
        );
        let wrapper = ServiceFault::from_mode(
            "40.25",
            "context-compiler",
            &FailureMode::result("stale graph", ErrorClass::Unavailable),
        )
        .caused_by(root);

        assert_eq!(wrapper.class, ErrorClass::Unavailable);
        assert_eq!(wrapper.root_class(), ErrorClass::PolicyDenied);
        assert_eq!(wrapper.chain().len(), 2);
    }

    #[test]
    fn a_fault_carries_evidence_locators_rather_than_evidence() {
        let fault = ServiceFault::from_mode(
            "40.25",
            "context-compiler",
            &FailureMode::projection("unresolvable source", ErrorClass::Unavailable),
        )
        .about("world-1")
        .with_evidence("cas://sha256:abc");

        assert_eq!(fault.subject.as_deref(), Some("world-1"));
        assert_eq!(fault.evidence, ["cas://sha256:abc"]);
        assert_eq!(fault.retryability, Retryability::Safe);
        assert_eq!(fault.invalidates, Invalidates::Projection);
    }

    #[test]
    fn a_fault_round_trips_through_json_with_its_cause_chain_intact() {
        let fault = ServiceFault::from_mode(
            "40.22",
            "mutation-runtime",
            &FailureMode::result("precondition false", ErrorClass::ContractViolation),
        )
        .caused_by(ServiceFault::from_mode(
            "40.18",
            "world-builder",
            &FailureMode::result("dependency not pinned", ErrorClass::InvalidInput),
        ));

        let json = serde_json::to_value(&fault).expect("a fault serialises");
        let back: ServiceFault = serde_json::from_value(json).expect("a fault deserialises");
        assert_eq!(back, fault);
        assert_eq!(back.chain().len(), 2);
    }
}
