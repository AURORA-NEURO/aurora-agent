//! The catalogue of claimed properties.
//!
//! Blueprint 19 (Reference Examples) and 38 (Reference BioWorlds and Vertical Slices) exist so
//! that the platform's claims stop being prose. This module is the list of those claims. Each
//! [`Property`] is one thing the architecture asserts, tagged with the blueprint modules that
//! assert it.
//!
//! The list is deliberately **larger than what this crate can run**. A catalogue that enumerates
//! only the demonstrated properties is a marketing document: it makes the untested surface
//! invisible by omitting it. Every property that cannot be exercised against the current crates
//! carries a [`Property::blocker`] naming the concrete reason — a missing wire field, a missing
//! generator knob, a code path nothing constructs — so the gap is a line item rather than a
//! silence. [`crate::registry::CoverageReport`] reports both halves.
//!
//! # A claim may be narrowed, and narrowing is not closing
//!
//! [`Property::AttestedResultBundleReplay`] was registered as `signed_result_bundle_replay` and
//! claimed that "a signed result bundle verifies independently of the runtime that produced it".
//! `bioprism-bundle` now exists and offers HMAC-SHA256 and nothing asymmetric, so a verifier needs
//! the producing secret and anyone holding it could have written the tag. Two words in the old
//! claim were therefore unearned: the bundle is *attested*, not signed, and verification is
//! independent of the *compiler* but not of the *producer*.
//!
//! The renamed property states only what the workspace can show, and the slice that exercises it
//! records the forgery as an observation rather than a caveat. Demonstrating the old wording
//! against the new crate would have been the worse outcome of the two available: a claim marked
//! green by evidence that does not reach it is harder to find than a claim marked blocked.

use serde::{Deserialize, Serialize};

/// One claim the platform makes about itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Property {
    /// The split-integrity oracle returns hand-checkable witnesses, not a score.
    ExactLeakageWitnesses,
    /// A world with no injected defect compiles to `valid` with no witnesses.
    CleanWorldValidVerdict,
    /// Each leakage mechanism can be injected alone and is detected alone.
    MechanismIsolation,
    /// The mandatory closure is unioned into the selection whatever relevance found.
    ProtectedClosureOverridesRelevance,
    /// Relevance-first selection reaches a confidently wrong verdict on the same world.
    RelevanceOnlySelectionIsUnsound,
    /// A protected tag that no fact carries is reported rather than silently satisfied.
    UnmatchedProtectedTagsReported,
    /// Evidence governed by an unreleased event is withheld at the decision cut.
    TemporalAccessibilityWithholdsEvidence,
    /// When obligations are undischarged the section carries a non-empty refinement frontier.
    RefinementFrontierNonEmpty,
    /// A selection that cannot fit the budget is refused, never trimmed.
    RefusalOverSilentTruncation,
    /// Omissions are grouped by structural reason and assigned an influence class.
    OmissionInfluenceClassified,
    /// A certificate recomputes its own digest and can be verified without the compiler.
    CertificateSelfVerifies,
    /// The same slice produces byte-identical artefacts on every run.
    DeterministicReplay,
    /// On a world built to discriminate, a neighbourhood walk has no sound, compact depth.
    StructuralDiscriminationNoUsableDepth,
    /// On the world the distribution ships, compilation has no advantage over a tuned walk.
    ReferenceWorldDoesNotDiscriminate,
    /// The compiler names the passes it did not run instead of pretending they ran.
    DeferredPassesDeclared,
    /// A reference world can be generated at the cohort size 38.01 specifies.
    CohortScaleContract,

    /// Sheaf gluing and obstruction tests over a declared cover.
    ObstructionAndGluing,
    /// Sound over-approximation of evidence via an abstract-domain registry.
    AbstractInterpretation,
    /// Quotienting the world by decision equivalence under permitted actions.
    DecisionEquivalenceQuotient,
    /// Selecting a context that minimises size subject to a decision-loss bound.
    RateDistortionOptimisation,
    /// An oracle status of `underdetermined`, i.e. representable abstention.
    UnderdeterminedAbstention,
    /// Omissions attributable to policy or consent, not to structure.
    PolicyBlockedOmission,
    /// Omissions whose influence on the decision is non-zero but explicitly bounded.
    BoundedInfluenceOmission,
    /// Choosing among the backend portfolio, and declining to compress when nothing is gained.
    BackendPortfolioSelection,
    /// Temporal withholding of evidence that is *not* also protected.
    NonProtectedTemporalWithholding,
    /// The six mutation families 38.01 names, not the four the generator implements.
    MutationFamilyCoverage,
    /// A result bundle recomputed and authenticated from its own bytes by a key holder.
    ///
    /// Narrowed from `signed_result_bundle_replay`; see the module docs for what was withdrawn.
    AttestedResultBundleReplay,
    /// Forking two architectures from one decision cell and localising the first divergence.
    FirstCausalDivergence,
}

impl Property {
    /// Every claim in the catalogue, exercised or not.
    pub const ALL: [Property; 28] = [
        Property::ExactLeakageWitnesses,
        Property::CleanWorldValidVerdict,
        Property::MechanismIsolation,
        Property::ProtectedClosureOverridesRelevance,
        Property::RelevanceOnlySelectionIsUnsound,
        Property::UnmatchedProtectedTagsReported,
        Property::TemporalAccessibilityWithholdsEvidence,
        Property::RefinementFrontierNonEmpty,
        Property::RefusalOverSilentTruncation,
        Property::OmissionInfluenceClassified,
        Property::CertificateSelfVerifies,
        Property::DeterministicReplay,
        Property::StructuralDiscriminationNoUsableDepth,
        Property::ReferenceWorldDoesNotDiscriminate,
        Property::DeferredPassesDeclared,
        Property::CohortScaleContract,
        Property::ObstructionAndGluing,
        Property::AbstractInterpretation,
        Property::DecisionEquivalenceQuotient,
        Property::RateDistortionOptimisation,
        Property::UnderdeterminedAbstention,
        Property::PolicyBlockedOmission,
        Property::BoundedInfluenceOmission,
        Property::BackendPortfolioSelection,
        Property::NonProtectedTemporalWithholding,
        Property::MutationFamilyCoverage,
        Property::AttestedResultBundleReplay,
        Property::FirstCausalDivergence,
    ];

    /// Stable snake_case identifier, matching the serde representation.
    pub fn id(self) -> &'static str {
        match self {
            Property::ExactLeakageWitnesses => "exact_leakage_witnesses",
            Property::CleanWorldValidVerdict => "clean_world_valid_verdict",
            Property::MechanismIsolation => "mechanism_isolation",
            Property::ProtectedClosureOverridesRelevance => "protected_closure_overrides_relevance",
            Property::RelevanceOnlySelectionIsUnsound => "relevance_only_selection_is_unsound",
            Property::UnmatchedProtectedTagsReported => "unmatched_protected_tags_reported",
            Property::TemporalAccessibilityWithholdsEvidence => {
                "temporal_accessibility_withholds_evidence"
            }
            Property::RefinementFrontierNonEmpty => "refinement_frontier_non_empty",
            Property::RefusalOverSilentTruncation => "refusal_over_silent_truncation",
            Property::OmissionInfluenceClassified => "omission_influence_classified",
            Property::CertificateSelfVerifies => "certificate_self_verifies",
            Property::DeterministicReplay => "deterministic_replay",
            Property::StructuralDiscriminationNoUsableDepth => {
                "structural_discrimination_no_usable_depth"
            }
            Property::ReferenceWorldDoesNotDiscriminate => "reference_world_does_not_discriminate",
            Property::DeferredPassesDeclared => "deferred_passes_declared",
            Property::CohortScaleContract => "cohort_scale_contract",
            Property::ObstructionAndGluing => "obstruction_and_gluing",
            Property::AbstractInterpretation => "abstract_interpretation",
            Property::DecisionEquivalenceQuotient => "decision_equivalence_quotient",
            Property::RateDistortionOptimisation => "rate_distortion_optimisation",
            Property::UnderdeterminedAbstention => "underdetermined_abstention",
            Property::PolicyBlockedOmission => "policy_blocked_omission",
            Property::BoundedInfluenceOmission => "bounded_influence_omission",
            Property::BackendPortfolioSelection => "backend_portfolio_selection",
            Property::NonProtectedTemporalWithholding => "non_protected_temporal_withholding",
            Property::MutationFamilyCoverage => "mutation_family_coverage",
            Property::AttestedResultBundleReplay => "attested_result_bundle_replay",
            Property::FirstCausalDivergence => "first_causal_divergence",
        }
    }

    /// The blueprint modules that assert this property.
    pub fn blueprint_modules(self) -> &'static [&'static str] {
        match self {
            Property::ExactLeakageWitnesses => &["43.41", "38.01"],
            Property::CleanWorldValidVerdict => &["43.41"],
            Property::MechanismIsolation => &["43.41", "38.01"],
            Property::ProtectedClosureOverridesRelevance => &["43.13", "39.05"],
            Property::RelevanceOnlySelectionIsUnsound => &["43.13", "43.41"],
            Property::UnmatchedProtectedTagsReported => &["43.13"],
            Property::TemporalAccessibilityWithholdsEvidence => &["43.09", "38.08"],
            Property::RefinementFrontierNonEmpty => &["43.25", "43.28"],
            Property::RefusalOverSilentTruncation => &["43.13", "43.25"],
            Property::OmissionInfluenceClassified => &["43.26"],
            Property::CertificateSelfVerifies => &["43.26", "40.05"],
            Property::DeterministicReplay => &["40.05", "43.26", "19.09"],
            Property::StructuralDiscriminationNoUsableDepth => &["43.39", "43.38", "43.01"],
            Property::ReferenceWorldDoesNotDiscriminate => &["43.38", "43.39", "43.41"],
            Property::DeferredPassesDeclared => &["43.16", "43.37"],
            Property::CohortScaleContract => &["38.01", "43.41"],
            Property::ObstructionAndGluing => &["43.06"],
            Property::AbstractInterpretation => &["43.11"],
            Property::DecisionEquivalenceQuotient => &["43.10"],
            Property::RateDistortionOptimisation => &["43.12"],
            Property::UnderdeterminedAbstention => &["43.28", "43.41"],
            Property::PolicyBlockedOmission => &["43.26", "39.05"],
            Property::BoundedInfluenceOmission => &["43.26"],
            Property::BackendPortfolioSelection => &["43.36", "43.37"],
            Property::NonProtectedTemporalWithholding => &["43.09"],
            Property::MutationFamilyCoverage => &["38.01", "19.05"],
            Property::AttestedResultBundleReplay => &["38.01", "19.06", "34.14"],
            Property::FirstCausalDivergence => &["38.01", "19.16"],
        }
    }

    /// The claim, in one sentence, as a reader would have to evaluate it.
    pub fn claim(self) -> &'static str {
        match self {
            Property::ExactLeakageWitnesses => {
                "the oracle returns a set of concrete leakage witnesses a human can check by hand, not a similarity score"
            }
            Property::CleanWorldValidVerdict => {
                "a world with no injected defect returns valid with no witnesses, so the oracle is not a constant invalid"
            }
            Property::MechanismIsolation => {
                "each leakage mechanism can be injected on its own and produces exactly its own witness kind"
            }
            Property::ProtectedClosureOverridesRelevance => {
                "protected evidence enters the selection whether or not any dependency path reaches it"
            }
            Property::RelevanceOnlySelectionIsUnsound => {
                "a relevance-first selection over the same world reaches a confidently wrong verdict"
            }
            Property::UnmatchedProtectedTagsReported => {
                "protecting a tag no fact carries is reported, rather than reported as a satisfied empty closure"
            }
            Property::TemporalAccessibilityWithholdsEvidence => {
                "evidence governed by an event not yet released is unreadable at the decision cut"
            }
            Property::RefinementFrontierNonEmpty => {
                "when an obligation is undischarged the section names a move that would discharge it"
            }
            Property::RefusalOverSilentTruncation => {
                "a selection that will not fit the budget is refused with a typed error, never trimmed to fit"
            }
            Property::OmissionInfluenceClassified => {
                "omitted evidence is grouped by structural reason and each group carries an influence class"
            }
            Property::CertificateSelfVerifies => {
                "a certificate recomputes its own digest, so a consumer can verify it without the compiler"
            }
            Property::DeterministicReplay => {
                "running the same slice twice produces byte-identical artefacts and the same digest"
            }
            Property::StructuralDiscriminationNoUsableDepth => {
                "on a world built to vary structure, no neighbourhood-walk depth is both sound and compact"
            }
            Property::ReferenceWorldDoesNotDiscriminate => {
                "on the world the distribution ships, a correctly tuned neighbourhood walk selects exactly the compiled set, so the benchmark measures nothing about the method"
            }
            Property::DeferredPassesDeclared => {
                "the compiler names every pass it could not run and why, rather than omitting it"
            }
            Property::CohortScaleContract => {
                "the reference world generates at the 80-200 subject scale 38.01 specifies"
            }
            Property::ObstructionAndGluing => {
                "local sections that disagree are detected as an obstruction over a declared cover"
            }
            Property::AbstractInterpretation => {
                "evidence is soundly over-approximated in a registered abstract domain"
            }
            Property::DecisionEquivalenceQuotient => {
                "worlds indistinguishable under the permitted actions collapse to one representative"
            }
            Property::RateDistortionOptimisation => {
                "the smallest context is chosen subject to a stated bound on decision loss"
            }
            Property::UnderdeterminedAbstention => {
                "the oracle can return underdetermined, so abstention is representable rather than forced"
            }
            Property::PolicyBlockedOmission => {
                "evidence withheld by policy or consent is classified as such, not as structurally irrelevant"
            }
            Property::BoundedInfluenceOmission => {
                "an omission with non-zero influence carries an explicit numeric bound on the distortion it can cause"
            }
            Property::BackendPortfolioSelection => {
                "a backend is chosen from a portfolio, and declining to compress is a first-class reported outcome"
            }
            Property::NonProtectedTemporalWithholding => {
                "temporal withholding is a separate failure from a protected-closure violation"
            }
            Property::MutationFamilyCoverage => {
                "all six mutation families 38.01 names are generable and semantically validated"
            }
            Property::AttestedResultBundleReplay => {
                "a result bundle carrying a compiled certificate is recomputed from its own bytes and its manifest authenticated, without the compiler that produced it and by a party holding the producing secret — who could equally have written the tag"
            }
            Property::FirstCausalDivergence => {
                "two architectures fork from one decision cell and the first divergence is localised"
            }
        }
    }

    /// Why no slice in this crate exercises the property, when none does.
    ///
    /// `None` means a slice exists; it does not mean the property holds — that is the
    /// [`crate::SliceReport`]'s job. A `Some` value names a concrete, checkable obstacle, so a
    /// later contributor knows what to change rather than merely that something is missing.
    pub fn blocker(self) -> Option<&'static str> {
        match self {
            Property::ObstructionAndGluing => Some(
                "fiber-world/0.1 carries no cover, so no two local sections are declared to overlap; bioprism-fiber lists obstruction_tests as a deferred pass on every compile",
            ),
            Property::AbstractInterpretation => Some(
                "fiber-world/0.1 carries no abstract-domain registry; bioprism-fiber lists abstract_interpretation as a deferred pass on every compile",
            ),
            Property::DecisionEquivalenceQuotient => Some(
                "fiber-query/0.1 carries neither permitted_actions nor decision_loss, and Query::missing_contract_fields reports both as absent",
            ),
            Property::RateDistortionOptimisation => Some(
                "the same missing decision_loss field: there is no loss to trade distortion against",
            ),
            Property::UnderdeterminedAbstention => Some(
                "OracleVerdict::abstain exists in bioprism-section but no path in bioprism-fiber constructs it — construction now lives behind injection: bioprism_fiber::compile_with_oracle judges a compile by any DecisionOracle, and bioprism-domain's rule oracles return abstaining verdicts through it (crates/fiber/tests/oracle_injection.rs, crates/domain/tests/end_to_end.rs). The default compile's split-integrity oracle still derives status solely from whether the witness list is empty, so the underdetermined bioworld still compiles to valid on the default path, and no slice in this crate exercises abstention yet",
            ),
            Property::BoundedInfluenceOmission => Some(
                "bioprism-fiber emits only InfluenceClass::Zero and DeferredAcquisition; nothing computes a numeric influence bound, so no group is ever Bounded",
            ),
            Property::BackendPortfolioSelection => Some(
                "bioprism-fiber hard-codes Backend::BackwardFactorSliceReference and leaves PlanDescriptor::fallback None on every compile; the other six backends have no implementation to select",
            ),
            Property::MutationFamilyCoverage => Some(
                "WorldSpec::LeakageMechanism has four members; 38.01 names six mutation families, and prevalence shift, segmentation perturbation and assay uncertainty have no generator knob",
            ),
            Property::FirstCausalDivergence => Some(
                "the two halves of this claim sit in crates that do not meet: bioprism-prism forks architectures from one decision cell but reports per-arm acceptance and a single attribution sentence, with no ordered trajectory along which a *first* divergence could be located, while bioprism-benchcompiler localises a first causal divergence only over a pair of bioprism-trace trajectories whose steps are agent decisions; a slice's compile emits pass receipts, which are compiler stages and not decisions, so taking either crate as a dependency here would add machinery with no trajectory to run it on",
            ),
            _ => None,
        }
    }
}

/// A [`Property`] flattened for reporting, with its coverage status attached.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PropertyClaim {
    pub property: Property,
    pub id: String,
    pub claim: String,
    pub blueprint_modules: Vec<String>,
    /// Slices that exercise this property, in registration order. Empty means unexercised.
    pub exercised_by: Vec<String>,
    /// Why it is unexercised, when a concrete obstacle is known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blocker: Option<String>,
}

impl PropertyClaim {
    pub fn new(property: Property, exercised_by: Vec<String>) -> Self {
        PropertyClaim {
            property,
            id: property.id().to_string(),
            claim: property.claim().to_string(),
            blueprint_modules: property
                .blueprint_modules()
                .iter()
                .map(|m| (*m).to_string())
                .collect(),
            blocker: property.blocker().map(str::to_string),
            exercised_by,
        }
    }

    pub fn is_exercised(&self) -> bool {
        !self.exercised_by.is_empty()
    }
}
