//! BioMutation IR — blueprint 25.19.
//!
//! A mutation program: what it applies to, what seed it ran with, what it changed, what semantic
//! relation it claims to preserve or to break, what it does to the oracle, what validates it, how
//! risky it is, and which generator version produced it.
//!
//! # Where the IR and the implementing crate disagree
//!
//! `bioprism-mutation` is the running implementation. Its `Mutation` carries an id, a `MutationKind`
//! and a `Relation`, and its `Family` carries lineage and effective diversity. Four of 25.19's nine
//! required field groups have no field on the source object:
//!
//! - **`seed`.** `bioprism_mutation::apply` is deterministic given a world and a mutation, so the
//!   crate needs no seed; 25.19 requires one because a *generator* that samples from a family does.
//!   The seed lives in whatever drove the generator, not in the mutation, so a projection from a
//!   `Mutation` alone cannot fill it.
//! - **`generator version`.** Same shape of gap: the crate version is a property of the run.
//! - **`risk`.** Not modelled anywhere in the crate.
//! - **`oracle changes`.** The crate's `Relation` says whether the oracle verdict should be
//!   preserved or should change, and `Relation::check` verifies it against a before/after pair —
//!   but the *identity* of the oracle that changed is not recorded on the mutation.
//!
//! None of these are widened into optional fields with plausible defaults. A projection declares
//! them as [`crate::projection::ProjectionGap`]s, which is the difference between an IR that is
//! honestly partial and one that is quietly wrong.

use crate::error::MutationIrError;
use crate::ids::MutationId;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fmt;

/// What the mutation claims about meaning. Mirrors `bioprism_mutation::Relation`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SemanticRelation {
    /// The answer must not change.
    Preserving,
    /// The answer must change, in a declared direction.
    Changing,
    /// The instance must become unanswerable.
    Invalidating,
}

impl SemanticRelation {
    pub fn as_str(self) -> &'static str {
        match self {
            SemanticRelation::Preserving => "preserving",
            SemanticRelation::Changing => "changing",
            SemanticRelation::Invalidating => "invalidating",
        }
    }

    /// True when the oracle must move for the mutation to be admitted.
    pub fn requires_oracle_change(self) -> bool {
        !matches!(self, SemanticRelation::Preserving)
    }
}

impl fmt::Display for SemanticRelation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// What kind of object the mutation transforms. 25.19: "worlds, assays, artifacts, timelines, tasks".
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransformationTarget {
    World,
    Assay,
    Artifact,
    Timeline,
    Task,
}

/// One change the program makes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Transformation {
    pub target: TransformationTarget,
    /// What is changed, as a path into the object.
    pub locator: String,
    pub description: String,
}

/// How the mutation was seeded. There is no `Unseeded` that reads as a value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "seeding", rename_all = "snake_case")]
pub enum SeedDeclaration {
    /// The generator ran with this seed and is replayable from it.
    Seeded { seed: u64 },
    /// The transformation is a pure function of its input and consumed no randomness.
    Deterministic,
}

/// How much the mutation could plausibly break. 25.19 requires a risk field and names no scale.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Risk {
    /// Syntactic: whitespace, ordering, identifier renaming.
    Cosmetic,
    /// Changes what the instance asks without changing whether it is answerable.
    Substantive,
    /// May make the instance biologically incoherent, and must be validated for it.
    Biological,
}

/// A mutation program.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MutationProgram {
    pub mutation_id: MutationId,
    /// The parent this descends from. 25.19: "Every descendant retains parent lineage."
    pub parent: Option<String>,
    /// What parents this program can be applied to, in prose.
    pub applicability: String,
    pub seed: SeedDeclaration,
    pub transformations: Vec<Transformation>,
    pub relation: SemanticRelation,
    /// Oracles whose verdict this mutation is expected to move.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub oracle_changes: BTreeSet<String>,
    /// Checks that must pass before the mutant is admitted.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub validations: Vec<String>,
    pub risk: Risk,
    pub generator_version: String,
}

impl MutationProgram {
    pub fn validate(&self) -> Result<(), MutationIrError> {
        let mutation = self.mutation_id.to_string();

        if self.parent.as_ref().is_none_or(|parent| parent.trim().is_empty()) {
            return Err(MutationIrError::LineageBroken { mutation });
        }

        if self.relation.requires_oracle_change() && self.oracle_changes.is_empty() {
            return Err(MutationIrError::SemanticChangeWithoutOracleUpdate {
                mutation,
                relation: self.relation.to_string(),
            });
        }

        if !self.relation.requires_oracle_change() {
            if let Some(oracle) = self.oracle_changes.iter().next() {
                return Err(MutationIrError::PreservingMutationChangesOracle {
                    mutation,
                    oracle: oracle.clone(),
                });
            }
        }

        if self.generator_version.trim().is_empty() {
            return Err(MutationIrError::UnseededGenerator { mutation });
        }

        Ok(())
    }

    /// The lineage chain this mutation asserts, nearest parent first.
    ///
    /// One link deep, because that is what a single program carries; assembling the full chain needs
    /// the family, which `bioprism-mutation` owns.
    pub fn lineage(&self) -> Vec<&str> {
        self.parent.as_deref().into_iter().collect()
    }
}
