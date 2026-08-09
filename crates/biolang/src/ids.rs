//! Typed identifiers for the IR family.
//!
//! The rule is `bioprism-ids`': a benchmark family, a parent world, a generated instance and a
//! scored result are never conflated, and distinct newtypes make conflation a compile error. The
//! identifiers already published elsewhere are *reused*, not redefined — [`bioprism_ids::WorldId`]
//! and [`bioprism_ids::RunId`] for worlds and runs, `bioprism-bioir`'s
//! [`bioprism_bioir::SpecimenId`], [`bioprism_bioir::EvidenceId`], [`bioprism_bioir::CohortId`] and
//! [`bioprism_bioir::LensId`] for material, evidence, cohorts and lenses. What is added here is only
//! what §25 names and nothing owns yet.

use crate::error::IrError;
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

            pub fn parse(value: impl Into<String>) -> Result<Self, IrError> {
                let value = value.into();
                if value.is_empty() {
                    return Err(IrError::MalformedId {
                        field: $kind.to_string(),
                        kind: $kind.to_string(),
                        detail: "identifier is empty".to_string(),
                    });
                }
                if value.chars().any(char::is_control) {
                    return Err(IrError::MalformedId {
                        field: $kind.to_string(),
                        kind: $kind.to_string(),
                        detail: "identifier contains a control character".to_string(),
                    });
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
            type Error = IrError;

            fn try_from(value: String) -> Result<Self, Self::Error> {
                $name::parse(value)
            }
        }
    };
}

typed_id!(
    /// A content-addressed asset inside a BioWorld (25.01).
    AssetId,
    "asset"
);
typed_id!(
    /// One forkable state (25.02).
    StateId,
    "state"
);
typed_id!(
    /// One longitudinal sequence of states (25.09).
    WorldlineId,
    "worldline"
);
typed_id!(
    /// An entry in a world's action catalog (25.06).
    ActionId,
    "action"
);
typed_id!(
    /// A falsifiable biological contract (25.07).
    FbcId,
    "fbc"
);
typed_id!(
    /// An evaluated system: model, pipeline or agent (25.14).
    SystemId,
    "system"
);
typed_id!(
    /// One component inside an evaluated system (25.14).
    ComponentId,
    "component"
);
typed_id!(
    /// A single scientific act (25.15).
    ActId,
    "act"
);
typed_id!(
    /// A packaged multi-agent workflow (25.17).
    MoleculeId,
    "molecule"
);
typed_id!(
    /// A mutation program (25.19).
    MutationId,
    "mutation"
);
typed_id!(
    /// An obligation a contract or capsule tracks (25.07, 25.16).
    ObligationId,
    "obligation"
);
