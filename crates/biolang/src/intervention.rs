//! Biological Intervention and Action IR — blueprint 25.06.
//!
//! An action with preconditions, effects, a declared reversibility, an authority requirement, a cost
//! and a result schema. The classification 25.06 exists for is [`ActionClass`]: reading a file,
//! running a differential expression, perturbing a model, recording a historical exposure and
//! actually consuming tissue are five different things, and the failure mode the module names is
//! that the third gets reported as the fifth.
//!
//! # Reversibility, and a sibling crate's finding
//!
//! `bioprism-choreography` records the result that compensation is not rollback: its
//! `SagaOutcome::CompensatedWithResidue` exists because a compensating transaction leaves the world
//! different from how it started. An intervention IR that offered a plain `Compensatable` variant
//! would contradict that, and the contradiction would be invisible — every planner reading the IR
//! would treat "compensatable" as "restorable" and plan accordingly.
//!
//! So [`Reversibility`] has no such variant. It has [`Reversibility::Reversible`] for actions that
//! genuinely restore prior state (a computation over an unchanged input),
//! [`Reversibility::CompensatableWithResidue`] which **must** list what compensation leaves behind,
//! and [`Reversibility::Irreversible`]. An empty residue list is
//! [`InterventionError::CompensationClaimsNoResidue`], because a compensation that leaves nothing
//! behind is a rollback and should be declared as one.
//!
//! # What is deliberately not implemented
//!
//! - **No precondition language.** A [`Precondition`] carries a plane and a prose expression. 25.06
//!   requires preconditions to exist and be checked; it never gives them a grammar, and BioQL is a
//!   query language over worlds, not a state-predicate language over actions. Reusing it here would
//!   mean claiming a semantics for `where` over an action's input state that nothing defines.
//! - **No execution, no retry, no scheduler.** [`Idempotence`] is a declaration a runtime reads.
//! - **No cost currency.** [`CostModel`] counts named resources, the same names
//!   [`crate::state::ResourceLedger`] uses. There is no conversion between them and no budget.

use crate::error::InterventionError;
use crate::ids::ActionId;
use crate::state::Plane;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// What kind of act this is. 25.06's whole purpose is that these five stay distinct.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionClass {
    /// Reading something that already exists. Changes what is known, not what is.
    InformationAcquisition,
    /// Deriving a new artifact from existing ones.
    ComputationalTransformation,
    /// A perturbation applied to a *model*. Never to biology.
    ModeledPerturbation,
    /// Recording an exposure that already happened outside the system.
    HistoricalExposure,
    /// Something that actually changes material or biology.
    RealWorldEffect,
}

impl ActionClass {
    pub fn as_str(self) -> &'static str {
        match self {
            ActionClass::InformationAcquisition => "information acquisition",
            ActionClass::ComputationalTransformation => "computational transformation",
            ActionClass::ModeledPerturbation => "modeled perturbation",
            ActionClass::HistoricalExposure => "historical exposure",
            ActionClass::RealWorldEffect => "real-world effect",
        }
    }

    /// True when the class asserts something happened in the world rather than in a model.
    pub fn touches_reality(self) -> bool {
        matches!(
            self,
            ActionClass::RealWorldEffect | ActionClass::HistoricalExposure
        )
    }
}

/// What the action does, and where.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Effect {
    pub plane: Plane,
    /// What changes, in prose.
    pub description: String,
    /// True when the change is a claim about the world rather than about a simulation of it.
    pub observed: bool,
}

/// A condition that must hold before the action may run.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Precondition {
    /// Which plane of the input state the condition reads.
    pub plane: Plane,
    /// The condition itself. Prose: 25.06 defines no predicate language.
    pub expression: String,
}

/// Whether the action can be undone, and at what cost.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "reversibility", rename_all = "snake_case")]
pub enum Reversibility {
    /// Undoing restores the prior state exactly. True for pure computations over unchanged inputs.
    Reversible,
    /// A compensating action exists, and it leaves the listed residue.
    ///
    /// Matches `bioprism-choreography`'s `SagaOutcome::CompensatedWithResidue`. There is
    /// deliberately no variant that claims compensation restores prior state.
    CompensatableWithResidue { residue: Vec<String> },
    Irreversible,
}

impl Reversibility {
    pub fn is_irreversible(&self) -> bool {
        matches!(self, Reversibility::Irreversible)
    }
}

/// What authority the actor must hold. 25.06: "Irreversible effects require explicit authority."
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "authority", rename_all = "snake_case")]
pub enum AuthorityRequirement {
    /// Anyone in the world may do this.
    None,
    /// A named capability, in `bioprism-weave`'s vocabulary.
    Capability { capability: String },
    /// A named human role must approve, out of band.
    HumanApproval { role: String },
}

impl AuthorityRequirement {
    pub fn is_none(&self) -> bool {
        matches!(self, AuthorityRequirement::None)
    }
}

/// Whether retrying is safe. 25.06: "Action retries are idempotent or explicitly non-idempotent."
///
/// No `Default`, because the safe-looking default is the wrong one for exactly the actions that
/// matter: retrying a tissue reservation consumes tissue twice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Idempotence {
    Idempotent,
    NotIdempotent,
}

/// Declared resource cost, keyed by the same resource names a state's ledger uses.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CostModel {
    costs: BTreeMap<String, f64>,
}

impl CostModel {
    pub fn new() -> Self {
        CostModel::default()
    }

    pub fn costing(mut self, resource: impl Into<String>, amount: f64) -> Self {
        self.costs.insert(resource.into(), amount);
        self
    }

    pub fn cost_of(&self, resource: &str) -> f64 {
        self.costs.get(resource).copied().unwrap_or(0.0)
    }

    pub fn resources(&self) -> impl Iterator<Item = &str> {
        self.costs.keys().map(String::as_str)
    }

    pub fn is_free(&self) -> bool {
        self.costs.is_empty()
    }
}

/// One entry in a world's action catalog.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActionDefinition {
    pub action_id: ActionId,
    pub class: ActionClass,
    /// Which planes of the input state the action reads.
    pub input_planes: BTreeSet<Plane>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub preconditions: Vec<Precondition>,
    pub effects: Vec<Effect>,
    pub reversibility: Reversibility,
    pub authority: AuthorityRequirement,
    pub cost: CostModel,
    /// Declared latency in seconds. A declaration, not a measurement.
    pub latency_seconds: f64,
    /// What is uncertain about the outcome, in prose. 25.06 requires the field and names no model.
    pub uncertainty: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub side_effects: Vec<String>,
    pub idempotence: Idempotence,
    /// A digest of the result schema. The schema itself belongs to whoever publishes it.
    pub result_schema: String,
}

impl ActionDefinition {
    /// Every invariant 25.06 states.
    pub fn validate(&self) -> Result<(), InterventionError> {
        let action = self.action_id.to_string();

        if self.class == ActionClass::ModeledPerturbation {
            if let Some(effect) = self
                .effects
                .iter()
                .find(|effect| effect.plane.is_ontic() && effect.observed)
            {
                return Err(InterventionError::SimulationClaimsRealEffect {
                    action,
                    plane: effect.plane.to_string(),
                });
            }
        }

        if self.class == ActionClass::RealWorldEffect
            && !self.effects.iter().any(|effect| effect.plane.is_ontic())
        {
            return Err(InterventionError::RealEffectWithoutRealPlane { action });
        }

        if self.reversibility.is_irreversible() && self.authority.is_none() {
            return Err(InterventionError::IrreversibleWithoutAuthority { action });
        }

        if let Reversibility::CompensatableWithResidue { residue } = &self.reversibility {
            if residue.is_empty() {
                return Err(InterventionError::CompensationClaimsNoResidue { action });
            }
        }

        let consumes_material = self
            .effects
            .iter()
            .any(|effect| effect.plane == Plane::Material);
        if consumes_material
            && self.reversibility.is_irreversible()
            && self.idempotence == Idempotence::Idempotent
        {
            return Err(
                InterventionError::IrreversibleConsumptionCannotBeIdempotent { action },
            );
        }

        for precondition in &self.preconditions {
            if !self.input_planes.contains(&precondition.plane) {
                return Err(InterventionError::PreconditionOffInputPlane {
                    action,
                    plane: precondition.plane.to_string(),
                });
            }
        }

        Ok(())
    }

    /// True when this action asserts a change to the world rather than to a model of it.
    ///
    /// The question `bioprism-onco`'s research boundary needs answered before anything downstream
    /// treats an action's outcome as evidence about biology.
    pub fn is_real_intervention(&self) -> bool {
        self.class.touches_reality() && self.effects.iter().any(|effect| effect.observed)
    }
}
