//! Blueprint 43.19's first evaluation item: "cross-check with naive enumeration on small worlds."
//!
//! Variable elimination is only interesting if it is *exact*. The reference answer here is
//! [`DirectMaterialization`], which shares the kernel and differs only in schedule, so a
//! disagreement localises to the elimination logic rather than to two independently buggy
//! implementations of the same arithmetic.
//!
//! Comparison is bit-for-bit rather than tolerant. Every potential is a small integer and every
//! intermediate stays below `2^53`, so no rounding occurs and reordering an exact-integer sum
//! cannot change the result. A tolerance would have hidden exactly the kind of bug — a factor
//! multiplied twice, a variable aggregated in the wrong bucket — that this test exists to catch.

use bioprism_backends::{
    CardinalityPolicy, Declined, DirectMaterialization, OrderStrategy, QueryBackend, QueryRegion,
    RegionFactor, Semiring, VariableElimination,
};
use bioprism_world::World;
use serde_json::Value;
use std::path::PathBuf;

fn two_factor_region(semiring: Semiring) -> QueryRegion {
    QueryRegion::builder("micro")
        .semiring(semiring)
        .variable("a", 2)
        .variable("b", 2)
        .variable("c", 2)
        .free("c")
        .factor(RegionFactor::with_table(
            "f",
            vec!["a", "b"],
            vec![1.0, 2.0, 3.0, 4.0],
        ))
        .factor(RegionFactor::with_table(
            "g",
            vec!["b", "c"],
            vec![5.0, 6.0, 7.0, 8.0],
        ))
        .build()
        .expect("the micro region is well formed")
}

fn splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut value = *state;
    value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    value ^ (value >> 31)
}

/// A region with randomly chosen factor scopes: neither a chain nor a clique, so the bucket
/// schedule has to cope with messages that overlap in ways a banded family never produces.
fn scattered_region(variables: usize, factors: usize, domain: usize, seed: u64) -> QueryRegion {
    let name = |index: usize| format!("v{index:02}");
    let mut state = seed;
    let mut builder = QueryRegion::builder(format!("scattered-{seed:x}")).free(name(0));
    for index in 0..variables {
        builder = builder.variable(name(index), domain);
    }
    for index in 0..factors {
        let arity = 2 + (splitmix64(&mut state) as usize % 2);
        let mut scope: Vec<String> = Vec::new();
        while scope.len() < arity {
            let candidate = name(splitmix64(&mut state) as usize % variables);
            if !scope.contains(&candidate) {
                scope.push(candidate);
            }
        }
        let entries = domain.pow(arity as u32);
        let table: Vec<f64> = (0..entries)
            .map(|_| (splitmix64(&mut state) % 4 + 1) as f64)
            .collect();
        builder = builder.factor(RegionFactor::with_table(
            format!("phi.{index:02}"),
            scope,
            table,
        ));
    }
    builder.build().expect("the scattered region is well formed")
}

fn reference_region() -> QueryRegion {
    let path: PathBuf = [
        env!("CARGO_MANIFEST_DIR"),
        "..",
        "..",
        "fixtures",
        "fiber-v0.1",
        "radiogenomic_world.json",
    ]
    .iter()
    .collect();
    let document: Value = serde_json::from_str(
        &std::fs::read_to_string(&path).expect("the reference world fixture is readable"),
    )
    .expect("the reference world fixture is valid JSON");
    let world = World::from_json(document).expect("the reference world fixture loads");
    QueryRegion::from_world_slice(
        &world,
        "split-integrity",
        ["split_integrity_status"],
        &CardinalityPolicy::default(),
    )
    .expect("the sliced region is well formed")
}

#[test]
fn variable_elimination_agrees_with_brute_force_on_small_worlds() {
    let mut checked = 0;
    for variables in 5..9 {
        for domain in 2..4 {
            for seed in [0x01u64, 0x2Fu64, 0xC7u64] {
                let region = scattered_region(variables, variables + 2, domain, seed);
                let eliminated = VariableElimination::default().execute(&region).unwrap();
                let enumerated = DirectMaterialization::new().execute(&region).unwrap();
                assert!(
                    eliminated.agrees_exactly_with(&enumerated),
                    "disagreement on {} variables, domain {domain}, seed {seed:#x}: {:?} vs {:?}",
                    variables,
                    eliminated.values(),
                    enumerated.values()
                );
                checked += 1;
            }
        }
    }
    assert_eq!(checked, 24);
}

#[test]
fn variable_elimination_agrees_with_brute_force_on_the_width_controlled_family() {
    for width in 1..7 {
        let region = bioprism_backends::band_region(8, width, 3, 0x9C);
        let eliminated = VariableElimination::default().execute(&region).unwrap();
        let enumerated = DirectMaterialization::new().execute(&region).unwrap();
        assert!(
            eliminated.agrees_exactly_with(&enumerated),
            "disagreement at band width {width}"
        );
    }
}

#[test]
fn elimination_reproduces_a_hand_computed_marginal() {
    let region = two_factor_region(Semiring::SumProduct);
    let computed = VariableElimination::default().execute(&region).unwrap();

    assert_eq!(computed.scope(), ["c"]);
    assert_eq!(computed.values(), [62.0, 72.0]);
}

#[test]
fn variable_elimination_agrees_with_brute_force_under_max_product() {
    let region = two_factor_region(Semiring::MaxProduct);
    let eliminated = VariableElimination::default().execute(&region).unwrap();
    let enumerated = DirectMaterialization::new().execute(&region).unwrap();

    assert_eq!(eliminated.values(), [28.0, 32.0]);
    assert!(eliminated.agrees_exactly_with(&enumerated));
}

#[test]
fn variable_elimination_agrees_with_brute_force_under_the_boolean_algebra() {
    let region = QueryRegion::builder("constraints")
        .semiring(Semiring::Boolean)
        .variable("a", 2)
        .variable("b", 2)
        .variable("c", 2)
        .free("c")
        .factor(RegionFactor::with_table(
            "equal",
            vec!["a", "b"],
            vec![1.0, 0.0, 0.0, 1.0],
        ))
        .factor(RegionFactor::with_table(
            "differ",
            vec!["b", "c"],
            vec![0.0, 1.0, 1.0, 0.0],
        ))
        .build()
        .unwrap();

    let eliminated = VariableElimination::default().execute(&region).unwrap();
    let enumerated = DirectMaterialization::new().execute(&region).unwrap();

    assert_eq!(eliminated.values(), [1.0, 1.0]);
    assert!(eliminated.agrees_exactly_with(&enumerated));
}

#[test]
fn the_answer_does_not_depend_on_the_elimination_order() {
    let region = scattered_region(8, 10, 3, 0x7A1);
    let reference = DirectMaterialization::new().execute(&region).unwrap();

    for strategy in [
        OrderStrategy::MinDegree,
        OrderStrategy::MinFill,
        OrderStrategy::ExactMinimumWidth,
    ] {
        let computed = VariableElimination::new(strategy).execute(&region).unwrap();
        assert!(
            computed.agrees_exactly_with(&reference),
            "{strategy:?} disagreed with enumeration"
        );
    }
}

#[test]
fn aggregating_every_variable_yields_the_total_mass_of_the_joint() {
    let region = QueryRegion::builder("closed")
        .variable("a", 2)
        .variable("b", 2)
        .variable("c", 2)
        .factor(RegionFactor::with_table(
            "f",
            vec!["a", "b"],
            vec![1.0, 2.0, 3.0, 4.0],
        ))
        .factor(RegionFactor::with_table(
            "g",
            vec!["b", "c"],
            vec![5.0, 6.0, 7.0, 8.0],
        ))
        .build()
        .unwrap();

    let computed = VariableElimination::default().execute(&region).unwrap();
    assert!(computed.scope().is_empty());
    assert_eq!(computed.values(), [62.0 + 72.0]);
    assert_eq!(computed.total(Semiring::SumProduct), 134.0);
}

#[test]
fn an_isolated_variable_still_multiplies_the_answer_by_its_cardinality() {
    let region = QueryRegion::builder("isolated")
        .variable("a", 2)
        .variable("unused", 3)
        .free("a")
        .factor(RegionFactor::with_table("f", vec!["a"], vec![2.0, 5.0]))
        .build()
        .unwrap();

    let eliminated = VariableElimination::default().execute(&region).unwrap();
    let enumerated = DirectMaterialization::new().execute(&region).unwrap();

    assert_eq!(eliminated.values(), [6.0, 15.0]);
    assert!(eliminated.agrees_exactly_with(&enumerated));
}

#[test]
fn a_world_derived_region_can_be_costed_but_declines_to_execute_without_a_valuation() {
    let region = reference_region();

    assert!(!region.has_tables());
    assert!(VariableElimination::default().estimate(&region).is_ok());

    let declined = VariableElimination::default().execute(&region).unwrap_err();
    assert!(
        matches!(declined, Declined::MissingFactorTable { .. }),
        "expected a typed refusal, got {declined}"
    );
    assert!(matches!(
        DirectMaterialization::new().execute(&region).unwrap_err(),
        Declined::MissingFactorTable { .. }
    ));
}

#[test]
fn uniform_potentials_turn_the_reference_region_into_a_count_of_assignments() {
    let region = reference_region()
        .with_uniform_tables()
        .expect("every reference factor is small enough to materialise");

    let expected: f64 = region
        .bound_variables()
        .iter()
        .map(|name| region.cardinality_of(name).unwrap() as f64)
        .product();

    let eliminated = VariableElimination::default().execute(&region).unwrap();
    let enumerated = DirectMaterialization::new().execute(&region).unwrap();

    assert_eq!(eliminated.scope(), ["split_integrity_status"]);
    assert!(eliminated.values().iter().all(|value| *value == expected));
    assert!(eliminated.agrees_exactly_with(&enumerated));
    assert!(region
        .assumptions()
        .iter()
        .any(|note| note.contains("carry no evidential content")));
}

#[test]
fn answers_over_different_scopes_are_a_structural_disagreement_not_a_small_error() {
    let region = two_factor_region(Semiring::SumProduct);
    let over_c = VariableElimination::default().execute(&region).unwrap();
    let same = DirectMaterialization::new().execute(&region).unwrap();

    assert_eq!(over_c.max_absolute_difference(&same), Some(0.0));

    let closed = QueryRegion::builder("closed")
        .variable("a", 2)
        .variable("b", 2)
        .variable("c", 2)
        .factor(RegionFactor::with_table(
            "f",
            vec!["a", "b"],
            vec![1.0, 2.0, 3.0, 4.0],
        ))
        .factor(RegionFactor::with_table(
            "g",
            vec!["b", "c"],
            vec![5.0, 6.0, 7.0, 8.0],
        ))
        .build()
        .unwrap();
    let scalar = VariableElimination::default().execute(&closed).unwrap();

    assert_eq!(over_c.max_absolute_difference(&scalar), None);
    assert!(!over_c.agrees_exactly_with(&scalar));
}

#[test]
fn the_execution_receipt_names_the_order_and_every_intermediate() {
    let region = bioprism_backends::band_region(7, 3, 2, 0x3B);
    let computed = VariableElimination::default().execute(&region).unwrap();
    let receipt = computed.receipt();

    assert_eq!(receipt.backend, bioprism_section::Backend::FaqInsideOut);
    assert_eq!(receipt.semiring, Semiring::SumProduct);
    assert_eq!(receipt.induced_width, Some(3));
    assert_eq!(receipt.order.len(), 6);
    assert_eq!(receipt.intermediates.len(), receipt.order.len() + 1);
    assert_eq!(
        receipt.intermediates.last().unwrap().scope,
        vec!["v00".to_string()]
    );
    assert!(receipt
        .intermediates
        .iter()
        .take(receipt.order.len())
        .all(|step| step.eliminated.len() == 1));
}
