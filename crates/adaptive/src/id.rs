//! Identifiers that cannot be confused with one another.
//!
//! Blueprint 08.01 lists "parent/mutation clusters" among the scheduler's inputs, and every 08
//! module restates the invariant that a benchmark family, a parent task, a generated instance, an
//! execution trial and a scored result are never conflated.
//!
//! Here that invariant is not bookkeeping, it is the statistics. The entire contribution of this
//! crate is that trials are grouped by *parent* rather than counted individually; a parent key
//! accidentally populated with an instance key produces one cluster per trial, an intraclass
//! correlation of zero, a design effect of one, and a clustered interval identical to the naive
//! one. The failure is completely silent — the numbers look fine and every confidence statement
//! is wrong. Separate newtypes make that a compile error.

use crate::error::AdaptiveError;
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

            pub fn parse(value: impl Into<String>) -> Result<Self, AdaptiveError> {
                let value = value.into();
                if value.is_empty() {
                    return Err(AdaptiveError::EmptyId { kind: $kind });
                }
                if value.chars().any(|c| c.is_control()) {
                    return Err(AdaptiveError::ControlCharacterId { kind: $kind, value });
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
            type Error = AdaptiveError;

            fn try_from(value: String) -> Result<Self, Self::Error> {
                $name::parse(value)
            }
        }

        impl std::str::FromStr for $name {
            type Err = AdaptiveError;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                $name::parse(value)
            }
        }
    };
}

typed_id!(
    /// Names a capability whose success rate the panel is estimating.
    ///
    /// One posterior is maintained per capability. A cell that loads on several capabilities with
    /// uncertain weights (08.02, "Multi-dimensionality") is *not* representable: it must be
    /// assigned to exactly one capability by the caller.
    CapabilityId,
    "capability"
);

typed_id!(
    /// Names the audited parent world a generated instance descends from.
    ///
    /// This is the clustering key, and the only dependence structure this crate models.
    ParentId,
    "parent"
);

typed_id!(
    /// Names one generated instance.
    ///
    /// Distinct from its parent and from the trial that executes it. Used for deduplication and
    /// for deterministic tie-breaking during selection.
    InstanceId,
    "instance"
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_identifier_is_rejected_with_its_kind_named() {
        assert_eq!(
            ParentId::parse(""),
            Err(AdaptiveError::EmptyId { kind: "parent" })
        );
    }

    #[test]
    fn a_control_character_in_an_identifier_is_rejected() {
        assert!(matches!(
            InstanceId::parse("inst\u{0}01"),
            Err(AdaptiveError::ControlCharacterId { .. })
        ));
    }

    #[test]
    fn identifiers_round_trip_through_json_as_bare_strings() {
        let id = CapabilityId::parse("leakage-detection").unwrap();
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, "\"leakage-detection\"");
        assert_eq!(serde_json::from_str::<CapabilityId>(&json).unwrap(), id);
    }
}
