//! Invariant tests for OncoWorld.
//!
//! Each test name states the claim it defends, so a failure reads as the invariant that broke
//! rather than as an assertion that tripped. The blueprint module each claim comes from is cited
//! on the type or function under test, not repeated here.

use bioprism_onco::{
    assess, classify, AcquisitionTime, AnalysisBias, AnalysisOutcome, AvailabilityTime,
    BoundaryDisposition, BoundaryRequest, CensoringAssumption, CensoringReason, ChangeHypothesis,
    ClinicalObservation, ClinicalTrend, Clocks, CnsEntity, CnsGrade, Compartment, ConfirmatoryStudy,
    ConsentBasis, DeathCause, DiagnosticResolution, DirectionOfChange, EndpointKind,
    EscalationNotice, EscalationRoute, EscalationTrigger, EvidenceRole, FollowUp, Histology,
    ImagingModality, ImagingObservation, IntercurrentEvent, IntercurrentEventStrategy, Karnofsky,
    MarkerCall, MarkerPanel, MolecularMarker, ObservationStatus, Observation, Observed, OncoError,
    OutputUse, Population, ProgressionBasis, ProgressionEvidence, Reclassification, RecordTime,
    ReleaseTime, RequestContext, ResearchBoundary, ResponseAssessment, ResponseCall,
    ResponseCategory, ResponseCriterion, ResponseRequest, SourceDiagnosis, SubjectRef, TargetLesion,
    TerminalAction, TerminalFact, Timepoint, TreatmentContext, TreatmentModality, TumourWorldline,
};
use bioprism_scope::{ScopeKey, ScopeValue, Timestamp};
use bioprism_world::Fact;
use std::collections::BTreeSet;

fn ts(text: &str) -> Timestamp {
    Timestamp::parse(text).expect("test timestamp is well formed")
}

fn at(text: &str) -> AcquisitionTime {
    AcquisitionTime::new(ts(text))
}

fn flat_clocks(day: &str) -> Clocks {
    clocks(day, day, day, day)
}

fn clocks(acquired: &str, recorded: &str, released: &str, visible: &str) -> Clocks {
    Clocks {
        acquired: AcquisitionTime::new(ts(acquired)),
        recorded: RecordTime::new(ts(recorded)),
        released: ReleaseTime::new(ts(released)),
        visible: AvailabilityTime::new(ts(visible)),
    }
}

fn lesion(long_mm: f64, perpendicular_mm: f64) -> TargetLesion {
    TargetLesion::new("target", long_mm, perpendicular_mm).expect("test lesion is measurable")
}

fn scan(lesions: Vec<TargetLesion>, new_lesion: bool) -> ImagingObservation {
    ImagingObservation {
        modality: ImagingModality::MriT1PostContrast,
        compartment: Compartment::ContrastEnhancing,
        target_lesions: lesions,
        new_lesion: Observed::Value(new_lesion),
        nonmeasurable_change: Observed::Value(DirectionOfChange::Unchanged),
        comparable_to_baseline: true,
    }
}

fn clinical(steroid_mg: f64, trend: ClinicalTrend) -> ClinicalObservation {
    ClinicalObservation {
        corticosteroid_dexamethasone_equivalent_mg_per_day: Observed::Value(steroid_mg),
        performance_status: Observed::Value(Karnofsky::new(80).expect("80 is a decile")),
        trend: Observed::Value(trend),
    }
}

fn imaging_timepoint(label: &str, clocks: Clocks, lesions: Vec<TargetLesion>) -> Timepoint {
    Timepoint::new(label, clocks, Observation::Imaging(scan(lesions, false)))
        .expect("test clocks are ordered")
}

/// A worked response scenario, owned so that [`ResponseRequest`]'s borrows have somewhere to live.
struct Scenario {
    criterion: ResponseCriterion,
    baseline: ImagingObservation,
    current: ImagingObservation,
    baseline_clinical: ClinicalObservation,
    current_clinical: ClinicalObservation,
    treatment: TreatmentContext,
    evidence: ProgressionEvidence,
    current_acquired: AcquisitionTime,
    nadir_spd_mm2: Option<f64>,
    measurement_error_fraction: f64,
}

impl Scenario {
    /// Chemoradiotherapy finished 2024-01-01; the index scan is 31 days later, well inside the
    /// 84-day window, and the enhancing burden has grown from 1200 to 2000 mm².
    fn progressing_inside_the_window() -> Self {
        Scenario {
            criterion: ResponseCriterion::high_grade_2010(),
            baseline: scan(vec![lesion(40.0, 30.0)], false),
            current: scan(vec![lesion(50.0, 40.0)], false),
            baseline_clinical: clinical(4.0, ClinicalTrend::Stable),
            current_clinical: clinical(4.0, ClinicalTrend::Stable),
            treatment: TreatmentContext {
                modality: TreatmentModality::ChemoRadiotherapy,
                completed: at("2024-01-01T00:00:00Z"),
            },
            evidence: ProgressionEvidence::default(),
            current_acquired: at("2024-02-01T00:00:00Z"),
            nadir_spd_mm2: None,
            measurement_error_fraction: 0.10,
        }
    }

    fn assess(&self) -> ResponseAssessment {
        assess(&ResponseRequest {
            criterion: &self.criterion,
            baseline: &self.baseline,
            current: &self.current,
            current_acquired: self.current_acquired,
            nadir_spd_mm2: self.nadir_spd_mm2,
            baseline_clinical: &self.baseline_clinical,
            current_clinical: &self.current_clinical,
            treatment: &self.treatment,
            evidence: &self.evidence,
            measurement_error_fraction: self.measurement_error_fraction,
        })
        .expect("scenario measurements are finite and non-negative")
    }
}

fn diffuse_glioma_panel() -> MarkerPanel {
    MarkerPanel::nothing_collected()
        .observed(MolecularMarker::IdhMutation, MarkerCall::Present)
        .observed(MolecularMarker::Codeletion1p19q, MarkerCall::Absent)
}

#[test]
fn histology_alone_is_unresolved_and_names_no_entity() {
    let resolution = classify(Histology::DiffuseGlioma, &MarkerPanel::nothing_collected());

    assert_eq!(resolution.entity(), None);
    assert!(!resolution.is_integrated());
    let DiagnosticResolution::Unresolved { candidates, obligations } = &resolution else {
        panic!("histology alone must be unresolved, got {resolution:?}");
    };
    assert!(candidates.len() > 1, "several entities remain admissible");
    assert!(
        !obligations.is_empty(),
        "an unresolved state must carry a prioritized evidence request"
    );
}

#[test]
fn an_unrun_assay_reports_not_collected_and_is_never_read_as_a_negative_call() {
    let panel = MarkerPanel::nothing_collected();

    assert_eq!(
        panel.state(MolecularMarker::IdhMutation),
        Observed::Unobserved(ObservationStatus::NotCollected)
    );
    assert_ne!(
        panel.state(MolecularMarker::IdhMutation),
        Observed::Value(MarkerCall::Absent)
    );
    assert!(!panel.state(MolecularMarker::IdhMutation).is_observed());
}

#[test]
fn a_technically_failed_assay_is_distinct_from_one_below_detection() {
    let failed = MarkerPanel::nothing_collected().unobserved(
        MolecularMarker::IdhMutation,
        ObservationStatus::TechnicallyFailed,
    );
    let below = MarkerPanel::nothing_collected().unobserved(
        MolecularMarker::IdhMutation,
        ObservationStatus::BelowDetection,
    );

    assert_ne!(
        failed.state(MolecularMarker::IdhMutation),
        below.state(MolecularMarker::IdhMutation)
    );
    assert!(!ObservationStatus::TechnicallyFailed.is_informative_about_the_value());
    assert!(ObservationStatus::BelowDetection.is_informative_about_the_value());
}

#[test]
fn integrated_classification_is_a_conjunction_of_histology_and_molecular_evidence() {
    let resolution = classify(Histology::DiffuseGlioma, &diffuse_glioma_panel());

    assert_eq!(resolution.entity(), Some(CnsEntity::AstrocytomaIdhMutant));
    let DiagnosticResolution::Integrated { evidence, .. } = &resolution else {
        panic!("expected an integrated resolution, got {resolution:?}");
    };
    assert!(evidence
        .iter()
        .any(|item| item.marker == MolecularMarker::IdhMutation
            && item.role == EvidenceRole::Required));
}

#[test]
fn an_observed_marker_that_contradicts_required_criteria_excludes_the_entity() {
    let codeleted = MarkerPanel::nothing_collected()
        .observed(MolecularMarker::IdhMutation, MarkerCall::Present)
        .observed(MolecularMarker::Codeletion1p19q, MarkerCall::Present);

    assert_eq!(
        classify(Histology::DiffuseGlioma, &codeleted).entity(),
        Some(CnsEntity::OligodendrogliomaIdhMutant1p19qCodeleted)
    );
}

#[test]
fn an_unobserved_marker_neither_advances_nor_excludes_a_candidate() {
    let idh_wildtype_only =
        MarkerPanel::nothing_collected().observed(MolecularMarker::IdhMutation, MarkerCall::Absent);

    let resolution = classify(Histology::DiffuseGlioma, &idh_wildtype_only);

    assert_eq!(resolution.entity(), None);
    let DiagnosticResolution::Unresolved { candidates, .. } = &resolution else {
        panic!("an unobserved H3 status leaves H3-altered entities admissible, got {resolution:?}");
    };
    assert!(candidates.contains(&CnsEntity::DiffuseMidlineGliomaH3K27Altered));
    assert!(candidates.contains(&CnsEntity::GlioblastomaIdhWildtype));

    let h3_tested = idh_wildtype_only
        .observed(MolecularMarker::H3K27Alteration, MarkerCall::Absent)
        .observed(MolecularMarker::H3G34Mutation, MarkerCall::Absent);
    let resolution = classify(Histology::DiffuseGlioma, &h3_tested);

    let DiagnosticResolution::Provisional { candidate, obligations } = &resolution else {
        panic!("one surviving candidate must be provisional, got {resolution:?}");
    };
    assert_eq!(*candidate, CnsEntity::GlioblastomaIdhWildtype);
    assert!(obligations
        .iter()
        .any(|obligation| obligation.marker == MolecularMarker::TertPromoterMutation));
}

#[test]
fn the_evidence_request_is_ordered_by_how_many_candidates_each_assay_discriminates() {
    let resolution = classify(Histology::DiffuseGlioma, &MarkerPanel::nothing_collected());
    let obligations = resolution.obligations();

    assert!(obligations.len() > 1);
    for pair in obligations.windows(2) {
        assert!(
            pair[0].discriminates >= pair[1].discriminates,
            "obligations must be ordered most discriminating first"
        );
    }
    assert_eq!(obligations[0].marker, MolecularMarker::IdhMutation);
}

#[test]
fn an_uncollected_cdkn2a_leaves_grade_unobserved_rather_than_defaulting_to_the_lower_grade() {
    let resolution = classify(Histology::DiffuseGlioma, &diffuse_glioma_panel());
    let DiagnosticResolution::Integrated { grade, .. } = &resolution else {
        panic!("expected an integrated resolution");
    };
    assert_eq!(
        *grade,
        Observed::Unobserved(ObservationStatus::NotCollected),
        "grade 2 and 3 need histology this crate does not model, so no grade may be asserted"
    );

    let deleted = diffuse_glioma_panel().observed(
        MolecularMarker::Cdkn2aCdkn2bHomozygousDeletion,
        MarkerCall::Present,
    );
    let DiagnosticResolution::Integrated { grade, .. } = classify(Histology::DiffuseGlioma, &deleted)
    else {
        panic!("expected an integrated resolution");
    };
    assert_eq!(grade, Observed::Value(CnsGrade::Four));
}

#[test]
fn histology_outside_the_implemented_scope_is_not_otherwise_resolved_rather_than_guessed() {
    let resolution = classify(
        Histology::OutsideImplementedScope,
        &MarkerPanel::nothing_collected(),
    );

    assert_eq!(resolution.entity(), None);
    assert!(matches!(
        resolution,
        DiagnosticResolution::NotOtherwiseResolved { .. }
    ));
}

#[test]
fn reclassification_cites_the_source_diagnosis_instead_of_overwriting_it() {
    let source = SourceDiagnosis::new("anaplastic astrocytoma", "who-cns-2007");
    let record = Reclassification::new(
        source.clone(),
        classify(Histology::DiffuseGlioma, &diffuse_glioma_panel()),
        "criteria-table-2024.1",
    );

    assert_eq!(record.source.text(), source.text());
    assert_eq!(record.source.ontology_version(), "who-cns-2007");
    assert_eq!(record.rule_version, "criteria-table-2024.1");
}

#[test]
fn agent_visibility_may_not_precede_release_and_the_four_clocks_are_ordered() {
    let disordered = clocks(
        "2024-01-01T00:00:00Z",
        "2024-01-02T00:00:00Z",
        "2024-01-03T00:00:00Z",
        "2024-01-01T00:00:00Z",
    );

    let failure = Timepoint::new(
        "t1",
        disordered,
        Observation::Clinical(clinical(4.0, ClinicalTrend::Stable)),
    )
    .expect_err("visibility before release must be refused");

    assert!(matches!(
        failure,
        OncoError::ClockOrderViolation {
            earlier_axis: "release",
            later_axis: "agent visibility",
            ..
        }
    ));
}

#[test]
fn record_order_and_biological_order_are_different_orders() {
    let early_scan_transcribed_late = imaging_timepoint(
        "jan",
        clocks(
            "2024-01-01T00:00:00Z",
            "2024-06-01T00:00:00Z",
            "2024-06-01T00:00:00Z",
            "2024-06-01T00:00:00Z",
        ),
        vec![lesion(40.0, 30.0)],
    );
    let later_scan_transcribed_promptly = imaging_timepoint(
        "mar",
        clocks(
            "2024-03-01T00:00:00Z",
            "2024-03-05T00:00:00Z",
            "2024-03-05T00:00:00Z",
            "2024-03-05T00:00:00Z",
        ),
        vec![lesion(40.0, 30.0)],
    );

    let mut worldline = TumourWorldline::new(
        SubjectRef::new("S-001").expect("pseudonym"),
        early_scan_transcribed_late,
    );
    worldline
        .push(later_scan_transcribed_promptly)
        .expect("distinct label");

    let biological: Vec<&str> = worldline
        .timepoints()
        .iter()
        .map(Timepoint::label)
        .collect();
    let recorded: Vec<&str> = worldline
        .in_record_order()
        .iter()
        .map(|timepoint| timepoint.label())
        .collect();

    assert_eq!(biological, vec!["jan", "mar"]);
    assert_eq!(recorded, vec!["mar", "jan"]);
    assert_ne!(biological, recorded);
}

#[test]
fn the_temporal_firewall_cuts_on_agent_visibility_not_on_acquisition() {
    let acquired_first_released_last = imaging_timepoint(
        "jan",
        clocks(
            "2024-01-01T00:00:00Z",
            "2024-06-01T00:00:00Z",
            "2024-06-01T00:00:00Z",
            "2024-06-01T00:00:00Z",
        ),
        vec![lesion(40.0, 30.0)],
    );
    let acquired_later_released_promptly = imaging_timepoint(
        "mar",
        clocks(
            "2024-03-01T00:00:00Z",
            "2024-03-05T00:00:00Z",
            "2024-03-05T00:00:00Z",
            "2024-03-05T00:00:00Z",
        ),
        vec![lesion(40.0, 30.0)],
    );

    let mut worldline = TumourWorldline::new(
        SubjectRef::new("S-001").expect("pseudonym"),
        acquired_first_released_last,
    );
    worldline
        .push(acquired_later_released_promptly)
        .expect("distinct label");

    let visible = worldline.visible_at(AvailabilityTime::new(ts("2024-04-01T00:00:00Z")));

    assert_eq!(visible.len(), 1);
    assert_eq!(
        visible[0].label(),
        "mar",
        "the January study was acquired first but had not been released"
    );
}

#[test]
fn observations_before_the_baseline_have_negative_time_from_baseline() {
    let baseline = imaging_timepoint(
        "baseline",
        flat_clocks("2024-02-01T00:00:00Z"),
        vec![lesion(40.0, 30.0)],
    );
    let preoperative = imaging_timepoint(
        "pre-op",
        flat_clocks("2024-01-15T00:00:00Z"),
        vec![lesion(45.0, 35.0)],
    );

    let mut worldline =
        TumourWorldline::new(SubjectRef::new("S-001").expect("pseudonym"), baseline);
    worldline.push(preoperative).expect("distinct label");

    let earliest = &worldline.timepoints()[0];
    assert_eq!(earliest.label(), "pre-op");
    assert_eq!(worldline.time_from_baseline(earliest).days(), -17);
}

#[test]
fn the_nadir_excludes_studies_the_agent_could_not_yet_have_seen() {
    let baseline = imaging_timepoint(
        "baseline",
        flat_clocks("2024-01-01T00:00:00Z"),
        vec![lesion(40.0, 30.0)],
    );
    let small_but_embargoed = imaging_timepoint(
        "embargoed",
        clocks(
            "2024-02-01T00:00:00Z",
            "2024-09-01T00:00:00Z",
            "2024-09-01T00:00:00Z",
            "2024-09-01T00:00:00Z",
        ),
        vec![lesion(10.0, 10.0)],
    );

    let mut worldline =
        TumourWorldline::new(SubjectRef::new("S-001").expect("pseudonym"), baseline);
    worldline.push(small_but_embargoed).expect("distinct label");

    let cutoff = AvailabilityTime::new(ts("2024-03-01T00:00:00Z"));
    let nadir = worldline.nadir_spd_mm2(
        Compartment::ContrastEnhancing,
        at("2024-04-01T00:00:00Z"),
        cutoff,
    );

    assert_eq!(nadir, Some(1200.0), "the 100 mm² study was not yet visible");
}

#[test]
fn pseudoprogression_inside_the_treatment_window_is_never_reported_as_progression() {
    let assessment = Scenario::progressing_inside_the_window().assess();

    assert_eq!(assessment.unconfirmed_reading, ResponseCategory::Progression);
    assert!(assessment.call.confirmed_progression().is_none());
    assert!(assessment.withheld_progression());
    assert!(matches!(
        assessment.call,
        ResponseCall::NotEvaluable(bioprism_onco::NotEvaluableReason::PostTreatmentChangeNotExcluded {
            days_since_treatment_end: 31,
            window_days: 84,
        })
    ));
}

#[test]
fn withholding_a_progression_call_yields_not_evaluable_and_never_stable_disease() {
    let assessment = Scenario::progressing_inside_the_window().assess();

    assert_ne!(assessment.call, ResponseCall::Stable);
    assert!(matches!(assessment.call, ResponseCall::NotEvaluable(_)));
    assert_eq!(assessment.call.label(), "not evaluable");
}

#[test]
fn timing_alone_withholds_progression_but_never_asserts_treatment_effect() {
    let assessment = Scenario::progressing_inside_the_window().assess();

    assert!(assessment
        .hypotheses
        .contains(&ChangeHypothesis::Progression));
    assert!(assessment
        .hypotheses
        .contains(&ChangeHypothesis::TreatmentEffect));
    assert!(assessment
        .hypotheses
        .contains(&ChangeHypothesis::MixedProcess));
    assert!(
        assessment.hypotheses.is_non_identifiable(),
        "timing cannot identify the cause of change"
    );
    assert!(!assessment.hypotheses.evidence_requests().is_empty());
}

#[test]
fn a_criterion_that_predates_pseudoprogression_still_does_not_get_a_progression_call() {
    let mut scenario = Scenario::progressing_inside_the_window();
    scenario.criterion = ResponseCriterion::macdonald_1990();

    let assessment = scenario.assess();

    assert!(assessment.call.confirmed_progression().is_none());
    let divergence = assessment
        .divergence_from_criterion
        .expect("the platform diverged from the criterion and must record it");
    assert_eq!(divergence.criterion_would_call, ResponseCategory::Progression);
    assert!(!divergence.criterion_recognises_post_treatment_change);
    assert_eq!(assessment.criterion_id, "macdonald");
    assert_eq!(assessment.criterion_version, "1990");
}

#[test]
fn progression_outside_the_treatment_window_is_confirmed_and_carries_its_basis() {
    let mut scenario = Scenario::progressing_inside_the_window();
    scenario.current_acquired = at("2024-05-01T00:00:00Z");

    let assessment = scenario.assess();

    let confirmed = assessment
        .call
        .confirmed_progression()
        .expect("121 days is beyond the 84-day window");
    assert_eq!(
        confirmed.basis(),
        ProgressionBasis::OutsidePostTreatmentWindow {
            days_since_treatment_end: 121
        }
    );
    assert!(assessment.divergence_from_criterion.is_none());
}

#[test]
fn histopathologic_confirmation_licenses_progression_inside_the_window() {
    let mut scenario = Scenario::progressing_inside_the_window();
    scenario.evidence.histopathologic_confirmation = Observed::Value(true);

    let assessment = scenario.assess();

    assert_eq!(
        assessment
            .call
            .confirmed_progression()
            .map(bioprism_onco::ConfirmedProgression::basis),
        Some(ProgressionBasis::Histopathologic)
    );
}

#[test]
fn a_confirmatory_scan_closer_than_the_criterion_interval_does_not_confirm_progression() {
    let mut scenario = Scenario::progressing_inside_the_window();
    scenario.evidence.confirmatory = Some(ConfirmatoryStudy {
        acquired: at("2024-02-15T00:00:00Z"),
        still_progressive: true,
    });

    let assessment = scenario.assess();

    assert!(assessment.call.confirmed_progression().is_none());
    assert!(matches!(
        assessment.call,
        ResponseCall::NotEvaluable(
            bioprism_onco::NotEvaluableReason::ConfirmationIntervalNotMet {
                interval_days: 14,
                required_days: 28,
            }
        )
    ));
}

#[test]
fn a_confirmatory_scan_beyond_the_window_and_the_interval_confirms_progression() {
    let mut scenario = Scenario::progressing_inside_the_window();
    scenario.evidence.confirmatory = Some(ConfirmatoryStudy {
        acquired: at("2024-04-01T00:00:00Z"),
        still_progressive: true,
    });

    let assessment = scenario.assess();

    assert_eq!(
        assessment
            .call
            .confirmed_progression()
            .map(bioprism_onco::ConfirmedProgression::basis),
        Some(ProgressionBasis::ConfirmedByFollowUp { interval_days: 60 })
    );
}

#[test]
fn a_measurement_within_inter_reader_error_of_a_threshold_is_not_evaluable() {
    let mut scenario = Scenario::progressing_inside_the_window();
    scenario.current_acquired = at("2024-05-01T00:00:00Z");
    scenario.current = scan(vec![lesion(52.0, 30.0)], false);

    let assessment = scenario.assess();

    assert!(assessment.sensitivity.flips_within_measurement_error);
    assert!(matches!(
        assessment.call,
        ResponseCall::NotEvaluable(bioprism_onco::NotEvaluableReason::NearThreshold { .. })
    ));
    assert_ne!(assessment.call, ResponseCall::Stable);
}

#[test]
fn rising_corticosteroids_make_an_apparent_response_not_evaluable() {
    let mut scenario = Scenario::progressing_inside_the_window();
    scenario.current = scan(vec![lesion(20.0, 15.0)], false);

    let flat_steroids = scenario.assess();
    assert_eq!(flat_steroids.unconfirmed_reading, ResponseCategory::Partial);
    assert_eq!(flat_steroids.call, ResponseCall::Partial);

    scenario.current_clinical = clinical(12.0, ClinicalTrend::Stable);
    let rising_steroids = scenario.assess();

    assert_eq!(rising_steroids.unconfirmed_reading, ResponseCategory::Partial);
    assert!(matches!(
        rising_steroids.call,
        ResponseCall::NotEvaluable(
            bioprism_onco::NotEvaluableReason::CorticosteroidDoseIncreased
        )
    ));
}

#[test]
fn a_criterion_measuring_another_compartment_cannot_evaluate_the_study() {
    let mut scenario = Scenario::progressing_inside_the_window();
    scenario.criterion = ResponseCriterion::low_grade_2011();

    let assessment = scenario.assess();

    assert!(matches!(
        assessment.call,
        ResponseCall::NotEvaluable(bioprism_onco::NotEvaluableReason::CompartmentMismatch {
            criterion: Compartment::T2Flair,
            observed: Compartment::ContrastEnhancing,
        })
    ));
}

#[test]
fn an_undocumented_new_lesion_status_blocks_every_call_including_stable() {
    let mut scenario = Scenario::progressing_inside_the_window();
    scenario.current = ImagingObservation {
        new_lesion: Observed::Unobserved(ObservationStatus::Missing),
        ..scan(vec![lesion(40.0, 30.0)], false)
    };

    let assessment = scenario.assess();

    assert_ne!(assessment.call, ResponseCall::Stable);
    assert!(matches!(
        assessment.call,
        ResponseCall::NotEvaluable(bioprism_onco::NotEvaluableReason::NewLesionStatusUnobserved(
            ObservationStatus::Missing
        ))
    ));
}

#[test]
fn anti_angiogenic_therapy_makes_falling_enhancement_not_evaluable_as_response() {
    let mut scenario = Scenario::progressing_inside_the_window();
    scenario.current = scan(vec![lesion(20.0, 15.0)], false);
    scenario.treatment = TreatmentContext {
        modality: TreatmentModality::AntiAngiogenic,
        completed: at("2024-01-01T00:00:00Z"),
    };

    let assessment = scenario.assess();

    assert_eq!(assessment.unconfirmed_reading, ResponseCategory::Partial);
    assert!(matches!(
        assessment.call,
        ResponseCall::NotEvaluable(bioprism_onco::NotEvaluableReason::PseudoresponseNotExcluded)
    ));
}

const ALL_ENDPOINTS: [EndpointKind; 4] = [
    EndpointKind::OverallSurvival,
    EndpointKind::ProgressionFreeSurvival,
    EndpointKind::TimeToProgression,
    EndpointKind::TimeToTreatmentFailure,
];

#[test]
fn loss_to_follow_up_is_censoring_under_every_endpoint_and_is_never_an_event() {
    for endpoint in ALL_ENDPOINTS {
        let outcome = endpoint.classify(&TerminalFact::LostToFollowUp);
        assert_eq!(
            outcome,
            AnalysisOutcome::Censored(CensoringReason::LostToFollowUp),
            "{endpoint:?} must censor a subject lost to follow-up"
        );
        assert!(!outcome.is_event());
    }
}

#[test]
fn loss_to_follow_up_is_flagged_as_potentially_informative_censoring() {
    assert!(CensoringReason::LostToFollowUp.is_potentially_informative());
    assert!(!CensoringReason::AdministrativeCutoff.is_potentially_informative());
}

#[test]
fn death_is_an_event_for_progression_free_survival_and_a_competing_risk_for_time_to_progression() {
    let death = TerminalFact::Death {
        cause: DeathCause::DiseaseRelated,
    };

    assert!(EndpointKind::ProgressionFreeSurvival
        .classify(&death)
        .is_event());
    assert_eq!(
        EndpointKind::TimeToProgression.classify(&death),
        AnalysisOutcome::Censored(CensoringReason::CompetingDeath)
    );
}

#[test]
fn a_not_evaluable_assessment_cannot_be_recorded_as_a_progression_event() {
    let assessment = Scenario::progressing_inside_the_window().assess();

    let failure = FollowUp::from_assessment(
        SubjectRef::new("S-001").expect("pseudonym"),
        at("2024-01-01T00:00:00Z"),
        at("2024-01-01T00:00:00Z"),
        &assessment,
    )
    .expect_err("a withheld progression is not an outcome event");

    assert_eq!(
        failure,
        OncoError::ResponseCallIsNotProgression {
            call: "not evaluable"
        }
    );
}

#[test]
fn a_confirmed_progression_can_be_recorded_as_a_progression_event() {
    let mut scenario = Scenario::progressing_inside_the_window();
    scenario.current_acquired = at("2024-05-01T00:00:00Z");
    let assessment = scenario.assess();

    let follow_up = FollowUp::from_assessment(
        SubjectRef::new("S-001").expect("pseudonym"),
        at("2024-01-01T00:00:00Z"),
        at("2024-01-01T00:00:00Z"),
        &assessment,
    )
    .expect("a confirmed progression is an outcome event");

    assert!(matches!(
        follow_up.terminal(),
        TerminalFact::Progression(_)
    ));
    let estimand = EndpointKind::TimeToProgression.default_estimand(Population::IntentionToTreat);
    assert!(follow_up.analyse(&estimand).outcome.is_event());
}

#[test]
fn at_risk_time_starts_at_risk_set_entry_so_immortal_time_is_not_counted() {
    let follow_up = FollowUp::new(
        SubjectRef::new("S-001").expect("pseudonym"),
        at("2024-01-01T00:00:00Z"),
        at("2024-03-01T00:00:00Z"),
        at("2024-09-01T00:00:00Z"),
        TerminalFact::AdministrativeCutoff,
    )
    .expect("delayed entry is legitimate");

    assert!(follow_up.is_left_truncated());
    assert_eq!(follow_up.immortal_time_days(), 60);
    assert_eq!(follow_up.at_risk_days(), 184);

    let estimand = EndpointKind::OverallSurvival.default_estimand(Population::IntentionToTreat);
    let analysed = follow_up.analyse(&estimand);
    assert_eq!(analysed.at_risk_days, 184);
    assert!(analysed.bias_flags.contains(&AnalysisBias::LeftTruncation));
}

#[test]
fn risk_set_entry_before_the_index_date_is_refused() {
    let failure = FollowUp::new(
        SubjectRef::new("S-001").expect("pseudonym"),
        at("2024-03-01T00:00:00Z"),
        at("2024-01-01T00:00:00Z"),
        at("2024-09-01T00:00:00Z"),
        TerminalFact::AdministrativeCutoff,
    )
    .expect_err("negative entry is not delayed entry");

    assert!(matches!(
        failure,
        OncoError::RiskSetEntryBeforeIndex { .. }
    ));
}

#[test]
fn time_to_progression_declares_its_censoring_potentially_informative() {
    let estimand = EndpointKind::TimeToProgression.default_estimand(Population::IntentionToTreat);

    assert!(matches!(
        estimand.censoring_assumption,
        CensoringAssumption::PotentiallyInformative { .. }
    ));
    assert!(estimand.intercurrent_event_strategies.contains(&(
        IntercurrentEvent::Death,
        IntercurrentEventStrategy::Hypothetical
    )));

    let overall = EndpointKind::OverallSurvival.default_estimand(Population::IntentionToTreat);
    assert_eq!(
        overall.censoring_assumption,
        CensoringAssumption::NoninformativeAssumed
    );
}

#[test]
fn an_analysed_record_carries_the_estimand_it_answers() {
    let follow_up = FollowUp::new(
        SubjectRef::new("S-001").expect("pseudonym"),
        at("2024-01-01T00:00:00Z"),
        at("2024-01-01T00:00:00Z"),
        at("2024-06-01T00:00:00Z"),
        TerminalFact::StartedSubsequentTherapy,
    )
    .expect("well-formed follow-up");

    let estimand =
        EndpointKind::ProgressionFreeSurvival.default_estimand(Population::EvaluableForResponse);
    let analysed = follow_up.analyse(&estimand);

    assert_eq!(analysed.estimand.endpoint, EndpointKind::ProgressionFreeSurvival);
    assert_eq!(analysed.estimand.population, Population::EvaluableForResponse);
    assert!(analysed
        .bias_flags
        .contains(&AnalysisBias::TreatmentSwitching));
}

fn research_request(uses: Vec<OutputUse>) -> BoundaryRequest {
    BoundaryRequest {
        purpose: "compare biomarker evidence across a published cohort".into(),
        context: RequestContext::Research,
        claimed_role: "research fellow".into(),
        claimed_urgency: false,
        consent: ConsentBasis::BroadResearchConsent,
        requested_uses: uses,
        direct_identifier_fields: Vec::new(),
    }
}

#[test]
fn the_boundary_refuses_treatment_recommendation_with_a_typed_error() {
    let boundary = ResearchBoundary::research_only();

    let failure = boundary
        .release("a cohort summary", OutputUse::TreatmentRecommendation)
        .expect_err("treatment recommendation is individual clinical use");

    assert!(matches!(
        failure,
        OncoError::OutsideResearchBoundary {
            attempted: OutputUse::TreatmentRecommendation,
            ..
        }
    ));
    assert!(failure.to_string().contains("does not diagnose a person"));
}

#[test]
fn the_boundary_refuses_every_individual_clinical_use() {
    let boundary = ResearchBoundary::research_only();
    for use_case in [
        OutputUse::IndividualDiagnosis,
        OutputUse::IndividualPrognosis,
        OutputUse::TreatmentRecommendation,
        OutputUse::CareTriage,
        OutputUse::ClinicalAlerting,
    ] {
        assert!(use_case.is_individual_clinical_use());
        assert!(!boundary.permits(use_case));
        assert!(boundary.check(use_case).is_err());
    }
}

#[test]
fn a_claimed_clinician_role_and_asserted_urgency_do_not_widen_the_boundary() {
    let boundary = ResearchBoundary::research_only();
    let plain = research_request(vec![OutputUse::TreatmentRecommendation]);
    let pressured = BoundaryRequest {
        claimed_role: "attending neuro-oncologist".into(),
        claimed_urgency: true,
        context: RequestContext::Care,
        ..plain.clone()
    };

    assert_eq!(
        boundary.triage(&plain).expect("no identifiers"),
        boundary.triage(&pressured).expect("no identifiers"),
        "role and urgency claims are recorded, never consulted"
    );
}

#[test]
fn a_mixed_request_keeps_the_cohort_analysis_and_refuses_the_individual_direction() {
    let boundary = ResearchBoundary::research_only();
    let request = research_request(vec![
        OutputUse::CohortAnalysis,
        OutputUse::TreatmentRecommendation,
    ]);

    let disposition = boundary.triage(&request).expect("no identifiers");

    assert_eq!(disposition.released(), [OutputUse::CohortAnalysis]);
    assert_eq!(disposition.refused(), [OutputUse::TreatmentRecommendation]);
    assert!(matches!(
        disposition,
        BoundaryDisposition::ReleasePartial { .. }
    ));
    assert_eq!(disposition.terminal_action(), TerminalAction::Escalate);
    assert!(disposition.escalation().is_some());
}

#[test]
fn a_wholly_research_request_is_released_in_full_without_escalation() {
    let boundary = ResearchBoundary::research_only();
    let request = research_request(vec![OutputUse::CohortAnalysis, OutputUse::MethodDevelopment]);

    let disposition = boundary.triage(&request).expect("no identifiers");

    assert!(matches!(
        disposition,
        BoundaryDisposition::ReleaseInFull { .. }
    ));
    assert!(disposition.refused().is_empty());
    assert!(disposition.escalation().is_none());
}

#[test]
fn direct_identifiers_are_refused_before_any_analysis_runs() {
    let boundary = ResearchBoundary::research_only();
    let request = BoundaryRequest {
        direct_identifier_fields: vec!["medical_record_number".into(), "date_of_birth".into()],
        ..research_request(vec![OutputUse::CohortAnalysis])
    };

    let failure = boundary
        .triage(&request)
        .expect_err("identifiable data never enter research outputs");

    assert_eq!(failure, OncoError::IdentifiersPresent { count: 2 });
    assert!(!failure.to_string().contains("medical_record_number"));
}

#[test]
fn an_escalation_notice_states_a_trigger_and_a_route_and_carries_no_action() {
    let notice = EscalationNotice::raise(
        EscalationTrigger::UnconfirmedProgressionSignal,
        EscalationRoute::StudyTeam,
    );

    assert_eq!(
        notice.trigger(),
        EscalationTrigger::UnconfirmedProgressionSignal
    );
    assert_eq!(notice.route(), EscalationRoute::StudyTeam);
    assert!(notice.to_string().contains("states no clinical action"));
    assert_eq!(
        notice,
        EscalationNotice::raise(
            EscalationTrigger::UnconfirmedProgressionSignal,
            EscalationRoute::StudyTeam
        ),
        "the notice holds nothing beyond its trigger and route"
    );
}

#[test]
fn a_boundary_cannot_be_extended_to_permit_individual_clinical_use() {
    let failure = ResearchBoundary::research_only()
        .extend(OutputUse::CareTriage)
        .expect_err("no configuration path may permit care triage");

    assert!(matches!(
        failure,
        OncoError::OutsideResearchBoundary {
            attempted: OutputUse::CareTriage,
            ..
        }
    ));
    assert!(ResearchBoundary::research_only()
        .extend(OutputUse::QualityControl)
        .is_ok());
}

#[test]
fn a_deserialised_boundary_that_permits_clinical_use_is_rejected() {
    let smuggled = serde_json::json!({ "permitted": ["cohort_analysis", "individual_diagnosis"] });

    let failure = serde_json::from_value::<ResearchBoundary>(smuggled)
        .expect_err("the guard cannot be widened through its wire format");

    assert!(failure.to_string().contains("research-only boundary"));
}

#[test]
fn a_released_output_records_the_use_it_was_released_for() {
    let boundary = ResearchBoundary::research_only();
    let released = boundary
        .release(vec![1_u32, 2, 3], OutputUse::CohortAnalysis)
        .expect("cohort analysis is permitted");

    assert_eq!(released.declared_use(), OutputUse::CohortAnalysis);
    assert_eq!(released.value(), &vec![1, 2, 3]);
    assert!(bioprism_onco::ResearchOutput::<u32>::STATEMENT.contains("Research use only"));
    assert_eq!(released.into_inner(), vec![1, 2, 3]);
}

fn timepoint_fact(id: &str, subject_scope: serde_json::Value, timepoint: &Timepoint, baseline: bool) -> Fact {
    let tags: Vec<&str> = if baseline {
        vec![bioprism_onco::ingest::BASELINE_TAG]
    } else {
        Vec::new()
    };
    Fact::from_json(&serde_json::json!({
        "id": id,
        "provides": bioprism_onco::ingest::TIMEPOINT_VARIABLE,
        "scope": subject_scope,
        "value": serde_json::to_value(timepoint).expect("timepoints serialize"),
        "tags": tags,
    }))
    .expect("test fact matches the fiber-world schema")
}

#[test]
fn a_worldline_is_assembled_from_facts_in_biological_order() {
    let baseline = imaging_timepoint(
        "baseline",
        flat_clocks("2024-01-01T00:00:00Z"),
        vec![lesion(40.0, 30.0)],
    );
    let follow_up = imaging_timepoint(
        "week-12",
        flat_clocks("2024-03-25T00:00:00Z"),
        vec![lesion(30.0, 20.0)],
    );
    let facts = vec![
        timepoint_fact(
            "fact:week-12",
            serde_json::json!({ "subject": "S-001" }),
            &follow_up,
            false,
        ),
        timepoint_fact(
            "fact:baseline",
            serde_json::json!({ "subject": "S-001" }),
            &baseline,
            true,
        ),
    ];

    let worldline = bioprism_onco::ingest::worldline_from_facts(&facts).expect("one subject");

    assert_eq!(worldline.subject().as_str(), "S-001");
    assert_eq!(worldline.baseline().label(), "baseline");
    let labels: Vec<&str> = worldline
        .timepoints()
        .iter()
        .map(Timepoint::label)
        .collect();
    assert_eq!(labels, vec!["baseline", "week-12"]);
}

#[test]
fn a_cohort_scope_cannot_produce_a_subject_worldline() {
    let cohort = ScopeKey::new().bind(
        "subject",
        ScopeValue::OneOf(BTreeSet::from(["S-001".to_string(), "S-002".to_string()])),
    );

    let failure = bioprism_onco::ingest::subject_from_scope(&cohort)
        .expect_err("a set of subjects is a cohort, not a worldline");

    assert!(matches!(
        failure,
        OncoError::SubjectNotSingular {
            dimension: "subject",
            ..
        }
    ));
}

#[test]
fn a_fact_whose_clocks_are_disordered_is_refused_at_ingest() {
    let timepoint = imaging_timepoint(
        "baseline",
        flat_clocks("2024-01-01T00:00:00Z"),
        vec![lesion(40.0, 30.0)],
    );
    let mut payload = serde_json::to_value(&timepoint).expect("timepoints serialize");
    payload["clocks"]["visible"] = serde_json::json!("2023-01-01T00:00:00Z");
    let fact = Fact::from_json(&serde_json::json!({
        "id": "fact:broken",
        "provides": bioprism_onco::ingest::TIMEPOINT_VARIABLE,
        "scope": { "subject": "S-001" },
        "value": payload,
        "tags": [bioprism_onco::ingest::BASELINE_TAG],
    }))
    .expect("the fact itself is schema-valid");

    let failure = bioprism_onco::ingest::timepoint_from_fact(&fact)
        .expect_err("a back-dated visibility stamp must not enter a worldline");

    let OncoError::MalformedObservation { message, .. } = failure else {
        panic!("expected a malformed-observation refusal");
    };
    assert!(message.contains("clock order"), "got {message}");
}

/// Measurement ratios are compared within tolerance rather than bitwise: this workspace pins
/// serde_json without its `float_roundtrip` feature, so its fast float parser can land one unit
/// in the last place from the value that was written. That is a property of the pinned
/// dependency, not of this crate, and no decision here turns on the 16th significant digit.
#[test]
fn public_types_round_trip_through_json() {
    let mut worldline = TumourWorldline::new(
        SubjectRef::new("S-001").expect("pseudonym"),
        imaging_timepoint(
            "baseline",
            flat_clocks("2024-01-01T00:00:00Z"),
            vec![lesion(40.0, 30.0)],
        ),
    );
    worldline
        .push(
            Timepoint::new(
                "molecular",
                flat_clocks("2024-01-05T00:00:00Z"),
                Observation::Molecular(diffuse_glioma_panel()),
            )
            .expect("ordered clocks"),
        )
        .expect("distinct label");

    let encoded = serde_json::to_string(&worldline).expect("worldlines serialize");
    let decoded: TumourWorldline = serde_json::from_str(&encoded).expect("worldlines deserialize");
    assert_eq!(decoded, worldline);

    let assessment = Scenario::progressing_inside_the_window().assess();
    let encoded = serde_json::to_string(&assessment).expect("assessments serialize");
    let decoded: ResponseAssessment =
        serde_json::from_str(&encoded).expect("assessments deserialize");
    assert_eq!(decoded.criterion_id, assessment.criterion_id);
    assert_eq!(decoded.criterion_version, assessment.criterion_version);
    assert_eq!(decoded.unconfirmed_reading, assessment.unconfirmed_reading);
    assert_eq!(decoded.call, assessment.call);
    assert_eq!(decoded.hypotheses, assessment.hypotheses);
    assert_eq!(
        decoded.divergence_from_criterion,
        assessment.divergence_from_criterion
    );
    assert_eq!(
        decoded.sensitivity.flips_within_measurement_error,
        assessment.sensitivity.flips_within_measurement_error
    );
    let decoded_distance = decoded
        .sensitivity
        .distance_to_progression_threshold
        .expect("the nadir is positive");
    let original_distance = assessment
        .sensitivity
        .distance_to_progression_threshold
        .expect("the nadir is positive");
    assert!((decoded_distance - original_distance).abs() < 1e-12);

    let resolution = classify(Histology::DiffuseGlioma, &diffuse_glioma_panel());
    let encoded = serde_json::to_string(&resolution).expect("resolutions serialize");
    let decoded: DiagnosticResolution =
        serde_json::from_str(&encoded).expect("resolutions deserialize");
    assert_eq!(decoded, resolution);

    let boundary = ResearchBoundary::research_only();
    let encoded = serde_json::to_string(&boundary).expect("boundaries serialize");
    let decoded: ResearchBoundary = serde_json::from_str(&encoded).expect("boundaries deserialize");
    assert_eq!(decoded, boundary);
}

#[test]
fn a_worldline_refuses_a_duplicate_timepoint_label() {
    let mut worldline = TumourWorldline::new(
        SubjectRef::new("S-001").expect("pseudonym"),
        imaging_timepoint(
            "baseline",
            flat_clocks("2024-01-01T00:00:00Z"),
            vec![lesion(40.0, 30.0)],
        ),
    );

    let failure = worldline
        .push(imaging_timepoint(
            "baseline",
            flat_clocks("2024-02-01T00:00:00Z"),
            vec![lesion(30.0, 20.0)],
        ))
        .expect_err("labels index the worldline");

    assert_eq!(failure, OncoError::DuplicateTimepoint("baseline".into()));
}

#[test]
fn a_negative_or_non_finite_measurement_is_refused() {
    assert!(matches!(
        TargetLesion::new("bad", -1.0, 10.0),
        Err(OncoError::InvalidMeasurement {
            field: "longest_diameter_mm",
            ..
        })
    ));
    assert!(TargetLesion::new("bad", f64::NAN, 10.0).is_err());
    assert!(Karnofsky::new(85).is_err());
    assert!(Karnofsky::new(80).is_ok());
    assert!(SubjectRef::new("").is_err());
}
