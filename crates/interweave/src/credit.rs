//! Multi-agent credit assignment, reward gates, and the conditions a credit claim must discharge.
//!
//! Blueprint 23.28.
//!
//! # The one thing this module is for
//!
//! 23.28: **"Safety constraints are gates, not merely negative reward terms."** [`evaluate`]
//! enforces that in the return type. A run with a failed gate yields [`Evaluation::Blocked`], which
//! carries the gate and no numbers at all; there is no field holding a score, no `score_or_zero`,
//! and no way to obtain a reward vector from a blocked evaluation. A gate that could be traded
//! against reward is a reward term with a stern name, which is precisely what the sentence rules
//! out.
//!
//! # Credit is arithmetic, and it is exact
//!
//! [`shapley`] is the real Shapley value, not an approximation: every one of the `2^n` coalitions
//! is enumerated and the result is an exact [`Rational`] over `n!`, reduced. That makes the four
//! classical axioms testable rather than asserted — efficiency, symmetry, the dummy player, and
//! additivity are properties of this function, and the tests check them.
//!
//! Exactness costs. [`shapley`] refuses above [`MAX_EXACT_PARTICIPANTS`] participants rather than
//! silently switching to sampling, because a number labelled "Shapley value" that was actually a
//! Monte-Carlo estimate with no stated error is the kind of quiet substitution this workspace does
//! not make. 23.28 itself only asks for "Shapley-style approximation ... for expensive release
//! analysis"; the approximation is the part that is not implemented.
//!
//! A second refusal matters more than the first. [`CoalitionValues`] cannot be built without a
//! value for **every** coalition including the empty one. A missing coalition is
//! [`CreditError::UnvaluedCoalition`], never an implicit zero — "this team was never run" and "this
//! team produced nothing" are different states, and averaging over the first as though it were the
//! second is how a participant gets credit for a coalition nobody tried.
//!
//! # A credit number is not reportable until its confounders are discharged
//!
//! 23.28's "avoiding misleading credit" section lists six ways a credit estimate can be wrong for
//! reasons that have nothing to do with the arithmetic. [`CreditEstimate`] carries a
//! [`Discharge`] for each, and [`CreditEstimate::reportable`] is false while any is
//! [`Discharge::NotControlled`]. The estimate still exists and is still readable; what it is not is
//! publishable, and the difference is visible in the type.
//!
//! # Not implemented
//!
//! No learning of any kind. No bandit, no Bayesian optimisation, no evolutionary search, no
//! constrained policy search, no imitation — 23.28's six search methods are named in
//! [`SEARCH_METHODS`] and none is implemented, here or anywhere in this workspace. No policy is
//! trained, no trace is ingested, and no reward is estimated from data; every value in this module
//! is supplied by a caller. 23.28's "training data" section describes seven corpora that do not
//! exist here.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// 23.28's thirteen learnable orchestration decisions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Decision {
    WhenToCreateAThread,
    WhichTopologyToUse,
    WhenToSpawnAnotherParticipant,
    WhichParticipantToBind,
    WhatContextCapsuleToSend,
    WhichCommunicativeActToUse,
    HowMuchDetailToCommunicate,
    WhenToFork,
    WhichBranchToContinue,
    WhenToChallenge,
    WhichVerifierToInvoke,
    HowToAggregate,
    WhenToStopOrEscalate,
}

impl Decision {
    pub const ALL: [Decision; 13] = [
        Decision::WhenToCreateAThread,
        Decision::WhichTopologyToUse,
        Decision::WhenToSpawnAnotherParticipant,
        Decision::WhichParticipantToBind,
        Decision::WhatContextCapsuleToSend,
        Decision::WhichCommunicativeActToUse,
        Decision::HowMuchDetailToCommunicate,
        Decision::WhenToFork,
        Decision::WhichBranchToContinue,
        Decision::WhenToChallenge,
        Decision::WhichVerifierToInvoke,
        Decision::HowToAggregate,
        Decision::WhenToStopOrEscalate,
    ];
}

/// 23.28's eight policy slots.
///
/// 23.28 recommends these over one controller: "a monolithic controller is harder to attribute and
/// safer to avoid initially". The attribution half is why the slot is a type — a credit estimate
/// names the slot it is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PolicySlot {
    TaskFingerprinting,
    RoleMatching,
    ContextProjection,
    TopologyControl,
    BudgetAllocation,
    ChallengeAndVerification,
    Aggregation,
    Termination,
}

impl PolicySlot {
    pub const ALL: [PolicySlot; 8] = [
        PolicySlot::TaskFingerprinting,
        PolicySlot::RoleMatching,
        PolicySlot::ContextProjection,
        PolicySlot::TopologyControl,
        PolicySlot::BudgetAllocation,
        PolicySlot::ChallengeAndVerification,
        PolicySlot::Aggregation,
        PolicySlot::Termination,
    ];
}

/// 23.28's ten reward dimensions.
///
/// Note what is *not* here: risk and permission compliance appears in the blueprint's reward list
/// and is represented as [`RewardDimension::RiskAndPermissionCompliance`], but the two safety
/// properties that gate a run are [`SafetyGate`] values rather than dimensions. 23.28 puts the
/// distinction in a sentence; this module puts it in two types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RewardDimension {
    EndTaskUtility,
    ProtocolCompliance,
    EvidenceQuality,
    CommunicationValue,
    CostAndLatency,
    RiskAndPermissionCompliance,
    CommitmentFulfilment,
    RecoveryQuality,
    Calibration,
    HumanAttention,
}

impl RewardDimension {
    pub const ALL: [RewardDimension; 10] = [
        RewardDimension::EndTaskUtility,
        RewardDimension::ProtocolCompliance,
        RewardDimension::EvidenceQuality,
        RewardDimension::CommunicationValue,
        RewardDimension::CostAndLatency,
        RewardDimension::RiskAndPermissionCompliance,
        RewardDimension::CommitmentFulfilment,
        RewardDimension::RecoveryQuality,
        RewardDimension::Calibration,
        RewardDimension::HumanAttention,
    ];
}

/// A safety constraint that gates a run.
///
/// The name is the constraint. A gate is not scored, weighted, or discounted; it passes or it
/// blocks.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SafetyGate(pub String);

impl SafetyGate {
    pub fn new(name: impl Into<String>) -> Self {
        SafetyGate(name.into())
    }
}

/// Whether a gate held on a run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GateResult {
    pub gate: SafetyGate,
    pub held: bool,
    pub detail: String,
}

/// The outcome of evaluating a run.
///
/// [`Evaluation::Blocked`] has no numeric field. That absence is the module's contribution: a
/// caller holding a blocked evaluation cannot extract a reward vector from it by any route, so a
/// downstream optimiser cannot learn that this run was "almost fine".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "evaluation")]
pub enum Evaluation {
    Blocked { gate: SafetyGate, detail: String },
    Scored { rewards: BTreeMap<RewardDimension, i64> },
}

impl Evaluation {
    /// The reward vector, if there is one. `None` for a blocked run, never an empty map.
    pub fn rewards(&self) -> Option<&BTreeMap<RewardDimension, i64>> {
        match self {
            Evaluation::Scored { rewards } => Some(rewards),
            Evaluation::Blocked { .. } => None,
        }
    }

    pub fn blocked(&self) -> bool {
        matches!(self, Evaluation::Blocked { .. })
    }
}

/// Evaluate a run: gates first, in order, and a reward vector only if all of them held.
///
/// The first failing gate wins, deterministically, so two runs failing the same gate produce the
/// same evaluation regardless of what else failed.
pub fn evaluate(
    gates: &[GateResult],
    rewards: BTreeMap<RewardDimension, i64>,
) -> Evaluation {
    for result in gates {
        if !result.held {
            return Evaluation::Blocked {
                gate: result.gate.clone(),
                detail: result.detail.clone(),
            };
        }
    }
    Evaluation::Scored { rewards }
}

/// An exact rational, reduced, with a positive denominator.
///
/// Shapley values are sums of `k!(n-k-1)!/n!` weighted marginals, which are rational and almost
/// never integral. Carrying them as `f64` would make the efficiency axiom fail by rounding and make
/// two runs of the same computation disagree across platforms; carrying them exactly makes both
/// testable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rational {
    pub numerator: i128,
    pub denominator: u128,
}

fn gcd(mut a: u128, mut b: u128) -> u128 {
    while b != 0 {
        let t = a % b;
        a = b;
        b = t;
    }
    a
}

impl Rational {
    /// Build a reduced rational. A zero denominator is not representable: the only constructors
    /// take a nonzero denominator or produce one.
    pub fn new(numerator: i128, denominator: u128) -> Option<Rational> {
        if denominator == 0 {
            return None;
        }
        let magnitude = numerator.unsigned_abs();
        let divisor = gcd(magnitude, denominator).max(1);
        Some(Rational {
            numerator: numerator / divisor as i128,
            denominator: denominator / divisor,
        })
    }

    pub fn integer(value: i128) -> Rational {
        Rational {
            numerator: value,
            denominator: 1,
        }
    }

    pub fn zero() -> Rational {
        Rational::integer(0)
    }

    pub fn is_zero(&self) -> bool {
        self.numerator == 0
    }
}

impl std::ops::Add for Rational {
    type Output = Rational;

    fn add(self, other: Rational) -> Rational {
        let numerator =
            self.numerator * other.denominator as i128 + other.numerator * self.denominator as i128;
        let denominator = self.denominator * other.denominator;
        Rational::new(numerator, denominator).expect("denominators are nonzero by construction")
    }
}

/// Why a credit computation could not be performed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[serde(rename_all = "snake_case", tag = "error")]
pub enum CreditError {
    #[error("coalition {coalition:?} has no recorded value")]
    UnvaluedCoalition { coalition: BTreeSet<String> },

    #[error("{participant} is not in the grand coalition")]
    UnknownParticipant { participant: String },

    #[error("exact Shapley over {count} participants exceeds the {limit} supported")]
    TooManyParticipants { count: usize, limit: usize },

    #[error("the two games range over different participants")]
    ParticipantMismatch,
}

/// The largest grand coalition [`shapley`] will enumerate exactly.
///
/// `2^16` coalitions is 65536 marginal terms, which is fast and finite. Above it the honest answer
/// is a refusal, because the alternative is an approximation wearing an exact name.
pub const MAX_EXACT_PARTICIPANTS: usize = 16;

/// A cooperative game: a value for every coalition of a fixed participant set.
///
/// Construction requires all `2^n` coalitions. This is a strong requirement and it is the point:
/// there is no way to compute a marginal contribution against a coalition nobody measured, and
/// treating an absent measurement as zero would credit a participant for a counterfactual that was
/// never run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoalitionValues {
    participants: Vec<String>,
    values: BTreeMap<BTreeSet<String>, i64>,
}

impl CoalitionValues {
    /// Build a game, checking that every coalition has a value.
    pub fn new(
        participants: impl IntoIterator<Item = impl Into<String>>,
        values: impl IntoIterator<Item = (BTreeSet<String>, i64)>,
    ) -> Result<Self, CreditError> {
        let mut ordered: Vec<String> = participants.into_iter().map(Into::into).collect();
        ordered.sort();
        ordered.dedup();
        if ordered.len() > MAX_EXACT_PARTICIPANTS {
            return Err(CreditError::TooManyParticipants {
                count: ordered.len(),
                limit: MAX_EXACT_PARTICIPANTS,
            });
        }
        let values: BTreeMap<BTreeSet<String>, i64> = values.into_iter().collect();
        for mask in 0u32..(1u32 << ordered.len()) {
            let coalition = coalition_of(&ordered, mask);
            if !values.contains_key(&coalition) {
                return Err(CreditError::UnvaluedCoalition { coalition });
            }
        }
        Ok(CoalitionValues {
            participants: ordered,
            values,
        })
    }

    pub fn participants(&self) -> &[String] {
        &self.participants
    }

    /// The value of a coalition. Absent is an error, never a zero.
    pub fn value(&self, coalition: &BTreeSet<String>) -> Result<i64, CreditError> {
        self.values
            .get(coalition)
            .copied()
            .ok_or_else(|| CreditError::UnvaluedCoalition {
                coalition: coalition.clone(),
            })
    }

    /// The grand coalition's value.
    pub fn grand(&self) -> Result<i64, CreditError> {
        self.value(&self.participants.iter().cloned().collect())
    }

    /// The empty coalition's value. Usually zero, and never assumed to be.
    pub fn empty(&self) -> Result<i64, CreditError> {
        self.value(&BTreeSet::new())
    }

    /// Pointwise sum of two games over the same participants, for testing additivity.
    pub fn plus(&self, other: &CoalitionValues) -> Result<CoalitionValues, CreditError> {
        if self.participants != other.participants {
            return Err(CreditError::ParticipantMismatch);
        }
        let mut values = BTreeMap::new();
        for (coalition, value) in &self.values {
            let right = other.value(coalition)?;
            values.insert(coalition.clone(), value + right);
        }
        CoalitionValues::new(self.participants.clone(), values)
    }
}

fn coalition_of(participants: &[String], mask: u32) -> BTreeSet<String> {
    participants
        .iter()
        .enumerate()
        .filter(|(index, _)| mask & (1 << index) != 0)
        .map(|(_, name)| name.clone())
        .collect()
}

fn factorial(n: usize) -> u128 {
    (1..=n as u128).product::<u128>().max(1)
}

/// 23.28's difference reward: "compare team outcome with and without a participant contribution".
///
/// `v(N) - v(N \ {i})`. Integral, so it needs no rational.
pub fn difference_reward(game: &CoalitionValues, participant: &str) -> Result<i64, CreditError> {
    if !game.participants.iter().any(|p| p == participant) {
        return Err(CreditError::UnknownParticipant {
            participant: participant.to_string(),
        });
    }
    let grand: BTreeSet<String> = game.participants.iter().cloned().collect();
    let mut without = grand.clone();
    without.remove(participant);
    Ok(game.value(&grand)? - game.value(&without)?)
}

/// The exact Shapley value of one participant.
///
/// `φ_i = Σ_{S ⊆ N\{i}} |S|!(n-|S|-1)!/n! · (v(S∪{i}) − v(S))`, enumerated in full.
pub fn shapley(game: &CoalitionValues, participant: &str) -> Result<Rational, CreditError> {
    let index = game
        .participants
        .iter()
        .position(|p| p == participant)
        .ok_or_else(|| CreditError::UnknownParticipant {
            participant: participant.to_string(),
        })?;
    let n = game.participants.len();
    let denominator = factorial(n);
    let mut numerator: i128 = 0;
    for mask in 0u32..(1u32 << n) {
        if mask & (1 << index) != 0 {
            continue;
        }
        let without = coalition_of(&game.participants, mask);
        let with = coalition_of(&game.participants, mask | (1 << index));
        let size = without.len();
        let weight = factorial(size) * factorial(n - size - 1);
        let marginal = game.value(&with)? as i128 - game.value(&without)? as i128;
        numerator += weight as i128 * marginal;
    }
    Ok(Rational::new(numerator, denominator).expect("n! is nonzero"))
}

/// Every participant's exact Shapley value.
pub fn shapley_all(game: &CoalitionValues) -> Result<BTreeMap<String, Rational>, CreditError> {
    game.participants
        .iter()
        .map(|p| shapley(game, p).map(|value| (p.clone(), value)))
        .collect()
}

/// 23.28's six credit-assignment methods.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Method {
    LocalCausalEffect,
    DifferenceRewards,
    ShapleyStyleApproximation,
    MessageLevelValue,
    CommitmentLevelCredit,
    CounterfactualVerifierCredit,
}

impl Method {
    pub const ALL: [Method; 6] = [
        Method::LocalCausalEffect,
        Method::DifferenceRewards,
        Method::ShapleyStyleApproximation,
        Method::MessageLevelValue,
        Method::CommitmentLevelCredit,
        Method::CounterfactualVerifierCredit,
    ];

    /// Whether this crate computes the method or only names it.
    ///
    /// Two of the six are computed. The other four need a forkable execution substrate, a message
    /// stream, a commitment ledger walk, and counterfactual replay respectively — all of which are
    /// runtime capabilities this crate does not have.
    pub fn computed_here(self) -> bool {
        matches!(self, Method::DifferenceRewards | Method::ShapleyStyleApproximation)
    }
}

/// 23.28's six ways credit goes wrong for non-arithmetic reasons.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Confounder {
    /// "downstream success may depend on hidden shared evidence".
    HiddenSharedEvidence,
    /// "agents can be complementary rather than independently strong".
    Complementarity,
    /// "a skeptic's value appears only on defective cases".
    SkepticValueOnlyOnDefects,
    /// "early context selection can dominate later execution".
    EarlyContextDominance,
    /// "correlated participants inflate apparent consensus".
    CorrelatedParticipants,
    /// "task difficulty and parent lineage must be controlled".
    UncontrolledDifficultyOrLineage,
}

impl Confounder {
    pub const ALL: [Confounder; 6] = [
        Confounder::HiddenSharedEvidence,
        Confounder::Complementarity,
        Confounder::SkepticValueOnlyOnDefects,
        Confounder::EarlyContextDominance,
        Confounder::CorrelatedParticipants,
        Confounder::UncontrolledDifficultyOrLineage,
    ];
}

/// Whether a confounder was controlled for, and how.
///
/// [`Discharge::Controlled`] requires a stated method. There is no bare "yes": a claim to have
/// controlled for correlated participants that cannot say how is not a control.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "discharge")]
pub enum Discharge {
    Controlled { by: String },
    NotControlled,
}

/// A credit number together with everything that decides whether it may be reported.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreditEstimate {
    pub participant: String,
    pub slot: PolicySlot,
    pub method: Method,
    pub value: Rational,
    discharges: BTreeMap<Confounder, Discharge>,
}

impl CreditEstimate {
    /// A new estimate with every confounder undischarged. That is the honest starting state: an
    /// estimate that has not been examined has not controlled for anything.
    pub fn new(
        participant: impl Into<String>,
        slot: PolicySlot,
        method: Method,
        value: Rational,
    ) -> Self {
        CreditEstimate {
            participant: participant.into(),
            slot,
            method,
            value,
            discharges: Confounder::ALL
                .into_iter()
                .map(|c| (c, Discharge::NotControlled))
                .collect(),
        }
    }

    pub fn controlling(mut self, confounder: Confounder, by: impl Into<String>) -> Self {
        self.discharges
            .insert(confounder, Discharge::Controlled { by: by.into() });
        self
    }

    pub fn discharge(&self, confounder: Confounder) -> &Discharge {
        self.discharges
            .get(&confounder)
            .unwrap_or(&Discharge::NotControlled)
    }

    /// Confounders still outstanding.
    pub fn outstanding(&self) -> BTreeSet<Confounder> {
        Confounder::ALL
            .into_iter()
            .filter(|c| matches!(self.discharge(*c), Discharge::NotControlled))
            .collect()
    }

    /// Whether this estimate may be published as a credit claim.
    ///
    /// The value is readable either way. What `false` withholds is the right to call the number a
    /// measurement of this participant's contribution.
    pub fn reportable(&self) -> bool {
        self.outstanding().is_empty()
    }
}

/// 23.28's eight required fields on an evolution card.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CardField {
    FailureClustersUsed,
    TrainingAndHoldoutPartitions,
    ArchitectureChanges,
    BeforeAfterCapabilityGraph,
    CostAndSafetyEffects,
    KnownRegressions,
    RollbackHash,
    ApprovalRecord,
}

impl CardField {
    pub const ALL: [CardField; 8] = [
        CardField::FailureClustersUsed,
        CardField::TrainingAndHoldoutPartitions,
        CardField::ArchitectureChanges,
        CardField::BeforeAfterCapabilityGraph,
        CardField::CostAndSafetyEffects,
        CardField::KnownRegressions,
        CardField::RollbackHash,
        CardField::ApprovalRecord,
    ];
}

/// 23.28: "Every learned policy update includes: ..."
///
/// [`EvolutionCard::missing`] treats an empty string as absent, because a required field filled
/// with nothing is the same failure as a field left out and should not be harder to detect.
///
/// [`CardField::KnownRegressions`] is the field this design is most careful about: "none" is a
/// legitimate and informative value there, so the field is present when the author wrote *any*
/// text, including "none observed on the holdout", and absent only when they wrote nothing.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvolutionCard {
    fields: BTreeMap<CardField, String>,
}

impl EvolutionCard {
    pub fn new() -> Self {
        EvolutionCard::default()
    }

    pub fn stating(mut self, field: CardField, value: impl Into<String>) -> Self {
        self.fields.insert(field, value.into());
        self
    }

    pub fn missing(&self) -> BTreeSet<CardField> {
        CardField::ALL
            .into_iter()
            .filter(|field| {
                self.fields
                    .get(field)
                    .map(|value| value.trim().is_empty())
                    .unwrap_or(true)
            })
            .collect()
    }

    pub fn complete(&self) -> bool {
        self.missing().is_empty()
    }
}

/// 23.28's six preferred search methods, recorded and not implemented.
///
/// 23.28 also states an ordering constraint this crate has no way to violate: "online reinforcement
/// learning in production is delayed until protected holdouts, rollback, and policy boundaries are
/// mature". Nothing here learns online, or at all.
pub const SEARCH_METHODS: [&str; 6] = [
    "offline analysis",
    "contextual bandits",
    "Bayesian optimization",
    "evolutionary architecture search",
    "constrained policy search",
    "supervised imitation from verified traces",
];

/// 23.28's seven training-data sources, recorded and unconsumed.
pub const TRAINING_SOURCES: [&str; 7] = [
    "successful and failed Weave traces",
    "imported orchestration traces",
    "PRISM counterfactual suffixes",
    "minimized Decision Cells",
    "human corrections",
    "generated perturbations",
    "synthetic protocol conformance tasks",
];
