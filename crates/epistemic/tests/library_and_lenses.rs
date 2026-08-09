//! Blueprint 43.31 (biological library) and 43.49 (query lenses).

use bioprism_epistemic::decision::{Belief, DecisionProblem};
use bioprism_epistemic::lens::{
    check_laws, view_receipt, Focus, IndexedLens, LawStatus, OpticKind, QueryLens,
    TransformRegistry,
};
use bioprism_epistemic::library::{
    disagreement_with_scope_registry, templates_with_undefined_inputs, NegativeResult, Observation,
    DIMENSIONS, FACTOR_TEMPLATES,
};
use bioprism_epistemic::EpistemicError;
use serde_json::json;
use std::collections::BTreeMap;

#[test]
fn the_library_names_dimensions_the_scope_registry_cannot_classify() {
    let report = disagreement_with_scope_registry();
    assert_eq!(report.entries.len(), DIMENSIONS.len());
    assert!(
        report.unclassified > 0,
        "the finding is that this library reaches past the registry's default table; if it did \
         not, the library would be adding nothing"
    );
    let names = report.unclassified_names();
    assert!(
        names.contains(&"organism") || names.contains(&"tissue") || names.contains(&"cell"),
        "expected the organism/tissue/cell axis to be outside the registry: {names:?}"
    );
}

#[test]
fn the_library_does_not_silently_reclassify_a_dimension_the_registry_already_owns() {
    let report = disagreement_with_scope_registry();
    assert_eq!(
        report.conflicts, 0,
        "a conflict here would mean this crate is asserting a different closure class for a \
         canonical dimension, which bioprism-scope forbids and this crate must not route around: \
         {:?}",
        report.entries
    );
}

#[test]
fn every_factor_template_input_is_a_dimension_the_library_defines() {
    let dangling = templates_with_undefined_inputs();
    assert!(
        dangling.is_empty(),
        "a factor signature naming a dimension the vocabulary does not define is exactly the \
         inference-from-filenames failure 43.31 exists to prevent: {dangling:?}"
    );
    assert!(FACTOR_TEMPLATES.len() >= 5);
}

#[test]
fn a_negative_below_the_detection_limit_constrains_nothing() {
    let result = NegativeResult::new("panel-v3", "sp-1", 200, 0.20, 0.05, "pipeline-1.4")
        .expect("admissible");

    let expected_fraction = 0.10;
    assert!(
        expected_fraction * result.purity < result.limit_of_detection,
        "the fixture is in the blind regime"
    );
    assert_eq!(
        result.likelihood_of_negative(expected_fraction),
        1.0,
        "a negative from an assay that could not have seen the variant is not weak evidence of \
         absence, it is no evidence of absence"
    );
    assert!(!result.is_informative_about(expected_fraction));
}

#[test]
fn a_negative_above_the_detection_limit_constrains_a_hypothesis() {
    let result = NegativeResult::new("panel-v3", "sp-1", 200, 0.90, 0.02, "pipeline-1.4")
        .expect("admissible");
    let likelihood = result.likelihood_of_negative(0.40);

    assert!(
        likelihood < 1e-6,
        "at 200x coverage and 36% effective fraction a negative is close to impossible, got \
         {likelihood}"
    );
    assert!(result.is_informative_about(0.40));
}

#[test]
fn a_negative_result_becomes_evidence_whose_likelihoods_follow_the_detection_model() {
    let result = NegativeResult::new("panel-v3", "sp-1", 100, 0.80, 0.03, "pipeline-1.4")
        .expect("admissible");
    let item = result
        .as_evidence("negative_egfr", 1.0, &[0.30, 0.0, 0.01])
        .expect("admissible item");

    assert!(item.likelihood(0) < item.likelihood(1));
    assert_eq!(
        item.likelihood(1),
        1.0,
        "a model predicting no variant makes a negative certain"
    );
    assert_eq!(
        item.likelihood(2),
        1.0,
        "a model predicting a fraction below the detection limit also makes a negative certain, \
         which is what makes the limit load-bearing"
    );
}

#[test]
fn missing_failed_negative_and_censored_are_four_distinct_states() {
    let negative = Observation::Negative(
        NegativeResult::new("a", "s", 100, 0.9, 0.02, "p").expect("admissible"),
    );
    let missing = Observation::Missing {
        reason: "never ordered".into(),
    };
    let failed = Observation::Failed {
        reason: "library prep failed QC".into(),
    };
    let censored = Observation::Censored {
        bound: 5.0,
        unit: "millimetre".into(),
        above: true,
    };

    assert!(negative.is_informative(0.5));
    assert!(!missing.is_informative(0.5));
    assert!(!failed.is_informative(0.5));
    assert!(censored.is_informative(0.5));
    assert_ne!(missing, failed);
}

#[test]
fn a_detection_limit_outside_zero_to_one_is_refused() {
    assert!(matches!(
        NegativeResult::new("a", "s", 10, 0.5, 1.5, "p"),
        Err(EpistemicError::InadmissibleCost { .. })
    ));
}

fn sample_document() -> serde_json::Value {
    json!({
        "study": {
            "name": "reference-cohort",
            "subjects": [
                { "id": "s1", "site": "alpha", "volume_mm3": 1200.0 },
                { "id": "s2", "site": "beta", "volume_mm3": 900.0 },
                { "id": "s3", "site": "alpha", "volume_mm3": 1500.0 }
            ]
        }
    })
}

#[test]
fn a_total_field_path_satisfies_all_three_lens_laws() {
    let lens = QueryLens::field_path("study_name", &["study", "name"]);
    let corpus = vec![
        (sample_document(), vec![json!("renamed")]),
        (sample_document(), vec![json!("another")]),
    ];
    let report = check_laws(&lens, &corpus).expect("checkable");

    assert!(report.lawful(), "field paths are the well-behaved case: {report:?}");
    assert!(report.applicability().holds());
}

#[test]
fn a_filtered_traversal_fails_put_get_with_a_counterexample_a_reader_can_rerun() {
    let lens = QueryLens::new(
        "alpha_site_subjects",
        vec![
            Focus::Field {
                name: "study".into(),
            },
            Focus::Field {
                name: "subjects".into(),
            },
            Focus::Where {
                field: "site".into(),
                value: json!("alpha"),
            },
        ],
    );
    assert_eq!(lens.kind(), OpticKind::Traversal);
    assert!(lens.is_predicate_focused());

    let replacement = vec![
        json!({ "id": "s1", "site": "gamma", "volume_mm3": 1.0 }),
        json!({ "id": "s3", "site": "gamma", "volume_mm3": 2.0 }),
    ];
    let report = check_laws(&lens, &[(sample_document(), replacement)]).expect("checkable");

    match &report.put_get {
        LawStatus::Fails { detail, .. } => assert!(!detail.is_empty()),
        other => panic!(
            "writing values that fail the predicate moves the focus, so put-get cannot hold: \
             {other:?}"
        ),
    }
    assert!(
        report.get_put.holds(),
        "get-put still holds: putting back exactly what was read changes nothing"
    );
    assert!(!report.lawful());
    assert!(!report.applicability().holds());
}

#[test]
fn an_aggregation_getter_has_no_put_at_all() {
    let lens = QueryLens::new(
        "total_volume",
        vec![
            Focus::Field {
                name: "study".into(),
            },
            Focus::Field {
                name: "subjects".into(),
            },
            Focus::Each,
            Focus::SumOf {
                field: "volume_mm3".into(),
            },
        ],
    );
    assert_eq!(lens.kind(), OpticKind::Getter);

    let document = sample_document();
    let read = lens.get(&document).expect("readable");
    assert_eq!(read, vec![json!(3600.0)]);

    assert!(matches!(
        lens.put(&document, &[json!(0.0)]),
        Err(EpistemicError::NoLawfulPut { .. })
    ));

    let report = check_laws(&lens, &[(document, vec![json!(0.0)])]).expect("checkable");
    assert!(matches!(report.get_put, LawStatus::NotApplicable { .. }));
    assert!(
        !report.lawful(),
        "not-applicable is not a pass; a read-only optic is not a lawful lens"
    );
}

#[test]
fn a_plain_traversal_satisfies_all_three_laws() {
    let lens = QueryLens::new(
        "each_subject_id",
        vec![
            Focus::Field {
                name: "study".into(),
            },
            Focus::Field {
                name: "subjects".into(),
            },
            Focus::Each,
            Focus::Field { name: "id".into() },
        ],
    );
    let corpus = vec![(
        sample_document(),
        vec![json!("x1"), json!("x2"), json!("x3")],
    )];
    let report = check_laws(&lens, &corpus).expect("checkable");
    assert!(report.lawful(), "{report:?}");
    assert_eq!(lens.get(&sample_document()).expect("readable").len(), 3);
}

#[test]
fn a_put_whose_arity_does_not_match_the_focus_count_is_refused() {
    let lens = QueryLens::new(
        "each_id",
        vec![
            Focus::Field {
                name: "study".into(),
            },
            Focus::Field {
                name: "subjects".into(),
            },
            Focus::Each,
            Focus::Field { name: "id".into() },
        ],
    );
    assert!(matches!(
        lens.put(&sample_document(), &[json!("only-one")]),
        Err(EpistemicError::PutArity { .. })
    ));
}

#[test]
fn composing_views_across_two_coordinate_frames_without_a_registered_transform_is_refused() {
    let viewport = IndexedLens::new(
        QueryLens::field_path("viewport", &["study", "name"]),
        BTreeMap::from([
            ("coordinate_frame".to_string(), "scan-42".to_string()),
            ("units".to_string(), "voxel".to_string()),
        ]),
    );
    let pathology = IndexedLens::new(
        QueryLens::field_path("pathology", &["study", "name"]),
        BTreeMap::from([("coordinate_frame".to_string(), "slide-7".to_string())]),
    );

    let registry = TransformRegistry::new();
    assert!(
        matches!(
            registry.compose(&viewport, &pathology),
            Err(EpistemicError::UnregisteredIndexTransform { .. })
        ),
        "erasing the frame index would preserve a claim that depends on it"
    );
}

#[test]
fn a_registered_transform_permits_the_composition_and_the_index_survives_it() {
    let viewport = IndexedLens::new(
        QueryLens::field_path("viewport", &["study"]),
        BTreeMap::from([
            ("coordinate_frame".to_string(), "scan-42".to_string()),
            ("units".to_string(), "voxel".to_string()),
        ]),
    );
    let pathology = IndexedLens::new(
        QueryLens::field_path("pathology", &["name"]),
        BTreeMap::from([("coordinate_frame".to_string(), "slide-7".to_string())]),
    );

    let mut registry = TransformRegistry::new();
    registry.register("coordinate_frame", "scan-42", "slide-7");
    let composed = registry.compose(&viewport, &pathology).expect("composable");

    assert!(
        composed.index.contains_key("coordinate_frame") && composed.index.contains_key("units"),
        "the composed view must still say what it is indexed by: {:?}",
        composed.index
    );
    assert_eq!(composed.lens.steps.len(), 2);
    assert_eq!(
        composed.lens.get(&sample_document()).expect("readable"),
        vec![json!("reference-cohort")]
    );
}

#[test]
fn a_view_receipt_carries_the_source_digest_and_the_focus_cardinality() {
    let view = IndexedLens::new(
        QueryLens::new(
            "subject_ids",
            vec![
                Focus::Field {
                    name: "study".into(),
                },
                Focus::Field {
                    name: "subjects".into(),
                },
                Focus::Each,
                Focus::Field { name: "id".into() },
            ],
        ),
        BTreeMap::from([("cohort".to_string(), "reference".to_string())]),
    );
    let receipt = view_receipt(&view, &sample_document()).expect("buildable");

    assert_eq!(receipt.foci, 3, "cardinality is exposed, not defaulted");
    assert_eq!(receipt.kind, OpticKind::Traversal);
    assert_eq!(receipt.source_digest.len(), 64);
    assert_eq!(
        receipt.source_digest,
        view_receipt(&view, &sample_document())
            .expect("buildable")
            .source_digest,
        "the same document must digest the same way on every run"
    );
}

#[test]
fn an_affine_focus_on_an_absent_field_returns_no_foci_rather_than_a_null() {
    let lens = QueryLens::field_path("absent", &["study", "no_such_key"]);
    assert_eq!(lens.kind(), OpticKind::AffineTraversal);
    assert!(
        lens.get(&sample_document()).expect("readable").is_empty(),
        "an absent focus is zero foci, not one focus holding null"
    );
}

#[test]
fn the_library_and_the_decision_calculus_join_through_a_negative_result() {
    let result = NegativeResult::new("panel-v3", "sp-1", 150, 0.85, 0.02, "pipeline-1.4")
        .expect("admissible");
    let item = result
        .as_evidence("negative_panel", 1.0, &[0.35, 0.0])
        .expect("admissible");
    let pool = bioprism_epistemic::EvidencePool::new(vec![item]).expect("pool");
    let problem = DecisionProblem::new(
        vec!["call_present".into(), "call_absent".into()],
        vec!["present".into(), "absent".into()],
        vec![0.0, 1.0, 1.0, 0.0],
    )
    .expect("well-formed");
    let prior = Belief::uniform(2).expect("uniform");

    let posterior = pool.full_posterior(&prior).expect("conditionable");
    assert_eq!(
        problem.bayes_action(&posterior),
        1,
        "an informative negative must move the decision to calling absence"
    );
}
