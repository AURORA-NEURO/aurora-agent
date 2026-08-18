use crate::policy::PolicyViolation;
use bioprism_world::WorldError;
use thiserror::Error;

/// Typed compiler failures.
///
/// Blueprint 43.16 requires phase-specific failures and 40.36 a documented error taxonomy. Each
/// variant names the pass that produced it, so a caller can tell a malformed query from a world
/// that cannot be compiled within budget.
#[derive(Debug, Error, PartialEq, Clone)]
pub enum FiberError {
    /// The document declares a version outside the accepted set.
    ///
    /// `expected` is a list because the reader preserves valid legacy 0.1/0.2 documents while
    /// also accepting the explicitly extended 0.3 decision-contract form.
    #[error("unsupported query schema: expected one of {expected:?}, got {actual:?}")]
    UnsupportedQuerySchema {
        expected: &'static [&'static str],
        actual: String,
    },

    #[error("query is not a JSON object")]
    QueryNotAnObject,

    /// The query carried a key the wire format does not declare.
    ///
    /// Refusing is not pedantry about spelling. Until 0.2 the parser read the keys it knew and
    /// dropped the rest, and the whole query document is hashed into `source_hashes.query_sha256`
    /// and thence into the certificate's own digest — so an undeclared key was ignored
    /// *semantically* and honoured *cryptographically*. Two compiles that selected identical
    /// facts, made identical omissions and delivered a byte-identical Decision Section carried
    /// different certificate digests. A caller who believed they had supplied a decision loss got
    /// no signal that nothing read it, and the artifact's identity moved for a change that
    /// provably could not affect the answer.
    ///
    /// Every offending path is reported, sorted, rather than the first one found: serde_json runs
    /// with `preserve_order`, so document order is not a stable thing to report, and a caller
    /// fixing a generator wants the whole list rather than one round trip per key.
    #[error("query carries undeclared field(s) {fields:?}; this schema version declares exactly {accepted:?}")]
    UnknownQueryFields {
        fields: Vec<String>,
        accepted: &'static [&'static str],
    },

    #[error("invalid decision contract: {0}")]
    InvalidDecisionContract(String),

    #[error("missing required query field {0:?}")]
    MissingQueryField(&'static str),

    #[error("query field {field:?} has the wrong type: expected {expected}")]
    WrongQueryFieldType {
        field: &'static str,
        expected: &'static str,
    },

    #[error("invalid query identifier: {0}")]
    InvalidIdentifier(String),

    #[error("invalid decision_time: {0}")]
    InvalidDecisionTime(String),

    /// The protected closure plus the dependency slice already exceed the caller's budget.
    ///
    /// 43.13 forbids trimming protected evidence to fit, and 43.25 forbids silent truncation, so
    /// this is a hard failure rather than a smaller context.
    #[error("protected/sliced facts exceed max_facts: {selected}")]
    BudgetExceeded { selected: usize, max_facts: usize },

    /// A subject's split assignment was missing while other subjects sharing the alias had one.
    ///
    /// The reference implementation raises `TypeError` here by sorting a set containing `None`
    /// alongside strings. Surfacing it as a typed error keeps the two implementations in
    /// agreement about which worlds compile.
    #[error("subjects sharing alias {alias:?} have a missing split assignment alongside {present:?}; the split-integrity oracle cannot order the groups")]
    UnorderableSplitGroups { alias: String, present: Vec<String> },

    /// The policy pass refused (43.33), including 40.25's named `policy conflict` failure.
    ///
    /// Carried transparently, exactly as [`FiberError::World`] carries `WorldError`. Policy has a
    /// taxonomy of its own and it belongs in [`PolicyViolation`]; giving the compiler's error type
    /// one variant per policy rule would spread that taxonomy across two enums and force every
    /// consumer classifying compiler failures to re-derive it.
    #[error(transparent)]
    Policy(#[from] PolicyViolation),

    #[error(transparent)]
    World(#[from] WorldError),
}
