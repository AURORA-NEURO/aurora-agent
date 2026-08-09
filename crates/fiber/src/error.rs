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
    #[error("unsupported query schema: expected {expected:?}, got {actual:?}")]
    UnsupportedQuerySchema { expected: &'static str, actual: String },

    #[error("query is not a JSON object")]
    QueryNotAnObject,

    #[error("missing required query field {0:?}")]
    MissingQueryField(&'static str),

    #[error("query field {field:?} has the wrong type: expected {expected}")]
    WrongQueryFieldType { field: &'static str, expected: &'static str },

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
