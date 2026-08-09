//! The worked microbenchmarks section 30 states, as executable invariants.
//!
//! Each module of `30_NEURO_ONCOLOGY_ONCOWORLD` ends with a paragraph describing one concrete
//! situation and what a system must do with it. Those paragraphs are the least boilerplate-heavy
//! text in the section — everything around them is shared across all thirty modules — so they are
//! the best available statement of what each module actually wants. The tests below encode the six
//! that fall inside this crate's scope, plus the two cross-module invariants the crate exists for.
//!
//! These are not benchmark harnesses. There is no cohort, no model and no oracle here; each test
//! checks that the *shape* the blueprint describes is the shape the types produce.

use bioprism_onco::{MarkerCall, MolecularMarker, ObservationStatus, Observed};
use bioprism_oncoworlds::clonal::{
    explain_new_alteration, CellularFraction, DetectionSensitivity, FractionDerivation,
    FractionEvidence, RegionId, ResistanceExplanation, SpecimenObservation, SpecimenSampling,
};
use bioprism_oncoworlds::entities::{declare_cluster, handle_event, EventHandling, FollowUpEvent, LesionEndpoint, LesionSet};
use bioprism_oncoworlds::era::{
    as_negative_call, comparable_cohorts, AssayAvailability, ClassificationVersion, Cohort,
    EntityLabel, SiteAssayContext,
};
use bioprism_oncoworlds::identity::{
    count_units, joinable, joinable_with_bridge, AnalysisUnit, Artifact, ArtifactLevel,
    DiseaseEpoch, EpochBridge, IdentityEvidence, Pseudonym,
};
use bioprism_oncoworlds::methylation::{
    reconcile_versions, Calibration, CalibratedScore, ClassifierVersion, MethylationClass,
    MethylationOutcome, RawScore, ScoreValue, UnclassifiableReason, VersionDivergence,
    VersionedResult,
};
use bioprism_oncoworlds::models::{
    transport_to_patients, EstablishmentCohort, FidelityAxis, FidelityEvidence, ModelIdentity,
    ModelResult, ModelSystem, ReplicateStructure, REQUIRED_ASSUMPTIONS as MODEL_ASSUMPTIONS,
};
use bioprism_oncoworlds::radiogenomics::{
    assert_claim, tumour_label, ClaimTarget, EvaluationDesign, RadiogenomicClaim, SplitUnit,
    REQUIRED_ASSUMPTIONS as RADIOGENOMIC_ASSUMPTIONS,
};
use bioprism_oncoworlds::transport::DeclaredTransport;
use bioprism_oncoworlds::{EntityWorldRefusal, ShiftRefusal, TransportRefusal};
use bioprism_scope::ScopeKey;

fn fraction(parts: u16) -> CellularFraction {
    CellularFraction::from_parts_per_ten_thousand(parts).expect("within the whole")
}

fn score(parts: u16) -> ScoreValue {
    ScoreValue::from_parts_per_ten_thousand(parts).expect("within the unit interval")
}

fn calibrated(parts: u16) -> CalibratedScore {
    RawScore(score(parts)).calibrate(&Calibration::new(
        "as documented by the classifier's validation report",
        "cal-1",
    ))
}

fn library(label: &str, patient: &str, specimen: &str, epoch: DiseaseEpoch) -> Artifact {
    Artifact::new(label, Pseudonym::new(patient), epoch)
        .at(ArtifactLevel::Encounter, Pseudonym::new(specimen))
        .at(ArtifactLevel::Specimen, Pseudonym::new(specimen))
        .at(ArtifactLevel::Aliquot, Pseudonym::new(label))
        .at(ArtifactLevel::Library, Pseudonym::new(label))
}

fn series(label: &str, patient: &str, epoch: DiseaseEpoch) -> Artifact {
    Artifact::new(label, Pseudonym::new(patient), epoch)
        .at(ArtifactLevel::Encounter, Pseudonym::new("E-pre"))
        .at(ArtifactLevel::ImagingStudy, Pseudonym::new("STU-1"))
        .at(ArtifactLevel::ImagingSeries, Pseudonym::new(label))
}

/// 30.03: "Two RNA-seq libraries share a patient but come from separate surgeries before and after
/// radiation. A radiogenomic task must determine whether they may be paired with the preoperative
/// MRI, used as repeated measures, or excluded."
///
/// All three answers are available and they are different objects: the preoperative library pairs
/// with the preoperative MRI, the post-radiation library does not without a stated bridge, and the
/// pair counts as one participant rather than two.
#[test]
fn two_libraries_across_a_radiation_boundary_are_not_two_independent_cases() {
    let preoperative = library("LIB-pre", "PT-1", "S-pre", DiseaseEpoch::Preoperative);
    let post_radiation = library("LIB-post", "PT-1", "S-post", DiseaseEpoch::OnTreatment);
    let mri = series("SER-pre", "PT-1", DiseaseEpoch::Preoperative);
    let evidence = IdentityEvidence::new();

    assert!(joinable(&preoperative, &mri, AnalysisUnit::Participant, &evidence).is_ok());
    assert!(joinable(&post_radiation, &mri, AnalysisUnit::Participant, &evidence).is_err());

    let bridge = EpochBridge {
        from: DiseaseEpoch::OnTreatment,
        to: DiseaseEpoch::Preoperative,
        warrant: "the analysis treats the post-radiation library as a repeated measure and models \
                  the treatment interval explicitly"
            .to_string(),
    };
    assert!(joinable_with_bridge(
        &post_radiation,
        &mri,
        AnalysisUnit::Participant,
        &evidence,
        Some(&bridge)
    )
    .is_ok());

    let count = count_units(
        &[preoperative, post_radiation],
        AnalysisUnit::Participant,
    );
    assert_eq!(count.effective, 1);
    assert!(count.pseudoreplicated());
}

/// 30.12: "A resistance-associated alteration is absent at diagnosis under low depth and present at
/// recurrence. The agent must compare de novo emergence, prior undetected subclone, and sampling
/// explanations."
///
/// All three survive, so no single explanation is available.
#[test]
fn an_alteration_absent_at_diagnosis_under_low_depth_leaves_three_explanations_open() {
    let diagnosis = SpecimenObservation::new(
        MolecularMarker::EgfrAmplification,
        SpecimenSampling::new("S-diagnosis")
            .sampling(RegionId::new("enhancing core"))
            .detecting_down_to(DetectionSensitivity {
                smallest_detectable_fraction: fraction(2_500),
                declared_by: "shallow sequencing, per the run's validation record".to_string(),
            }),
        Observed::Value(MarkerCall::Absent),
    );
    let recurrence = SpecimenObservation::new(
        MolecularMarker::EgfrAmplification,
        SpecimenSampling::new("S-recurrence")
            .sampling(RegionId::new("enhancing core"))
            .sampling(RegionId::new("infiltrating edge"))
            .detecting_down_to(DetectionSensitivity {
                smallest_detectable_fraction: fraction(200),
                declared_by: "deep sequencing, per the run's validation record".to_string(),
            }),
        Observed::Value(MarkerCall::Present),
    )
    .at_fraction(FractionEvidence::Cellular {
        fraction: fraction(4_000),
        derivation: FractionDerivation {
            purity: fraction(8_000),
            local_copy_number: 4,
            multiplicity: 2,
            derived_by: "the caller's variant caller".to_string(),
        },
    });

    let explanations = explain_new_alteration(&diagnosis, &recurrence);
    for expected in [
        ResistanceExplanation::DeNovoEmergence,
        ResistanceExplanation::PreexistingSubcloneSelected,
        ResistanceExplanation::UnsampledRegionAtDiagnosis,
        ResistanceExplanation::BelowDetectionAtDiagnosis,
    ] {
        assert!(explanations.contains(expected), "{expected:?} was excluded");
    }
    assert!(explanations.sole().is_err());
}

/// 30.11: "A sample moves from one low-confidence class to another after a reference update while
/// its copy-number profile is stable. The system must report version-conditioned evidence rather
/// than call one historical result wrong."
#[test]
fn a_reference_update_that_moves_a_low_confidence_class_reports_both_versions() {
    let under = |version: &str, label: &str| VersionedResult {
        classifier: ClassifierVersion::new("illustrative classifier", version, "ref-1")
            .reporting_at(score(5_000)),
        outcome: MethylationOutcome::Classified {
            class: MethylationClass::new(label),
            score: calibrated(5_100),
        },
    };
    let comparison = reconcile_versions(&under("v1", "class-a"), &under("v2", "class-b"));
    let VersionDivergence::VersionConditioned {
        under_left,
        under_right,
    } = &comparison.divergence
    else {
        panic!("a class change across versions must be version-conditioned");
    };
    assert_eq!(under_left.as_ref().unwrap().as_str(), "class-a");
    assert_eq!(under_right.as_ref().unwrap().as_str(), "class-b");

    let encoded = serde_json::to_string(&comparison).expect("the comparison serialises");
    for forbidden in ["wrong", "corrected", "superseded"] {
        assert!(!encoded.contains(forbidden), "report adjudicated: {forbidden}");
    }
}

/// 30.11 again: an abstention is a result, and it never becomes its nearest class.
#[test]
fn an_abstaining_classifier_reports_an_outcome_not_a_gap() {
    let outcome = MethylationOutcome::Unclassifiable {
        reason: UnclassifiableReason::NoClassAboveThreshold {
            best: score(4_900),
            threshold: score(5_000),
        },
        nearest: None,
    };
    assert!(outcome.class().is_none());
    assert!(outcome.require_class().is_err());
    let encoded = serde_json::to_string(&outcome).expect("the outcome serialises");
    assert!(encoded.contains("unclassifiable"));
    assert!(encoded.contains("no_class_above_threshold"));
}

/// 30.08: "IDH status prevalence differs sharply by site and scanner. A model achieves high pooled
/// AUROC but fails within sites. The agent must identify site leakage and redesign the evaluation."
///
/// The claim that a design like this cannot support is the mechanistic one; the same result
/// stratified by site and scanner can.
#[test]
fn a_pooled_result_supports_an_association_but_not_a_mechanism_until_site_is_stratified() {
    let observation = SpecimenObservation::new(
        MolecularMarker::IdhMutation,
        SpecimenSampling::new("S1").sampling(RegionId::new("enhancing core")),
        Observed::Value(MarkerCall::Present),
    );
    let mut transport = DeclaredTransport::new(
        ScopeKey::new().exact("imaging_series", "SER-1"),
        ScopeKey::new().exact("patient", "PT-1"),
        "an imaging phenotype stands for the molecular state of the tumour",
    )
    .losing("regional molecular variation within the lesion")
    .adding_uncertainty("scanner and protocol variation across sites");
    for assumption in RADIOGENOMIC_ASSUMPTIONS {
        transport = transport.assuming(*assumption, "stated in the analysis plan");
    }

    let pooled = EvaluationDesign::new(SplitUnit::Participant, "features-v1")
        .features_fitted_on_training_split();
    let mechanism = RadiogenomicClaim {
        target: ClaimTarget::Mechanism,
        statement: "the imaging phenotype reflects the alteration's biology".to_string(),
    };
    assert!(matches!(
        assert_claim(mechanism.clone(), &pooled, &observation, &transport).unwrap_err(),
        TransportRefusal::UnstratifiedClaim { .. }
    ));

    let association = RadiogenomicClaim {
        target: ClaimTarget::Association,
        statement: "the imaging feature carries information about the call in this cohort"
            .to_string(),
    };
    assert!(assert_claim(association, &pooled, &observation, &transport).is_ok());

    let stratified = pooled.stratified_by("site").stratified_by("scanner");
    assert!(assert_claim(mechanism, &stratified, &observation, &transport).is_ok());
}

/// 30.19: "Only aggressive specimens establish organoids. A drug appears effective in the
/// established panel. The system must account for establishment selection before describing
/// population relevance."
#[test]
fn a_drug_effect_in_a_selected_organoid_panel_is_not_a_population_claim() {
    let result = ModelResult::new(
        ModelIdentity::new("ORG-1", ModelSystem::Organoid, "S1", 4).verified(),
        "the compound reduced viability",
        ReplicateStructure {
            technical_wells: 48,
            biological_replicates: 4,
        },
    )
    .resting_on(FidelityAxis::Genomic);
    let fidelity = FidelityEvidence::new().measured(FidelityAxis::Genomic, 4);
    let mut transport = DeclaredTransport::new(
        ScopeKey::new().exact("specimen", "S1"),
        ScopeKey::new().exact("cohort", "the population models were attempted from"),
        "an ex vivo sensitivity stands for a population-level statement",
    )
    .losing("the microenvironment, immune compartment and blood-brain barrier")
    .adding_uncertainty("passage drift between establishment and assay");
    for assumption in MODEL_ASSUMPTIONS {
        transport = transport.assuming(*assumption, "stated in the study protocol");
    }

    let selected = EstablishmentCohort::new(37, 9);
    assert_eq!(
        transport_to_patients(&result, &fidelity, selected, 4, &transport).unwrap_err(),
        TransportRefusal::UnmodelledEstablishmentSelection {
            attempted: 37,
            established: 9,
        }
    );

    let modelled = selected.with_selection_modelled();
    let claim = transport_to_patients(&result, &fidelity, modelled, 4, &transport)
        .expect("with selection modelled and the transport declared, the claim is available");
    assert_eq!(claim.establishment().attempted, 37);

    assert!(matches!(
        transport_to_patients(&result, &fidelity, modelled, 48, &transport).unwrap_err(),
        TransportRefusal::TechnicalReplicatesAsBiological { .. }
    ));
}

/// 30.27: "A model depends on availability of an advanced MRI sequence absent at many sites. The
/// evaluation compares full-resource and constrained-resource architectures rather than treating
/// missingness as a patient defect."
#[test]
fn an_assay_a_site_cannot_run_is_a_fact_about_the_site() {
    let constrained = SiteAssayContext::new(
        "a site without the platform",
        "an advanced acquisition",
        AssayAvailability::UnavailableAtSite,
    );
    assert_eq!(
        constrained.observation(),
        Observed::Unobserved(ObservationStatus::NotCollected)
    );
    assert!(matches!(
        as_negative_call(&constrained).unwrap_err(),
        ShiftRefusal::ResourceAbsenceReadAsBiology { .. }
    ));
}

/// 30.23: "A survival model trained on one institution's newly diagnosed GBM cohort is evaluated on
/// a mixed historical cohort. The agent must identify classification, treatment-era, and
/// ascertainment shifts."
#[test]
fn a_cohort_spanning_a_criteria_revision_needs_a_stated_mapping_before_comparison() {
    let current = Cohort::new("single institution, current", "site A", ClassificationVersion::new("criteria-B"))
        .containing(EntityLabel::new("entity-1a"));
    let historical = Cohort::new("mixed historical", "site B", ClassificationVersion::new("criteria-A"))
        .containing(EntityLabel::new("entity-1"))
        .containing(EntityLabel::new("entity-2"));
    assert!(matches!(
        comparable_cohorts(&current, &historical, None).unwrap_err(),
        ShiftRefusal::UnmappedClassificationChange { .. }
    ));
}

/// 30.24: "A radiosurgery outcome model sees many lesions per patient. PRISM checks whether splits,
/// confidence intervals, and estimands respect the patient-level cluster and competing events."
#[test]
fn many_lesions_per_patient_need_a_cluster_and_a_competing_risk() {
    let set = LesionSet {
        lesions: 63,
        participants: 21,
    };
    assert!(matches!(
        declare_cluster(set, false).unwrap_err(),
        EntityWorldRefusal::UndeclaredCluster { .. }
    ));
    assert!(declare_cluster(set, true).is_ok());
    assert!(matches!(
        handle_event(
            LesionEndpoint::LocalControl,
            FollowUpEvent::SystemicDeath,
            EventHandling::Censoring
        )
        .unwrap_err(),
        EntityWorldRefusal::CompetingEventAsCensoring { .. }
    ));
}

/// The crate's first thesis, across two modules: a negative call on one fragment cannot become the
/// ground truth a radiogenomic model is trained against.
#[test]
fn a_negative_call_on_one_fragment_is_not_a_tumour_level_training_label() {
    let negative = SpecimenObservation::new(
        MolecularMarker::IdhMutation,
        SpecimenSampling::new("S1")
            .sampling(RegionId::new("enhancing core"))
            .detecting_down_to(DetectionSensitivity {
                smallest_detectable_fraction: fraction(500),
                declared_by: "the assay's validation report".to_string(),
            }),
        Observed::Value(MarkerCall::Absent),
    );
    assert!(matches!(
        tumour_label(&negative).unwrap_err(),
        TransportRefusal::SpecimenScopedTarget { .. }
    ));

    let positive = SpecimenObservation::new(
        MolecularMarker::IdhMutation,
        SpecimenSampling::new("S1").sampling(RegionId::new("enhancing core")),
        Observed::Value(MarkerCall::Present),
    );
    assert!(tumour_label(&positive).is_ok());
}

/// The crate's second thesis: without a transport, a model-system result stays a model-system
/// result, and the sentence it licenses says so.
#[test]
fn an_untransported_organoid_result_still_says_what_it_is_about() {
    let result = ModelResult::new(
        ModelIdentity::new("PDX-1", ModelSystem::PatientDerivedXenograft, "S1", 2).verified(),
        "tumour growth slowed",
        ReplicateStructure {
            technical_wells: 12,
            biological_replicates: 3,
        },
    );
    let stated = result.as_stated();
    assert!(stated.starts_with("in patient-derived xenograft PDX-1 at passage 2"));

    let bare = DeclaredTransport::new(
        ScopeKey::new().exact("specimen", "S1"),
        ScopeKey::new().exact("cohort", "patients"),
        "the model stands for the patient",
    );
    assert!(matches!(
        transport_to_patients(
            &result,
            &FidelityEvidence::new(),
            EstablishmentCohort::new(5, 5),
            3,
            &bare
        )
        .unwrap_err(),
        TransportRefusal::UndeclaredLoss { .. }
    ));
}
