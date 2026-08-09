//! Equal-engineering comparison, including the results that are inconvenient.
//!
//! Blueprint 43.41 states the stop rule plainly: "If graph baselines remain compact under equal
//! optimization, report that result." The same obligation applies to any baseline that matches
//! FIBER. These tests pin the measurements as they actually are on the reference world, including
//! the one where a lexical retriever ties the compiler.

use bioprism_baseline::{
    compare, default_panel, ConnectedComponent, ContextStrategy, FiberCompiled, KHopIncidence,
    LexicalTopK, QueryGraph,
};
use bioprism_fiber::Query;
use bioprism_world::World;
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

fn world() -> World {
    World::from_json(fixture("radiogenomic_world.json")).unwrap()
}

fn query() -> Query {
    Query::from_json(fixture("leakage_query.json")).unwrap()
}

#[test]
fn the_full_panel_agrees_with_the_reference_baseline_script() {
    let world = world();
    let query = query();
    let panel = default_panel();
    let borrowed: Vec<&dyn ContextStrategy> = panel.iter().map(|b| b.as_ref()).collect();
    let comparison = compare(&world, &query, &borrowed);

    let by_name = |name: &str| {
        comparison
            .results
            .iter()
            .find(|r| r.name == name)
            .unwrap_or_else(|| panic!("missing strategy {name}"))
    };

    assert_eq!(by_name("full-context").facts_exposed, 761);
    assert_eq!(by_name("graph-7-hop").facts_exposed, 761);
    assert_eq!(by_name("hypergraph-component").facts_exposed, 761);
    assert_eq!(by_name("fiber").facts_exposed, 11);
}

/// The graph baseline is not beaten on this world — it is beaten only by a *mistuned* one.
///
/// The depth sweep is `[0, 0, 0, 0, 11, 11, 761, 761]`. At depth 5 or 6 the incidence walk
/// selects **exactly the eleven facts FIBER compiles**, soundly, with full protected recall.
/// Below that it returns nothing; above it, the whole world.
///
/// This matters because the reference distribution's own `compare_baselines.py` measures the
/// graph baseline at depth 7 and unbounded only — the two settings where it explodes — and never
/// at 5 or 6. 43.38 calls that an unequal-engineering comparison, and 43.41 requires reporting it
/// when "graph baselines remain compact under equal optimization".
///
/// What survives as a real distinction is the *width of the correct window*: two values out of
/// eight, with unsound results on one side and no compression on the other, and no way to know
/// which from the query alone. FIBER has no such knob.
#[test]
fn a_correctly_tuned_graph_walk_matches_fiber_exactly() {
    let world = world();
    let query = query();

    let counts: Vec<usize> = (1..=8)
        .map(|depth| KHopIncidence { depth }.select(&world, &query).facts.len())
        .collect();
    assert_eq!(
        counts,
        vec![0, 0, 0, 0, 11, 11, 761, 761],
        "the published depth sweep changed"
    );

    let compiled = FiberCompiled.select(&world, &query).facts;
    for depth in [5, 6] {
        let walked = KHopIncidence { depth }.select(&world, &query).facts;
        assert_eq!(
            walked, compiled,
            "at depth {depth} the graph walk selects exactly the compiled set"
        );
    }

    let sound_depths: Vec<usize> = (1..=8)
        .filter(|depth| {
            let strategy = KHopIncidence { depth: *depth };
            let panel: Vec<&dyn ContextStrategy> = vec![&strategy];
            compare(&world, &query, &panel).results[0].verdict_preserving
        })
        .collect();
    assert_eq!(
        sound_depths,
        vec![5, 6, 7, 8],
        "depths 1-4 drop every decisive witness"
    );

    let compact_and_sound: Vec<usize> = sound_depths
        .iter()
        .copied()
        .filter(|depth| KHopIncidence { depth: *depth }.select(&world, &query).facts.len() < 761)
        .collect();
    assert_eq!(
        compact_and_sound,
        vec![5, 6],
        "the usable window is two settings wide and cannot be derived from the query"
    );
}

#[test]
fn shallow_graph_selections_are_unsound_not_merely_small() {
    let world = world();
    let query = query();
    let shallow = KHopIncidence { depth: 2 };
    let panel: Vec<&dyn ContextStrategy> = vec![&shallow, &QueryGraph, &FiberCompiled];
    let comparison = compare(&world, &query, &panel);

    let graph = comparison
        .results
        .iter()
        .find(|r| r.name == "graph-2-hop")
        .unwrap();
    assert_eq!(graph.facts_exposed, 0);
    assert!(!graph.verdict_preserving);
    assert_eq!(graph.missing_witnesses.len(), 4);
    assert_eq!(graph.protected_recall, 0.0);

    let fiber = comparison.results.iter().find(|r| r.name == "fiber").unwrap();
    assert!(fiber.verdict_preserving);
    assert_eq!(fiber.protected_recall, 1.0);

    assert_eq!(
        comparison.cheapest_admissible().map(|r| r.name.as_str()),
        Some("fiber")
    );
}

/// The inconvenient result, pinned so it cannot quietly disappear.
///
/// A BM25 retriever given the query's protected tags matches FIBER exactly on this world: same
/// 11 facts, same verdict, full protected recall. That is not evidence that FIBER works — it is
/// evidence that this world does not *discriminate*, because the protected-tag list names the
/// decisive facts almost uniquely, so lexical scoring over tags reads the answer key.
///
/// 43.39 calls for synthetic families that vary structure independently; this test is the
/// standing reason one is needed.
#[test]
fn a_lexical_retriever_ties_fiber_on_this_world() {
    let world = world();
    let query = query();

    let lexical = LexicalTopK { k: 11 };
    let selected = lexical.select(&world, &query);
    let compiled = FiberCompiled.select(&world, &query);

    assert_eq!(selected.facts.len(), compiled.facts.len());
    assert_eq!(
        selected.facts, compiled.facts,
        "BM25 over protected tags selects exactly the compiled set on this world"
    );

    let panel: Vec<&dyn ContextStrategy> = vec![&lexical, &FiberCompiled];
    let comparison = compare(&world, &query, &panel);
    assert!(comparison.results.iter().all(|r| r.verdict_preserving));
}

/// Widening the lexical budget stops being free once it must guess.
///
/// At k=50 the retriever keeps its 11 decisive facts and adds 39 distractors: same verdict, four
/// and a half times the context. On this world FIBER's advantage is not recall, it is knowing
/// when to stop.
#[test]
fn lexical_retrieval_cannot_tell_when_to_stop() {
    let world = world();
    let query = query();

    let narrow = LexicalTopK { k: 11 }.select(&world, &query);
    let wide = LexicalTopK { k: 50 }.select(&world, &query);

    assert!(narrow.facts.is_subset(&wide.facts));
    assert_eq!(wide.facts.len(), 50);

    let panel: Vec<&dyn ContextStrategy> = vec![&LexicalTopK { k: 50 }, &FiberCompiled];
    let comparison = compare(&world, &query, &panel);
    let lexical = comparison
        .results
        .iter()
        .find(|r| r.name == "lexical-top-50")
        .unwrap();
    let fiber = comparison.results.iter().find(|r| r.name == "fiber").unwrap();

    assert!(lexical.verdict_preserving && fiber.verdict_preserving);
    assert!(lexical.facts_exposed > fiber.facts_exposed);
}

/// Every strategy sees the same world and the same query.
#[test]
fn no_strategy_receives_privileged_access() {
    let world = world();
    let query = query();
    let panel = default_panel();

    for strategy in &panel {
        let selection = strategy.select(&world, &query);
        assert!(
            selection.facts.iter().all(|id| world.fact(id).is_some()),
            "{} returned an id that is not a fact in the shared world",
            strategy.name()
        );
        assert!(
            !strategy.method().is_empty(),
            "{} must declare its method so a reader can judge fairness",
            strategy.name()
        );
    }
}

#[test]
fn the_comparison_serialises_for_ci_consumption() {
    let world = world();
    let query = query();
    let panel = default_panel();
    let borrowed: Vec<&dyn ContextStrategy> = panel.iter().map(|b| b.as_ref()).collect();
    let comparison = compare(&world, &query, &borrowed);

    let document = comparison.to_json();
    assert_eq!(document["total_facts"], Value::from(761));
    assert_eq!(document["reference"]["status"], Value::from("invalid"));
    assert_eq!(document["results"].as_array().unwrap().len(), 10);

    let markdown = comparison.to_markdown();
    assert!(markdown.contains("Facts exposed is a cost, not a score"));
    assert!(markdown.contains("not universal superiority"));
}

#[test]
fn the_connected_component_is_the_whole_world() {
    let world = world();
    let query = query();
    assert_eq!(ConnectedComponent.select(&world, &query).facts.len(), 761);
}
