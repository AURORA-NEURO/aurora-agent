//! Equal-engineering comparison, including the results that are inconvenient.
//!
//! Blueprint 43.41 states the stop rule plainly: "If graph baselines remain compact under equal
//! optimization, report that result." The same obligation applies to any baseline that matches
//! FIBER. These tests pin the measurements as they actually are on the reference world, including
//! the one where a lexical retriever ties the compiler.

use bioprism_baseline::{
    compare, default_panel, CompareError, Comparison, ConnectedComponent, ContextStrategy,
    FiberCompiled, Judgement, KHopIncidence, LexicalTopK, QueryGraph, RowRefusal, RowVerdict,
    Selection,
};
use bioprism_fiber::{FiberError, Query};
use bioprism_section::OracleStatus;
use bioprism_world::World;
use serde_json::{json, Value};
use std::collections::BTreeSet;
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

/// Every shipped world reaches a full-context verdict, so a test that does not say otherwise is
/// entitled to a table rather than an error.
fn compared(world: &World, query: &Query, panel: &[&dyn ContextStrategy]) -> Comparison {
    compare(world, query, panel).expect("the shipped world reaches a full-context verdict")
}

#[test]
fn the_full_panel_agrees_with_the_reference_baseline_script() {
    let world = world();
    let query = query();
    let panel = default_panel();
    let borrowed: Vec<&dyn ContextStrategy> = panel.iter().map(|b| b.as_ref()).collect();
    let comparison = compared(&world, &query, &borrowed);

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
            compared(&world, &query, &panel).results[0].verdict_preserving() == Some(true)
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
    let comparison = compared(&world, &query, &panel);

    let graph = comparison
        .results
        .iter()
        .find(|r| r.name == "graph-2-hop")
        .unwrap();
    assert_eq!(graph.facts_exposed, 0);
    assert_eq!(graph.verdict_preserving(), Some(false));
    assert_eq!(graph.judgement().unwrap().missing_witnesses.len(), 4);
    assert_eq!(graph.protected_recall, 0.0);

    let fiber = comparison.results.iter().find(|r| r.name == "fiber").unwrap();
    assert_eq!(fiber.verdict_preserving(), Some(true));
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
    let comparison = compared(&world, &query, &panel);
    assert!(comparison
        .results
        .iter()
        .all(|r| r.verdict_preserving() == Some(true)));
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
    let comparison = compared(&world, &query, &panel);
    let lexical = comparison
        .results
        .iter()
        .find(|r| r.name == "lexical-top-50")
        .unwrap();
    let fiber = comparison.results.iter().find(|r| r.name == "fiber").unwrap();

    assert_eq!(lexical.verdict_preserving(), Some(true));
    assert_eq!(fiber.verdict_preserving(), Some(true));
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
    let comparison = compared(&world, &query, &borrowed);

    let document = comparison.to_json();
    assert_eq!(document["total_facts"], Value::from(761));
    assert_eq!(document["reference"]["status"], Value::from("invalid"));
    assert_eq!(document["results"].as_array().unwrap().len(), 13);

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

/// A strategy that selects exactly what a test tells it to.
///
/// Needed because the shipped panel cannot reach a refusal on any shipped world, and a state that
/// only ever exists in prose is a state nobody has checked.
struct Fixed {
    label: &'static str,
    facts: BTreeSet<String>,
}

impl Fixed {
    fn of(label: &'static str, ids: &[&str]) -> Self {
        Fixed {
            label,
            facts: ids.iter().map(|id| (*id).to_string()).collect(),
        }
    }
}

impl ContextStrategy for Fixed {
    fn name(&self) -> String {
        self.label.into()
    }

    fn method(&self) -> String {
        "a fixed selection supplied by the test".into()
    }

    fn select(&self, _world: &World, _query: &Query) -> Selection {
        Selection::new(self.facts.clone())
    }
}

/// The shipped fixture with one subject's split arm deleted.
///
/// `ALT-77` then spans a subject assigned to `train` and a subject assigned to nothing, which is
/// the condition `bioprism_fiber::oracle` refuses on. It is the same edit `crates/mutation` makes
/// to build a parent nobody can judge.
fn world_the_oracle_refuses() -> World {
    let mut raw = fixture("radiogenomic_world.json");
    for fact in raw["facts"].as_array_mut().expect("facts are an array") {
        if fact["id"] == "fact.split" {
            fact["value"]
                .as_object_mut()
                .expect("split assignments are an object")
                .remove("S003");
        }
    }
    World::from_json(raw).expect("a world the oracle refuses still loads")
}

/// The same fixture, with a complete second `split_assignment` sorting after the broken one.
///
/// The last fact wins when two provide the same variable, so full-context reads the complete
/// assignment and reaches a verdict while a selection holding only the broken one is refused. That
/// is the sole shape in which a row can refuse under a reference that did not, and it is why
/// `compare` keeps a row-level state as well as an error.
fn world_where_only_a_subset_is_refused() -> World {
    let mut raw = fixture("radiogenomic_world.json");
    let facts = raw["facts"].as_array_mut().expect("facts are an array");
    let mut complete = facts
        .iter()
        .find(|fact| fact["id"] == "fact.split")
        .cloned()
        .expect("the fixture assigns splits");
    complete["id"] = json!("fact.split.repaired");
    for fact in facts.iter_mut() {
        if fact["id"] == "fact.split" {
            fact["value"]
                .as_object_mut()
                .expect("split assignments are an object")
                .remove("S003");
        }
    }
    facts.push(complete);
    World::from_json(raw).expect("a world with a shadowed variable still loads")
}

/// The defect, stated as the thing that can no longer happen.
///
/// The reference and every strategy used to collapse to `abstain` with an empty witness list,
/// compare equal, and be published as a panel of verdict-preserving admissible strategies. There is
/// now no table at all: the type says a comparison needs a reference verdict, and none was obtained.
#[test]
fn two_refused_verdicts_do_not_compare_equal() {
    let world = world_the_oracle_refuses();
    let query = query();
    let panel = default_panel();
    let borrowed: Vec<&dyn ContextStrategy> = panel.iter().map(|b| b.as_ref()).collect();

    match compare(&world, &query, &borrowed) {
        Err(CompareError::OracleRefusedReference { facts, source }) => {
            assert_eq!(facts, 761);
            assert!(
                matches!(source, FiberError::UnorderableSplitGroups { .. }),
                "the oracle's own error must survive, got {source:?}"
            );
        }
        Ok(comparison) => panic!(
            "a comparison was published against a reference nobody obtained: cheapest admissible \
             was {:?}",
            comparison.cheapest_admissible().map(|r| r.name.clone())
        ),
    }
}

/// The row-level half, and every derived field that used to read as a measurement.
#[test]
fn a_refused_row_is_neither_sound_nor_admissible() {
    let world = world_where_only_a_subset_is_refused();
    let query = query();
    let broken = Fixed::of("broken-split", &["fact.subject_aliases", "fact.split"]);
    let panel: Vec<&dyn ContextStrategy> = vec![&FiberCompiled, &broken];
    let comparison = compared(&world, &query, &panel);

    assert_eq!(comparison.reference_status, OracleStatus::Invalid);

    let row = comparison
        .results
        .iter()
        .find(|r| r.name == "broken-split")
        .expect("the fixed strategy has a row");

    assert!(row.judgement().is_none());
    assert_eq!(row.verdict_preserving(), None);
    assert_eq!(row.status(), None);
    assert!(!row.admissible());
    assert!(matches!(
        row.refusal(),
        Some(RowRefusal::OracleRefused {
            facts_exposed: 2,
            ..
        })
    ));

    assert!(
        !comparison.unsound().any(|r| r.name == "broken-split"),
        "a refusal is not a refutation, so an unexamined row must not be reported as unsound"
    );
    assert!(!comparison.lucky().any(|r| r.name == "broken-split"));
    assert_eq!(comparison.refused().count(), 1);
    assert!(!comparison.is_fully_judged());
    assert_eq!(
        comparison.cheapest_admissible().map(|r| r.name.as_str()),
        Some("fiber"),
        "a row nobody judged must never win the cost ranking"
    );
}

/// A refusal must not reach a reader as a blank, a zero or a `false`.
#[test]
fn a_refused_row_renders_as_refused_in_both_the_table_and_the_json() {
    let world = world_where_only_a_subset_is_refused();
    let query = query();
    let broken = Fixed::of("broken-split", &["fact.subject_aliases", "fact.split"]);
    let panel: Vec<&dyn ContextStrategy> = vec![&FiberCompiled, &broken];
    let comparison = compared(&world, &query, &panel);

    let markdown = comparison.to_markdown();
    let row = markdown
        .lines()
        .find(|line| line.starts_with("| broken-split |"))
        .expect("the refused strategy has a table row");
    assert!(row.contains("**refused**"), "table row was {row:?}");
    assert!(
        !row.contains("underdetermined") && !row.contains("yes"),
        "a refused row must not borrow a verdict it never received: {row:?}"
    );
    assert!(markdown.contains("`broken-split` was **not judged**"));
    assert!(markdown.contains("cannot be admissible"));

    let document = comparison.to_json();
    let rows = document["results"].as_array().expect("results are an array");
    let refused = rows
        .iter()
        .find(|row| row["name"] == "broken-split")
        .expect("the refused strategy has a JSON row");

    assert_eq!(refused["judged"], Value::from(false));
    assert!(refused["refusal"]
        .as_str()
        .expect("the refusal is reported verbatim")
        .contains("ALT-77"));
    for absent in [
        "status",
        "verdict_preserving",
        "missing_witnesses",
        "spurious_witnesses",
        "witnesses",
        "admissible",
    ] {
        assert!(
            refused.get(absent).is_none(),
            "a refused row must carry no {absent:?} key for a reader to default or count"
        );
    }

    let judged = rows
        .iter()
        .find(|row| row["name"] == "fiber")
        .expect("fiber has a JSON row");
    assert_eq!(judged["judged"], Value::from(true));
    assert_eq!(judged["verdict_preserving"], Value::from(true));
    assert!(judged.get("refusal").is_none());
}

/// An abstention and a refusal are two values, not one.
///
/// Constructed rather than measured, because `bioprism_fiber::oracle::evaluate` builds its verdict
/// with `OracleVerdict::new`, which returns only `valid` or `invalid` — the shipped oracle never
/// abstains, so `underdetermined` in a published comparison table could only ever have been a
/// refusal wearing an abstention's clothes. The distinction is pinned here so that an oracle which
/// does abstain cannot silently re-merge the two.
#[test]
fn an_abstention_and_a_refusal_do_not_share_a_representation() {
    let abstained = RowVerdict::Judged(Judgement {
        status: OracleStatus::Underdetermined,
        witnesses: Vec::new(),
        verdict_preserving: true,
        missing_witnesses: Vec::new(),
        spurious_witnesses: Vec::new(),
    });
    let refused = RowVerdict::Refused(RowRefusal::OracleRefused {
        facts_exposed: 0,
        source: FiberError::UnorderableSplitGroups {
            alias: "ALT-77".into(),
            present: vec!["train".into()],
        },
    });
    assert_ne!(abstained, refused);
}
