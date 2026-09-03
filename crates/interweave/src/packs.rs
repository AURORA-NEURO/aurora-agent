//! WeaveBench packs, the difficulty vector, parent-task admission, and multidimensional reporting.
//!
//! Blueprint 23.39.
//!
//! # The one thing this module is for
//!
//! 23.39 closes with **"No universal 'multi-agent intelligence' scalar is endorsed."**
//! [`Scorecard`] therefore has ten dimensions, no `overall()`, no `Ord`, no `PartialOrd`, and no
//! arithmetic. Comparing two scorecards returns [`Dominance`], whose most common answer on real
//! data is [`Dominance::Incomparable`] — a system better on epistemic quality and worse on cost is
//! not ranked, and there is no method that will rank it.
//!
//! This is the same shape `bioprism_fabric::reputation::Reputation` takes for a different reason.
//! There, a score without its context is meaningless; here, a score without its ten dimensions is
//! meaningless. Both refuse the collapse in the type rather than in a warning.
//!
//! # Difficulty is a vector too
//!
//! 23.39's twelve difficulty dimensions ([`Difficulty`]) do not sum either. Two instances at the
//! same "total difficulty" can differ by everything that matters — a six-role protocol with no
//! adversary is not a two-role protocol with one — so [`Difficulty::harder_than`] is a partial
//! order that returns `false` in both directions for incomparable pairs.
//!
//! # Parent tasks are admitted, not assumed
//!
//! 23.39 lists ten things "every parent includes". [`ParentTask::admit`] checks all ten and names
//! what is missing. Two of them are structural rather than documentary: the global choreography is
//! a `bioprism_choreography::WellFormedGlobal`, so a parent whose protocol is not well-formed
//! cannot be constructed, and the participant-local views are that global's own projections rather
//! than a separately authored list that could drift from it.
//!
//! # Not implemented
//!
//! No pack content. The twelve packs are a taxonomy with their item lists, and not one instance of
//! any of them exists in this workspace: writing them needs worlds, participants and an execution
//! substrate that this crate deliberately does not have. `baseline results` is a required
//! deliverable that no [`ParentTask`] here supplies, which is why [`ParentTask::admit`] on every
//! parent this crate can currently build reports it missing.

use bioprism_choreography::{Role, WellFormedGlobal};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// 23.39's twelve packs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Pack {
    ActSemantics,
    ContextCapsules,
    Commitments,
    Authority,
    EpistemicCoordination,
    Continuations,
    Topology,
    Aggregation,
    NegotiationAndBudgets,
    SagasAndRecovery,
    SemanticInteroperability,
    SecurityAndPrivacy,
}

impl Pack {
    pub const ALL: [Pack; 12] = [
        Pack::ActSemantics,
        Pack::ContextCapsules,
        Pack::Commitments,
        Pack::Authority,
        Pack::EpistemicCoordination,
        Pack::Continuations,
        Pack::Topology,
        Pack::Aggregation,
        Pack::NegotiationAndBudgets,
        Pack::SagasAndRecovery,
        Pack::SemanticInteroperability,
        Pack::SecurityAndPrivacy,
    ];

    /// 23.39's pack number, one-based as printed.
    pub fn number(self) -> u8 {
        match self {
            Pack::ActSemantics => 1,
            Pack::ContextCapsules => 2,
            Pack::Commitments => 3,
            Pack::Authority => 4,
            Pack::EpistemicCoordination => 5,
            Pack::Continuations => 6,
            Pack::Topology => 7,
            Pack::Aggregation => 8,
            Pack::NegotiationAndBudgets => 9,
            Pack::SagasAndRecovery => 10,
            Pack::SemanticInteroperability => 11,
            Pack::SecurityAndPrivacy => 12,
        }
    }

    /// The items 23.39 lists in this pack, verbatim.
    pub fn items(self) -> &'static [&'static str] {
        match self {
            Pack::ActSemantics => &[
                "distinguish claim, inform, attest, and accept",
                "send valid payload type",
                "reject out-of-order act",
                "handle acknowledgment level",
                "retract without deleting history",
            ],
            Pack::ContextCapsules => &[
                "choose decisive evidence",
                "omit distractors",
                "preserve assumptions and provenance",
                "request expansion",
                "tailor to recipient capability",
                "track context debt",
            ],
            Pack::Commitments => &[
                "create and accept obligation",
                "delegate execution",
                "preserve accountability",
                "verify satisfaction",
                "handle timeout, violation, and compensation",
            ],
            Pack::Authority => &[
                "grant attenuation",
                "transitive revocation",
                "secret brokering",
                "decision-right separation",
                "confused-deputy resistance",
                "irreversible-action gate",
            ],
            Pack::EpistemicCoordination => &[
                "preserve conflicting claims",
                "distinguish working assumption from verified fact",
                "assess source dependence",
                "request missing evidence obligation",
                "challenge unsupported conclusion",
                "maintain applicability scope",
            ],
            Pack::Continuations => &[
                "move versus fork",
                "resume with sufficient state",
                "reject stale snapshot",
                "preserve open commitments",
                "merge compatible branches",
                "avoid production effects in replay",
            ],
            Pack::Topology => &[
                "spawn specialist",
                "retire redundant agent",
                "choose star, jury, pipeline, or blackboard",
                "recover after dropout",
                "fuse a stable subteam",
                "avoid topology thrashing",
            ],
            Pack::Aggregation => &[
                "deterministic reducer",
                "evidence-weighted synthesis",
                "quorum with veto",
                "correlated-agent correction",
                "malicious juror",
                "dissent preservation",
            ],
            Pack::NegotiationAndBudgets => &[
                "evaluate offers",
                "allocate budget leases",
                "request justified expansion",
                "handle service-level failure",
                "avoid denial-of-wallet",
                "stop when marginal value is low",
            ],
            Pack::SagasAndRecovery => &[
                "idempotent retry",
                "compensation ordering",
                "partial failure",
                "rebind role",
                "reconcile diverged state",
                "downgrade assurance visibly",
            ],
            Pack::SemanticInteroperability => &[
                "schema negotiation",
                "unit conversion",
                "ontology mismatch",
                "lossy adapter",
                "transport switch with invariant semantics",
                "opaque fallback",
            ],
            Pack::SecurityAndPrivacy => &[
                "prompt injection across agents",
                "data label propagation",
                "capsule minimization",
                "sybil consensus",
                "malicious capability card",
                "trace redaction",
                "public bundle leakage",
            ],
        }
    }
}

/// 23.39's twelve difficulty dimensions.
///
/// A vector, never a sum. The fields are `u8` because the blueprint gives no scale; a caller may
/// use any monotone encoding it likes as long as it uses one consistently, and comparisons are
/// per-dimension so the encoding never has to be commensurable across dimensions.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Difficulty {
    pub number_of_roles: u8,
    pub protocol_horizon: u8,
    pub partial_observability: u8,
    pub message_ambiguity: u8,
    pub schema_distance: u8,
    pub world_state_complexity: u8,
    pub authority_depth: u8,
    pub budget_pressure: u8,
    pub failure_rate: u8,
    pub adversariality: u8,
    pub irreversibility: u8,
    pub required_evidence_strength: u8,
}

impl Difficulty {
    /// The twelve components in the blueprint's order, for iteration.
    pub fn components(&self) -> [(&'static str, u8); 12] {
        [
            ("number of roles", self.number_of_roles),
            ("protocol horizon", self.protocol_horizon),
            ("partial observability", self.partial_observability),
            ("message ambiguity", self.message_ambiguity),
            ("schema distance", self.schema_distance),
            ("world-state complexity", self.world_state_complexity),
            ("authority depth", self.authority_depth),
            ("budget pressure", self.budget_pressure),
            ("failure rate", self.failure_rate),
            ("adversariality", self.adversariality),
            ("irreversibility", self.irreversibility),
            (
                "required evidence strength",
                self.required_evidence_strength,
            ),
        ]
    }

    /// Dominance in the partial order: at least as hard everywhere, strictly harder somewhere.
    ///
    /// Two instances that trade dimensions are incomparable and this returns `false` for both
    /// orderings, which is the honest answer and the reason there is no `Ord`.
    pub fn harder_than(&self, other: &Difficulty) -> bool {
        let mine = self.components();
        let theirs = other.components();
        let all_ge = mine.iter().zip(theirs.iter()).all(|(a, b)| a.1 >= b.1);
        let any_gt = mine.iter().zip(theirs.iter()).any(|(a, b)| a.1 > b.1);
        all_ge && any_gt
    }
}

/// 23.39's ten reporting dimensions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScoreDimension {
    ProtocolConformance,
    TaskUtility,
    CommunicationValue,
    CommitmentIntegrity,
    AuthoritySafety,
    EpistemicQuality,
    Recovery,
    CostAndLatency,
    Robustness,
    Calibration,
}

impl ScoreDimension {
    pub const ALL: [ScoreDimension; 10] = [
        ScoreDimension::ProtocolConformance,
        ScoreDimension::TaskUtility,
        ScoreDimension::CommunicationValue,
        ScoreDimension::CommitmentIntegrity,
        ScoreDimension::AuthoritySafety,
        ScoreDimension::EpistemicQuality,
        ScoreDimension::Recovery,
        ScoreDimension::CostAndLatency,
        ScoreDimension::Robustness,
        ScoreDimension::Calibration,
    ];
}

/// One dimension's result.
///
/// Following `bioprism-atlas`: a dimension with no evidence is [`Measurement::Unmeasured`], which
/// is categorically distinct from a measured zero. There is no constructor that turns the first
/// into the second.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "measurement")]
pub enum Measurement {
    /// Basis points, 0–10000, so the type carries no float and comparison is exact.
    Measured {
        basis_points: u16,
    },
    Unmeasured,
}

impl Measurement {
    pub fn measured(basis_points: u16) -> Self {
        Measurement::Measured {
            basis_points: basis_points.min(10_000),
        }
    }
}

/// A per-dimension comparison of two scorecards.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DimensionComparison {
    Better,
    Worse,
    Equal,
    /// At least one side is unmeasured on this dimension.
    Undetermined,
}

/// The verdict of comparing two scorecards.
///
/// `Incomparable` is the point of the type. Note that a single [`Measurement::Unmeasured`] on
/// either side makes the whole comparison [`Dominance::Undetermined`] rather than silently
/// dropping that dimension from the comparison — dropping it is how an unmeasured weakness turns
/// into a win.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "dominance")]
pub enum Dominance {
    LeftDominates,
    RightDominates,
    Equivalent,
    Incomparable {
        left_better: BTreeSet<ScoreDimension>,
        right_better: BTreeSet<ScoreDimension>,
    },
    Undetermined {
        unmeasured: BTreeSet<ScoreDimension>,
    },
}

/// 23.39's multidimensional score.
///
/// No `Ord`, no `PartialOrd`, no `overall()`, no `sum()`, no weights argument anywhere. The only
/// comparison offered is [`compare`], which is allowed to say the two are not comparable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Scorecard {
    pub system: String,
    scores: BTreeMap<ScoreDimension, Measurement>,
}

impl Scorecard {
    pub fn new(system: impl Into<String>) -> Self {
        Scorecard {
            system: system.into(),
            scores: ScoreDimension::ALL
                .into_iter()
                .map(|d| (d, Measurement::Unmeasured))
                .collect(),
        }
    }

    pub fn scoring(mut self, dimension: ScoreDimension, measurement: Measurement) -> Self {
        self.scores.insert(dimension, measurement);
        self
    }

    /// A dimension never recorded reads as unmeasured, never as zero.
    pub fn score(&self, dimension: ScoreDimension) -> Measurement {
        self.scores
            .get(&dimension)
            .copied()
            .unwrap_or(Measurement::Unmeasured)
    }

    pub fn unmeasured(&self) -> BTreeSet<ScoreDimension> {
        ScoreDimension::ALL
            .into_iter()
            .filter(|d| self.score(*d) == Measurement::Unmeasured)
            .collect()
    }
}

/// Compare two scorecards dimension by dimension.
pub fn compare(left: &Scorecard, right: &Scorecard) -> Dominance {
    let unmeasured: BTreeSet<ScoreDimension> = left
        .unmeasured()
        .union(&right.unmeasured())
        .copied()
        .collect();
    if !unmeasured.is_empty() {
        return Dominance::Undetermined { unmeasured };
    }
    let mut left_better = BTreeSet::new();
    let mut right_better = BTreeSet::new();
    for dimension in ScoreDimension::ALL {
        match (left.score(dimension), right.score(dimension)) {
            (
                Measurement::Measured { basis_points: a },
                Measurement::Measured { basis_points: b },
            ) => {
                if a > b {
                    left_better.insert(dimension);
                } else if b > a {
                    right_better.insert(dimension);
                }
            }
            _ => {
                return Dominance::Undetermined {
                    unmeasured: [dimension].into_iter().collect(),
                };
            }
        }
    }
    match (left_better.is_empty(), right_better.is_empty()) {
        (true, true) => Dominance::Equivalent,
        (false, true) => Dominance::LeftDominates,
        (true, false) => Dominance::RightDominates,
        (false, false) => Dominance::Incomparable {
            left_better,
            right_better,
        },
    }
}

/// Compare one dimension of two scorecards.
pub fn compare_dimension(
    left: &Scorecard,
    right: &Scorecard,
    dimension: ScoreDimension,
) -> DimensionComparison {
    match (left.score(dimension), right.score(dimension)) {
        (Measurement::Measured { basis_points: a }, Measurement::Measured { basis_points: b }) => {
            match a.cmp(&b) {
                std::cmp::Ordering::Greater => DimensionComparison::Better,
                std::cmp::Ordering::Less => DimensionComparison::Worse,
                std::cmp::Ordering::Equal => DimensionComparison::Equal,
            }
        }
        _ => DimensionComparison::Undetermined,
    }
}

/// 23.39's ten parent-task requirements, as a checklist.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParentRequirement {
    ExecutableOrSimulatableWorld,
    GlobalChoreography,
    ParticipantLocalViews,
    AcceptableActionSet,
    DeterministicOrReviewableOracle,
    FailureInjections,
    MutationRelations,
    ProvenanceAndLicense,
    HumanAuditNotes,
    BaselineResults,
}

impl ParentRequirement {
    pub const ALL: [ParentRequirement; 10] = [
        ParentRequirement::ExecutableOrSimulatableWorld,
        ParentRequirement::GlobalChoreography,
        ParentRequirement::ParticipantLocalViews,
        ParentRequirement::AcceptableActionSet,
        ParentRequirement::DeterministicOrReviewableOracle,
        ParentRequirement::FailureInjections,
        ParentRequirement::MutationRelations,
        ParentRequirement::ProvenanceAndLicense,
        ParentRequirement::HumanAuditNotes,
        ParentRequirement::BaselineResults,
    ];
}

/// A parent task offered to the registry.
///
/// The choreography is a `WellFormedGlobal` rather than a `GlobalType`, following the rule
/// `bioprism-fabric` set for the same reason: well-formedness is decided once, in the crate that
/// owns session types, and a parent that could deadlock cannot be smuggled in as data.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParentTask {
    pub id: String,
    pub pack: Pack,
    pub difficulty: Difficulty,
    pub choreography: WellFormedGlobal,
    /// The documentary requirements the author claims to have supplied. The two structural ones —
    /// global choreography and participant-local views — are not listed here because they are
    /// discharged by the `choreography` field itself.
    pub supplied: BTreeSet<ParentRequirement>,
}

impl ParentTask {
    pub fn new(
        id: impl Into<String>,
        pack: Pack,
        difficulty: Difficulty,
        choreography: WellFormedGlobal,
    ) -> Self {
        ParentTask {
            id: id.into(),
            pack,
            difficulty,
            choreography,
            supplied: BTreeSet::new(),
        }
    }

    pub fn supplying(mut self, requirement: ParentRequirement) -> Self {
        self.supplied.insert(requirement);
        self
    }

    /// The roles the protocol actually contains, which is what "participant-local views" projects
    /// over. Derived rather than declared, so it cannot drift from the choreography.
    pub fn roles(&self) -> BTreeSet<Role> {
        self.choreography.roles().cloned().collect()
    }

    /// Which of the ten requirements are missing.
    ///
    /// `GlobalChoreography` is never missing — the field is non-optional. `ParticipantLocalViews`
    /// is missing exactly when the protocol has fewer than two roles, since a one-role protocol has
    /// no local views worth projecting and is not a multi-party parent.
    pub fn missing(&self) -> BTreeSet<ParentRequirement> {
        ParentRequirement::ALL
            .into_iter()
            .filter(|requirement| match requirement {
                ParentRequirement::GlobalChoreography => false,
                ParentRequirement::ParticipantLocalViews => self.roles().len() < 2,
                other => !self.supplied.contains(other),
            })
            .collect()
    }

    /// Whether every requirement is met.
    pub fn admit(&self) -> Result<(), BTreeSet<ParentRequirement>> {
        let missing = self.missing();
        if missing.is_empty() {
            Ok(())
        } else {
            Err(missing)
        }
    }
}
