//! Microbenchmark generation, mutation relations, the oracle hierarchy, and scale accounting.
//!
//! Blueprint 23.27.
//!
//! # The one thing this module is for
//!
//! 23.27 ends its scale-accounting section with a sentence that is the whole reason this module
//! exists: **"Do not claim one million independent benchmarks when instances share parents."**
//! [`ScaleAccount`] has eight separately reported quantities and no `total()`. There is no method
//! anywhere that adds generated instances to parents, and [`ScaleAccount::independent_claim`]
//! refuses any claim larger than the number of audited parents, naming the ratio it refused.
//!
//! This is the same rule `bioprism-atlas` states as "instance count is not benchmark count", and it
//! is stated once more here because 23.27 is where the million-instance registry is proposed.
//!
//! # Mutation relations carry their own expectation
//!
//! A metamorphic relation is only useful if it says what should happen. 23.27 splits mutations into
//! semantics-preserving, controlled-semantic, and fault injection, and the split is enforced by
//! construction: [`Mutation::preserving`] takes no expectation because the expectation is
//! *invariance*, [`Mutation::controlled`] cannot be built without one, and
//! [`Mutation::fault`] records that correct behaviour is a recovery rather than an answer. There is
//! no constructor that produces a controlled-semantic mutation with an unstated consequence, which
//! is the failure mode that turns a mutation suite into noise.
//!
//! # Oracle ordering is a rule, not a preference
//!
//! 23.27 numbers seven oracles and 23.29 says "deterministic checks before model judges".
//! [`OraclePlan::check_order`] refuses a plan that reaches a model judge while a cheaper
//! deterministic oracle in the same plan has not run. The refusal names both oracles.
//!
//! # Not implemented
//!
//! Nothing is generated. There is no parent weave, no variant expansion, no execution, no scoring
//! and no registry storage; [`select`] samples from a caller-supplied instance list and
//! [`ScaleAccount`] counts numbers a caller supplies. 23.27's *evaluation levels* L0–L4 and its
//! *core microbenchmark families* are taxonomies with predicates over a plan, not executable
//! suites — asking whether a participant "can choose a recipient" requires a participant, and this
//! crate has none.

use crate::packs::Pack;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// 23.27's five evaluation levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Level {
    /// Conformance: schemas, message order, effects, identifiers.
    L0,
    /// Atomic coordination: one recipient, one commitment, one challenge, one stop.
    L1,
    /// Short protocol: several roles, bounded choreography, partial failure.
    L2,
    /// Full task, end to end.
    L3,
    /// Adaptive architecture: choosing and reconfiguring the team.
    L4,
}

impl Level {
    pub const ALL: [Level; 5] = [Level::L0, Level::L1, Level::L2, Level::L3, Level::L4];

    /// The question 23.27 asks at this level, verbatim in substance.
    pub fn question(self) -> &'static str {
        match self {
            Level::L0 => "does a participant parse schemas, follow message order, honor effects, and maintain identifiers",
            Level::L1 => "can it choose a recipient, create a valid commitment, challenge a defect, or stop correctly",
            Level::L2 => "can several roles complete a bounded choreography under partial failure",
            Level::L3 => "can a molecule solve an end-to-end task",
            Level::L4 => "can the system choose and reconfigure the right team and inference policy",
        }
    }

    /// Whether this level can be exercised by a single participant in isolation.
    ///
    /// L0 and L1 can; L2 upward need several roles by their own wording. The distinction matters
    /// for [`ScaleAccount`]: a suite claiming multi-agent coordination coverage from L0 instances
    /// alone has measured one agent many times.
    pub fn single_participant(self) -> bool {
        matches!(self, Level::L0 | Level::L1)
    }
}

/// 23.27's seven core microbenchmark families.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Family {
    Communication,
    EpistemicCoordination,
    Commitments,
    Authority,
    Topology,
    Execution,
    Aggregation,
}

impl Family {
    pub const ALL: [Family; 7] = [
        Family::Communication,
        Family::EpistemicCoordination,
        Family::Commitments,
        Family::Authority,
        Family::Topology,
        Family::Execution,
        Family::Aggregation,
    ];

    /// The behaviours 23.27 lists under this family.
    pub fn behaviours(self) -> &'static [&'static str] {
        match self {
            Family::Communication => &[
                "choose act type",
                "select information to transmit",
                "tailor resolution to recipient",
                "acknowledge accurately",
                "detect misunderstanding",
                "avoid redundant broadcast",
            ],
            Family::EpistemicCoordination => &[
                "distinguish claim from observation",
                "surface hidden assumptions",
                "preserve conflict",
                "request decisive evidence",
                "retract after contradiction",
                "maintain provenance",
            ],
            Family::Commitments => &[
                "create after acceptance",
                "delegate without losing accountability",
                "detect insufficient fulfillment",
                "handle deadline and compensation",
                "avoid orphan obligations",
            ],
            Family::Authority => &[
                "attenuate grants",
                "reject privilege escalation",
                "react to revocation",
                "separate execution from decision rights",
                "protect secrets",
            ],
            Family::Topology => &[
                "spawn, retire, fuse, or split at the right time",
                "select complementary agents",
                "avoid correlated redundancy",
                "recover after participant failure",
            ],
            Family::Execution => &[
                "continuation handoff",
                "fork/join correctness",
                "idempotent retry",
                "saga compensation",
                "stale state handling",
            ],
            Family::Aggregation => &[
                "choose deterministic evidence over votes",
                "preserve dissent",
                "handle malicious participant",
                "calibrate confidence",
                "detect false consensus",
            ],
        }
    }

    /// The 23.39 packs this family's behaviours land in.
    ///
    /// **Not in the blueprint.** 23.27 gives seven families and 23.39 gives twelve packs; they
    /// overlap heavily, share four names outright, and §23 never relates them. This mapping is this
    /// crate's reading, and the interesting consequence is in [`PACKS_WITH_NO_FAMILY`]: the
    /// families are *not* a cover of the packs, so a suite generated only from 23.27's family list
    /// omits three of 23.39's twelve packs entirely.
    pub fn packs(self) -> &'static [Pack] {
        match self {
            Family::Communication => &[Pack::ActSemantics, Pack::ContextCapsules],
            Family::EpistemicCoordination => &[Pack::EpistemicCoordination],
            Family::Commitments => &[Pack::Commitments],
            Family::Authority => &[Pack::Authority],
            Family::Topology => &[Pack::Topology],
            Family::Execution => &[Pack::Continuations, Pack::SagasAndRecovery],
            Family::Aggregation => &[Pack::Aggregation],
        }
    }
}

/// The 23.39 packs no 23.27 family reaches.
///
/// A generator driven by 23.27's family list alone produces nothing for negotiation and budgets,
/// semantic interoperability, or security and privacy. Stated as a constant so the omission is
/// visible in the API rather than discovered by a reader diffing two lists.
pub const PACKS_WITH_NO_FAMILY: [Pack; 3] = [
    Pack::NegotiationAndBudgets,
    Pack::SemanticInteroperability,
    Pack::SecurityAndPrivacy,
];

/// 23.27's sixteen procedural-generation axes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GenerationAxis {
    RoleToAgentBindings,
    ModelFamilies,
    Topology,
    MessageOrder,
    ContextResolution,
    EvidencePlacement,
    SchemaVersions,
    Budget,
    Latency,
    ParticipantDropout,
    MaliciousMessages,
    AuthorityScopes,
    DataLabels,
    ToolFaults,
    BranchCount,
    AggregationPolicy,
}

impl GenerationAxis {
    pub const ALL: [GenerationAxis; 16] = [
        GenerationAxis::RoleToAgentBindings,
        GenerationAxis::ModelFamilies,
        GenerationAxis::Topology,
        GenerationAxis::MessageOrder,
        GenerationAxis::ContextResolution,
        GenerationAxis::EvidencePlacement,
        GenerationAxis::SchemaVersions,
        GenerationAxis::Budget,
        GenerationAxis::Latency,
        GenerationAxis::ParticipantDropout,
        GenerationAxis::MaliciousMessages,
        GenerationAxis::AuthorityScopes,
        GenerationAxis::DataLabels,
        GenerationAxis::ToolFaults,
        GenerationAxis::BranchCount,
        GenerationAxis::AggregationPolicy,
    ];
}

/// What a mutation does to the expected behaviour of its parent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "expectation")]
pub enum Expectation {
    /// 23.27's semantics-preserving class: "the expected correct behavior remains invariant".
    Invariant,
    /// 23.27's controlled-semantic class: "expected behavior changes predictably", and the change
    /// is named. There is no way to build this variant without naming it.
    Changes { to: String },
    /// 23.27's fault-injection class. The instance has no expected *answer*; it has an expected
    /// *response*, which is not the same object and is kept in a different field.
    RecoveryRequired { response: String },
}

/// One mutation relation applied to a parent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mutation {
    pub relation: String,
    pub expectation: Expectation,
}

impl Mutation {
    /// A semantics-preserving mutation. Takes no expectation because the expectation *is* the class.
    pub fn preserving(relation: impl Into<String>) -> Self {
        Mutation {
            relation: relation.into(),
            expectation: Expectation::Invariant,
        }
    }

    /// A controlled-semantic mutation. The predicted change is a required argument.
    pub fn controlled(relation: impl Into<String>, to: impl Into<String>) -> Self {
        Mutation {
            relation: relation.into(),
            expectation: Expectation::Changes { to: to.into() },
        }
    }

    /// A fault injection. The required response is a required argument.
    pub fn fault(relation: impl Into<String>, response: impl Into<String>) -> Self {
        Mutation {
            relation: relation.into(),
            expectation: Expectation::RecoveryRequired {
                response: response.into(),
            },
        }
    }

    /// Whether a passing parent must also pass this mutant with the identical verdict.
    pub fn verdict_must_match_parent(&self) -> bool {
        matches!(self.expectation, Expectation::Invariant)
    }
}

/// 23.27's seven semantics-preserving relations, verbatim.
pub const SEMANTICS_PRESERVING: [&str; 7] = [
    "rename agents and roles",
    "reorder independent messages",
    "paraphrase rendered text",
    "change transport binding",
    "convert artifact format with verified adapter",
    "alter irrelevant topology labels",
    "inject redundant non-informative agent",
];

/// 23.27's eight controlled-semantic relations, verbatim.
pub const CONTROLLED_SEMANTIC: [&str; 8] = [
    "revoke a grant",
    "make evidence stale",
    "introduce a counterexample",
    "change one participant's capability",
    "remove a required artifact",
    "make an action irreversible",
    "change quorum rule",
    "alter cost or deadline",
];

/// 23.27's eight fault injections, verbatim.
pub const FAULT_INJECTION: [&str; 8] = [
    "duplicate message",
    "partial stream",
    "unavailable participant",
    "corrupted capsule",
    "stale continuation",
    "schema drift",
    "poisoned memory",
    "verifier failure",
];

/// 23.27's oracle hierarchy, in its own numbering.
///
/// Ordered so that `<` means "runs earlier and costs less", which is what makes
/// [`OraclePlan::check_order`] expressible.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Oracle {
    ProtocolStateMachine,
    TypeEffectAuthorityChecker,
    DeterministicWorldState,
    PropertyAndMetamorphic,
    ExecutionTest,
    StatisticalEvaluator,
    CalibratedExpertOrModelJudge,
}

impl Oracle {
    pub const ALL: [Oracle; 7] = [
        Oracle::ProtocolStateMachine,
        Oracle::TypeEffectAuthorityChecker,
        Oracle::DeterministicWorldState,
        Oracle::PropertyAndMetamorphic,
        Oracle::ExecutionTest,
        Oracle::StatisticalEvaluator,
        Oracle::CalibratedExpertOrModelJudge,
    ];

    /// Whether the oracle's verdict is a function of its inputs.
    ///
    /// The first four are; the last three are not. 23.29's control "deterministic checks before
    /// model judges" is stated over exactly this split.
    pub fn deterministic(self) -> bool {
        self <= Oracle::PropertyAndMetamorphic
    }
}

/// Why an oracle plan is not admissible.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[serde(rename_all = "snake_case", tag = "defect")]
pub enum PlanDefect {
    #[error("{judge:?} is scheduled before the deterministic oracle {deterministic:?}")]
    JudgeBeforeDeterministic {
        judge: Oracle,
        deterministic: Oracle,
    },

    #[error("the plan has no oracle")]
    Empty,
}

/// An ordered plan of oracles for one instance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OraclePlan {
    pub steps: Vec<Oracle>,
}

impl OraclePlan {
    pub fn new(steps: impl IntoIterator<Item = Oracle>) -> Self {
        OraclePlan {
            steps: steps.into_iter().collect(),
        }
    }

    /// 23.29's control as a check over 23.27's hierarchy.
    ///
    /// A plan is refused when a nondeterministic oracle is scheduled before a deterministic one
    /// the same plan also contains. Omitting the deterministic oracle entirely is *not* refused
    /// here — that is a coverage question, and conflating "ran in the wrong order" with "did not
    /// run" would give one error two meanings.
    pub fn check_order(&self) -> Result<(), PlanDefect> {
        if self.steps.is_empty() {
            return Err(PlanDefect::Empty);
        }
        for (index, step) in self.steps.iter().enumerate() {
            if step.deterministic() {
                continue;
            }
            if let Some(later) = self.steps[index + 1..]
                .iter()
                .copied()
                .find(|o| o.deterministic())
            {
                return Err(PlanDefect::JudgeBeforeDeterministic {
                    judge: *step,
                    deterministic: later,
                });
            }
        }
        Ok(())
    }
}

/// Why a scale claim was refused.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[serde(rename_all = "snake_case", tag = "refusal")]
pub enum ScaleRefusal {
    #[error("claimed {claimed} independent benchmarks from {parents} audited parents")]
    MoreIndependentThanParents { claimed: u64, parents: u64 },

    #[error("no parent weave programs were audited, so no independent claim is available")]
    NoAuditedParents,
}

/// 23.27's scale accounting: eight quantities, reported separately.
///
/// There is deliberately no `total()`, no `Sum`, and no field that adds two of these together. The
/// eight are not commensurable — a parent weave program and a validated generated instance are
/// different kinds of thing — and the single number a reader would take away from a total is
/// exactly the number 23.27 forbids claiming.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScaleAccount {
    pub parent_weave_programs: u64,
    pub parent_task_environments: u64,
    pub decision_cell_parents: u64,
    pub validated_generated_instances: u64,
    pub unique_mutation_relations: u64,
    pub effective_diversity: u64,
    pub executed_trials: u64,
    pub independent_human_audits: u64,
}

impl ScaleAccount {
    /// How many instances each parent weave program accounts for, rounded down.
    ///
    /// Returns `None` when there are no parents, because the ratio is undefined and a zero here
    /// would read as "no amplification" rather than "no denominator".
    pub fn instances_per_parent(&self) -> Option<u64> {
        (self.parent_weave_programs > 0)
            .then(|| self.validated_generated_instances / self.parent_weave_programs)
    }

    /// The largest number of *independent* benchmarks this account supports.
    ///
    /// 23.27: "Do not claim one million independent benchmarks when instances share parents."
    /// Independence is bounded above by the number of audited parents, so a claim beyond that is
    /// refused with the two numbers that refute it.
    pub fn independent_claim(&self, claimed: u64) -> Result<u64, ScaleRefusal> {
        if self.parent_weave_programs == 0 {
            return Err(ScaleRefusal::NoAuditedParents);
        }
        if claimed > self.parent_weave_programs {
            return Err(ScaleRefusal::MoreIndependentThanParents {
                claimed,
                parents: self.parent_weave_programs,
            });
        }
        Ok(claimed)
    }
}

/// 23.27's seven adaptive-selection criteria.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SelectionCriterion {
    CapabilityPosteriorUncertainty,
    ArchitectureChangeImpact,
    RegressionSimilarity,
    MandatorySafetyCoverage,
    TopologyAndMutationDiversity,
    ParentAwareStatistics,
    ComputeBudget,
}

impl SelectionCriterion {
    pub const ALL: [SelectionCriterion; 7] = [
        SelectionCriterion::CapabilityPosteriorUncertainty,
        SelectionCriterion::ArchitectureChangeImpact,
        SelectionCriterion::RegressionSimilarity,
        SelectionCriterion::MandatorySafetyCoverage,
        SelectionCriterion::TopologyAndMutationDiversity,
        SelectionCriterion::ParentAwareStatistics,
        SelectionCriterion::ComputeBudget,
    ];
}

/// One entry in a registry a selection may draw from.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Instance {
    pub id: String,
    pub parent: String,
    pub pack: Pack,
    /// Weight per criterion, supplied by the caller. This crate computes no priors.
    pub weights: BTreeMap<SelectionCriterion, u32>,
    /// Whether 23.27's "mandatory safety coverage" criterion applies to this instance.
    pub safety_mandatory: bool,
}

impl Instance {
    fn priority(&self, criteria: &BTreeSet<SelectionCriterion>) -> u32 {
        criteria
            .iter()
            .filter_map(|c| self.weights.get(c))
            .copied()
            .sum()
    }
}

/// Sample a routine-evaluation set from a registry, deterministically.
///
/// Two rules, both from 23.27's list. **Mandatory safety coverage is not a weight**: every instance
/// flagged safety-mandatory is included regardless of budget, and the budget applies to the rest —
/// a safety item that can be outbid is not mandatory. **Parent-aware statistics** is enforced
/// structurally: at most one instance per parent is drawn before any parent contributes a second,
/// so a budget of ten across two parents cannot return ten siblings.
///
/// Ties break on instance id, which is what makes the result reproducible.
pub fn select(
    registry: &[Instance],
    criteria: &BTreeSet<SelectionCriterion>,
    budget: usize,
) -> Vec<String> {
    let mut chosen: Vec<String> = registry
        .iter()
        .filter(|i| i.safety_mandatory)
        .map(|i| i.id.clone())
        .collect();
    chosen.sort();

    let mut remaining: Vec<&Instance> = registry.iter().filter(|i| !i.safety_mandatory).collect();
    remaining.sort_by(|a, b| {
        b.priority(criteria)
            .cmp(&a.priority(criteria))
            .then_with(|| a.id.cmp(&b.id))
    });

    let mut per_parent: BTreeMap<&str, usize> = BTreeMap::new();
    let mut round = 0usize;
    let mut taken = 0usize;
    while taken < budget {
        let mut progressed = false;
        for instance in &remaining {
            if taken >= budget {
                break;
            }
            let count = per_parent.entry(instance.parent.as_str()).or_insert(0);
            if *count != round {
                continue;
            }
            *count += 1;
            chosen.push(instance.id.clone());
            taken += 1;
            progressed = true;
        }
        if !progressed {
            break;
        }
        round += 1;
    }
    chosen
}

/// Why a registry may not be released at the claimed size.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[serde(rename_all = "snake_case", tag = "block")]
pub enum ReleaseBlock {
    #[error("validity is not demonstrated: {0}")]
    ValidityNotDemonstrated(String),

    #[error("diversity is not demonstrated: {0}")]
    DiversityNotDemonstrated(String),

    #[error("{available} validated instances available, {claimed} claimed")]
    ClaimExceedsValidated { available: u64, claimed: u64 },
}

/// 23.27's headline release: WeaveBench-100K from several hundred audited parents.
pub const HEADLINE_INSTANCES: u64 = 100_000;

/// 23.27: "a million-instance registry only **after** validity and diversity are demonstrated".
///
/// The order is the rule. A registry release above the headline size is refused unless both
/// demonstrations are present, and the two are reported separately because a suite can be valid
/// and homogeneous.
pub fn admit_release(
    account: &ScaleAccount,
    claimed_instances: u64,
    validity_demonstrated: bool,
    diversity_demonstrated: bool,
) -> Result<(), ReleaseBlock> {
    if claimed_instances > account.validated_generated_instances {
        return Err(ReleaseBlock::ClaimExceedsValidated {
            available: account.validated_generated_instances,
            claimed: claimed_instances,
        });
    }
    if claimed_instances > HEADLINE_INSTANCES {
        if !validity_demonstrated {
            return Err(ReleaseBlock::ValidityNotDemonstrated(
                "release above WeaveBench-100K requires a demonstrated validity result".into(),
            ));
        }
        if !diversity_demonstrated {
            return Err(ReleaseBlock::DiversityNotDemonstrated(
                "release above WeaveBench-100K requires a demonstrated diversity result".into(),
            ));
        }
    }
    Ok(())
}
