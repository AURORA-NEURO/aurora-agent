//! Structural families, and the experiment they make possible.
//!
//! `docs/FINDINGS.md` records that the shipped reference world cannot separate FIBER from a tuned
//! graph walk or a lexical retriever: all three select the identical eleven facts. That is a fact
//! about the benchmark, not the method, and blueprint 43.39 exists to fix it.
//!
//! These tests hold the decisive skeleton fixed and vary only the structure around it. The
//! reference-like corner reproduces the tie; the discriminating corner separates the strategies
//! cleanly. Both are asserted, so neither result can quietly drift.

use bioprism_baseline::{
    compare, Comparison, ContextStrategy, FiberCompiled, KHopIncidence, LexicalTopK,
};
use bioprism_fiber::Query;
use bioprism_world::{validate, World};
use bioprism_worldgen::{generate, DistractorAttachment, LeakageMechanism, TagStyle, WorldSpec};

const MAX_DEPTH: usize = 16;

/// Every generated world assigns a split arm to every subject, so the oracle reaches a
/// full-context verdict on all of them and a test that does not say otherwise is entitled to a
/// table rather than an error.
fn compared(world: &World, query: &Query, panel: &[&dyn ContextStrategy]) -> Comparison {
    compare(world, query, panel).expect("a generated world reaches a full-context verdict")
}

fn build(spec: &WorldSpec) -> (World, Query) {
    let generated = generate(spec);
    let world = World::from_json(generated.world).expect("generated world must load");
    let query = Query::from_json(generated.query).expect("generated query must load");
    (world, query)
}

fn sound_and_compact_depths(world: &World, query: &Query) -> Vec<usize> {
    (1..=MAX_DEPTH)
        .filter(|depth| {
            let strategy = KHopIncidence { depth: *depth };
            let selection = strategy.select(world, query);
            let panel: Vec<&dyn ContextStrategy> = vec![&strategy];
            let result = &compared(world, query, &panel).results[0];
            result.verdict_preserving() == Some(true)
                && selection.facts.len() < world.facts.len() - 1
        })
        .collect()
}

#[test]
fn generation_is_deterministic_and_produces_valid_worlds() {
    let spec = WorldSpec::discriminating(50);
    let first = generate(&spec).world;
    let second = generate(&spec).world;
    assert_eq!(first, second, "the same spec must produce the same world");

    let world = World::from_json(first).expect("valid world");
    let report = validate(&world, &Default::default());
    assert!(
        !report.has_errors(),
        "generated world has structural errors: {:#?}",
        report.diagnostics
    );
}

/// The reference corner reproduces the tie that motivated this crate.
#[test]
fn the_reference_like_corner_does_not_discriminate() {
    let (world, query) = build(&WorldSpec::reference_like(750));
    let compiled = FiberCompiled.select(&world, &query).facts;
    assert_eq!(compiled.len(), 11);

    let usable = sound_and_compact_depths(&world, &query);
    assert_eq!(
        usable,
        vec![5, 6],
        "a tuned graph walk should still find a sound, compact window here"
    );
    for depth in &usable {
        assert_eq!(
            KHopIncidence { depth: *depth }.select(&world, &query).facts,
            compiled,
            "at depth {depth} the walk selects exactly the compiled set"
        );
    }

    let lexical = LexicalTopK { k: 11 }.select(&world, &query);
    assert_eq!(lexical.facts, compiled, "BM25 also ties on this corner");
}

/// Moving distractors near the target and pushing decisive facts behind relays removes the
/// separating depth entirely.
///
/// Depths 5-10 admit all 750 distractors while still missing decisive evidence — unsound *and*
/// uncompressed. Depth 11 first becomes sound, and by then it has taken the whole world.
#[test]
fn the_discriminating_corner_leaves_no_usable_graph_depth() {
    let (world, query) = build(&WorldSpec::discriminating(750));

    assert!(
        sound_and_compact_depths(&world, &query).is_empty(),
        "no depth may be both sound and compact on the discriminating world"
    );

    let mid = KHopIncidence { depth: 7 };
    let selection = mid.select(&world, &query);
    let panel: Vec<&dyn ContextStrategy> = vec![&mid];
    let result = &compared(&world, &query, &panel).results[0];
    assert_eq!(selection.facts.len(), 750);
    assert_eq!(
        result.verdict_preserving(),
        Some(false),
        "depth 7 takes every distractor and still misses decisive evidence"
    );

    let deep = KHopIncidence { depth: 11 };
    let deep_panel: Vec<&dyn ContextStrategy> = vec![&deep];
    assert_eq!(
        compared(&world, &query, &deep_panel).results[0].verdict_preserving(),
        Some(true)
    );
    assert_eq!(deep.select(&world, &query).facts.len(), world.facts.len() - 1);
}

/// Camouflaged tags defeat lexical retrieval, and it fails in the most dangerous way.
///
/// BM25 still reaches the correct verdict at k=11 — but only by luck: it drops a protected fact
/// that happens not to participate in any witness. Protected closure is *mandatory* under 43.13,
/// so a strategy at 91% closure has violated the contract even though its answer was right. No
/// budget fixes it: the dropped fact stays below rank 50.
#[test]
fn lexical_retrieval_silently_breaks_protected_closure() {
    let (world, query) = build(&WorldSpec::discriminating(750));

    for k in [11, 12, 15, 20, 50] {
        let strategy = LexicalTopK { k };
        let panel: Vec<&dyn ContextStrategy> = vec![&strategy];
        let result = &compared(&world, &query, &panel).results[0];
        assert!(
            result.protected_recall < 1.0,
            "BM25 at k={k} should not achieve full protected closure here, got {}",
            result.protected_recall
        );
    }

    let panel: Vec<&dyn ContextStrategy> = vec![&FiberCompiled];
    let fiber = &compared(&world, &query, &panel).results[0];
    assert_eq!(fiber.protected_recall, 1.0);
    assert_eq!(fiber.verdict_preserving(), Some(true));
    assert_eq!(fiber.facts_exposed, 11);
}

/// The summary claim, as one assertion.
#[test]
fn fiber_is_uniquely_sound_compact_and_closed_on_the_discriminating_world() {
    let (world, query) = build(&WorldSpec::discriminating(750));

    let depths: Vec<KHopIncidence> = (1..=MAX_DEPTH).map(|depth| KHopIncidence { depth }).collect();
    let lexicals: Vec<LexicalTopK> = [11, 12, 15, 20, 50]
        .into_iter()
        .map(|k| LexicalTopK { k })
        .collect();

    let mut panel: Vec<&dyn ContextStrategy> = Vec::new();
    for strategy in &depths {
        panel.push(strategy);
    }
    for strategy in &lexicals {
        panel.push(strategy);
    }
    panel.push(&FiberCompiled);

    let comparison = compared(&world, &query, &panel);
    let qualifying: Vec<&str> = comparison
        .results
        .iter()
        .filter(|r| {
            r.verdict_preserving() == Some(true)
                && r.protected_recall == 1.0
                && r.facts_exposed < world.facts.len() / 2
        })
        .map(|r| r.name.as_str())
        .collect();

    assert_eq!(
        qualifying,
        vec!["fiber"],
        "exactly one strategy should be sound, closed and compact"
    );
}

/// A clean world must come back valid, or the oracle is just always saying invalid.
#[test]
fn a_world_without_injected_defects_compiles_to_a_valid_verdict() {
    let spec = WorldSpec::discriminating(20)
        .with_world_id("generated-clean-v1")
        .with_leakage(vec![]);
    let (world, query) = build(&spec);

    let out = bioprism_fiber::compile(&world, &query).expect("compiles");
    assert_eq!(
        out.certificate.oracle.status,
        bioprism_section::OracleStatus::Valid
    );
    assert!(out.certificate.oracle.witnesses.is_empty());
    assert!(out.protected_closure_satisfied());
}

/// Each mechanism can be injected on its own, which is what makes the oracle diagnostic rather
/// than a single pass/fail bit.
#[test]
fn each_leakage_mechanism_can_be_injected_independently() {
    for (mechanism, expected) in [
        (LeakageMechanism::Identity, "identity_leakage"),
        (LeakageMechanism::Site, "site_leakage"),
        (LeakageMechanism::Temporal, "temporal_leakage"),
        (LeakageMechanism::Preprocessing, "preprocessing_leakage"),
    ] {
        let spec = WorldSpec::discriminating(10)
            .with_world_id(format!("generated-{expected}-only"))
            .with_leakage(vec![mechanism]);
        let (world, query) = build(&spec);
        let out = bioprism_fiber::compile(&world, &query).expect("compiles");
        assert_eq!(
            out.certificate.oracle.witness_kinds(),
            vec![expected],
            "{mechanism:?} should produce exactly one witness kind"
        );
    }
}

#[test]
fn the_structural_knobs_are_independent() {
    let hub_distinct = WorldSpec {
        attachment: DistractorAttachment::Hub,
        tag_style: TagStyle::Distinct,
        ..WorldSpec::discriminating(100)
    };
    let hub_camouflaged = WorldSpec {
        attachment: DistractorAttachment::Hub,
        tag_style: TagStyle::Camouflaged,
        ..WorldSpec::discriminating(100)
    };

    let (world_a, query_a) = build(&hub_distinct);
    let (world_b, query_b) = build(&hub_camouflaged);

    assert_eq!(
        sound_and_compact_depths(&world_a, &query_a),
        sound_and_compact_depths(&world_b, &query_b),
        "tag style must not change graph reachability"
    );

    let recall = |world: &World, query: &Query| {
        let strategy = LexicalTopK { k: 11 };
        let panel: Vec<&dyn ContextStrategy> = vec![&strategy];
        compared(world, query, &panel).results[0].protected_recall
    };
    let distinct = recall(&world_a, &query_a);
    let camouflaged = recall(&world_b, &query_b);
    assert!(
        distinct > camouflaged,
        "camouflage must degrade lexical protected recall ({distinct} vs {camouflaged})"
    );
}
