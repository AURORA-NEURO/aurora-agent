//! Invariants of 06.07 state and context minimization.
//!
//! The claims under test are about *correctness of the reduction*, not about how small it gets. A
//! minimizer that always returns the input is useless; one that returns something smaller which no
//! longer demonstrates the finding is actively harmful, and these tests are weighted accordingly.

use bioprism_benchcompiler::minimize::{
    minimize, minimize_preserving, ContextItem, InterestProbe, InterestSignature, MinimizeBudget,
    Tier,
};
use bioprism_benchcompiler::MinimizeError;
use std::collections::BTreeSet;

fn interesting() -> InterestSignature {
    InterestSignature::new("invalid").with_witness("identity_leakage")
}

fn boring() -> InterestSignature {
    InterestSignature::new("valid")
}

/// Interesting exactly when every named item is still present.
struct RequiresAll(Vec<&'static str>);

impl InterestProbe for RequiresAll {
    fn observe(&mut self, kept: &BTreeSet<String>) -> InterestSignature {
        if self.0.iter().all(|id| kept.contains(*id)) {
            interesting()
        } else {
            boring()
        }
    }
}

fn item(id: &str, tier: Tier) -> ContextItem {
    ContextItem::new(id, tier)
}

#[test]
fn a_minimization_that_loses_the_preserved_property_is_an_error_not_a_smaller_cell() {
    struct DriftsAfterFiveProbes(usize);
    impl InterestProbe for DriftsAfterFiveProbes {
        fn observe(&mut self, _kept: &BTreeSet<String>) -> InterestSignature {
            self.0 += 1;
            if self.0 <= 4 {
                interesting()
            } else {
                boring()
            }
        }
    }

    let items = vec![item("a", Tier::Field), item("b", Tier::Field)];
    let error = minimize(
        &items,
        &mut DriftsAfterFiveProbes(0),
        MinimizeBudget::default(),
    )
    .expect_err("the independent re-check must catch a reduction that stopped being interesting");

    match error {
        MinimizeError::PropertyLost { expected, observed } => {
            assert_eq!(expected, interesting().describe());
            assert_eq!(observed, boring().describe());
        }
        other => panic!("expected PropertyLost, got {other:?}"),
    }
}

#[test]
fn every_item_remaining_after_minimization_has_a_recorded_probe_proving_it_load_bearing() {
    let items = vec![
        item("keep_one", Tier::Field),
        item("keep_two", Tier::Field),
        item("noise_a", Tier::Field),
        item("noise_b", Tier::Field),
    ];
    let result = minimize(
        &items,
        &mut RequiresAll(vec!["keep_one", "keep_two"]),
        MinimizeBudget::default(),
    )
    .expect("the full context is interesting, so minimization runs");

    assert_eq!(result.minimal, vec!["keep_one", "keep_two"]);
    assert_eq!(result.minimality_witnesses.len(), result.minimal.len());
    for witness in &result.minimality_witnesses {
        assert_ne!(
            witness.observed_without, result.preserved,
            "a witness that reproduces the preserved signature proves the opposite of minimality"
        );
    }
}

#[test]
fn one_minimality_is_claimed_and_global_minimality_is_not() {
    struct BothOrNeither;
    impl InterestProbe for BothOrNeither {
        fn observe(&mut self, kept: &BTreeSet<String>) -> InterestSignature {
            if kept.contains("a") == kept.contains("b") {
                interesting()
            } else {
                boring()
            }
        }
    }

    let items = vec![
        item("a", Tier::Field),
        item("b", Tier::Field),
        item("c", Tier::Field),
    ];
    let result = minimize(&items, &mut BothOrNeither, MinimizeBudget::default())
        .expect("the full context is interesting");

    assert_eq!(result.minimal, vec!["a", "b"]);
    assert!(
        result.guarantee.contains("Not globally minimal"),
        "the empty set is also interesting here; the guarantee must not overstate"
    );
}

#[test]
fn a_removal_the_first_pass_could_not_make_is_made_after_a_later_removal_unlocks_it() {
    struct AImpliesX;
    impl InterestProbe for AImpliesX {
        fn observe(&mut self, kept: &BTreeSet<String>) -> InterestSignature {
            let anchor = kept.contains("anchor");
            let implication = !kept.contains("y") || kept.contains("x");
            if anchor && implication {
                interesting()
            } else {
                boring()
            }
        }
    }

    let items = vec![
        item("anchor", Tier::Field),
        item("x", Tier::Field),
        item("y", Tier::Field),
    ];
    let result = minimize(&items, &mut AImpliesX, MinimizeBudget::default())
        .expect("the full context is interesting");

    assert_eq!(result.minimal, vec!["anchor"]);
    assert!(
        result.passes >= 2,
        "x only became removable once y was gone; a single-pass minimizer would have kept it"
    );
}

#[test]
fn an_item_pinned_to_task_intent_survives_even_when_the_probe_does_not_need_it() {
    let items = vec![
        item("load_bearing", Tier::Field),
        item("states_the_question", Tier::Field).pinned(),
        item("noise", Tier::Field),
    ];
    let result = minimize(
        &items,
        &mut RequiresAll(vec!["load_bearing"]),
        MinimizeBudget::default(),
    )
    .expect("the full context is interesting");

    assert_eq!(result.minimal, vec!["load_bearing", "states_the_question"]);
    assert_eq!(result.pinned, vec!["states_the_question"]);
    assert!(
        result
            .minimality_witnesses
            .iter()
            .all(|witness| witness.unit_root != "states_the_question"),
        "a pinned item is retained by the guard, so it must not be counted as load-bearing evidence"
    );
}

#[test]
fn a_container_holding_a_pinned_item_is_never_removed_as_a_unit() {
    let items = vec![
        item("service", Tier::Service),
        item("artifact", Tier::Artifact).inside("service").pinned(),
        item("anchor", Tier::Field),
    ];
    let result = minimize(
        &items,
        &mut RequiresAll(vec!["anchor"]),
        MinimizeBudget::default(),
    )
    .expect("the full context is interesting");

    assert!(
        result.minimal.contains(&"service".to_string()),
        "removing the container would have taken the pinned artifact with it"
    );
    assert!(result.minimal.contains(&"artifact".to_string()));
}

#[test]
fn removing_a_container_retires_everything_inside_it_in_one_probe() {
    let items = vec![
        item("unrelated_service", Tier::Service),
        item("doc_one", Tier::Artifact).inside("unrelated_service"),
        item("doc_two", Tier::Artifact).inside("unrelated_service"),
        item("field_one", Tier::Field).inside("doc_one"),
        item("anchor", Tier::Field),
    ];
    let result = minimize(
        &items,
        &mut RequiresAll(vec!["anchor"]),
        MinimizeBudget::default(),
    )
    .expect("the full context is interesting");

    assert_eq!(result.minimal, vec!["anchor"]);
    assert_eq!(result.removed.len(), 4);
    assert!(
        result.evaluations < 10,
        "coarse-first removal should retire the subtree in one probe, not one probe per field"
    );
}

#[test]
fn a_nondeterministic_probe_is_rejected_before_any_removal_is_attempted() {
    struct AlternatesEveryCall(bool);
    impl InterestProbe for AlternatesEveryCall {
        fn observe(&mut self, _kept: &BTreeSet<String>) -> InterestSignature {
            self.0 = !self.0;
            if self.0 {
                interesting()
            } else {
                boring()
            }
        }
    }

    let items = vec![item("a", Tier::Field), item("b", Tier::Field)];
    let error = minimize(&items, &mut AlternatesEveryCall(false), MinimizeBudget::default())
        .expect_err("delta debugging over a coin flip is not a reduction");

    assert!(matches!(
        error,
        MinimizeError::NondeterministicProbe { size: 2, .. }
    ));
}

#[test]
fn minimizing_a_context_that_never_showed_the_property_is_an_error() {
    let items = vec![item("a", Tier::Field)];
    let error = minimize_preserving(
        &items,
        &mut RequiresAll(vec!["absent"]),
        &interesting(),
        MinimizeBudget::default(),
    )
    .expect_err("a smaller context that still does not show the finding is not progress");

    assert!(matches!(
        error,
        MinimizeError::NotInterestingToBeginWith { .. }
    ));
}

#[test]
fn minimization_is_deterministic_across_repeated_runs() {
    let items = vec![
        item("z", Tier::Field),
        item("m", Tier::Artifact),
        item("a", Tier::Service),
        item("k", Tier::Field),
    ];
    let first = minimize(
        &items,
        &mut RequiresAll(vec!["k", "m"]),
        MinimizeBudget::default(),
    )
    .expect("interesting");
    let second = minimize(
        &items,
        &mut RequiresAll(vec!["k", "m"]),
        MinimizeBudget::default(),
    )
    .expect("interesting");

    assert_eq!(first, second);
    assert_eq!(first.evaluations, second.evaluations);
}

#[test]
fn an_empty_candidate_context_is_an_error_not_an_empty_minimization() {
    let error = minimize(&[], &mut RequiresAll(vec![]), MinimizeBudget::default())
        .expect_err("there is nothing to reduce");
    assert_eq!(error, MinimizeError::NothingToMinimize);
}

#[test]
fn a_dangling_containment_parent_is_rejected_rather_than_ignored() {
    let items = vec![item("child", Tier::Field).inside("nowhere")];
    let error = minimize(&items, &mut RequiresAll(vec![]), MinimizeBudget::default())
        .expect_err("an item inside something absent means the caller lost part of the context");
    assert!(matches!(error, MinimizeError::DanglingParent { .. }));
}

#[test]
fn a_containment_cycle_is_rejected_rather_than_looped_over() {
    let items = vec![
        item("a", Tier::Field).inside("b"),
        item("b", Tier::Field).inside("a"),
    ];
    let error = minimize(&items, &mut RequiresAll(vec![]), MinimizeBudget::default())
        .expect_err("containment must be a forest");
    assert!(matches!(error, MinimizeError::CyclicContainment { .. }));
}

#[test]
fn exhausting_the_evaluation_budget_is_an_error_not_a_partial_result() {
    let items = vec![item("a", Tier::Field), item("b", Tier::Field)];
    let error = minimize(
        &items,
        &mut RequiresAll(vec!["a"]),
        MinimizeBudget { max_evaluations: 1 },
    )
    .expect_err("a truncated reduction has no minimality claim to make");
    assert!(matches!(
        error,
        MinimizeError::BudgetExhausted { budget: 1, .. }
    ));
}

#[test]
fn the_preserved_signature_separates_same_verdict_from_same_reason() {
    struct WitnessDependsOnB;
    impl InterestProbe for WitnessDependsOnB {
        fn observe(&mut self, kept: &BTreeSet<String>) -> InterestSignature {
            let base = InterestSignature::new("invalid");
            if kept.contains("b") {
                base.with_witness("identity_leakage")
            } else {
                base.with_witness("site_confound")
            }
        }
    }

    let items = vec![item("a", Tier::Field), item("b", Tier::Field)];
    let result = minimize(&items, &mut WitnessDependsOnB, MinimizeBudget::default())
        .expect("the full context is interesting");

    assert_eq!(result.minimal, vec!["b"]);
    assert!(
        result.preserved.witnesses.contains("identity_leakage"),
        "the verdict is unchanged without b, so only the witness set can have held b in place"
    );
}

#[test]
fn a_reduction_reports_its_own_reduction_ratio_and_survives_an_independent_recheck() {
    let items: Vec<ContextItem> = (0..10)
        .map(|index| item(&format!("item_{index:02}"), Tier::Field))
        .collect();
    let result = minimize(
        &items,
        &mut RequiresAll(vec!["item_03"]),
        MinimizeBudget::default(),
    )
    .expect("interesting");

    assert_eq!(result.minimal, vec!["item_03"]);
    assert!((result.reduction_ratio() - 0.1).abs() < 1e-9);
    assert!(result.preserves_under(&mut RequiresAll(vec!["item_03"])));
    assert!(!result.preserves_under(&mut RequiresAll(vec!["item_07"])));
}
