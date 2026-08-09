//! Ground truth for the 43.11 pass, in the style the crate already uses for its other methods.
//!
//! `soundness.rs` asserts every individual method sound rather than only their minimum, because a
//! minimum can be sound while one of its arguments is wrong. The same rule applies here: the
//! abstract interpretation is checked on its own against brute force, not as part of whatever the
//! analyzer happened to report.
//!
//! The removal perturbation has exactly one realisation, so a passing assertion against it is a
//! proof for that region. The multiplicative range has a continuum and `crate::bruteforce`
//! enumerates the box's vertices plus a seeded interior sample; against that class a passing
//! assertion is a falsification search — a lower bound on the true maximum — and the test names say
//! which of the two it is.

use bioprism_influence::domains::support::certainly_positive;
use bioprism_influence::{
    interpret_with_standard_domains, maximum_influence, smallworld, BoundMethod, Convergence,
    Family, InfluenceAnalyzer, InfluenceError, Perturbation, SmallWorldSpec, UnknownReason,
};

const FLOAT_REORDERING: f64 = 1e-12;

fn factor_ids(region: &bioprism_backends::QueryRegion) -> Vec<String> {
    region
        .factors()
        .iter()
        .map(|factor| factor.id().to_string())
        .collect()
}

#[test]
fn the_abstract_interpretation_is_never_exceeded_by_the_true_removal_influence() {
    let mut accepted = 0usize;
    let mut declined = 0usize;
    let mut families = std::collections::BTreeSet::<&'static str>::new();
    let mut tightest = f64::INFINITY;
    let mut witness = String::new();
    for spec in smallworld::family() {
        let region = smallworld::generate(&spec).expect("the family builds");
        for id in factor_ids(&region) {
            let subject = vec![id.clone()];
            match interpret_with_standard_domains(&region, &subject, &Perturbation::Removal)
                .expect("the region is well formed")
            {
                Ok(interpretation) => {
                    let truth = maximum_influence(&region, &subject, &Perturbation::Removal, 0, 0)
                        .expect("removal has exactly one realisation");
                    assert!(truth.exhaustive);
                    assert!(
                        truth.found_influence <= interpretation.bound.value() + FLOAT_REORDERING,
                        "{} / {id}: true influence {} exceeds the abstract bound {}",
                        spec.label(),
                        truth.found_influence,
                        interpretation.bound.value()
                    );
                    if truth.found_influence > 0.0 {
                        let ratio = interpretation.bound.value() / truth.found_influence;
                        if ratio < tightest {
                            tightest = ratio;
                            witness = format!("{} / {id}", spec.label());
                        }
                    }
                    accepted += 1;
                    families.insert(spec.family.as_str());
                }
                Err(_) => declined += 1,
            }
        }
    }
    println!("abstract interpretation: {accepted} factors bounded, {declined} declined, families {families:?}");
    println!("tightest observed bound/truth ratio: {tightest:.6} on {witness}");
    assert!(
        tightest >= 1.0,
        "a ratio below one is a counterexample to the comparison theorem as implemented, on {witness}"
    );
    assert!(
        tightest < 2.0,
        "the tightest case was {tightest}x; a method whose best case is that loose gives the soundness assertion no discriminating power"
    );
    assert!(
        accepted >= 20,
        "only {accepted} factors were bounded by the 43.11 pass; a method nothing accepts is not evidence of soundness"
    );
    assert!(
        families.contains("weak_cycle"),
        "the pass must accept a cyclic region, which is the class chain contraction refuses"
    );
}

#[test]
fn the_abstract_interpretation_survives_a_falsification_search_under_a_stated_range() {
    let perturbation = Perturbation::relative_tolerance(0.2).expect("a legal tolerance");
    let mut searched = 0usize;
    for spec in smallworld::family() {
        let region = smallworld::generate(&spec).expect("the family builds");
        for id in factor_ids(&region) {
            let subject = vec![id.clone()];
            let Ok(interpretation) =
                interpret_with_standard_domains(&region, &subject, &perturbation)
                    .expect("the region is well formed")
            else {
                continue;
            };
            match maximum_influence(&region, &subject, &perturbation, 24, spec.seed) {
                Ok(truth) => {
                    assert!(!truth.exhaustive);
                    assert!(
                        truth.found_influence <= interpretation.bound.value() + FLOAT_REORDERING,
                        "{} / {id}: search found {} above the abstract bound {}",
                        spec.label(),
                        truth.found_influence,
                        interpretation.bound.value()
                    );
                    searched += 1;
                }
                Err(InfluenceError::BruteForceTooLarge { .. }) => {}
                Err(other) => panic!("{other}"),
            }
        }
    }
    assert!(searched >= 10, "only {searched} range perturbations were searched");
}

#[test]
fn a_group_bound_from_the_abstract_interpretation_is_never_exceeded_jointly() {
    let mut checked = 0usize;
    for spec in smallworld::family() {
        let region = smallworld::generate(&spec).expect("the family builds");
        let ids = factor_ids(&region);
        if ids.len() < 2 {
            continue;
        }
        let group: Vec<String> = ids.iter().take(2).cloned().collect();
        let Ok(interpretation) =
            interpret_with_standard_domains(&region, &group, &Perturbation::Removal)
                .expect("the region is well formed")
        else {
            continue;
        };
        let truth = maximum_influence(&region, &group, &Perturbation::Removal, 0, 0)
            .expect("removal has exactly one realisation");
        assert!(
            truth.found_influence <= interpretation.bound.value() + FLOAT_REORDERING,
            "{} / {group:?}: joint influence {} exceeds the abstract bound {}",
            spec.label(),
            truth.found_influence,
            interpretation.bound.value()
        );
        checked += 1;
    }
    assert!(checked >= 5, "only {checked} groups were checked");
}

#[test]
fn a_cyclic_region_is_bounded_by_the_43_11_pass_and_refused_by_chain_contraction() {
    let spec = SmallWorldSpec {
        family: Family::WeakCycle,
        size: 3,
        cardinality: 2,
        seed: 0x5EED_0001,
    };
    let region = smallworld::generate(&spec).expect("the family builds");
    let chain = bioprism_influence::chain_of(&region);
    assert!(
        matches!(chain, Err(UnknownReason::RegionOutsideMethodClass { .. })),
        "the cycle must be outside the contraction method's class"
    );

    let analysis = InfluenceAnalyzer::default()
        .structural_only()
        .analyse_factor(&region, "f.c1", &Perturbation::Removal)
        .expect("the region is well formed");
    let reported = analysis.estimate.bound().expect("a cycle is now bounded");
    assert!(matches!(
        reported.method(),
        BoundMethod::AbstractInterpretation | BoundMethod::WidenedAbstractInterpretation
    ));
    let truth = maximum_influence(&region, &["f.c1".to_string()], &Perturbation::Removal, 0, 0)
        .unwrap()
        .found_influence;
    assert!(truth <= reported.value() + FLOAT_REORDERING);
    println!(
        "weak cycle: truth {truth:.9}, abstract interpretation {:.9} (x{:.2}) by {}",
        reported.value(),
        reported.value() / truth,
        reported.method().as_str()
    );
}

#[test]
fn a_factor_with_no_potential_is_refused_rather_than_bounded_by_one() {
    use bioprism_backends::{QueryRegion, RegionFactor};
    let region = QueryRegion::builder("structural")
        .observed_variable("a", 2)
        .observed_variable("b", 2)
        .factor(RegionFactor::structural("f.ab", vec!["a", "b"]))
        .factor(RegionFactor::structural("f.a", vec!["a"]))
        .free("b")
        .build()
        .unwrap();
    let outcome = interpret_with_standard_domains(
        &region,
        &["f.ab".to_string()],
        &Perturbation::Removal,
    )
    .expect("the region is well formed");
    let reason = outcome.expect_err("a region with no potentials must not produce a bound");
    assert!(
        matches!(reason, UnknownReason::NoFactorTable { .. }),
        "expected the missing-potential clause, got {reason}"
    );
}

#[test]
fn a_zero_entry_is_refused_because_a_single_site_conditional_need_not_exist() {
    use bioprism_backends::{QueryRegion, RegionFactor};
    let region = QueryRegion::builder("zero-entry")
        .observed_variable("a", 2)
        .observed_variable("b", 2)
        .factor(RegionFactor::with_table(
            "f.ab",
            vec!["a", "b"],
            vec![1.0, 2.0, 3.0, 4.0],
        ))
        .factor(RegionFactor::with_table("f.gate", vec!["a"], vec![0.0, 1.0]))
        .free("b")
        .build()
        .unwrap();
    assert!(!certainly_positive(
        &bioprism_influence::Support::of_table(&[0.0, 1.0])
    ));
    let reason = interpret_with_standard_domains(
        &region,
        &["f.gate".to_string()],
        &Perturbation::Removal,
    )
    .expect("the region is well formed")
    .expect_err("a forbidden assignment must be refused, not approximated");
    let text = reason.to_string();
    assert!(
        text.contains("zero or non-finite entry"),
        "the refusal must name the clause that failed, got {text}"
    );
}

#[test]
fn a_strongly_coupled_region_is_refused_by_name_rather_than_bounded_anyway() {
    let spec = SmallWorldSpec {
        family: Family::Triangle,
        size: 0,
        cardinality: 3,
        seed: 0x5EED_0001,
    };
    let region = smallworld::generate(&spec).expect("the family builds");
    let outcome =
        interpret_with_standard_domains(&region, &["f.ab".to_string()], &Perturbation::Removal)
            .expect("the region is well formed");
    match outcome {
        Err(UnknownReason::RegionOutsideMethodClass { detail, .. }) => {
            assert!(
                detail.contains("maximum row sum"),
                "the refusal must name the Dobrushin condition, got {detail}"
            );
        }
        Err(other) => panic!("unexpected refusal {other}"),
        Ok(interpretation) => {
            assert!(
                interpretation.contraction < 1.0,
                "a region that was accepted must have satisfied the condition"
            );
        }
    }
}

/// The measurement the widening trade is worth stating in numbers.
///
/// Three quantities on one accepted cyclic region: what widening alone would have reported, what
/// the scheduled narrowing recovered, and the exact solution of `(I − C)u = b` the two are
/// approximations of. The gap between the second and the third is the residual slack the narrowing
/// budget leaves, and it is printed rather than assumed to be negligible.
#[test]
fn the_suite_reports_what_widening_gave_away_and_what_narrowing_took_back() {
    use bioprism_influence::comparison_system;
    let spec = SmallWorldSpec {
        family: Family::WeakCycle,
        size: 4,
        cardinality: 3,
        seed: 0x5EED_0002 ^ 0x22,
    };
    let region = smallworld::generate(&spec).expect("the family builds");
    let subject = vec!["f.c2".to_string()];
    let system = comparison_system(&region, &subject, &Perturbation::Removal)
        .expect("the region is well formed")
        .expect("the weak cycle satisfies the Dobrushin condition");
    let exact = system.exact_solution().expect("I - C is invertible");
    let free = system.free_positions()[0];

    let interpretation = interpret_with_standard_domains(&region, &subject, &Perturbation::Removal)
        .expect("the region is well formed")
        .expect("the weak cycle is accepted");
    let truth = maximum_influence(&region, &subject, &Perturbation::Removal, 0, 0)
        .unwrap()
        .found_influence;

    println!(
        "weak cycle 4x3: contraction {:.6}, exact (I-C)^-1 b at the free site {:.9}, reported {:.9} by {}, true influence {:.9}",
        system.contraction(),
        exact[free],
        interpretation.bound.value(),
        interpretation.convergence.as_str(),
        truth
    );
    assert!(interpretation.widened(), "this chain does not stabilise under join");
    assert_eq!(interpretation.convergence, Convergence::WideningThenNarrowing);
    assert!(
        interpretation.bound.value() >= exact[free] - 1e-9,
        "a post-fixpoint must dominate the exact solution it approximates"
    );
    assert!(
        interpretation.bound.value() <= exact[free] + 1e-6,
        "narrowing left {} of slack over the exact solution {}",
        interpretation.bound.value() - exact[free],
        exact[free]
    );
    assert!(truth <= interpretation.bound.value() + FLOAT_REORDERING);
    assert!(
        interpretation.bound.value() < 1.0,
        "a bound of one on this fixture would mean narrowing recovered nothing"
    );
}

#[test]
fn the_abstract_transfer_over_approximates_the_concrete_one_it_abstracts() {
    use bioprism_influence::comparison_system;
    use bioprism_influence::domains::displacement::DisplacementDomain;
    use bioprism_influence::interpret::transfer;
    use bioprism_influence::{AbstractDomain, Displacement};

    let spec = SmallWorldSpec {
        family: Family::WeakCycle,
        size: 3,
        cardinality: 2,
        seed: 0x5EED_0003,
    };
    let region = smallworld::generate(&spec).expect("the family builds");
    let system = comparison_system(&region, &["f.c0".to_string()], &Perturbation::Removal)
        .expect("well formed")
        .expect("accepted");

    let domain = DisplacementDomain;
    let abstracted: Vec<Displacement> = (0..system.sites().len())
        .map(|index| Displacement::range(0.0, 0.1 * (index as f64 + 1.0)).unwrap())
        .collect();
    let concrete: Vec<f64> = (0..system.sites().len())
        .map(|index| 0.05 * (index as f64 + 1.0))
        .collect();
    for (element, value) in abstracted.iter().zip(&concrete) {
        assert!(domain.concretises(element, value));
    }

    let abstract_image = transfer(&system, &abstracted);
    let concrete_image = system.apply(&concrete);
    for (element, value) in abstract_image.iter().zip(&concrete_image) {
        assert!(
            domain.concretises(element, value),
            "f(γ(a)) escaped γ(f#(a)): {value} not in {}",
            domain.render(element)
        );
    }
}
