//! The crate's spine: an unscored dimension is not a zero-scored one (26.17).

use std::fs;
use std::path::{Path, PathBuf};

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
    let mut plane = ScorePlane::declare(
        "predictor",
        CapabilityTier::FixedInputModel,
        three_dimensions(),
    )
    .expect("declaration is well formed");
    plane.score("task_success", 1.0).expect("in tier");
    plane.score("evidence_quality", 1.0).expect("in tier");

    let fold = plane
        .fold(FoldPolicy::ExcludeInapplicable)
        .expect("only out-of-tier dimensions remain");

    assert_eq!(
        fold.value, 1.0,
        "an excluded dimension must not drag the fold"
    );
    assert_eq!(fold.excluded.len(), 1);
    assert_eq!(fold.excluded[0].id, "action_value");
    assert_eq!(fold.included, vec!["task_success", "evidence_quality"]);
}

#[test]
fn scoring_a_dimension_the_system_cannot_reach_is_refused_at_the_point_of_offer() {
    let mut plane = ScorePlane::declare(
        "predictor",
        CapabilityTier::FixedInputModel,
        three_dimensions(),
    )
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
    let mut predictor = ScorePlane::declare(
        "predictor",
        CapabilityTier::FixedInputModel,
        three_dimensions(),
    )
    .expect("declaration is well formed");
    for dimension in ["task_success", "evidence_quality"] {
        predictor.score(dimension, 0.5).expect("in tier");
    }

    let a = agent
        .fold(FoldPolicy::ExcludeInapplicable)
        .expect("complete");
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
fn a_persisted_plane_cannot_forge_its_cell_map_or_tier_state() {
    let plane = ScorePlane::declare("predictor", CapabilityTier::FixedInputModel, three_dimensions())
        .expect("declaration is well formed");
    let mut missing_cell = serde_json::to_value(&plane).expect("plane serializes");
    missing_cell["cells"]
        .as_object_mut()
        .expect("cells object")
        .remove("task_success");
    let parsed_missing: Result<ScorePlane, _> = serde_json::from_value(missing_cell);
    assert!(parsed_missing.is_err());

    let mut forged_state = serde_json::to_value(&plane).expect("plane serializes");
    forged_state["cells"]["action_value"] =
        serde_json::json!({"state": "scored", "score": 0.0});
    let parsed_forged: Result<ScorePlane, _> = serde_json::from_value(forged_state);
    assert!(parsed_forged.is_err());
}

#[test]
fn persisted_unscored_reasons_are_validated_with_the_plane() {
    let plane = ScorePlane::declare("agent", CapabilityTier::ToolUsingAgent, three_dimensions())
        .expect("declaration is well formed");
    let mut encoded = serde_json::to_value(&plane).expect("plane serializes");
    encoded["cells"]["evidence_quality"] = serde_json::json!({
        "state": "unscored",
        "reason": "evaluator_unhealthy",
        "evaluator": " "
    });

    let parsed: Result<ScorePlane, _> = serde_json::from_value(encoded);
    assert!(parsed.is_err());
}

#[test]
fn finite_but_overflowing_weights_cannot_produce_a_fake_fold() {
    let mut plane = ScorePlane::declare(
        "agent",
        CapabilityTier::ToolUsingAgent,
        vec![
            Dimension::universal("a").weighing(f64::MAX),
            Dimension::universal("b").weighing(f64::MAX),
        ],
    )
    .expect("individual weights are finite");
    plane.score("a", 1.0).expect("in range");
    plane.score("b", 1.0).expect("in range");

    assert!(matches!(
        plane.fold(FoldPolicy::ExcludeInapplicable),
        Err(PlaneError::FoldOverflow)
    ));
}

#[test]
fn invalid_plane_and_dimension_identity_is_rejected_at_declaration() {
    assert!(matches!(
        ScorePlane::declare(" agent", CapabilityTier::ToolUsingAgent, vec![]),
        Err(PlaneError::InvalidSystem(_))
    ));
    assert!(matches!(
        ScorePlane::declare(
            "agent",
            CapabilityTier::ToolUsingAgent,
            vec![Dimension::universal("evidence\n")]
        ),
        Err(PlaneError::InvalidDimension { .. })
    ));
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

/// The three ways a missing score has historically become a zero, as text.
///
/// "Unmeasured is not zero" is a rule about code that no type in this crate can carry, because the
/// offending expression never constructs a [`Score`] — it produces an `f64` that a later step then
/// treats as one. So it is checked over the source, which makes the check only as good as its
/// ability to fire. See `the_imputation_scanner_sees_a_planted_violation`.
fn imputations(file: &str, text: &str) -> Vec<String> {
    let mut offenders = Vec::new();
    for (number, line) in text.lines().enumerate() {
        let code = line.trim_start();
        if code.starts_with("//") {
            continue;
        }
        for pattern in ["unwrap_or(0.", "unwrap_or_default()", "or_zero"] {
            if code.contains(pattern) {
                offenders.push(format!("{file}:{}: {pattern}", number + 1));
            }
        }
    }
    offenders
}

/// Every `.rs` file under `src`, at any depth.
///
/// `read_dir` alone was what this used, and it reads one level. The crate is flat today, so the two
/// agree today; the day someone adds `src/plane/` the flat version starts reporting a clean bill of
/// health for files it never opened, and nothing announces that it has stopped checking anything.
fn source_files() -> Vec<PathBuf> {
    let mut found = Vec::new();
    collect_rust_files(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
        &mut found,
    );
    found.sort();
    found
}

fn collect_rust_files(directory: &Path, found: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(directory).expect("directory is readable") {
        let path = entry.expect("directory entry").path();
        if path.is_dir() {
            collect_rust_files(&path, found);
            continue;
        }
        if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            found.push(path);
        }
    }
}

#[test]
fn the_imputation_scanner_sees_a_planted_violation() {
    // A scanner that detects nothing is worse than no scanner: it certifies the crate clean
    // forever, including after the rule stops being true.
    assert_eq!(
        imputations(
            "plane.rs",
            "        let value = cell.score().unwrap_or(0.0);"
        )
        .len(),
        1
    );
    assert_eq!(
        imputations(
            "fold.rs",
            "    let total: f64 = measured.unwrap_or_default();"
        )
        .len(),
        1
    );
    assert_eq!(
        imputations("plane.rs", "    fn score_or_zero(&self) -> f64 {").len(),
        1
    );
    assert!(
        imputations("plane.rs", "    let name = label.unwrap_or(\"unnamed\");").is_empty(),
        "a defaulted string is not an imputed score"
    );
    assert!(
        imputations("plane.rs", "// never unwrap_or_default() a measurement").is_empty(),
        "prose about the rule is not a breach of it"
    );
}

#[test]
fn no_source_file_in_this_crate_imputes_a_missing_score() {
    let files = source_files();
    assert!(
        files.len() > 1,
        "the walk found {} source files; an empty walk and a clean crate look identical from here",
        files.len()
    );
    let offenders: Vec<String> = files
        .iter()
        .flat_map(|path| {
            let text = fs::read_to_string(path).expect("source is readable");
            imputations(&path.display().to_string(), &text)
        })
        .collect();
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
