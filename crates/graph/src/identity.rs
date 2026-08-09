//! Stable handles for objects the Decision Section leaves anonymous.
//!
//! Facts, factors and events arrive with ids. Obligations, oracle witnesses and refinement
//! options do not — 43.25 models them as positional lists. A view still has to name them, because
//! 43.01's invariant is that "every projection is reversible to stable source handles", and a
//! deep link into an obligation that has no handle is not reversible.
//!
//! The naming rule is therefore positional and prefixed: position within the section's list plus
//! the variant's discriminator. It is stable for a fixed section — which is all that is needed,
//! since a view is bound to one section digest — and it is deliberately *not* a hash, so a reader
//! comparing a view against the section it came from can find the referent by counting.

use bioprism_section::{LeakageWitness, UnresolvedObligation};

pub const DECISION_PREFIX: &str = "decision:";
pub const VARIABLE_PREFIX: &str = "variable:";
pub const SOURCE_PREFIX: &str = "source:";
pub const ORACLE_PREFIX: &str = "oracle:";
pub const OBLIGATION_PREFIX: &str = "obligation:";
pub const CONFLICT_PREFIX: &str = "conflict:";
pub const REFINEMENT_PREFIX: &str = "refinement:";

/// The root node of a projected region: the Decision Section itself, named by its query.
pub fn decision_node_id(query_id: &str) -> String {
    format!("{DECISION_PREFIX}{query_id}")
}

pub fn variable_node_id(variable: &str) -> String {
    format!("{VARIABLE_PREFIX}{variable}")
}

/// A provenance handle echoed from an evidence capsule, kept verbatim after the prefix so the
/// original string survives the round trip.
pub fn source_node_id(handle: &str) -> String {
    format!("{SOURCE_PREFIX}{handle}")
}

pub fn oracle_node_id(oracle_kind: &str) -> String {
    format!("{ORACLE_PREFIX}{oracle_kind}")
}

pub fn obligation_id(index: usize, obligation: &UnresolvedObligation) -> String {
    format!("{OBLIGATION_PREFIX}{index}:{}", obligation_kind(obligation))
}

pub fn conflict_id(index: usize, witness: &LeakageWitness) -> String {
    format!("{CONFLICT_PREFIX}{index}:{}", witness.kind())
}

pub fn refinement_id(index: usize, action: &str) -> String {
    format!("{REFINEMENT_PREFIX}{index}:{action}")
}

/// The wire discriminator of an obligation variant, matching its serde tag.
pub fn obligation_kind(obligation: &UnresolvedObligation) -> &'static str {
    match obligation {
        UnresolvedObligation::InaccessibleAtCut { .. } => "inaccessible_at_cut",
        UnresolvedObligation::Obstructed { .. } => "obstructed",
        UnresolvedObligation::PolicyBlocked { .. } => "policy_blocked",
    }
}
