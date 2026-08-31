//! The mutation release program: what a transformed case must declare before it may ship.
//!
//! `bioprism-mutation` applies transformations and checks oracle postconditions. `bioprism-stress`
//! perturbs a cohort and finds the intensity at which a conclusion breaks. Neither answers the
//! question every §32 module ends with, in identical words: *may this descendant enter a released
//! pack?* Each module states eight release gates and a validation program, and the answer is a
//! predicate over the declaration, not over the transformed world. That predicate is this module.
//!
//! # The declaration
//!
//! [`MutationDeclaration`] is §32's `transformation contract` YAML block as a Rust type: a
//! content-addressed parent and descendant, the four [`StatePlanes`] the transformation touches, the
//! [`ExpectedRelation`] it claims, its seed, and its validation oracles.
//!
//! # Disposition, and why it is two-valued
//!
//! [`validate`] returns [`Disposition::Released`] or [`Disposition::Quarantined`], and quarantine
//! carries the unmet gates *and* a [`Missing`] for each. §32: "A transformation that produces a
//! plausible-looking file but violates its declared semantic relation is quarantined as an
//! experimental generator." Quarantine is not rejection — the artifact still exists and is still
//! useful — so the type is not `Result`.
//!
//! # The coherence check that is not in any gate list
//!
//! A declaration that changes no state plane and claims a non-invariant relation is incoherent: if
//! nothing moved, the required conclusion cannot have moved either. §32's release gate "State-plane
//! changes are explicit" is what makes the check possible, and 32.21's failure risk "two mutations
//! cancel" is what makes it necessary. [`coherence`] runs it separately so a caller can ask the
//! question without running the whole gate list.
//!
//! # Not implemented
//!
//! No generation, no transformation, no oracle execution. A [`MutationDeclaration`] is checked
//! against itself and its parent; whether the descendant *actually* satisfies its declared relation is
//! `bioprism-mutation`'s `Relation::check` and `bioprism-stress`'s `StressRelation::check`, and
//! [`ExpectedRelation::as_mutation_relation`] maps onto the former where a correspondence exists.
//! Gates that need a human — blinded expert review of an open semantic relation — are reported as
//! unmet-and-unmeetable-here rather than quietly passed.

use std::collections::BTreeSet;

use bioprism_ids::ContentHash;
use bioprism_mutation::Relation as MutationRelation;
use bioprism_stress::Direction as StressDirection;
use serde::{Deserialize, Serialize};

use crate::error::OracleXError;
use crate::verdict::{Determination, Missing, Witness};

/// The four state planes §32's section index tables, minus the fifth, which is the conclusion.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
pub struct StatePlanes {
    /// Layout, identifier, format, ordering, wording. Semantics fixed.
    pub representation: bool,
    /// Assay noise, batch, missingness, preprocessing, sampling.
    pub observation: bool,
    /// The underlying biological state.
    pub latent_biology: bool,
    /// Tools, permissions, budgets, time, evidence availability.
    pub workflow: bool,
}

impl StatePlanes {
    pub fn none() -> Self {
        StatePlanes::default()
    }

    pub fn any(&self) -> bool {
        self.representation || self.observation || self.latent_biology || self.workflow
    }

    pub fn named(&self) -> BTreeSet<&'static str> {
        let mut set = BTreeSet::new();
        if self.representation {
            set.insert("representation");
        }
        if self.observation {
            set.insert("observation");
        }
        if self.latent_biology {
            set.insert("latent_biology");
        }
        if self.workflow {
            set.insert("workflow");
        }
        set
    }
}

/// The six relations §32's transformation contract enumerates.
///
/// `Ord` is deliberately not derived. `bioprism_stress::Direction` does not implement it, and rather
/// than shadow that type with a local copy — two vocabularies for one concept, which is how a
/// projection drifts — this enum simply is not ordered. Nothing here needs it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "relation", rename_all = "snake_case")]
pub enum ExpectedRelation {
    /// The required conclusion does not move.
    Invariant,
    /// The conclusion moves with the transformation in a declared way.
    Equivariant { under: String },
    /// The conclusion moves in one direction as intensity rises.
    Monotone { direction: StressDirection },
    /// The conclusion stays inside a declared envelope.
    Bounded { envelope: String },
    /// The correct answer becomes a different answer.
    AnswerFlip { from: String, to: String },
    /// The correct behaviour becomes abstention, or stops being abstention.
    AbstentionChange { now_abstains: bool },
}

impl ExpectedRelation {
    pub fn as_str(&self) -> &'static str {
        match self {
            ExpectedRelation::Invariant => "invariant",
            ExpectedRelation::Equivariant { .. } => "equivariant",
            ExpectedRelation::Monotone { .. } => "monotone",
            ExpectedRelation::Bounded { .. } => "bounded",
            ExpectedRelation::AnswerFlip { .. } => "answer_flip",
            ExpectedRelation::AbstentionChange { .. } => "abstention_change",
        }
    }

    /// Whether the required conclusion is claimed to stay put.
    pub fn is_invariant(&self) -> bool {
        matches!(self, ExpectedRelation::Invariant)
    }

    /// The `bioprism-mutation` postcondition this relation corresponds to, where one exists.
    ///
    /// Only [`ExpectedRelation::Invariant`] maps: that crate's `Relation` is about a split-integrity
    /// verdict and its witness set, so `PreservesVerdict` is exactly "invariant" in its currency. The
    /// other five have no correspondent there, and returning `None` for them is the honest answer.
    /// Inventing a mapping would let a caller believe a monotone relation had been checked by a
    /// postcondition that cannot express it.
    pub fn as_mutation_relation(&self) -> Option<MutationRelation> {
        match self {
            ExpectedRelation::Invariant => Some(MutationRelation::PreservesVerdict),
            _ => None,
        }
    }
}

/// Which §32 family a declaration belongs to.
///
/// Every variant is a family this crate implements a checker for somewhere, and
/// [`Family::implemented_by`] names the module. A family with no checker would be a coverage claim
/// with nothing behind it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Family {
    /// 32.05 specimen identity, swap, mixture, relatedness.
    SpecimenIdentity,
    /// 32.10 modality missingness, access, censoring.
    Missingness,
    /// 32.12 label noise, weak supervision, oracle mutations.
    LabelNoise,
    /// 32.13 multimodal contradiction and partial alignment.
    MultimodalContradiction,
    /// 32.14 tool, pipeline, dependency, execution fault.
    ExecutionFault,
    /// 32.15 literature, citation staleness, adversarial evidence.
    AdversarialEvidence,
    /// 32.16 units, scales, normalization, thresholds.
    UnitsAndThresholds,
    /// 32.17 causal intervention and counterfactual.
    CausalIntervention,
    /// 32.18 expert disagreement, adjudication, policy.
    ExpertPolicy,
    /// 32.19 privacy, permission, data locality.
    PrivacyLocality,
    /// 32.20 mechanistic simulation and digital twin.
    DigitalTwin,
    /// 32.21 composition, interactions, minimization.
    Composition,
}

impl Family {
    pub const ALL: [Family; 12] = [
        Family::SpecimenIdentity,
        Family::Missingness,
        Family::LabelNoise,
        Family::MultimodalContradiction,
        Family::ExecutionFault,
        Family::AdversarialEvidence,
        Family::UnitsAndThresholds,
        Family::CausalIntervention,
        Family::ExpertPolicy,
        Family::PrivacyLocality,
        Family::DigitalTwin,
        Family::Composition,
    ];

    pub fn blueprint_module(self) -> &'static str {
        match self {
            Family::SpecimenIdentity => "32.05",
            Family::Missingness => "32.10",
            Family::LabelNoise => "32.12",
            Family::MultimodalContradiction => "32.13",
            Family::ExecutionFault => "32.14",
            Family::AdversarialEvidence => "32.15",
            Family::UnitsAndThresholds => "32.16",
            Family::CausalIntervention => "32.17",
            Family::ExpertPolicy => "32.18",
            Family::PrivacyLocality => "32.19",
            Family::DigitalTwin => "32.20",
            Family::Composition => "32.21",
        }
    }

    /// The module in this crate holding the family's checker.
    pub fn implemented_by(self) -> &'static str {
        match self {
            Family::SpecimenIdentity => "crate::identity",
            Family::Missingness | Family::PrivacyLocality => "crate::missing",
            Family::LabelNoise => "crate::standard",
            Family::MultimodalContradiction => "crate::orthogonal",
            Family::ExecutionFault | Family::DigitalTwin => "crate::execution",
            Family::AdversarialEvidence => "crate::citation",
            Family::UnitsAndThresholds => "crate::units",
            Family::CausalIntervention => "crate::perturbation",
            Family::ExpertPolicy => "crate::panel",
            Family::Composition => "crate::compose",
        }
    }

    /// The validation evidence the family's own "Validation program" section requires, named so a
    /// quarantine can say which one is absent.
    ///
    /// One representative requirement per family rather than the full bullet list: the point is that a
    /// declaration must name *a* validation oracle appropriate to its family, and a crate-side
    /// enumeration of every bullet would drift out of date silently the first time the blueprint
    /// revises one.
    pub fn required_validation(self) -> &'static str {
        match self {
            Family::SpecimenIdentity => "genotype or fingerprint truth",
            Family::Missingness => "typed missing-state propagation",
            Family::LabelNoise => "a known clean subset",
            Family::MultimodalContradiction => "a declared contradiction mechanism",
            Family::ExecutionFault => "fault injection at a known step",
            Family::AdversarialEvidence => "source-passage matching",
            Family::UnitsAndThresholds => "dimensional analysis",
            Family::CausalIntervention => "a known causal graph",
            Family::ExpertPolicy => "reader-level annotations",
            Family::PrivacyLocality => "no-egress validation",
            Family::DigitalTwin => "parameter recovery",
            Family::Composition => "failure preservation",
        }
    }
}

/// How restricted the source material is (§32 release gate: "Controlled data policies are inherited
/// and cannot be weakened").
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccessPolicy {
    Public,
    Federated,
    Controlled,
}

impl AccessPolicy {
    /// Whether `self` is at least as restrictive as `parent`.
    ///
    /// The derived `Ord` follows declaration order, so `Public < Federated < Controlled` means
    /// "less restrictive than". A descendant may tighten and may not loosen.
    pub fn inherits_from(self, parent: AccessPolicy) -> bool {
        self >= parent
    }
}

/// Which seed pool a case was drawn from (§32 release gate: "Training/public and hidden seed ranges
/// are separated").
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SeedPool {
    Public,
    Hidden,
}

/// The §32 transformation contract, as a value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MutationDeclaration {
    pub id: String,
    pub family: Family,
    pub parent: ContentHash,
    pub descendant: ContentHash,
    pub parent_policy: AccessPolicy,
    pub descendant_policy: AccessPolicy,
    pub changes: StatePlanes,
    pub expected_relation: ExpectedRelation,
    pub seed: u64,
    pub seed_pool: SeedPool,
    /// Names of the oracles that validated this descendant.
    pub validation_oracles: BTreeSet<String>,
    /// Whether an automatic proof or property test passed (§32 release gate 3).
    pub property_test_passed: bool,
    /// Whether a blinded expert reviewed an open semantic relation (§32 release gate 4). `None` when
    /// the relation is closed and no review was needed.
    pub blinded_review: Option<bool>,
    /// The semantic signature under which descendants are deduplicated (§32 release gate 6).
    pub semantic_signature: String,
}

impl MutationDeclaration {
    pub fn new(
        id: impl Into<String>,
        family: Family,
        parent: ContentHash,
        descendant: ContentHash,
        expected_relation: ExpectedRelation,
        seed: u64,
    ) -> Self {
        MutationDeclaration {
            id: id.into(),
            family,
            parent,
            descendant,
            parent_policy: AccessPolicy::Public,
            descendant_policy: AccessPolicy::Public,
            changes: StatePlanes::none(),
            expected_relation,
            seed,
            seed_pool: SeedPool::Public,
            validation_oracles: BTreeSet::new(),
            property_test_passed: false,
            blinded_review: None,
            semantic_signature: String::new(),
        }
    }

    pub fn changing(mut self, changes: StatePlanes) -> Self {
        self.changes = changes;
        self
    }

    pub fn under_policy(mut self, parent: AccessPolicy, descendant: AccessPolicy) -> Self {
        self.parent_policy = parent;
        self.descendant_policy = descendant;
        self
    }

    pub fn from_pool(mut self, pool: SeedPool) -> Self {
        self.seed_pool = pool;
        self
    }

    pub fn validated_by(mut self, oracle: impl Into<String>) -> Self {
        self.validation_oracles.insert(oracle.into());
        self
    }

    pub fn with_property_test(mut self, passed: bool) -> Self {
        self.property_test_passed = passed;
        self
    }

    pub fn with_blinded_review(mut self, passed: bool) -> Self {
        self.blinded_review = Some(passed);
        self
    }

    pub fn signed(mut self, signature: impl Into<String>) -> Self {
        self.semantic_signature = signature.into();
        self
    }
}

/// One of §32's eight release gates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Gate {
    ContentAddressed,
    StatePlanesExplicit,
    PropertyTestPassed,
    BlindedReviewOfOpenRelation,
    ReproducibleSeed,
    SemanticDeduplication,
    SeedPoolSeparation,
    PolicyInheritance,
}

impl Gate {
    pub const ALL: [Gate; 8] = [
        Gate::ContentAddressed,
        Gate::StatePlanesExplicit,
        Gate::PropertyTestPassed,
        Gate::BlindedReviewOfOpenRelation,
        Gate::ReproducibleSeed,
        Gate::SemanticDeduplication,
        Gate::SeedPoolSeparation,
        Gate::PolicyInheritance,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Gate::ContentAddressed => "parent and descendant are content-addressed",
            Gate::StatePlanesExplicit => "state-plane changes are explicit",
            Gate::PropertyTestPassed => "at least one automatic proof or property test passes",
            Gate::BlindedReviewOfOpenRelation => {
                "open semantic relations receive blinded expert review"
            }
            Gate::ReproducibleSeed => "generator seeds and versions are reproducible",
            Gate::SemanticDeduplication => {
                "descendants are deduplicated at artifact and semantic levels"
            }
            Gate::SeedPoolSeparation => "training/public and hidden seed ranges are separated",
            Gate::PolicyInheritance => {
                "controlled data policies are inherited and cannot be weakened"
            }
        }
    }
}

/// What the release program decided.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "disposition", rename_all = "snake_case")]
pub enum Disposition {
    Released {
        signature: String,
    },
    /// Not rejected. §32 keeps the artifact as an experimental generator, so the unmet gates travel
    /// with it and a later run can clear them.
    Quarantined {
        unmet: BTreeSet<Gate>,
        missing: Vec<Missing>,
    },
}

impl Disposition {
    pub fn is_released(&self) -> bool {
        matches!(self, Disposition::Released { .. })
    }

    pub fn unmet(&self) -> BTreeSet<Gate> {
        match self {
            Disposition::Released { .. } => BTreeSet::new(),
            Disposition::Quarantined { unmet, .. } => unmet.clone(),
        }
    }
}

/// Whether the declared state-plane changes can produce the declared relation.
///
/// A transformation that moves nothing cannot flip an answer, change an abstention, or be
/// equivariant under anything. The converse is *not* checked: changing a plane is perfectly
/// compatible with an invariant relation, and that combination is the most common valid mutation
/// there is.
pub fn coherence(declaration: &MutationDeclaration) -> Determination {
    if declaration.changes.any() {
        return Determination::supported(
            bioprism_oracle::EvidenceTier::Deterministic,
            format!(
                "{} changes {:?} and claims {}",
                declaration.id,
                declaration.changes.named(),
                declaration.expected_relation.as_str()
            ),
        );
    }
    if declaration.expected_relation.is_invariant() {
        return Determination::supported(
            bioprism_oracle::EvidenceTier::Deterministic,
            format!(
                "{} changes no state plane and claims invariance",
                declaration.id
            ),
        );
    }
    Determination::contradicted(
        bioprism_oracle::EvidenceTier::Deterministic,
        Witness::RelationViolated {
            relation: format!("declared relation of {}", declaration.id),
            expected: "a changed state plane to justify a non-invariant relation".to_string(),
            observed: format!(
                "no plane changed, yet the declaration claims {}",
                declaration.expected_relation.as_str()
            ),
        },
    )
}

/// Runs every gate that can be decided from the declaration.
///
/// `known_signatures` are the semantic signatures already in the pack, for the deduplication gate.
pub fn validate(
    declaration: &MutationDeclaration,
    known_signatures: &BTreeSet<String>,
) -> Result<Disposition, OracleXError> {
    if declaration.parent.as_str().is_empty() || declaration.descendant.as_str().is_empty() {
        return Err(OracleXError::UnrootedDeclaration {
            declaration: declaration.id.clone(),
        });
    }

    let mut unmet: BTreeSet<Gate> = BTreeSet::new();
    let mut missing: Vec<Missing> = Vec::new();

    if declaration.parent == declaration.descendant {
        unmet.insert(Gate::ContentAddressed);
        missing.push(Missing::new(
            "a descendant digest distinct from its parent",
            "the transformation produced a byte-identical artifact, so it transformed nothing",
        ));
    }

    if !coherence(declaration).is_supported() {
        unmet.insert(Gate::StatePlanesExplicit);
        missing.push(Missing::new(
            "a declared state-plane change",
            format!(
                "the relation {} requires something to have moved",
                declaration.expected_relation.as_str()
            ),
        ));
    }

    if !declaration.property_test_passed {
        unmet.insert(Gate::PropertyTestPassed);
        missing.push(Missing::new(
            "an automatic proof or property test",
            "release gate 3 requires at least one to pass",
        ));
    }

    if declaration
        .validation_oracles
        .iter()
        .all(|oracle| oracle != declaration.family.required_validation())
    {
        unmet.insert(Gate::PropertyTestPassed);
        missing.push(Missing::new(
            declaration.family.required_validation(),
            format!(
                "{}'s validation program requires it and the declaration names {:?}",
                declaration.family.blueprint_module(),
                declaration.validation_oracles
            ),
        ));
    }

    let relation_is_open = declaration
        .expected_relation
        .as_mutation_relation()
        .is_none();
    match (relation_is_open, declaration.blinded_review) {
        (true, None) => {
            unmet.insert(Gate::BlindedReviewOfOpenRelation);
            missing.push(Missing::new(
                "a blinded expert review",
                format!(
                    "{} has no executable postcondition in bioprism-mutation, so release gate 4 applies",
                    declaration.expected_relation.as_str()
                ),
            ));
        }
        (true, Some(false)) => {
            unmet.insert(Gate::BlindedReviewOfOpenRelation);
            missing.push(Missing::new(
                "a passing blinded expert review",
                "the review was performed and did not accept the declared relation",
            ));
        }
        _ => {}
    }

    if declaration.semantic_signature.trim().is_empty() {
        unmet.insert(Gate::SemanticDeduplication);
        missing.push(Missing::new(
            "a semantic signature",
            "descendants cannot be deduplicated semantically without one",
        ));
    } else if known_signatures.contains(&declaration.semantic_signature) {
        unmet.insert(Gate::SemanticDeduplication);
        missing.push(Missing::new(
            "a semantically distinct descendant",
            format!(
                "signature '{}' is already in the pack",
                declaration.semantic_signature
            ),
        ));
    }

    if declaration.seed_pool == SeedPool::Public
        && declaration.descendant_policy != AccessPolicy::Public
    {
        unmet.insert(Gate::SeedPoolSeparation);
        missing.push(Missing::new(
            "a hidden-pool seed for controlled material",
            "a public seed makes a controlled descendant reconstructible from the generator",
        ));
    }

    if !declaration
        .descendant_policy
        .inherits_from(declaration.parent_policy)
    {
        unmet.insert(Gate::PolicyInheritance);
        missing.push(Missing::new(
            "a descendant policy at least as restrictive as its parent's",
            format!(
                "parent is {:?} and descendant is {:?}",
                declaration.parent_policy, declaration.descendant_policy
            ),
        ));
    }

    Ok(if unmet.is_empty() {
        Disposition::Released {
            signature: declaration.semantic_signature.clone(),
        }
    } else {
        Disposition::Quarantined { unmet, missing }
    })
}
