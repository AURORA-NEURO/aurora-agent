//! 27.14. What a contradiction is, what it is not, and what may never be silently reconciled.

use bioprism_scope::{Interval, ScopeKey, ScopeValue, Timestamp};
use bioprism_worldfactory::contradiction::{
    check_intent, cue_scan, expectedness, next_actions, pose, validate, Contradiction, Discordance,
    DiscordanceClass, DiscriminatingAction, EvidenceId, Expectedness, Hypothesis, HypothesisSet,
    IntentCheck, Lens, MissingEvidence, ModalityId, Obtainability, Reading, ReadingValue,
    ReferenceDiscordance, Reported, ResolutionState, SpatialExtent,
};
use bioprism_worldfactory::error::ContradictionRefusal;
use std::collections::BTreeSet;

const QUANTITY: &str = "lesion-burden";

fn aligned_scope() -> ScopeKey {
    ScopeKey::new().exact("specimen", "S1").exact("time", "T1")
}

fn imaging(value: ReadingValue) -> Reading {
    Reading::new(
        "imaging",
        QUANTITY,
        Lens::new("mri-t1c", "macroscopic volume")
            .detecting_down_to(100)
            .over(SpatialExtent::Whole),
        aligned_scope(),
        Reported::Value(value),
    )
}

fn pathology(value: ReadingValue) -> Reading {
    Reading::new(
        "pathology",
        QUANTITY,
        Lens::new("he-slide", "sampled fragment")
            .detecting_down_to(1)
            .over(SpatialExtent::Sampled {
                region: "block A".to_string(),
            }),
        aligned_scope(),
        Reported::Value(value),
    )
}

fn disagreement() -> Contradiction {
    pose(
        imaging(ReadingValue::interval(50, 60)),
        pathology(ReadingValue::interval(0, 10)),
    )
    .expect("the readings are aligned and disagree")
}

fn hypotheses() -> HypothesisSet {
    HypothesisSet::new()
        .with(Hypothesis::new("h-sampling", Discordance::SpatialSampling {
            modality: ModalityId::new("pathology"),
        }))
        .with(Hypothesis::new("h-scope", Discordance::AssayScope))
        .with(Hypothesis::new(
            "h-handling",
            Discordance::PreanalyticFault {
                stage: "preservation".to_string(),
            },
        ))
        .with(Hypothesis::new(
            "h-irreducible",
            Discordance::IrreducibleDiscordance,
        ))
}

fn program() -> bioprism_worldfactory::ContradictionProgram {
    bioprism_worldfactory::ContradictionProgram::new(
        disagreement(),
        DiscordanceClass::Resolvable,
        hypotheses(),
    )
}

#[test]
fn readings_whose_scopes_do_not_overlap_are_not_a_contradiction_and_the_dimension_is_the_finding() {
    let elsewhere = Reading::new(
        "pathology",
        QUANTITY,
        Lens::new("he-slide", "sampled fragment"),
        ScopeKey::new().exact("specimen", "S2").exact("time", "T1"),
        Reported::Value(ReadingValue::interval(0, 10)),
    );
    let refusal = pose(imaging(ReadingValue::interval(50, 60)), elsewhere)
        .expect_err("different specimens are not in disagreement");
    match refusal {
        ContradictionRefusal::ScopesDoNotOverlap { dimension, .. } => {
            assert_eq!(dimension, "specimen");
        }
        other => panic!("expected a scope refusal naming the dimension, got {other:?}"),
    }
}

#[test]
fn a_dimension_bound_to_values_of_different_kinds_is_a_modelling_error_not_a_disagreement() {
    let windowed = Reading::new(
        "pathology",
        QUANTITY,
        Lens::new("he-slide", "sampled fragment"),
        ScopeKey::new().exact("specimen", "S1").bind(
            "time",
            ScopeValue::Window(Interval {
                start: Some(Timestamp::parse("2026-01-01T00:00:00Z").unwrap()),
                end: None,
            }),
        ),
        Reported::Value(ReadingValue::interval(0, 10)),
    );
    let refusal = pose(imaging(ReadingValue::interval(50, 60)), windowed)
        .expect_err("an exact value and a window are not comparable");
    assert!(matches!(
        refusal,
        ContradictionRefusal::IncomparableScopes { dimension } if dimension == "time"
    ));
}

#[test]
fn a_modality_that_was_not_examined_has_not_disagreed_with_anything() {
    let unexamined = Reading::new(
        "pathology",
        QUANTITY,
        Lens::new("he-slide", "sampled fragment"),
        aligned_scope(),
        Reported::NotExamined,
    );
    let refusal = pose(imaging(ReadingValue::interval(50, 60)), unexamined)
        .expect_err("an absence of evidence is not a conflict");
    assert!(matches!(
        refusal,
        ContradictionRefusal::ModalityNotExamined { modality } if modality == "pathology"
    ));
}

#[test]
fn two_intervals_that_overlap_agree_rather_than_manufacturing_a_disagreement() {
    let refusal = pose(
        imaging(ReadingValue::interval(40, 60)),
        pathology(ReadingValue::interval(55, 80)),
    )
    .expect_err("intersecting uncertainty ranges are agreement");
    assert!(matches!(refusal, ContradictionRefusal::ReadingsAgree { .. }));
}

#[test]
fn readings_of_different_quantities_have_no_shared_subject_to_disagree_about() {
    let other_quantity = Reading::new(
        "pathology",
        "mitotic-count",
        Lens::new("he-slide", "sampled fragment"),
        aligned_scope(),
        Reported::Value(ReadingValue::interval(0, 10)),
    );
    let refusal = pose(imaging(ReadingValue::interval(50, 60)), other_quantity)
        .expect_err("different quantities are not a contradiction");
    assert!(matches!(
        refusal,
        ContradictionRefusal::DifferentQuantities { .. }
    ));
}

#[test]
fn not_yet_examined_is_never_reported_as_unresolvable() {
    let program = program().with_action(
        DiscriminatingAction::new("re-cut-deeper-levels", 2).refuting("h-sampling"),
    );
    match program.state() {
        ResolutionState::NotYetExamined { available } => {
            assert_eq!(available.len(), 1, "one action sits unopened");
        }
        other => panic!("unexamined evidence must not read as unresolvable, got {other:?}"),
    }
}

#[test]
fn unresolvable_is_only_reached_once_every_available_action_has_been_examined() {
    let mut program = program()
        .with_action(DiscriminatingAction::new("re-cut", 2).refuting("h-sampling"))
        .with_missing(MissingEvidence {
            description: "a second specimen from the same lesion".to_string(),
            would_refute: BTreeSet::new(),
            obtainability: Obtainability::RequiresNewSpecimen,
        });
    program
        .examine(&EvidenceId::new("re-cut"))
        .expect("the action refutes one of four");
    match program.state() {
        ResolutionState::Unresolvable {
            examined,
            would_resolve,
        } => {
            assert_eq!(examined.len(), 1);
            assert_eq!(
                would_resolve.len(),
                1,
                "an unresolvable contradiction names what would settle it"
            );
        }
        other => panic!("expected unresolvable after exhausting the world, got {other:?}"),
    }
}

#[test]
fn examining_evidence_narrows_the_set_without_choosing_a_winner() {
    let mut program =
        program().with_action(DiscriminatingAction::new("re-cut", 2).refuting("h-sampling"));
    let before = program.live().len();
    let after = program
        .examine(&EvidenceId::new("re-cut"))
        .expect("narrowing is legal")
        .len();
    assert_eq!(before - 1, after);
    assert!(
        program.live().sole().is_none(),
        "three accounts remain, so there is no sole answer to report"
    );
}

#[test]
fn a_discriminator_that_refutes_every_account_is_a_defect_not_a_resolution() {
    let mut program = program().with_action(
        DiscriminatingAction::new("everything", 1)
            .refuting("h-sampling")
            .refuting("h-scope")
            .refuting("h-handling")
            .refuting("h-irreducible"),
    );
    let refusal = program
        .examine(&EvidenceId::new("everything"))
        .expect_err("an empty hypothesis set is not an answer");
    assert!(matches!(
        refusal,
        ContradictionRefusal::AllHypothesesRefuted { .. }
    ));
    assert_eq!(
        program.live().len(),
        4,
        "the refused narrowing must not have been applied"
    );
}

#[test]
fn preferring_a_modality_is_never_evidence() {
    let program = program();
    let refusal = program.prefer_modality(&ModalityId::new("pathology"));
    assert!(matches!(
        refusal,
        ContradictionRefusal::ModalityPreferredWithoutEvidence { modality } if modality == "pathology"
    ));
}

#[test]
fn sensitivity_cannot_explain_a_finding_made_by_the_modality_with_the_declared_floor() {
    let contradiction = disagreement();
    let blaming_the_reporter = Discordance::SensitivityLimit {
        modality: ModalityId::new("imaging"),
    };
    assert!(
        contradiction.admissibility(&blaming_the_reporter).is_err(),
        "imaging reported the higher value; its own floor cannot explain the disagreement away"
    );
    let blaming_the_quiet_one = Discordance::SensitivityLimit {
        modality: ModalityId::new("pathology"),
    };
    assert!(contradiction.admissibility(&blaming_the_quiet_one).is_ok());
}

#[test]
fn an_account_whose_axis_the_scopes_agree_on_is_inadmissible() {
    let contradiction = disagreement();
    assert!(
        contradiction
            .admissibility(&Discordance::DifferentSpecimen)
            .is_err(),
        "both readings bind specimen to S1"
    );
    assert!(
        contradiction
            .admissibility(&Discordance::DifferentTime)
            .is_err(),
        "both readings bind time to T1"
    );
}

#[test]
fn a_disagreement_no_account_can_explain_is_refused_rather_than_scored() {
    let identical_lens = Lens::new("shared", "identical scope");
    let left = Reading::new(
        "a",
        QUANTITY,
        identical_lens.clone(),
        aligned_scope(),
        Reported::Value(ReadingValue::Categorical("positive".into())),
    );
    let right = Reading::new(
        "b",
        QUANTITY,
        identical_lens,
        aligned_scope(),
        Reported::Value(ReadingValue::Categorical("negative".into())),
    );
    let contradiction = pose(left, right).expect("the categories differ");
    let only_impossible = HypothesisSet::new()
        .with(Hypothesis::new("h-time", Discordance::DifferentTime))
        .with(Hypothesis::new(
            "h-scope",
            Discordance::AssayScope,
        ));
    let program = bioprism_worldfactory::ContradictionProgram::new(
        contradiction,
        DiscordanceClass::Irreducible,
        only_impossible,
    );
    assert!(matches!(
        validate(program).expect_err("neither account can apply"),
        ContradictionRefusal::NoAdmissibleExplanation { .. }
    ));
}

#[test]
fn an_account_named_in_an_annotation_is_an_answer_cue() {
    let leaky = imaging(ReadingValue::interval(50, 60))
        .annotated("flagged: possible preanalytic_fault during preservation")
        .annotated("second note so the sole-annotation cue is not what fires");
    let contradiction = pose(leaky, pathology(ReadingValue::interval(0, 10)))
        .expect("still a disagreement");
    let program = bioprism_worldfactory::ContradictionProgram::new(
        contradiction,
        DiscordanceClass::Resolvable,
        hypotheses(),
    );
    let cues = cue_scan(&program);
    assert!(!cues.is_empty(), "the seeded account is legible on the surface");
    assert!(matches!(
        validate(program).expect_err("a cued program is not a benchmark"),
        ContradictionRefusal::TrivialCue { .. }
    ));
}

#[test]
fn exactly_one_annotated_reading_is_itself_a_cue_even_when_the_note_says_nothing() {
    let contradiction = pose(
        imaging(ReadingValue::interval(50, 60)).annotated("reviewed"),
        pathology(ReadingValue::interval(0, 10)),
    )
    .expect("still a disagreement");
    let program = bioprism_worldfactory::ContradictionProgram::new(
        contradiction,
        DiscordanceClass::Resolvable,
        hypotheses(),
    );
    assert!(
        !cue_scan(&program).is_empty(),
        "asymmetric presence points at the interesting reading without being read"
    );
}

#[test]
fn action_ordering_prefers_the_action_that_refutes_more_live_accounts() {
    let program = program()
        .with_action(
            DiscriminatingAction::new("broad", 9)
                .refuting("h-sampling")
                .refuting("h-scope"),
        )
        .with_action(DiscriminatingAction::new("narrow", 1).refuting("h-handling"));
    let ranked = next_actions(&program);
    assert_eq!(ranked[0].evidence.as_str(), "broad");
    assert_eq!(ranked[0].refutes_live, 2);
    assert_eq!(ranked[1].refutes_live, 1);
}

#[test]
fn expectedness_refuses_without_a_reference_distribution_rather_than_guessing() {
    let bare = program();
    assert!(matches!(
        expectedness(&bare, 500).expect_err("no reference for this modality pair"),
        ContradictionRefusal::NoReferenceDistribution { .. }
    ));

    let with_reference =
        program().with_reference(ReferenceDiscordance::new("imaging", "pathology", 1_000, 120));
    assert_eq!(
        expectedness(&with_reference, 500).expect("a reference is declared"),
        Expectedness::Routine {
            rate_per_ten_thousand: 1_200
        }
    );
}

#[test]
fn a_reference_series_with_no_comparisons_has_no_rate_rather_than_a_rate_of_zero() {
    let empty = ReferenceDiscordance::new("imaging", "pathology", 0, 0);
    assert_eq!(empty.rate_per_ten_thousand(), None);
}

#[test]
fn declaring_a_discordance_resolvable_is_checked_against_the_evidence_the_world_contains() {
    let cannot_narrow =
        program().with_action(DiscriminatingAction::new("re-cut", 2).refuting("h-sampling"));
    assert_eq!(
        check_intent(&cannot_narrow),
        IntentCheck::DeclaredResolvableButCannotNarrow { remaining: 3 }
    );

    let can_narrow = program()
        .with_action(
            DiscriminatingAction::new("re-cut", 2)
                .refuting("h-sampling")
                .refuting("h-scope"),
        )
        .with_action(DiscriminatingAction::new("repeat-assay", 3).refuting("h-handling"));
    assert_eq!(check_intent(&can_narrow), IntentCheck::Consistent);
}

#[test]
fn declaring_a_discordance_irreducible_is_refuted_by_evidence_that_settles_it() {
    let mut settleable = bioprism_worldfactory::ContradictionProgram::new(
        disagreement(),
        DiscordanceClass::Irreducible,
        hypotheses(),
    );
    settleable = settleable
        .with_action(
            DiscriminatingAction::new("re-cut", 2)
                .refuting("h-sampling")
                .refuting("h-scope"),
        )
        .with_action(DiscriminatingAction::new("repeat-assay", 3).refuting("h-handling"));
    assert!(matches!(
        check_intent(&settleable),
        IntentCheck::DeclaredIrreducibleButEvidenceSettlesIt { .. }
    ));
}

#[test]
fn validation_drops_inadmissible_accounts_and_keeps_the_rest() {
    let mixed = hypotheses().with(Hypothesis::new("h-specimen", Discordance::DifferentSpecimen));
    let program = bioprism_worldfactory::ContradictionProgram::new(
        disagreement(),
        DiscordanceClass::Irreducible,
        mixed,
    );
    let validated = validate(program).expect("four of five accounts are admissible");
    assert_eq!(validated.admissible().len(), 4);
    assert!(
        !validated
            .admissible()
            .contains_key(&bioprism_worldfactory::contradiction::HypothesisId::new("h-specimen")),
        "both readings are of specimen S1, so a specimen mix-up cannot be the account"
    );
}

#[test]
fn the_three_resolution_states_keep_their_names_through_serde() {
    let program = program().with_action(DiscriminatingAction::new("re-cut", 1));
    let encoded = serde_json::to_string(&program.state()).expect("states serialise");
    assert!(encoded.contains("not_yet_examined"), "{encoded}");
    let decoded: ResolutionState = serde_json::from_str(&encoded).expect("and round-trip");
    assert_eq!(decoded.as_str(), "not_yet_examined");
}
