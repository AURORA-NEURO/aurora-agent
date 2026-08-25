//! The oracle's fact-shaped entry points agree with the full value map.
//!
//! [`oracle::evaluate_facts`] skips facts whose variable no check reads, which is sound only
//! because the oracle looks up a fixed set of keys and never iterates the map. That is a property
//! of the current checks, not a guarantee of the type, so it is pinned here rather than asserted
//! in prose: a seventh variable added to a check but not to the skip list would leave the two
//! paths disagreeing, and these tests are what notices.

use bioprism_fiber::oracle;
use bioprism_section::OracleVerdict;
use bioprism_world::{Fact, World};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

fn golden_world() -> World {
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
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("missing fixture {}: {e}", path.display()));
    World::from_json(serde_json::from_str(&text).expect("fixture is valid JSON"))
        .expect("golden world loads")
}

/// The unnarrowed map every caller built before the oracle learned to read facts directly.
fn full_value_map(world: &World) -> BTreeMap<String, Value> {
    world
        .facts
        .iter()
        .map(|fact| (fact.provides.as_str().to_string(), fact.value.clone()))
        .collect()
}

#[test]
fn narrowing_to_the_variables_the_oracle_reads_preserves_the_verdict() {
    let world = golden_world();
    let full = oracle::evaluate(&full_value_map(&world)).expect("full map evaluates");
    let narrowed = oracle::evaluate_facts(world.facts.iter()).expect("facts evaluate");

    assert!(
        !full.witnesses.is_empty(),
        "the golden world must produce witnesses, or this test compares two empty verdicts \
         and proves nothing"
    );
    assert_eq!(narrowed, full);
}

#[test]
fn the_narrowed_map_is_far_smaller_than_the_world_it_came_from() {
    let world = golden_world();
    assert!(
        world.facts.len() > 700,
        "the golden world is the wide one; a narrow world would not exercise the saving"
    );
    assert_eq!(
        full_value_map(&world).len(),
        world.facts.len(),
        "every golden fact provides a distinct variable, so the map the callers used to clone \
         was as wide as the world"
    );
}

#[test]
fn selecting_every_fact_by_id_matches_iterating_them() {
    let world = golden_world();
    let ids: BTreeSet<String> = world
        .facts
        .iter()
        .map(|fact| fact.id.as_str().to_string())
        .collect();

    let selected = oracle::evaluate_selected(&world, &ids).expect("selection evaluates");
    let iterated = oracle::evaluate_facts(world.facts.iter()).expect("facts evaluate");
    assert_eq!(selected, iterated);
}

#[test]
fn a_selection_naming_an_unknown_fact_is_judged_on_the_facts_that_exist() {
    let world = golden_world();
    let mut ids: BTreeSet<String> = world
        .facts
        .iter()
        .map(|fact| fact.id.as_str().to_string())
        .collect();
    let known: OracleVerdict =
        oracle::evaluate_selected(&world, &ids).expect("selection evaluates");

    ids.insert("fact_that_is_not_in_this_world".into());
    let with_phantom = oracle::evaluate_selected(&world, &ids).expect("selection still evaluates");

    assert_eq!(with_phantom, known);
}

/// The shadowed-evidence reference fixture: two facts, one variable, document order the tiebreak.
///
/// The same file behind `the_new_multi_output_and_shadowed_fixtures_reproduce_the_cpython_digests`
/// in `reference_parity.rs`, read here for its fact documents rather than for a digest.
fn shadowed_evidence_world() -> Value {
    let path: PathBuf = [
        env!("CARGO_MANIFEST_DIR"),
        "..",
        "..",
        "reference",
        "fiber_runtime",
        "examples",
        "shadowed_evidence_world.json",
    ]
    .iter()
    .collect();
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("missing fixture {}: {e}", path.display()));
    serde_json::from_str(&text).expect("fixture is valid JSON")
}

/// The fixture's colliding pair, moved onto a variable the oracle actually reads.
///
/// The fixture collides on `risk_score`, which is not one of the six variables `evaluate_facts`
/// keeps, so as shipped both providers are filtered out before the map is built and the tiebreak
/// is unobservable. Re-pointing the same two fact documents at `split_assignment` preserves
/// everything the fixture is demonstrating — one variable, two providers, nothing but their order
/// separating them — and puts the survivor somewhere a check can be asked about it.
///
/// The two values are chosen so the winner decides a verdict rather than only a map entry:
/// `fact.risk_score_provisional` keeps both aliased subjects in one split, and
/// `fact.risk_score_final` splits them apart, which is exactly the identity leakage the oracle
/// looks for.
fn shadowed_pair_over_split_assignment() -> Vec<Fact> {
    let fixture = shadowed_evidence_world();
    let facts = fixture["facts"]
        .as_array()
        .expect("the fixture lists facts");

    let mut collided: Vec<Value> = facts
        .iter()
        .filter(|fact| fact["provides"] == json!("risk_score"))
        .cloned()
        .collect();
    assert_eq!(
        collided.len(),
        2,
        "this test needs the fixture's two providers of one variable; if the fixture stopped \
         colliding it is no longer the shadowed-evidence world"
    );

    let splits = [
        json!({"subject:s1": "train", "subject:s2": "train"}),
        json!({"subject:s1": "train", "subject:s2": "test"}),
    ];
    for (fact, split) in collided.iter_mut().zip(splits) {
        fact["provides"] = json!("split_assignment");
        fact["value"] = split;
    }

    let aliases = json!({
        "id": "fact.aliases",
        "provides": "subject_aliases",
        "value": {"subject:s1": ["alias-a"], "subject:s2": ["alias-a"]},
        "scope": {"cohort": "SHADOW-001"},
        "tags": ["identity"],
        "provenance": ["registry/aliases.json"],
    });

    std::iter::once(&aliases)
        .chain(collided.iter())
        .map(|fact| Fact::from_json(fact).expect("the retargeted fact documents are well formed"))
        .collect()
}

/// Where two facts provide one variable, the last one the caller yields is the one that decides.
///
/// `evaluate_facts` builds its value map by `collect`, so a repeated key is overwritten and the
/// caller's iteration order — not any rule inside the oracle — picks the survivor. That sentence
/// is in the function's own docs and is what made the consolidation safe for the callers that had
/// been building the map themselves, so it is asserted rather than trusted: the same three facts
/// are evaluated forwards and backwards and must disagree.
#[test]
fn the_last_fact_providing_a_variable_wins_so_reversing_the_input_changes_the_verdict() {
    let facts = shadowed_pair_over_split_assignment();

    let forwards = oracle::evaluate_facts(facts.iter()).expect("facts evaluate");
    let backwards = oracle::evaluate_facts(facts.iter().rev()).expect("facts evaluate");

    assert_eq!(
        forwards.witnesses.len(),
        1,
        "the last provider splits the aliased subjects apart, so identity leakage is witnessed"
    );
    assert!(
        backwards.witnesses.is_empty(),
        "reversed, the last provider keeps them together and there is nothing to witness"
    );
    assert_ne!(forwards, backwards);
}
