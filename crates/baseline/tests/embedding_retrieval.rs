//! The embedding retriever, measured wherever its results land.
//!
//! `docs/FINDINGS.md` recorded this baseline as missing and BM25 as its stand-in. Now that it
//! exists, the measured result is *worse than the stand-in on both shipped worlds*: at k=11 the
//! hashed-trigram basis drops a protected fact on the reference world where BM25 tied FIBER
//! exactly, and under camouflage it falls to a 36% protected closure where BM25 held 91%. Both
//! directions are pinned here, because a baseline that only appears in prose is a baseline nobody
//! measured — and because these numbers are for a *fixed-basis* embedding, not a neural one, they
//! bound what character-level similarity does, not what a trained encoder would do.

use bioprism_baseline::{compare, ContextStrategy, EmbeddingTopK, FiberCompiled, LexicalTopK};
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

/// The embedding is a pure function of the world and query: a fixed hash basis, no training, no
/// randomness. Two runs must agree fact for fact, or the sweep built on it is not reproducible.
#[test]
fn the_same_world_and_query_embed_to_the_same_selection_twice() {
    let (world, query) = reference();
    let first = EmbeddingTopK { k: 11 }.select(&world, &query);
    let second = EmbeddingTopK { k: 11 }.select(&world, &query);
    assert_eq!(first.facts, second.facts);

    let (world, query) = discriminating();
    let first = EmbeddingTopK { k: 50 }.select(&world, &query);
    let second = EmbeddingTopK { k: 50 }.select(&world, &query);
    assert_eq!(first.facts, second.facts);
}

/// On the reference world, where BM25 ties FIBER exactly, the embedding retriever does not.
///
/// At k=11 it selects eleven facts but not FIBER's eleven: it drops `fact.label_source` — a
/// protected fact and the carrier of the temporal-leakage witness — in favour of a distractor,
/// so it is unsound at a 91% closure. The trigram basis dilutes the tag signal BM25's IDF
/// concentrates: `label_lineage` shares no trigram with any query token, while a distractor's
/// serialised value is full of them.
#[test]
fn on_the_reference_world_the_embedding_retriever_is_unsound_at_the_budget_bm25_ties_fiber_at() {
    let (world, query) = reference();
    let strategy = EmbeddingTopK { k: 11 };
    let selection = strategy.select(&world, &query);
    let compiled = FiberCompiled.select(&world, &query).facts;

    assert_eq!(selection.facts.len(), 11);
    assert_ne!(selection.facts, compiled, "the tie BM25 achieves does not transfer");
    assert!(!selection.facts.contains("fact.label_source"));

    let panel: Vec<&dyn ContextStrategy> = vec![&strategy];
    let row = &compare(&world, &query, &panel).expect("reference verdict exists").results[0];
    assert_eq!(row.verdict_preserving(), Some(false));
    assert!(row.protected_recall < 1.0);
    assert!(!row.admissible());
}

/// Width buys back on the reference world what precision lost: at k=50 the embedding retriever is
/// sound and closed, at four and a half times FIBER's context.
#[test]
fn a_wider_embedding_budget_recovers_admissibility_on_the_reference_world_at_50_facts() {
    let (world, query) = reference();
    let strategy = EmbeddingTopK { k: 50 };
    let panel: Vec<&dyn ContextStrategy> = vec![&strategy];
    let row = &compare(&world, &query, &panel).expect("reference verdict exists").results[0];

    assert_eq!(row.facts_exposed, 50);
    assert_eq!(row.verdict_preserving(), Some(true));
    assert_eq!(row.protected_recall, 1.0);
    assert!(row.admissible());
}

/// Camouflage hits the embedding retriever *harder* than BM25, and no measured budget fixes it.
///
/// Camouflaged tags are built from the protected vocabulary plus a suffix, so they share most of
/// their trigrams with the query — exactly the similarity a character basis rewards. At k=11 the
/// closure falls to 4 of 11 protected facts (BM25 holds 10 of 11), and at k=50 it is still
/// incomplete and unsound.
#[test]
fn camouflage_degrades_the_embedding_retriever_below_bm25_on_the_discriminating_world() {
    let (world, query) = discriminating();

    let embedding = EmbeddingTopK { k: 11 };
    let lexical = LexicalTopK { k: 11 };
    let panel: Vec<&dyn ContextStrategy> = vec![&embedding, &lexical];
    let comparison = compare(&world, &query, &panel).expect("generated verdict exists");

    let embedded = &comparison.results[0];
    let bm25 = &comparison.results[1];
    assert_eq!(embedded.verdict_preserving(), Some(false));
    assert!(
        embedded.protected_recall < bm25.protected_recall,
        "trigram similarity rewards camouflage more than token identity does ({} vs {})",
        embedded.protected_recall,
        bm25.protected_recall
    );
    assert!(embedded.protected_recall < 0.5);

    let wide = EmbeddingTopK { k: 50 };
    let wide_panel: Vec<&dyn ContextStrategy> = vec![&wide];
    let row = &compare(&world, &query, &wide_panel).expect("generated verdict exists").results[0];
    assert_eq!(row.verdict_preserving(), Some(false));
    assert!(row.protected_recall < 1.0);
    assert!(!row.admissible());
}
