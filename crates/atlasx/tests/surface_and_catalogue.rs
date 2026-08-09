//! The cell that has no blank, and the register of metrics nothing defines.

mod common;

use bioprism_atlas::UnmeasuredReason;
use bioprism_atlasx::{
    audit, browse, catalogue, Answer, AtlasxError, DebtStatement, Facet, Question, Share,
    SurfaceCell, Unanswerable, DEFINED_HERE, METRICS_NAMED_IN_SCOPE, NAMED_NEVER_DEFINED,
};
use bioprism_hub::PublicationState;
use common::{grid, grid_with_holes};
use std::collections::BTreeSet;

#[test]
fn a_share_over_nothing_is_refused_rather_than_returning_zero() {
    assert!(matches!(
        Share::new(0, 0),
        Err(AtlasxError::EmptyDenominator { .. })
    ));
}

#[test]
fn a_share_above_one_is_refused() {
    assert!(matches!(
        Share::new(3, 2),
        Err(AtlasxError::ShareAboveOne { .. })
    ));
}

#[test]
fn a_share_carries_its_denominator_through_serialization() {
    let share = Share::new(2, 5).expect("valid share");
    let encoded = serde_json::to_string(&share).expect("serializes");
    assert!(encoded.contains("\"denominator\":5"));
    let decoded: Share = serde_json::from_str(&encoded).expect("deserializes");
    assert_eq!(decoded.numerator(), 2);
    assert_eq!(decoded.denominator(), 5);
}

#[test]
fn a_stored_share_with_a_zero_denominator_fails_to_load() {
    let value = serde_json::json!({"numerator": 0, "denominator": 0});
    assert!(serde_json::from_value::<Share>(value).is_err());
}

#[test]
fn a_hole_cell_serializes_with_no_numeric_key_at_all() {
    let cell = SurfaceCell::hole(UnmeasuredReason::NotAttempted);
    let encoded = serde_json::to_value(&cell).expect("serializes");
    assert_eq!(encoded["kind"], "hole");
    assert!(encoded.get("value").is_none());
}

#[test]
fn a_withheld_cell_serializes_with_no_numeric_key_at_all() {
    let cell = SurfaceCell::withheld(PublicationState::UnderReview);
    let encoded = serde_json::to_value(&cell).expect("serializes");
    assert_eq!(encoded["kind"], "withheld");
    assert!(encoded.get("value").is_none());
}

#[test]
fn only_the_numeric_arms_of_a_cell_yield_a_number() {
    assert_eq!(SurfaceCell::count(3).as_number(), Some(3.0));
    assert_eq!(
        SurfaceCell::share(1, 2).expect("share").as_number(),
        Some(0.5)
    );
    assert!(SurfaceCell::hole(UnmeasuredReason::NotAttempted)
        .as_number()
        .is_none());
    assert!(SurfaceCell::withheld(PublicationState::Stale)
        .as_number()
        .is_none());
}

#[test]
fn an_answer_is_either_a_cell_or_a_reason_and_never_both() {
    let answered = Answer::answered(SurfaceCell::count(1));
    let refused = Answer::refused(Unanswerable::EvidenceAbsent);
    assert!(answered.cell().is_some() && answered.refusal().is_none());
    assert!(refused.cell().is_none() && refused.refusal().is_some());
}

#[test]
fn the_three_refusals_a_reader_must_not_conflate_are_distinct_values() {
    let states = BTreeSet::from([
        Unanswerable::EvidenceAbsent,
        Unanswerable::NoAttemptDenominator,
        Unanswerable::NotDeclared,
    ]);
    assert_eq!(
        states.len(),
        3,
        "measuring, asking a different object, and asking the wrong thing have different remedies"
    );
}

#[test]
fn an_audit_records_a_refused_declared_question_without_calling_it_a_defect() {
    let browsed = browse("system-a", &[], Facet::Mechanism).expect("browses");
    let report = audit(&browsed);
    assert!(report.sound());
    assert!(report
        .declared_but_refused
        .iter()
        .any(|(question, _)| *question == Question::ModalBucket));
}

#[test]
fn an_audit_reports_the_subject_the_surface_is_a_reading_of() {
    let report = audit(&DebtStatement::of(&grid("system-a")));
    assert_eq!(report.subject, "system-a");
}

#[test]
fn an_audit_lists_every_non_numeric_cell_so_a_publication_check_can_see_them() {
    let debt = DebtStatement::of(&grid_with_holes(
        "system-x",
        &[
            ("a", UnmeasuredReason::InaccessibleByPolicy),
            ("b", UnmeasuredReason::NotAttempted),
        ],
    ));
    let report = audit(&debt);
    assert!(report.non_numeric_cells.contains(&"a".to_string()));
    assert!(report.non_numeric_cells.contains(&"b".to_string()));
}

#[test]
fn the_scope_names_one_hundred_and_eighteen_metrics() {
    assert_eq!(catalogue::named_in_scope(), METRICS_NAMED_IN_SCOPE);
    assert_eq!(NAMED_NEVER_DEFINED.len(), METRICS_NAMED_IN_SCOPE - 1);
}

#[test]
fn every_metric_in_the_register_is_undefined_here() {
    let defined: BTreeSet<&str> = DEFINED_HERE.iter().map(|entry| entry.metric).collect();
    for named in NAMED_NEVER_DEFINED {
        assert!(
            !defined.contains(named.metric),
            "{} is listed as undefined and also defined",
            named.metric
        );
    }
}

#[test]
fn exactly_one_blueprint_metric_is_given_a_denominator_here() {
    let from_blueprint: Vec<&str> = DEFINED_HERE
        .iter()
        .filter(|entry| entry.blueprint_name)
        .map(|entry| entry.metric)
        .collect();
    assert_eq!(from_blueprint, vec!["profile coverage"]);
}

#[test]
fn every_metric_this_crate_emits_states_a_numerator_and_a_denominator() {
    for entry in DEFINED_HERE {
        assert!(!entry.numerator.is_empty(), "{}", entry.metric);
        assert!(!entry.denominator.is_empty(), "{}", entry.metric);
        assert!(!entry.refuses.is_empty(), "{}", entry.metric);
    }
}

#[test]
fn only_five_named_metrics_state_even_a_denominator() {
    let with_denominator = NAMED_NEVER_DEFINED
        .iter()
        .filter(|metric| metric.denominator.is_some())
        .count();
    assert_eq!(
        with_denominator, 5,
        "the rest are bare noun phrases with no formula, estimator or denominator"
    );
}

#[test]
fn the_register_covers_twenty_two_modules_ten_of_metrics_and_twelve_of_hub() {
    let mut metrics_modules = BTreeSet::new();
    let mut hub_modules = BTreeSet::new();
    for named in NAMED_NEVER_DEFINED {
        match named.origin {
            catalogue::Origin::CapabilityMetrics => metrics_modules.insert(named.module_title),
            catalogue::Origin::PublicHub => hub_modules.insert(named.module_title),
        };
    }
    assert_eq!(metrics_modules.len(), 10);
    assert_eq!(hub_modules.len(), 12);
}

#[test]
fn no_module_and_metric_pair_appears_twice_in_the_register() {
    let mut seen = BTreeSet::new();
    for named in NAMED_NEVER_DEFINED {
        assert!(
            seen.insert((named.module_title, named.metric)),
            "{} / {} listed twice",
            named.module_title,
            named.metric
        );
    }
}

#[test]
fn the_register_can_be_read_one_module_at_a_time() {
    let spine = catalogue::by_module("Translation Spine and Evidence Maturity Metrics");
    assert_eq!(spine.len(), 5);
    assert!(spine.iter().any(|m| m.metric == "weakest-link maturity"));
}

#[test]
fn the_register_names_no_blueprint_module_ids() {
    let mut text = String::new();
    for named in NAMED_NEVER_DEFINED {
        text.push_str(named.module_title);
        text.push(' ');
        text.push_str(named.metric);
        text.push('\n');
    }
    let bytes: Vec<char> = text.chars().collect();
    for window in bytes.windows(5) {
        let looks_like_id = window[0].is_ascii_digit()
            && window[1].is_ascii_digit()
            && window[2] == '.'
            && window[3].is_ascii_digit()
            && window[4].is_ascii_digit();
        assert!(
            !looks_like_id,
            "the register keys modules by title so it cannot move a coverage percentage"
        );
    }
}
