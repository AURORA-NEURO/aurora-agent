//! Operational semantics: a small-step evaluator over WeaveIR.
//!
//! Blueprint 23.34. Its configuration is `Σ = ⟨W, E, C, A, B, T, P, Q, H⟩` and its transition is
//! `Σ --event--> Σ'`; [`Machine`] carries all nine components under those names and [`Machine::step`]
//! is that arrow. Its eight safety properties are [`Invariant`], each one an executable check run
//! after every step rather than a comment claiming the property holds.
//!
//! # Determinism is the property that matters
//!
//! The same program over the same inputs produces the same event sequence and the same digest.
//! Everything about this module is subordinate to that: transitions fire in the order the state
//! graph declares them, event identifiers are derived from content rather than minted, `par`
//! interleaves in source order, and there is no clock and no randomness anywhere in the crate. A
//! trace that differed between two runs would make every other guarantee unfalsifiable, because
//! there would be nothing stable to compare.
//!
//! # What 23.34 asks for that this cannot do
//!
//! 23.34's liveness targets are stated for "the statically analyzable subset", and it says outright
//! that an implementation "must not claim universal deadlock freedom for dynamic opaque agents".
//! Taking that seriously, [`LivenessReport`] *reports* rather than asserts, and three of its
//! targets are not achieved here at all:
//!
//! - **"Bounded loops terminate"** is not checked. `repeat until <expression>` bounds depend on
//!   runtime values, and this crate has no expression evaluator over world state. A loop bound is
//!   carried into the IR as a transition guard string and never evaluated. Claiming termination
//!   would be false.
//! - **"No protocol waits forever without timeout"** is checked only in the weak form that a
//!   commitment with a deadline gets a `Timeout` monitor. Whether the timeout ever fires needs a
//!   clock, which would destroy determinism, so it is not modelled.
//! - **"Participant departure triggers a handler"** is not checked, because WeaveIR as compiled
//!   here has no departure event to trigger on. 23.20's dynamic topology is out of scope.
//!
//! Also absent, deliberately: no scheduler, no concurrency, no network, no distributed execution,
//! no tool invocation, no model call. The evaluator moves a token through a graph and records what
//! the move would have committed to; it never performs an effect. That is why replay is the default
//! mode and live mode has to be asked for by name.

use crate::diagnostic::{Diagnostic, Span};
use crate::ir::*;
use bioprism_ids::ContentHash;
use bioprism_weave::{Budget, BudgetError, Resource};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

/// Whether the evaluator is replaying history or authorised to touch the world.
///
/// 23.34's replay-safety property: "Historical replay cannot invoke production side effects unless
/// a separately authorized live mode is explicitly selected." [`ExecutionMode::Replay`] is the
/// default and there is no way to fall through from it to live, which is the same shape as
/// `bioprism-prism`'s `ReplayHost` having no source field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionMode {
    #[default]
    Replay,
    Live,
}

/// 23.34's safety properties, as checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Invariant {
    AuthoritySafety,
    DelegationAttenuation,
    BudgetConservation,
    CommitmentAccountability,
    EpistemicIntegrity,
    InformationNonEscalation,
    CausalIntegrity,
    ReplaySafety,
}

impl Invariant {
    pub fn as_str(self) -> &'static str {
        match self {
            Invariant::AuthoritySafety => "authority safety",
            Invariant::DelegationAttenuation => "delegation attenuation",
            Invariant::BudgetConservation => "budget conservation",
            Invariant::CommitmentAccountability => "commitment accountability",
            Invariant::EpistemicIntegrity => "epistemic integrity",
            Invariant::InformationNonEscalation => "information non-escalation",
            Invariant::CausalIntegrity => "causal integrity",
            Invariant::ReplaySafety => "replay safety",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InvariantViolation {
    pub invariant: Invariant,
    pub detail: String,
    pub at_event: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SemanticsError {
    #[error("{} violated at event {}: {}", violation.invariant.as_str(), violation.at_event, violation.detail)]
    InvariantViolated { violation: InvariantViolation },

    #[error("transition `{transition}` mutates the world ({effects:?}) and this run is a replay; select live mode explicitly to permit it")]
    ReplayWouldMutateWorld {
        transition: String,
        effects: Vec<String>,
    },

    #[error("the policy allocates no {resource:?}, so no transition can be charged for one")]
    NoBudgetAllocated { resource: Resource },

    #[error("transition `{transition}` cannot be charged: {source}")]
    BudgetRefused {
        transition: String,
        source: BudgetError,
    },

    #[error("state `{0}` is not in the state graph")]
    UnknownState(String),

    #[error(transparent)]
    Ir(#[from] IrError),
}

impl Diagnostic for SemanticsError {
    fn code(&self) -> &'static str {
        match self {
            SemanticsError::InvariantViolated { .. } => "WEAVE-E5001",
            SemanticsError::ReplayWouldMutateWorld { .. } => "WEAVE-E5002",
            SemanticsError::NoBudgetAllocated { .. } => "WEAVE-E5003",
            SemanticsError::BudgetRefused { .. } => "WEAVE-E5004",
            SemanticsError::UnknownState(_) => "WEAVE-E5005",
            SemanticsError::Ir(error) => error.code(),
        }
    }

    /// Always `None`. A semantics failure is a property of a run, not of a source location; the
    /// transition identifier in the message is what a reader needs, and inventing a span for it
    /// would point at the wrong thing.
    fn span(&self) -> Option<Span> {
        None
    }
}

/// One entry of the epistemic ledger, `E`.
///
/// A claim is never removed when challenged: 23.08 and 23.34's epistemic-integrity property both
/// require claim, assumption and verified proposition to remain distinct states, which they cannot
/// do if a challenge overwrites a claim.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EpistemicRecord {
    pub id: String,
    pub claimant: String,
    pub proposition: String,
    pub challenges: Vec<String>,
}

/// One entry of the commitment ledger, `C`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommitmentRecord {
    pub id: String,
    pub debtor: String,
    pub creditor: String,
    pub discharged: bool,
    pub quality_predicates: Vec<String>,
}

/// One grant of the authority graph, `A`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GrantRecord {
    pub id: String,
    pub holder: String,
    pub effects: BTreeSet<String>,
    pub parent: Option<String>,
}

/// A recorded run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Trace {
    pub program_id: String,
    pub events: Vec<WeaveEvent>,
    /// Digest over the canonical bytes of `events`. Two runs of one program agree here or the
    /// evaluator is not deterministic.
    pub digest: String,
}

/// What a completed run can and cannot say about liveness (23.34).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LivenessReport {
    pub messages_left_unconsumed: usize,
    pub commitments_left_open: Vec<String>,
    /// Non-terminal states with no outgoing transition: a wait with no way out.
    pub states_without_exit: Vec<String>,
    /// States no transition ever enters. 23.03 phase 6 asks for this and it is the one piece of
    /// choreography analysis implemented here.
    pub unreachable_states: Vec<String>,
    /// Always false. Stated as a field so a caller cannot read the absence of violations as a proof
    /// this module does not offer.
    pub deadlock_freedom_proven: bool,
}

/// The outcome of one step.
#[derive(Debug, Clone, PartialEq)]
pub enum Step {
    /// A transition fired and produced an event.
    ///
    /// The event is boxed: it is an order of magnitude larger than the halt variant, and a `Step`
    /// is returned from every call to [`Machine::step`].
    Fired {
        transition: String,
        event: Box<WeaveEvent>,
    },
    /// No transition is enabled from the current state; the run is over.
    Halted { state: String },
}

/// 23.34's global configuration `Σ`, plus the program it is running.
///
/// Not `Clone`, and not by omission: it owns a `bioprism_weave::Budget`, which is affine. A
/// configuration that could be copied would let a caller run two futures off one allowance, which
/// is exactly the duplication 23.16 forbids.
pub struct Machine {
    ir: WeaveIr,
    mode: ExecutionMode,
    thread_id: String,
    /// `W`
    world: BTreeMap<String, Value>,
    /// `E`
    epistemic: Vec<EpistemicRecord>,
    /// `C`
    commitments: BTreeMap<String, CommitmentRecord>,
    /// `A`
    authority: Vec<GrantRecord>,
    /// `B`
    budget: Budget,
    /// `T`
    topology: BTreeMap<String, String>,
    /// `P`
    state: String,
    /// `Q`
    queue: VecDeque<String>,
    /// `H`
    history: Vec<WeaveEvent>,
    logical_clock: u64,
    issued: BTreeMap<Resource, u64>,
    fired: BTreeSet<String>,
    thread_label: SecurityLabel,
    violations: Vec<InvariantViolation>,
}

impl Machine {
    /// Loads a compiled program. The initial configuration is derived entirely from the IR, so two
    /// machines built from the same IR are indistinguishable.
    pub fn load(ir: WeaveIr, mode: ExecutionMode, thread_id: impl Into<String>) -> Self {
        let mut budget = Budget::new();
        let mut issued = BTreeMap::new();
        for policy in ir.policies.values() {
            for allowance in &policy.budgets {
                budget = budget.with(allowance.resource, allowance.limit);
                *issued.entry(allowance.resource).or_insert(0) += allowance.limit;
            }
        }

        let authority = ir
            .roles
            .iter()
            .map(|role| GrantRecord {
                id: format!("grant:{}", role.id),
                holder: role.id.clone(),
                effects: role.requires_effects.iter().cloned().collect(),
                parent: None,
            })
            .collect();

        let topology = ir
            .participants
            .iter()
            .map(|participant| (participant.id.clone(), participant.role.clone()))
            .collect();

        let queue = ir
            .state_graph
            .transitions
            .iter()
            .map(|transition| transition.id.clone())
            .collect();

        let state = ir.choreography.initial_state.clone();
        Machine {
            ir,
            mode,
            thread_id: thread_id.into(),
            world: BTreeMap::new(),
            epistemic: Vec::new(),
            commitments: BTreeMap::new(),
            authority,
            budget,
            topology,
            state,
            queue,
            history: Vec::new(),
            logical_clock: 0,
            issued,
            fired: BTreeSet::new(),
            thread_label: SecurityLabel::new("public"),
            violations: Vec::new(),
        }
    }

    pub fn state(&self) -> &str {
        &self.state
    }

    /// `T`: which participant occupies which role.
    pub fn topology(&self) -> &BTreeMap<String, String> {
        &self.topology
    }

    pub fn history(&self) -> &[WeaveEvent] {
        &self.history
    }

    pub fn epistemic(&self) -> &[EpistemicRecord] {
        &self.epistemic
    }

    pub fn commitments(&self) -> &BTreeMap<String, CommitmentRecord> {
        &self.commitments
    }

    pub fn remaining(&self, resource: Resource) -> u64 {
        self.budget.remaining(resource)
    }

    /// One step of `Σ --event--> Σ'`.
    ///
    /// The enabled transition chosen is the first one the state graph declares out of the current
    /// state that has not already fired. Declaration order is the tie-break, and it is the whole of
    /// the scheduling policy.
    pub fn step(&mut self) -> Result<Step, SemanticsError> {
        let Some(transition) = self
            .ir
            .state_graph
            .outgoing(&self.state)
            .into_iter()
            .find(|transition| !self.fired.contains(&transition.id))
            .cloned()
        else {
            return Ok(Step::Halted {
                state: self.state.clone(),
            });
        };

        if self.mode == ExecutionMode::Replay && transition.effects.mutates_world() {
            return Err(SemanticsError::ReplayWouldMutateWorld {
                transition: transition.id.clone(),
                effects: transition.effects.mutating.clone(),
            });
        }

        self.charge(&transition)?;

        let label = self.label_of(&transition.actor_role);
        let payload = json!({
            "transition": transition.id,
            "act": transition.act,
            "payloadType": transition.payload_type,
            "guard": transition.guard,
        });
        let payload_ref = content_ref(&payload)?;
        self.logical_clock += 1;
        let event = WeaveEvent {
            weave_version: WEAVE_EVENT_VERSION.to_string(),
            event_id: derive_event_id(&self.thread_id, self.logical_clock, &payload_ref),
            event_type: format!("aurora.weave.act.{}.v1", transition.act),
            source: if transition.actor_role.is_empty() {
                "kernel".to_string()
            } else {
                format!("role:{}", transition.actor_role)
            },
            thread_id: self.thread_id.clone(),
            program_id: self.ir.program_id.clone(),
            choreography_state: transition.from.clone(),
            logical_clock: self.logical_clock,
            causal_parents: self
                .history
                .last()
                .map(|previous| vec![previous.event_id.clone()])
                .unwrap_or_default(),
            time: None,
            schema: transition.payload_type.clone(),
            security_label: label.clone(),
            payload_ref,
            idempotency_key: format!(
                "{}:{}:{}",
                self.thread_id, transition.id, self.logical_clock
            ),
        };

        self.apply_ledger_effects(&transition, &event);
        self.apply_authority_effects(&transition);
        for key in &transition.effects.world {
            self.world.insert(key.clone(), json!(transition.id));
        }
        if SecurityLabel::rank(&label.level) > SecurityLabel::rank(&self.thread_label.level) {
            self.thread_label = label;
        }

        self.fired.insert(transition.id.clone());
        self.queue.retain(|queued| queued != &transition.id);
        self.history.push(event.clone());
        self.state = transition.to.clone();

        if let Some(violation) = self.check_invariants().into_iter().next() {
            self.violations.push(violation.clone());
            return Err(SemanticsError::InvariantViolated { violation });
        }

        Ok(Step::Fired {
            transition: transition.id,
            event: Box::new(event),
        })
    }

    /// Runs to a halt, returning the trace and its digest.
    ///
    /// Bounded by the number of transitions in the graph, because a transition fires at most once.
    /// The bound is structural, not a step limit: an evaluator with a step cap would silently turn
    /// a non-terminating program into a terminating one.
    pub fn run(&mut self) -> Result<Trace, SemanticsError> {
        while let Step::Fired { .. } = self.step()? {}
        let events = serde_json::to_value(&self.history)
            .map_err(|error| IrError::Encoding(error.to_string()))?;
        let digest = ContentHash::of_value(&events)
            .map_err(|error| IrError::Canonical(error.to_string()))?
            .as_str()
            .to_string();
        Ok(Trace {
            program_id: self.ir.program_id.clone(),
            events: self.history.clone(),
            digest,
        })
    }

    /// Charges one tool call to the program budget.
    ///
    /// Only tool calls. Token cost depends on a model this crate never calls, and inventing a
    /// per-transition token figure would produce a budget report that looks measured and is not.
    fn charge(&mut self, transition: &TransitionIr) -> Result<(), SemanticsError> {
        if !self.issued.contains_key(&Resource::ToolCalls) {
            return Err(SemanticsError::NoBudgetAllocated {
                resource: Resource::ToolCalls,
            });
        }
        self.budget
            .spend(Resource::ToolCalls, 1)
            .map_err(|source| SemanticsError::BudgetRefused {
                transition: transition.id.clone(),
                source,
            })?;
        Ok(())
    }

    fn label_of(&self, role: &str) -> SecurityLabel {
        self.ir
            .roles
            .iter()
            .find(|declared| declared.id == role)
            .map(|declared| declared.clearance.clone())
            .unwrap_or_else(|| SecurityLabel::new("public"))
    }

    fn apply_ledger_effects(&mut self, transition: &TransitionIr, event: &WeaveEvent) {
        for entry in &transition.effects.ledger {
            match entry.as_str() {
                "create_commitment" => {
                    self.commitments.insert(
                        transition.id.clone(),
                        CommitmentRecord {
                            id: transition.id.clone(),
                            debtor: transition.actor_role.clone(),
                            creditor: transition.actor_role.clone(),
                            discharged: false,
                            quality_predicates: transition.guard.clone(),
                        },
                    );
                }
                "discharge_commitment" => {
                    for commitment in self.commitments.values_mut() {
                        if !commitment.discharged {
                            commitment.discharged = true;
                            commitment.quality_predicates = transition.guard.clone();
                            break;
                        }
                    }
                }
                _ => {}
            }
        }
        match transition.act.as_str() {
            "claim" => self.epistemic.push(EpistemicRecord {
                id: event.event_id.clone(),
                claimant: event.source.clone(),
                proposition: transition.payload_type.clone(),
                challenges: Vec::new(),
            }),
            "challenge" => {
                if let Some(target) = self
                    .epistemic
                    .iter_mut()
                    .rev()
                    .find(|record| record.challenges.is_empty())
                {
                    target.challenges.push(event.event_id.clone());
                }
            }
            _ => {}
        }
    }

    fn apply_authority_effects(&mut self, transition: &TransitionIr) {
        for entry in &transition.effects.authority {
            let parent = self
                .authority
                .iter()
                .find(|grant| grant.holder == transition.actor_role)
                .map(|grant| (grant.id.clone(), grant.effects.clone()));
            let (parent_id, parent_effects) = match parent {
                Some(found) => found,
                None => continue,
            };
            let effects: BTreeSet<String> = match entry.as_str() {
                // Attenuation, not amplification: the child receives the world effects this
                // transition actually needs, intersected with what the parent holds.
                "attenuate" => transition
                    .effects
                    .world
                    .iter()
                    .filter(|effect| parent_effects.contains(*effect))
                    .cloned()
                    .collect(),
                "issue_grant" => parent_effects.clone(),
                _ => continue,
            };
            self.authority.push(GrantRecord {
                id: format!("{}:{}", parent_id, transition.id),
                holder: transition.actor_role.clone(),
                effects,
                parent: Some(parent_id),
            });
        }
    }

    /// Runs all eight of 23.34's safety properties against the current configuration.
    pub fn check_invariants(&self) -> Vec<InvariantViolation> {
        let mut violations = Vec::new();
        let at_event = self.history.len();

        // Authority safety: no world-mutating transition without a covering grant.
        for transition in &self.ir.state_graph.transitions {
            if !self.fired.contains(&transition.id) || !transition.effects.mutates_world() {
                continue;
            }
            let covered = self.authority.iter().any(|grant| {
                grant.holder == transition.actor_role
                    && transition
                        .effects
                        .world
                        .iter()
                        .all(|effect| grant.effects.contains(effect))
            });
            if !covered {
                violations.push(InvariantViolation {
                    invariant: Invariant::AuthoritySafety,
                    detail: format!(
                        "`{}` mutates {:?} with no covering grant for role `{}`",
                        transition.id, transition.effects.world, transition.actor_role
                    ),
                    at_event,
                });
            }
        }

        // Delegation attenuation: a child grant never exceeds its parent.
        for grant in &self.authority {
            let Some(parent_id) = &grant.parent else {
                continue;
            };
            let Some(parent) = self
                .authority
                .iter()
                .find(|candidate| &candidate.id == parent_id)
            else {
                continue;
            };
            let excess: Vec<&String> = grant.effects.difference(&parent.effects).collect();
            if !excess.is_empty() {
                violations.push(InvariantViolation {
                    invariant: Invariant::DelegationAttenuation,
                    detail: format!("grant `{}` adds {excess:?} to its parent", grant.id),
                    at_event,
                });
            }
        }

        // Budget conservation: consumed plus remaining never exceeds issued.
        for (resource, issued) in &self.issued {
            let remaining = self.budget.remaining(*resource);
            if remaining > *issued {
                violations.push(InvariantViolation {
                    invariant: Invariant::BudgetConservation,
                    detail: format!("{resource:?} shows {remaining} remaining of {issued} issued"),
                    at_event,
                });
            }
        }

        // Commitment accountability: an identifiable debtor and creditor for every commitment.
        for commitment in self.commitments.values() {
            if commitment.debtor.is_empty() || commitment.creditor.is_empty() {
                violations.push(InvariantViolation {
                    invariant: Invariant::CommitmentAccountability,
                    detail: format!("commitment `{}` has no identifiable party", commitment.id),
                    at_event,
                });
            }
        }

        // Epistemic integrity: a challenged claim is still a claim.
        for record in &self.epistemic {
            if record.proposition.is_empty() {
                violations.push(InvariantViolation {
                    invariant: Invariant::EpistemicIntegrity,
                    detail: format!("claim `{}` lost its proposition", record.id),
                    at_event,
                });
            }
        }

        // Information non-escalation: every actor dominates the thread's accumulated label.
        if let Some(event) = self.history.last() {
            let actor = event
                .source
                .strip_prefix("role:")
                .unwrap_or(event.source.as_str());
            let held = self.label_of(actor);
            if !event.source.starts_with("kernel") && !held.dominates(&self.thread_label) {
                violations.push(InvariantViolation {
                    invariant: Invariant::InformationNonEscalation,
                    detail: format!(
                        "`{}` is cleared for {} but the thread has accumulated {}",
                        actor, held.level, self.thread_label.level
                    ),
                    at_event,
                });
            }
        }

        // Causal integrity: every event but the first names its predecessor and the state it left.
        for (index, event) in self.history.iter().enumerate() {
            let expected: Vec<String> = if index == 0 {
                Vec::new()
            } else {
                vec![self.history[index - 1].event_id.clone()]
            };
            if event.causal_parents != expected {
                violations.push(InvariantViolation {
                    invariant: Invariant::CausalIntegrity,
                    detail: format!("event {index} does not name its causal parent"),
                    at_event,
                });
            }
            if event.logical_clock != (index as u64) + 1 {
                violations.push(InvariantViolation {
                    invariant: Invariant::CausalIntegrity,
                    detail: format!("event {index} has logical clock {}", event.logical_clock),
                    at_event,
                });
            }
        }

        // Replay safety: nothing in a replayed history touched the world.
        if self.mode == ExecutionMode::Replay {
            for transition in &self.ir.state_graph.transitions {
                if self.fired.contains(&transition.id) && transition.effects.mutates_world() {
                    violations.push(InvariantViolation {
                        invariant: Invariant::ReplaySafety,
                        detail: format!("`{}` mutated the world during a replay", transition.id),
                        at_event,
                    });
                }
            }
        }

        violations
    }

    /// What this run can say about 23.34's liveness targets. See the module docs for what it cannot.
    pub fn liveness(&self) -> LivenessReport {
        let reachable: BTreeSet<&str> = self
            .ir
            .state_graph
            .transitions
            .iter()
            .map(|transition| transition.to.as_str())
            .chain(std::iter::once(self.ir.choreography.initial_state.as_str()))
            .collect();
        let terminal: BTreeSet<&str> = self
            .ir
            .choreography
            .terminal_states
            .iter()
            .map(String::as_str)
            .collect();

        LivenessReport {
            messages_left_unconsumed: self.queue.len(),
            commitments_left_open: self
                .commitments
                .values()
                .filter(|commitment| !commitment.discharged)
                .map(|commitment| commitment.id.clone())
                .collect(),
            states_without_exit: self
                .ir
                .state_graph
                .nodes
                .iter()
                .filter(|node| node.enabled_acts.is_empty() && !terminal.contains(node.id.as_str()))
                .map(|node| node.id.clone())
                .collect(),
            unreachable_states: self
                .ir
                .state_graph
                .nodes
                .iter()
                .filter(|node| !reachable.contains(node.id.as_str()))
                .map(|node| node.id.clone())
                .collect(),
            deadlock_freedom_proven: false,
        }
    }
}
