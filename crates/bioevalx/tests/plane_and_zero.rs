//! The crate's spine: an unscored dimension is not a zero-scored one (26.17).

use std::fs;
use std::path::Path;

use bioprism_bioevalx::error::PlaneError;
use bioprism_bioevalx::plane::{
    CapabilityTier, Cell, Dimension, FoldPolicy, Score, ScorePlane, UnscoredReason,
};

fn three_dimensions() -> Vec<Dimension> {
    vec![
        Dimension::universal("task_success"),
        Dimension::universal("evidence_quality"),
        Dimension::requiring("action_value", CapabilityTier::ToolUsingAgent),
    ]
}

#[test]
fn an_unscored_dimension_has_no_path_to_becoming_a_zero() {
    let plane = ScorePlane::declare("agent", CapabilityTier::ToolUsingAgent, three_dimensions())
        .expect("declaration is well formed");
    let cell = plane.cell("task_success").expect("dimension was declared");

    assert!(cell.score().is_none());
    assert!(!cell.is_measured());
    let json = serde_json::to_value(cell).expect("cell serializes");
    assert!(
        json.get("score").is_none(),
        "an unscored cell must not serialize a score key, got {json}"
    );
}

#[test]
fn folding_a_plane_with_an_unscored_dimension_refuses_rather_than_imputing() {
    let mut plane =
        ScorePlane::declare("agent", CapabilityTier::ToolUsingAgent, three_dimensions())
            .expect("declaration is well formed");
    plane.score("task_success", 0.9).expect("in tier");
    plane.score("action_value", 0.5).expect("in tier");

    match plane.fold(FoldPolicy::ExcludeInapplicable) {
        Err(PlaneError::UnscoredDimensions { unscored }) => {
            assert_eq!(unscored, vec!["evidence_quality".to_string()]);
        }
        other => panic!("expected a refusal naming the unscored dimension, got {other:?}"),
    }
}

#[test]
fn a_capability_the_system_cannot_have_is_excluded_not_zeroed() {
    let mut plane =
        ScorePlane::declare("predictor", CapabilityTier::FixedInputModel, three_dimensions())
            .expect("declaration is well formed");
    plane.score("task_success", 1.0).expect("in tier");
    plane.score("evidence_quality", 1.0).expect("in tier");

    let fold = plane
        .fold(FoldPolicy::ExcludeInapplicable)
        .expect("only out-of-tier dimensions remain");

    assert_eq!(fold.value, 1.0, "an excluded dimension must not drag the fold");
    assert_eq!(fold.excluded.len(), 1);
    assert_eq!(fold.excluded[0].id, "action_value");
    assert_eq!(fold.included, vec!["task_success", "evidence_quality"]);
}

#[test]
fn scoring_a_dimension_the_system_cannot_reach_is_refused_at_the_point_of_offer() {
    let mut plane =
        ScorePlane::declare("predictor", CapabilityTier::FixedInputModel, three_dimensions())
            .expect("declaration is well formed");

    match plane.score("action_value", 0.0) {
        Err(PlaneError::OutOfTier {
            dimension,
            declared,
            required,
        }) => {
            assert_eq!(dimension, "action_value");
            assert_eq!(declared, "fixed-input predictive model");
            assert_eq!(required, "tool-using agent");
        }
        other => panic!("expected an out-of-tier refusal, got {other:?}"),
    }
}

#[test]
fn two_folds_over_different_dimension_sets_report_that_they_are_not_the_same_basis() {
    let mut agent =
        ScorePlane::declare("agent", CapabilityTier::ToolUsingAgent, three_dimensions())
            .expect("declaration is well formed");
    for dimension in ["task_success", "evidence_quality", "action_value"] {
        agent.score(dimension, 0.5).expect("in tier");
    }
    let mut predictor =
        ScorePlane::declare("predictor", CapabilityTier::FixedInputModel, three_dimensions())
            .expect("declaration is well formed");
    for dimension in ["task_success", "evidence_quality"] {
        predictor.score(dimension, 0.5).expect("in tier");
    }

    let a = agent.fold(FoldPolicy::ExcludeInapplicable).expect("complete");
    let b = predictor
        .fold(FoldPolicy::ExcludeInapplicable)
        .expect("complete");

    assert_eq!(a.value, b.value, "the numbers coincide");
    assert!(
        !a.same_basis(&b),
        "identical numbers over different denominators must not read as comparable"
    );
}

#[test]
fn a_score_outside_the_unit_interval_cannot_be_constructed_or_deserialized() {
    assert!(matches!(
        Score::new("d", 1.5),
        Err(PlaneError::ScoreOutOfRange { .. })
    ));
    assert!(matches!(
        Score::new("d", f64::NAN),
        Err(PlaneError::ScoreOutOfRange { .. })
    ));
    let round_trip: Result<Score, _> = serde_json::from_str("1.5");
    assert!(
        round_trip.is_err(),
        "deserialization must go through the same gate as construction"
    );
}

#[test]
fn a_declared_dimension_starts_unscored_rather_than_absent() {
    let plane = ScorePlane::declare("agent", CapabilityTier::ToolUsingAgent, three_dimensions())
        .expect("declaration is well formed");

    assert_eq!(
        plane.unscored(),
        vec!["action_value", "evidence_quality", "task_success"],
        "every declared dimension is a hole until measured"
    );
    assert!(plane.inapplicable().is_empty());
}

#[test]
fn recording_that_the_evaluator_broke_keeps_the_dimension_out_of_the_fold() {
    let mut plane =
        ScorePlane::declare("agent", CapabilityTier::ToolUsingAgent, three_dimensions())
            .expect("declaration is well formed");
    plane.score("task_success", 0.9).expect("in tier");
    plane.score("action_value", 0.8).expect("in tier");
    plane
        .leave_unscored(
            "evidence_quality",
            UnscoredReason::EvaluatorUnhealthy {
                evaluator: "grounding-grader".into(),
            },
        )
        .expect("dimension was declared");

    assert!(matches!(
        plane.cell("evidence_quality"),
        Some(Cell::Unscored { .. })
    ));
    assert!(plane.fold(FoldPolicy::ExcludeInapplicable).is_err());
}

#[test]
fn no_source_file_in_this_crate_imputes_a_missing_score() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut offenders = Vec::new();
    for entry in fs::read_dir(&src).expect("src is readable") {
        let path = entry.expect("directory entry").path();
        if path.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        let text = fs::read_to_string(&path).expect("source is readable");
        for (number, line) in text.lines().enumerate() {
            let code = line.trim_start();
            if code.starts_with("//") {
                continue;
            }
            for pattern in ["unwrap_or(0.", "unwrap_or_default()", "or_zero"] {
                if code.contains(pattern) {
                    offenders.push(format!("{}:{}: {pattern}", path.display(), number + 1));
                }
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "imputation of a missing score has entered the crate: {offenders:?}"
    );
}

#[test]
fn every_capability_tier_admits_exactly_the_tiers_at_or_below_it() {
    for (index, tier) in CapabilityTier::ALL.iter().enumerate() {
        for (other_index, other) in CapabilityTier::ALL.iter().enumerate() {
            assert_eq!(
                tier.admits(*other),
                other_index <= index,
                "{tier:?} vs {other:?}"
            );
        }
    }
}

