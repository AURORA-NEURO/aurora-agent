//! Hub-local identifiers and the hub's notion of order.
//!
//! `bioprism_ids` owns the identifiers of the *engine* — worlds, queries, runs. Those are the
//! identifiers of things that were computed. The hub additionally needs identifiers for things
//! that were *asserted by a person*: a submission, a submitter, a board. They are separate types
//! because conflating "this run happened" with "this person claims this run happened" is the
//! specific mistake a public hub exists to prevent.
//!
//! Validation matches `bioprism_ids`: non-empty, no control characters. The hub is not an
//! identity provider and cannot check that a `SubmitterId` corresponds to a real person, an
//! institution, or a held key. It only guarantees that whatever string was presented is the same
//! string on every subsequent record, so an audit can follow it.

use bioprism_ids::IdError;
use serde::{Deserialize, Serialize};
use std::fmt;

macro_rules! hub_id {
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

hub_id!(
    /// Identifies one public submission. Immutable: a corrected artifact is a new submission that
    /// supersedes the old one, because 34.15 requires that "corrections preserve prior versions".
    SubmissionId,
    "submission"
);
hub_id!(
    /// Identifies whoever presented a submission. Opaque to the hub; see the module note on why
    /// this is not an authenticated principal.
    SubmitterId,
    "submitter"
);
hub_id!(
    /// Identifies a leaderboard. A board is defined by its comparability conditions, so its
    /// identifier is a label for humans, not the thing that makes entries comparable.
    BoardId,
    "board"
);

/// A monotonically increasing sequence number supplied by the hub operator.
///
/// Not a wall clock. A submitter-supplied timestamp is an assertion, and the one property the
/// audit trail actually needs — that events are totally ordered and cannot be backdated — is
/// exactly what a wall clock fails to provide across machines. The operator advances the epoch;
/// [`crate::moderation::ModerationLedger`] refuses any event that would move it backwards.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct Epoch(pub u64);

impl Epoch {
    pub const ORIGIN: Epoch = Epoch(0);

    pub fn next(self) -> Epoch {
        Epoch(self.0.saturating_add(1))
    }

    pub fn get(self) -> u64 {
        self.0
    }
}

impl fmt::Display for Epoch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}
