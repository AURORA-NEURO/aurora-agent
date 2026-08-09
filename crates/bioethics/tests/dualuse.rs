//! 36.11: misuse assessment in front of section 13's release gate.

use bioprism_bioethics::dualuse::{refer, CapabilityRelease, MisuseSurface, SurfaceAssessment};
use bioprism_bioethics::BioethicsError;
use bioprism_safety::release::{
    GateDecision, Rating, RiskAssessment, RiskDimension, SensitiveCategory, WithholdScope,
};
use bioprism_safety::SafetyError;

fn fully_rated(subject: &str, high: &[RiskDimension]) -> RiskAssessment {
    let mut assessment =
        RiskAssessment::for_subject(subject).in_category(SensitiveCategory::BiologicalDesign);
    for dimension in RiskDimension::ALL {
        let rating = if high.contains(&dimension) {
            Rating::High
        } else {
            Rating::Low
        };
        assessment = assessment.rating(dimension, rating);
    }
    assessment
}

fn assessed(subject: &str) -> CapabilityRelease {
    CapabilityRelease::new(
        subject,
        SurfaceAssessment::assessed(
            "named biosafety reviewer",
            [MisuseSurface::PathogenRelevantAnalysis],
        ),
    )
}

#[test]
fn an_unassessed_task_cannot_be_referred_however_well_rated_its_risk_is() {
    let release = CapabilityRelease::new("variant-caller", SurfaceAssessment::NotAssessed);
    let risk = fully_rated("variant-caller", &[]);
    let error = refer(&release, &risk).expect_err("nobody asked the misuse question");
    assert!(matches!(
        error,
        BioethicsError::MisuseSurfacesUnassessed { .. }
    ));
}

#[test]
fn an_assessed_task_with_no_misuse_surface_is_not_an_unassessed_one() {
    let looked_and_found_none = SurfaceAssessment::assessed("named biosafety reviewer", []);
    assert!(looked_and_found_none.was_assessed());
    assert_eq!(
        looked_and_found_none
            .surfaces()
            .expect("an assessed value reports its surfaces")
            .len(),
        0
    );
    assert!(
        SurfaceAssessment::NotAssessed.surfaces().is_none(),
        "the unassessed state must not present itself as an empty set"
    );

    let release = CapabilityRelease::new("variant-caller", looked_and_found_none);
    let referral = refer(&release, &fully_rated("variant-caller", &[]))
        .expect("an assessed-and-empty finding is a finding");
    assert!(referral.surfaces().is_empty());
}

#[test]
fn the_verdict_is_section_thirteens_gate_and_not_a_second_one() {
    let release = assessed("designer");
    let blocked = fully_rated(
        "designer",
        &[
            RiskDimension::CapabilityUplift,
            RiskDimension::Actionability,
        ],
    );
    let referral = refer(&release, &blocked).expect("assessed and fully rated");
    assert!(matches!(referral.decision(), GateDecision::Blocked { .. }));

    let conditioned = fully_rated("designer", &[RiskDimension::CapabilityUplift]);
    let referral = refer(&release, &conditioned).expect("assessed and fully rated");
    assert!(matches!(
        referral.decision(),
        GateDecision::Conditioned { .. }
    ));

    let cleared = fully_rated("designer", &[]);
    let referral = refer(&release, &cleared).expect("assessed and fully rated");
    assert!(referral.decision().is_cleared());
}

#[test]
fn an_unrated_dimension_blocks_the_referral_with_safetys_own_error() {
    let release = assessed("designer");
    let partial = RiskAssessment::for_subject("designer")
        .in_category(SensitiveCategory::BiologicalDesign)
        .rating(RiskDimension::CapabilityUplift, Rating::Low);
    let error = refer(&release, &partial).expect_err("unrated is not low");
    assert!(
        matches!(
            error,
            BioethicsError::Safety(SafetyError::UnratedDimension { .. })
        ),
        "the gate's refusal must arrive unaltered: {error}"
    );
}

#[test]
fn a_risk_assessment_about_a_different_subject_is_refused() {
    let release = assessed("designer");
    let error = refer(&release, &fully_rated("some-other-thing", &[]))
        .expect_err("two subjects is one assessment too few");
    assert!(matches!(
        error,
        BioethicsError::AssessmentSubjectMismatch { .. }
    ));
}

#[test]
fn the_correspondence_between_the_two_six_item_lists_is_demanded_rather_than_invented() {
    let release = assessed("designer");
    let mut uncategorised = RiskAssessment::for_subject("designer");
    for dimension in RiskDimension::ALL {
        uncategorised = uncategorised.rating(dimension, Rating::Low);
    }
    let error = refer(&release, &uncategorised)
        .expect_err("no module of the blueprint maps misuse surfaces onto sensitive categories");
    assert!(matches!(
        error,
        BioethicsError::SensitiveCategoryUnstated { .. }
    ));
}

#[test]
fn withholding_the_existence_of_a_finding_is_refused_by_the_crate_that_owns_the_rule() {
    let release = assessed("designer");
    let referral = refer(&release, &fully_rated("designer", &[])).expect("assessed and rated");

    assert_eq!(
        referral
            .withhold("a screening gap", WithholdScope::ExploitDetail)
            .expect("withholding how is a legitimate dual-use control"),
        WithholdScope::ExploitDetail
    );

    let error = referral
        .withhold("a screening gap", WithholdScope::Existence)
        .expect_err("deleting the fact that a weakness exists is not a safety control");
    assert!(matches!(
        error,
        BioethicsError::Safety(SafetyError::SuppressionDisguisedAsSafety { .. })
    ));
}

#[test]
fn a_referral_records_who_assessed_it_and_which_surfaces_they_named() {
    let release = CapabilityRelease::new(
        "designer",
        SurfaceAssessment::assessed(
            "named biosafety reviewer",
            [
                MisuseSurface::SequenceDesign,
                MisuseSurface::ScreeningEvasion,
            ],
        ),
    );
    let referral = refer(&release, &fully_rated("designer", &[])).expect("assessed and rated");
    assert_eq!(referral.assessor(), "named biosafety reviewer");
    assert!(referral.surfaces().contains(&MisuseSurface::SequenceDesign));
    assert!(referral
        .surfaces()
        .contains(&MisuseSurface::ScreeningEvasion));
    assert_eq!(referral.category(), SensitiveCategory::BiologicalDesign);
    assert_eq!(MisuseSurface::ALL.len(), 6);
}
