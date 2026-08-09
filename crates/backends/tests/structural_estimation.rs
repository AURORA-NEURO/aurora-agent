//! Blueprint 43.18: width, not size, predicts cost.
//!
//! Each test below pins one half of that claim. The hand-computed cases fix what induced width
//! *is* — a star is cheap from the leaves and expensive from the centre, a clique is expensive from
//! anywhere — and the reference-world cases fix that the statistic is genuinely query-specific: a
//! 756-factor world reached by six factors at width five.

use bioprism_backends::{
    bucket_schedule, direct_schedule, elimination_order, Budget, CardinalityPolicy,
    CardinalitySource, DirectMaterialization, OrderStrategy, PrimalGraph, QueryBackend,
    QueryRegion, RegionError, RegionFactor, Semiring, VariableElimination, WidthMetric,
};
use bioprism_world::World;
use serde_json::Value;
use std::path::PathBuf;

fn chain(variables: usize, domain: usize) -> QueryRegion {
    let name = |index: usize| format!("v{index:02}");
    let mut builder = QueryRegion::builder("chain").free(name(0));
    for index in 0..variables {
        builder = builder.variable(name(index), domain);
    }
    for index in 1..variables {
        builder = builder.factor(RegionFactor::structural(
            format!("edge.{index:02}"),
            vec![name(index - 1), name(index)],
        ));
    }
    builder.build().expect("chain is well formed")
}

fn star() -> QueryRegion {
    QueryRegion::builder("star")
        .variable("centre", 2)
        .variable("a", 2)
        .variable("b", 2)
        .variable("c", 2)
        .factor(RegionFactor::structural("s.a", vec!["centre", "a"]))
        .factor(RegionFactor::structural("s.b", vec!["centre", "b"]))
        .factor(RegionFactor::structural("s.c", vec!["centre", "c"]))
        .build()
        .expect("star is well formed")
}

fn clique(variables: usize, domain: usize) -> QueryRegion {
    let name = |index: usize| format!("v{index:02}");
    let scope: Vec<String> = (0..variables).map(name).collect();
    let mut builder = QueryRegion::builder("clique");
    for variable in &scope {
        builder = builder.variable(variable.clone(), domain);
    }
    builder
        .factor(RegionFactor::structural("all", scope))
        .build()
        .expect("clique is well formed")
}

fn reference_world() -> World {
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
    World::from_json(document).expect("the reference world fixture loads")
}

fn reference_region(world: &World) -> QueryRegion {
    QueryRegion::from_world_slice(
        world,
        "split-integrity",
        ["split_integrity_status"],
        &CardinalityPolicy::default(),
    )
    .expect("the sliced region is well formed")
}

#[test]
fn the_primal_graph_makes_a_clique_of_every_factor_scope() {
    let region = star();
    let graph = PrimalGraph::of_region(&region);

    assert_eq!(region.label(), "star");
    assert_eq!(graph.variable_count(), 4);
    assert_eq!(graph.edge_count(), 3);
    assert_eq!(graph.degree("centre"), Some(3));
    assert_eq!(graph.degree("a"), Some(1));
    assert_eq!(graph.neighbours("a"), Some(vec!["centre"]));
    assert_eq!(graph.degree("absent"), None);

    let order = elimination_order(&region, OrderStrategy::MinDegree);
    let widest = order
        .cliques
        .iter()
        .max_by_key(|clique| clique.width())
        .expect("every elimination records its clique");
    assert_eq!(widest.width(), order.induced_width);
    assert_eq!(widest.clique.len(), order.induced_width + 1);
}

#[test]
fn a_chain_eliminates_at_induced_width_one() {
    let order = elimination_order(&chain(8, 2), OrderStrategy::MinFill);
    assert_eq!(order.induced_width, 1);
    assert_eq!(order.order.len(), 7);
}

#[test]
fn eliminating_a_star_centre_first_costs_width_three_where_leaves_first_costs_one() {
    let graph = PrimalGraph::of_region(&star());
    let names = |list: [&str; 4]| list.map(str::to_string).to_vec();

    let (centre_first, _) = graph.induced_width_of(&names(["centre", "a", "b", "c"]));
    let (leaves_first, _) = graph.induced_width_of(&names(["a", "b", "c", "centre"]));

    assert_eq!(centre_first, 3);
    assert_eq!(leaves_first, 1);
    assert_eq!(
        elimination_order(&star(), OrderStrategy::MinDegree).induced_width,
        1
    );
}

#[test]
fn a_clique_has_induced_width_one_below_its_size_under_every_order() {
    let region = clique(5, 2);
    let graph = PrimalGraph::of_region(&region);
    let orders = [
        vec!["v00", "v01", "v02", "v03", "v04"],
        vec!["v04", "v03", "v02", "v01", "v00"],
        vec!["v02", "v00", "v04", "v01", "v03"],
    ];
    for order in orders {
        let order: Vec<String> = order.into_iter().map(str::to_string).collect();
        assert_eq!(graph.induced_width_of(&order).0, 4);
    }
    assert_eq!(
        elimination_order(&region, OrderStrategy::ExactMinimumWidth).induced_width,
        4
    );
}

#[test]
fn min_fill_and_min_degree_both_recover_the_band_family_width() {
    for width in 1..7 {
        let region = bioprism_backends::band_region(8, width, 2, 0xB0);
        for strategy in [OrderStrategy::MinDegree, OrderStrategy::MinFill] {
            let order = elimination_order(&region, strategy);
            assert_eq!(
                order.induced_width, width,
                "{strategy:?} missed the band width {width}"
            );
        }
    }
}

#[test]
fn exact_search_confirms_the_heuristic_optimum_on_a_small_region() {
    let region = bioprism_backends::band_region(8, 3, 2, 0xE1);
    let exact = elimination_order(&region, OrderStrategy::ExactMinimumWidth);
    let heuristic = elimination_order(&region, OrderStrategy::MinFill);

    assert_eq!(exact.used, OrderStrategy::ExactMinimumWidth);
    assert_eq!(exact.bound, bioprism_backends::Bound::Exact);
    assert_eq!(exact.induced_width, 3);
    assert_eq!(heuristic.induced_width, exact.induced_width);
}

#[test]
fn exact_search_steps_down_to_min_fill_and_records_that_it_did() {
    let region = chain(40, 2);
    let order = elimination_order(&region, OrderStrategy::ExactMinimumWidth);

    assert_eq!(order.requested, OrderStrategy::ExactMinimumWidth);
    assert_eq!(order.used, OrderStrategy::MinFill);
    assert_eq!(order.bound, bioprism_backends::Bound::HeuristicUpperBound);
    assert_eq!(order.induced_width, 1);
}

#[test]
fn induced_width_is_independent_of_region_size() {
    let large = chain(200, 2);
    let small = clique(6, 2);

    let large_width = elimination_order(&large, OrderStrategy::MinFill).induced_width;
    let small_width = elimination_order(&small, OrderStrategy::MinFill).induced_width;

    assert!(large.variable_count() > small.variable_count() * 30);
    assert!(large.factors().len() > small.factors().len() * 30);
    assert!(
        large_width < small_width,
        "the far larger region eliminated at width {large_width}, the tiny one at {small_width}"
    );
    assert!(large.joint_entries() > small.joint_entries());
}

#[test]
fn the_reference_world_query_region_is_six_factors_of_a_seven_hundred_factor_world() {
    let world = reference_world();
    let region = reference_region(&world);

    assert_eq!(world.factors.len(), 756);
    assert_eq!(world.facts.len(), 761);
    assert_eq!(region.factors().len(), 6);
    assert_eq!(region.variable_count(), 17);
    assert_eq!(region.provenance().compiled_fact_count, 11);
    assert_eq!(region.max_factor_arity(), 6);
}

#[test]
fn the_reference_world_query_region_eliminates_at_induced_width_five() {
    let world = reference_world();
    let region = reference_region(&world);
    let estimate = VariableElimination::default()
        .estimate(&region)
        .expect("the reference region fits every budget");

    assert_eq!(estimate.width_metric, WidthMetric::InducedWidth);
    assert_eq!(estimate.induced_width, Some(5));
    assert_eq!(estimate.order.len(), 16);
    assert!(
        estimate.predicted_ops() < DirectMaterialization::new().estimate(&region).unwrap().predicted_ops(),
        "elimination should be cheaper than enumerating this region's joint space"
    );
}

#[test]
fn cardinalities_come_from_the_providing_facts_and_defaults_are_declared_as_assumptions() {
    let world = reference_world();
    let region = reference_region(&world);

    assert_eq!(
        region.cardinality_source("split_assignment"),
        Some(CardinalitySource::Observed)
    );
    assert_eq!(region.cardinality_of("split_assignment"), Some(4));
    assert_eq!(
        region.cardinality_source("split_integrity_status"),
        Some(CardinalitySource::Assumed)
    );
    assert_eq!(region.cardinality_of("split_integrity_status"), Some(2));

    let assumed = region.assumed_cardinality_fraction();
    assert!((assumed - 6.0 / 17.0).abs() < 1e-12, "assumed fraction {assumed}");

    let estimate = VariableElimination::default().estimate(&region).unwrap();
    assert_eq!(estimate.uncertainty, assumed);
    assert!(estimate
        .assumptions
        .iter()
        .any(|note| note.contains("no providing fact")));
    assert!(estimate
        .assumptions
        .iter()
        .any(|note| note.contains("upper bound on the region's treewidth")));
}

#[test]
fn free_variables_are_never_eliminated() {
    let region = bioprism_backends::band_region(7, 2, 2, 0x5E);
    for strategy in [
        OrderStrategy::MinDegree,
        OrderStrategy::MinFill,
        OrderStrategy::ExactMinimumWidth,
    ] {
        let order = elimination_order(&region, strategy);
        for free in region.free_variables() {
            assert!(
                !order.order.contains(free),
                "{strategy:?} eliminated the free variable {free}"
            );
        }
        assert_eq!(order.order.len(), region.variable_count() - region.free_variables().len());
    }
}

#[test]
fn estimated_peak_intermediate_equals_the_observed_peak() {
    let region = bioprism_backends::band_region(9, 4, 3, 0xA7);
    let elimination = VariableElimination::default();

    let estimate = elimination.estimate(&region).unwrap();
    let computed = elimination.execute(&region).unwrap();

    assert_eq!(
        estimate.predicted_peak_entries,
        computed.receipt().observed_peak_entries as f64
    );
    assert_eq!(
        estimate.predicted_total_entries,
        computed.receipt().observed_total_entries as f64
    );
    assert_eq!(estimate.predicted_peak_entries, 3f64.powi(4));
}

#[test]
fn predicted_work_equals_observed_work_because_both_walk_the_same_schedule() {
    let region = bioprism_backends::band_region(9, 4, 3, 0xA7);

    for (name, estimate, receipt) in [
        (
            "elimination",
            VariableElimination::default().estimate(&region).unwrap(),
            VariableElimination::default()
                .execute(&region)
                .unwrap()
                .receipt()
                .clone(),
        ),
        (
            "direct",
            DirectMaterialization::new().estimate(&region).unwrap(),
            DirectMaterialization::new()
                .execute(&region)
                .unwrap()
                .receipt()
                .clone(),
        ),
    ] {
        assert_eq!(
            estimate.predicted_multiply_ops, receipt.observed_multiply_ops as f64,
            "{name} multiply ops"
        );
        assert_eq!(
            estimate.predicted_aggregate_ops, receipt.observed_aggregate_ops as f64,
            "{name} aggregate ops"
        );
    }
}

#[test]
fn the_direct_schedule_is_one_step_and_the_bucket_schedule_is_one_per_bound_variable() {
    let region = bioprism_backends::band_region(6, 2, 2, 0x11);
    let order = elimination_order(&region, OrderStrategy::MinFill);

    let bucket = bucket_schedule(&region, &order);
    let direct = direct_schedule(&region);

    assert_eq!(bucket.steps.len(), region.bound_variables().len() + 1);
    assert_eq!(direct.steps.len(), 1);
    assert_eq!(direct.peak_cells(), region.joint_entries());
    assert!(bucket.peak_cells() < direct.peak_cells());
    assert_eq!(direct.peak_entries(), region.free_entries());
}

#[test]
fn a_region_rejects_a_table_whose_size_contradicts_its_scope() {
    let error = QueryRegion::builder("bad")
        .variable("a", 2)
        .variable("b", 3)
        .factor(RegionFactor::with_table("f", vec!["a", "b"], vec![1.0; 5]))
        .build()
        .unwrap_err();

    assert_eq!(
        error,
        RegionError::TableSizeMismatch {
            factor: "f".into(),
            expected: 6,
            actual: 5
        }
    );
}

#[test]
fn a_region_rejects_a_potential_the_algebra_cannot_carry() {
    let error = QueryRegion::builder("bad")
        .semiring(Semiring::MaxProduct)
        .variable("a", 2)
        .factor(RegionFactor::with_table("f", vec!["a"], vec![1.0, -1.0]))
        .build()
        .unwrap_err();

    assert!(matches!(
        error,
        RegionError::InadmissibleTableEntry { index: 1, .. }
    ));

    let boolean = QueryRegion::builder("bad")
        .semiring(Semiring::Boolean)
        .variable("a", 2)
        .factor(RegionFactor::with_table("f", vec!["a"], vec![1.0, 0.5]))
        .build()
        .unwrap_err();

    assert!(matches!(
        boolean,
        RegionError::InadmissibleTableEntry { index: 1, .. }
    ));
}

#[test]
fn a_region_rejects_a_factor_over_a_variable_it_does_not_declare() {
    let error = QueryRegion::builder("bad")
        .variable("a", 2)
        .factor(RegionFactor::structural("f", vec!["a", "ghost"]))
        .build()
        .unwrap_err();

    assert_eq!(
        error,
        RegionError::UnknownVariable {
            factor: "f".into(),
            variable: "ghost".into()
        }
    );
}

#[test]
fn a_region_rejects_a_repeated_scope_variable_and_a_zero_cardinality() {
    let repeated = QueryRegion::builder("bad")
        .variable("a", 2)
        .factor(RegionFactor::structural("f", vec!["a", "a"]))
        .build()
        .unwrap_err();
    assert!(matches!(repeated, RegionError::RepeatedScopeVariable { .. }));

    let empty = QueryRegion::builder("bad")
        .variable("a", 0)
        .build()
        .unwrap_err();
    assert!(matches!(empty, RegionError::ZeroCardinality { .. }));
}

#[test]
fn a_budget_is_a_constraint_rather_than_a_cost_term() {
    let region = bioprism_backends::band_region(10, 6, 4, 0xC0);
    let generous = VariableElimination::default();
    let cramped = VariableElimination::default()
        .with_budget(Budget::default().with_max_peak_entries(1000.0));

    assert!(generous.estimate(&region).is_ok());
    let declined = cramped.estimate(&region).unwrap_err();
    assert!(matches!(
        declined,
        bioprism_backends::Declined::PeakMemoryAboveBudget { .. }
    ));
}
