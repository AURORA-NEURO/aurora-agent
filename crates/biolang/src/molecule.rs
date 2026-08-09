//! BioCapability Molecule IR — blueprint 25.17.
//!
//! A verified multi-agent workflow behind one stable interface: roles, bindings, a choreography,
//! declared guarantees, nested interfaces and the evaluation evidence the guarantees rest on.
//!
//! # The invariants
//!
//! - *Nested molecules do not broaden authority.* [`Molecule::validate`] refuses a nested molecule
//!   requiring a capability its parent does not hold. This is the containment property that makes
//!   composition safe: wrapping a workflow must not be a way to acquire permissions.
//! - *Guarantees are backed by evaluation evidence.* A [`Guarantee`] with no [`CapabilityEvidence`]
//!   behind it is refused. An unbacked guarantee is a marketing claim in a machine-readable format.
//! - *Internal attribution is preserved.* Every choreography step names the role that performs it,
//!   and a molecule that publishes a result with no attributed steps is refused. A molecule is a
//!   stable interface, not an anonymiser.
//!
//! # What is deliberately not implemented
//!
//! 25.17 lists "protocol model checking" under validation. There is no model checker here and no
//! process calculus: a [`Choreography`] is an ordered list of steps with role bindings, checked for
//! role coverage and nothing more. `bioprism-choreography` owns saga semantics, including the
//! finding that compensation leaves residue; a second protocol semantics in this crate would be a
//! second thing to keep consistent with it.

use crate::error::MoleculeError;
use crate::ids::MoleculeId;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// A role the molecule needs filled.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoleBinding {
    pub role: String,
    /// What is bound to it: a system id, a nested molecule id, a human role name.
    pub bound_to: String,
    /// Capabilities the bound party exercises inside this molecule.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub capabilities: BTreeSet<String>,
}

/// One step of the workflow.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Step {
    pub step_id: String,
    /// The role that performs it. Attribution is not optional.
    pub role: String,
    pub description: String,
}

/// The ordered workflow.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Choreography {
    pub steps: Vec<Step>,
}

impl Choreography {
    pub fn then(mut self, step: Step) -> Self {
        self.steps.push(step);
        self
    }
}

/// Evidence that the molecule does what it claims.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityEvidence {
    pub evidence_id: String,
    /// Which benchmark or evaluation produced it.
    pub source: String,
    /// What it showed, in prose. No score is stored here: a score without its bundle is not a
    /// measurement, and [`crate::bundle`] is where the bundle lives.
    pub finding: String,
}

/// A promise the molecule makes at its interface.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Guarantee {
    pub guarantee_id: String,
    pub statement: String,
    /// Evidence ids from [`Molecule::evidence`]. Empty is refused.
    pub backed_by: BTreeSet<String>,
}

/// How the molecule fails, declared rather than discovered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureSemantics {
    /// Nothing is left behind on failure.
    AllOrNothing,
    /// Partial work survives and is visible to the caller.
    PartialWithResidue,
    /// Failure is reported and the caller must inspect what happened.
    ReportOnly,
}

/// A nested molecule and the authority it asks for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NestedInterface {
    pub molecule: MoleculeId,
    pub requires: BTreeSet<String>,
}

/// A packaged multi-agent workflow.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Molecule {
    pub molecule_id: MoleculeId,
    pub input_schema: String,
    pub output_schema: String,
    pub roles: Vec<RoleBinding>,
    pub choreography: Choreography,
    /// The capabilities the molecule itself holds. Nested molecules may not exceed this.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub authority: BTreeSet<String>,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub effects: BTreeSet<String>,
    pub guarantees: Vec<Guarantee>,
    pub evidence: Vec<CapabilityEvidence>,
    pub failure: FailureSemantics,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub nested: Vec<NestedInterface>,
    pub version: String,
}

impl Molecule {
    pub fn validate(&self) -> Result<(), MoleculeError> {
        let roles: BTreeSet<&str> = self.roles.iter().map(|role| role.role.as_str()).collect();
        for step in &self.choreography.steps {
            if !roles.contains(step.role.as_str()) {
                return Err(MoleculeError::UnboundStep {
                    step: step.step_id.clone(),
                });
            }
        }

        if !self.choreography.steps.is_empty()
            && self
                .choreography
                .steps
                .iter()
                .all(|step| step.role.trim().is_empty())
        {
            return Err(MoleculeError::AttributionErased {
                molecule: self.molecule_id.to_string(),
            });
        }

        let evidence: BTreeSet<&str> = self
            .evidence
            .iter()
            .map(|item| item.evidence_id.as_str())
            .collect();
        for guarantee in &self.guarantees {
            let backed = guarantee
                .backed_by
                .iter()
                .any(|id| evidence.contains(id.as_str()));
            if !backed {
                return Err(MoleculeError::UnbackedGuarantee {
                    guarantee: guarantee.guarantee_id.clone(),
                });
            }
        }

        for nested in &self.nested {
            for capability in &nested.requires {
                if !self.authority.contains(capability) {
                    return Err(MoleculeError::NestedAuthorityBroadened {
                        parent: self.molecule_id.to_string(),
                        nested: nested.molecule.to_string(),
                        capability: capability.clone(),
                    });
                }
            }
        }

        Ok(())
    }
}
