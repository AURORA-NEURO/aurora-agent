//! The laws a domain claims, and the registry's refusal to let two of them mix.
//!
//! Two grades of evidence live here and the suite keeps them apart, because
//! `crate::bruteforce`'s distinction between an exhaustive removal and a falsification search over a
//! continuum is the same distinction and the crate would be inconsistent to make it once.
//!
//! - [`bioprism_influence::SupportDomain`] has a finite concretisation and a universe that realises
//!   every sign pattern, so a law checked over it is checked over every equivalence class `γ` can
//!   distinguish. Those tests are proofs and say so in their names.
//! - The two interval domains concretise to the non-negative reals. Their universes are grids, and
//!   a law checked over a grid is a falsification search: a failure is a counterexample, a pass is
//!   evidence.

use bioprism_influence::domain::laws;
use bioprism_influence::domains::displacement::{self, Displacement, DisplacementDomain};
use bioprism_influence::domains::product::ProductDomain;
use bioprism_influence::domains::ratio_interval::{RatioInterval, RatioIntervalDomain};
use bioprism_influence::domains::support::{EntrySign, Support, SupportDomain};
use bioprism_influence::registry::{AbstractValue, DomainRegistry};
use bioprism_influence::{AbstractDomain, DomainError, DomainId, EnumerableConcretisation, FactClass};

fn ratio_elements() -> Vec<RatioInterval> {
    vec![
        RatioInterval::Bottom,
        RatioInterval::identity(),
        RatioInterval::range(0.0, 0.0).unwrap(),
        RatioInterval::range(0.5, 2.0).unwrap(),
        RatioInterval::range(0.25, 0.75).unwrap(),
        RatioInterval::range(2.0, 8.0).unwrap(),
        RatioInterval::range(0.0, f64::INFINITY).unwrap(),
    ]
}

fn displacement_elements() -> Vec<Displacement> {
    vec![
        Displacement::Bottom,
        Displacement::exactly(0.0).unwrap(),
        Displacement::at_most(0.25).unwrap(),
        Displacement::range(0.125, 0.5).unwrap(),
        Displacement::range(0.75, 3.0).unwrap(),
        Displacement::range(0.0, f64::INFINITY).unwrap(),
    ]
}

fn support_elements(length: usize) -> Vec<Support> {
    let mut elements = vec![Support::Bottom, Support::unknown(length)];
    for pattern in [
        vec![EntrySign::Zero, EntrySign::Positive, EntrySign::Positive],
        vec![EntrySign::Positive, EntrySign::Positive, EntrySign::Positive],
        vec![EntrySign::Zero, EntrySign::Zero, EntrySign::Either],
        vec![EntrySign::Positive, EntrySign::Either, EntrySign::Zero],
    ] {
        elements.push(Support::Pattern {
            signs: pattern.into_iter().take(length).collect(),
        });
    }
    elements
}

#[test]
fn join_is_an_upper_bound_commutative_idempotent_and_associative_in_every_shipped_domain() {
    let ratio = RatioIntervalDomain;
    for left in ratio_elements() {
        assert!(laws::join_is_idempotent(&ratio, &left));
        for right in ratio_elements() {
            assert!(laws::join_is_an_upper_bound(&ratio, &left, &right));
            assert!(laws::join_is_commutative(&ratio, &left, &right));
            assert!(laws::meet_is_a_lower_bound(&ratio, &left, &right));
            for third in ratio_elements() {
                assert!(laws::join_is_associative(&ratio, &left, &right, &third));
            }
        }
    }

    let answer = DisplacementDomain;
    for left in displacement_elements() {
        assert!(laws::join_is_idempotent(&answer, &left));
        for right in displacement_elements() {
            assert!(laws::join_is_an_upper_bound(&answer, &left, &right));
            assert!(laws::join_is_commutative(&answer, &left, &right));
            assert!(laws::meet_is_a_lower_bound(&answer, &left, &right));
            for third in displacement_elements() {
                assert!(laws::join_is_associative(&answer, &left, &right, &third));
            }
        }
    }

    let support = SupportDomain::of_length(3);
    for left in support_elements(3) {
        assert!(laws::join_is_idempotent(&support, &left));
        for right in support_elements(3) {
            assert!(laws::join_is_an_upper_bound(&support, &left, &right));
            assert!(laws::join_is_commutative(&support, &left, &right));
            assert!(laws::meet_is_a_lower_bound(&support, &left, &right));
            for third in support_elements(3) {
                assert!(laws::join_is_associative(&support, &left, &right, &third));
            }
        }
    }
}

#[test]
fn join_over_approximates_concretisation_by_proof_on_the_finite_domain() {
    let support = SupportDomain::of_length(3);
    assert!(
        support.universe_is_complete(),
        "a universe that is not complete makes this a search, not a proof"
    );
    assert_eq!(support.concrete_universe().len(), 27);
    assert!(laws::bottom_concretises_to_nothing(&support));
    assert!(laws::top_concretises_to_everything(&support));

    for left in support_elements(3) {
        for right in support_elements(3) {
            assert!(
                laws::join_over_approximates_concretisation(&support, &left, &right),
                "{left:?} joined with {right:?} lost a concretisation"
            );
            assert!(laws::order_implies_concretisation_inclusion(
                &support, &left, &right
            ));
        }
    }
}

#[test]
fn a_join_that_merely_kept_the_left_operand_would_fail_the_concretisation_law() {
    let support = SupportDomain::of_length(3);
    let zeroed = Support::Pattern {
        signs: vec![EntrySign::Zero; 3],
    };
    let positive = Support::Pattern {
        signs: vec![EntrySign::Positive; 3],
    };
    let joined = support.join(&zeroed, &positive);
    assert_ne!(joined, zeroed);
    assert_ne!(joined, positive);

    let witness = vec![0.5, 0.5, 0.5];
    assert!(support.concretises(&positive, &witness));
    assert!(!support.concretises(&zeroed, &witness));
    assert!(
        support.concretises(&joined, &witness),
        "the join must contain the right operand's concretisation, not only its representation"
    );
}

#[test]
fn join_over_approximates_concretisation_under_a_falsification_search_on_the_interval_domains() {
    let ratio = RatioIntervalDomain;
    assert!(
        !ratio.universe_is_complete(),
        "the reals are not enumerable and the domain must not claim they are"
    );
    assert!(laws::bottom_concretises_to_nothing(&ratio));
    assert!(laws::top_concretises_to_everything(&ratio));
    for left in ratio_elements() {
        for right in ratio_elements() {
            assert!(laws::join_over_approximates_concretisation(
                &ratio, &left, &right
            ));
            assert!(laws::order_implies_concretisation_inclusion(
                &ratio, &left, &right
            ));
        }
    }

    let answer = DisplacementDomain;
    assert!(!answer.universe_is_complete());
    assert!(laws::bottom_concretises_to_nothing(&answer));
    for left in displacement_elements() {
        for right in displacement_elements() {
            assert!(laws::join_over_approximates_concretisation(
                &answer, &left, &right
            ));
        }
    }
}

#[test]
fn every_transformer_maps_concrete_values_into_the_concretisation_of_its_abstract_result() {
    let ratio = RatioIntervalDomain;
    let answer = DisplacementDomain;
    for left in ratio_elements() {
        for right in ratio_elements() {
            let product = ratio_interval_multiply(left, right);
            for first in ratio.concrete_universe() {
                if !ratio.concretises(&left, &first) {
                    continue;
                }
                for second in ratio.concrete_universe() {
                    if !ratio.concretises(&right, &second) {
                        continue;
                    }
                    assert!(
                        ratio.concretises(&product, &(first * second)),
                        "{first} * {second} escaped γ of the abstract product"
                    );
                }
            }
        }
        for concrete in ratio.concrete_universe() {
            if !ratio.concretises(&left, &concrete) || concrete == 0.0 {
                continue;
            }
            let inverted = bioprism_influence::domains::ratio_interval::reciprocal(left);
            assert!(ratio.concretises(&inverted, &(1.0 / concrete)));
        }
    }

    for left in displacement_elements() {
        for right in displacement_elements() {
            let sum = displacement::add(left, right);
            for first in answer.concrete_universe() {
                if !answer.concretises(&left, &first) {
                    continue;
                }
                for second in answer.concrete_universe() {
                    if !answer.concretises(&right, &second) {
                        continue;
                    }
                    assert!(answer.concretises(&sum, &(first + second)));
                }
            }
        }
        for coefficient in [0.0, 0.25, 0.5, 1.0] {
            let scaled = displacement::scale(left, coefficient);
            for concrete in answer.concrete_universe() {
                if !answer.concretises(&left, &concrete) {
                    continue;
                }
                assert!(answer.concretises(&scaled, &(concrete * coefficient)));
            }
        }
    }
}

fn ratio_interval_multiply(left: RatioInterval, right: RatioInterval) -> RatioInterval {
    bioprism_influence::domains::ratio_interval::multiply(left, right)
}

#[test]
fn widening_is_an_upper_bound_of_both_arguments_in_every_shipped_domain() {
    let ratio = RatioIntervalDomain;
    for previous in ratio_elements() {
        for next in ratio_elements() {
            assert!(laws::widening_is_an_upper_bound(&ratio, &previous, &next));
        }
    }
    let answer = DisplacementDomain;
    for previous in displacement_elements() {
        for next in displacement_elements() {
            assert!(laws::widening_is_an_upper_bound(&answer, &previous, &next));
        }
    }
    let support = SupportDomain::of_length(3);
    for previous in support_elements(3) {
        for next in support_elements(3) {
            assert!(laws::widening_is_an_upper_bound(&support, &previous, &next));
        }
    }
}

#[test]
fn the_product_domain_inherits_its_laws_coordinatewise() {
    let product = ProductDomain::new(DisplacementDomain, 3);
    let elements: Vec<Vec<Displacement>> = displacement_elements()
        .into_iter()
        .map(|element| vec![element; 3])
        .collect();
    for left in &elements {
        assert!(laws::join_is_idempotent(&product, left));
        for right in &elements {
            assert!(laws::join_is_an_upper_bound(&product, left, right));
            assert!(laws::widening_is_an_upper_bound(&product, left, right));
            assert!(laws::meet_is_a_lower_bound(&product, left, right));
        }
    }
    assert_eq!(product.bottom().len(), 3);
    assert_eq!(product.top().len(), 3);
}

#[test]
fn registering_a_domain_twice_is_refused_rather_than_silently_replacing_it() {
    let mut registry = DomainRegistry::standard().unwrap();
    let error = registry.register(RatioIntervalDomain).unwrap_err();
    assert_eq!(
        error,
        DomainError::DuplicateRegistration {
            id: DomainId::new("ratio_interval")
        }
    );
    assert_eq!(registry.len(), 2);
}

#[test]
fn one_domains_abstraction_cannot_reach_another_domains_transformer() {
    let registry = DomainRegistry::standard().unwrap();
    let ratio_id = DomainId::new("ratio_interval");
    let answer_id = DomainId::new("answer_displacement_interval");

    let reweighting = AbstractValue::of(&RatioIntervalDomain, RatioInterval::identity());
    let displacement = AbstractValue::of(&DisplacementDomain, Displacement::at_most(0.5).unwrap());

    let error = registry
        .join(&answer_id, &reweighting, &displacement)
        .unwrap_err();
    assert_eq!(
        error,
        DomainError::ForeignAbstractValue {
            expected: answer_id.clone(),
            found: ratio_id.clone(),
        }
    );

    let error = registry
        .join(&ratio_id, &displacement, &reweighting)
        .unwrap_err();
    assert_eq!(
        error,
        DomainError::ForeignAbstractValue {
            expected: ratio_id,
            found: answer_id,
        }
    );
}

#[test]
fn two_lengths_of_the_support_domain_are_two_domains_and_do_not_share_a_lattice() {
    let mut registry = DomainRegistry::new();
    registry.register(SupportDomain::of_length(2)).unwrap();
    registry.register(SupportDomain::of_length(3)).unwrap();
    assert_eq!(registry.len(), 2);

    let short = AbstractValue::of(&SupportDomain::of_length(2), Support::unknown(2));
    let error = registry
        .join(&DomainId::new("factor_support/3"), &short, &short)
        .unwrap_err();
    assert_eq!(
        error,
        DomainError::ForeignAbstractValue {
            expected: DomainId::new("factor_support/3"),
            found: DomainId::new("factor_support/2"),
        }
    );
}

#[test]
fn the_registry_reports_which_domains_abstract_which_class_of_facts() {
    let mut registry = DomainRegistry::standard().unwrap();
    registry.register(SupportDomain::of_length(4)).unwrap();

    assert_eq!(
        registry.abstracting(FactClass::JointReweighting),
        vec![DomainId::new("ratio_interval")]
    );
    assert_eq!(
        registry.abstracting(FactClass::AnswerDisplacement),
        vec![DomainId::new("answer_displacement_interval")]
    );
    assert_eq!(
        registry.abstracting(FactClass::FactorPotential),
        vec![DomainId::new("factor_support/4")]
    );
    assert!(registry.abstracting(FactClass::JointReweighting)
        != registry.abstracting(FactClass::AnswerDisplacement));
}

#[test]
fn an_unregistered_domain_is_named_rather_than_defaulted_to() {
    let registry = DomainRegistry::standard().unwrap();
    let missing = DomainId::new("octagon");
    let error = registry.top(&missing).unwrap_err();
    assert_eq!(error, DomainError::UnregisteredDomain { id: missing });
}

#[test]
fn the_two_interval_domains_share_a_lattice_shape_and_not_a_meaning() {
    let ratio = RatioIntervalDomain;
    let answer = DisplacementDomain;
    assert_ne!(ratio.id(), answer.id());
    assert_ne!(ratio.abstracts(), answer.abstracts());

    let reweighting = RatioInterval::range(0.25, 4.0).unwrap();
    let displaced = Displacement::range(0.25, 4.0).unwrap();
    assert_eq!(ratio.render(&reweighting), answer.render(&displaced));
    assert!((reweighting.total_variation_bound() - 0.6).abs() < 1e-12);
    assert_eq!(displaced.total_variation_bound(), 1.0);
}
