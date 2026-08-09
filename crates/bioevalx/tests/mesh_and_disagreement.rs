//! Evaluator independence and what disagreement means (26.01).

use bioprism_bioevalx::error::MeshError;
use bioprism_bioevalx::mesh::{
    Disagreement, EvaluatorDecl, EvaluatorKind, EvaluatorVerdict, Mesh,
};
use bioprism_bioeval::{ConsensusPolicy, ConsensusState, PanelAggregate};
use bioprism_evalengine::{compose, Conclusion, ScoreTier, UnknownPolicy};

fn mesh_over_one_report() -> Mesh {
    let mut mesh = Mesh::for_system(["system-weights".to_string(), "system-outputs".to_string()]);
    for id in ["reader-a", "reader-b", "reader-c"] {
        mesh.admit(EvaluatorDecl::new(id, EvaluatorKind::ExpertReview).reading("report-77"))
            .expect("readers are distinct");
    }
    mesh
}

#[test]
fn an_oracle_built_from_the_system_it_grades_is_refused_at_admission() {
    let mut mesh = Mesh::for_system(["system-weights".to_string()]);

    let outcome = mesh.admit(
        EvaluatorDecl::new("distilled-judge", EvaluatorKind::CalibratedModelJudge)
            .reading("answer-1")
            .built_from("system-weights"),
    );

    match outcome {
        Err(MeshError::CircularOracle { evaluator, artifact }) => {
            assert_eq!(evaluator, "distilled-judge");
            assert_eq!(artifact, "system-weights");
        }
        other => panic!("expected a circularity refusal, got {other:?}"),
    }
    assert!(mesh.evaluators().is_empty(), "a refused oracle is not admitted");
}

#[test]
fn reading_the_systems_answer_is_not_being_built_from_it() {
    let mut mesh = Mesh::for_system(["system-weights".to_string()]);

    mesh.admit(
        EvaluatorDecl::new("judge", EvaluatorKind::CalibratedModelJudge).reading("system-weights"),
    )
    .expect("an evaluator may read the artifact it grades");

    assert_eq!(mesh.evaluators().len(), 1);
}

#[test]
fn three_raters_sharing_one_evidence_source_are_one_independent_class() {
    let mesh = mesh_over_one_report();

    let census = mesh.census().expect("mesh is non-empty");

    assert_eq!(census.evaluators, 3);
    assert_eq!(
        census.independent_classes, 1,
        "shared evidence collapses the vote count"
    );
    assert!(census.independence_verified());
}

#[test]
fn independence_classes_are_transitive_through_a_shared_middle_evaluator() {
    let mut mesh = Mesh::for_system([]);
    mesh.admit(EvaluatorDecl::new("a", EvaluatorKind::ExpertReview).reading("slide"))
        .expect("distinct");
    mesh.admit(
        EvaluatorDecl::new("b", EvaluatorKind::ExpertReview)
            .reading("slide")
            .reading("panel"),
    )
    .expect("distinct");
    mesh.admit(EvaluatorDecl::new("c", EvaluatorKind::ExpertReview).reading("panel"))
        .expect("distinct");

    let classes = mesh.independence_classes();

    assert_eq!(classes.len(), 1, "a and c are linked through b, got {classes:?}");
}

#[test]
fn an_evaluator_that_declared_no_inputs_has_unverified_rather_than_established_independence() {
    let mut mesh = Mesh::for_system([]);
    mesh.admit(EvaluatorDecl::new("silent", EvaluatorKind::StatisticalReference))
        .expect("distinct");

    let census = mesh.census().expect("mesh is non-empty");

    assert_eq!(census.independent_classes, 1);
    assert!(
        !census.independence_verified(),
        "declaring nothing must not read as sharing nothing"
    );
    assert_eq!(census.inputs_undeclared, vec!["silent".to_string()]);
}

#[test]
fn two_raters_on_one_report_disagreeing_is_an_evaluator_defect_not_a_hard_case() {
    let mesh = mesh_over_one_report();

    let found = mesh
        .disagreements(&[
            EvaluatorVerdict::called("reader-a", "progression"),
            EvaluatorVerdict::called("reader-b", "treatment-effect"),
        ])
        .expect("evaluators are declared");

    assert_eq!(found.len(), 1);
    assert!(matches!(found[0], Disagreement::WithinClass(_)));
    assert!(!found[0].is_about_the_case());
    assert_eq!(found[0].witness().left_position, "progression");
}

#[test]
fn two_independent_lines_of_evidence_disagreeing_is_a_finding_with_a_witness() {
    let mut mesh = Mesh::for_system([]);
    mesh.admit(EvaluatorDecl::new("imaging", EvaluatorKind::ExpertReview).reading("mri-4"))
        .expect("distinct");
    mesh.admit(EvaluatorDecl::new("molecular", EvaluatorKind::ExecutableAnalysis).reading("panel-9"))
        .expect("distinct");

    let found = mesh
        .disagreements(&[
            EvaluatorVerdict::called("imaging", "progression"),
            EvaluatorVerdict::called("molecular", "pseudoprogression"),
        ])
        .expect("evaluators are declared");

    assert_eq!(found.len(), 1);
    assert!(found[0].is_about_the_case());
    let witness = found[0].witness();
    assert_eq!(witness.left, "imaging");
    assert_eq!(witness.right_position, "pseudoprogression");
}

#[test]
fn an_abstention_is_retained_and_never_becomes_a_disagreement() {
    let mesh = mesh_over_one_report();

    let found = mesh
        .disagreements(&[
            EvaluatorVerdict::called("reader-a", "progression"),
            EvaluatorVerdict::abstention("reader-b"),
        ])
        .expect("evaluators are declared");

    assert!(found.is_empty(), "an abstention is not a dissent");
}

#[test]
fn an_abstention_reaches_the_ladder_as_unknown_rather_than_as_a_failure() {
    let mesh = mesh_over_one_report();

    let contributions = mesh
        .contributions(
            &[
                EvaluatorVerdict::called("reader-a", "progression"),
                EvaluatorVerdict::abstention("reader-b"),
            ],
            "progression",
        )
        .expect("evaluators are declared");

    assert_eq!(contributions[0].conclusion, Conclusion::Pass);
    assert_eq!(contributions[1].conclusion, Conclusion::Unknown);
    assert!(contributions[1].notes[0].contains("abstained"));
}

#[test]
fn a_model_judge_cannot_raise_a_deterministic_conclusion_through_this_mesh() {
    let mut mesh = Mesh::for_system([]);
    mesh.admit(EvaluatorDecl::new("checksum", EvaluatorKind::DeterministicProperty).reading("out"))
        .expect("distinct");
    mesh.admit(EvaluatorDecl::new("judge", EvaluatorKind::CalibratedModelJudge).reading("prose"))
        .expect("distinct");

    let contributions = mesh
        .contributions(
            &[
                EvaluatorVerdict::called("checksum", "mismatch"),
                EvaluatorVerdict::called("judge", "correct"),
            ],
            "correct",
        )
        .expect("evaluators are declared");
    let scored = compose("case-1", &contributions, &UnknownPolicy::default())
        .expect("contributions are non-empty");

    assert_eq!(scored.deciding_tier, ScoreTier::Deterministic);
    assert_eq!(scored.conclusion, Conclusion::Fail);
}

#[test]
fn a_mesh_of_only_model_judges_reports_zero_non_model_classes() {
    let mut mesh = Mesh::for_system([]);
    mesh.admit(EvaluatorDecl::new("judge-a", EvaluatorKind::CalibratedModelJudge).reading("x"))
        .expect("distinct");
    mesh.admit(EvaluatorDecl::new("judge-b", EvaluatorKind::CalibratedModelJudge).reading("y"))
        .expect("distinct");

    let census = mesh.census().expect("mesh is non-empty");

    assert_eq!(census.independent_classes, 2);
    assert_eq!(census.non_model_classes, 0);
}

#[test]
fn three_readers_of_one_report_reach_the_reader_panel_as_one_rating() {
    let mesh = mesh_over_one_report();

    let ratings = mesh
        .independent_ratings(&[
            EvaluatorVerdict::called("reader-a", "progression"),
            EvaluatorVerdict::called("reader-b", "progression"),
            EvaluatorVerdict::called("reader-c", "progression"),
        ])
        .expect("the class agrees with itself");

    assert_eq!(ratings.len(), 1, "one shared report is one read");
    assert_eq!(ratings[0].rater, "reader-a+reader-b+reader-c");
    assert_eq!(ratings[0].position, "progression");

    let panel = PanelAggregate::tally(&ConsensusPolicy::conventional("p1"), ratings)
        .expect("panel is non-empty");
    assert_eq!(panel.ratings().len(), 1);
}

#[test]
fn a_class_that_disagrees_with_itself_cannot_be_collapsed_into_a_distribution() {
    let mesh = mesh_over_one_report();

    match mesh.independent_ratings(&[
        EvaluatorVerdict::called("reader-a", "progression"),
        EvaluatorVerdict::called("reader-b", "treatment-effect"),
    ]) {
        Err(MeshError::ClassSplit { positions, .. }) => {
            assert_eq!(positions.len(), 2);
        }
        other => panic!("expected a class-split refusal, got {other:?}"),
    }
}

#[test]
fn independent_classes_each_contribute_one_rating_and_the_disagreement_survives() {
    let mut mesh = Mesh::for_system([]);
    mesh.admit(EvaluatorDecl::new("imaging", EvaluatorKind::ExpertReview).reading("mri-4"))
        .expect("distinct");
    mesh.admit(EvaluatorDecl::new("molecular", EvaluatorKind::ExecutableAnalysis).reading("panel-9"))
        .expect("distinct");

    let ratings = mesh
        .independent_ratings(&[
            EvaluatorVerdict::called("imaging", "progression"),
            EvaluatorVerdict::called("molecular", "pseudoprogression"),
        ])
        .expect("each class is internally consistent");

    assert_eq!(ratings.len(), 2);
    let panel = PanelAggregate::tally(&ConsensusPolicy::conventional("p1"), ratings)
        .expect("panel is non-empty");
    assert!(
        matches!(panel.consensus(), ConsensusState::None { .. }),
        "two independent lines pointing different ways is not a consensus, got {:?}",
        panel.consensus()
    );
}

#[test]
fn a_class_in_which_everyone_abstained_contributes_no_rating() {
    let mesh = mesh_over_one_report();

    let ratings = mesh
        .independent_ratings(&[
            EvaluatorVerdict::abstention("reader-a"),
            EvaluatorVerdict::abstention("reader-b"),
        ])
        .expect("abstentions are not a split");

    assert!(ratings.is_empty(), "an abstention is not a position");
}

#[test]
fn every_declared_evaluator_kind_maps_to_a_ladder_tier() {
    for kind in EvaluatorKind::ALL {
        let tier = kind.tier();
        assert!(
            ScoreTier::ALL.contains(&tier),
            "{kind:?} mapped outside the ladder"
        );
    }
    assert_eq!(
        EvaluatorKind::ExpertReview.tier(),
        EvaluatorKind::CalibratedModelJudge.tier(),
        "neither may override an executable invariant"
    );
}
