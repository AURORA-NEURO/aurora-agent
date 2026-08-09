//! Typed identifiers.
//!
//! The blueprint's invariant (40.05, and the MCP invariant list in 11.11) is that a benchmark
//! family, a parent world, a generated instance, an execution trial and a scored result are
//! never conflated. Distinct newtypes make conflation a compile error rather than a convention.

use crate::error::IdError;
use serde::{Deserialize, Serialize};
use std::fmt;

macro_rules! typed_id {
    ($(#[$meta:meta])* $name:ident, $kind:literal) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(try_from = "String", into = "String")]
        pub struct $name(String);

        impl $name {
            pub const KIND: &'static str = $kind;

            pub fn parse(value: impl Into<String>) -> Result<Self, IdError> {
                let value = value.into();
                if value.is_empty() {
                    return Err(IdError::Empty { kind: $kind });
                }
                if value.chars().any(|c| c.is_control()) {
                    return Err(IdError::ControlCharacter { kind: $kind, value });
                }
                Ok($name(value))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl From<$name> for String {
            fn from(value: $name) -> Self {
                value.0
            }
        }

        impl TryFrom<String> for $name {
            type Error = IdError;

            fn try_from(value: String) -> Result<Self, Self::Error> {
                $name::parse(value)
            }
        }
    };
}

typed_id!(
    /// Identifies an immutable world release.
    WorldId,
    "world"
);
typed_id!(
    /// Identifies a typed decision query contract.
    QueryId,
    "query"
);
typed_id!(
    /// Identifies a local evidence section within a world.
    FactId,
    "fact"
);
typed_id!(
    /// Identifies a typed factor within a world.
    FactorId,
    "factor"
);
typed_id!(
    /// Identifies a causal event in the world's event structure.
    EventId,
    "event"
);
typed_id!(
    /// Names a variable produced by a fact or a factor.
    VariableName,
    "variable"
);
typed_id!(
    /// Identifies one execution of the compiler or runtime.
    RunId,
    "run"
);
