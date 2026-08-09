//! 43.18's normative decision, tested against 43.39's structural generators.
//!
//! "Never use total graph size as the primary complexity predictor" is a claim about two things
//! varying independently, so it needs worlds where they do. `bioprism-worldgen` varies distractor
//! count — pure size, no new dependency path — separately from relay depth, which lengthens the
//! chain the query must eliminate along. The width statistic should be blind to the first and
//! responsive to the second, and if it were not, it would be a size statistic wearing a different
//! name.

use bioprism_backends::{
    elimination_order, CardinalityPolicy, DirectMaterialization, OrderStrategy, Portfolio,
    QueryBackend, QueryRegion, VariableElimination,
};
use bioprism_section::Backend;
use bioprism_world::World;
use bioprism_worldgen::{generate, WorldSpec};

fn region_of(spec: &WorldSpec) -> (World, QueryRegion) {
    let generated = generate(spec);
    let world = World::from_json(generated.world).expect("the generated world loads");
    let region = QueryRegion::from_world_slice(
        &world,
        spec.world_id.clone(),
        ["split_integrity_status"],
        &CardinalityPolicy::default(),
    )
    .expect("the sliced region is well formed");
    (world, region)
}

#[test]
fn growing_a_world_by_two_thousand_distractors_does_not_move_the_query_width() {
    let (small_world, small) = region_of(&WorldSpec::reference_like(8));
    let (large_world, large) = region_of(&WorldSpec::reference_like(2000));

    assert!(
        large_world.facts.len() > small_world.facts.len() * 20,
        "{} against {}",
        large_world.facts.len(),
        small_world.facts.len()
    );

    let small_order = elimination_order(&small, OrderStrategy::MinFill);
    let large_order = elimination_order(&large, OrderStrategy::MinFill);

    assert_eq!(large.factors().len(), small.factors().len());
    assert_eq!(large.variable_count(), small.variable_count());
    assert_eq!(large_order.induced_width, small_order.induced_width);
    assert_eq!(large_order.order, small_order.order);

    let small_cost = VariableElimination::default()
        .estimate(&small)
        .unwrap()
        .predicted_ops();
    let large_cost = VariableElimination::default()
        .estimate(&large)
        .unwrap()
        .predicted_ops();
    assert_eq!(small_cost, large_cost);
}

#[test]
fn relay_depth_changes_the_region_because_it_changes_the_dependency_path() {
    let (_, flat) = region_of(&WorldSpec::reference_like(8));
    let (_, relayed) = region_of(&WorldSpec::discriminating(8));

    assert!(
        relayed.factors().len() > flat.factors().len(),
        "relays should add factors to the slice: {} against {}",
        relayed.factors().len(),
        flat.factors().len()
    );
    assert!(relayed.variable_count() > flat.variable_count());
}

#[test]
fn the_plan_descriptor_separates_the_compiled_region_from_the_world_it_came_from() {
    let (world, region) = region_of(&WorldSpec::reference_like(2000));
    let selection = Portfolio::reference().select(&region).unwrap();
    let plan = selection.plan_descriptor(&region);

    assert_eq!(plan.total_fact_count, world.facts.len());
    assert_eq!(plan.total_factor_count, world.factors.len());
    assert!(plan.compiled_factor_count < plan.total_factor_count / 50);
    assert!(plan.fact_selection_ratio() < 0.02);
    assert!(plan.factor_selection_ratio() < 0.02);
}

#[test]
fn a_generated_region_under_uniform_potentials_still_agrees_with_enumeration() {
    let (_, region) = region_of(&WorldSpec::reference_like(8));
    let region = region
        .with_uniform_tables()
        .expect("the generated factors are small enough to materialise");

    let eliminated = VariableElimination::default().execute(&region).unwrap();
    let enumerated = DirectMaterialization::new().execute(&region).unwrap();

    assert!(eliminated.agrees_exactly_with(&enumerated));
    assert_eq!(
        eliminated.receipt().backend,
        Backend::FaqInsideOut
    );
    assert_eq!(
        enumerated.receipt().backend,
        Backend::DirectMaterialization
    );
    assert!(
        eliminated.receipt().observed_ops() < enumerated.receipt().observed_ops(),
        "elimination executed {} operations against enumeration's {}",
        eliminated.receipt().observed_ops(),
        enumerated.receipt().observed_ops()
    );
}
