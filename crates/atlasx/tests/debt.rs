//! Coverage debt is derived from a grid, never asserted — blueprint 34.08.

mod common;

use bioprism_atlas::UnmeasuredReason;
use bioprism_atlasx::{
    audit, Answer, AtlasxError, DebtStatement, Question, Surface, SurfaceCell, Unanswerable,
};
use bioprism_hub::PublicationState;
use common::{capability, grid, grid_with_holes};

#[test]
fn a_debt_names_every_hole_rather_than_counting_them() {
    let debt = DebtStatement::of(&grid("system-a"));
    assert_eq!(debt.unmeasured(), 1);
    assert_eq!(debt.holes().len(), 1);
    assert_eq!(
        debt.holes()[0].capability.as_str(),
        "causal.interpretation",
        "a debt that is only a count cannot be reproduced or discharged"
    );
}

#[test]
fn a_debt_takes_its_subject_from_the_grid_label_not_from_the_caller() {
    let debt = DebtStatement::of(&grid("system-a"));
    assert_eq!(debt.subject(), "system-a");
}

#[test]
fn a_restricted_reading_produces_a_debt_about_a_different_subject() {
    let whole = grid("system-a");
    let restricted = whole.restricted_to("system-a/pack-3", &[capability("identity.lineage")]);
    let debt_whole = DebtStatement::of(&whole);
    let debt_part = DebtStatement::of(&restricted);
    assert_ne!(debt_whole.subject(), debt_part.subject());
    assert!(matches!(
        debt_whole.discharged_by(&debt_part),
        Err(AtlasxError::DebtSubjectsDiffer { .. })
    ));
}

#[test]
fn profile_coverage_over_an_empty_grid_refuses_instead_of_returning_zero() {
    let empty = grid_with_holes("system-empty", &[]);
    let debt = DebtStatement::of(&empty);
    assert!(debt.is_vacuous());
    assert_eq!(
        debt.profile_coverage(),
        Answer::refused(Unanswerable::EvidenceAbsent),
        "zero of zero is the absence of a measurement, not zero percent"
    );
}

#[test]
fn profile_coverage_carries_its_own_denominator() {
    let debt = DebtStatement::of(&grid("system-a"));
    let coverage = debt.profile_coverage();
    let Some(SurfaceCell::Share { value }) = coverage.cell() else {
        panic!("expected a share");
    };
    assert_eq!(value.numerator(), 2);
    assert_eq!(value.denominator(), 3);
}

#[test]
fn a_hole_closed_by_declared_use_does_not_block_a_claim_and_others_do() {
    let debt = DebtStatement::of(&grid_with_holes(
        "system-b",
        &[
            ("a", UnmeasuredReason::OutOfScopeByDeclaredUse),
            ("b", UnmeasuredReason::NotAttempted),
        ],
    ));
    assert_eq!(debt.closed_by_declaration(), 1);
    assert_eq!(debt.blocking().len(), 1);
    assert_eq!(debt.blocking()[0].capability.as_str(), "b");
}

#[test]
fn a_policy_inaccessible_hole_renders_as_controlled_not_as_a_generic_hole() {
    let debt = DebtStatement::of(&grid_with_holes(
        "system-c",
        &[("a", UnmeasuredReason::InaccessibleByPolicy)],
    ));
    assert_eq!(
        debt.holes()[0].cell(),
        SurfaceCell::withheld(PublicationState::Controlled)
    );
}

#[test]
fn no_hole_ever_renders_as_a_number() {
    let debt = DebtStatement::of(&grid_with_holes(
        "system-d",
        &[
            ("a", UnmeasuredReason::NotAttempted),
            ("b", UnmeasuredReason::InaccessibleByPolicy),
            ("c", UnmeasuredReason::DeferredAcquisition),
            ("d", UnmeasuredReason::AllTrialsAbstained),
        ],
    ));
    for hole in debt.holes() {
        assert!(
            hole.cell().as_number().is_none(),
            "hole for {} produced a number",
            hole.capability
        );
    }
}

#[test]
fn a_stored_debt_whose_summary_disagrees_with_its_holes_fails_to_load() {
    let debt = DebtStatement::of(&grid_with_holes(
        "system-e",
        &[("a", UnmeasuredReason::NotAttempted)],
    ));
    let mut value: serde_json::Value =
        serde_json::to_value(&debt).expect("a debt statement serializes");
    value["closed_by_declaration"] = serde_json::json!(1);
    let reloaded = serde_json::from_value::<DebtStatement>(value);
    assert!(
        reloaded.is_err(),
        "a hand-edited summary must not load with a friendlier number"
    );
}

#[test]
fn a_stored_debt_naming_more_holes_than_capabilities_fails_to_load() {
    let value = serde_json::json!({
        "subject": "forged",
        "total_capabilities": 1,
        "holes": [
            {"capability": "a", "reason": "not_attempted"},
            {"capability": "b", "reason": "not_attempted"}
        ],
        "closed_by_declaration": 0
    });
    assert!(serde_json::from_value::<DebtStatement>(value).is_err());
}

#[test]
fn a_stored_debt_naming_one_capability_twice_fails_to_load() {
    let value = serde_json::json!({
        "subject": "forged",
        "total_capabilities": 4,
        "holes": [
            {"capability": "a", "reason": "not_attempted"},
            {"capability": "a", "reason": "not_attempted"}
        ],
        "closed_by_declaration": 0
    });
    assert!(serde_json::from_value::<DebtStatement>(value).is_err());
}

#[test]
fn a_debt_round_trips_through_json() {
    let debt = DebtStatement::of(&grid("system-a"));
    let encoded = serde_json::to_string(&debt).expect("serializes");
    let decoded: DebtStatement = serde_json::from_str(&encoded).expect("deserializes");
    assert_eq!(debt, decoded);
}

#[test]
fn measuring_a_hole_and_declaring_it_away_are_recorded_separately() {
    let before = DebtStatement::of(&grid_with_holes(
        "system-f",
        &[
            ("measured-later", UnmeasuredReason::NotAttempted),
            ("declared-away", UnmeasuredReason::NotAttempted),
            ("still-open", UnmeasuredReason::NotAttempted),
        ],
    ));
    let after = DebtStatement::of(&grid_with_holes(
        "system-f",
        &[
            ("declared-away", UnmeasuredReason::OutOfScopeByDeclaredUse),
            ("still-open", UnmeasuredReason::NotAttempted),
        ],
    ));
    let discharge = before.discharged_by(&after).expect("same subject");
    assert_eq!(discharge.measured, vec!["measured-later".to_string()]);
    assert_eq!(discharge.declared_away, vec!["declared-away".to_string()]);
    assert_eq!(discharge.persisting, vec!["still-open".to_string()]);
    assert!(discharge.newly_unmeasured.is_empty());
}

#[test]
fn a_discharge_of_declarations_only_reports_no_evidence() {
    let before = DebtStatement::of(&grid_with_holes(
        "system-g",
        &[("a", UnmeasuredReason::NotAttempted)],
    ));
    let after = DebtStatement::of(&grid_with_holes(
        "system-g",
        &[("a", UnmeasuredReason::OutOfScopeByDeclaredUse)],
    ));
    let discharge = before.discharged_by(&after).expect("same subject");
    assert!(!discharge.any_evidence());
}

#[test]
fn a_capability_the_earlier_reading_never_had_is_not_counted_against_it() {
    let before = DebtStatement::of(&grid_with_holes(
        "system-h",
        &[("a", UnmeasuredReason::NotAttempted)],
    ));
    let after = DebtStatement::of(&grid_with_holes(
        "system-h",
        &[
            ("a", UnmeasuredReason::NotAttempted),
            ("b", UnmeasuredReason::NotAttempted),
        ],
    ));
    let discharge = before.discharged_by(&after).expect("same subject");
    assert_eq!(discharge.newly_unmeasured, vec!["b".to_string()]);
    assert!(discharge.measured.is_empty());
}

#[test]
fn a_debt_statement_refuses_every_question_about_how_the_subject_performed() {
    let debt = DebtStatement::of(&grid("system-a"));
    for question in [
        Question::CapabilityStanding {
            capability: "identity.lineage".to_string(),
        },
        Question::RatePerAttempt {
            capability: "identity.lineage".to_string(),
        },
        Question::ComparisonWith {
            subject: "system-b".to_string(),
        },
        Question::ModalBucket,
    ] {
        assert_eq!(
            debt.answer(&question),
            Answer::refused(Unanswerable::NotDeclared),
            "a record of what was measured cannot say how it went"
        );
    }
}

#[test]
fn a_debt_statement_answers_only_what_it_declares() {
    let report = audit(&DebtStatement::of(&grid("system-a")));
    assert!(report.sound(), "{:?}", report.undeclared_but_answered);
}

#[test]
fn a_vacuous_debt_renders_coverage_as_a_hole_rather_than_omitting_the_cell() {
    let debt = DebtStatement::of(&grid_with_holes("system-empty", &[]));
    let report = audit(&debt);
    assert!(report
        .non_numeric_cells
        .contains(&"profile_coverage".to_string()));
}
