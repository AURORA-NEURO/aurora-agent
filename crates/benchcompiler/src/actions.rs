//! Candidate action set reconstruction.
//!
//! Blueprint 06.04. The question is what the agent *could* have done at a historical decision, and
//! the failure mode is hindsight: an option that is obvious once you know how the run ended was
//! very likely not obvious at the time. 06.04 calls the rule the **hindsight firewall** — future
//! events and hidden grader state "may be used to validate candidates but cannot be shown to a
//! candidate agent or used to claim an option was obvious at decision time".
//!
//! Here the firewall is a type, not a convention. Every candidate carries a [`Provenance`] saying
//! where it came from, and [`CandidateActionSet::visible_to_agent`] returns only those an agent
//! standing at the decision could have derived. Future-sourced candidates remain in the set, and
//! [`CandidateActionSet::validation_only`] returns them, because 06.04 explicitly permits them for
//! validation. What is refused, with [`ActionError::HindsightLeak`], is a candidate that *claims*
//! it was visible at decision time while citing a later step — a false provenance, which is the
//! only way the firewall can be breached without anyone noticing.
//!
//! ## Semantic properties
//!
//! 06.04 asks for both concrete actions and semantic properties such as "inspect idempotency
//! semantics before editing", so that several different tool calls can satisfy one intent. Both are
//! carried; the semantic property is what a set-valued oracle should grade against, and the
//! concrete action is what makes the option checkable for feasibility.
//!
//! ## What is deliberately not implemented
//!
//! No controlled generation of new options from the visible state. 06.04 lists it as a source and
//! it needs a model; a generator here would be inventing counterfactual actions and then scoring an
//! agent against its own imagination. Options come from the trace, from declared tool schemas, from
//! architecture policy, or from peer trajectories — all recorded, all attributable.
//!
//! Coverage is *reported*, not enforced. 06.04 says the set "need not be exhaustive to support
//! property oracles", so [`Coverage`] tells a reviewer what is present and lets them decide.

use crate::error::ActionError;
use bioprism_trace::{EventKind, Trace};
use serde::{Deserialize, Serialize};

/// Where a candidate option came from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "source", rename_all = "snake_case")]
pub enum Provenance {
    /// Recorded as an alternative at the decision itself.
    RecordedAlternative,
    /// Derivable from evidence visible at or before the decision.
    VisibleAtDecisionTime { from_step: usize },
    /// Enumerated from a tool's declared schema.
    ToolSchema { tool: String },
    /// Permitted or required by the architecture's own policy.
    ArchitecturePolicy { policy: String },
    /// Taken from another run that faced the same state.
    PeerTrajectory { trace_id: String, step: usize },
    /// Derived from something that happened after the decision. Usable to validate the set, never
    /// to claim the option was available.
    FromFuture { from_step: usize },
}

impl Provenance {
    /// Whether an agent standing at the decision could have had this option.
    pub fn behind_the_firewall(&self) -> bool {
        !matches!(self, Provenance::FromFuture { .. })
    }
}

/// Whether an option could actually have been taken.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum Feasibility {
    Feasible,
    /// 06.04: infeasible options stay as diagnostic hypotheses but are not acceptable actions.
    Infeasible { reason: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CandidateAction {
    pub label: String,
    /// What the option accomplishes, independent of which tool call realises it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semantic_property: Option<String>,
    pub provenance: Provenance,
    pub feasibility: Feasibility,
    /// Whether a reviewer considers this a defensible choice. Used only for coverage reporting.
    pub strong: bool,
}

impl CandidateAction {
    pub fn new(label: impl Into<String>, provenance: Provenance) -> Self {
        CandidateAction {
            label: label.into(),
            semantic_property: None,
            provenance,
            feasibility: Feasibility::Feasible,
            strong: false,
        }
    }

    pub fn accomplishing(mut self, property: impl Into<String>) -> Self {
        self.semantic_property = Some(property.into());
        self
    }

    pub fn infeasible(mut self, reason: impl Into<String>) -> Self {
        self.feasibility = Feasibility::Infeasible {
            reason: reason.into(),
        };
        self
    }

    /// Marks the option as one a competent agent could defensibly have taken.
    pub fn strong(mut self) -> Self {
        self.strong = true;
        self
    }
}

/// What a reviewer needs in order to judge whether the set is adequate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Coverage {
    pub total: usize,
    pub visible_at_decision_time: usize,
    pub validation_only: usize,
    pub feasible: usize,
    pub strong: usize,
    /// Feasible options behind the firewall that nobody marked strong. 06.04 wants at least one
    /// strong action and the major plausible wrong alternatives represented.
    pub plausible_wrong_alternatives: usize,
    pub adequate: bool,
}

/// The options available at one historical decision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CandidateActionSet {
    pub trace_id: String,
    pub decision_step: usize,
    actions: Vec<CandidateAction>,
}

impl CandidateActionSet {
    /// Starts a set at a decision, seeded with whatever alternatives the trace recorded.
    ///
    /// Refuses a step the agent did not control. `bioprism_trace::EventKind::is_decision_bearing`
    /// is the authority; an action set at an observation would enumerate options for something the
    /// agent never chose.
    pub fn reconstruct(trace: &Trace, decision_step: usize) -> Result<Self, ActionError> {
        let event = trace
            .at(decision_step)
            .ok_or(ActionError::StepNotInTrace { step: decision_step })?;
        if !event.kind.is_decision_bearing() {
            return Err(ActionError::NotDecisionBearing {
                step: decision_step,
                kind: event.kind.as_str(),
            });
        }

        let mut actions = Vec::new();
        if let Some(taken) = event
            .payload
            .get("tool")
            .or_else(|| event.payload.get("action"))
            .and_then(|value| value.as_str())
        {
            actions.push(CandidateAction::new(taken, Provenance::RecordedAlternative));
        }
        for alternative in event
            .payload
            .get("alternatives")
            .and_then(|value| value.as_array())
            .into_iter()
            .flatten()
        {
            let label = alternative
                .get("tool")
                .and_then(|value| value.as_str())
                .or_else(|| alternative.as_str())
                .unwrap_or("<unlabelled alternative>");
            actions.push(CandidateAction::new(label, Provenance::RecordedAlternative));
        }

        Ok(CandidateActionSet {
            trace_id: trace.trace_id.clone(),
            decision_step,
            actions,
        })
    }

    /// Adds an option, checking its provenance against the firewall.
    ///
    /// A candidate that claims to have been visible at decision time while citing a later step is
    /// rejected. Marking it [`Provenance::FromFuture`] instead is always allowed — the firewall
    /// separates validation evidence from agent-visible options, it does not discard either.
    pub fn add(&mut self, action: CandidateAction) -> Result<(), ActionError> {
        let claimed = match &action.provenance {
            Provenance::VisibleAtDecisionTime { from_step } => Some(*from_step),
            Provenance::PeerTrajectory { step, .. } => Some(*step),
            _ => None,
        };
        if let Some(from_step) = claimed {
            if from_step > self.decision_step {
                return Err(ActionError::HindsightLeak {
                    action: action.label,
                    from_step,
                    decision_step: self.decision_step,
                });
            }
        }
        self.actions.push(action);
        Ok(())
    }

    pub fn all(&self) -> &[CandidateAction] {
        &self.actions
    }

    /// The options that may be shown to a candidate agent.
    pub fn visible_to_agent(&self) -> Vec<&CandidateAction> {
        self.actions
            .iter()
            .filter(|action| action.provenance.behind_the_firewall())
            .collect()
    }

    /// The options that exist only to validate the set. Never shown, never graded against.
    pub fn validation_only(&self) -> Vec<&CandidateAction> {
        self.actions
            .iter()
            .filter(|action| !action.provenance.behind_the_firewall())
            .collect()
    }

    /// Options that are both behind the firewall and actually takeable.
    pub fn acceptable(&self) -> Vec<&CandidateAction> {
        self.actions
            .iter()
            .filter(|action| {
                action.provenance.behind_the_firewall()
                    && action.feasibility == Feasibility::Feasible
            })
            .collect()
    }

    pub fn coverage(&self) -> Coverage {
        let visible = self.visible_to_agent();
        let feasible = self.acceptable();
        let strong = feasible.iter().filter(|action| action.strong).count();
        let wrong = feasible.len() - strong;
        Coverage {
            total: self.actions.len(),
            visible_at_decision_time: visible.len(),
            validation_only: self.actions.len() - visible.len(),
            feasible: feasible.len(),
            strong,
            plausible_wrong_alternatives: wrong,
            adequate: strong >= 1 && wrong >= 1,
        }
    }
}

/// Restates why observations cannot host an action set, for callers building error messages.
pub fn why_not_decision_bearing(kind: EventKind) -> Option<&'static str> {
    if kind.is_decision_bearing() {
        return None;
    }
    Some("the agent had no alternative at this step; enumerating options for it would invent a decision that was never made")
}
