use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq, Clone)]
pub enum TimeError {
    #[error("malformed RFC 3339 timestamp: {0:?}")]
    Malformed(String),

    #[error("timestamp field out of range: {0:?}")]
    OutOfRange(String),
}

#[derive(Debug, Error, PartialEq, Eq, Clone)]
pub enum ScopeError {
    #[error("scope dimension {dimension:?} is not a JSON object entry that can be typed: {value}")]
    UntypableValue { dimension: String, value: String },

    #[error("scope must be a JSON object, got {0}")]
    NotAnObject(String),

    #[error(transparent)]
    Time(#[from] TimeError),
}
