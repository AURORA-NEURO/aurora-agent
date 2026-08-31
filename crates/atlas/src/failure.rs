//! The failure taxonomy.
//!
//! Implements blueprint 03.10, with the localisation vocabulary of 33.18. The purpose is to make
//! a failure *diagnosed* rather than counted: a bare failure count tells you a system is worse
//! without telling you what to fix, and 33.18's worked example is the whole argument — a wrong
//! biological conclusion traced to a sample-identity join *before* the statistical analysis
//! prevents wasted effort on model tuning.
//!
//! Three design decisions follow from that.
//!
//! The mechanism set is **closed**. There is no free-text bucket. What there is instead is
//! [`FailureMechanism::Unclassified`], which means the taxonomy did not cover the observation —
//! a defect in this enum, reported by [`crate::Inconsistency::UnclassifiedFailure`] and counted
//! as taxonomy debt in the coverage report. A free-text field would make the same failure
//! invisible by making it unaggregatable.
//!
//! The **stage is derived from the mechanism**, not declared alongside it. 03.10 lists stage and
//! mechanism as separate axes, but a record that says "tool schema misunderstood, stage:
//! communication" is simply wrong, and the cheapest way to make a wrong state unreachable is not
//! to represent it.
//!
//! The **causal chain is not flattened**. 03.10: "One run may have an initiating cause, first
//! causal divergence, intermediate manifestations, and terminal failure. Labels should not
//! flatten these into one category." [`CausalChain`] keeps all four and refuses an ordering in
//! which the divergence comes after what it is supposed to have caused.
//!
//! Every record names the capability it implicates, and [`crate::AtlasBuilder::build`] refuses a
//! record naming a capability the ontology does not contain — an unattributable failure is a
//! failure nobody owns.
//!
//! NOT implemented: automated label inference, minimal-failure extraction, and the propagation
//! graph of 33.18. Those need trajectory replay, which lives in the runtime, not the atlas. This
//! module is the vocabulary and its integrity rules.

use crate::error::AtlasError;
use crate::ontology::CapabilityId;
use bioprism_ids::RunId;
use serde::de::Error as DeError;
use serde::{Deserialize, Deserializer, Serialize};
use std::fmt;

/// Where in an agent's trajectory a failure sits. Derived from the mechanism; never declared.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureStage {
    Observation,
    Context,
    Reasoning,
    Planning,
    Tools,
    Memory,
    Execution,
    Verification,
    Control,
    Communication,
    Safety,
}

impl FailureStage {
    pub fn as_str(self) -> &'static str {
        match self {
            FailureStage::Observation => "observation",
            FailureStage::Context => "context",
            FailureStage::Reasoning => "reasoning",
            FailureStage::Planning => "planning",
            FailureStage::Tools => "tools",
            FailureStage::Memory => "memory",
            FailureStage::Execution => "execution",
            FailureStage::Verification => "verification",
            FailureStage::Control => "control",
            FailureStage::Communication => "communication",
            FailureStage::Safety => "safety",
        }
    }
}

impl fmt::Display for FailureStage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The closed mechanism set. The first eight are the worked examples of 03.10, transcribed; the
/// remainder close the stage coverage that 03.10's responsibilities list requires.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureMechanism {
    RelevantEvidenceNotAcquired,
    StaleEvidenceTrusted,
    HypothesisCollapsedTooEarly,
    ToolSchemaMisunderstood,
    SideEffectMaterialisedPrematurely,
    SuccessfulCommandMistakenForTaskSuccess,
    VerifierAcceptedHackedReward,
    MemoryContaminatedNewTask,
    ContextOmittedDecisionChangingEvidence,
    PlanIgnoredKnownConstraint,
    RecoveryNotAttemptedAfterDetection,
    CoordinationDuplicatedOrDeadlocked,
    UncertaintyMisreportedToCaller,
    SafetyBoundaryCrossed,
    /// The taxonomy did not cover the observation. Not a bucket to hide in: this is a defect
    /// report against this enum, and the atlas counts it as such.
    Unclassified,
}

impl FailureMechanism {
    pub const CLOSED_SET: [FailureMechanism; 15] = [
        FailureMechanism::RelevantEvidenceNotAcquired,
        FailureMechanism::StaleEvidenceTrusted,
        FailureMechanism::HypothesisCollapsedTooEarly,
        FailureMechanism::ToolSchemaMisunderstood,
        FailureMechanism::SideEffectMaterialisedPrematurely,
        FailureMechanism::SuccessfulCommandMistakenForTaskSuccess,
        FailureMechanism::VerifierAcceptedHackedReward,
        FailureMechanism::MemoryContaminatedNewTask,
        FailureMechanism::ContextOmittedDecisionChangingEvidence,
        FailureMechanism::PlanIgnoredKnownConstraint,
        FailureMechanism::RecoveryNotAttemptedAfterDetection,
        FailureMechanism::CoordinationDuplicatedOrDeadlocked,
        FailureMechanism::UncertaintyMisreportedToCaller,
        FailureMechanism::SafetyBoundaryCrossed,
        FailureMechanism::Unclassified,
    ];

    /// `None` only for [`FailureMechanism::Unclassified`]: an unclassified failure has no stage
    /// because nobody localised it, and reporting a stage anyway would be an invention.
    pub fn stage(self) -> Option<FailureStage> {
        Some(match self {
            FailureMechanism::RelevantEvidenceNotAcquired => FailureStage::Observation,
            FailureMechanism::StaleEvidenceTrusted => FailureStage::Observation,
            FailureMechanism::ContextOmittedDecisionChangingEvidence => FailureStage::Context,
            FailureMechanism::HypothesisCollapsedTooEarly => FailureStage::Reasoning,
            FailureMechanism::PlanIgnoredKnownConstraint => FailureStage::Planning,
            FailureMechanism::ToolSchemaMisunderstood => FailureStage::Tools,
            FailureMechanism::MemoryContaminatedNewTask => FailureStage::Memory,
            FailureMechanism::SideEffectMaterialisedPrematurely => FailureStage::Execution,
            FailureMechanism::SuccessfulCommandMistakenForTaskSuccess => FailureStage::Verification,
            FailureMechanism::VerifierAcceptedHackedReward => FailureStage::Verification,
            FailureMechanism::RecoveryNotAttemptedAfterDetection => FailureStage::Control,
            FailureMechanism::CoordinationDuplicatedOrDeadlocked => FailureStage::Control,
            FailureMechanism::UncertaintyMisreportedToCaller => FailureStage::Communication,
            FailureMechanism::SafetyBoundaryCrossed => FailureStage::Safety,
            FailureMechanism::Unclassified => return None,
        })
    }

    pub fn is_classified(self) -> bool {
        !matches!(self, FailureMechanism::Unclassified)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            FailureMechanism::RelevantEvidenceNotAcquired => "relevant_evidence_not_acquired",
            FailureMechanism::StaleEvidenceTrusted => "stale_evidence_trusted",
            FailureMechanism::HypothesisCollapsedTooEarly => "hypothesis_collapsed_too_early",
            FailureMechanism::ToolSchemaMisunderstood => "tool_schema_misunderstood",
            FailureMechanism::SideEffectMaterialisedPrematurely => {
                "side_effect_materialised_prematurely"
            }
            FailureMechanism::SuccessfulCommandMistakenForTaskSuccess => {
                "successful_command_mistaken_for_task_success"
            }
            FailureMechanism::VerifierAcceptedHackedReward => "verifier_accepted_hacked_reward",
            FailureMechanism::MemoryContaminatedNewTask => "memory_contaminated_new_task",
            FailureMechanism::ContextOmittedDecisionChangingEvidence => {
                "context_omitted_decision_changing_evidence"
            }
            FailureMechanism::PlanIgnoredKnownConstraint => "plan_ignored_known_constraint",
            FailureMechanism::RecoveryNotAttemptedAfterDetection => {
                "recovery_not_attempted_after_detection"
            }
            FailureMechanism::CoordinationDuplicatedOrDeadlocked => {
                "coordination_duplicated_or_deadlocked"
            }
            FailureMechanism::UncertaintyMisreportedToCaller => "uncertainty_misreported_to_caller",
            FailureMechanism::SafetyBoundaryCrossed => "safety_boundary_crossed",
            FailureMechanism::Unclassified => "unclassified",
        }
    }
}

impl fmt::Display for FailureMechanism {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Whether the evidence behind the diagnosis survived. 03.10 requires evidence preservation; a
/// diagnosis built on lost evidence is a hypothesis and is labelled as one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceStatus {
    Preserved,
    PartiallyPreserved,
    Lost,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Reversibility {
    Reversible,
    ReversibleWithCost,
    Irreversible,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Detectability {
    DetectedByDeterministicCheck,
    DetectedByReview,
    Undetected,
}

/// Ascending severity. `WrongConclusion` and above are the cases 33.01 forbids a composite score
/// from compensating for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Cosmetic,
    Degraded,
    WrongConclusion,
    UnsafeAction,
}

/// 03.10: "whether the failure is task-, environment-, model-, or evaluator-induced". An
/// evaluator-induced failure is a benchmark defect and must not be charged to the system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Inducement {
    TaskInduced,
    EnvironmentInduced,
    ModelInduced,
    EvaluatorInduced,
}

/// The non-mechanism axes of 03.10.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FailureAxes {
    pub evidence_status: EvidenceStatus,
    pub reversibility: Reversibility,
    pub detectability: Detectability,
    pub severity: Severity,
    pub inducement: Inducement,
    /// How many times this failure family has already been seen. 33.18 tracks recurrence because
    /// a recurring failure is a fix that did not transfer.
    #[serde(default)]
    pub recurrence: usize,
    /// Which architecture component the diagnosis points at, when one is identified. `None` is
    /// honest; a placeholder string is not.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub architecture_component: Option<String>,
}

impl FailureAxes {
    pub fn new(
        evidence_status: EvidenceStatus,
        reversibility: Reversibility,
        detectability: Detectability,
        severity: Severity,
        inducement: Inducement,
    ) -> Self {
        FailureAxes {
            evidence_status,
            reversibility,
            detectability,
            severity,
            inducement,
            recurrence: 0,
            architecture_component: None,
        }
    }

    pub fn with_recurrence(mut self, recurrence: usize) -> Self {
        self.recurrence = recurrence;
        self
    }

    pub fn with_component(mut self, component: impl Into<String>) -> Self {
        self.architecture_component = Some(component.into());
        self
    }

    /// Whether the diagnosis is charged to the system under test at all.
    pub fn charges_the_system(&self) -> bool {
        !matches!(self.inducement, Inducement::EvaluatorInduced)
    }
}

/// One point on the trajectory, localised by step index.
///
/// The step is what makes 33.18's "first divergence" checkable rather than rhetorical: without an
/// index there is no way to assert that the divergence preceded the failure it explains.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct FailureLabel {
    pub mechanism: FailureMechanism,
    pub step: usize,
}

impl FailureLabel {
    pub fn new(mechanism: FailureMechanism, step: usize) -> Self {
        FailureLabel { mechanism, step }
    }

    /// Derived, never declared. See the module docs.
    pub fn stage(&self) -> Option<FailureStage> {
        self.mechanism.stage()
    }
}

/// The four-part causal structure of 03.10, kept apart.
///
/// Ordering is enforced on construction: the initiating cause is at or before the first
/// divergence, every manifestation is at or after the divergence, and the terminal failure is
/// last. A chain that violates this is not a weak diagnosis, it is an incoherent one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CausalChain {
    initiating_cause: FailureLabel,
    first_divergence: FailureLabel,
    manifestations: Vec<FailureLabel>,
    terminal: FailureLabel,
}

#[derive(Debug, Deserialize)]
struct CausalChainFields {
    initiating_cause: FailureLabel,
    first_divergence: FailureLabel,
    manifestations: Vec<FailureLabel>,
    terminal: FailureLabel,
}

impl<'de> Deserialize<'de> for CausalChain {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let fields = CausalChainFields::deserialize(deserializer)?;
        Self::new(
            "deserialized",
            fields.initiating_cause,
            fields.first_divergence,
            fields.manifestations,
            fields.terminal,
        )
        .map_err(D::Error::custom)
    }
}

impl CausalChain {
    pub fn new(
        failure_id: &str,
        initiating_cause: FailureLabel,
        first_divergence: FailureLabel,
        manifestations: Vec<FailureLabel>,
        terminal: FailureLabel,
    ) -> Result<Self, AtlasError> {
        if initiating_cause.step > first_divergence.step {
            return Err(AtlasError::ChainOutOfOrder {
                failure_id: failure_id.to_string(),
                label: "first_divergence",
                step: first_divergence.step,
                predecessor: "initiating_cause",
                predecessor_step: initiating_cause.step,
            });
        }
        for manifestation in &manifestations {
            if manifestation.step < first_divergence.step {
                return Err(AtlasError::ChainOutOfOrder {
                    failure_id: failure_id.to_string(),
                    label: "manifestation",
                    step: manifestation.step,
                    predecessor: "first_divergence",
                    predecessor_step: first_divergence.step,
                });
            }
            if manifestation.step > terminal.step {
                return Err(AtlasError::ChainOutOfOrder {
                    failure_id: failure_id.to_string(),
                    label: "terminal",
                    step: terminal.step,
                    predecessor: "manifestation",
                    predecessor_step: manifestation.step,
                });
            }
        }
        if terminal.step < first_divergence.step {
            return Err(AtlasError::ChainOutOfOrder {
                failure_id: failure_id.to_string(),
                label: "terminal",
                step: terminal.step,
                predecessor: "first_divergence",
                predecessor_step: first_divergence.step,
            });
        }
        Ok(CausalChain {
            initiating_cause,
            first_divergence,
            manifestations,
            terminal,
        })
    }

    pub fn initiating_cause(&self) -> FailureLabel {
        self.initiating_cause
    }

    /// The localisation 33.18 exists to produce: "the earliest decision that made downstream
    /// success materially less likely".
    pub fn first_divergence(&self) -> FailureLabel {
        self.first_divergence
    }

    pub fn manifestations(&self) -> &[FailureLabel] {
        &self.manifestations
    }

    pub fn terminal(&self) -> FailureLabel {
        self.terminal
    }

    pub fn labels(&self) -> Vec<FailureLabel> {
        let mut out = vec![self.initiating_cause, self.first_divergence];
        out.extend(self.manifestations.iter().copied());
        out.push(self.terminal);
        out
    }

    /// Whether the chain says the same thing four times. Reported rather than refused: a genuinely
    /// immediate failure is flat, and the atlas cannot tell that apart from a lazy label. What it
    /// can do is stop such a record from counting as a localisation.
    pub fn is_flattened(&self) -> bool {
        let mechanism = self.initiating_cause.mechanism;
        self.labels().iter().all(|l| l.mechanism == mechanism)
            && self.initiating_cause.step == self.terminal.step
    }

    /// How far the error propagated before it terminated, in trajectory steps.
    pub fn propagation_span(&self) -> usize {
        self.terminal
            .step
            .saturating_sub(self.first_divergence.step)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LabelWeight {
    pub mechanism: FailureMechanism,
    pub weight: f64,
}

/// A distribution over mechanisms, with the reviewers' rationale attached.
///
/// 03.10: "Reviewers may disagree or evidence may be incomplete. The record retains label
/// distributions and rationale rather than forcing false certainty." So [`LabelDistribution::modal`]
/// returns `Option` and yields `None` on a tie — a two-way tie has no majority, and inventing one
/// by declaration order would turn a disagreement into a fact.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LabelDistribution {
    weights: Vec<LabelWeight>,
    pub rationale: String,
}

#[derive(Debug, Deserialize)]
struct LabelDistributionFields {
    weights: Vec<LabelWeight>,
    rationale: String,
}

impl<'de> Deserialize<'de> for LabelDistribution {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let fields = LabelDistributionFields::deserialize(deserializer)?;
        let weights = fields
            .weights
            .into_iter()
            .map(|entry| (entry.mechanism, entry.weight));
        Self::contested("deserialized", weights, fields.rationale).map_err(D::Error::custom)
    }
}

impl LabelDistribution {
    /// The uncontested case: one mechanism, full weight.
    pub fn certain(mechanism: FailureMechanism, rationale: impl Into<String>) -> Self {
        LabelDistribution {
            weights: vec![LabelWeight {
                mechanism,
                weight: 1.0,
            }],
            rationale: rationale.into(),
        }
    }

    /// The contested case. Weights must be positive, finite, and sum to one within 1e-9.
    pub fn contested(
        failure_id: &str,
        weights: impl IntoIterator<Item = (FailureMechanism, f64)>,
        rationale: impl Into<String>,
    ) -> Result<Self, AtlasError> {
        let mut collected: Vec<LabelWeight> = weights
            .into_iter()
            .map(|(mechanism, weight)| LabelWeight { mechanism, weight })
            .collect();
        if collected.is_empty() {
            return Err(AtlasError::EmptyLabelDistribution {
                failure_id: failure_id.to_string(),
            });
        }
        for entry in &collected {
            if !entry.weight.is_finite() || entry.weight <= 0.0 {
                return Err(AtlasError::MalformedLabelWeight {
                    failure_id: failure_id.to_string(),
                    mechanism: entry.mechanism.as_str(),
                    weight: entry.weight,
                });
            }
        }
        collected.sort_by_key(|entry| entry.mechanism);
        if let Some(pair) = collected
            .windows(2)
            .find(|pair| pair[0].mechanism == pair[1].mechanism)
        {
            return Err(AtlasError::DuplicateLabelMechanism {
                failure_id: failure_id.to_string(),
                mechanism: pair[0].mechanism.as_str(),
            });
        }
        let total: f64 = collected.iter().map(|e| e.weight).sum();
        if (total - 1.0).abs() > 1e-9 {
            return Err(AtlasError::MalformedLabelDistribution {
                failure_id: failure_id.to_string(),
                total,
            });
        }
        Ok(LabelDistribution {
            weights: collected,
            rationale: rationale.into(),
        })
    }

    pub fn weights(&self) -> &[LabelWeight] {
        &self.weights
    }

    pub fn weight_of(&self, mechanism: FailureMechanism) -> f64 {
        self.weights
            .iter()
            .find(|e| e.mechanism == mechanism)
            .map_or(0.0, |e| e.weight)
    }

    pub fn is_contested(&self) -> bool {
        self.weights.len() > 1
    }

    /// The single mechanism the reviewers agree on, if there is one. `None` on a tie.
    pub fn modal(&self) -> Option<FailureMechanism> {
        let best = self
            .weights
            .iter()
            .max_by(|a, b| a.weight.total_cmp(&b.weight))?;
        let tied = self
            .weights
            .iter()
            .filter(|e| (e.weight - best.weight).abs() <= 1e-12)
            .count();
        if tied > 1 {
            return None;
        }
        Some(best.mechanism)
    }

    /// Whether any weight at all sits on [`FailureMechanism::Unclassified`].
    pub fn has_unclassified(&self) -> bool {
        self.weights
            .iter()
            .any(|e| e.mechanism == FailureMechanism::Unclassified)
    }
}

/// A diagnosed failure, attributed to exactly one capability.
///
/// `implicates` is not optional. 03.10 connects taxonomy nodes to cell selection, scheduler
/// coverage and architecture diagnosis, and all three need to know which capability the failure
/// is evidence about. A failure with no capability is a complaint.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FailureRecord {
    pub failure_id: String,
    pub run: RunId,
    pub implicates: CapabilityId,
    pub ontology_version: String,
    pub chain: CausalChain,
    pub axes: FailureAxes,
    pub labels: LabelDistribution,
}

impl FailureRecord {
    pub fn new(
        failure_id: impl Into<String>,
        run: RunId,
        implicates: CapabilityId,
        ontology_version: impl Into<String>,
        chain: CausalChain,
        axes: FailureAxes,
        labels: LabelDistribution,
    ) -> Self {
        FailureRecord {
            failure_id: failure_id.into(),
            run,
            implicates,
            ontology_version: ontology_version.into(),
            chain,
            axes,
            labels,
        }
    }

    /// Whether this record actually localises the failure, as opposed to recording that one
    /// happened. A record is diagnosed when the taxonomy covered it, the reviewers agreed, the
    /// chain is not a single label repeated four times, and the evidence survived.
    pub fn is_diagnosed(&self) -> bool {
        !self.labels.has_unclassified()
            && self.labels.modal().is_some()
            && !self.chain.is_flattened()
            && self.axes.evidence_status != EvidenceStatus::Lost
    }

    pub fn first_divergence_stage(&self) -> Option<FailureStage> {
        self.chain.first_divergence().stage()
    }
}
