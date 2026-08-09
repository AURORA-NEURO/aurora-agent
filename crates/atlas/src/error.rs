//! Typed atlas errors.
//!
//! Blueprint 03.09 and 03.10 both carry the same mitigation clause: "detect the condition
//! explicitly, fail closed where integrity or safety is affected, preserve the underlying
//! evidence, and emit an actionable diagnostic rather than silently repairing or discarding
//! state." Every variant here names a condition that is cheaper to refuse than to repair.
//!
//! There is deliberately no `Other(String)` variant. An error the atlas cannot name is a gap in
//! this enum, and the gap should be visible as a compile-time exhaustiveness failure rather than
//! absorbed into free text.

use crate::claim::ClaimTier;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Error)]
pub enum AtlasError {
    #[error("empty capability identifier")]
    EmptyCapabilityId,

    #[error("capability identifier contains a control character: {0:?}")]
    ControlCharacterInCapabilityId(String),

    #[error("capability {capability} is not present in ontology {ontology_version}")]
    UnknownCapability {
        capability: String,
        ontology_version: String,
    },

    #[error("capability {capability} is declared twice in the ontology")]
    DuplicateCapability { capability: String },

    #[error("capability {capability} declares an is_a parent {parent} that does not exist")]
    UnknownParent { capability: String, parent: String },

    #[error("capability {capability} declares a {relation} relation to {target}, which does not exist")]
    UnknownRelationTarget {
        capability: String,
        relation: &'static str,
        target: String,
    },

    /// The modelling error 03.09 exists to catch: a capability that reaches itself by `is_a`.
    /// Every ancestor query over such an ontology is meaningless, so the ontology is refused
    /// whole rather than answered per node.
    #[error("capability {capability} is its own ancestor via is_a path {}", .cycle.join(" -> "))]
    CyclicIsA {
        capability: String,
        cycle: Vec<String>,
    },

    #[error("capability {capability} declares a {relation} relation to itself")]
    SelfRelation {
        capability: String,
        relation: &'static str,
    },

    /// `is_a` is the hierarchy. Representing it twice — once as a parent and once as a loose
    /// relation — lets the two drift apart, so the loose form is refused.
    #[error("capability {capability} declares is_a as a loose relation; use the parent list")]
    IsAOutsideHierarchy { capability: String },

    /// 03.09: "Historical results retain their original mapping and may be reprojected with an
    /// explicit transformation." No transformation is implemented, so the mismatch is refused
    /// rather than silently reinterpreted under the current hierarchy.
    #[error("evidence for {subject} was compiled under ontology {found}, atlas is {expected}")]
    OntologyVersionMismatch {
        subject: String,
        expected: String,
        found: String,
    },

    /// The load-bearing refusal. A measurement with no pass and no fail has an empty denominator,
    /// and a ratio over an empty denominator is not zero — it does not exist. Such a capability is
    /// [`crate::CapabilityCell::Unmeasured`].
    #[error("refusing to build a measurement for {capability} with zero evaluable trials")]
    EmptyDenominator { capability: String },

    #[error("measurement for {capability} claims {claimed} {unit} across {trials} trials")]
    ImpossibleClusterCount {
        capability: String,
        unit: &'static str,
        claimed: usize,
        trials: usize,
    },

    /// Two oracles of equal authority disagree about the same trial. 03.10 keeps label
    /// distributions rather than forcing false certainty; at the trial level there is no
    /// distribution to keep, so the conflict is surfaced instead of arbitrated.
    #[error("trial {trial} of {capability} has conflicting {oracle} outcomes")]
    ConflictingEvidence {
        capability: String,
        trial: String,
        oracle: &'static str,
    },

    #[error("failure {failure_id}: causal chain step {label} at {step} precedes {predecessor} at {predecessor_step}")]
    ChainOutOfOrder {
        failure_id: String,
        label: &'static str,
        step: usize,
        predecessor: &'static str,
        predecessor_step: usize,
    },

    #[error("failure {failure_id}: label distribution is empty")]
    EmptyLabelDistribution { failure_id: String },

    #[error("failure {failure_id}: label weights sum to {total}, expected 1.0")]
    MalformedLabelDistribution { failure_id: String, total: f64 },

    #[error("failure {failure_id}: label weight {weight} for {mechanism} is not a positive finite number")]
    MalformedLabelWeight {
        failure_id: String,
        mechanism: &'static str,
        weight: f64,
    },

    /// 43.40: "No lower tier justifies a higher-tier claim."
    #[error("claim about {capability} requested at {requested} but evidence supports only {permitted}")]
    ClaimAboveEvidence {
        capability: String,
        requested: ClaimTier,
        permitted: ClaimTier,
    },

    #[error("a claim about {capability} at the no-claim tier is vacuous and is not licensed")]
    VacuousClaim { capability: String },

    /// 03.09 refuses to pretend capabilities are perfectly separable. Weighting two capabilities
    /// that the ontology marks `confounds_with` counts one signal twice.
    #[error("weighting policy aggregates confounded capabilities {left} and {right} as independent")]
    ConfoundedAggregation { left: String, right: String },

    #[error("weighting policy is malformed: {detail}")]
    MalformedWeightingPolicy { detail: String },

    /// 33.01: the hub refuses to rank when aggregation is not scientifically defensible.
    #[error("composite is ineligible: {reason}")]
    CompositeIneligible { reason: String },
}
