use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq, Clone)]
pub enum CanonicalError {
    #[error("serialization failed during canonicalization: {0}")]
    Serialization(String),

    #[error("non-finite number cannot be canonically serialized: {0}")]
    NonFiniteNumber(String),

    #[error("unrepresentable number: {0}")]
    UnrepresentableNumber(String),
}

#[derive(Debug, Error, PartialEq, Eq, Clone)]
pub enum IdError {
    #[error("empty identifier for {kind}")]
    Empty { kind: &'static str },

    #[error("identifier for {kind} contains a control character: {value:?}")]
    ControlCharacter { kind: &'static str, value: String },

    #[error("malformed content hash: expected 64 lowercase hex characters, got {0:?}")]
    MalformedContentHash(String),
}
