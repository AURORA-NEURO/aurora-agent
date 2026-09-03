//! OncoWorld domain depth: the parts of blueprint section 30 that `bioprism-onco` left out.
//!
//! `bioprism-onco` owns section 30's core — disease taxonomy (30.01), the longitudinal worldline
//! (30.02), response criteria (30.06, 30.07), integrated molecular classification (30.10), outcomes
//! and estimands (30.26) and the research-only clinical boundary (30.30). This crate owns the
//! remaining twenty-two modules, weighted towards the ones whose content is checkable rather than
//! descriptive:
//!
//! | Module | Here |
//! | --- | --- |
//! | 30.03 patient, specimen, imaging and assay alignment | [`identity`] |
//! | 30.08 radiogenomics and cross-modal prediction | [`radiogenomics`] |
//! | 30.11 methylation classification and epigenetic evidence | [`methylation`] |
//! | 30.12 clonal evolution, heterogeneity and resistance | [`clonal`] |
//! | 30.19 patient-derived models, organoids and PDX | [`models`] |
//! | 30.27 site, population, era shift and global equity | [`era`] |
//! | 30.20–30.24 the entity worlds | [`entities`] |
//! | cross-cutting transport machinery for 30.08 and 30.19 | [`transport`] |
//!
//! # The modules of section 30 that are still uncovered
//!
//! Eleven of the twenty-two remain unimplemented, and naming them is cheaper than letting a reader
//! infer coverage from a crate name: 30.04 (MRI acquisition and harmonisation), 30.05
//! (segmentation and volumetry), 30.09 (digital neuropathology), 30.13 (single-cell and spatial),
//! 30.14 (liquid biopsy and CSF), 30.15 (surgery and intraoperative), 30.16 (radiotherapy plan and
//! dose), 30.17 (pharmacology and BBB penetration), 30.18 (target discovery and dependency), 30.25
//! (trial eligibility and evidence synthesis), 30.28 (the multi-agent research tumour board) and
//! 30.29 (the benchmark portfolio). Those are the modules whose content is mostly *pipeline* — an
//! acquisition, a segmentation, a dose grid, a screen — and a type-level treatment of them without
//! the pipeline would be scaffolding. The six this crate leads with were chosen because their
//! content is a checkable distinction rather than a description of a process.
//!
//! 30.27 also pairs with `crates/stress`, which owns the perturbation side of shift. This crate
//! owns the standing structural fact: that some cohorts were never one cohort.
//!
//! # This crate is not a clinical system
//!
//! `bioprism_onco::boundary` fixes 30.30's constraint as a type, and nothing here routes around
//! it. This crate does not diagnose a person, does not recommend a treatment, does not triage
//! anyone, and claims no medical-device functionality. It contains no cohort data, no trained
//! classifier, no imaging pipeline and no estimator, and it never produces
//! `bioprism_onco::ResearchOutput` — that type is minted only by the boundary's own `release`.
//! What it produces are refusals and constrained claims.
//!
//! # The two domain facts this crate exists to encode
//!
//! **A tumour is not one thing.** 30.12's subclones mean that a measurement on a specimen is a
//! measurement on a sample of a heterogeneous population. So "absent in this specimen" and "absent
//! in this tumour" are different types here, and only one of them exists:
//! [`clonal::TumourClaim`] has a variant for presence and a variant for a bound over the sampled
//! regions, and no variant for absence at all. A marker undetected in a fragment constrains the
//! tumour exactly as far as the fragment's sampling and the assay's declared sensitivity support,
//! and [`clonal::SpecimenObservation::as_tumour_claim`] is the only route between the two scopes.
//! This is `bioprism-lens`'s missingness discipline in the currency the domain uses.
//!
//! **A model system is evidence about a model system.** 30.19's organoid or PDX result transports
//! to a patient only under stated assumptions, so [`models::ModelResult`] is freely constructible
//! and [`models::PatientRelevantClaim`] has no public constructor: it is produced only by
//! [`models::transport_to_patients`], which requires a `bioprism_scope::LossLedger` that is not
//! empty. An untransported cross-system claim does not typecheck as a claim about the disease. The
//! same device gives [`radiogenomics::TumourLabel`] and [`radiogenomics::SupportedClaim`] their
//! meaning.
//!
//! # What this crate reuses rather than reinvents
//!
//! * `bioprism_scope` — [`identity::Artifact::scope_key`] produces a real `ScopeKey`, and
//!   [`identity::onco_dimension_registry`] *extends* the canonical `DimensionRegistry` with the
//!   lineage levels 30.03 names instead of building a parallel vocabulary.
//!   [`transport::DeclaredTransport`] renders as a real `ScopeMapping` and inherits its
//!   undeclared-loss check.
//! * `bioprism_onco` — [`clonal`] uses `ObservationStatus` and `Observed` unchanged, so "the assay
//!   never ran" and "the assay ran and saw nothing" stay distinct, and `MolecularMarker` supplies
//!   the marker vocabulary rather than a second invented one.
//! * `bioprism_standards` — [`identity::joinable`] follows its first-blocking discipline: a caller
//!   is told the first dimension that blocks, in a stated order, because six simultaneous
//!   complaints are not actionable and the later ones are meaningless until the earlier is fixed.
//!   [`identity::align_regional_position`] then hands the coordinate question straight to
//!   `comparable`, which refuses unstated frames outright and — because
//!   `ReferenceSpace::SubjectNative` carries the subject identifier — makes two participants'
//!   native spaces incomparable however similar their numbers look.
//!
//! # Where the blueprint supplies no constant
//!
//! Section 30 is an architecture specification. It fixes the *shape* of these objects and
//! enumerates almost none of their contents, so wherever a number or a name was needed and the
//! blueprint gave none, this crate makes the caller supply it and says so at the point of use:
//!
//! * **no detection limit** — [`clonal::DetectionSensitivity`] is declared by the caller, and a
//!   negative result without one bounds nothing;
//! * **no classifier reporting threshold** —
//!   [`methylation::ClassifierVersion::reporting_threshold`] is an `Option` with no default, and
//!   [`methylation::classify`] refuses without it;
//! * **no methylation class names** — [`methylation::MethylationClass`] is an opaque string;
//! * **no WHO edition or entity list** — [`era::ClassificationVersion`] and [`era::EntityLabel`]
//!   are opaque strings;
//! * **no minimum subgroup size, no minimum establishment rate, no minimum class count** — the
//!   corresponding checks require an uncertainty interval, a modelled selection, or a non-zero
//!   count, all of which are arithmetic rather than legislation;
//! * **no allele-fraction-to-cellular-fraction arithmetic** — [`clonal::FractionEvidence`] records
//!   whether the caller declared purity, local copy number and multiplicity; it does not convert.
//!
//! The one place a marker vocabulary appears at all is `bioprism_onco::MolecularMarker`, which
//! that crate documents as a worked instantiation. Nothing here adds to it.
//!
//! # Boilerplate in section 30
//!
//! Measured across the thirty modules of `30_NEURO_ONCOLOGY_ONCOWORLD`, **60.1%** of non-blank
//! lines are byte-identical in *every* module — the "Why this is a BioWorld", "BioDecision Cell
//! families", "Five-layer review", "Implementation contract" and "Release gates" sections in full,
//! plus the closing paragraph of "Evaluation task ladder", "Oracle mesh", "Primary metrics",
//! "Mutation and stress program" and "Worked microbenchmark". The figure is identical whether the
//! threshold is "shared by all thirty" or "shared by at least twenty-seven", because there is no
//! partial sharing: a line is either in the template or unique to its module. The differentiated
//! content is the purpose sentence, the required-state list, six ladder items, an oracle list, a
//! metric list, a mutation list, five characteristic failures and one microbenchmark paragraph —
//! and it is the characteristic-failure lists that carry nearly all of this crate's testable
//! content.
//!
//! # Example
//!
//! ```
//! use bioprism_oncoworlds::clonal::{
//!     CellularFraction, DetectionSensitivity, RegionId, SpecimenObservation, SpecimenSampling,
//!     TumourClaim,
//! };
//! use bioprism_onco::{MarkerCall, MolecularMarker, Observed};
//!
//! let sampling = SpecimenSampling::new("S1")
//!     .sampling(RegionId::new("enhancing core"))
//!     .detecting_down_to(DetectionSensitivity {
//!         smallest_detectable_fraction: CellularFraction::from_parts_per_ten_thousand(500)?,
//!         declared_by: "the assay's validation report".to_string(),
//!     });
//!
//! let observation = SpecimenObservation::new(
//!     MolecularMarker::EgfrAmplification,
//!     sampling,
//!     Observed::Value(MarkerCall::Absent),
//! );
//!
//! let claim = observation.as_tumour_claim()?;
//! // The strongest available statement is a bound over the region that was sampled.
//! assert!(matches!(claim, TumourClaim::UndetectedAboveFraction { .. }));
//! // And it says nothing at all about the region that was not.
//! assert!(!claim.excludes_subclone(CellularFraction::WHOLE, &RegionId::new("infiltrating edge")));
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

pub mod clonal;
pub mod entities;
pub mod era;
pub mod federated_statistical_analysis_workbench;
pub mod federated_resource_discovery_assurance;
pub mod error;
pub mod identity;
pub mod methylation;
pub mod models;
pub mod prospective_evidence_surveillance_copilot;
pub mod prospective_replication_negative_results_assurance;
pub mod radiogenomics;
pub mod transport;

pub use clonal::{
    attribute_to_treatment, compatible_histories, explain_new_alteration, CausalDesign,
    CellularFraction, ClonalHistory, CompatibleHistories, DetectionSensitivity, ExplanationSet,
    FractionDerivation, FractionEvidence, RegionId, ResistanceExplanation, SpecimenObservation,
    SpecimenSampling, Subclone, SubcloneId, TumourClaim, TumourPopulation,
};
pub use entities::{
    declare_cluster, feasibility, handle_event, pool_alterations, pool_provenance,
    AlterationMechanism, BenchmarkFeasibility, EventHandling, FollowUpEvent, LesionEndpoint,
    LesionSet, PublishedPerformance, RarePerformanceReport, TissueProvenance,
};
pub use era::{
    as_negative_call, comparable_cohorts, equity_report, subgroup_claim, use_descriptor,
    AssayAvailability, ClassificationVersion, Cohort, DescriptorUse, EntityLabel, EntityMapping,
    EquityReport, LabelFate, PooledScore, PopulationDescriptor, SiteAssayContext, SubgroupClaim,
    SubgroupResult, UncertaintyInterval,
};
pub use federated_statistical_analysis_workbench::{
    qualify_oncoworlds_analysis_workbench, oncoworlds_analysis_workbench_manifest,
    OncoworldsAnalysisCandidate8, OncoworldsAnalysisWorkbenchError,
    OncoworldsAnalysisWorkbenchReceipt, OncoworldsAnalysisWorkbenchRequest,
    CONTRACT_VERSION as ONCOWORLDS_ANALYSIS_WORKBENCH_CONTRACT_VERSION,
    CONTENT_TYPE as ONCOWORLDS_ANALYSIS_WORKBENCH_CONTENT_TYPE,
    FEATURE_ID as ONCOWORLDS_ANALYSIS_WORKBENCH_FEATURE_ID,
    INPUT_SCHEMA as ONCOWORLDS_ANALYSIS_WORKBENCH_INPUT_SCHEMA,
    OUTPUT_SCHEMA as ONCOWORLDS_ANALYSIS_WORKBENCH_OUTPUT_SCHEMA,
};
pub use federated_resource_discovery_assurance::{
    assure_oncoworlds_resources, oncoworlds_resource_discovery_manifest,
    OncoworldsPeerResourceSummary4, OncoworldsQualifiedResource7,
    OncoworldsQualifiedResourceSet7, OncoworldsResourceArtifact7,
    OncoworldsResourceDiscoveryError, OncoworldsResourceDisposition,
    OncoworldsResourceEndpoint4, OncoworldsResourceManifest, OncoworldsResourceNeed4,
    EndpointStatus as OncoworldsEndpointStatus, EvidenceState as OncoworldsResourceEvidenceState,
    CONTRACT_VERSION as ONCOWORLDS_RESOURCE_DISCOVERY_CONTRACT_VERSION,
    CONTENT_TYPE as ONCOWORLDS_RESOURCE_DISCOVERY_CONTENT_TYPE,
    FEATURE_ID as ONCOWORLDS_RESOURCE_DISCOVERY_FEATURE_ID,
    INPUT_SCHEMA as ONCOWORLDS_RESOURCE_DISCOVERY_INPUT_SCHEMA,
    OUTPUT_SCHEMA as ONCOWORLDS_RESOURCE_DISCOVERY_OUTPUT_SCHEMA,
};
pub use error::{
    EntityWorldRefusal, FractionError, JoinRefusal, MethylationRefusal, OncoWorldsError,
    PhylogenyRefusal, PromotionRefusal, ShiftRefusal, TransportRefusal,
};
pub use identity::{
    align_regional_position, align_to_image_region, count_units, joinable, joinable_with_bridge,
    onco_dimension_registry, report_join, AnalysisUnit, Artifact, ArtifactLevel, DiseaseEpoch,
    EpochBridge, IdentityEvidence, IdentityLink, IdentityRelation, JoinReport, JoinVerdict,
    LinkBasis, LinkConfidence, PermissibleUse, Pseudonym, RegionProvenance, UnitCount,
    COORDINATE_CHECK_ORDER, JOIN_CHECK_ORDER,
};
pub use methylation::{
    classify, compare_raw_across_versions, corroborate, reconcile_versions, CalibratedScore,
    Calibration, ClassificationReport, ClassifierVersion, CopyNumberProvenance, Corroboration,
    EvaluationCohort, EvidenceUse, MethylationClass, MethylationOutcome, NearestClass, QcOutcome,
    RawScore, RoleLedger, SampleContext, ScoreValue, UnclassifiableReason, VersionComparison,
    VersionDivergence, VersionedResult,
};
pub use prospective_evidence_surveillance_copilot::{
    oncoworlds_evidence_surveillance_copilot_manifest,
    run_oncoworlds_evidence_surveillance_copilot, OncoworldsEvidenceCopilotDisposition,
    OncoworldsEvidenceObservation, OncoworldsEvidenceSurveillanceCopilotError,
    OncoworldsEvidenceSurveillanceCopilotReceipt, OncoworldsEvidenceSurveillanceCopilotRequest,
    OncoworldsQualifiedEvidenceSet,
    CONTRACT_VERSION as ONCOWORLDS_EVIDENCE_SURVEILLANCE_COPILOT_CONTRACT_VERSION,
    FEATURE_ID as ONCOWORLDS_EVIDENCE_SURVEILLANCE_COPILOT_FEATURE_ID,
    INPUT_SCHEMA as ONCOWORLDS_EVIDENCE_SURVEILLANCE_COPILOT_INPUT_SCHEMA,
    OUTPUT_SCHEMA as ONCOWORLDS_EVIDENCE_SURVEILLANCE_COPILOT_OUTPUT_SCHEMA,
};
pub use prospective_replication_negative_results_assurance::{
    assure_oncoworlds_replication, oncoworlds_replication_negative_results_manifest,
    OncoworldsClaimAndProtocol, OncoworldsReplicationAssuranceError,
    OncoworldsReplicationClaim, OncoworldsReplicationDisposition,
    OncoworldsReplicationOutcome, OncoworldsReplicationRecord,
    CONTRACT_VERSION as ONCOWORLDS_REPLICATION_ASSURANCE_CONTRACT_VERSION,
    FEATURE_ID as ONCOWORLDS_REPLICATION_ASSURANCE_FEATURE_ID,
    INPUT_SCHEMA as ONCOWORLDS_REPLICATION_ASSURANCE_INPUT_SCHEMA,
    OUTPUT_SCHEMA as ONCOWORLDS_REPLICATION_ASSURANCE_OUTPUT_SCHEMA,
};
pub use models::{
    transport_to_patients, EstablishmentCohort, FidelityAxis, FidelityEvidence, ModelIdentity,
    ModelResult, ModelSystem, PatientRelevantClaim, ReplicateStructure,
};
pub use radiogenomics::{
    assert_claim, tumour_label, ClaimTarget, CohortSelection, EvaluationDesign, FeatureProvenance,
    RadiogenomicClaim, SplitUnit, SupportedClaim, TumourLabel, MECHANISM_STRATA,
};
pub use transport::DeclaredTransport;
