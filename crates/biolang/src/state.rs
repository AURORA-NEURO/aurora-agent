//! BioState IR — blueprint 25.02.
//!
//! A state is forkable, scoped, and split across six planes: biological, material, observation,
//! knowledge, resource, protocol. The blueprint's three invariants are the whole design:
//!
//! 1. *State hashes change only when a represented plane changes.* So the state's digest is over
//!    per-plane hashes, and [`BioState::changed_planes`] can say exactly which plane moved.
//! 2. *Biological and epistemic changes are never conflated.* So a [`Transition`] must **declare**
//!    which planes it changes, and [`Transition::validate`] refuses a declaration that does not
//!    match the hashes. A run that learns something new about a tumour and a run in which the
//!    tumour grew produce different declarations, and neither can be written as the other.
//! 3. *A fork copies logical state but cannot duplicate affine resources.* So [`BioState::fork`]
//!    carries the parent's consumption forward and [`BioState::validate_fork`] refuses a child that
//!    consumed less than its parent — un-spending is how an affine budget gets duplicated.
//!
//! # What is deliberately not implemented
//!
//! - **No plane contents.** A plane is a [`ContentHash`] here, not a document. What is in the
//!   biological plane is the world's business; this IR's business is that a change to it is visible.
//! - **No resource semantics.** [`ResourceLedger`] counts consumption of named resources; it does
//!   not know that tissue is affine and compute is not. `bioprism-weave`'s `Budget` owns that, and
//!   it enforces affineness by refusing to implement `Clone` — a stronger guarantee than any
//!   validation function here could make.
//! - **No uncertainty computation.** 25.02 requires an "uncertainty summary"; `bioprism-bioir`'s
//!   `UncertaintyBudget` is the real vocabulary and this IR carries a reference to one, not a
//!   reimplementation.

use crate::clock::Stamped;
use crate::error::StateError;
use crate::ids::StateId;
use bioprism_ids::{ContentHash, WorldId};
use bioprism_scope::{ScopeKey, Timestamp};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// The six planes 25.02 names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Plane {
    /// What is true of the biology.
    Biological,
    /// What material exists, where, and how much of it is left.
    Material,
    /// What has been measured.
    Observation,
    /// What is believed, and with what support. The epistemic plane.
    Knowledge,
    /// What has been spent.
    Resource,
    /// What procedure is in force.
    Protocol,
}

impl Plane {
    pub const ALL: [Plane; 6] = [
        Plane::Biological,
        Plane::Material,
        Plane::Observation,
        Plane::Knowledge,
        Plane::Resource,
        Plane::Protocol,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Plane::Biological => "biological",
            Plane::Material => "material",
            Plane::Observation => "observation",
            Plane::Knowledge => "knowledge",
            Plane::Resource => "resource",
            Plane::Protocol => "protocol",
        }
    }

    /// True for the plane where the world changed, as opposed to where belief changed.
    ///
    /// The distinction 25.02 protects: an assay result moves the observation and knowledge planes
    /// and leaves the biological plane exactly where it was.
    pub fn is_ontic(self) -> bool {
        matches!(self, Plane::Biological | Plane::Material)
    }
}

impl fmt::Display for Plane {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Consumption of named resources, monotone by construction of the API.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ResourceLedger {
    consumed: BTreeMap<String, f64>,
}

impl ResourceLedger {
    pub fn new() -> Self {
        ResourceLedger::default()
    }

    /// Records further consumption. There is no `release` and no `set`: this ledger only goes up.
    pub fn consume(mut self, resource: impl Into<String>, amount: f64) -> Self {
        *self.consumed.entry(resource.into()).or_insert(0.0) += amount;
        self
    }

    pub fn consumed(&self, resource: &str) -> f64 {
        self.consumed.get(resource).copied().unwrap_or(0.0)
    }

    pub fn resources(&self) -> impl Iterator<Item = &str> {
        self.consumed.keys().map(String::as_str)
    }

    pub fn is_empty(&self) -> bool {
        self.consumed.is_empty()
    }

    /// The first resource whose recorded amount is not a finite number.
    ///
    /// Exists because `serde_json` silently renders a non-finite float as JSON `null`, which the
    /// canonical encoder then hashes as an absent field. See [`crate::canonical`] for the full
    /// finding.
    pub fn first_non_finite(&self) -> Option<&str> {
        self.consumed
            .iter()
            .find(|(_, amount)| !amount.is_finite())
            .map(|(resource, _)| resource.as_str())
    }
}

/// A pointer to the uncertainty accounting for this state.
///
/// 25.02 requires an "uncertainty summary". `bioprism-bioir` owns the vocabulary (25.12), so what is
/// carried here is a digest of the budget plus the count of components that were left unaccounted —
/// enough to tell whether the summary is complete, not enough to pretend it was recomputed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UncertaintySummary {
    pub budget_digest: ContentHash,
    /// Kinds of uncertainty the budget names but does not quantify. Zero is a claim, not a default.
    pub unaccounted_components: usize,
}

/// One forkable state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BioState {
    pub state_id: StateId,
    pub world_id: WorldId,
    /// When the state obtained in the world.
    pub event_time: Timestamp,
    /// When the system recorded it.
    pub record_time: Timestamp,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_state: Option<StateId>,
    /// Where this state is valid. A worldline may not mix scopes; see [`crate::worldline`].
    pub scope: ScopeKey,
    pub plane_hashes: BTreeMap<Plane, ContentHash>,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub open_obligations: BTreeSet<String>,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub active_grants: BTreeSet<String>,
    pub consumed: ResourceLedger,
    pub uncertainty: UncertaintySummary,
}

impl BioState {
    pub fn new(
        state_id: StateId,
        world_id: WorldId,
        event_time: Timestamp,
        record_time: Timestamp,
        uncertainty: UncertaintySummary,
    ) -> Self {
        BioState {
            state_id,
            world_id,
            event_time,
            record_time,
            parent_state: None,
            scope: ScopeKey::new(),
            plane_hashes: BTreeMap::new(),
            open_obligations: BTreeSet::new(),
            active_grants: BTreeSet::new(),
            consumed: ResourceLedger::new(),
            uncertainty,
        }
    }

    pub fn within(mut self, scope: ScopeKey) -> Self {
        self.scope = scope;
        self
    }

    pub fn with_plane(mut self, plane: Plane, hash: ContentHash) -> Self {
        self.plane_hashes.insert(plane, hash);
        self
    }

    pub fn owing(mut self, obligation: impl Into<String>) -> Self {
        self.open_obligations.insert(obligation.into());
        self
    }

    pub fn granting(mut self, grant: impl Into<String>) -> Self {
        self.active_grants.insert(grant.into());
        self
    }

    pub fn having_consumed(mut self, ledger: ResourceLedger) -> Self {
        self.consumed = ledger;
        self
    }

    /// The event and record instants, each tagged with its clock.
    pub fn stamps(&self) -> [Stamped; 2] {
        [
            Stamped::event(self.event_time),
            Stamped::record(self.record_time),
        ]
    }

    /// 25.02 validation, "clock-order checks", plus the encodability check the canonical form needs.
    pub fn validate(&self) -> Result<(), StateError> {
        if self.record_time.as_nanos_utc() < self.event_time.as_nanos_utc() {
            return Err(StateError::RecordBeforeEvent {
                state: self.state_id.to_string(),
                event: self.event_time.to_rfc3339(),
                record: self.record_time.to_rfc3339(),
            });
        }
        if let Some(resource) = self.consumed.first_non_finite() {
            return Err(StateError::NonFiniteAmount {
                state: self.state_id.to_string(),
                resource: resource.to_string(),
            });
        }
        Ok(())
    }

    /// Which planes differ between two states.
    ///
    /// A plane present on one side and absent on the other counts as changed: an absent plane hash
    /// is "this state does not represent that plane", which is a different state of affairs from
    /// any hash value, and treating absence as equal to absence-elsewhere would let a plane vanish
    /// without appearing in a transition's declaration.
    pub fn changed_planes(&self, other: &BioState) -> BTreeSet<Plane> {
        Plane::ALL
            .into_iter()
            .filter(|plane| self.plane_hashes.get(plane) != other.plane_hashes.get(plane))
            .collect()
    }

    /// A child state that inherits this state's scope, grants and consumption.
    ///
    /// The consumption ledger is *carried*, not reset. That is the affine-resource invariant in the
    /// only form an IR can express it: a fork may spend more than its parent and may not spend less.
    pub fn fork(
        &self,
        child_id: StateId,
        event_time: Timestamp,
        record_time: Timestamp,
    ) -> BioState {
        BioState {
            state_id: child_id,
            world_id: self.world_id.clone(),
            event_time,
            record_time,
            parent_state: Some(self.state_id.clone()),
            scope: self.scope.clone(),
            plane_hashes: self.plane_hashes.clone(),
            open_obligations: self.open_obligations.clone(),
            active_grants: self.active_grants.clone(),
            consumed: self.consumed.clone(),
            uncertainty: self.uncertainty.clone(),
        }
    }

    /// Checks that `child` is a legitimate fork of `self`.
    pub fn validate_fork(&self, child: &BioState) -> Result<(), StateError> {
        if child.parent_state.as_ref() != Some(&self.state_id) {
            return Err(StateError::ForkParentMismatch {
                child: child.state_id.to_string(),
                parent: self.state_id.to_string(),
            });
        }
        for resource in self.consumed.resources() {
            let parent_consumed = self.consumed.consumed(resource);
            let child_consumed = child.consumed.consumed(resource);
            if child_consumed < parent_consumed {
                return Err(StateError::ForkUnspendsResource {
                    child: child.state_id.to_string(),
                    resource: resource.to_string(),
                    parent_consumed,
                    consumed: child_consumed,
                });
            }
        }
        Ok(())
    }
}

/// A declared step from one state to another.
///
/// The declaration is the point. 25.02's second invariant — biological and epistemic changes are
/// never conflated — is unenforceable if a transition merely happens; it becomes checkable the
/// moment the author has to write down which planes moved.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Transition {
    pub label: String,
    pub from: StateId,
    pub to: StateId,
    pub declared_planes: BTreeSet<Plane>,
}

impl Transition {
    pub fn new(label: impl Into<String>, from: StateId, to: StateId) -> Self {
        Transition {
            label: label.into(),
            from,
            to,
            declared_planes: BTreeSet::new(),
        }
    }

    pub fn changing(mut self, plane: Plane) -> Self {
        self.declared_planes.insert(plane);
        self
    }

    /// True when this transition claims the world itself moved, not only what is known about it.
    pub fn is_ontic(&self) -> bool {
        self.declared_planes.iter().copied().any(Plane::is_ontic)
    }

    /// Checks the declaration against the two states' plane hashes.
    pub fn validate(&self, from: &BioState, to: &BioState) -> Result<(), StateError> {
        let actual = from.changed_planes(to);
        for plane in &actual {
            if !self.declared_planes.contains(plane) {
                return Err(StateError::UndeclaredPlaneChange {
                    label: self.label.clone(),
                    plane: plane.to_string(),
                });
            }
        }
        for plane in &self.declared_planes {
            if !actual.contains(plane) {
                return Err(StateError::DeclaredPlaneUnchanged {
                    label: self.label.clone(),
                    plane: plane.to_string(),
                });
            }
        }
        Ok(())
    }
}
