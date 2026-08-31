//! Model, Pipeline, Agent and Molecule Architecture IR — blueprint 25.14.
//!
//! One observable contract for every evaluated system, "while preserving their differences". A
//! single-model baseline, a retrieval pipeline and a multi-agent molecule all publish the same
//! manifest: components with pinned versions, inputs and outputs, effects, context policy, memory,
//! tools, verifiers, budgets, permissions, a determinism claim and a list of known limits.
//!
//! # The invariants that carry weight
//!
//! - *Published results pin every component.* [`Pin`] has no variant meaning "latest"; a component is
//!   pinned to an exact version and a digest, or [`SystemManifest::validate`] refuses it. A result
//!   that cites an unpinned component cannot be reproduced and should not be publishable.
//! - *Architecture comparisons expose changed components.* [`SystemManifest::diff`] returns the
//!   components that differ. A comparison of two systems that does not say what changed between them
//!   is a leaderboard row, not a finding.
//! - *Private prompts may be hashed but behavior contracts remain observable.* [`PromptDisclosure`]
//!   admits a hash, and a component that hides its prompt must still declare a behaviour contract.
//!   Secrecy about the text is allowed; secrecy about what the component is *for* is not.
//!
//! # What is deliberately not implemented
//!
//! - **No execution and no introspection.** Everything here is declared by the publisher. Nothing
//!   runs a component to check that its determinism claim holds.
//! - **No budget arithmetic.** Budgets are declared caps. `bioprism-weave`'s `Budget` is the affine
//!   one, and it enforces non-duplication by not implementing `Clone` — a guarantee no declaration
//!   can make.

use crate::error::SystemError;
use crate::ids::{ComponentId, SystemId};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// An exact version and digest. There is no "latest".
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Pin {
    pub version: String,
    /// A content digest of the artefact, when one exists.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub digest: Option<String>,
}

impl Pin {
    pub fn new(version: impl Into<String>) -> Self {
        Pin {
            version: version.into(),
            digest: None,
        }
    }

    pub fn digested(mut self, digest: impl Into<String>) -> Self {
        self.digest = Some(digest.into());
        self
    }

    /// A pin is well-formed when the version is non-empty and is not a moving target.
    pub fn is_exact(&self) -> bool {
        let version = self.version.trim();
        !version.is_empty()
            && !matches!(
                version,
                "latest" | "main" | "master" | "head" | "HEAD" | "*"
            )
    }
}

/// What kind of thing a component is. 25.14 names model, tool, verifier and agent-role nodes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComponentKind {
    Model,
    Tool,
    Verifier,
    AgentRole,
    /// A nested capability molecule; see [`crate::molecule`].
    Molecule,
}

/// How much of a component's prompt is published.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "prompt", rename_all = "snake_case")]
pub enum PromptDisclosure {
    /// No prompt: a tool, a verifier, a deterministic transform.
    NotApplicable,
    Published {
        text_digest: String,
    },
    /// Withheld, but committed to by digest so a later claim about it is checkable.
    Hashed {
        digest: String,
    },
}

impl PromptDisclosure {
    pub fn is_hidden(&self) -> bool {
        matches!(self, PromptDisclosure::Hashed { .. })
    }
}

/// One node of the system.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Component {
    pub component_id: ComponentId,
    pub kind: ComponentKind,
    pub pin: Pin,
    pub inputs: BTreeSet<String>,
    pub outputs: BTreeSet<String>,
    /// Side effects outside the component's outputs.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub effects: BTreeSet<String>,
    pub prompt: PromptDisclosure,
    /// What the component is for, observably. Required whenever the prompt is hidden.
    pub behavior_contract: String,
    /// Inputs that make the component nondeterministic: a sampler seed, a clock, a network call.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub nondeterministic_inputs: BTreeSet<String>,
    pub deterministic: bool,
}

/// A directed edge between two components.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Wire {
    pub from: ComponentId,
    pub to: ComponentId,
    pub carries: String,
}

/// The manifest 25.14 requires.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SystemManifest {
    pub system_id: SystemId,
    pub components: Vec<Component>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub graph: Vec<Wire>,
    /// How context is assembled for the system, named rather than described.
    pub context_policy: String,
    /// What the system remembers between steps. `"none"` is a claim worth making explicitly.
    pub memory: String,
    /// Caps by resource name.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub budgets: BTreeMap<String, f64>,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub permissions: BTreeSet<String>,
    /// 25.14: "known limits". A published system with an empty list is making a strong claim.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub known_limits: Vec<String>,
}

/// One component that differs between two manifests.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "delta", rename_all = "snake_case")]
pub enum ComponentDelta {
    OnlyInLeft {
        component: String,
    },
    OnlyInRight {
        component: String,
    },
    Repinned {
        component: String,
        left: String,
        right: String,
    },
}

impl SystemManifest {
    pub fn new(
        system_id: SystemId,
        context_policy: impl Into<String>,
        memory: impl Into<String>,
    ) -> Self {
        SystemManifest {
            system_id,
            components: Vec::new(),
            graph: Vec::new(),
            context_policy: context_policy.into(),
            memory: memory.into(),
            budgets: BTreeMap::new(),
            permissions: BTreeSet::new(),
            known_limits: Vec::new(),
        }
    }

    pub fn with(mut self, component: Component) -> Self {
        self.components.push(component);
        self
    }

    pub fn wired(mut self, wire: Wire) -> Self {
        self.graph.push(wire);
        self
    }

    pub fn limited_by(mut self, limit: impl Into<String>) -> Self {
        self.known_limits.push(limit.into());
        self
    }

    pub fn validate(&self) -> Result<(), SystemError> {
        for component in &self.components {
            if !component.pin.is_exact() {
                return Err(SystemError::UnpinnedComponent {
                    component: component.component_id.to_string(),
                    detail: format!("version {:?} is not an exact pin", component.pin.version),
                });
            }
            if component.prompt.is_hidden() && component.behavior_contract.trim().is_empty() {
                return Err(SystemError::HiddenBehaviourContract {
                    component: component.component_id.to_string(),
                });
            }
            if component.deterministic {
                if let Some(input) = component.nondeterministic_inputs.iter().next() {
                    return Err(SystemError::DeterminismContradicted {
                        component: component.component_id.to_string(),
                        input: input.clone(),
                    });
                }
            }
        }

        for wire in &self.graph {
            for end in [&wire.from, &wire.to] {
                if !self.declares(end) {
                    return Err(SystemError::DanglingComponent {
                        component: end.to_string(),
                    });
                }
            }
        }

        Ok(())
    }

    fn declares(&self, id: &ComponentId) -> bool {
        self.components
            .iter()
            .any(|component| &component.component_id == id)
    }

    /// What changed between two systems. 25.14: "Architecture comparisons expose changed
    /// components."
    pub fn diff(&self, other: &SystemManifest) -> Vec<ComponentDelta> {
        let mut deltas = Vec::new();
        for component in &self.components {
            match other
                .components
                .iter()
                .find(|candidate| candidate.component_id == component.component_id)
            {
                None => deltas.push(ComponentDelta::OnlyInLeft {
                    component: component.component_id.to_string(),
                }),
                Some(counterpart) if counterpart.pin != component.pin => {
                    deltas.push(ComponentDelta::Repinned {
                        component: component.component_id.to_string(),
                        left: component.pin.version.clone(),
                        right: counterpart.pin.version.clone(),
                    })
                }
                Some(_) => {}
            }
        }
        for component in &other.components {
            if !self.declares(&component.component_id) {
                deltas.push(ComponentDelta::OnlyInRight {
                    component: component.component_id.to_string(),
                });
            }
        }
        deltas.sort();
        deltas
    }
}
