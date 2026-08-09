//! Why widening exists, on a chain that demonstrates it rather than a doc comment that asserts it.
//!
//! The chain is not a fixture invented to need widening. It is the Neumann series
//! `D = Σ_n Cⁿ` of the Dobrushin comparison theorem, computed on a region the crate ships, and it
//! ascends forever for a structural reason: conditional dependence between two sites runs in both
//! directions, so `C` is never nilpotent and every partial sum is strictly below the limit.
//!
//! Four claims are separated here on purpose, because collapsing any pair of them would let a
//! solver look correct while being useless:
//!
//! - join alone does not reach a fixed point, and reporting the last iterate would report a
//!   *pre*-fixpoint, which over-approximates nothing;
//! - widening reaches one, and the element it reaches is sound and — on this analysis — vacuous;
//! - narrowing recovers the precision, and every descending iterate is still sound;
//! - the result says which of those happened, all the way onto the method a certificate carries.

use bioprism_influence::domains::displacement::{Displacement, DisplacementDomain};
use bioprism_influence::domains::product::ProductDomain;
use bioprism_influence::interpret::transfer;
use bioprism_influence::solver::{
    ascend_by_join_only_from, join_iterates, solve_from, RefinementSchedule,
};
use bioprism_influence::{
    comparison_system, domains, interpret_with_standard_domains, smallworld, AbstractDomain,
    BoundMethod, ComparisonSystem, Convergence, DomainError, DomainId, Family, InfluenceAnalyzer,
    Perturbation, SmallWorldSpec,
};

fn weak_cycle_system() -> ComparisonSystem {
    let spec = SmallWorldSpec {
        family: Family::WeakCycle,
        size: 4,
        cardinality: 3,
        seed: 0x5EED_0002 ^ 0x22,
    };
    let region = smallworld::generate(&spec).expect("the family builds");
    comparison_system(&region, &["f.c2".to_string()], &Perturbation::Removal)
        .expect("the region is well formed")
        .expect("the weak cycle satisfies the Dobrushin condition")
}

/// A two-site region coupled hard enough that its Neumann series converges slowly.
///
/// `f.ab` is an Ising-shaped potential with off-diagonal `0.005`, giving an interdependence
/// coefficient of `(1 − k)/(1 + k) ≈ 0.99` in each direction — inside Dobrushin's condition and
/// close to its edge. `f.a` is a near-uniform prior, and it is the factor the perturbation removes,
/// so the accumulated bound stays well below one and the fixed point is worth computing.
fn strongly_coupled_system() -> ComparisonSystem {
    use bioprism_backends::{QueryRegion, RegionFactor};
    let coupling = 0.005;
    let region = QueryRegion::builder("strongly-coupled")
        .observed_variable("a", 2)
        .observed_variable("b", 2)
        .factor(RegionFactor::with_table(
            "f.ab",
            vec!["a", "b"],
            vec![1.0, coupling, coupling, 1.0],
        ))
        .factor(RegionFactor::with_table("f.a", vec!["a"], vec![1.0, 1.01]))
        .free("b")
        .build()
        .unwrap();
    comparison_system(&region, &["f.a".to_string()], &Perturbation::Removal)
        .expect("the region is well formed")
        .expect("the coupling is inside Dobrushin's condition")
}

fn seed(size: usize) -> Vec<Displacement> {
    vec![Displacement::exactly(0.0).expect("zero is admissible"); size]
}

#[test]
fn the_join_only_ascent_is_strictly_increasing_at_every_step_and_does_not_stabilise() {
    let system = strongly_coupled_system();
    assert!(system.contraction() > 0.98);
    let product = ProductDomain::new(DisplacementDomain, system.sites().len());
    let iterates = join_iterates(
        &product,
        seed(system.sites().len()),
        |state| transfer(&system, state),
        500,
    );
    for window in iterates.windows(2) {
        assert!(
            product.leq(&window[0], &window[1]),
            "the chain must ascend: {:?} is not below {:?}",
            window[0],
            window[1]
        );
        assert_ne!(
            window[0], window[1],
            "the chain must ascend strictly, or it would have stabilised"
        );
    }
}

#[test]
fn a_chain_that_does_not_converge_under_join_alone_does_converge_under_widening() {
    let system = strongly_coupled_system();
    let product = ProductDomain::new(DisplacementDomain, system.sites().len());

    let refused = ascend_by_join_only_from(
        &product,
        seed(system.sites().len()),
        |state| transfer(&system, state),
        1_000,
    )
    .expect_err("join alone must not claim a fixed point it never reached");
    assert_eq!(
        refused,
        DomainError::AscendingChainDidNotStabilise { steps: 1_000 }
    );

    let widened = solve_from(
        &product,
        seed(system.sites().len()),
        |state| transfer(&system, state),
        RefinementSchedule::default(),
    )
    .expect("widening terminates the chain");
    assert!(widened.widenings >= 1);
    assert!(
        widened.joins + widened.widenings + widened.narrowings < 1_000,
        "widening has to be cheaper than the ascent it replaces, and took {} steps",
        widened.joins + widened.widenings + widened.narrowings
    );
    assert!(
        product.leq(&transfer(&system, &widened.value), &widened.value),
        "the result must be a post-fixpoint"
    );
}

/// A measurement that came out against the obvious reading of "infinite ascending chain".
///
/// In exact arithmetic the Neumann series `Σ Cⁿ b` never reaches its limit, so the Kleene chain is
/// genuinely infinite. In IEEE doubles it does reach it, because once the increment `Cⁿ b` falls
/// below an ulp of the partial sum the addition stops changing anything. On the weakly coupled cycle
/// that happens after a couple of dozen joins.
///
/// The tempting conclusion — that widening is unnecessary here and a large join budget would do —
/// is wrong, and this test is where that is written down. The step count at which rounding rescues
/// the iteration is `≈ 52·ln2 / −ln r`, which is not a property of the lattice, not bounded over the
/// class of regions the method accepts, and grows without limit as the contraction approaches one.
/// The strongly coupled system needs thousands of joins for the same rescue and the analysis has no
/// way to know in advance which case it is in. A solver that relied on it would be one whose
/// termination depended on the magnitudes in a factor table.
#[test]
fn join_only_termination_on_a_weakly_coupled_region_is_a_rounding_artefact_not_a_lattice_property() {
    let weak = weak_cycle_system();
    let weak_product = ProductDomain::new(DisplacementDomain, weak.sites().len());
    let weak_ascent = ascend_by_join_only_from(
        &weak_product,
        seed(weak.sites().len()),
        |state| transfer(&weak, state),
        10_000,
    )
    .expect("rounding stabilises this chain");

    let strong = strongly_coupled_system();
    let strong_product = ProductDomain::new(DisplacementDomain, strong.sites().len());
    let strong_ascent = ascend_by_join_only_from(
        &strong_product,
        seed(strong.sites().len()),
        |state| transfer(&strong, state),
        10_000,
    )
    .expect("rounding eventually stabilises this one too");

    println!(
        "join-only joins to stabilise: contraction {:.4} took {}, contraction {:.4} took {}",
        weak.contraction(),
        weak_ascent.joins,
        strong.contraction(),
        strong_ascent.joins
    );
    assert!(
        strong_ascent.joins > 20 * weak_ascent.joins,
        "the step count must scale with the contraction, not sit at a constant a budget could cover"
    );
    assert!(
        strong_ascent.joins > 1_000,
        "the strongly coupled chain took only {} joins",
        strong_ascent.joins
    );
    assert_eq!(weak_ascent.reached_by, Convergence::Join);
}

#[test]
fn a_bound_reached_by_widening_is_not_reported_as_one_reached_by_join() {
    let cyclic = SmallWorldSpec {
        family: Family::WeakCycle,
        size: 3,
        cardinality: 2,
        seed: 0x5EED_0001,
    };
    let region = smallworld::generate(&cyclic).expect("the family builds");
    let widened =
        interpret_with_standard_domains(&region, &["f.c1".to_string()], &Perturbation::Removal)
            .expect("well formed")
            .expect("accepted");
    assert!(widened.widened());
    assert_eq!(
        widened.bound.method(),
        BoundMethod::WidenedAbstractInterpretation
    );
    assert!(widened.bound.method().used_widening());

    let joined = single_site_interpretation();
    assert!(!joined.widened());
    assert_eq!(joined.convergence, Convergence::Join);
    assert_eq!(joined.bound.method(), BoundMethod::AbstractInterpretation);
    assert!(!joined.bound.method().used_widening());

    assert_ne!(
        widened.bound.method().as_str(),
        joined.bound.method().as_str(),
        "the distinction has to survive onto the certificate, where only the method string travels"
    );
}

/// A region with one variable and one factor: `C = 0`, so the chain stabilises after one join.
fn single_site_interpretation() -> bioprism_influence::AbstractInterpretation {
    use bioprism_backends::{QueryRegion, RegionFactor};
    let region = QueryRegion::builder("single-site")
        .observed_variable("a", 2)
        .factor(RegionFactor::with_table("f.a", vec!["a"], vec![1.0, 2.0]))
        .free("a")
        .build()
        .unwrap();
    interpret_with_standard_domains(&region, &["f.a".to_string()], &Perturbation::Removal)
        .expect("well formed")
        .expect("accepted")
}

#[test]
fn a_chain_with_nothing_to_accumulate_stabilises_under_join_and_the_bound_is_exact() {
    let interpretation = single_site_interpretation();
    assert_eq!(interpretation.convergence, Convergence::Join);
    assert_eq!(interpretation.widenings, 0);
    assert_eq!(interpretation.narrowings, 0);
    assert_eq!(interpretation.contraction, 0.0);
    assert!(
        (interpretation.bound.value() - 1.0 / 6.0).abs() < 1e-12,
        "the marginal moves from (1/3, 2/3) to (1/2, 1/2), which is a total variation of 1/6, and got {}",
        interpretation.bound.value()
    );
}

#[test]
fn the_widened_bound_before_narrowing_is_vacuous_and_narrowing_is_what_makes_it_useful() {
    let system = weak_cycle_system();
    let product = ProductDomain::new(DisplacementDomain, system.sites().len());
    let free = system.free_positions()[0];
    let exact = system.exact_solution().expect("I - C is invertible");

    let without_narrowing = solve_from(
        &product,
        seed(system.sites().len()),
        |state| transfer(&system, state),
        RefinementSchedule {
            narrowing_steps: 0,
            ..RefinementSchedule::default()
        },
    )
    .expect("widening terminates the chain");
    assert_eq!(without_narrowing.reached_by, Convergence::Widening);
    assert_eq!(
        without_narrowing.value[free].total_variation_bound(),
        1.0,
        "widening alone lands on a threshold rung above one, which reads out as the vacuous bound"
    );

    let with_narrowing = solve_from(
        &product,
        seed(system.sites().len()),
        |state| transfer(&system, state),
        RefinementSchedule::default(),
    )
    .expect("widening terminates the chain");
    assert_eq!(
        with_narrowing.reached_by,
        Convergence::WideningThenNarrowing
    );
    let recovered = with_narrowing.value[free].total_variation_bound();
    assert!(recovered < 0.01, "narrowing recovered only {recovered}");
    assert!(
        recovered >= exact[free] - 1e-12,
        "no descending iterate may fall below the least fixed point"
    );
    println!(
        "widening alone reported 1.0; narrowing recovered {recovered:.9}; the exact fixed point is {:.9}",
        exact[free]
    );
}

#[test]
fn every_narrowing_iterate_is_a_post_fixpoint_and_the_sequence_only_descends() {
    let system = weak_cycle_system();
    let product = ProductDomain::new(DisplacementDomain, system.sites().len());
    let free = system.free_positions()[0];
    let exact = system.exact_solution().expect("I - C is invertible");

    let mut previous = f64::INFINITY;
    for budget in 0..8 {
        let solved = solve_from(
            &product,
            seed(system.sites().len()),
            |state| transfer(&system, state),
            RefinementSchedule {
                narrowing_steps: budget,
                ..RefinementSchedule::default()
            },
        )
        .expect("widening terminates the chain");
        let value = solved.value[free].total_variation_bound();
        assert!(
            value <= previous + 1e-12,
            "a longer narrowing budget must not report a larger bound"
        );
        assert!(
            value >= exact[free] - 1e-12,
            "narrowing must stay above the least fixed point"
        );
        assert!(
            product.leq(&transfer(&system, &solved.value), &solved.value),
            "stopping the descent early must still leave a post-fixpoint"
        );
        previous = value;
    }
}

/// What the narrowing budget costs when the contraction is close to one.
///
/// The descending sequence closes the gap by a factor of `r` per step, so a fixed step budget buys
/// a fixed *ratio* and not a fixed accuracy. At `r ≈ 0.99` the default sixty-four steps leave the
/// reported bound well above the fixed point it is descending towards, and the honest response is to
/// measure the residual rather than to raise the budget until a test passes. The bound stays sound
/// throughout — that is what makes stopping early legitimate — and it stays loose, which is what a
/// reader of the certificate needs to be able to see from
/// [`BoundMethod::WidenedAbstractInterpretation`].
#[test]
fn the_narrowing_budget_buys_a_ratio_and_not_an_accuracy() {
    let system = strongly_coupled_system();
    let product = ProductDomain::new(DisplacementDomain, system.sites().len());
    let free = system.free_positions()[0];
    let exact = system.exact_solution().expect("I - C is invertible");

    let mut reported = Vec::new();
    for budget in [0usize, 64, 256, 1024] {
        let solved = solve_from(
            &product,
            seed(system.sites().len()),
            |state| transfer(&system, state),
            RefinementSchedule {
                narrowing_steps: budget,
                ..RefinementSchedule::default()
            },
        )
        .expect("widening terminates the chain");
        let value = solved.value[free].total_variation_bound();
        assert!(value >= exact[free] - 1e-12);
        reported.push((budget, value));
    }
    println!(
        "contraction {:.4}, exact fixed point {:.9}; narrowing budgets {:?}",
        system.contraction(),
        exact[free],
        reported
    );
    assert!(reported[0].1 > reported[1].1);
    assert!(reported[1].1 > reported[3].1);
    assert!(
        reported[1].1 > exact[free] * 1.5,
        "the default budget was expected to leave real slack at this contraction, and left {}",
        reported[1].1 - exact[free]
    );
}

#[test]
fn a_widening_operator_that_cannot_terminate_a_chain_is_reported_as_a_defect_not_as_a_bound() {
    let domain = DisplacementDomain;
    let doubling = |state: &Displacement| {
        domains::displacement::add(
            domains::displacement::scale(*state, 2.0),
            Displacement::at_most(1.0).unwrap(),
        )
    };

    let starved = solve_from(
        &domain,
        Displacement::exactly(0.0).unwrap(),
        doubling,
        RefinementSchedule {
            joins_before_widening: 1,
            widening_steps: 2,
            narrowing_steps: 0,
        },
    )
    .expect_err("two widenings cannot climb a thirty-one rung ladder");
    assert_eq!(
        starved,
        DomainError::WideningDidNotStabilise {
            id: DomainId::new("answer_displacement_interval"),
            steps: 2,
        }
    );

    let terminated = solve_from(
        &domain,
        Displacement::exactly(0.0).unwrap(),
        doubling,
        RefinementSchedule {
            joins_before_widening: 1,
            widening_steps: 64,
            narrowing_steps: 8,
        },
    )
    .expect("the ladder ends at infinity, so the chain does terminate");
    assert!(
        !terminated.value.is_bounded_above(),
        "a genuinely divergent chain must widen to the top element, not to a finite lie"
    );
    assert_eq!(terminated.value.total_variation_bound(), 1.0);
}

#[test]
fn the_analyzer_prefers_a_tighter_bound_from_any_method_and_still_records_that_this_one_widened() {
    let spec = SmallWorldSpec {
        family: Family::WeakCycle,
        size: 3,
        cardinality: 3,
        seed: 0x5EED_0004,
    };
    let region = smallworld::generate(&spec).expect("the family builds");
    let analysis = InfluenceAnalyzer::default()
        .structural_only()
        .analyse_factor(&region, "f.c0", &Perturbation::Removal)
        .expect("well formed");

    let widened = analysis
        .attempted
        .iter()
        .find(|outcome| outcome.method == BoundMethod::WidenedAbstractInterpretation)
        .expect("the cyclic region is analysed by the 43.11 pass");
    assert!(widened.value.is_some());

    let dynamic = analysis
        .attempted
        .iter()
        .find(|outcome| outcome.method == BoundMethod::DynamicRange)
        .and_then(|outcome| outcome.value)
        .expect("the dynamic-range method always applies to a valued factor");
    let reported = analysis.estimate.bound().expect("bounded").value();
    assert!(reported <= dynamic + 1e-12);
    println!(
        "weak cycle 3x3 / f.c0: dynamic_range {dynamic:.9}, 43.11 pass {:.9}, reported {reported:.9} by {}",
        widened.value.unwrap(),
        analysis.estimate.bound().unwrap().method().as_str()
    );
}
