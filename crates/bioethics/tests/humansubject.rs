//! 36.22: a determination that can be raised and never lowered.

use bioprism_bioethics::humansubject::{
    screen, Determination, EngagementKind, InstitutionalDetermination,
    InstitutionalDeterminationDocument, RecordedOutcome, ReturnOfResults, StudyDescription,
    UndeterminedReason,
};
use bioprism_bioethics::BioethicsError;
use bioprism_onco::{OncoError, ResearchBoundary};
use bioprism_policy::{Consent, Purpose, PurposeSet};
use bioprism_scope::Timestamp;

fn at(text: &str) -> Timestamp {
    Timestamp::parse(text).expect("fixture timestamp parses")
}

fn research_study() -> StudyDescription {
    StudyDescription::new(
        "reader-agreement-study",
        PurposeSet::of([Purpose::ResearchAnalysis]),
    )
    .engaging(EngagementKind::ExpertPerformanceStudy)
}

#[test]
fn screening_can_require_review_and_has_no_way_to_grant_an_exemption() {
    let determination = screen(&research_study());
    assert!(determination.requires_review());
    assert_eq!(
        determination.triggers(),
        &[EngagementKind::ExpertPerformanceStudy]
    );
    assert!(
        matches!(determination, Determination::ReviewRequired { .. }),
        "the only other variant is Undetermined; there is no exemption to reach"
    );
}

#[test]
fn a_study_declaring_no_engagement_is_undetermined_rather_than_cleared() {
    let study = StudyDescription::new("undescribed", PurposeSet::of([Purpose::MethodDevelopment]));
    let determination = screen(&study);
    assert!(!determination.requires_review());
    assert!(determination.triggers().is_empty());
    assert!(matches!(
        determination,
        Determination::Undetermined {
            reason: UndeterminedReason::NoEngagementWasDeclared
        }
    ));
}

#[test]
fn every_declared_engagement_becomes_a_trigger_because_the_module_grades_none_of_them() {
    let mut study =
        StudyDescription::new("everything", PurposeSet::of([Purpose::ResearchAnalysis]));
    for engagement in EngagementKind::ALL {
        study = study.engaging(engagement);
    }
    let determination = screen(&study);
    assert_eq!(determination.triggers().len(), EngagementKind::ALL.len());
}

#[test]
fn an_exemption_exists_only_as_a_transcription_of_a_named_external_body() {
    let recorded = InstitutionalDetermination::record(
        "reader-agreement-study",
        "an institutional review board",
        "a determination reference",
        RecordedOutcome::DeterminedNotHumanSubjectResearch,
    )
    .expect("a body and a reference were supplied");
    assert_eq!(
        recorded.outcome(),
        RecordedOutcome::DeterminedNotHumanSubjectResearch
    );
    assert_eq!(recorded.body(), "an institutional review board");

    assert!(
        screen(&research_study()).requires_review(),
        "the transcription does not change what screening concludes; only a body can do that"
    );
}

#[test]
fn a_determination_with_no_body_or_no_reference_is_refused() {
    let no_body = InstitutionalDetermination::record(
        "reader-agreement-study",
        "   ",
        "a determination reference",
        RecordedOutcome::Approved,
    )
    .expect_err("an unattributed determination determines nothing");
    match no_body {
        BioethicsError::IncompleteInstitutionalDetermination { field, .. } => {
            assert_eq!(field, "body");
        }
        other => panic!("expected the empty field to be named: {other}"),
    }

    let no_reference = InstitutionalDetermination::record(
        "reader-agreement-study",
        "an institutional review board",
        "",
        RecordedOutcome::WaiverGranted,
    )
    .expect_err("a determination nobody can look up is not a determination");
    match no_reference {
        BioethicsError::IncompleteInstitutionalDetermination { field, .. } => {
            assert_eq!(field, "reference");
        }
        other => panic!("expected the empty field to be named: {other}"),
    }
}

#[test]
fn a_determination_document_with_a_blank_field_does_not_decode() {
    let document = InstitutionalDeterminationDocument {
        study: "reader-agreement-study".to_string(),
        body: String::new(),
        reference: "a determination reference".to_string(),
        outcome: RecordedOutcome::Approved,
    };
    let encoded = serde_json::to_string(&document).expect("serialisable");
    assert!(
        serde_json::from_str::<InstitutionalDetermination>(&encoded).is_err(),
        "the decode runs the same emptiness checks the constructor does"
    );
}

#[test]
fn a_purpose_outside_consent_carries_the_policy_crates_own_refusal() {
    let study = StudyDescription::new(
        "training-run",
        PurposeSet::of([Purpose::ResearchAnalysis, Purpose::ModelTraining]),
    );
    let consent = Consent::new("consent-a", PurposeSet::of([Purpose::ResearchAnalysis]));
    let error = study
        .check_consent(&consent, at("2026-01-01T00:00:00Z"))
        .expect_err("the participants never agreed to model training");
    match error {
        BioethicsError::PurposeOutsideConsent {
            purpose, refusal, ..
        } => {
            assert_eq!(purpose, "model_training");
            assert!(
                refusal.contains("consent-a"),
                "the refusal must be policy's sentence, naming its own consent: {refusal}"
            );
        }
        other => panic!("expected a consent refusal: {other}"),
    }
}

#[test]
fn a_study_whose_purposes_are_all_consented_passes() {
    let study = research_study();
    let consent = Consent::new(
        "consent-a",
        PurposeSet::of([Purpose::ResearchAnalysis, Purpose::QualityAssurance]),
    );
    assert!(study
        .check_consent(&consent, at("2026-01-01T00:00:00Z"))
        .is_ok());
}

#[test]
fn returning_individual_findings_meets_the_research_boundary() {
    let study = research_study().returning(ReturnOfResults::IndividualFindings);
    let error = study
        .check_return_of_results(&ResearchBoundary::research_only())
        .expect_err("this platform is not the thing that produces a person-level finding");
    assert!(
        matches!(
            error,
            BioethicsError::Onco(OncoError::OutsideResearchBoundary { .. })
        ),
        "the refusal must be bioprism-onco's own: {error}"
    );
}

#[test]
fn returning_nothing_or_returning_aggregate_findings_is_not_this_crates_question() {
    let boundary = ResearchBoundary::research_only();
    assert!(research_study()
        .returning(ReturnOfResults::NotReturned)
        .check_return_of_results(&boundary)
        .is_ok());
    assert!(research_study()
        .returning(ReturnOfResults::AggregateToParticipants)
        .check_return_of_results(&boundary)
        .is_ok());
}
