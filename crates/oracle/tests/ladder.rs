//! The evidence ladder and the 11.11 override invariant (31.01, 31.14, 40.21 invariant 2).

mod common;

use bioprism_oracle::{Determinism, EvidenceTier, Position};
use common::{circular_judgement, judgement};

#[test]
fn the_ladder_ranks_deterministic_above_execution_above_property_above_statistical_above_judge() {
    assert!(EvidenceTier::Deterministic > EvidenceTier::Execution);
    assert!(EvidenceTier::Execution > EvidenceTier::Property);
    assert!(EvidenceTier::Property > EvidenceTier::Statistical);
    assert!(EvidenceTier::Statistical > EvidenceTier::Judge);

    let mut sorted = EvidenceTier::ALL;
    sorted.sort();
    assert_eq!(sorted, EvidenceTier::ALL, "ALL is declared weakest-first");
}

#[test]
fn a_judge_may_not_override_a_deterministic_tier() {
    assert!(!EvidenceTier::Judge.may_override(EvidenceTier::Deterministic));
}

#[test]
fn a_judge_may_not_override_an_execution_grounded_tier() {
    assert!(!EvidenceTier::Judge.may_override(EvidenceTier::Execution));
}

#[test]
fn a_statistical_oracle_may_not_override_a_grounded_tier_either() {
    assert!(!EvidenceTier::Statistical.may_override(EvidenceTier::Deterministic));
    assert!(!EvidenceTier::Statistical.may_override(EvidenceTier::Execution));
}

#[test]
fn a_property_oracle_may_not_override_a_deterministic_one() {
    assert!(!EvidenceTier::Property.may_override(EvidenceTier::Deterministic));
    assert!(EvidenceTier::Deterministic.may_override(EvidenceTier::Property));
}

#[test]
fn an_equal_tier_is_not_an_override_but_a_disagreement() {
    for tier in EvidenceTier::ALL {
        assert!(
            tier.may_override(tier),
            "{tier} against itself is a same-tier disagreement, resolved by set-valued \
             combination rather than by precedence"
        );
    }
}

#[test]
fn the_override_rule_never_consults_confidence() {
    let certain_judge = judgement("judge", EvidenceTier::Judge, Position::Supported, 1.0);
    let doubtful_checker = judgement(
        "schema",
        EvidenceTier::Deterministic,
        Position::Contradicted,
        0.01,
    );

    assert!(!certain_judge.tier.may_override(doubtful_checker.tier));
    assert!(doubtful_checker.tier.may_override(certain_judge.tier));
}

#[test]
fn determinism_and_tier_are_independent_axes() {
    assert_eq!(
        EvidenceTier::Property.determinism(),
        Determinism::Reproducible,
        "a property oracle recomputes; it ranks lower because a pass establishes less, not \
         because it is stochastic"
    );
    assert!(!EvidenceTier::Property.is_grounded());
    assert_eq!(
        EvidenceTier::Statistical.determinism(),
        Determinism::Nondeterministic
    );
}

#[test]
fn a_circular_oracle_is_demoted_one_rung() {
    let demoted = circular_judgement(
        "schema",
        EvidenceTier::Deterministic,
        Position::Supported,
        1.0,
    );
    assert_eq!(demoted.declared_tier, EvidenceTier::Deterministic);
    assert_eq!(demoted.tier, EvidenceTier::Execution);
    assert!(demoted.was_demoted());
}

#[test]
fn a_circular_reproducible_oracle_does_not_demote_into_a_nondeterministic_tier() {
    let demoted = circular_judgement("props", EvidenceTier::Property, Position::Supported, 1.0);
    assert_eq!(
        demoted.tier,
        EvidenceTier::Property,
        "circularity weakens what a result shows; it does not make arithmetic stochastic"
    );
    assert_eq!(demoted.tier.determinism(), Determinism::Reproducible);
}

#[test]
fn a_circular_judge_cannot_be_demoted_below_the_bottom_rung() {
    let demoted = circular_judgement("judge", EvidenceTier::Judge, Position::Supported, 1.0);
    assert_eq!(demoted.tier, EvidenceTier::Judge);
}
