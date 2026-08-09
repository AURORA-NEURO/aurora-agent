//! 23.27 and 23.39: mutation expectations, oracle ordering, scale accounting, packs and scoring.

use bioprism_choreography::{GlobalType, Role};
use bioprism_interweave::microbench::{
    admit_release, select, Expectation, Family, Instance, Level, Mutation, Oracle, OraclePlan,
    PlanDefect, ReleaseBlock, ScaleAccount, ScaleRefusal, SelectionCriterion, CONTROLLED_SEMANTIC,
    FAULT_INJECTION, HEADLINE_INSTANCES, PACKS_WITH_NO_FAMILY, SEMANTICS_PRESERVING,
};
use bioprism_interweave::packs::{
    compare, compare_dimension, Difficulty, DimensionComparison, Dominance, Measurement, Pack,
    ParentRequirement, ParentTask, ScoreDimension, Scorecard,
};
use std::collections::{BTreeMap, BTreeSet};

#[test]
fn a_semantics_preserving_mutation_requires_the_parents_verdict_unchanged() {
    let mutation = Mutation::preserving("rename agents and roles");
    assert_eq!(mutation.expectation, Expectation::Invariant);
    assert!(mutation.verdict_must_match_parent());
}

#[test]
fn a_controlled_semantic_mutation_cannot_exist_without_a_stated_consequence() {
    let mutation = Mutation::controlled("revoke a grant", "the delegate's next act is refused");
    assert!(!mutation.verdict_must_match_parent());
    match mutation.expectation {
        Expectation::Changes { to } => assert!(!to.is_empty()),
        other => panic!("expected a stated change, got {other:?}"),
    }
}

#[test]
fn a_fault_injection_expects_a_recovery_rather_than_an_answer() {
    let mutation = Mutation::fault("unavailable participant", "rebind the role and continue");
    assert!(!mutation.verdict_must_match_parent());
    assert!(matches!(
        mutation.expectation,
        Expectation::RecoveryRequired { .. }
    ));
}

#[test]
fn the_three_mutation_classes_have_the_counts_the_blueprint_lists() {
    assert_eq!(SEMANTICS_PRESERVING.len(), 7);
    assert_eq!(CONTROLLED_SEMANTIC.len(), 8);
    assert_eq!(FAULT_INJECTION.len(), 8);
}

#[test]
fn only_the_first_four_oracles_are_deterministic() {
    let deterministic: Vec<Oracle> = Oracle::ALL.into_iter().filter(|o| o.deterministic()).collect();
    assert_eq!(
        deterministic,
        vec![
            Oracle::ProtocolStateMachine,
            Oracle::TypeEffectAuthorityChecker,
            Oracle::DeterministicWorldState,
            Oracle::PropertyAndMetamorphic,
        ]
    );
}

#[test]
fn a_plan_running_a_model_judge_before_a_deterministic_oracle_is_refused() {
    let plan = OraclePlan::new([
        Oracle::CalibratedExpertOrModelJudge,
        Oracle::ProtocolStateMachine,
    ]);
    assert_eq!(
        plan.check_order(),
        Err(PlanDefect::JudgeBeforeDeterministic {
            judge: Oracle::CalibratedExpertOrModelJudge,
            deterministic: Oracle::ProtocolStateMachine,
        })
    );
}

#[test]
fn a_plan_in_hierarchy_order_is_admitted() {
    assert!(OraclePlan::new(Oracle::ALL).check_order().is_ok());
}

#[test]
fn a_plan_that_omits_a_deterministic_oracle_entirely_is_not_an_ordering_defect() {
    let plan = OraclePlan::new([Oracle::CalibratedExpertOrModelJudge]);
    assert!(plan.check_order().is_ok());
}

#[test]
fn an_empty_oracle_plan_is_refused() {
    assert_eq!(OraclePlan::new([]).check_order(), Err(PlanDefect::Empty));
}

fn account() -> ScaleAccount {
    ScaleAccount {
        parent_weave_programs: 300,
        parent_task_environments: 40,
        decision_cell_parents: 120,
        validated_generated_instances: 100_000,
        unique_mutation_relations: 23,
        effective_diversity: 88,
        executed_trials: 250_000,
        independent_human_audits: 12,
    }
}

#[test]
fn a_million_instances_from_three_hundred_parents_cannot_be_claimed_as_independent() {
    let mut inflated = account();
    inflated.validated_generated_instances = 1_000_000;
    assert_eq!(
        inflated.independent_claim(1_000_000),
        Err(ScaleRefusal::MoreIndependentThanParents {
            claimed: 1_000_000,
            parents: 300,
        })
    );
}

#[test]
fn an_independent_claim_within_the_parent_count_is_allowed() {
    assert_eq!(account().independent_claim(300), Ok(300));
    assert_eq!(account().independent_claim(1), Ok(1));
}

#[test]
fn an_account_with_no_audited_parents_supports_no_independent_claim_at_all() {
    let empty = ScaleAccount::default();
    assert_eq!(
        empty.independent_claim(1),
        Err(ScaleRefusal::NoAuditedParents)
    );
    assert_eq!(empty.instances_per_parent(), None);
}

#[test]
fn instances_per_parent_reports_the_amplification_the_account_hides() {
    assert_eq!(account().instances_per_parent(), Some(333));
}

#[test]
fn a_registry_above_the_headline_size_needs_validity_and_diversity_separately() {
    let mut large = account();
    large.validated_generated_instances = 1_000_000;
    assert!(matches!(
        admit_release(&large, 1_000_000, false, true),
        Err(ReleaseBlock::ValidityNotDemonstrated(_))
    ));
    assert!(matches!(
        admit_release(&large, 1_000_000, true, false),
        Err(ReleaseBlock::DiversityNotDemonstrated(_))
    ));
    assert!(admit_release(&large, 1_000_000, true, true).is_ok());
}

#[test]
fn the_headline_release_needs_neither_demonstration_because_it_is_the_starting_point() {
    assert!(admit_release(&account(), HEADLINE_INSTANCES, false, false).is_ok());
}

#[test]
fn a_release_cannot_claim_more_instances_than_were_validated() {
    assert_eq!(
        admit_release(&account(), 200_000, true, true),
        Err(ReleaseBlock::ClaimExceedsValidated {
            available: 100_000,
            claimed: 200_000,
        })
    );
}

fn instance(id: &str, parent: &str, weight: u32, safety: bool) -> Instance {
    Instance {
        id: id.into(),
        parent: parent.into(),
        pack: Pack::ActSemantics,
        weights: BTreeMap::from([(SelectionCriterion::RegressionSimilarity, weight)]),
        safety_mandatory: safety,
    }
}

#[test]
fn mandatory_safety_instances_are_selected_regardless_of_budget() {
    let registry = vec![
        instance("safety-1", "p1", 0, true),
        instance("safety-2", "p2", 0, true),
        instance("ordinary", "p1", 99, false),
    ];
    let chosen = select(&registry, &BTreeSet::new(), 0);
    assert_eq!(chosen, vec!["safety-1".to_string(), "safety-2".to_string()]);
}

#[test]
fn selection_draws_from_every_parent_before_drawing_twice_from_one() {
    let registry = vec![
        instance("a1", "p1", 100, false),
        instance("a2", "p1", 99, false),
        instance("b1", "p2", 1, false),
    ];
    let chosen = select(
        &registry,
        &BTreeSet::from([SelectionCriterion::RegressionSimilarity]),
        2,
    );
    assert_eq!(chosen, vec!["a1".to_string(), "b1".to_string()]);
}

#[test]
fn selection_is_deterministic_across_repeated_calls() {
    let registry = vec![
        instance("a1", "p1", 5, false),
        instance("b1", "p2", 5, false),
        instance("c1", "p3", 5, false),
    ];
    let criteria = BTreeSet::from([SelectionCriterion::RegressionSimilarity]);
    let first = select(&registry, &criteria, 2);
    let second = select(&registry, &criteria, 2);
    assert_eq!(first, second);
}

#[test]
fn the_seven_families_are_not_a_cover_of_the_twelve_packs() {
    let covered: BTreeSet<Pack> = Family::ALL
        .into_iter()
        .flat_map(|family| family.packs().iter().copied())
        .collect();
    let uncovered: BTreeSet<Pack> = Pack::ALL
        .into_iter()
        .filter(|pack| !covered.contains(pack))
        .collect();
    assert_eq!(uncovered, PACKS_WITH_NO_FAMILY.into_iter().collect());
    assert_eq!(uncovered.len(), 3);
}

#[test]
fn every_pack_reachable_from_a_family_is_reachable_from_exactly_one() {
    for pack in Pack::ALL {
        let owners = Family::ALL
            .into_iter()
            .filter(|family| family.packs().contains(&pack))
            .count();
        assert!(owners <= 1, "{pack:?} is claimed by {owners} families");
    }
}

#[test]
fn levels_below_l2_can_be_exercised_by_one_participant_and_the_rest_cannot() {
    let single: Vec<Level> = Level::ALL
        .into_iter()
        .filter(|l| l.single_participant())
        .collect();
    assert_eq!(single, vec![Level::L0, Level::L1]);
}

#[test]
fn every_pack_lists_at_least_five_items() {
    for pack in Pack::ALL {
        assert!(pack.items().len() >= 5, "{pack:?} lists too few items");
    }
}

#[test]
fn pack_numbers_are_one_through_twelve_without_a_gap() {
    let numbers: BTreeSet<u8> = Pack::ALL.into_iter().map(Pack::number).collect();
    assert_eq!(numbers, (1u8..=12).collect());
}

#[test]
fn a_scorecard_with_an_unmeasured_dimension_cannot_be_compared_at_all() {
    let complete = ScoreDimension::ALL
        .into_iter()
        .fold(Scorecard::new("left"), |card, dimension| {
            card.scoring(dimension, Measurement::measured(5000))
        });
    let partial = Scorecard::new("right")
        .scoring(ScoreDimension::TaskUtility, Measurement::measured(9000));
    match compare(&complete, &partial) {
        Dominance::Undetermined { unmeasured } => assert_eq!(unmeasured.len(), 9),
        other => panic!("expected undetermined, got {other:?}"),
    }
}

fn full_card(name: &str, value: u16) -> Scorecard {
    ScoreDimension::ALL
        .into_iter()
        .fold(Scorecard::new(name), |card, dimension| {
            card.scoring(dimension, Measurement::measured(value))
        })
}

#[test]
fn a_system_better_on_one_dimension_and_worse_on_another_is_incomparable() {
    let left = full_card("left", 5000)
        .scoring(ScoreDimension::EpistemicQuality, Measurement::measured(9000));
    let right = full_card("right", 5000)
        .scoring(ScoreDimension::CostAndLatency, Measurement::measured(9000));
    match compare(&left, &right) {
        Dominance::Incomparable {
            left_better,
            right_better,
        } => {
            assert_eq!(
                left_better,
                BTreeSet::from([ScoreDimension::EpistemicQuality])
            );
            assert_eq!(
                right_better,
                BTreeSet::from([ScoreDimension::CostAndLatency])
            );
        }
        other => panic!("expected incomparable, got {other:?}"),
    }
}

#[test]
fn a_system_at_least_as_good_everywhere_and_better_somewhere_dominates() {
    let left = full_card("left", 5000)
        .scoring(ScoreDimension::Recovery, Measurement::measured(6000));
    let right = full_card("right", 5000);
    assert_eq!(compare(&left, &right), Dominance::LeftDominates);
    assert_eq!(compare(&right, &left), Dominance::RightDominates);
}

#[test]
fn two_identical_scorecards_are_equivalent_rather_than_either_dominating() {
    assert_eq!(
        compare(&full_card("a", 4000), &full_card("b", 4000)),
        Dominance::Equivalent
    );
}

#[test]
fn a_dimension_never_recorded_reads_as_unmeasured_rather_than_zero() {
    let card = Scorecard::new("fresh");
    assert_eq!(card.unmeasured().len(), 10);
    assert_eq!(
        card.score(ScoreDimension::Calibration),
        Measurement::Unmeasured
    );
    assert_eq!(
        compare_dimension(&card, &full_card("other", 1), ScoreDimension::Calibration),
        DimensionComparison::Undetermined
    );
}

#[test]
fn difficulty_dominance_is_partial_so_trading_dimensions_orders_neither_way() {
    let wide = Difficulty {
        number_of_roles: 6,
        adversariality: 0,
        ..Difficulty::default()
    };
    let hostile = Difficulty {
        number_of_roles: 2,
        adversariality: 5,
        ..Difficulty::default()
    };
    assert!(!wide.harder_than(&hostile));
    assert!(!hostile.harder_than(&wide));
}

#[test]
fn difficulty_dominance_holds_when_one_instance_is_at_least_as_hard_everywhere() {
    let base = Difficulty {
        number_of_roles: 2,
        ..Difficulty::default()
    };
    let harder = Difficulty {
        number_of_roles: 3,
        adversariality: 1,
        ..Difficulty::default()
    };
    assert!(harder.harder_than(&base));
    assert!(!base.harder_than(&harder));
    assert!(!base.harder_than(&base));
}

#[test]
fn difficulty_reports_all_twelve_of_its_dimensions() {
    assert_eq!(Difficulty::default().components().len(), 12);
}

fn two_role_protocol() -> bioprism_choreography::WellFormedGlobal {
    GlobalType::message(
        Role::new("lead"),
        Role::new("skeptic"),
        "challenge",
        GlobalType::End,
    )
    .well_formed()
    .expect("a single message between two roles is well formed")
}

#[test]
fn a_parent_task_that_supplies_nothing_owes_every_documentary_requirement() {
    let parent = ParentTask::new(
        "p1",
        Pack::ActSemantics,
        Difficulty::default(),
        two_role_protocol(),
    );
    let missing = parent.missing();
    assert!(!missing.contains(&ParentRequirement::GlobalChoreography));
    assert!(!missing.contains(&ParentRequirement::ParticipantLocalViews));
    assert_eq!(missing.len(), 8);
    assert!(parent.admit().is_err());
}

#[test]
fn a_parent_task_supplying_everything_documentary_is_admitted() {
    let parent = ParentRequirement::ALL.into_iter().fold(
        ParentTask::new(
            "p1",
            Pack::Topology,
            Difficulty::default(),
            two_role_protocol(),
        ),
        ParentTask::supplying,
    );
    assert_eq!(parent.admit(), Ok(()));
}

#[test]
fn a_parent_tasks_local_views_are_derived_from_its_choreography_rather_than_declared() {
    let parent = ParentTask::new(
        "p1",
        Pack::ActSemantics,
        Difficulty::default(),
        two_role_protocol(),
    );
    assert_eq!(
        parent.roles(),
        BTreeSet::from([Role::new("lead"), Role::new("skeptic")])
    );
}
