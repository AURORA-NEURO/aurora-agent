//! Action gating.
//!
//! Blueprint 39.06, final sentence: *high-regret actions declare prerequisite obligation
//! predicates; the runtime blocks them when predicates are unmet*. This module is that runtime.
//! It is the point of the whole crate — an obligation graph nobody consults is documentation, and
//! documentation does not stop a model from writing to a registry at three in the morning.
//!
//! [`may_perform`] fails closed, in four ways that matter more than the happy path:
//!
//! 1. **An undeclared gate is not an open gate.** A high-regret action that declares no
//!    prerequisites is blocked, not allowed. Otherwise the cheapest way to pass the gate is to
//!    forget to write it, and that is exactly the failure the gate exists to prevent.
//! 2. **An unknown obligation blocks.** A predicate naming an obligation the graph does not
//!    contain is a broken gate, and a broken gate is closed. Deleting the obligation must not be
//!    a way to satisfy the predicate.
//! 3. **An unverifiable graph blocks.** If the graph has a cycle or a dangling dependency, no
//!    effective state can be computed and nothing may be permitted on the strength of it.
//! 4. **Irreversible actions answer to the whole mandatory closure**, not only to the
//!    prerequisites their author happened to list. Declaring the three easy obligations is
//!    otherwise a way to route around the fourth.
//!
//! Predicates name the *acceptable resolution states* for an obligation, so accepting a waiver is
//! something a gate has to say out loud: [`ObligationPredicate::satisfied`] does not admit
//! `waived_with_reason`, and [`ObligationPredicate::waivable`] does, visibly, in the audit record.
//!
//! Not implemented here: authority. Whether the actor requesting the action is entitled to it is
//! 39.19's role filter and the Weave authority ABI; this module answers only whether the evidence
//! preconditions hold.

use crate::graph::ObligationGraph;
use crate::state::ObligationState;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// How much a wrong action costs, which decides how hard the gate is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RegretClass {
    /// Undo is free.
    Reversible,
    /// Undo costs something but is available.
    Recoverable,
    /// Undo is not available in practice: a published result, a released cohort, a filed report.
    HighRegret,
    /// Undo is not available at all: a consumed specimen, an executed treatment, an egress.
    Irreversible,
}

impl RegretClass {
    /// Whether an empty prerequisite list is itself a reason to block.
    pub fn requires_declared_prerequisites(self) -> bool {
        matches!(self, RegretClass::HighRegret | RegretClass::Irreversible)
    }

    /// Whether the action answers to every mandatory obligation, declared or not.
    pub fn requires_full_mandatory_closure(self) -> bool {
        matches!(self, RegretClass::Irreversible)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            RegretClass::Reversible => "reversible",
            RegretClass::Recoverable => "recoverable",
            RegretClass::HighRegret => "high_regret",
            RegretClass::Irreversible => "irreversible",
        }
    }
}

/// A condition on one obligation that an action requires before it may run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ObligationPredicate {
    pub obligation: String,
    /// The resolution states this gate accepts. Anything outside the set blocks.
    pub accept: BTreeSet<ObligationState>,
    /// Floor on the recording actor's stated confidence. Uncalibrated; see [`crate::state`].
    pub min_confidence: f64,
}

impl ObligationPredicate {
    /// Accepts only evidence-backed satisfaction.
    pub fn satisfied(obligation: impl Into<String>) -> Self {
        ObligationPredicate::accepting(obligation, [ObligationState::Satisfied])
    }

    /// Accepts satisfaction or a reasoned declaration of inapplicability.
    pub fn discharged(obligation: impl Into<String>) -> Self {
        ObligationPredicate::accepting(
            obligation,
            [ObligationState::Satisfied, ObligationState::NotApplicable],
        )
    }

    /// Accepts a recorded waiver as well, which the gate is thereby stating in public.
    pub fn waivable(obligation: impl Into<String>) -> Self {
        ObligationPredicate::accepting(
            obligation,
            [
                ObligationState::Satisfied,
                ObligationState::NotApplicable,
                ObligationState::WaivedWithReason,
            ],
        )
    }

    pub fn accepting<I>(obligation: impl Into<String>, accept: I) -> Self
    where
        I: IntoIterator<Item = ObligationState>,
    {
        ObligationPredicate {
            obligation: obligation.into(),
            accept: accept.into_iter().collect(),
            min_confidence: 0.0,
        }
    }

    pub fn with_min_confidence(mut self, min_confidence: f64) -> Self {
        self.min_confidence = min_confidence;
        self
    }

    fn accepted_states(&self) -> Vec<ObligationState> {
        self.accept.iter().copied().collect()
    }
}

/// A thing an agent wants to do, and what it must establish first.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Action {
    pub id: String,
    pub description: String,
    pub regret: RegretClass,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub prerequisites: Vec<ObligationPredicate>,
}

impl Action {
    pub fn new(id: impl Into<String>, regret: RegretClass) -> Self {
        Action {
            id: id.into(),
            description: String::new(),
            regret,
            prerequisites: Vec::new(),
        }
    }

    pub fn described(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }

    pub fn requiring(mut self, predicate: ObligationPredicate) -> Self {
        self.prerequisites.push(predicate);
        self
    }
}

/// Why an action was refused.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "reason", rename_all = "snake_case")]
pub enum BlockReason {
    /// A high-regret action with no prerequisites. Silence is not permission.
    UndeclaredPrerequisites,
    /// A prerequisite names an obligation the graph does not contain.
    UnknownObligation { obligation: String },
    /// The graph could not be checked, so no state derived from it can be trusted.
    GraphUnverifiable { detail: String },
    /// Declared prerequisites are not in an acceptable state.
    PrerequisitesUnmet,
    /// An irreversible action with a mandatory obligation outstanding that it never declared.
    MandatoryObligationOutstanding {
        obligation: String,
        effective: ObligationState,
    },
}

/// One prerequisite that failed, with both what was recorded and what the graph allows it to mean.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UnmetPrerequisite {
    pub obligation: String,
    pub required: Vec<ObligationState>,
    pub recorded: ObligationState,
    pub effective: ObligationState,
    pub confidence: f64,
    pub min_confidence: f64,
    pub detail: String,
}

/// The gate decision. Serialisable because it is an audit record, not a boolean.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "gate", rename_all = "snake_case")]
pub enum Gate {
    Allowed {
        action: String,
        /// Obligations checked to reach this decision, so the permission can be replayed.
        checked: Vec<String>,
    },
    Blocked {
        action: String,
        reason: BlockReason,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        unmet: Vec<UnmetPrerequisite>,
    },
}

impl Gate {
    pub fn is_allowed(&self) -> bool {
        matches!(self, Gate::Allowed { .. })
    }

    pub fn is_blocked(&self) -> bool {
        !self.is_allowed()
    }

    pub fn block_reason(&self) -> Option<&BlockReason> {
        match self {
            Gate::Allowed { .. } => None,
            Gate::Blocked { reason, .. } => Some(reason),
        }
    }

    pub fn unmet(&self) -> &[UnmetPrerequisite] {
        match self {
            Gate::Allowed { .. } => &[],
            Gate::Blocked { unmet, .. } => unmet,
        }
    }
}

/// Decides whether an action may run against the current obligation graph.
///
/// The only function in this crate that returns a permission, and it is written to be boring:
/// every path that cannot prove the preconditions returns [`Gate::Blocked`].
pub fn may_perform(action: &Action, graph: &ObligationGraph) -> Gate {
    let effective = match graph.effective_states() {
        Ok(states) => states,
        Err(error) => {
            return Gate::Blocked {
                action: action.id.clone(),
                reason: BlockReason::GraphUnverifiable {
                    detail: error.to_string(),
                },
                unmet: Vec::new(),
            }
        }
    };

    if action.regret.requires_declared_prerequisites() && action.prerequisites.is_empty() {
        return Gate::Blocked {
            action: action.id.clone(),
            reason: BlockReason::UndeclaredPrerequisites,
            unmet: Vec::new(),
        };
    }

    let mut checked = Vec::with_capacity(action.prerequisites.len());
    let mut unmet = Vec::new();
    for predicate in &action.prerequisites {
        let Some(obligation) = graph.get(&predicate.obligation) else {
            return Gate::Blocked {
                action: action.id.clone(),
                reason: BlockReason::UnknownObligation {
                    obligation: predicate.obligation.clone(),
                },
                unmet: Vec::new(),
            };
        };
        checked.push(predicate.obligation.clone());

        let state = effective
            .get(&predicate.obligation)
            .copied()
            .unwrap_or(ObligationState::Unseen);
        let recorded = obligation.recorded_state();
        let confidence = obligation.confidence();

        if !predicate.accept.contains(&state) {
            let detail = if recorded == state {
                format!("obligation is `{}`", state.as_str())
            } else {
                format!(
                    "obligation was recorded `{}` but its dependencies cap it at `{}`",
                    recorded.as_str(),
                    state.as_str()
                )
            };
            unmet.push(UnmetPrerequisite {
                obligation: predicate.obligation.clone(),
                required: predicate.accepted_states(),
                recorded,
                effective: state,
                confidence,
                min_confidence: predicate.min_confidence,
                detail,
            });
        } else if confidence < predicate.min_confidence {
            unmet.push(UnmetPrerequisite {
                obligation: predicate.obligation.clone(),
                required: predicate.accepted_states(),
                recorded,
                effective: state,
                confidence,
                min_confidence: predicate.min_confidence,
                detail: format!(
                    "stated confidence {confidence} is below the declared floor {}",
                    predicate.min_confidence
                ),
            });
        }
    }

    if !unmet.is_empty() {
        return Gate::Blocked {
            action: action.id.clone(),
            reason: BlockReason::PrerequisitesUnmet,
            unmet,
        };
    }

    if action.regret.requires_full_mandatory_closure() {
        for id in graph.mandatory_ids() {
            let state = effective
                .get(id)
                .copied()
                .unwrap_or(ObligationState::Unseen);
            if !state.is_evidentially_discharged() {
                return Gate::Blocked {
                    action: action.id.clone(),
                    reason: BlockReason::MandatoryObligationOutstanding {
                        obligation: id.to_string(),
                        effective: state,
                    },
                    unmet: Vec::new(),
                };
            }
        }
    }

    Gate::Allowed {
        action: action.id.clone(),
        checked,
    }
}
