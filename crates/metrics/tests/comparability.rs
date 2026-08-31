//! Two scores measured under different conditions are not comparable, and silence is not a match.

mod common;

use bioprism_atlas::OracleTier;
use bioprism_metrics::{
    comparable, comparable_under, ComparabilityPolicy, ComparisonReport, CoveredAggregate,
    Direction, ScoreIncomparability, ScoringRule, Stratum, Subject, UnrecordedSide, CHECK_ORDER,
};
use common::{grid_of, point_cell, recorded, unrecorded};

#[test]
fn two_unrecorded_pack_versions_do_not_match_each_other() {
    let left = unrecorded("system-a");
    let right = unrecorded("system-a");

    match comparable(&left, &right) {
        Err(ScoreIncomparability::ConditionUnrecorded { dimension, side }) => {
            assert_eq!(dimension, "ontology version");
            assert_eq!(side, UnrecordedSide::Both);
        }
        other => panic!("silence must not match silence, got {other:?}"),
    }
}

#[test]
fn a_recorded_dimension_against_an_unrecorded_one_names_the_silent_side() {
    let left = unrecorded("system-a").with_ontology_version("test-ontology/1");
    let right = unrecorded("system-a");

    match comparable(&left, &right) {
        Err(ScoreIncomparability::ConditionUnrecorded { dimension, side }) => {
            assert_eq!(dimension, "ontology version");
            assert_eq!(side, UnrecordedSide::Right);
        }
        other => panic!("expected the right side to be named, got {other:?}"),
    }
}

#[test]
fn identical_fully_recorded_conditions_are_comparable() {
    assert!(comparable(&recorded("system-a"), &recorded("system-a")).is_ok());
}

#[test]
fn malformed_condition_metadata_blocks_comparison_before_matching() {
    let mut left = recorded("system-a");
    left.scoring_rule.name = "  ".into();
    match comparable(&left, &recorded("system-a")) {
        Err(ScoreIncomparability::MalformedConditions { side, .. }) => {
            assert_eq!(side, "left")
        }
        other => panic!("malformed scoring metadata must block, got {other:?}"),
    }

    let mut right = recorded("system-a");
    right.budget = bioprism_metrics::Condition::recorded(bioprism_metrics::Budget::labelled(" "));
    assert!(matches!(
        comparable(&recorded("system-a"), &right),
        Err(ScoreIncomparability::MalformedConditions { side, .. }) if side == "right"
    ));
}

#[test]
fn different_subjects_block_even_when_every_other_condition_matches() {
    let left = recorded("system-a");
    let mut right = recorded("system-a");
    right.subject = Subject::grid("system-b");

    match comparable(&left, &right) {
        Err(ScoreIncomparability::DifferentSubject { .. }) => {}
        other => panic!("expected a subject block, got {other:?}"),
    }
}

#[test]
fn a_disagreeing_scoring_rule_blocks_before_any_condition_is_examined() {
    let left = recorded("system-a");
    let mut right = recorded("system-a").with_pack_version("pack/9");
    right.scoring_rule = ScoringRule::new("brier score", Direction::LowerIsBetter, "squared error");

    match comparable(&left, &right) {
        Err(ScoreIncomparability::DifferentScoringRule { .. }) => {}
        other => panic!("the scoring rule is checked before the pack version, got {other:?}"),
    }
}

#[test]
fn the_first_blocking_dimension_follows_the_declared_check_order() {
    let left = recorded("system-a");
    let right = recorded("system-a")
        .with_ontology_version("test-ontology/2")
        .with_pack_version("pack/9")
        .with_evidence_base("something else");

    let reason = comparable(&left, &right).expect_err("three dimensions disagree");
    assert_eq!(reason.dimension(), "ontology version");
    let position = CHECK_ORDER
        .iter()
        .position(|d| *d == "ontology version")
        .expect("declared in CHECK_ORDER");
    assert!(
        position
            < CHECK_ORDER
                .iter()
                .position(|d| *d == "pack version")
                .unwrap()
    );
}

#[test]
fn a_different_oracle_floor_blocks_because_a_model_judge_is_not_a_deterministic_oracle() {
    let left = recorded("system-a");
    let right = recorded("system-a").with_oracle_floor(OracleTier::ModelJudge);

    match comparable(&left, &right) {
        Err(ScoreIncomparability::DifferentOracleFloor { .. }) => {}
        other => panic!("expected an oracle block, got {other:?}"),
    }
}

#[test]
fn a_stratum_dimension_present_on_one_side_only_blocks_rather_than_matching_a_wildcard() {
    let left = recorded("system-a");
    let right = recorded("system-a").with_stratum(Stratum::new().with("system version", "1.0.0"));

    let reason = comparable(&left, &right).expect_err("the left stratum has more coordinates");
    assert!(matches!(
        reason,
        ScoreIncomparability::ConditionUnrecorded { .. }
    ));
    assert!(reason.is_absence());
}

#[test]
fn a_disagreeing_stratum_value_names_the_dimension_that_disagrees() {
    let left = recorded("system-a");
    let mut stratum_right = recorded("system-a");
    stratum_right.stratum = Stratum::new()
        .with("system version", "1.0.0")
        .with("architecture version", "a1")
        .with("model version", "m1")
        .with("parent world", "w1")
        .with("decision family", "prognosis")
        .with("biological scale", "tissue")
        .with("modality", "imaging")
        .with("disease entity", "glioma")
        .with("site/platform", "site-b")
        .with("population/time stratum", "2026-h1")
        .with("mutation family", "paraphrase");

    match comparable(&left, &stratum_right) {
        Err(ScoreIncomparability::DifferentStratum { dimension, .. }) => {
            assert_eq!(dimension, "site/platform");
        }
        other => panic!("expected the site coordinate to be named, got {other:?}"),
    }
}

#[test]
fn a_waiver_unblocks_exactly_the_dimension_it_names_and_nothing_else() {
    let left = recorded("system-a");
    let right = recorded("system-a")
        .with_pack_version("pack/9")
        .with_evidence_base("public-observed/2026-06");

    let waiving_pack = ComparabilityPolicy::strict().waiving("pack version");
    let reason = comparable_under(&left, &right, &waiving_pack)
        .expect_err("the evidence base still disagrees");
    assert_eq!(reason.dimension(), "evidence base");

    let waiving_both = waiving_pack.waiving("evidence base");
    assert!(comparable_under(&left, &right, &waiving_both).is_ok());
}

#[test]
fn a_waiver_appears_in_the_report_so_the_assumption_is_printed_rather_than_assumed() {
    let left_grid = grid_of(
        "system-a",
        recorded("system-a"),
        vec![("verify.oracle", point_cell(0.9, 12))],
    );
    let right_grid = grid_of(
        "system-a",
        recorded("system-a").with_pack_version("pack/9"),
        vec![("verify.oracle", point_cell(0.8, 12))],
    );
    let left = CoveredAggregate::mean(&left_grid).expect("measured");
    let right = CoveredAggregate::mean(&right_grid).expect("measured");

    let policy = ComparabilityPolicy::strict().waiving("pack version");
    let report = ComparisonReport::of_aggregates(&left, &right, &policy);
    assert!(report.verdict.is_comparable());
    assert_eq!(report.waived, vec!["pack version".to_string()]);
    assert!(!policy.is_strict());
}

#[test]
fn a_report_caveats_an_aggregate_that_does_not_cover_its_own_grid() {
    let complete = grid_of(
        "system-a",
        recorded("system-a"),
        vec![("verify.oracle", point_cell(0.9, 12))],
    );
    let partial = grid_of(
        "system-a",
        recorded("system-a"),
        vec![
            ("verify.oracle", point_cell(0.8, 12)),
            (
                "safety.escalation",
                bioprism_metrics::GridCell::unmeasured(
                    bioprism_atlas::UnmeasuredReason::NotAttempted,
                ),
            ),
        ],
    );
    let left = CoveredAggregate::mean(&complete).expect("measured");
    let right = CoveredAggregate::mean(&partial).expect("one cell measured");

    let report = ComparisonReport::of_aggregates(&left, &right, &ComparabilityPolicy::strict());
    assert!(report.verdict.is_comparable());
    assert!(report
        .caveats
        .iter()
        .any(|caveat| caveat.starts_with("right aggregate covers")));
}

#[test]
fn a_report_caveats_two_aggregates_built_under_different_rules() {
    let grid = grid_of(
        "system-a",
        recorded("system-a"),
        vec![
            ("verify.oracle", point_cell(0.9, 12)),
            ("memory.recall", point_cell(0.3, 12)),
        ],
    );
    let mean = CoveredAggregate::mean(&grid).expect("measured");
    let worst = CoveredAggregate::worst(&grid).expect("measured");

    let report = ComparisonReport::of_aggregates(&mean, &worst, &ComparabilityPolicy::strict());
    assert!(report
        .caveats
        .iter()
        .any(|caveat| caveat.contains("aggregation rules differ")));
}

#[test]
fn a_blocked_verdict_distinguishes_an_absence_from_a_disagreement() {
    let grid_left = grid_of(
        "system-a",
        unrecorded("system-a"),
        vec![("verify.oracle", point_cell(0.9, 12))],
    );
    let grid_right = grid_of(
        "system-a",
        unrecorded("system-a"),
        vec![("verify.oracle", point_cell(0.8, 12))],
    );
    let left = CoveredAggregate::mean(&grid_left).expect("measured");
    let right = CoveredAggregate::mean(&grid_right).expect("measured");

    let report = ComparisonReport::of_aggregates(&left, &right, &ComparabilityPolicy::strict());
    assert!(!report.verdict.is_comparable());
    assert!(report.verdict.blocked_by_absence());
}

#[test]
fn a_comparison_report_discloses_missing_conditions_on_both_sides() {
    let left_grid = grid_of(
        "system-a",
        unrecorded("system-a"),
        vec![("verify.oracle", point_cell(0.9, 12))],
    );
    let right_grid = grid_of(
        "system-a",
        unrecorded("system-a"),
        vec![("verify.oracle", point_cell(0.8, 12))],
    );
    let left = CoveredAggregate::mean(&left_grid).expect("measured");
    let right = CoveredAggregate::mean(&right_grid).expect("measured");

    let report = ComparisonReport::of_aggregates(&left, &right, &ComparabilityPolicy::strict());
    assert!(report
        .caveats
        .iter()
        .any(|caveat| caveat.starts_with("left conditions leave")));
    assert!(report
        .caveats
        .iter()
        .any(|caveat| caveat.starts_with("right conditions leave")));
}

#[test]
fn a_comparison_report_digest_is_stable_and_changes_with_the_verdict() {
    let grid_left = grid_of(
        "system-a",
        recorded("system-a"),
        vec![("verify.oracle", point_cell(0.9, 12))],
    );
    let grid_right = grid_of(
        "system-a",
        recorded("system-a"),
        vec![("verify.oracle", point_cell(0.8, 12))],
    );
    let left = CoveredAggregate::mean(&grid_left).expect("measured");
    let right = CoveredAggregate::mean(&grid_right).expect("measured");

    let strict = ComparisonReport::of_aggregates(&left, &right, &ComparabilityPolicy::strict());
    let again = ComparisonReport::of_aggregates(&left, &right, &ComparabilityPolicy::strict());
    assert_eq!(
        strict.digest().expect("hashable"),
        again.digest().expect("hashable")
    );

    let waived = ComparisonReport::of_aggregates(
        &left,
        &right,
        &ComparabilityPolicy::strict().waiving("pack version"),
    );
    assert_ne!(
        strict.digest().expect("hashable"),
        waived.digest().expect("hashable")
    );
}

#[test]
fn unrecorded_coordinates_are_reported_against_the_stratification_key_of_thirty_three_oh_one() {
    let bare = unrecorded("system-a");
    let missing = bare.unrecorded_coordinates();
    assert!(missing.contains(&"pack version"));
    assert!(missing.contains(&"mutation family"));
    assert!(missing.contains(&"site/platform"));

    let full = recorded("system-a");
    assert!(full.unrecorded_coordinates().is_empty());
}
