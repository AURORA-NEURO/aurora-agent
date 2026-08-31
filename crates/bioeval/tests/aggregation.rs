//! Aggregation that carries the disagreement rather than averaging it away (26.15, 26.20).

mod common;

use bioprism_bioeval::{
    AggregationError, BioScore, BiologicalErrorClass, CollapsePolicy, ConsensusPolicy,
    ConsensusState, Dispersion, PanelAggregate, PooledScore, Prediction, Rating, ReferenceStandard,
};
use common::{
    grader, progression_reference, score, witness, PROGRESSION, REQUIREMENT_ID, TREATMENT_EFFECT,
};
use serde_json::json;

fn policy() -> ConsensusPolicy {
    ConsensusPolicy::conventional("bioeval/panel/1")
}

fn split_panel() -> PanelAggregate {
    PanelAggregate::tally(
        &policy(),
        [
            Rating::new("reader-a", PROGRESSION),
            Rating::new("reader-b", PROGRESSION),
            Rating::new("reader-c", PROGRESSION),
            Rating::new("reader-d", TREATMENT_EFFECT),
            Rating::new("reader-e", TREATMENT_EFFECT),
        ],
    )
    .expect("five distinct readers")
}

#[test]
fn a_split_panel_becomes_a_distribution_not_a_majority_label() {
    let distribution = split_panel().distribution();

    assert_eq!(distribution.get(PROGRESSION), Some(&0.6));
    assert_eq!(distribution.get(TREATMENT_EFFECT), Some(&0.4));
    assert!(split_panel().entropy_bits() > 0.9);
}

#[test]
fn a_panel_fed_forward_as_a_reference_denies_its_own_majority_a_clean_pass() {
    let reference = split_panel()
        .into_reference_standard(Dispersion::Aleatoric)
        .expect("the empirical shares are a distribution");

    let graded = score(&Prediction::categorical(PROGRESSION), &reference);

    assert!(
        !graded.is_clean_pass(),
        "three of five readers is not the truth, and a system agreeing with them has not been \
         proven right"
    );
    assert!(graded.interval().width() > 0.0);
}

#[test]
fn every_rating_survives_aggregation() {
    let panel = split_panel();

    assert_eq!(panel.ratings().len(), 5);
    assert_eq!(panel.raters_for(TREATMENT_EFFECT).len(), 2);
    let minority = panel.minority_positions();
    assert_eq!(minority.len(), 1);
    assert_eq!(minority[0].0, TREATMENT_EFFECT);
    assert_eq!(minority[0].1.len(), 2);
}

#[test]
fn a_two_thirds_majority_names_its_dissenters() {
    let panel = PanelAggregate::tally(
        &policy(),
        [
            Rating::new("reader-a", PROGRESSION),
            Rating::new("reader-b", PROGRESSION),
            Rating::new("reader-c", TREATMENT_EFFECT),
        ],
    )
    .expect("three distinct readers");

    match panel.consensus() {
        ConsensusState::Majority {
            position,
            dissenters,
            ..
        } => {
            assert_eq!(position, PROGRESSION);
            assert_eq!(dissenters, &vec!["reader-c".to_string()]);
        }
        other => panic!("expected a majority with named dissent, got {other:?}"),
    }
}

#[test]
fn a_panel_short_of_the_threshold_reports_no_consensus() {
    assert!(matches!(
        split_panel().consensus(),
        ConsensusState::None { .. }
    ));
    assert!(split_panel().consensus().actionable_position().is_none());
}

#[test]
fn a_lone_dissenter_on_a_safety_reaching_class_vetoes_consensus() {
    let panel = PanelAggregate::tally(
        &policy(),
        [
            Rating::new("reader-a", PROGRESSION),
            Rating::new("reader-b", PROGRESSION),
            Rating::new("reader-c", PROGRESSION),
            Rating::new("reader-d", PROGRESSION),
            Rating::new("reader-e", PROGRESSION),
            Rating::new("reader-f", PROGRESSION),
            Rating::new("reader-g", PROGRESSION),
            Rating::new("reader-h", PROGRESSION),
            Rating::new("reader-i", PROGRESSION)
                .flagging(BiologicalErrorClass::Laterality)
                .because("the report says left; the study is of the right hemisphere"),
        ],
    )
    .expect("nine distinct readers");

    let vetoes = panel.vetoes();
    assert_eq!(vetoes.len(), 1);
    assert_eq!(vetoes[0].rater, "reader-i");
    assert_eq!(vetoes[0].class, BiologicalErrorClass::Laterality);
    assert!(
        panel.consensus().actionable_position().is_none(),
        "eight out of nine does not make the wrong side of a head into the right one"
    );
}

#[test]
fn a_veto_can_be_switched_off_but_only_by_a_named_policy() {
    let permissive = ConsensusPolicy {
        policy_id: "bioeval/panel-no-veto/1".to_string(),
        majority_threshold: 2.0 / 3.0,
        veto_on_safety_reaching: false,
    };
    let panel = PanelAggregate::tally(
        &permissive,
        [
            Rating::new("reader-a", PROGRESSION),
            Rating::new("reader-b", PROGRESSION),
            Rating::new("reader-c", PROGRESSION).flagging(BiologicalErrorClass::Laterality),
        ],
    )
    .expect("three distinct readers");

    assert!(matches!(
        panel.consensus(),
        ConsensusState::Unanimous { .. }
    ));
    assert_eq!(panel.policy_id(), "bioeval/panel-no-veto/1");
    assert!(panel.vetoes().is_empty());
}

#[test]
fn a_unanimous_panel_is_reported_as_unanimous_not_as_a_majority() {
    let panel = PanelAggregate::tally(
        &policy(),
        [
            Rating::new("reader-a", PROGRESSION),
            Rating::new("reader-b", PROGRESSION),
        ],
    )
    .expect("two distinct readers");

    assert!(matches!(
        panel.consensus(),
        ConsensusState::Unanimous { .. }
    ));
    assert_eq!(panel.entropy_bits(), 0.0);
    assert!(panel.minority_positions().is_empty());
}

#[test]
fn a_duplicate_rater_is_refused_rather_than_counted_twice() {
    let refusal = PanelAggregate::tally(
        &policy(),
        [
            Rating::new("reader-a", PROGRESSION),
            Rating::new("reader-a", PROGRESSION),
            Rating::new("reader-b", TREATMENT_EFFECT),
        ],
    )
    .expect_err("counting one reader twice is the cheapest way to manufacture a majority");

    assert!(matches!(
        refusal,
        AggregationError::DuplicateRater { rater } if rater == "reader-a"
    ));
}

#[test]
fn an_empty_panel_is_not_consensus() {
    let refusal = PanelAggregate::tally(&policy(), [])
        .expect_err("absence of raters is not agreement among them");

    assert!(matches!(refusal, AggregationError::EmptyPanel));
}

#[test]
fn malformed_policy_is_refused_before_it_can_change_consensus() {
    let policy = ConsensusPolicy {
        policy_id: " ".to_string(),
        majority_threshold: f64::NAN,
        veto_on_safety_reaching: true,
    };

    let refusal = PanelAggregate::tally(&policy, [Rating::new("reader-a", PROGRESSION)])
        .expect_err("an aggregate needs a named, bounded policy");

    assert!(matches!(refusal, AggregationError::InvalidPolicy { .. }));
}

#[test]
fn malformed_rating_payloads_are_refused_instead_of_becoming_panel_evidence() {
    let refusal = PanelAggregate::tally(
        &policy(),
        [Rating::new("reader-a", " ").because("line\nnoise")],
    )
    .expect_err("empty positions and control-bearing rationales are not evidence");

    assert!(
        matches!(refusal, AggregationError::InvalidRating { rater, .. } if rater == "reader-a")
    );
}

#[test]
fn a_rating_cannot_repeat_the_same_error_class_to_multiply_a_veto() {
    let refusal = PanelAggregate::tally(
        &policy(),
        [Rating::new("reader-a", PROGRESSION)
            .flagging(BiologicalErrorClass::Laterality)
            .flagging(BiologicalErrorClass::Laterality)],
    )
    .expect_err("a flagged class is a set-valued assertion, not a vote count");

    assert!(matches!(refusal, AggregationError::InvalidRating { .. }));
}

#[test]
fn panel_order_is_canonicalized_before_tallying() {
    let forward = PanelAggregate::tally(
        &policy(),
        [
            Rating::new("reader-b", TREATMENT_EFFECT),
            Rating::new("reader-a", PROGRESSION),
        ],
    )
    .expect("valid panel");
    let reverse = PanelAggregate::tally(
        &policy(),
        [
            Rating::new("reader-a", PROGRESSION),
            Rating::new("reader-b", TREATMENT_EFFECT),
        ],
    )
    .expect("valid panel");

    assert_eq!(
        serde_json::to_value(forward).expect("panel serialises"),
        serde_json::to_value(reverse).expect("panel serialises"),
        "the same ratings must not produce input-order-dependent aggregate records"
    );
}

#[test]
fn pooling_scores_across_comparability_gates_is_refused() {
    let here = score(
        &Prediction::categorical(PROGRESSION),
        &progression_reference(Dispersion::Aleatoric),
    );

    let other_gate = bioprism_bioeval::ComparabilityRequirement::over(
        "bioeval/other/1",
        [bioprism_bioeval::FrameDimension::Unit],
    );
    let other_witness = bioprism_bioeval::gate(
        &other_gate,
        &common::declared_frame(),
        &common::declared_frame(),
        &[],
    )
    .expect("one matching dimension satisfies the narrow gate");
    let there = bioprism_bioeval::Grader::new("other", other_gate)
        .grade(
            &other_witness,
            "case-2",
            &Prediction::categorical(PROGRESSION),
            &progression_reference(Dispersion::Aleatoric),
        )
        .expect("grades cleanly")
        .score()
        .expect("not an abstention")
        .clone();

    let refusal = PooledScore::pool([here, there])
        .expect_err("results that passed different gates are not results about the same thing");

    assert!(matches!(
        refusal,
        AggregationError::MixedRequirements { expected, .. } if expected == REQUIREMENT_ID
    ));
}

#[test]
fn pooling_rechecks_persisted_score_fields_before_reading_their_band() {
    let clean = score(
        &Prediction::categorical(PROGRESSION),
        &progression_reference(Dispersion::Aleatoric),
    );
    let mut forged = serde_json::to_value(&clean).expect("score serialises");
    forged["subject"] = json!(" ");
    let forged: BioScore = serde_json::from_value(forged).expect("shape still parses");

    let refusal = PooledScore::pool([forged])
        .expect_err("a parsed score is not trustworthy evidence until aggregation validates it");

    assert!(matches!(refusal, AggregationError::InvalidScore { .. }));
}

#[test]
fn a_pool_refuses_to_collapse_when_any_single_case_refuses() {
    let attributed = score(
        &Prediction::categorical(PROGRESSION),
        &progression_reference(Dispersion::Aleatoric),
    );
    let unattributed = score(
        &Prediction::categorical(PROGRESSION),
        &progression_reference(Dispersion::Unattributed),
    );

    let pool = PooledScore::pool([attributed, unattributed]).expect("same gate");
    let refusal = pool
        .collapse(&CollapsePolicy::strict("bioeval/publish/1"))
        .expect_err("dropping the case that refused would restrict the mean to the easy cases");

    assert!(matches!(
        refusal,
        AggregationError::CaseNotCollapsible { .. }
    ));
}

#[test]
fn a_pool_that_can_collapse_still_reports_its_band_and_its_critical_count() {
    let clean = score(
        &Prediction::categorical(PROGRESSION),
        &progression_reference(Dispersion::Aleatoric),
    );
    let flagged = score(
        &Prediction::categorical(PROGRESSION),
        &progression_reference(Dispersion::Aleatoric),
    )
    .with_errors([bioprism_bioeval::ClassifiedError::new(
        bioprism_bioeval::CorrectnessLayer::SpecimenIdentity,
        BiologicalErrorClass::SpecimenIdentity,
        "S-07",
        "S-70",
    )]);

    let pool = PooledScore::pool([clean, flagged]).expect("same gate");
    let mean = pool
        .collapse(&CollapsePolicy::strict("bioeval/publish/1"))
        .expect("both cases are attributed and above the floor");

    let (lo, hi) = pool.band();
    assert!(hi > lo, "the pooled result is still a band");
    assert!((mean - 0.8325).abs() < 1e-6);
    assert_eq!(
        pool.critical_error_count(),
        1,
        "the mean is 0.83 and one of the two cases was about the wrong subject"
    );
    assert_eq!(pool.clean_pass_count(), 0);
    assert_eq!(pool.len(), 2);
    assert!(!pool.is_empty());
}

#[test]
fn the_widest_case_in_a_pool_is_the_one_its_reference_could_least_decide() {
    let sharp = score(
        &Prediction::categorical(PROGRESSION),
        &common::resolved_reference(PROGRESSION),
    );
    let diffuse = score(
        &Prediction::categorical(PROGRESSION),
        &progression_reference(Dispersion::Aleatoric),
    );

    let pool = PooledScore::pool([sharp, diffuse]).expect("same gate");
    let widest = pool.widest_case().expect("a non-empty pool has one");

    assert!(widest.interval().width() > 0.0);
    assert_eq!(pool.clean_pass_count(), 1);
}

#[test]
fn mutated_descendants_of_one_world_do_not_count_as_independent_observations() {
    let parent = bioprism_ids::WorldId::parse("world:gbm-progression:1").expect("a valid id");
    let sibling = bioprism_ids::WorldId::parse("world:gbm-progression:2").expect("a valid id");
    let case = || {
        score(
            &Prediction::categorical(PROGRESSION),
            &progression_reference(Dispersion::Aleatoric),
        )
    };

    let pool = PooledScore::pool([
        case().from_world(parent.clone()),
        case().from_world(parent.clone()),
        case().from_world(parent),
        case().from_world(sibling),
    ])
    .expect("same gate");

    assert_eq!(pool.len(), 4);
    assert_eq!(
        pool.effective_n(),
        Some(2),
        "four mutants of two worlds are two observations, not four"
    );
    assert!(pool.is_clustered());
}

#[test]
fn a_pool_with_an_undeclared_parent_world_cannot_state_its_effective_size() {
    let declared = score(
        &Prediction::categorical(PROGRESSION),
        &progression_reference(Dispersion::Aleatoric),
    )
    .from_world(bioprism_ids::WorldId::parse("world:gbm-progression:1").expect("a valid id"));
    let undeclared = score(
        &Prediction::categorical(PROGRESSION),
        &progression_reference(Dispersion::Aleatoric),
    );

    let pool = PooledScore::pool([declared, undeclared]).expect("same gate");

    assert_eq!(
        pool.effective_n(),
        None,
        "an undeclared parent cannot be shown to be distinct, and guessing that it is inflates n"
    );
    assert!(!pool.is_clustered());
}

#[test]
fn an_unresolved_panel_can_be_handed_on_as_an_unresolved_reference() {
    let deadlocked = ReferenceStandard::Unresolved {
        reason: "panel split 3-3 with no adjudicator available".to_string(),
    };

    assert!(!deadlocked.can_certify_a_clean_pass());
    assert!(grader()
        .grade(
            &witness(),
            "case-1",
            &Prediction::categorical(PROGRESSION),
            &deadlocked
        )
        .is_err());
}
