//! BioWorldline IR — blueprint 25.09.
//!
//! A worldline is an ordered sequence of [`crate::state::BioState`]s with the transitions between
//! them, plus the machinery 25.09 names: branches, reveal gates, alignment confidence, censoring and
//! a follow-up window.
//!
//! # The invariant worth having
//!
//! **A worldline cannot silently interleave states from different scopes.** Every state must refine
//! the worldline's declared scope, or [`Worldline::validate`] returns
//! [`WorldlineError::ScopeInterleaving`] naming the dimension. This is the failure that produces
//! confident nonsense: a longitudinal volume trajectory assembled from two coordinate frames, or two
//! genome builds, or two subjects, looks exactly like a real trajectory and every delta computed
//! from it is a difference between incomparable things. `bioprism-scope` already has the refinement
//! order; this module only insists that it be checked.
//!
//! **Comparing two worldlines requires their scopes to be comparable.** [`Worldline::comparable_with`]
//! takes the [`bioprism_scope::meet`] of the two scopes and refuses when it is empty, carrying the
//! [`bioprism_scope::EmptyReason`] the meet produced. Two worldlines whose scopes do not overlap
//! have no common ground on which a difference between them means anything.
//!
//! # What is deliberately not implemented
//!
//! - **No clock alignment.** 25.09 asks for "alignment confidence"; [`AlignmentConfidence`] carries a
//!   declared number and its method. Estimating alignment needs the events and a model of the two
//!   clocks, neither of which is here, and this crate reads no clock at all.
//! - **No survival analysis.** [`Censoring`] records what the blueprint asks to be explicit. It does
//!   not compute a risk set, a Kaplan-Meier estimate or a follow-up-adjusted rate.
//! - **No reveal enforcement at runtime.** [`RevealGate`] is checked *within* a worldline: a gated
//!   state may not appear before its reveal instant. Preventing a participant from reading a hidden
//!   value is `bioprism-weave`'s capsule projection, which has the recipient and the labels.

use crate::clock::Clock;
use crate::error::WorldlineError;
use crate::ids::{StateId, WorldlineId};
use crate::state::{BioState, Transition};
use bioprism_ids::WorldId;
use bioprism_scope::{meet, EmptyReason, Interval, Meet, ScopeKey, Timestamp};
use serde::{Deserialize, Serialize};

/// How confident the worldline is that its events are on a common timeline.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AlignmentConfidence {
    /// A declared number in `[0, 1]`. Declared by whoever assembled the worldline, not computed.
    pub confidence: f64,
    /// How it was arrived at, in prose, because the methods are not enumerable.
    pub method: String,
}

impl AlignmentConfidence {
    pub fn declared(confidence: f64, method: impl Into<String>) -> Self {
        AlignmentConfidence {
            confidence,
            method: method.into(),
        }
    }
}

/// How follow-up ended. 25.09: "Censoring is explicit."
///
/// There is no `Default` and no `Unknown`. An author must choose, because the difference between
/// "this subject had no event" and "we stopped looking" is the difference between a rate and a
/// wrong rate.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "censoring", rename_all = "snake_case")]
pub enum Censoring {
    /// Followed to the end of the window with the event observed, or the window genuinely complete.
    NotCensored,
    /// Follow-up ended at a known instant with no event yet observed.
    RightCensored { at: Timestamp },
    /// The event happened somewhere inside a window that was not observed continuously.
    IntervalCensored { after: Timestamp, before: Timestamp },
    /// Contact was lost. Distinct from right censoring, because loss is often informative.
    LostToFollowUp { last_contact: Timestamp },
}

/// A gate that holds a state back until an instant on the reveal clock.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RevealGate {
    pub gate_id: String,
    pub gated_state: StateId,
    pub reveal_at: Timestamp,
}

/// A fork of the worldline at a named state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Branch {
    pub branch_id: String,
    pub parent_state: StateId,
    pub worldline: WorldlineId,
}

/// A correction that supersedes a prior state without deleting it.
///
/// 25.09: "Temporal corrections preserve prior recorded beliefs." So a revision is an *append*:
/// [`Worldline::validate`] refuses a revision whose superseded state is no longer on the worldline.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Revision {
    pub revision_id: String,
    pub supersedes: StateId,
    pub replacement: StateId,
    pub reason: String,
    pub recorded_at: Timestamp,
}

/// An ordered sequence of states, with everything 25.09 requires around it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Worldline {
    pub worldline_id: WorldlineId,
    pub world_id: WorldId,
    /// The scope every state on this worldline must refine.
    pub scope: ScopeKey,
    /// Which clock the sequence is ordered on.
    pub ordering_clock: Clock,
    pub states: Vec<BioState>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub transitions: Vec<Transition>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub branches: Vec<Branch>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reveals: Vec<RevealGate>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub revisions: Vec<Revision>,
    pub alignment: AlignmentConfidence,
    pub censoring: Censoring,
    pub follow_up: Interval,
}

impl Worldline {
    pub fn new(
        worldline_id: WorldlineId,
        world_id: WorldId,
        scope: ScopeKey,
        ordering_clock: Clock,
        alignment: AlignmentConfidence,
        censoring: Censoring,
    ) -> Self {
        Worldline {
            worldline_id,
            world_id,
            scope,
            ordering_clock,
            states: Vec::new(),
            transitions: Vec::new(),
            branches: Vec::new(),
            reveals: Vec::new(),
            revisions: Vec::new(),
            alignment,
            censoring,
            follow_up: Interval::UNBOUNDED,
        }
    }

    pub fn then(mut self, state: BioState) -> Self {
        self.states.push(state);
        self
    }

    pub fn transitioning(mut self, transition: Transition) -> Self {
        self.transitions.push(transition);
        self
    }

    pub fn branching(mut self, branch: Branch) -> Self {
        self.branches.push(branch);
        self
    }

    pub fn gated_by(mut self, gate: RevealGate) -> Self {
        self.reveals.push(gate);
        self
    }

    pub fn revised_by(mut self, revision: Revision) -> Self {
        self.revisions.push(revision);
        self
    }

    pub fn followed_for(mut self, window: Interval) -> Self {
        self.follow_up = window;
        self
    }

    /// The instant a state carries on the worldline's ordering clock.
    ///
    /// `Decision` and `Reveal` have no field on [`BioState`], which 25.02 does not give them; a
    /// worldline ordered on those clocks falls back to record time and says so through
    /// [`Worldline::orders_on_a_carried_clock`].
    fn ordering_instant(&self, state: &BioState) -> Timestamp {
        match self.ordering_clock {
            Clock::Event => state.event_time,
            _ => state.record_time,
        }
    }

    /// False when the ordering clock is one [`BioState`] does not carry.
    ///
    /// A gap worth surfacing rather than hiding: 25.09 requires `decision_time` on a worldline and
    /// 25.02 gives a state only `event_time` and `record_time`, so a decision-ordered worldline is
    /// ordered on a proxy. See the crate documentation's list of §25 constructs that are named but
    /// never specified.
    pub fn orders_on_a_carried_clock(&self) -> bool {
        matches!(self.ordering_clock, Clock::Event | Clock::Record)
    }

    /// Every invariant 25.09 states, in the order a reader would want them.
    pub fn validate(&self) -> Result<(), WorldlineError> {
        self.validate_scopes()?;
        self.validate_order()?;
        self.validate_branches()?;
        self.validate_reveals()?;
        self.validate_revisions()
    }

    /// The interleaving check: every state sits inside the worldline's scope.
    pub fn validate_scopes(&self) -> Result<(), WorldlineError> {
        for state in &self.states {
            if !state.scope.refines(&self.scope) {
                let dimension = self
                    .scope
                    .iter()
                    .find(|(dimension, coarse)| {
                        !state
                            .scope
                            .get(dimension)
                            .is_some_and(|fine| fine.refines(coarse))
                    })
                    .map(|(dimension, _)| dimension.to_string())
                    .unwrap_or_else(|| "unknown".to_string());
                return Err(WorldlineError::ScopeInterleaving {
                    state: state.state_id.to_string(),
                    dimension,
                });
            }
        }
        Ok(())
    }

    fn validate_order(&self) -> Result<(), WorldlineError> {
        let mut previous: Option<Timestamp> = None;
        for state in &self.states {
            let at = self.ordering_instant(state);
            if let Some(previous) = previous {
                if at.as_nanos_utc() < previous.as_nanos_utc() {
                    return Err(WorldlineError::OutOfOrder {
                        state: state.state_id.to_string(),
                        clock: self.ordering_clock,
                        at: at.to_rfc3339(),
                        previous: previous.to_rfc3339(),
                    });
                }
            }
            previous = Some(at);
        }
        Ok(())
    }

    fn validate_branches(&self) -> Result<(), WorldlineError> {
        for branch in &self.branches {
            if !self.contains(&branch.parent_state) {
                return Err(WorldlineError::BranchParentMissing {
                    branch: branch.branch_id.clone(),
                    parent: branch.parent_state.to_string(),
                });
            }
        }
        Ok(())
    }

    fn validate_reveals(&self) -> Result<(), WorldlineError> {
        for gate in &self.reveals {
            let Some(state) = self.state(&gate.gated_state) else {
                continue;
            };
            let at = self.ordering_instant(state);
            if at.as_nanos_utc() < gate.reveal_at.as_nanos_utc() {
                return Err(WorldlineError::PrematureReveal {
                    state: state.state_id.to_string(),
                    gate: gate.gate_id.clone(),
                    at: at.to_rfc3339(),
                    reveal_at: gate.reveal_at.to_rfc3339(),
                });
            }
        }
        Ok(())
    }

    fn validate_revisions(&self) -> Result<(), WorldlineError> {
        for revision in &self.revisions {
            if !self.contains(&revision.supersedes) {
                return Err(WorldlineError::RevisionErasesHistory {
                    revision: revision.revision_id.clone(),
                    superseded: revision.supersedes.to_string(),
                });
            }
        }
        Ok(())
    }

    pub fn contains(&self, state_id: &StateId) -> bool {
        self.states.iter().any(|state| &state.state_id == state_id)
    }

    pub fn state(&self, state_id: &StateId) -> Option<&BioState> {
        self.states.iter().find(|state| &state.state_id == state_id)
    }

    /// Whether two worldlines may be compared at all, and on what common scope.
    ///
    /// Delegates to [`bioprism_scope::meet`]: the overlap of two scopes is already defined there and
    /// re-deriving it would be a second partial order to keep in agreement with the first. Two
    /// worldlines whose scopes do not overlap have no common ground on which a difference between
    /// them means anything, so the refusal names the dimension where the overlap failed.
    pub fn comparable_with(&self, other: &Worldline) -> Result<ScopeKey, WorldlineError> {
        match meet(&self.scope, &other.scope) {
            Meet::Scope(common) => Ok(common),
            Meet::Empty { dimension, reason } => Err(WorldlineError::ScopesDisjoint {
                left: self.worldline_id.to_string(),
                right: other.worldline_id.to_string(),
                reason: format!("dimension {dimension:?}: {}", describe_empty(reason)),
            }),
            Meet::Conflict { dimension, .. } => Err(WorldlineError::ScopesDisagree {
                left: self.worldline_id.to_string(),
                right: other.worldline_id.to_string(),
                dimension,
            }),
        }
    }
}

fn describe_empty(reason: EmptyReason) -> &'static str {
    match reason {
        EmptyReason::UnequalExactValues => "the two worldlines bind it to different values",
        EmptyReason::DisjointSets => "their permitted value sets do not intersect",
        EmptyReason::EmptyInterval => "their time windows do not overlap",
    }
}
