//! Typed ingest failures.
//!
//! Blueprint 40.17's failure semantics list — format/version unsupported, identity ambiguous,
//! unit/reference unknown, partial ingest, source changes — becomes one variant each, and
//! 40.36 requires each to be actionable on its own. Every variant that refers to something
//! inside a source therefore carries a [`SourceLocation`], so the message identifies the
//! offending byte range rather than the adapter that tripped over it.
//!
//! Errors here are for *unusable* sources. Anything the adapter can ingest while telling the
//! truth about what it dropped is a [`crate::SemanticLoss`] entry instead, never an error:
//! turning recoverable loss into a hard failure would push adapter authors toward silently
//! dropping fields to keep the pipeline green, which is the exact failure this crate prevents.
//!
//! Locations are boxed. A [`SourceLocation`] is four owned strings wide, and an unboxed one in
//! every variant would make `Result<Ingestion, AdapterError>` pay for the error path on every
//! successful return — the constructors below keep the boxing out of call sites.

use crate::location::SourceLocation;
use bioprism_ids::{CanonicalError, IdError};
use thiserror::Error;

/// Failures of the hand-rolled tabular reader.
///
/// The reader is strict on purpose. RFC 4180 leaves several situations undefined — a bare
/// quote inside an unquoted field, a lone carriage return, a ragged row — and every permissive
/// resolution of them is a guess about what the author meant. Guessing is what 40.17 invariant
/// 2 forbids, so each ambiguity is an error that names its record.
#[derive(Debug, Error, PartialEq, Eq, Clone)]
pub enum CsvError {
    #[error("{source_id} exceeds the {maximum}-byte source limit")]
    SourceTooLarge { source_id: String, maximum: usize },

    #[error("{source_id} uses an invalid delimiter byte {found}")]
    InvalidDelimiter { source_id: String, found: u8 },

    #[error("{source_id} exceeds the {maximum}-column limit")]
    TooManyColumns { source_id: String, maximum: usize },

    #[error("{source_id} exceeds the {maximum}-record limit")]
    TooManyRecords { source_id: String, maximum: usize },

    #[error("{location} exceeds the {maximum}-byte field limit")]
    FieldTooLarge {
        location: Box<SourceLocation>,
        maximum: usize,
    },

    #[error("{source_id} is not valid UTF-8 at byte {offset}")]
    NotUtf8 { source_id: String, offset: usize },

    #[error("{source_id} is empty and has no header record")]
    NoHeader { source_id: String },

    #[error("{source_id} declares column {name:?} twice, at positions {first} and {second}")]
    DuplicateColumn {
        source_id: String,
        name: String,
        first: usize,
        second: usize,
    },

    #[error("{source_id} has an empty column name at position {position}")]
    EmptyColumnName { source_id: String, position: usize },

    #[error("{location} has {actual} fields but the header declares {expected}")]
    RaggedRecord {
        location: Box<SourceLocation>,
        expected: usize,
        actual: usize,
    },

    #[error("{location} opens a quoted field that is never closed")]
    UnterminatedQuote { location: Box<SourceLocation> },

    #[error("{location} contains a quote inside an unquoted field")]
    BareQuote { location: Box<SourceLocation> },

    #[error(
        "{location} has character {found:?} after a closing quote, expected a delimiter or a \
         record end"
    )]
    TrailingGarbageAfterQuote {
        location: Box<SourceLocation>,
        found: char,
    },

    #[error("{location} contains a carriage return that is not part of a CRLF record end")]
    LoneCarriageReturn { location: Box<SourceLocation> },
}

impl CsvError {
    pub(crate) fn ragged(location: SourceLocation, expected: usize, actual: usize) -> Self {
        CsvError::RaggedRecord {
            location: Box::new(location),
            expected,
            actual,
        }
    }

    pub(crate) fn unterminated_quote(location: SourceLocation) -> Self {
        CsvError::UnterminatedQuote {
            location: Box::new(location),
        }
    }

    pub(crate) fn bare_quote(location: SourceLocation) -> Self {
        CsvError::BareQuote {
            location: Box::new(location),
        }
    }

    pub(crate) fn trailing_garbage(location: SourceLocation, found: char) -> Self {
        CsvError::TrailingGarbageAfterQuote {
            location: Box::new(location),
            found,
        }
    }

    pub(crate) fn lone_carriage_return(location: SourceLocation) -> Self {
        CsvError::LoneCarriageReturn {
            location: Box::new(location),
        }
    }
}

/// Everything that can stop an ingest.
#[derive(Debug, Error)]
pub enum AdapterError {
    #[error("invalid source: {0}")]
    InvalidSource(String),

    #[error("invalid semantic loss: {0}")]
    InvalidLoss(String),

    #[error("adapter source traversal limit exceeded at {path}: maximum {maximum}")]
    TraversalLimit { path: String, maximum: usize },

    #[error("conformance report is invalid: {0}")]
    Conformance(String),

    #[error("adapter {adapter} does not accept source {source_id}: {expected}")]
    UnsupportedSource {
        adapter: &'static str,
        source_id: String,
        expected: &'static str,
    },

    #[error("adapter {adapter} does not support format {declared:?} for source {source_id}")]
    UnsupportedFormat {
        adapter: &'static str,
        source_id: String,
        declared: String,
    },

    #[error(transparent)]
    Csv(#[from] CsvError),

    #[error("io error at {path}: {message}")]
    Io { path: String, message: String },

    /// A scope dimension the mapping declares as identity-bearing has no value in this record.
    /// 40.17 invariant 2: the adapter must not invent one, and must not silently emit a fact
    /// whose validity scope is wider than the evidence supports.
    #[error("{location} is bound to identity dimension {dimension:?} but is empty")]
    AmbiguousIdentity {
        location: Box<SourceLocation>,
        dimension: String,
    },

    /// The mapping policy names a field the source does not have. Not a loss — a loss is
    /// something the source had and the world does not — but a signal that the source drifted
    /// away from the policy, which 40.17 lists as its own failure mode.
    #[error(
        "{location} is required by the mapping policy but the source has no such field; \
         expected {expected}"
    )]
    SchemaDrift {
        location: Box<SourceLocation>,
        expected: String,
    },

    /// A cell does not hold the type the mapping policy declares for its column. Coercing it,
    /// or dropping the record, would both be silent repairs of a disagreement between the
    /// policy and the data that only a human can resolve.
    #[error("{location} holds {found:?}, which is not a {expected}")]
    ValueTypeMismatch {
        location: Box<SourceLocation>,
        expected: &'static str,
        found: String,
    },

    #[error("{location} produced an invalid identifier: {message}")]
    Identifier {
        location: Box<SourceLocation>,
        message: String,
    },

    /// An emitted fact does not parse under the `fiber-world/0.1` fact schema. 40.17 invariant
    /// 4 ("validation precedes publication") is enforced inside [`crate::Ingestion::new`], so
    /// this is the error an adapter author sees rather than a malformed world downstream.
    #[error("{location} produced a fact that is not valid under fiber-world/0.1: {message}")]
    MalformedFact {
        location: Box<SourceLocation>,
        message: String,
    },

    #[error("two emitted facts share the id {id:?}, at {first} and {second}")]
    DuplicateFactId {
        id: String,
        first: Box<SourceLocation>,
        second: Box<SourceLocation>,
    },

    #[error("could not canonicalize an emitted value at {location}: {source}")]
    Canonical {
        location: Box<SourceLocation>,
        #[source]
        source: CanonicalError,
    },
}

impl AdapterError {
    pub(crate) fn identifier(location: SourceLocation, error: IdError) -> Self {
        AdapterError::Identifier {
            location: Box::new(location),
            message: error.to_string(),
        }
    }

    pub(crate) fn malformed_fact(location: SourceLocation, message: impl Into<String>) -> Self {
        AdapterError::MalformedFact {
            location: Box::new(location),
            message: message.into(),
        }
    }

    pub(crate) fn ambiguous_identity(
        location: SourceLocation,
        dimension: impl Into<String>,
    ) -> Self {
        AdapterError::AmbiguousIdentity {
            location: Box::new(location),
            dimension: dimension.into(),
        }
    }

    pub(crate) fn schema_drift(location: SourceLocation, expected: impl Into<String>) -> Self {
        AdapterError::SchemaDrift {
            location: Box::new(location),
            expected: expected.into(),
        }
    }

    pub(crate) fn type_mismatch(
        location: SourceLocation,
        expected: &'static str,
        found: impl Into<String>,
    ) -> Self {
        AdapterError::ValueTypeMismatch {
            location: Box::new(location),
            expected,
            found: found.into(),
        }
    }

    pub(crate) fn duplicate_fact_id(
        id: impl Into<String>,
        first: SourceLocation,
        second: SourceLocation,
    ) -> Self {
        AdapterError::DuplicateFactId {
            id: id.into(),
            first: Box::new(first),
            second: Box::new(second),
        }
    }

    pub(crate) fn canonical(location: SourceLocation, source: CanonicalError) -> Self {
        AdapterError::Canonical {
            location: Box::new(location),
            source,
        }
    }

    pub(crate) fn io(path: impl Into<String>, error: &std::io::Error) -> Self {
        AdapterError::Io {
            path: path.into(),
            message: error.to_string(),
        }
    }
}
