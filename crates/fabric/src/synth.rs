//! Goal-to-molecule compiler and verified team synthesis.
//!
//! Blueprint 23.42.
//!
//! # What is real here and what is not
//!
//! 23.42's pipeline has nine stages. Three of them are constrained *checking* and are implemented:
//! stage 2 (an abstract role graph, built before any agent is chosen), stage 5 (static rejection,
//! nine reasons), and the hard-constraint half of stage 7. The rest need things this crate does not
//! have — a model to propose decompositions, a registry to retrieve from, a probe harness to run
//! microbenchmarks, and a runtime to bind against. [`unimplemented_stages`] lists them.
//!
//! What survives is the part 23.42 says is the actual research claim: "agent composition can become
//! an explicit, measurable, constrained compilation problem instead of prompt folklore." A
//! constrained compilation problem is one where candidates are *rejected for stated reasons*, and
//! [`reject`] is that.
//!
//! # Hard constraints are never penalties
//!
//! 23.42: "Hard constraints are never converted into small penalties." [`Score`] therefore cannot
//! be computed for a candidate that failed a hard constraint — [`evaluate`] returns
//! [`Candidacy::Rejected`] with the reasons and no score at all, so no downstream sort can rank a
//! forbidden candidate above a permitted one because its cost was low.
//!
//! # Complementarity, not headcount
//!
//! 23.42: "Adding agents is beneficial only when they contribute distinct information or error
//! modes." [`correlated_failure_penalty`] counts *shared runtime lineages*, reusing
//! [`crate::reputation::independent_subjects`] so a team of five clones scores as one source. A
//! one-agent molecule may win, and the test that says so is the point of the function.

use crate::contract::AgentContract;
use crate::effect::{EffectSet, Inclusion, Irreversibility};
use crate::flow::{FlowDecision, Labelling};
use crate::reputation::{CapabilityCard, EvidenceLayer};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// 23.42's typed task contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Goal {
    pub intent: String,
    /// "The input includes the expected artifact, not only a natural-language goal." A goal without
    /// one is refused by [`Goal::new`].
    pub output_artifact: String,
    pub effects_allowed: EffectSet,
    pub effects_forbidden: EffectSet,
    pub budget_minor: u64,
    pub latency_units: u64,
    pub privacy_label: Labelling,
    pub required_assurance: BTreeSet<AssuranceRequirement>,
    pub minimum_rung: EvidenceLayer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssuranceRequirement {
    DeterministicTests,
    IndependentVerification,
    HumanApproval,
}

impl Goal {
    pub fn new(
        intent: impl Into<String>,
        output_artifact: impl Into<String>,
    ) -> Result<Self, SynthesisError> {
        let output_artifact = output_artifact.into();
        if output_artifact.is_empty() {
            return Err(SynthesisError::GoalWithoutArtifact);
        }
        Ok(Goal {
            intent: intent.into(),
            output_artifact,
            effects_allowed: EffectSet::new(),
            effects_forbidden: EffectSet::new(),
            budget_minor: 0,
            latency_units: 0,
            privacy_label: Labelling::Unlabelled,
            required_assurance: BTreeSet::new(),
            minimum_rung: EvidenceLayer::SelfDeclared,
        })
    }

    pub fn allowing(mut self, effects: EffectSet) -> Self {
        self.effects_allowed = effects;
        self
    }

    pub fn forbidding(mut self, effects: EffectSet) -> Self {
        self.effects_forbidden = effects;
        self
    }

    pub fn within(mut self, budget_minor: u64, latency_units: u64) -> Self {
        self.budget_minor = budget_minor;
        self.latency_units = latency_units;
        self
    }

    pub fn labelled(mut self, label: Labelling) -> Self {
        self.privacy_label = label;
        self
    }

    pub fn requiring(mut self, requirement: AssuranceRequirement) -> Self {
        self.required_assurance.insert(requirement);
        self
    }

    pub fn at_rung(mut self, rung: EvidenceLayer) -> Self {
        self.minimum_rung = rung;
        self
    }
}

/// Stage 2: a role, specified before any agent is considered.
///
/// 23.42: "Roles specify capabilities and assurance, not personas." There is no name field for a
/// persona and no place to put one.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Role {
    pub function: String,
    pub capability: String,
    pub minimum_rung: EvidenceLayer,
    pub permitted_effects: EffectSet,
}

impl Role {
    pub fn new(
        function: impl Into<String>,
        capability: impl Into<String>,
        minimum_rung: EvidenceLayer,
    ) -> Self {
        Role {
            function: function.into(),
            capability: capability.into(),
            minimum_rung,
            permitted_effects: EffectSet::new(),
        }
    }

    pub fn permitting(mut self, effects: EffectSet) -> Self {
        self.permitted_effects = effects;
        self
    }
}

/// An abstract role graph: roles and who verifies whom.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoleGraph {
    pub roles: BTreeSet<Role>,
    pub verifies: BTreeSet<(String, String)>,
}

impl RoleGraph {
    pub fn new() -> Self {
        RoleGraph::default()
    }

    pub fn with(mut self, role: Role) -> Self {
        self.roles.insert(role);
        self
    }

    pub fn verifying(mut self, verifier: impl Into<String>, subject: impl Into<String>) -> Self {
        self.verifies.insert((verifier.into(), subject.into()));
        self
    }

    /// Whether some role verifies another's work. 23.42's `independent-verification` assurance
    /// requirement is exactly this, and a graph where the producer verifies itself does not count.
    pub fn has_independent_verification(&self) -> bool {
        self.verifies.iter().any(|(v, s)| v != s)
    }
}

/// A synthesised candidate: a role graph bound to participants.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Candidate {
    pub name: String,
    pub graph: RoleGraph,
    pub bindings: BTreeMap<String, AgentContract>,
    pub cards: Vec<CapabilityCard>,
    pub terminal_states: BTreeSet<String>,
    pub compensated: EffectSet,
}

impl Candidate {
    pub fn new(name: impl Into<String>, graph: RoleGraph) -> Self {
        Candidate {
            name: name.into(),
            graph,
            bindings: BTreeMap::new(),
            cards: Vec::new(),
            terminal_states: BTreeSet::new(),
            compensated: EffectSet::new(),
        }
    }

    pub fn binding(mut self, role: impl Into<String>, contract: AgentContract) -> Self {
        self.bindings.insert(role.into(), contract);
        self
    }

    pub fn carded(mut self, card: CapabilityCard) -> Self {
        self.cards.push(card);
        self
    }

    pub fn terminating_at(mut self, state: impl Into<String>) -> Self {
        self.terminal_states.insert(state.into());
        self
    }

    pub fn compensating(mut self, effects: EffectSet) -> Self {
        self.compensated = effects;
        self
    }

    fn total_effects(&self) -> EffectSet {
        self.bindings
            .values()
            .fold(EffectSet::new(), |acc, c| acc.union(&c.effects))
    }

    fn total_cost_minor(&self) -> u64 {
        self.bindings
            .values()
            .map(|c| c.envelope.declared_cost_minor)
            .sum()
    }

    fn peak_latency(&self) -> u64 {
        self.bindings
            .values()
            .map(|c| c.envelope.declared_latency_units)
            .max()
            .unwrap_or(0)
    }
}

/// 23.42's nine static rejection reasons.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "reason")]
pub enum RejectionReason {
    UnresolvedTypes { roles: BTreeSet<String> },
    AuthorityEscalation { roles: BTreeSet<String> },
    NonCompensatedIrreversibleEffect { effects: Vec<crate::effect::Effect> },
    OrphanCommitments { commitments: BTreeSet<String> },
    MissingTerminalStates,
    InformationFlowViolation { detail: String },
    ImpossibleBudgetBound { required: u64, allowed: u64 },
    KnownProtocolDeadlock { detail: String },
    InsufficientVerifiedAssurance { roles: BTreeSet<String> },
}

/// Stage 5. Every reason a candidate fails, collected.
///
/// `deadlock` is supplied by the caller because `bioprism-choreography` owns model checking and
/// this crate will not re-derive it; passing `Some(detail)` records a counterexample that crate
/// found.
pub fn reject(goal: &Goal, candidate: &Candidate, deadlock: Option<&str>) -> Vec<RejectionReason> {
    let mut out = Vec::new();

    let unbound: BTreeSet<String> = candidate
        .graph
        .roles
        .iter()
        .filter(|role| !candidate.bindings.contains_key(&role.function))
        .map(|role| role.function.clone())
        .collect();
    if !unbound.is_empty() {
        out.push(RejectionReason::UnresolvedTypes { roles: unbound });
    }

    let escalating: BTreeSet<String> = candidate
        .graph
        .roles
        .iter()
        .filter(|role| {
            candidate
                .bindings
                .get(&role.function)
                .map(|contract| {
                    !matches!(
                        role.permitted_effects.includes(&contract.effects),
                        Inclusion::Holds
                    )
                })
                .unwrap_or(false)
        })
        .map(|role| role.function.clone())
        .collect();
    if !escalating.is_empty() {
        out.push(RejectionReason::AuthorityEscalation { roles: escalating });
    }

    let effects = candidate.total_effects();
    let uncompensated: Vec<crate::effect::Effect> = effects
        .escalation_over(&candidate.compensated)
        .into_iter()
        .filter(|e| e.class >= Irreversibility::E4)
        .collect();
    if !uncompensated.is_empty() {
        out.push(RejectionReason::NonCompensatedIrreversibleEffect {
            effects: uncompensated,
        });
    }

    let orphans: BTreeSet<String> = if candidate.terminal_states.is_empty() {
        candidate
            .bindings
            .values()
            .flat_map(|c| c.mandatory_commitments())
            .map(|c| c.id)
            .collect()
    } else {
        BTreeSet::new()
    };
    if !orphans.is_empty() {
        out.push(RejectionReason::OrphanCommitments {
            commitments: orphans,
        });
    }

    if candidate.terminal_states.is_empty() {
        out.push(RejectionReason::MissingTerminalStates);
    }

    for (role, contract) in &candidate.bindings {
        if let FlowDecision::Refused { refusals } =
            goal.privacy_label.flows_to(&contract.output_labelling)
        {
            out.push(RejectionReason::InformationFlowViolation {
                detail: format!("{role}: {refusals:?}"),
            });
            break;
        }
    }

    if !matches!(
        goal.effects_allowed.includes(&effects),
        Inclusion::Holds
    ) || !effects
        .escalation_over(&goal.effects_forbidden)
        .is_empty()
        && goal
            .effects_forbidden
            .iter()
            .any(|forbidden| effects.iter().any(|e| e.kind == forbidden.kind))
    {
        out.push(RejectionReason::AuthorityEscalation {
            roles: candidate.bindings.keys().cloned().collect(),
        });
    }

    let cost = candidate.total_cost_minor();
    if cost > goal.budget_minor {
        out.push(RejectionReason::ImpossibleBudgetBound {
            required: cost,
            allowed: goal.budget_minor,
        });
    }
    let latency = candidate.peak_latency();
    if latency > goal.latency_units {
        out.push(RejectionReason::ImpossibleBudgetBound {
            required: latency,
            allowed: goal.latency_units,
        });
    }

    if let Some(detail) = deadlock {
        out.push(RejectionReason::KnownProtocolDeadlock {
            detail: detail.to_string(),
        });
    }

    let under_assured: BTreeSet<String> = candidate
        .bindings
        .iter()
        .filter(|(_, contract)| contract.assurance.verified_at < goal.minimum_rung)
        .map(|(role, _)| role.clone())
        .collect();
    if !under_assured.is_empty() {
        out.push(RejectionReason::InsufficientVerifiedAssurance {
            roles: under_assured,
        });
    }
    if goal
        .required_assurance
        .contains(&AssuranceRequirement::IndependentVerification)
        && !candidate.graph.has_independent_verification()
    {
        out.push(RejectionReason::InsufficientVerifiedAssurance {
            roles: [String::from("<no independent verifier in the role graph>")]
                .into_iter()
                .collect(),
        });
    }

    out
}

/// The soft terms of 23.42's objective, once the hard constraints have already passed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Score {
    pub cost_minor: u64,
    pub latency_units: u64,
    /// Count of effects at E3 or above. 23.42's "privilege surface".
    pub privilege_surface: usize,
    /// Independent evidence sources missing relative to team size.
    pub correlated_failure: usize,
    pub team_size: usize,
}

/// The result of evaluating one candidate.
///
/// No score on the rejected arm, on purpose.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "candidacy")]
pub enum Candidacy {
    Admissible { score: Score },
    Rejected { reasons: Vec<RejectionReason> },
}

/// How many members share a runtime lineage with another member.
///
/// Zero for a team of one, and zero for a team of genuinely distinct participants. Nonzero exactly
/// when adding a member added headcount and no information.
pub fn correlated_failure_penalty(cards: &[CapabilityCard]) -> usize {
    cards
        .len()
        .saturating_sub(crate::reputation::independent_subjects(cards).len())
}

/// Run stage 5 and, if it passes, compute the soft score.
pub fn evaluate(goal: &Goal, candidate: &Candidate, deadlock: Option<&str>) -> Candidacy {
    let reasons = reject(goal, candidate, deadlock);
    if !reasons.is_empty() {
        return Candidacy::Rejected { reasons };
    }
    Candidacy::Admissible {
        score: Score {
            cost_minor: candidate.total_cost_minor(),
            latency_units: candidate.peak_latency(),
            privilege_surface: candidate
                .total_effects()
                .iter()
                .filter(|e| e.class >= Irreversibility::E3)
                .count(),
            correlated_failure: correlated_failure_penalty(&candidate.cards),
            team_size: candidate.bindings.len(),
        },
    }
}

/// The Pareto frontier over admissible candidates.
///
/// 23.42: "Select a Pareto candidate rather than maximizing one score." No weights, therefore no
/// single winner: this returns the frontier and lets a caller with a preference pick from it. A
/// function that returned one candidate would be smuggling in the λ coefficients 23.42 leaves
/// unspecified.
pub fn pareto_frontier(candidates: &[(String, Score)]) -> BTreeSet<String> {
    candidates
        .iter()
        .filter(|(_, score)| {
            !candidates
                .iter()
                .any(|(_, other)| other != score && dominates(other, score))
        })
        .map(|(name, _)| name.clone())
        .collect()
}

fn dominates(a: &Score, b: &Score) -> bool {
    let axes = [
        (a.cost_minor, b.cost_minor),
        (a.latency_units, b.latency_units),
        (a.privilege_surface as u64, b.privilege_surface as u64),
        (a.correlated_failure as u64, b.correlated_failure as u64),
    ];
    axes.iter().all(|(x, y)| x <= y) && axes.iter().any(|(x, y)| x < y)
}

/// A synthesis artifact: what was chosen, and what was eliminated and why.
///
/// 23.42: "The compiler should expose why one topology was selected and which constraints
/// eliminated alternatives." The rejection map is not optional and is not a debug log.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SynthesisArtifact {
    pub frontier: BTreeSet<String>,
    pub scores: BTreeMap<String, Score>,
    pub eliminated: BTreeMap<String, Vec<RejectionReason>>,
}

/// Evaluate a candidate set and build the artifact.
pub fn synthesize(goal: &Goal, candidates: &[Candidate]) -> SynthesisArtifact {
    let mut scores = BTreeMap::new();
    let mut eliminated = BTreeMap::new();
    for candidate in candidates {
        match evaluate(goal, candidate, None) {
            Candidacy::Admissible { score } => {
                scores.insert(candidate.name.clone(), score);
            }
            Candidacy::Rejected { reasons } => {
                eliminated.insert(candidate.name.clone(), reasons);
            }
        }
    }
    let pairs: Vec<(String, Score)> = scores.iter().map(|(n, s)| (n.clone(), *s)).collect();
    SynthesisArtifact {
        frontier: pareto_frontier(&pairs),
        scores,
        eliminated,
    }
}

/// 23.42's pipeline stages this crate does not run, and what each needs.
pub fn unimplemented_stages() -> &'static [(&'static str, &'static str)] {
    &[
        (
            "1. contract decomposition",
            "needs a model to propose decompositions; only the deterministic checker half is here",
        ),
        (
            "3. capability graph retrieval",
            "needs a registry, live availability and capability posteriors",
        ),
        (
            "4. topology synthesis",
            "the operators exist in crate::algebra; enumerating candidate topologies is a search \
             this crate does not run",
        ),
        (
            "6. microbenchmark probing",
            "needs a probe harness and Decision Cells",
        ),
        (
            "8. binding and compilation",
            "needs participant version pinning and local role machines",
        ),
        (
            "9. admission test",
            "needs a conformance runner and a dry-run environment",
        ),
        (
            "runtime recompilation",
            "needs a running thread whose participants can become unavailable",
        ),
    ]
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SynthesisError {
    #[error("a goal must name the expected artifact, not only a natural-language intent")]
    GoalWithoutArtifact,
}
