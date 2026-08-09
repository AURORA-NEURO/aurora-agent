use bioprism_scope::ScopeError;
use thiserror::Error;

/// Errors that make a world unusable.
///
/// The first three variants mirror the CPython reference `validate_world` exactly, in the same
/// order, so that a world rejected by one implementation is rejected by the other. Anything the
/// reference accepts must at most produce a [`crate::validate::Diagnostic`] here, never an error.
#[derive(Debug, Error, PartialEq, Eq, Clone)]
pub enum WorldError {
    #[error("unsupported world schema: expected {expected:?}, got {actual:?}")]
    UnsupportedSchema { expected: &'static str, actual: String },

    #[error("duplicate fact id: {0}")]
    DuplicateFactId(String),

    #[error("duplicate factor id: {0}")]
    DuplicateFactorId(String),

    #[error("factor {factor} has unknown inputs {missing:?}")]
    UnknownFactorInputs { factor: String, missing: Vec<String> },

    #[error("world is not a JSON object")]
    NotAnObject,

    #[error("missing required field {field:?} on {subject}")]
    MissingField { field: &'static str, subject: String },

    #[error("field {field:?} on {subject} has the wrong type: expected {expected}")]
    WrongType {
        field: &'static str,
        subject: String,
        expected: &'static str,
    },

    #[error("invalid scope on {subject}: {source}")]
    Scope {
        subject: String,
        #[source]
        source: ScopeError,
    },

    #[error("invalid timestamp on {subject}: {message}")]
    Timestamp { subject: String, message: String },

    #[error("invalid identifier on {subject}: {message}")]
    Identifier { subject: String, message: String },
}
