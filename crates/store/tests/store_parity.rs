//! The indexed store must be logically indistinguishable from the eager world.
//!
//! Blueprint 43.16: "logical semantics are independent of physical backend." That is only a real
//! guarantee if it is checked byte for byte, because a certificate compiled against one backend is
//! meant to replay against the other.

use bioprism_fiber::{compile, Query};
use bioprism_ids::to_canonical_string;
use bioprism_section::CertificateProfile;
use bioprism_store::{build, LazyWorld, SortedIndex, SortedIndexWriter, StoreError};
use bioprism_world::{World, WorldSource};
use bioprism_worldgen::{generate, WorldSpec};
use serde_json::Value;
use std::collections::BTreeSet;
use std::path::PathBuf;

fn scratch(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("bioprism-store-{name}"));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).expect("scratch dir");
    path
}

fn reference_example(name: &str) -> Value {
    let path: PathBuf = [
        env!("CARGO_MANIFEST_DIR"),
        "..",
        "..",
        "reference",
        "fiber_runtime",
        "examples",
        name,
    ]
    .iter()
    .collect();
    serde_json::from_str(&std::fs::read_to_string(&path).expect("reference example readable"))
        .expect("valid JSON")
}

fn reference_fixture(name: &str) -> Value {
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
        .expect("valid JSON")
}

#[test]
fn the_lazy_path_reproduces_the_reference_certificate_byte_for_byte() {
    let world_json = reference_fixture("radiogenomic_world.json");
    let query = Query::from_json(reference_fixture("leakage_query.json")).unwrap();

    let directory = scratch("reference");
    build(&world_json, &directory).expect("store builds");
    let lazy = LazyWorld::open(&directory).expect("store opens");
    let eager = World::from_json(world_json).expect("world loads");

    let from_eager = compile(&eager, &query).expect("eager compiles");
    let from_lazy = compile(&lazy, &query).expect("lazy compiles");

    let eager_certificate = to_canonical_string(
        &from_eager
            .certificate
            .to_json(CertificateProfile::Reference)
            .unwrap(),
    )
    .unwrap();
    let lazy_certificate = to_canonical_string(
        &from_lazy
            .certificate
            .to_json(CertificateProfile::Reference)
            .unwrap(),
    )
    .unwrap();

    assert_eq!(eager_certificate, lazy_certificate, "backends disagree");
    assert_eq!(
        from_lazy
            .certificate
            .digest(CertificateProfile::Reference)
            .unwrap()
            .as_str(),
        "c0da17ffc80465258345c8a538171bfd868100cd883e9a20780a0dc5477e7ea4",
        "the lazy path must reproduce the published CPython digest"
    );
    assert_eq!(
        from_eager.section.to_canonical_string().unwrap(),
        from_lazy.section.to_canonical_string().unwrap()
    );
}

#[test]
fn the_two_backends_agree_on_generated_worlds_too() {
    for (label, spec) in [
        ("reference-like", WorldSpec::reference_like(500)),
        ("discriminating", WorldSpec::discriminating(500)),
    ] {
        let generated = generate(&spec);
        let query = Query::from_json(generated.query).unwrap();

        let directory = scratch(label);
        build(&generated.world, &directory).expect("store builds");
        let lazy = LazyWorld::open(&directory).expect("store opens");
        let eager = World::from_json(generated.world).expect("world loads");

        let from_eager = compile(&eager, &query).expect("eager compiles");
        let from_lazy = compile(&lazy, &query).expect("lazy compiles");

        assert_eq!(
            from_eager
                .certificate
                .digest(CertificateProfile::Reference)
                .unwrap(),
            from_lazy
                .certificate
                .digest(CertificateProfile::Reference)
                .unwrap(),
            "{label}: backends disagree"
        );
    }
}

#[test]
fn manifest_aggregates_match_the_eager_world() {
    let generated = generate(&WorldSpec::discriminating(300));
    let directory = scratch("aggregates");
    let manifest = build(&generated.world, &directory).expect("store builds");
    let eager = World::from_json(generated.world).expect("world loads");
    let lazy = LazyWorld::open(&directory).expect("store opens");

    assert_eq!(manifest.total_facts, eager.facts.len());
    assert_eq!(manifest.total_factors, eager.factors.len());
    assert_eq!(lazy.world_id(), WorldSource::world_id(&eager));
    assert_eq!(lazy.world_digest(), eager.content_hash());
    assert_eq!(
        lazy.count_with_tag("exploratory"),
        WorldSource::count_with_tag(&eager, "exploratory")
    );
    assert_eq!(lazy.events().len(), eager.events.len());
}

#[test]
fn point_lookups_return_the_same_records() {
    let generated = generate(&WorldSpec::reference_like(200));
    let directory = scratch("lookups");
    build(&generated.world, &directory).expect("store builds");
    let lazy = LazyWorld::open(&directory).expect("store opens");
    let eager = World::from_json(generated.world).expect("world loads");

    for id in ["fact.subject_aliases", "fact.split", "fact.policy"] {
        let from_lazy = lazy.fact(id).expect("present in store");
        let from_eager = WorldSource::fact(&eager, id).expect("present in world");
        assert_eq!(from_lazy.raw(), from_eager.raw(), "record {id} differs");
    }

    assert_eq!(
        lazy.fact_providing("split_assignment")
            .map(|f| f.id.as_str().to_string()),
        Some("fact.split".to_string())
    );
    assert_eq!(
        lazy.producer_ids("identity_leakage"),
        WorldSource::producer_ids(&eager, "identity_leakage")
    );
    assert!(lazy.fact("fact.does-not-exist").is_none());
    assert!(lazy.producer_ids("no-such-variable").is_empty());

    let protected: BTreeSet<String> = ["protected".to_string()].into_iter().collect();
    assert_eq!(
        lazy.fact_ids_with_any_tag(&protected),
        WorldSource::fact_ids_with_any_tag(&eager, &protected)
    );
}

#[test]
fn sorted_index_binary_search_finds_every_key_and_no_others() {
    let directory = scratch("index");
    let mut writer = SortedIndexWriter::new();
    for n in 0..500 {
        writer.insert(format!("key-{n:04}"), format!("{{\"n\":{n}}}"));
    }
    writer.finish(&directory, "probe").expect("index writes");

    let index = SortedIndex::open(&directory, "probe").expect("index opens");
    assert_eq!(index.len(), 500);
    for n in 0..500 {
        assert_eq!(
            index.get(&format!("key-{n:04}")).unwrap(),
            Some(format!("{{\"n\":{n}}}"))
        );
    }
    assert_eq!(index.get("key-9999").unwrap(), None);
    assert_eq!(index.get("").unwrap(), None);
    assert_eq!(index.get("zzz").unwrap(), None);
}

#[test]
fn later_duplicates_win_matching_reference_dict_semantics() {
    let directory = scratch("duplicates");
    let mut writer = SortedIndexWriter::new();
    writer.insert("shared", "\"first\"");
    writer.insert("shared", "\"second\"");
    writer.finish(&directory, "dupes").expect("index writes");

    let index = SortedIndex::open(&directory, "dupes").expect("index opens");
    assert_eq!(index.len(), 1);
    assert_eq!(index.get("shared").unwrap(), Some("\"second\"".to_string()));
}

#[test]
fn keys_containing_separators_are_rejected_rather_than_silently_corrupting() {
    let directory = scratch("badkeys");
    let mut writer = SortedIndexWriter::new();
    writer.insert("has\ttab", "\"x\"");
    assert!(matches!(
        writer.finish(&directory, "bad"),
        Err(StoreError::UnsupportedKey(_))
    ));
}

#[test]
fn an_empty_index_is_openable_and_answers_nothing() {
    let directory = scratch("empty");
    SortedIndexWriter::new()
        .finish(&directory, "nothing")
        .expect("empty index writes");
    let index = SortedIndex::open(&directory, "nothing").expect("empty index opens");
    assert!(index.is_empty());
    assert_eq!(index.get("anything").unwrap(), None);
}

/// The third implementation must see shadowing too, or it would classify omissions differently.
///
/// `bioprism-fiber` decides whether an omission is provably irrelevant or merely unexamined by
/// asking the world source which facts a later fact displaced. An indexed store that could not
/// answer would compile the same world to a *more confident* certificate than the eager path —
/// a shadowed fact folded into the zero group with a bound of `0.0` — which is the worst possible
/// direction for the two backends to disagree in. The `variables` index keeps only the winner, so
/// the store carries a separate `shadowed` index and `bioprism-store/0.2` refuses a directory
/// written before it existed.
#[test]
fn the_lazy_path_reports_the_same_shadowed_providers_as_the_eager_world() {
    let world_json = reference_example("shadowed_evidence_world.json");
    let query = Query::from_json(reference_example("shadowed_evidence_query.json")).unwrap();

    let directory = scratch("shadowed");
    build(&world_json, &directory).expect("store builds");
    let lazy = LazyWorld::open(&directory).expect("store opens");
    let eager = World::from_json(world_json).expect("world loads");

    assert_eq!(
        eager.shadowed_provider_ids("risk_score"),
        vec!["fact.risk_score_provisional".to_string()]
    );
    assert_eq!(
        lazy.shadowed_provider_ids("risk_score"),
        eager.shadowed_provider_ids("risk_score")
    );
    assert!(lazy.shadowed_provider_ids("cohort_id").is_empty());

    let from_eager = compile(&eager, &query).expect("eager compiles");
    let from_lazy = compile(&lazy, &query).expect("lazy compiles");

    assert_eq!(
        to_canonical_string(
            &from_lazy
                .certificate
                .to_json(CertificateProfile::Extended)
                .unwrap()
        )
        .unwrap(),
        to_canonical_string(
            &from_eager
                .certificate
                .to_json(CertificateProfile::Extended)
                .unwrap()
        )
        .unwrap(),
        "the two backends must agree about which omission was proved and which was not"
    );
    assert!(!from_lazy.certificate.manifest.supports_sufficiency_claim());
}

/// A store written before the `shadowed` index existed is refused rather than read as unshadowed.
#[test]
fn a_store_from_an_earlier_schema_is_rejected_instead_of_reporting_nothing_shadowed() {
    let directory = scratch("stale-schema");
    build(
        &reference_example("shadowed_evidence_world.json"),
        &directory,
    )
    .expect("store builds");

    let manifest_path = directory.join("manifest.json");
    let mut manifest: Value =
        serde_json::from_str(&std::fs::read_to_string(&manifest_path).unwrap()).unwrap();
    manifest["schema_version"] = Value::String("bioprism-store/0.1".into());
    std::fs::write(
        &manifest_path,
        serde_json::to_string_pretty(&manifest).unwrap(),
    )
    .unwrap();

    assert!(matches!(
        LazyWorld::open(&directory),
        Err(StoreError::UnsupportedSchema { .. })
    ));
}
