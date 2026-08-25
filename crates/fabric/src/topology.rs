//! Dynamic team topology: spawn, fuse, split and retire.
//!
//! Blueprint 23.20.
//!
//! # Two rules with teeth
//!
//! **"Retirement requires state and obligation handoff."** [`Topology::apply`] refuses a
//! [`TopologyAction::Retire`] while the participant holds an open commitment, and names it.
//! [`TopologyAction::Evict`] is the variant that does not refuse — eviction is for a participant
//! that is compromised or gone, and its commitments become
//! [`TopologyChange::stranded_commitments`] rather than quietly disappearing. The difference
//! between the two actions *is* whether the obligations were handed off, and that is why 23.20
//! lists them separately.
//!
//! **"Fusion does not create a new source of authority."** After a fuse, the molecule's capability
//! set is checked against the union of its members'. A fusion that gained a capability is
//! [`TopologyError::FusionCreatedAuthority`], which is the same law
//! [`crate::algebra::check_authority_attenuation`] states over a composition, applied to the one
//! operation where a reader might expect an exception.
//!
//! # Thrashing prevention is arithmetic, not vibes
//!
//! 23.20 lists seven mechanisms. [`Controller`] implements the four that are computable from a
//! change history with a logical step counter — minimum dwell, cooldown after a failed spawn,
//! bounded changes per window, and a required reason code — and names the three it does not
//! (hysteresis thresholds, reconfiguration cost, stable role identifiers across rebinding) in
//! [`Controller::unimplemented_mechanisms`], because a partial anti-thrashing policy that presents
//! as complete is worse than none.
//!
//! # Not implemented
//!
//! No controller policy: 23.20's controller takes ten inputs including capability posteriors and
//! historical PRISM results, and [`Controller`] does not decide *whether* to spawn — it decides
//! whether a proposed change is admissible. Choosing is somebody's optimiser and not this crate's.
//! No fault domains, no isolation, no quarantine; those need a runtime.

use crate::contract::ComponentId;
use bioprism_weave::Capability;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// 23.20's eleven topology patterns.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Pattern {
    Pipeline,
    Star,
    Hierarchy,
    Mesh,
    Blackboard,
    Jury,
    Market,
    MapReduce,
    Saga,
    Swarm,
    Hybrid,
}

/// 23.20's ten topology actions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "action")]
pub enum TopologyAction {
    Spawn {
        role: String,
        participant: ComponentId,
        authority: BTreeSet<Capability>,
    },
    Admit {
        participant: ComponentId,
    },
    /// Clean departure. Refused while obligations are open.
    Retire {
        participant: ComponentId,
    },
    /// Forced departure. Permitted while obligations are open, and strands them visibly.
    Evict {
        participant: ComponentId,
        reason: String,
    },
    Fuse {
        members: BTreeSet<ComponentId>,
        molecule: ComponentId,
        /// What the fused molecule asks to hold. Checked against the union of its members', which
        /// is the only place the "fusion creates no authority" rule can be violated.
        requested_authority: BTreeSet<Capability>,
    },
    Split {
        molecule: ComponentId,
    },
    Rebind {
        role: String,
        to: ComponentId,
    },
    Rewire {
        from: ComponentId,
        to: ComponentId,
        connected: bool,
    },
    Promote {
        participant: ComponentId,
        capability: Capability,
    },
    Demote {
        participant: ComponentId,
        capability: Capability,
    },
}

/// A participant in the current topology.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Member {
    pub id: ComponentId,
    pub role: String,
    pub authority: BTreeSet<Capability>,
    pub open_commitments: BTreeSet<String>,
    /// The logical step this member joined. Drives minimum dwell.
    pub joined_at: u64,
}

impl Member {
    pub fn new(id: impl Into<String>, role: impl Into<String>, joined_at: u64) -> Self {
        Member {
            id: ComponentId::new(id),
            role: role.into(),
            authority: BTreeSet::new(),
            open_commitments: BTreeSet::new(),
            joined_at,
        }
    }

    pub fn holding(mut self, capability: Capability) -> Self {
        self.authority.insert(capability);
        self
    }

    pub fn owing(mut self, commitment: impl Into<String>) -> Self {
        self.open_commitments.insert(commitment.into());
        self
    }
}

/// What one applied action changed.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TopologyChange {
    pub added: BTreeSet<ComponentId>,
    pub removed: BTreeSet<ComponentId>,
    /// Obligations that left with an evicted participant and now belong to nobody. Reported, never
    /// dropped: 23.48's orphan-commitment detection has to have something to detect.
    pub stranded_commitments: BTreeSet<String>,
    pub authority_delta: BTreeMap<ComponentId, BTreeSet<Capability>>,
}

/// The current team.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Topology {
    pub pattern: Pattern,
    members: BTreeMap<ComponentId, Member>,
    edges: BTreeSet<(ComponentId, ComponentId)>,
    molecules: BTreeMap<ComponentId, BTreeSet<ComponentId>>,
    step: u64,
}

impl Topology {
    pub fn new(pattern: Pattern) -> Self {
        Topology {
            pattern,
            members: BTreeMap::new(),
            edges: BTreeSet::new(),
            molecules: BTreeMap::new(),
            step: 0,
        }
    }

    pub fn with(mut self, member: Member) -> Self {
        self.members.insert(member.id.clone(), member);
        self
    }

    pub fn member(&self, id: &ComponentId) -> Option<&Member> {
        self.members.get(id)
    }

    pub fn members(&self) -> impl Iterator<Item = &Member> {
        self.members.values()
    }

    pub fn size(&self) -> usize {
        self.members.len()
    }

    pub fn step(&self) -> u64 {
        self.step
    }

    pub fn molecule_members(&self, molecule: &ComponentId) -> Option<&BTreeSet<ComponentId>> {
        self.molecules.get(molecule)
    }

    /// Apply one action, advancing the logical step.
    pub fn apply(&mut self, action: &TopologyAction) -> Result<TopologyChange, TopologyError> {
        self.step += 1;
        let mut change = TopologyChange::default();
        match action {
            TopologyAction::Spawn {
                role,
                participant,
                authority,
            } => {
                if self.members.contains_key(participant) {
                    return Err(TopologyError::AlreadyPresent {
                        participant: participant.clone(),
                    });
                }
                let mut member = Member::new(participant.as_str(), role.clone(), self.step);
                member.authority = authority.clone();
                self.members.insert(participant.clone(), member);
                change.added.insert(participant.clone());
                change
                    .authority_delta
                    .insert(participant.clone(), authority.clone());
            }
            TopologyAction::Admit { participant } => {
                if self.members.contains_key(participant) {
                    return Err(TopologyError::AlreadyPresent {
                        participant: participant.clone(),
                    });
                }
                self.members.insert(
                    participant.clone(),
                    Member::new(participant.as_str(), "admitted", self.step),
                );
                change.added.insert(participant.clone());
            }
            TopologyAction::Retire { participant } => {
                let member = self.require(participant)?;
                if !member.open_commitments.is_empty() {
                    return Err(TopologyError::RetireWithOpenCommitments {
                        participant: participant.clone(),
                        commitments: member.open_commitments.clone(),
                    });
                }
                self.members.remove(participant);
                self.drop_edges(participant);
                change.removed.insert(participant.clone());
            }
            TopologyAction::Evict { participant, .. } => {
                let member = self.require(participant)?.clone();
                change.stranded_commitments = member.open_commitments.clone();
                self.members.remove(participant);
                self.drop_edges(participant);
                change.removed.insert(participant.clone());
            }
            TopologyAction::Fuse {
                members,
                molecule,
                requested_authority,
            } => {
                let mut available = BTreeSet::new();
                for id in members {
                    available.extend(self.require(id)?.authority.iter().cloned());
                }
                if self.members.contains_key(molecule) {
                    return Err(TopologyError::AlreadyPresent {
                        participant: molecule.clone(),
                    });
                }
                let created: BTreeSet<Capability> = requested_authority
                    .difference(&available)
                    .cloned()
                    .collect();
                if !created.is_empty() {
                    return Err(TopologyError::FusionCreatedAuthority {
                        molecule: molecule.clone(),
                        capabilities: created,
                    });
                }
                let mut fused = Member::new(molecule.as_str(), "molecule", self.step);
                fused.authority = requested_authority.clone();
                fused.open_commitments = members
                    .iter()
                    .filter_map(|id| self.members.get(id))
                    .flat_map(|m| m.open_commitments.iter().cloned())
                    .collect();
                self.molecules.insert(molecule.clone(), members.clone());
                self.members.insert(molecule.clone(), fused);
                change.added.insert(molecule.clone());
                change
                    .authority_delta
                    .insert(molecule.clone(), requested_authority.clone());
            }
            TopologyAction::Split { molecule } => {
                let members = self.molecules.remove(molecule).ok_or_else(|| {
                    TopologyError::NotAMolecule {
                        participant: molecule.clone(),
                    }
                })?;
                self.members.remove(molecule);
                self.drop_edges(molecule);
                change.removed.insert(molecule.clone());
                change.added = members;
            }
            TopologyAction::Rebind { role, to } => {
                let previous: Vec<ComponentId> = self
                    .members
                    .values()
                    .filter(|m| &m.role == role)
                    .map(|m| m.id.clone())
                    .collect();
                for id in &previous {
                    let member = self.members[id].clone();
                    if !member.open_commitments.is_empty() {
                        return Err(TopologyError::RetireWithOpenCommitments {
                            participant: id.clone(),
                            commitments: member.open_commitments,
                        });
                    }
                    self.members.remove(id);
                    self.drop_edges(id);
                    change.removed.insert(id.clone());
                }
                self.members.insert(
                    to.clone(),
                    Member::new(to.as_str(), role.clone(), self.step),
                );
                change.added.insert(to.clone());
            }
            TopologyAction::Rewire {
                from,
                to,
                connected,
            } => {
                self.require(from)?;
                self.require(to)?;
                if *connected {
                    self.edges.insert((from.clone(), to.clone()));
                } else {
                    self.edges.remove(&(from.clone(), to.clone()));
                }
            }
            TopologyAction::Promote {
                participant,
                capability,
            } => {
                let held_elsewhere = self
                    .members
                    .values()
                    .any(|m| &m.id != participant && m.authority.contains(capability));
                if !held_elsewhere {
                    return Err(TopologyError::PromotionCreatesAuthority {
                        participant: participant.clone(),
                        capability: capability.clone(),
                    });
                }
                let member = self.members.get_mut(participant).ok_or_else(|| {
                    TopologyError::NotPresent {
                        participant: participant.clone(),
                    }
                })?;
                member.authority.insert(capability.clone());
                change.authority_delta.insert(
                    participant.clone(),
                    [capability.clone()].into_iter().collect(),
                );
            }
            TopologyAction::Demote {
                participant,
                capability,
            } => {
                let member = self.members.get_mut(participant).ok_or_else(|| {
                    TopologyError::NotPresent {
                        participant: participant.clone(),
                    }
                })?;
                member.authority.remove(capability);
            }
        }
        Ok(change)
    }

    fn require(&self, id: &ComponentId) -> Result<&Member, TopologyError> {
        self.members
            .get(id)
            .ok_or_else(|| TopologyError::NotPresent {
                participant: id.clone(),
            })
    }

    fn drop_edges(&mut self, id: &ComponentId) {
        self.edges.retain(|(a, b)| a != id && b != id);
    }
}

/// A reason code. 23.20's "explicit reason codes" as a required argument rather than an optional
/// field, so a change without one cannot be proposed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReasonCode {
    MissingCapability,
    HighUncertainty,
    IndependentVerificationNeeded,
    ParallelisableWork,
    DeadlineApproaching,
    AdversarialReview,
    ParticipantFailure,
    CommitmentsClosed,
    NegligibleContribution,
    RedundantWork,
    BudgetScarce,
    CapabilityMismatch,
    ParticipantStale,
    CommunicationOverhead,
}

/// Thrashing-prevention policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DwellPolicy {
    /// A member may not be removed within this many steps of joining.
    pub minimum_dwell: u64,
    /// After a refused spawn, no spawn for this many steps.
    pub spawn_cooldown: u64,
    /// Maximum changes within `window`.
    pub max_changes: usize,
    pub window: u64,
}

impl DwellPolicy {
    pub fn new(minimum_dwell: u64, spawn_cooldown: u64, max_changes: usize, window: u64) -> Self {
        DwellPolicy {
            minimum_dwell,
            spawn_cooldown,
            max_changes,
            window,
        }
    }
}

/// Admits or refuses proposed topology changes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Controller {
    policy: DwellPolicy,
    history: Vec<(u64, ReasonCode)>,
    last_failed_spawn: Option<u64>,
}

impl Controller {
    pub fn new(policy: DwellPolicy) -> Self {
        Controller {
            policy,
            history: Vec::new(),
            last_failed_spawn: None,
        }
    }

    pub fn note_failed_spawn(&mut self, at: u64) {
        self.last_failed_spawn = Some(at);
    }

    /// Whether a proposed action may be applied now.
    pub fn propose(
        &mut self,
        topology: &Topology,
        action: &TopologyAction,
        reason: ReasonCode,
    ) -> Result<(), TopologyError> {
        let now = topology.step() + 1;
        let recent = self
            .history
            .iter()
            .filter(|(at, _)| now.saturating_sub(*at) < self.policy.window)
            .count();
        if recent >= self.policy.max_changes {
            return Err(TopologyError::ChangeBudgetExhausted {
                window: self.policy.window,
                max_changes: self.policy.max_changes,
            });
        }
        if let TopologyAction::Spawn { .. } = action {
            if let Some(failed) = self.last_failed_spawn {
                if now.saturating_sub(failed) < self.policy.spawn_cooldown {
                    return Err(TopologyError::SpawnCooldown {
                        remaining: self.policy.spawn_cooldown - (now - failed),
                    });
                }
            }
        }
        if let TopologyAction::Retire { participant } | TopologyAction::Evict { participant, .. } =
            action
        {
            if let Some(member) = topology.member(participant) {
                let dwelt = now.saturating_sub(member.joined_at);
                if dwelt < self.policy.minimum_dwell {
                    return Err(TopologyError::MinimumDwellNotMet {
                        participant: participant.clone(),
                        dwelt,
                        required: self.policy.minimum_dwell,
                    });
                }
            }
        }
        self.history.push((now, reason));
        Ok(())
    }

    pub fn change_count(&self) -> usize {
        self.history.len()
    }

    /// The thrashing mechanisms 23.20 lists and this controller does not implement.
    pub fn unimplemented_mechanisms() -> &'static [(&'static str, &'static str)] {
        &[
            (
                "hysteresis thresholds",
                "needs a utility estimate to have a threshold on, and nothing here estimates \
                 utility",
            ),
            (
                "reconfiguration cost",
                "needs a cost model over topology transitions",
            ),
            (
                "stable role identifiers",
                "Rebind assigns a fresh member; preserving identity across rebinding needs a \
                 participant registry this crate does not own",
            ),
        ]
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TopologyError {
    #[error("{participant} is already in this topology")]
    AlreadyPresent { participant: ComponentId },

    #[error("{participant} is not in this topology")]
    NotPresent { participant: ComponentId },

    #[error("{participant} cannot retire holding {commitments:?}; evict it or hand them off")]
    RetireWithOpenCommitments {
        participant: ComponentId,
        commitments: BTreeSet<String>,
    },

    #[error("{participant} is not a molecule")]
    NotAMolecule { participant: ComponentId },

    #[error("fusing into {molecule} would create {capabilities:?}, which no member held")]
    FusionCreatedAuthority {
        molecule: ComponentId,
        capabilities: BTreeSet<Capability>,
    },

    #[error("promoting {participant} to {capability:?} would create authority nobody holds")]
    PromotionCreatesAuthority {
        participant: ComponentId,
        capability: Capability,
    },

    #[error("{participant} has been present {dwelt} steps and the minimum dwell is {required}")]
    MinimumDwellNotMet {
        participant: ComponentId,
        dwelt: u64,
        required: u64,
    },

    #[error("no spawn for another {remaining} steps after a failed one")]
    SpawnCooldown { remaining: u64 },

    #[error("already made {max_changes} changes in the last {window} steps")]
    ChangeBudgetExhausted { window: u64, max_changes: usize },
}
