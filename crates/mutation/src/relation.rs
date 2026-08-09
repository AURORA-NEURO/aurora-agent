//! Metamorphic relations and their executable postconditions.
//!
//! Blueprint 03.08 and 32: a mutation declares *what the oracle should do* as a result of the
//! transformation, and that declaration is checked by running the oracle. A mutation whose
//! postcondition fails is a defect in the mutation, not a new benchmark instance, and is rejected.
//!
//! This is the difference between a mutation engine and a paraphrase generator. "Rename every
//! subject" is only a valid instance if the verdict is genuinely invariant under renaming; if it
//! is not, the transformation broke something and the engine must say so rather than ship it.

use bioprism_section::{OracleStatus, OracleVerdict};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mechanism {
    Identity,
    Site,
    Temporal,
    Preprocessing,
}

impl Mechanism {
    pub const ALL: [Mechanism; 4] = [
        Mechanism::Identity,
        Mechanism::Site,
        Mechanism::Temporal,
        Mechanism::Preprocessing,
    ];

    pub fn witness_kind(self) -> &'static str {
        match self {
            Mechanism::Identity => "identity_leakage",
            Mechanism::Site => "site_leakage",
            Mechanism::Temporal => "temporal_leakage",
            Mechanism::Preprocessing => "preprocessing_leakage",
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Mechanism::Identity => "identity",
            Mechanism::Site => "site",
            Mechanism::Temporal => "temporal",
            Mechanism::Preprocessing => "preprocessing",
        }
    }
}

/// What the oracle must do after a transformation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "relation", rename_all = "snake_case")]
pub enum Relation {
    /// Status and witness set must be exactly unchanged. The strongest relation, and the one that
    /// catches a transformation that quietly damaged the world.
    PreservesVerdict,
    /// This witness must disappear and no other may change.
    RemovesWitness { kind: String },
    /// This witness must appear and no other may change.
    AddsWitness { kind: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum PostconditionResult {
    Held,
    Violated { expected: String, observed: String },
}

impl PostconditionResult {
    pub fn held(&self) -> bool {
        matches!(self, PostconditionResult::Held)
    }
}

fn kinds(verdict: &OracleVerdict) -> BTreeSet<String> {
    verdict.witness_kinds().into_iter().map(str::to_string).collect()
}

fn describe(status: OracleStatus, set: &BTreeSet<String>) -> String {
    if set.is_empty() {
        format!("{} with no witnesses", status.as_str())
    } else {
        format!(
            "{} with {}",
            status.as_str(),
            set.iter().cloned().collect::<Vec<_>>().join(", ")
        )
    }
}

impl Relation {
    /// Runs the postcondition against the before and after verdicts.
    pub fn check(&self, before: &OracleVerdict, after: &OracleVerdict) -> PostconditionResult {
        let before_kinds = kinds(before);
        let after_kinds = kinds(after);

        match self {
            Relation::PreservesVerdict => {
                if before.status == after.status && before_kinds == after_kinds {
                    PostconditionResult::Held
                } else {
                    PostconditionResult::Violated {
                        expected: describe(before.status, &before_kinds),
                        observed: describe(after.status, &after_kinds),
                    }
                }
            }
            Relation::RemovesWitness { kind } => {
                let mut expected_kinds = before_kinds.clone();
                let was_present = expected_kinds.remove(kind);
                let expected_status = if expected_kinds.is_empty() {
                    OracleStatus::Valid
                } else {
                    OracleStatus::Invalid
                };
                if was_present && after_kinds == expected_kinds && after.status == expected_status {
                    PostconditionResult::Held
                } else {
                    PostconditionResult::Violated {
                        expected: describe(expected_status, &expected_kinds),
                        observed: describe(after.status, &after_kinds),
                    }
                }
            }
            Relation::AddsWitness { kind } => {
                let mut expected_kinds = before_kinds.clone();
                let was_absent = expected_kinds.insert(kind.clone());
                if was_absent && after_kinds == expected_kinds && after.status == OracleStatus::Invalid
                {
                    PostconditionResult::Held
                } else {
                    PostconditionResult::Violated {
                        expected: describe(OracleStatus::Invalid, &expected_kinds),
                        observed: describe(after.status, &after_kinds),
                    }
                }
            }
        }
    }
}
