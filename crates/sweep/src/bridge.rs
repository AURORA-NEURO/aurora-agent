//! Borrowing someone else's fork runtime without borrowing their claims.
//!
//! Implements blueprint 05.11 (Shepherd and External Fork-Runtime Integration). `bioprism-runtime`
//! owns 05.05's fork/replay against its own WorldTape; this module is what happens when the
//! efficient substrate belongs to somebody else.
//!
//! # Role separation is a partition, and partitions can be checked
//!
//! 05.11: "The external runtime owns efficient process/environment state and branch operations.
//! PRISM owns which decision becomes a cell, oracle semantics, mutation lineage, adaptive
//! evaluation, and result attestation."
//!
//! [`Concern`] enumerates both halves and [`Concern::owner`] assigns each one. The value of writing
//! it down is that the assignment becomes testable: [`owned_by_prism`] must never include a
//! substrate concern, because a system that lets the external runtime decide which decision becomes
//! a cell has outsourced the thing it exists to do. The test is trivial to write and would not have
//! been written at all if the partition lived in a paragraph.
//!
//! # Falling back lowers the declaration
//!
//! 05.11: "When the integration is unavailable, PRISM reconstructs a world state in the local
//! executor and **lowers the reproducibility declaration**."
//!
//! [`Bridge::resolve`] returns both the execution route and the resulting
//! [`crate::fidelity::Declaration`], and the fallback route can only lower it — [`Declaration`] has
//! no raising operation at all. A caller cannot obtain a local-fallback route that still declares
//! the fidelity the external route would have had.
//!
//! # Merges are off unless the task is about merging
//!
//! 05.11: "merges are generally disabled during evaluation unless the task explicitly tests them".
//! [`Bridge::merge`] requires a task that declares merge-under-test, and refuses otherwise. A merge
//! during evaluation silently reunites two branches that were supposed to be independent evidence.
//!
//! # What is not implemented
//!
//! There is no external runtime. Nothing here forks a process, and no provider is linked — 05.11's
//! own dependency-risk section says to "pin supported versions and keep the adapter optional", and
//! optional here means absent. [`ExternalHandle`] is an opaque string: the bridge records that a
//! handle was mapped, never what it points at. The blueprint's evaluation section (fork latency,
//! state fidelity, cache reuse, cost relative to fresh reruns) asks for **measurements**, and no
//! measurement has been taken, so no number appears in this module.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::error::{require_nonempty, SweepError};
use crate::fidelity::{Declaration, Level};

/// A concern that either the external runtime or PRISM owns. 05.11's two lists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Concern {
    ProcessAndEnvironmentState,
    BranchOperations,
    CellSelection,
    OracleSemantics,
    MutationLineage,
    AdaptiveEvaluation,
    ResultAttestation,
}

/// Which side owns a concern.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Owner {
    External,
    Prism,
}

impl Concern {
    pub const ALL: [Concern; 7] = [
        Concern::ProcessAndEnvironmentState,
        Concern::BranchOperations,
        Concern::CellSelection,
        Concern::OracleSemantics,
        Concern::MutationLineage,
        Concern::AdaptiveEvaluation,
        Concern::ResultAttestation,
    ];

    /// The assignment 05.11 makes. Total and, being a function, disjoint by construction.
    pub fn owner(self) -> Owner {
        match self {
            Concern::ProcessAndEnvironmentState | Concern::BranchOperations => Owner::External,
            Concern::CellSelection
            | Concern::OracleSemantics
            | Concern::MutationLineage
            | Concern::AdaptiveEvaluation
            | Concern::ResultAttestation => Owner::Prism,
        }
    }
}

/// The concerns PRISM keeps whatever substrate is underneath.
pub fn owned_by_prism() -> Vec<Concern> {
    Concern::ALL.into_iter().filter(|c| c.owner() == Owner::Prism).collect()
}

/// An opaque reference into the external runtime. Never interpreted here.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ExternalHandle(String);

impl ExternalHandle {
    pub fn new(value: impl Into<String>) -> Result<Self, SweepError> {
        let value = value.into();
        require_nonempty(&value, "ExternalHandle", "value")?;
        Ok(ExternalHandle(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// What PRISM concept an external handle corresponds to. 05.11's mapping section.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MappedConcept {
    /// Typed external effects map to Event IR.
    Effect,
    /// External commit/scope handles map to WorldTape checkpoints.
    Checkpoint,
    /// External forks create PRISM branches.
    Branch,
}

/// A transformation the integration either supports or explicitly does not.
///
/// 05.11's third responsibility is "declare unsupported transformations". The reason is required,
/// because a transformation listed as unsupported with no explanation cannot be worked around.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "state")]
pub enum Transformation {
    Supported,
    Unsupported { reason: String },
}

/// Whether the external runtime is reachable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Availability {
    Available,
    Unavailable,
}

/// How a trial will actually run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Route {
    External,
    /// 05.11's fallback: reconstruct locally, and say so.
    LocalFallback,
}

/// A route together with the reproducibility it is entitled to declare.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Resolved {
    pub route: Route,
    pub declaration: Declaration,
}

/// A task's own statement about whether merging is part of what it tests.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskPolicy {
    pub task_id: String,
    /// 05.11: merges are disabled "unless the task explicitly tests them".
    pub merge_under_test: bool,
}

/// The integration surface: mappings, unsupported transformations, and a route decision.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bridge {
    provider: String,
    pinned_version: String,
    mappings: BTreeMap<String, MappedConcept>,
    transformations: BTreeMap<String, Transformation>,
    /// What the external route may declare. Supplied by the caller because this crate cannot
    /// measure another runtime's state fidelity.
    external_declaration: Declaration,
}

impl Bridge {
    /// A bridge must pin a provider version. 05.11's dependency-risk section asks for it, and an
    /// unpinned integration is one whose fidelity claim is about a version nobody recorded.
    pub fn new(
        provider: impl Into<String>,
        pinned_version: impl Into<String>,
        external_declaration: Declaration,
    ) -> Result<Self, SweepError> {
        let provider = provider.into();
        let pinned_version = pinned_version.into();
        require_nonempty(&provider, "Bridge", "provider")?;
        require_nonempty(&pinned_version, "Bridge", "pinned_version")?;
        Ok(Bridge {
            provider,
            pinned_version,
            mappings: BTreeMap::new(),
            transformations: BTreeMap::new(),
            external_declaration,
        })
    }

    pub fn provider(&self) -> &str {
        &self.provider
    }

    pub fn pinned_version(&self) -> &str {
        &self.pinned_version
    }

    pub fn mapping(mut self, handle: &ExternalHandle, concept: MappedConcept) -> Self {
        self.mappings.insert(handle.as_str().to_string(), concept);
        self
    }

    pub fn concept_of(&self, handle: &ExternalHandle) -> Option<MappedConcept> {
        self.mappings.get(handle.as_str()).copied()
    }

    /// Declare a transformation unsupported, with a reason.
    pub fn declaring_unsupported(
        mut self,
        transformation: impl Into<String>,
        reason: impl Into<String>,
    ) -> Result<Self, SweepError> {
        let reason = reason.into();
        require_nonempty(&reason, "Bridge::declaring_unsupported", "reason")?;
        self.transformations
            .insert(transformation.into(), Transformation::Unsupported { reason });
        Ok(self)
    }

    pub fn declaring_supported(mut self, transformation: impl Into<String>) -> Self {
        self.transformations.insert(transformation.into(), Transformation::Supported);
        self
    }

    /// What the bridge says about a transformation.
    ///
    /// An undeclared transformation comes back `None`, not `Supported`. "We never considered it"
    /// and "we considered it and it works" are the two states this crate never merges.
    pub fn transformation(&self, name: &str) -> Option<&Transformation> {
        self.transformations.get(name)
    }

    /// Choose a route and the declaration that goes with it.
    ///
    /// The fallback declaration is at most `Degraded`. It can be lower — if the external
    /// declaration was already `Absent`, the meet keeps it there — but never higher, because
    /// [`Declaration::lowered_to`] refuses to raise.
    pub fn resolve(&self, availability: Availability) -> Result<Resolved, SweepError> {
        match availability {
            Availability::Available => Ok(Resolved {
                route: Route::External,
                declaration: self.external_declaration.clone(),
            }),
            Availability::Unavailable => {
                let declaration = if self.external_declaration.level() <= Level::Degraded {
                    self.external_declaration.clone()
                } else {
                    self.external_declaration.lowered_to(
                        Level::Degraded,
                        format!(
                            "{} unavailable; world state reconstructed in the local executor",
                            self.provider
                        ),
                    )?
                };
                Ok(Resolved { route: Route::LocalFallback, declaration })
            }
        }
    }

    /// Merge two branches, if the task is about merging.
    pub fn merge(
        &self,
        task: &TaskPolicy,
        left: &ExternalHandle,
        right: &ExternalHandle,
    ) -> Result<ExternalHandle, SweepError> {
        if !task.merge_under_test {
            return Err(SweepError::UndeclaredPrecondition {
                operation: "branch merge during evaluation",
                declaration: "a task that explicitly tests merging",
            });
        }
        ExternalHandle::new(format!("{}+{}", left.as_str(), right.as_str()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bridge() -> Bridge {
        Bridge::new(
            "shepherd",
            "0.4.2",
            Declaration::equivalent("provider-native replay proof retained").unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn prism_keeps_cell_selection_oracle_semantics_lineage_evaluation_and_attestation() {
        let prism = owned_by_prism();
        assert_eq!(prism.len(), 5);
        assert!(prism.contains(&Concern::CellSelection));
        assert!(prism.contains(&Concern::OracleSemantics));
        assert!(prism.contains(&Concern::ResultAttestation));
        assert!(!prism.contains(&Concern::ProcessAndEnvironmentState));
        assert!(!prism.contains(&Concern::BranchOperations));
    }

    #[test]
    fn the_ownership_partition_is_total_over_the_seven_concerns() {
        assert_eq!(Concern::ALL.len(), 7);
        let external = Concern::ALL.iter().filter(|c| c.owner() == Owner::External).count();
        assert_eq!(external + owned_by_prism().len(), Concern::ALL.len());
    }

    #[test]
    fn an_unavailable_external_runtime_lowers_the_declaration_rather_than_keeping_it() {
        let resolved = bridge().resolve(Availability::Unavailable).unwrap();
        assert_eq!(resolved.route, Route::LocalFallback);
        assert_eq!(resolved.declaration.level(), Level::Degraded);
        assert!(resolved.declaration.basis().contains("shepherd unavailable"));
    }

    #[test]
    fn an_available_external_runtime_keeps_the_declaration_it_was_given() {
        let resolved = bridge().resolve(Availability::Available).unwrap();
        assert_eq!(resolved.route, Route::External);
        assert_eq!(resolved.declaration.level(), Level::Equivalent);
    }

    #[test]
    fn a_fallback_never_raises_an_already_worse_declaration() {
        let weak = Bridge::new(
            "shepherd",
            "0.4.2",
            Declaration::absent("no external state was ever captured").unwrap(),
        )
        .unwrap();
        let resolved = weak.resolve(Availability::Unavailable).unwrap();
        assert_eq!(resolved.declaration.level(), Level::Absent);
    }

    #[test]
    fn a_bridge_must_pin_a_provider_version() {
        assert!(Bridge::new("shepherd", "", Declaration::exact()).is_err());
        assert!(Bridge::new("", "0.4.2", Declaration::exact()).is_err());
        assert_eq!(bridge().pinned_version(), "0.4.2");
        assert_eq!(bridge().provider(), "shepherd");
    }

    #[test]
    fn an_undeclared_transformation_is_unknown_not_supported() {
        let b = bridge().declaring_supported("checkpoint-restore");
        assert_eq!(b.transformation("checkpoint-restore"), Some(&Transformation::Supported));
        assert_eq!(b.transformation("live-migration"), None);
    }

    #[test]
    fn declaring_a_transformation_unsupported_requires_a_reason() {
        assert!(bridge().declaring_unsupported("merge", "").is_err());
        let b = bridge()
            .declaring_unsupported("merge", "branch reunification is not modelled in Event IR")
            .unwrap();
        assert!(matches!(
            b.transformation("merge"),
            Some(Transformation::Unsupported { .. })
        ));
    }

    #[test]
    fn merging_is_refused_unless_the_task_tests_merging() {
        let b = bridge();
        let left = ExternalHandle::new("br-1").unwrap();
        let right = ExternalHandle::new("br-2").unwrap();
        let ordinary = TaskPolicy { task_id: "t1".into(), merge_under_test: false };
        assert!(matches!(
            b.merge(&ordinary, &left, &right),
            Err(SweepError::UndeclaredPrecondition { .. })
        ));
        let merging = TaskPolicy { task_id: "t2".into(), merge_under_test: true };
        assert_eq!(b.merge(&merging, &left, &right).unwrap().as_str(), "br-1+br-2");
    }

    #[test]
    fn external_handles_map_to_prism_concepts_and_are_otherwise_opaque() {
        let checkpoint = ExternalHandle::new("scope-9").unwrap();
        let b = bridge().mapping(&checkpoint, MappedConcept::Checkpoint);
        assert_eq!(b.concept_of(&checkpoint), Some(MappedConcept::Checkpoint));
        assert_eq!(b.concept_of(&ExternalHandle::new("scope-10").unwrap()), None);
    }

    #[test]
    fn an_empty_external_handle_is_refused() {
        assert!(ExternalHandle::new("   ").is_err());
    }
}
