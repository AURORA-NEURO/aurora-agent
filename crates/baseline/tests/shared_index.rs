//! The panel's shared intermediates must be an accounting change and nothing else.
//!
//! `bioprism_baseline::index` exists to stop five incidence builds and four full-corpus
//! tokenisations happening per comparison. That is a claim about how often work runs, and it is
//! only worth anything if the work still returns what it returned when each strategy did its own —
//! otherwise the sweep got faster by measuring something else.
//!
//! Both entry points survive so the two can be compared: [`ContextStrategy::select`] builds a
//! private index of one, which is exactly the pre-sharing behaviour, and
//! [`ContextStrategy::select_indexed`] reads the index `compare` fills once and lends to every
//! strategy in turn. These tests run the shipped panels both ways and require the selections to
//! agree fact for fact and note for note.

use bioprism_baseline::{compare, default_panel, sweep_panel, ContextStrategy, PanelIndex};
use bioprism_fiber::Query;
use bioprism_world::World;
use bioprism_worldgen::{generate, TagStyle, WorldSpec};
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

fn reference_world() -> (World, Query) {
    (
        World::from_json(fixture("radiogenomic_world.json")).expect("the shipped world loads"),
        Query::from_json(fixture("leakage_query.json")).expect("the shipped query loads"),
    )
}

/// A camouflaged world, where the two retrieval rankings are the intermediates under most strain:
/// distractor tags share trigrams with the protected vocabulary, so scores are close together and
/// any drift in a shared corpus would reorder the list rather than leave it alone.
fn camouflaged_world() -> (World, Query) {
    let mut spec = WorldSpec::reference_like(750);
    spec.tag_style = TagStyle::Camouflaged;
    let generated = generate(&spec);
    (
        World::from_json(generated.world).expect("the generated world loads"),
        Query::from_json(generated.query).expect("the generated query loads"),
    )
}

/// Runs one panel both ways over one world, in panel order, against a single shared index — the
/// order and the reuse `compare` itself produces.
fn both_ways_agree(panel: &[Box<dyn ContextStrategy>], world: &World, query: &Query) {
    let shared = PanelIndex::new(world, query);
    for strategy in panel {
        let own = strategy.select(world, query);
        let borrowed = strategy.select_indexed(&shared);
        assert_eq!(
            own.facts,
            borrowed.facts,
            "{} selected different facts from the shared index",
            strategy.name()
        );
        assert_eq!(
            own.notes,
            borrowed.notes,
            "{} reported different notes from the shared index",
            strategy.name()
        );
    }
}

#[test]
fn every_default_panel_strategy_selects_the_same_facts_from_a_shared_index_as_from_its_own() {
    let (world, query) = reference_world();
    both_ways_agree(&default_panel(), &world, &query);
}

#[test]
fn every_sweep_panel_strategy_selects_the_same_facts_from_a_shared_index_as_from_its_own() {
    let (world, query) = camouflaged_world();
    both_ways_agree(&sweep_panel(), &world, &query);
}

/// The index is filled lazily, so the first strategy to want a cell pays for it and later ones do
/// not. A panel whose members ran in a different order would fill the cells in a different order,
/// and must still reach the same table — otherwise a cached value is carrying state from whoever
/// asked first.
#[test]
fn panel_order_does_not_change_any_row_the_comparison_reports() {
    let (world, query) = camouflaged_world();
    let panel = default_panel();

    let forward: Vec<&dyn ContextStrategy> = panel.iter().map(|boxed| boxed.as_ref()).collect();
    let mut reversed = forward.clone();
    reversed.reverse();

    let first = compare(&world, &query, &forward).expect("the generated world reaches a verdict");
    let second = compare(&world, &query, &reversed).expect("the generated world reaches a verdict");

    assert_eq!(first.reference_status, second.reference_status);
    assert_eq!(first.reference_witnesses, second.reference_witnesses);

    for result in &first.results {
        let mirrored = second
            .results
            .iter()
            .find(|other| other.name == result.name)
            .unwrap_or_else(|| panic!("{} is missing from the reversed panel", result.name));
        assert_eq!(
            result.facts_exposed, mirrored.facts_exposed,
            "{} exposed a different number of facts when the panel ran backwards",
            result.name
        );
        assert_eq!(
            result.verdict_preserving(),
            mirrored.verdict_preserving(),
            "{} reached a different verdict when the panel ran backwards",
            result.name
        );
        assert_eq!(
            result.notes, mirrored.notes,
            "{} drifted its notes",
            result.name
        );
    }
}

/// A caller's panel of one must not pay for intermediates nobody reads.
///
/// The saving would otherwise be a redistribution: [`compare`] is public API and
/// `bioprism_routing`'s lab reaches it with caller-supplied panels, so an index that eagerly built
/// a token corpus would charge a single graph strategy for a corpus it never touches. The cells
/// are lazy for that reason, and the selection a lone strategy makes is the proof it is unaffected.
#[test]
fn a_panel_of_one_reaches_the_same_row_it_reaches_inside_the_full_panel() {
    let (world, query) = reference_world();
    let panel = default_panel();
    let full: Vec<&dyn ContextStrategy> = panel.iter().map(|boxed| boxed.as_ref()).collect();
    let together = compare(&world, &query, &full).expect("the shipped world reaches a verdict");

    for strategy in &panel {
        let alone = compare(&world, &query, &[strategy.as_ref()])
            .expect("the shipped world reaches a verdict");
        let row = &alone.results[0];
        let inside = together
            .results
            .iter()
            .find(|other| other.name == row.name)
            .unwrap_or_else(|| panic!("{} is missing from the full panel", row.name));
        assert_eq!(
            row.facts_exposed, inside.facts_exposed,
            "{} exposed a different number of facts on its own",
            row.name
        );
        assert_eq!(
            row.notes, inside.notes,
            "{} drifted its notes on its own",
            row.name
        );
    }
}
