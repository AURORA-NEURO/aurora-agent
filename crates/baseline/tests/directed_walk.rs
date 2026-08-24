//! The directed dependency walk, and the negative result it delivers.
//!
//! `docs/FINDINGS.md` predicted a directed-edge walk "would recover much of what backward slicing
//! does". Measured, the prediction understates it: the unbounded walk selects **exactly** the
//! facts FIBER compiles — the identical set, not the same count — on the reference world *and* on
//! the discriminating world that defeats every other baseline. Equal engineering therefore leaves
//! FIBER's selection behaviour indistinguishable from a fair directed baseline on every world in
//! the shipped sweep. That is a result against FIBER and it ships as such, per 43.41.
//!
//! The tests also pin *why* the tie is overdetermined here: the mandatory protected closure alone
//! is already decision-sufficient on these worlds, so the walk would be admissible even with an
//! empty slice. What survives as FIBER-only is not selection: it is the temporal cut, the policy
//! screen and the certificate — none of which these worlds' verdicts exercise.

use bioprism_baseline::{compare, ContextStrategy, DirectedDependencyWalk, FiberCompiled};
use bioprism_fiber::Query;
use bioprism_world::World;
use bioprism_worldgen::{generate, WorldSpec};
use serde_json::Value;
use std::path::PathBuf;

fn fixture(name: &str) -> Value {
    let path: PathBuf = [
        env!("CARGO_MANIFEST_DIR"),
        "..",
        "..",
        "fixtures",
        "fiber-v0.1",
        name,
    ]
    .iter()
    .collect();
    serde_json::from_str(&std::fs::read_to_string(&path).expect("fixture readable"))
        .expect("fixture is valid JSON")
}

fn reference() -> (World, Query) {
    (
        World::from_json(fixture("radiogenomic_world.json")).expect("the shipped world loads"),
        Query::from_json(fixture("leakage_query.json")).expect("the shipped query loads"),
    )
}

fn discriminating() -> (World, Query) {
    let generated = generate(&WorldSpec::discriminating(750));
    (
        World::from_json(generated.world).expect("generated world loads"),
        Query::from_json(generated.query).expect("generated query loads"),
    )
}

fn admissible(world: &World, query: &Query, strategy: &dyn ContextStrategy) -> bool {
    let panel: Vec<&dyn ContextStrategy> = vec![strategy];
    compare(world, query, &panel).expect("verdict exists").results[0].admissible()
}

/// The negative result, reference-world half: not a similar selection, the identical one.
///
/// Even the raw slice — no protected closure — reproduces FIBER's eleven facts, so the tie is the
/// walk's own and not the closure's.
#[test]
fn the_unbounded_walk_selects_exactly_the_facts_fiber_compiles_a_tie_not_a_win_for_fiber() {
    let (world, query) = reference();
    let walk = DirectedDependencyWalk::unbounded();
    let compiled = FiberCompiled.select(&world, &query).facts;

    assert_eq!(walk.select(&world, &query).facts, compiled);
    assert_eq!(walk.slice(&world, &query), compiled, "the slice alone already ties");
    assert!(admissible(&world, &query, &walk));
}

/// The discriminating world was built to separate FIBER from adjacency and from lexical
/// similarity, and does. It does not separate FIBER from directed dependency: same eleven facts,
/// identical set, admissible. The separation FINDINGS.md §3 reports is against *undirected*
/// adjacency; direction alone closes the whole gap.
#[test]
fn the_discriminating_world_does_not_separate_fiber_from_the_directed_walk() {
    let (world, query) = discriminating();
    let walk = DirectedDependencyWalk::unbounded();
    let compiled = FiberCompiled.select(&world, &query).facts;

    assert_eq!(compiled.len(), 11);
    assert_eq!(walk.select(&world, &query).facts, compiled);
    assert_eq!(walk.slice(&world, &query), compiled);
    assert!(admissible(&world, &query, &walk));
}

/// The depth ladder, stated per world so the walk's one tuning knob is on the record.
///
/// The raw slice needs depth 2 on the reference world (claim factor, then checks) and depth 5 on
/// the discriminating world (claim factor, three relays, then checks); below that it is empty,
/// and from the threshold up it is exactly FIBER's set. The window is one-sided — unlike the
/// undirected walk's two-value window, over-shooting the depth never admits a distractor, because
/// distractor factors only *consume* decisive variables and a backward step never enters a
/// consumer.
#[test]
fn the_raw_slice_is_empty_below_the_dependency_depth_and_exactly_fibers_set_from_it_upward() {
    let (world, query) = reference();
    let compiled = FiberCompiled.select(&world, &query).facts;
    for depth in 0..=1 {
        let walk = DirectedDependencyWalk { depth: Some(depth) };
        assert!(walk.slice(&world, &query).is_empty(), "depth {depth} reaches no fact");
    }
    for depth in 2..=8 {
        let walk = DirectedDependencyWalk { depth: Some(depth) };
        assert_eq!(walk.slice(&world, &query), compiled, "depth {depth}");
    }

    let (world, query) = discriminating();
    let compiled = FiberCompiled.select(&world, &query).facts;
    for depth in 0..=4 {
        let walk = DirectedDependencyWalk { depth: Some(depth) };
        assert!(
            walk.slice(&world, &query).is_empty(),
            "depth {depth} is inside the relay chain and reaches no fact"
        );
    }
    for depth in 5..=8 {
        let walk = DirectedDependencyWalk { depth: Some(depth) };
        assert_eq!(walk.slice(&world, &query), compiled, "depth {depth}");
    }
}

/// The benchmark property that overdetermines the tie, pinned so nobody credits selection skill
/// the worlds cannot measure: with the mandatory closure taken first, even a depth-0 walk — whose
/// slice is empty — is admissible on both shipped worlds, because the closure alone carries every
/// decisive witness. Any admissibility comparison on these worlds is a comparison of closures.
#[test]
fn the_protected_closure_alone_is_decision_sufficient_so_even_a_depth_zero_walk_is_admissible() {
    for (world, query) in [reference(), discriminating()] {
        let stump = DirectedDependencyWalk { depth: Some(0) };
        assert!(stump.slice(&world, &query).is_empty());
        assert_eq!(
            stump.select(&world, &query).facts,
            FiberCompiled.select(&world, &query).facts,
            "closure alone equals the compiled selection on this world"
        );
        assert!(admissible(&world, &query, &stump));
    }
}
