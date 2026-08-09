//! The five units of analysis.
//!
//! Blueprint 24.01 promises to turn "one biological research workflow into thousands of
//! forkable, falsifiable microbenchmarks", and 24.14 expects a laboratory to replay the exact
//! decision where a workflow failed. Both promises die the moment a benchmark family, the
//! parent world it draws from, a generated instance, one execution trial of that instance and
//! the scored result of that trial are allowed to share a type. A number attributed to the
//! wrong unit is worse than a missing number, because it looks like evidence.
//!
//! So the five are five types. Passing a [`FamilyId`] where a [`WorldId`] is required does not
//! fail a validator at runtime — it fails to compile. [`WorldId`] itself is not redefined here;
//! it belongs to `bioprism-ids` and is re-exported so that the parent-world unit has exactly
//! one spelling across the workspace.
//!
//! [`ProvenanceChain`] then enforces the *order*: a result cannot exist without the trial that
//! produced it, which cannot exist without the instance it ran, which cannot exist without the
//! world it was generated from. A score whose chain is incomplete is unattributable, and
//! [`ProvenanceChain::finish`] refuses to produce one.

use crate::error::ProvenanceError;
use serde::{Deserialize, Serialize};
use std::fmt;

pub use bioprism_ids::WorldId;

macro_rules! unit_id {
    ($(#[$meta:meta])* $name:ident, $unit:literal) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(try_from = "String", into = "String")]
        pub struct $name(String);

        impl $name {
            /// The unit this identifier names, for diagnostics and wire tagging.
            pub const UNIT: &'static str = $unit;

            /// Rejects empty and control-bearing identifiers, matching `bioprism-ids`
            /// so that the two families of identifier cannot disagree about what is valid.
            pub fn parse(value: impl Into<String>) -> Result<Self, ProvenanceError> {
                let value = value.into();
                if value.is_empty() {
                    return Err(ProvenanceError::EmptyIdentifier { unit: $unit });
                }
                if value.chars().any(|c| c.is_control()) {
                    return Err(ProvenanceError::ControlCharacter { unit: $unit });
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
            type Error = ProvenanceError;

            fn try_from(value: String) -> Result<Self, Self::Error> {
                $name::parse(value)
            }
        }
    };
}

unit_id!(
    /// A benchmark family: the *intent* of a set of related instances, which is never itself
    /// executed and therefore never carries a score.
    FamilyId,
    "benchmark_family"
);
unit_id!(
    /// One generated instance: a concrete task drawn from a family against a parent world.
    /// Aggregating scores across instances of different parent worlds is a family-level claim,
    /// not an instance-level one.
    InstanceId,
    "generated_instance"
);
unit_id!(
    /// One execution trial of one instance by one system. Repeated trials of the same instance
    /// are the only honest source of variance, so they must remain individually addressable.
    TrialId,
    "execution_trial"
);
unit_id!(
    /// One scored result of one trial under one oracle version. Rescoring a trial with a newer
    /// oracle produces a *different* result, not an updated one.
    ResultId,
    "scored_result"
);

/// Which of the five units an object belongs to, for contexts that must be dynamic (wire
/// formats, registries, diagnostics). Type-level separation is the primary defence; this enum
/// is the escape hatch, and it is deliberately closed so that a sixth unit cannot be smuggled
/// in as a string.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "unit", content = "id")]
pub enum UnitOfAnalysis {
    BenchmarkFamily(FamilyId),
    ParentWorld(WorldId),
    GeneratedInstance(InstanceId),
    ExecutionTrial(TrialId),
    ScoredResult(ResultId),
}

impl UnitOfAnalysis {
    pub fn unit_name(&self) -> &'static str {
        match self {
            UnitOfAnalysis::BenchmarkFamily(_) => FamilyId::UNIT,
            UnitOfAnalysis::ParentWorld(_) => "parent_world",
            UnitOfAnalysis::GeneratedInstance(_) => InstanceId::UNIT,
            UnitOfAnalysis::ExecutionTrial(_) => TrialId::UNIT,
            UnitOfAnalysis::ScoredResult(_) => ResultId::UNIT,
        }
    }

    /// Depth in the containment order, family being shallowest. Two units at the same depth are
    /// never comparable, and a deeper unit never justifies a claim about a shallower one on its
    /// own — that is an aggregation, and aggregation is a separate, declared step.
    pub fn depth(&self) -> u8 {
        match self {
            UnitOfAnalysis::BenchmarkFamily(_) => 0,
            UnitOfAnalysis::ParentWorld(_) => 1,
            UnitOfAnalysis::GeneratedInstance(_) => 2,
            UnitOfAnalysis::ExecutionTrial(_) => 3,
            UnitOfAnalysis::ScoredResult(_) => 4,
        }
    }
}

/// A complete attribution path from family to scored result.
///
/// Constructed only through [`ProvenanceChain`], so an instance of this type is proof that the
/// chain was assembled in order and none of its links were invented.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Provenance {
    pub family: FamilyId,
    pub parent_world: WorldId,
    pub instance: InstanceId,
    pub trial: TrialId,
    pub result: ResultId,
}

impl Provenance {
    /// The five links in containment order. Useful for rendering a breadcrumb without
    /// hand-writing the order at every call site and getting it wrong once.
    pub fn units(&self) -> [UnitOfAnalysis; 5] {
        [
            UnitOfAnalysis::BenchmarkFamily(self.family.clone()),
            UnitOfAnalysis::ParentWorld(self.parent_world.clone()),
            UnitOfAnalysis::GeneratedInstance(self.instance.clone()),
            UnitOfAnalysis::ExecutionTrial(self.trial.clone()),
            UnitOfAnalysis::ScoredResult(self.result.clone()),
        ]
    }
}

/// Builds a [`Provenance`] one link at a time, refusing links that skip a level.
///
/// The builder is not a formality. In a system that generates instances by mutation and runs
/// them repeatedly across architectures, the cheapest bug is a result attached to the wrong
/// trial, and the second cheapest is a trial attached to a family instead of an instance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProvenanceChain {
    family: FamilyId,
    parent_world: WorldId,
    instance: Option<InstanceId>,
    trial: Option<TrialId>,
    result: Option<ResultId>,
}

impl ProvenanceChain {
    /// A chain begins with a family and the parent world the instances will be drawn from.
    /// Neither is optional: a world with no family is an artifact, not a benchmark, and a
    /// family with no world generates nothing.
    pub fn new(family: FamilyId, parent_world: WorldId) -> Self {
        ProvenanceChain {
            family,
            parent_world,
            instance: None,
            trial: None,
            result: None,
        }
    }

    pub fn instance(mut self, instance: InstanceId) -> Self {
        self.instance = Some(instance);
        self
    }

    /// Refuses when no instance is bound: a trial executes an instance, and a trial with no
    /// instance is an execution of nothing in particular.
    pub fn trial(mut self, trial: TrialId) -> Result<Self, ProvenanceError> {
        if self.instance.is_none() {
            return Err(ProvenanceError::TrialWithoutInstance);
        }
        self.trial = Some(trial);
        Ok(self)
    }

    /// Refuses when no trial is bound. A score that names no execution cannot be re-run,
    /// and an unreproducible score is the failure mode section 24 exists to prevent.
    pub fn result(mut self, result: ResultId) -> Result<Self, ProvenanceError> {
        if self.trial.is_none() {
            return Err(ProvenanceError::ResultWithoutTrial);
        }
        self.result = Some(result);
        Ok(self)
    }

    pub fn finish(self) -> Result<Provenance, ProvenanceError> {
        let instance = self
            .instance
            .ok_or(ProvenanceError::Incomplete { level: "generated_instance" })?;
        let trial = self
            .trial
            .ok_or(ProvenanceError::Incomplete { level: "execution_trial" })?;
        let result = self
            .result
            .ok_or(ProvenanceError::Incomplete { level: "scored_result" })?;
        Ok(Provenance {
            family: self.family,
            parent_world: self.parent_world,
            instance,
            trial,
            result,
        })
    }
}

/// A generated instance cannot be passed where a parent world is required.
///
/// ```compile_fail
/// use bioprism_foundation::unit::{InstanceId, ProvenanceChain, FamilyId};
/// let family = FamilyId::parse("fam:glioma:response").unwrap();
/// let instance = InstanceId::parse("inst:0001").unwrap();
/// let _ = ProvenanceChain::new(family, instance);
/// ```
///
/// A scored result cannot be passed where an execution trial is required.
///
/// ```compile_fail
/// use bioprism_foundation::unit::{FamilyId, ProvenanceChain, ResultId, InstanceId, WorldId};
/// let chain = ProvenanceChain::new(
///     FamilyId::parse("fam:x").unwrap(),
///     WorldId::parse("world:x").unwrap(),
/// )
/// .instance(InstanceId::parse("inst:x").unwrap());
/// let _ = chain.trial(ResultId::parse("res:x").unwrap());
/// ```
#[cfg(doctest)]
pub struct UnitsAreNotInterchangeable;

#[cfg(test)]
mod tests {
    use super::*;

    fn family() -> FamilyId {
        FamilyId::parse("fam:glioma:response").unwrap()
    }

    fn world() -> WorldId {
        WorldId::parse("world:glioma:0001").unwrap()
    }

    #[test]
    fn a_trial_cannot_be_attached_before_an_instance_exists() {
        let err = ProvenanceChain::new(family(), world())
            .trial(TrialId::parse("trial:1").unwrap())
            .unwrap_err();
        assert_eq!(err, ProvenanceError::TrialWithoutInstance);
    }

    #[test]
    fn a_scored_result_cannot_be_attached_before_a_trial_exists() {
        let err = ProvenanceChain::new(family(), world())
            .instance(InstanceId::parse("inst:1").unwrap())
            .result(ResultId::parse("res:1").unwrap())
            .unwrap_err();
        assert_eq!(err, ProvenanceError::ResultWithoutTrial);
    }

    #[test]
    fn an_incomplete_chain_refuses_to_produce_an_attributable_provenance() {
        let err = ProvenanceChain::new(family(), world())
            .instance(InstanceId::parse("inst:1").unwrap())
            .finish()
            .unwrap_err();
        assert_eq!(err, ProvenanceError::Incomplete { level: "execution_trial" });
    }

    #[test]
    fn a_complete_chain_orders_its_five_units_by_containment_depth() {
        let provenance = ProvenanceChain::new(family(), world())
            .instance(InstanceId::parse("inst:1").unwrap())
            .trial(TrialId::parse("trial:1").unwrap())
            .unwrap()
            .result(ResultId::parse("res:1").unwrap())
            .unwrap()
            .finish()
            .unwrap();
        let depths: Vec<u8> = provenance.units().iter().map(UnitOfAnalysis::depth).collect();
        assert_eq!(depths, vec![0, 1, 2, 3, 4]);
    }

    #[test]
    fn two_units_sharing_a_string_are_still_distinct_on_the_wire() {
        let family = UnitOfAnalysis::BenchmarkFamily(FamilyId::parse("x").unwrap());
        let instance = UnitOfAnalysis::GeneratedInstance(InstanceId::parse("x").unwrap());
        assert_ne!(
            serde_json::to_string(&family).unwrap(),
            serde_json::to_string(&instance).unwrap()
        );
        assert_ne!(family.unit_name(), instance.unit_name());
    }

    #[test]
    fn an_identifier_with_a_control_character_is_rejected_with_its_unit_named() {
        let err = TrialId::parse("trial:\u{7}1").unwrap_err();
        assert_eq!(err, ProvenanceError::ControlCharacter { unit: "execution_trial" });
    }

    #[test]
    fn provenance_round_trips_through_json_without_losing_unit_tags() {
        let provenance = ProvenanceChain::new(family(), world())
            .instance(InstanceId::parse("inst:1").unwrap())
            .trial(TrialId::parse("trial:1").unwrap())
            .unwrap()
            .result(ResultId::parse("res:1").unwrap())
            .unwrap()
            .finish()
            .unwrap();
        let encoded = serde_json::to_string(&provenance).unwrap();
        let decoded: Provenance = serde_json::from_str(&encoded).unwrap();
        assert_eq!(provenance, decoded);
    }
}
