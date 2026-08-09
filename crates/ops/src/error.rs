//! Typed failures for the operational contracts, classified under the taxonomy this workspace
//! already has.
//!
//! Every one of the seven §40 modules written in the progressive-disclosure template repeats the
//! same sentence: *a failure must be emitted as a typed event with the module ID, world/run/cell
//! identity where applicable, causal parent event, retry classification, evidence references, and
//! whether the failure invalidates only the current projection or the underlying result*.
//!
//! `bioprism-services` already turned that sentence into types — [`ErrorClass`], [`Retryability`]
//! and [`Invalidates`] — so this crate borrows them rather than growing a second taxonomy that
//! would drift. [`OpsError::class`] and [`OpsError::invalidates`] are the two answers the sentence
//! demands; the retry classification is derived from the class and is not stated independently,
//! because a variant free to disagree with its own class is a variant that eventually will.
//!
//! # The invalidation split carries an invariant
//!
//! 40.34's second non-negotiable invariant is *telemetry is an import/export projection*. If that
//! is true then no telemetry failure can invalidate a result: an export that never left the process
//! cannot make a computed answer wrong. So every variant raised by [`crate::telemetry`] returns
//! [`Invalidates::Projection`], every variant raised by [`crate::config`] and [`crate::flags`]
//! returns [`Invalidates::Result`] — a run whose effective configuration is unresolvable is a run
//! whose output nobody can reproduce — and a test holds the partition. The invariant is thereby
//! checkable rather than quoted.
//!
//! # What is deliberately not here
//!
//! No error codes, no exit codes, no HTTP statuses and no retry executor. `bioprism_services::
//! ErrorClass::cli_exit_code` already maps the taxonomy to a process exit code, and acting on a
//! retryability is the business of whoever owns the effect. Nothing in this crate sleeps, retries
//! or backs off.

use bioprism_services::{ErrorClass, Invalidates, Retryability};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Every failure this crate can produce.
///
/// Read each message as a statement about a *model* — a configuration that cannot be resolved, a
/// metric that no observation supports, a criterion that cannot be claimed. Nothing here intercepts
/// anything at runtime; see the crate docs for the full list of what does not exist.
#[derive(Debug, Clone, PartialEq, Eq, Error, Serialize, Deserialize)]
#[serde(tag = "error", rename_all = "snake_case")]
pub enum OpsError {
    /// A layer bound a key the schema does not declare. 40.10's `unknown setting`.
    #[error("no setting {key:?} is declared; {origin} bound it anyway")]
    UnknownSetting { key: String, origin: String },

    /// Two sources at the same precedence level bound one key to different values, so the
    /// resolution order between them decides the run and nothing states that order. 40.10's
    /// `ambiguous precedence`.
    #[error(
        "{key:?} is bound at layer {layer} by both {first} and {second} with different values; \
         precedence within a layer is undefined"
    )]
    AmbiguousPrecedence {
        key: String,
        layer: String,
        first: String,
        second: String,
    },

    /// A permission defaulted to granted. 40.10's fourth invariant, *unsafe defaults are denied*,
    /// read as the only thing it can mean for a permission: absence grants nothing.
    #[error("permission {key:?} is granted by the defaults layer; a permission may only default to denied")]
    UnsafeDefault { key: String },

    /// A secret setting was declared as participating in an emitted artifact. Unreachable through
    /// this crate's constructors and reachable through a deserialized schema, which is why it
    /// exists — see [`crate::config::SettingSpec`].
    #[error(
        "setting {key:?} is a secret and is declared as entering an emitted artifact; the artifact \
         digest would then depend on a value no result bundle may record"
    )]
    SecretInDigest { key: String },

    /// A deployment profile bound a setting that enters an emitted artifact. 40.38's first
    /// invariant, *deployment changes providers, not object semantics*.
    #[error(
        "deployment profile {profile:?} binds {key:?}, which enters emitted artifacts; a profile \
         may bind operational settings only"
    )]
    ProfileChangesSemantics { profile: String, key: String },

    /// A declared setting with no default was left unbound by every layer.
    #[error("setting {key:?} is required and no layer bound it")]
    MissingRequiredSetting { key: String },

    /// A binding's value does not have the declared type.
    #[error("setting {key:?} is declared {expected} and {origin} bound a {actual}")]
    TypeMismatch {
        key: String,
        expected: String,
        actual: String,
        origin: String,
    },

    /// A secret was requested and no reference for it resolved. 40.10's `secret unavailable`.
    #[error("no secret reference is bound for {key:?}")]
    SecretUnavailable { key: String },

    /// A lease was consulted outside the epoch window it was issued for. 40.10's `secret expiry`
    /// under this workspace's logical clock rather than a wall clock.
    #[error("lease on {reference} expired at {expires} and was consulted at {consulted}")]
    LeaseExpired {
        reference: String,
        expires: u64,
        consulted: u64,
    },

    /// A secret reference reachable from process-wide state with no lease naming an execution
    /// boundary. 40.39's third invariant, *no ambient credentials*.
    #[error(
        "secret {reference} is readable from process-wide state and no lease scopes it to an \
         execution boundary"
    )]
    AmbientCredential { reference: String },

    /// An effect was exercised that the run did not declare. 40.39's fourth invariant, *network and
    /// filesystem effects are explicit*, with the default set to deny.
    #[error("{run:?} did not declare the effect {effect}; undeclared effects are denied")]
    UndeclaredEffect { run: String, effect: String },

    /// A flag declared as a runtime toggle changes what a compile emits. Reachable the same way
    /// [`OpsError::SecretInDigest`] is: through a deserialized declaration, not a constructor.
    #[error(
        "flag {flag:?} is declared a runtime toggle and changes emitted artifacts; such a flag is \
         an artifact version, not a toggle"
    )]
    ToggleMovesEmittedArtifact { flag: String },

    /// A pinned run consulted a flag the pin does not contain. 40.10's `flag changes during pinned
    /// run`, in its quieter form: a flag nobody pinned can take either value between two runs.
    #[error("run pinned at {pin} consulted flag {flag:?}, which the pin does not contain")]
    FlagNotPinned { flag: String, pin: String },

    /// A pinned run observed a flag at a value other than the pinned one.
    #[error("flag {flag:?} is pinned to {pinned} and was consulted as {observed} in the same run")]
    FlagChangedDuringPinnedRun {
        flag: String,
        pinned: String,
        observed: String,
    },

    /// An author labelled a flag change compatible when the change moves emitted artifacts. The
    /// same shape as `bioprism_governance::DigestBreach`, in the currency of feature flags.
    #[error(
        "change to flag {flag:?} was declared {declared} and is {derived}: {reason}"
    )]
    FlagChangeMisclassified {
        flag: String,
        declared: String,
        derived: String,
        reason: String,
    },

    /// A derived metric was evaluated over signals nobody observed.
    #[error("metric {metric:?} is not supported by observation; unobserved inputs: {missing:?}")]
    UnsupportedMetric {
        metric: String,
        missing: Vec<String>,
    },

    /// A metric's value is not determined by its observations — a ratio over a zero denominator is
    /// the case that matters, because reporting it as zero or as one are both lies.
    #[error("metric {metric:?} is indeterminate: {reason}")]
    IndeterminateMetric { metric: String, reason: String },

    /// A field reached an export with no treatment declared for its scope class. Deny by default:
    /// forgetting to classify must not mean emitting.
    #[error(
        "field {field:?} is classified {class} and redaction policy {policy} declares no treatment \
         for that class"
    )]
    RedactionMiss {
        field: String,
        class: String,
        policy: String,
    },

    /// A redaction policy would export fields whose scope class is unclassified.
    ///
    /// `bioprism_scope::ScopeClass::Unclassified` exists so that an unknown dimension is reported
    /// rather than treated as an opaque string. An export policy that emits it is claiming a
    /// dimension nobody classified carries nothing restricted.
    #[error(
        "redaction policy {policy} declares that unclassified fields are emitted; an unclassified \
         dimension cannot be shown to be safe to export"
    )]
    UnclassifiedEmission { policy: String },

    /// A telemetry record was correlated with a domain event it does not project. 40.34's
    /// `trace/domain event mismatch`.
    #[error("telemetry record projects event {record:?} and was correlated with event {event:?}")]
    TraceMismatch { record: String, event: String },

    /// A signal exceeded its declared label budget. 40.34's `cardinality explosion`.
    #[error("signal {signal:?} reached {distinct} distinct labels against a budget of {ceiling}")]
    CardinalityExceeded {
        signal: String,
        distinct: usize,
        ceiling: usize,
    },

    /// A workload contains an operation with no traversal bound. 40.35's second invariant, *queries
    /// and actions are bounded*: an unbounded operation has no capacity, not an unknown one.
    #[error("operation {operation:?} of workload {workload:?} declares no traversal bound")]
    UnboundedWorkload { workload: String, operation: String },

    /// A workload materialises an artifact above the model's memory ceiling. 40.35's third
    /// invariant, *large artifacts stream*.
    #[error(
        "operation {operation:?} materialises {bytes} bytes against a ceiling of {ceiling}; \
         artifacts above the ceiling must stream"
    )]
    UnstreamedArtifact {
        operation: String,
        bytes: u64,
        ceiling: u64,
    },

    /// A degradation plan concedes nothing. 40.35's first invariant, *correctness and policy do not
    /// degrade silently under load*: a plan that names no concession is the silent case.
    #[error("degradation plan {plan:?} names no concession, so degradation under it is invisible")]
    SilentDegradation { plan: String },

    /// An acceptance criterion was recorded as met on the strength of somebody saying so.
    #[error(
        "criterion {criterion} was recorded met on a {basis} basis; only an observation that \
         entails the criterion can support met"
    )]
    AssertedAcceptance { criterion: String, basis: String },

    /// A name that has to be stable was empty or carried whitespace.
    #[error("{field} {value:?} is not a well-formed name")]
    MalformedName { field: String, value: String },
}

impl OpsError {
    /// The 40.36 class this failure belongs to.
    pub fn class(&self) -> ErrorClass {
        match self {
            OpsError::UnknownSetting { .. }
            | OpsError::TypeMismatch { .. }
            | OpsError::MissingRequiredSetting { .. }
            | OpsError::MalformedName { .. } => ErrorClass::InvalidInput,

            OpsError::AmbiguousPrecedence { .. }
            | OpsError::SecretInDigest { .. }
            | OpsError::ProfileChangesSemantics { .. }
            | OpsError::ToggleMovesEmittedArtifact { .. }
            | OpsError::FlagChangeMisclassified { .. }
            | OpsError::UnsupportedMetric { .. }
            | OpsError::UnboundedWorkload { .. }
            | OpsError::UnstreamedArtifact { .. }
            | OpsError::SilentDegradation { .. }
            | OpsError::AssertedAcceptance { .. }
            | OpsError::TraceMismatch { .. } => ErrorClass::ContractViolation,

            OpsError::UnsafeDefault { .. }
            | OpsError::AmbientCredential { .. }
            | OpsError::UndeclaredEffect { .. }
            | OpsError::UnclassifiedEmission { .. }
            | OpsError::RedactionMiss { .. } => ErrorClass::PolicyDenied,

            OpsError::FlagNotPinned { .. }
            | OpsError::FlagChangedDuringPinnedRun { .. }
            | OpsError::LeaseExpired { .. } => ErrorClass::Stale,

            OpsError::SecretUnavailable { .. } => ErrorClass::Unavailable,

            OpsError::IndeterminateMetric { .. } => ErrorClass::Indeterminate,

            OpsError::CardinalityExceeded { .. } => ErrorClass::Conflict,
        }
    }

    /// Whether the failure invalidates only the projection or the underlying result.
    ///
    /// The telemetry variants are all [`Invalidates::Projection`] and that is not a stylistic
    /// choice: it is 40.34's second invariant. If an export failure could invalidate a result then
    /// telemetry would be part of the computation rather than a projection of it.
    pub fn invalidates(&self) -> Invalidates {
        match self {
            OpsError::UnsupportedMetric { .. }
            | OpsError::IndeterminateMetric { .. }
            | OpsError::RedactionMiss { .. }
            | OpsError::UnclassifiedEmission { .. }
            | OpsError::TraceMismatch { .. }
            | OpsError::CardinalityExceeded { .. }
            | OpsError::UnboundedWorkload { .. }
            | OpsError::UnstreamedArtifact { .. }
            | OpsError::SilentDegradation { .. } => Invalidates::Projection,
            _ => Invalidates::Result,
        }
    }

    /// Derived from [`OpsError::class`], never stated separately.
    pub fn retryability(&self) -> Retryability {
        self.class().retryability()
    }

    /// The blueprint module whose contract this failure belongs to.
    ///
    /// The template's failure sentence asks for "the module ID", and a failure that cannot say
    /// which contract it violates is a failure nobody can route.
    pub fn module_id(&self) -> &'static str {
        match self {
            OpsError::UnknownSetting { .. }
            | OpsError::AmbiguousPrecedence { .. }
            | OpsError::UnsafeDefault { .. }
            | OpsError::SecretInDigest { .. }
            | OpsError::MissingRequiredSetting { .. }
            | OpsError::TypeMismatch { .. }
            | OpsError::SecretUnavailable { .. }
            | OpsError::LeaseExpired { .. }
            | OpsError::ToggleMovesEmittedArtifact { .. }
            | OpsError::FlagNotPinned { .. }
            | OpsError::FlagChangedDuringPinnedRun { .. }
            | OpsError::FlagChangeMisclassified { .. }
            | OpsError::MalformedName { .. } => "40.10",

            OpsError::ProfileChangesSemantics { .. } => "40.38",

            OpsError::UnsupportedMetric { .. }
            | OpsError::IndeterminateMetric { .. }
            | OpsError::RedactionMiss { .. }
            | OpsError::UnclassifiedEmission { .. }
            | OpsError::TraceMismatch { .. }
            | OpsError::CardinalityExceeded { .. } => "40.34",

            OpsError::UnboundedWorkload { .. }
            | OpsError::UnstreamedArtifact { .. }
            | OpsError::SilentDegradation { .. } => "40.35",

            OpsError::AmbientCredential { .. } | OpsError::UndeclaredEffect { .. } => "40.39",

            OpsError::AssertedAcceptance { .. } => "40.42",
        }
    }
}

pub(crate) fn well_formed_name(field: &str, value: &str) -> Result<String, OpsError> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed != value || value.contains(char::is_whitespace) {
        return Err(OpsError::MalformedName {
            field: field.to_string(),
            value: value.to_string(),
        });
    }
    Ok(value.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_telemetry_failure_can_invalidate_a_result() {
        let telemetry = [
            OpsError::UnsupportedMetric {
                metric: "trace_coverage".into(),
                missing: vec!["spans_total".into()],
            },
            OpsError::RedactionMiss {
                field: "subject".into(),
                class: "identity".into(),
                policy: "v1".into(),
            },
            OpsError::CardinalityExceeded {
                signal: "compile".into(),
                distinct: 90_000,
                ceiling: 100,
            },
        ];
        for error in telemetry {
            assert_eq!(
                error.invalidates(),
                Invalidates::Projection,
                "40.34 invariant 2 says telemetry is a projection; {error} claimed otherwise"
            );
        }
    }

    #[test]
    fn an_unresolvable_configuration_invalidates_the_result_not_only_the_projection() {
        let error = OpsError::AmbiguousPrecedence {
            key: "store.root".into(),
            layer: "environment".into(),
            first: "BIOPRISM_STORE".into(),
            second: "BIOPRISM_STORE_ROOT".into(),
        };
        assert_eq!(error.invalidates(), Invalidates::Result);
    }

    #[test]
    fn retryability_is_derived_from_the_class_and_cannot_disagree_with_it() {
        let error = OpsError::SecretUnavailable {
            key: "hub.token".into(),
        };
        assert_eq!(error.class(), ErrorClass::Unavailable);
        assert_eq!(error.retryability(), ErrorClass::Unavailable.retryability());
    }

    #[test]
    fn every_failure_names_the_contract_it_violates() {
        let samples = [
            OpsError::UnsafeDefault { key: "k".into() },
            OpsError::ProfileChangesSemantics {
                profile: "p".into(),
                key: "k".into(),
            },
            OpsError::TraceMismatch {
                record: "a".into(),
                event: "b".into(),
            },
            OpsError::SilentDegradation { plan: "p".into() },
            OpsError::AmbientCredential {
                reference: "env:T".into(),
            },
            OpsError::AssertedAcceptance {
                criterion: "c".into(),
                basis: "author".into(),
            },
        ];
        for error in samples {
            let id = error.module_id();
            assert!(id.starts_with("40."), "{error} named {id}");
        }
    }

    #[test]
    fn a_name_with_interior_whitespace_is_rejected_rather_than_normalised() {
        assert!(well_formed_name("flag", "graph compiled").is_err());
        assert!(well_formed_name("flag", " trimmed").is_err());
        assert_eq!(well_formed_name("flag", "graph.compiled").unwrap(), "graph.compiled");
    }
}
