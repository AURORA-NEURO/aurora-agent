//! Building worldlines from FIBER evidence.
//!
//! Blueprint 30.02, read against the world model of 43.04. A [`Fact`] is a value that holds
//! inside a scope; a worldline is one subject's ordered stream. The join between them is the
//! `subject` scope dimension, and this module is deliberately strict about it.
//!
//! # Why a cohort scope is refused
//!
//! `bioprism_scope::ScopeValue` can bind a dimension to an exact value, to a set, or to a time
//! window. Only the exact binding names a subject. A fact scoped to `subject: [S1, S2]` is a
//! statement about a cohort, and attaching it to a worldline would assert an observation about
//! someone who may not have been observed. That is refused rather than resolved.
//!
//! # Not implemented
//!
//! Specimen and aliquot lineage, treatment intervals, and the `provenance` and `tags` fields
//! beyond the baseline marker. Facts whose `provides` is not [`TIMEPOINT_VARIABLE`] are ignored
//! rather than rejected, so a mixed world can be handed in whole.

use crate::error::OncoError;
use crate::worldline::{SubjectRef, Timepoint, TumourWorldline};
use bioprism_ids::VariableName;
use bioprism_scope::{ScopeKey, ScopeValue};
use bioprism_world::Fact;

/// The scope dimension that names a subject.
pub const SUBJECT_DIMENSION: &str = "subject";

/// The variable a timepoint fact provides.
pub const TIMEPOINT_VARIABLE: &str = "onco.timepoint";

/// The tag marking the fact that anchors the worldline baseline.
pub const BASELINE_TAG: &str = "onco.baseline";

/// [`TIMEPOINT_VARIABLE`] as a typed identifier.
pub fn timepoint_variable() -> VariableName {
    VariableName::parse(TIMEPOINT_VARIABLE)
        .expect("the constant is non-empty and free of control characters")
}

/// The single subject a scope names.
pub fn subject_from_scope(scope: &ScopeKey) -> Result<SubjectRef, OncoError> {
    let found = match scope.get(SUBJECT_DIMENSION) {
        Some(ScopeValue::Exact(value)) => return SubjectRef::new(value.clone()),
        Some(other) => other.describe(),
        None => "unbound".to_string(),
    };
    Err(OncoError::SubjectNotSingular {
        dimension: SUBJECT_DIMENSION,
        found,
    })
}

/// Decode one timepoint fact.
///
/// The payload is the serialised form of [`Timepoint`], so decoding runs the clock-order check
/// in [`Timepoint::new`]: a fact whose four stamps disagree is refused at the boundary rather
/// than entering a worldline and corrupting every later interval.
pub fn timepoint_from_fact(fact: &Fact) -> Result<Timepoint, OncoError> {
    serde_json::from_value(fact.value.clone()).map_err(|error| OncoError::MalformedObservation {
        fact: fact.id.to_string(),
        message: error.to_string(),
    })
}

/// Assemble one subject's worldline from facts.
///
/// Facts providing other variables are skipped. Exactly one of the remaining facts must carry
/// [`BASELINE_TAG`]; zero leaves the worldline unanchored and more than one leaves it ambiguous,
/// and neither is silently resolved.
pub fn worldline_from_facts(facts: &[Fact]) -> Result<TumourWorldline, OncoError> {
    let variable = timepoint_variable();
    let relevant: Vec<&Fact> = facts
        .iter()
        .filter(|fact| fact.provides == variable)
        .collect();

    let mut subject: Option<SubjectRef> = None;
    for fact in &relevant {
        let found = subject_from_scope(&fact.scope)?;
        match &subject {
            None => subject = Some(found),
            Some(existing) if existing == &found => {}
            Some(existing) => {
                return Err(OncoError::MixedSubjects {
                    first: existing.to_string(),
                    second: found.to_string(),
                })
            }
        }
    }

    let baselines: Vec<&&Fact> = relevant
        .iter()
        .filter(|fact| fact.has_tag(BASELINE_TAG))
        .collect();
    if baselines.len() != 1 {
        return Err(OncoError::BaselineTagAmbiguous {
            tag: BASELINE_TAG,
            found: baselines.len(),
        });
    }

    let baseline_fact = *baselines[0];
    let subject = subject.ok_or(OncoError::BaselineTagAmbiguous {
        tag: BASELINE_TAG,
        found: 0,
    })?;
    let mut worldline =
        TumourWorldline::new(subject, timepoint_from_fact(baseline_fact)?);

    for fact in relevant {
        if std::ptr::eq(fact, baseline_fact) {
            continue;
        }
        worldline.push(timepoint_from_fact(fact)?)?;
    }
    Ok(worldline)
}
