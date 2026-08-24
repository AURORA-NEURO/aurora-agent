use thiserror::Error;

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("index key contains a tab or newline and cannot be stored: {0:?}")]
    UnsupportedKey(String),

    #[error("index value for key {0:?} contains a newline")]
    UnsupportedValue(String),

    #[error("corrupt index: {0}")]
    CorruptIndex(String),

    #[error("unsupported store schema: expected {expected:?}, got {actual:?}")]
    UnsupportedSchema {
        expected: &'static str,
        actual: String,
    },

    #[error("world is not a JSON object with facts, factors and events")]
    MalformedWorld,
}
