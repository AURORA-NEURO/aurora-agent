//! Typed identifiers for biological subjects, material, lenses, cohorts and evidence.
//!
//! Blueprint 39.05 protects "subject, lesion, region, specimen, aliquot, assay, artifact, and
//! model-system identity" as a non-compressible class. Conflating a subject with the specimen
//! taken from them is the single most common way an evaluation leaks identity between splits,
//! so the distinction is carried in the type system rather than in a naming convention.
//!
//! These mirror the construction of [`bioprism_ids::WorldId`] and friends and reuse
//! [`bioprism_ids::IdError`]. They are defined here rather than in `bioprism-ids` because
//! `bioprism-ids` sits at the bottom of the dependency graph and is scoped to compiler
//! identity, not to biological material.

use bioprism_ids::IdError;
use serde::{Deserialize, Serialize};
use std::fmt;

macro_rules! bio_id {
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

bio_id!(
    /// Identifies the biological source entity: a participant, an animal, a donor, a cell line.
    SubjectId,
    "subject"
);
bio_id!(
    /// Identifies one piece of biological material.
    ///
    /// Aliquots are not a separate type. An aliquot is a specimen whose [`crate::Origin`] is a
    /// draw from a parent, which is exactly what makes lineage queries uniform: 25.04 requires
    /// "derived data point to the exact material ancestor", and a split type would let a caller
    /// hold an ancestor reference that the lineage walk does not understand.
    SpecimenId,
    "specimen"
);
bio_id!(
    /// Identifies a measurement lens (25.05). Lens *version* is a separate field because
    /// 25.05 makes processing versions part of lens identity without making them part of the id.
    LensId,
    "lens"
);
bio_id!(
    /// Identifies a cohort definition (25.13).
    CohortId,
    "cohort"
);
bio_id!(
    /// Identifies one row of a cohort: one unit-of-analysis record at one index date.
    ///
    /// A subject contributing repeated measures owns several of these, which is the structure
    /// that makes the repeated-measures leakage check in [`crate::cohort`] expressible at all.
    ObservationId,
    "observation"
);
bio_id!(
    /// Identifies an atomic evidence unit (25.11).
    EvidenceId,
    "evidence"
);
