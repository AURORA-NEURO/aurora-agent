//! Blueprint 43.30 (continuations), 43.32 (query patterns) and 43.47 (the guarantee gate).

use bioprism_epistemic::continuation::{conservation, rebase, Checkpoint, ConservationBreach, Rebase};
use bioprism_epistemic::decision::{Belief, DecisionProblem};
use bioprism_epistemic::evidence::{EvidenceItem, EvidencePool};
use bioprism_epistemic::patterns::{
    clinical_boundary_violations, oracle_tier_inconsistencies, wire_gap, OracleTier, PATTERNS,
};
use bioprism_epistemic::ratedistortion::DistortionCriterion;
use bioprism_epistemic::theorem::{Guarantee, COUNTEREXAMPLES};
use bioprism_epistemic::NOT_IMPLEMENTED;
use std::collections::BTreeSet;

const FLOOR: f64 = 0.01;

fn problem() -> DecisionProblem {
    DecisionProblem::new(
        vec!["call_present".into(), "call_absent".into()],
        vec!["present".into(), "absent".into()],
        vec![0.0, 1.0, 1.0, 0.0],
    )
    .expect("well-formed")
}

fn checkpoint(action: usize, retained: Vec<usize>, tolerance: f64) -> Checkpoint {
    Checkpoint {
        query_id: "q-cohort-integrity-0001".into(),
        section_digest: "0".repeat(64),
        world_cut: 10,
        schema_version: "fiber-world/0.1".into(),
        action,
        tolerance,
        retained,
        authority: BTreeSet::from(["read_cohort".to_string()]),
        budget_remaining: 100.0,
    }
}

fn pool(likelihoods: &[[f64; 2]]) -> EvidencePool {
    EvidencePool::new(
        likelihoods
            .iter()
            .enumerate()
            .map(|(i, l)| EvidenceItem::new(format!("e{i}"), 1.0, l.to_vec()).expect("item"))
            .collect(),
    )
    .expect("pool")
}

#[test]
fn a_stale_world_cut_is_rejected_before_any_regret_is_computed() {
    let outcome = rebase(
        &checkpoint(0, vec![0], 0.1),
        9,
        "fiber-world/0.1",
        &problem(),
        &Belief::uniform(2).expect("uniform"),
        &pool(&[[0.9, 0.1]]),
        DistortionCriterion::BayesRegret,
        FLOOR,
    )
    .expect("classifiable");

    assert!(matches!(outcome, Rebase::Stale { .. }));
    assert!(!outcome.resumable(), "stale continuations never silently execute");
}

#[test]
fn a_schema_change_invalidates_rather_than_stales() {
    let outcome = rebase(
        &checkpoint(0, vec![0], 0.1),
        11,
        "fiber-world/0.2",
        &problem(),
        &Belief::uniform(2).expect("uniform"),
        &pool(&[[0.9, 0.1]]),
        DistortionCriterion::BayesRegret,
        FLOOR,
    )
    .expect("classifiable");

    assert!(
        matches!(outcome, Rebase::Invalid { .. }),
        "a version-key mismatch means the decision-theoretic question is not even well posed"
    );
}

#[test]
fn a_world_delta_that_leaves_the_decision_alone_is_equivalent() {
    let outcome = rebase(
        &checkpoint(0, vec![0], 0.1),
        11,
        "fiber-world/0.1",
        &problem(),
        &Belief::uniform(2).expect("uniform"),
        &pool(&[[0.95, 0.05], [0.9, 0.1]]),
        DistortionCriterion::BayesRegret,
        FLOOR,
    )
    .expect("classifiable");

    assert!(matches!(outcome, Rebase::Equivalent { .. }), "{outcome:?}");
    assert!(outcome.resumable() && outcome.preserves_the_decision());
}

#[test]
fn a_world_delta_that_changes_the_action_obstructs_the_rebase() {
    let outcome = rebase(
        &checkpoint(0, vec![0], 0.5),
        11,
        "fiber-world/0.1",
        &problem(),
        &Belief::uniform(2).expect("uniform"),
        &pool(&[[0.05, 0.95], [0.5, 0.5]]),
        DistortionCriterion::BayesRegret,
        FLOOR,
    )
    .expect("classifiable");

    match outcome {
        Rebase::Obstructed {
            sealed_action,
            current_action,
            ..
        } => {
            assert_eq!(sealed_action, 0);
            assert_ne!(current_action, 0);
        }
        other => panic!("resuming would execute an unsupported decision: {other:?}"),
    }
}

#[test]
fn a_world_delta_that_keeps_the_action_but_breaks_tolerance_is_refined_not_obstructed() {
    let outcome = rebase(
        &checkpoint(1, vec![0], 0.0),
        11,
        "fiber-world/0.1",
        &problem(),
        &Belief::uniform(2).expect("uniform"),
        &pool(&[[0.1, 0.9], [0.99, 0.01], [0.99, 0.01]]),
        DistortionCriterion::BayesRegret,
        FLOOR,
    )
    .expect("classifiable");

    match outcome {
        Rebase::Refined {
            action,
            distortion,
            tolerance,
        } => {
            assert_eq!(action, 1);
            assert!(distortion > tolerance);
        }
        other => panic!("expected a refinement obligation, got {other:?}"),
    }
    assert!(
        matches!(
            rebase(
                &checkpoint(1, vec![0], 0.0),
                11,
                "fiber-world/0.1",
                &problem(),
                &Belief::uniform(2).expect("uniform"),
                &pool(&[[0.1, 0.9], [0.99, 0.01], [0.99, 0.01]]),
                DistortionCriterion::BayesRegret,
                FLOOR,
            )
            .expect("classifiable"),
            Rebase::Refined { .. }
        ),
        "classification must be deterministic across runs"
    );
}

#[test]
fn a_retained_index_that_no_longer_exists_invalidates_rather_than_resuming_on_less() {
    let outcome = rebase(
        &checkpoint(0, vec![0, 7], 0.1),
        11,
        "fiber-world/0.1",
        &problem(),
        &Belief::uniform(2).expect("uniform"),
        &pool(&[[0.9, 0.1]]),
        DistortionCriterion::BayesRegret,
        FLOOR,
    )
    .expect("classifiable");
    assert!(matches!(outcome, Rebase::Invalid { .. }));
}

#[test]
fn a_child_holding_a_capability_its_parent_lacked_is_a_conservation_breach() {
    let parent = checkpoint(0, vec![0], 0.1);
    let mut child = checkpoint(0, vec![0], 0.1);
    child.query_id = "q-child".into();
    child.authority.insert("write_cohort".to_string());
    child.budget_remaining = 10.0;

    let breaches = conservation(&parent, &[child]);
    assert!(breaches.iter().any(|b| matches!(
        b,
        ConservationBreach::AuthorityExpanded { .. }
    )));
}

#[test]
fn children_budgets_summing_above_the_parent_is_a_conservation_breach() {
    let parent = checkpoint(0, vec![0], 0.1);
    let mut first = checkpoint(0, vec![0], 0.1);
    first.query_id = "q-a".into();
    first.budget_remaining = 70.0;
    let mut second = checkpoint(0, vec![0], 0.1);
    second.query_id = "q-b".into();
    second.budget_remaining = 70.0;

    let breaches = conservation(&parent, &[first, second]);
    match breaches
        .iter()
        .find(|b| matches!(b, ConservationBreach::BudgetOverdrawn { .. }))
    {
        Some(ConservationBreach::BudgetOverdrawn { children, parent }) => {
            assert!(children > parent);
        }
        other => panic!("expected an overdraw with both numbers, got {other:?}"),
    }
}

#[test]
fn a_faithful_fork_reports_no_conservation_breach() {
    let parent = checkpoint(0, vec![0], 0.1);
    let mut first = checkpoint(0, vec![0], 0.1);
    first.query_id = "q-a".into();
    first.budget_remaining = 40.0;
    let mut second = checkpoint(0, vec![0], 0.1);
    second.query_id = "q-b".into();
    second.budget_remaining = 40.0;

    assert!(conservation(&parent, &[first, second]).is_empty());
}

#[test]
fn no_query_pattern_can_round_trip_through_fiber_query_0_1() {
    let gaps = wire_gap();
    assert_eq!(gaps.len(), PATTERNS.len());
    assert!(
        gaps.iter().all(|gap| !gap.round_trips()),
        "the measurement is that none of them survive the wire format, not that some need an \
         extension: {:?}",
        gaps.iter()
            .filter(|g| g.round_trips())
            .map(|g| &g.pattern)
            .collect::<Vec<_>>()
    );
    assert!(gaps
        .iter()
        .all(|gap| gap.unrepresentable.contains(&"permitted_actions".to_string())));
}

#[test]
fn every_pattern_declares_the_actions_it_is_choosing_between() {
    for pattern in PATTERNS {
        assert!(
            pattern.permitted_actions.len() >= 2,
            "{} names fewer than two actions, so it is not a decision",
            pattern.id
        );
        assert!(!pattern.allowed_outputs.is_empty());
        assert!(!pattern.protected_closure.is_empty());
    }
}

#[test]
fn no_pattern_allows_a_clinical_output() {
    let violations = clinical_boundary_violations();
    assert!(
        violations.is_empty(),
        "the non-clinical boundary is a checked field, not a paragraph: {violations:?}"
    );
}

#[test]
fn a_pattern_without_an_adequate_oracle_stays_at_the_synthetic_tier() {
    assert!(
        oracle_tier_inconsistencies().is_empty(),
        "43.32: no adequate oracle keeps a pattern experimental"
    );
    assert!(
        PATTERNS.iter().any(|p| p.oracle == OracleTier::None),
        "the registry must contain at least one honestly experimental pattern, or the rule is \
         untested"
    );
}

#[test]
fn every_counterexample_in_the_corpus_names_a_guarantee_this_crate_can_actually_make() {
    assert!(!COUNTEREXAMPLES.is_empty());
    for counterexample in COUNTEREXAMPLES {
        assert!(!counterexample.construction.is_empty());
        assert!(!counterexample.required_behaviour.is_empty());
        assert!(
            matches!(
                counterexample.guarantee,
                Guarantee::GreedyCardinality
                    | Guarantee::SeparatorCompositionExact
                    | Guarantee::LensLawful
                    | Guarantee::DecisionSufficiency
            ),
            "a counterexample against a guarantee nothing can claim is a coverage number wearing \
             a type"
        );
        assert!(!counterexample.guarantee.blueprint_module().is_empty());
    }
}

#[test]
fn the_not_implemented_list_names_the_causal_identification_gap_explicitly() {
    assert!(
        NOT_IMPLEMENTED
            .iter()
            .any(|entry| entry.contains("do-calculus")),
        "the most confusable omission in this crate has to be stated, not implied"
    );
    assert!(NOT_IMPLEMENTED.len() >= 10);
}
