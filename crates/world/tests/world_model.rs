use bioprism_scope::{DimensionRegistry, Timestamp};
use bioprism_world::{validate, Severity, World, WorldError, WorldSource};
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::path::PathBuf;

fn golden_world_json() -> Value {
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
    serde_json::from_str(&text).expect("fixture is valid JSON")
}

fn tagged_fact(id: &str, provides: &str, tags: &[&str]) -> Value {
    json!({
        "id": id,
        "provides": provides,
        "value": 1,
        "scope": {},
        "tags": tags
    })
}

fn minimal_world(facts: Value, factors: Value, events: Value) -> Value {
    json!({
        "schema_version": "fiber-world/0.1",
        "world_id": "unit-test-world",
        "facts": facts,
        "factors": factors,
        "events": events
    })
}

#[test]
fn loads_the_golden_radiogenomic_world() {
    let world = World::from_json(golden_world_json()).expect("golden world must load");
    assert_eq!(world.world_id.as_str(), "radiogenomic-integrity-demo-v1");
    assert_eq!(world.facts.len(), 761);
    assert_eq!(world.factors.len(), 756);
    assert_eq!(world.events.len(), 2);
}

#[test]
fn retains_raw_document_so_hashes_match_the_reference() {
    let raw = golden_world_json();
    let world = World::from_json(raw.clone()).unwrap();
    assert_eq!(world.raw(), &raw);
    assert_eq!(
        world.content_hash().as_str(),
        "b3809731cf93040fcd8aef43deb2a552492064b49154e07ea58caa724c10cbb5"
    );
}

/// The digest is cached after the first call. A cache that answered a later call differently
/// would silently fork every certificate compiled against the same world.
#[test]
fn the_cached_world_digest_is_the_digest_a_fresh_world_computes() {
    let raw = golden_world_json();
    let world = World::from_json(raw.clone()).unwrap();

    let first = world.content_hash();
    let second = world.content_hash();
    assert_eq!(first, second);

    let fresh = World::from_json(raw.clone()).unwrap();
    assert_eq!(fresh.content_hash(), first);

    let cloned = world.clone();
    assert_eq!(cloned.content_hash(), first);

    let never_asked = World::from_json(raw).unwrap().clone();
    assert_eq!(never_asked.content_hash(), first);
}

/// `WorldSource::world_digest` documents the digest as precomputed rather than re-derived. Two
/// calls that walk the corpus twice would be a contract the eager world states and breaks.
#[test]
fn asking_the_eager_world_for_its_digest_twice_does_not_recanonicalise_it_twice() {
    let world = World::from_json(golden_world_json()).unwrap();

    let started = std::time::Instant::now();
    let first = WorldSource::world_digest(&world);
    let cold = started.elapsed();

    let repeats = 200;
    let started = std::time::Instant::now();
    for _ in 0..repeats {
        assert_eq!(WorldSource::world_digest(&world), first);
    }
    let warm = started.elapsed();

    assert!(
        warm < cold,
        "{repeats} repeat digests took {warm:?} against a single first digest of {cold:?}, so \
         the document is being re-canonicalised on every call"
    );
}

/// The tag index answers exactly what a full corpus scan answers, on a world where facts carry
/// several tags each and a queried tag matches nothing.
#[test]
fn the_tag_index_answers_what_a_full_corpus_scan_answers() {
    let scan_count = |world: &World, tag: &str| -> usize {
        world.facts.iter().filter(|fact| fact.has_tag(tag)).count()
    };
    let scan_ids = |world: &World, tags: &BTreeSet<String>| -> BTreeSet<String> {
        world
            .facts
            .iter()
            .filter(|fact| fact.has_any_tag(tags))
            .map(|fact| fact.id.as_str().to_string())
            .collect()
    };

    let golden = World::from_json(golden_world_json()).unwrap();
    let overlapping = World::from_json(minimal_world(
        json!([
            tagged_fact("fact.a", "alpha", &["protected", "identity", "split"]),
            tagged_fact("fact.b", "beta", &["identity"]),
            tagged_fact("fact.c", "gamma", &[]),
            tagged_fact("fact.d", "delta", &["split", "protected"])
        ]),
        json!([]),
        json!([]),
    ))
    .expect("world loads");

    for world in [&golden, &overlapping] {
        let every_tag: BTreeSet<String> = world
            .facts
            .iter()
            .flat_map(|fact| fact.tags.iter().cloned())
            .collect();

        for tag in every_tag
            .iter()
            .chain(["absent-from-this-world".to_string()].iter())
        {
            assert_eq!(
                WorldSource::count_with_tag(world, tag),
                scan_count(world, tag),
                "count for tag {tag}"
            );
        }

        let mut queries: Vec<BTreeSet<String>> = vec![
            BTreeSet::new(),
            ["absent-from-this-world".to_string()].into_iter().collect(),
            every_tag.clone(),
        ];
        for tag in &every_tag {
            queries.push([tag.clone()].into_iter().collect());
            queries.push(
                [tag.clone(), "absent-from-this-world".to_string()]
                    .into_iter()
                    .collect(),
            );
        }
        if every_tag.len() >= 2 {
            queries.push(every_tag.iter().take(2).cloned().collect());
        }

        for tags in &queries {
            assert_eq!(
                WorldSource::fact_ids_with_any_tag(world, tags),
                scan_ids(world, tags),
                "ids for tags {tags:?}"
            );
        }
    }
}

#[test]
fn indexes_facts_factors_and_producers() {
    let world = World::from_json(golden_world_json()).unwrap();

    let aliases = world
        .fact("fact.subject_aliases")
        .expect("protected fact present");
    assert_eq!(aliases.provides.as_str(), "subject_aliases");
    assert!(aliases.has_tag("protected"));
    assert!(aliases.has_tag("identity"));

    assert!(world.fact_providing("split_assignment").is_some());

    let producers: Vec<&str> = world
        .producers_of("identity_leakage")
        .map(|f| f.id.as_str())
        .collect();
    assert_eq!(producers, vec!["factor.identity_check"]);

    let checker = world.factor("factor.identity_check").unwrap();
    assert!(checker.is_deterministic_rule());
    assert_eq!(checker.arity(), 4);
}

#[test]
fn event_availability_gates_reading_not_event_time() {
    let world = World::from_json(golden_world_json()).unwrap();
    let cut = Timestamp::parse("2025-01-01T00:00:00Z").unwrap();

    let future = world
        .events
        .iter()
        .find(|e| e.id.as_str() == "event.future_label")
        .expect("fixture has a future-label event");

    assert!(future.event_time < future.availability_time);
    assert!(
        !future.is_available_at(cut),
        "a result recorded in June is not readable by a January decision"
    );

    let training = world
        .events
        .iter()
        .find(|e| e.id.as_str() == "event.training")
        .unwrap();
    assert!(training.is_available_at(cut));

    let managed = world.event_managed_variables();
    assert!(managed.contains("future_label_value"));
    assert!(managed.contains("split_assignment"));
    assert!(!managed.contains("subject_aliases"));
}

#[test]
fn rejects_the_wrong_schema_version() {
    let mut raw = golden_world_json();
    raw["schema_version"] = json!("fiber-world/0.2");
    assert!(matches!(
        World::from_json(raw),
        Err(WorldError::UnsupportedSchema { .. })
    ));
}

#[test]
fn rejects_duplicate_fact_and_factor_ids() {
    let facts = json!([
        { "id": "fact.a", "provides": "a", "value": 1, "scope": {} },
        { "id": "fact.a", "provides": "b", "value": 2, "scope": {} }
    ]);
    let world = minimal_world(facts, json!([]), json!([]));
    assert!(matches!(
        World::from_json(world),
        Err(WorldError::DuplicateFactId(id)) if id == "fact.a"
    ));

    let factors = json!([
        { "id": "factor.a", "inputs": [], "outputs": ["x"], "kind": "rule" },
        { "id": "factor.a", "inputs": [], "outputs": ["y"], "kind": "rule" }
    ]);
    let world = minimal_world(json!([]), factors, json!([]));
    assert!(matches!(
        World::from_json(world),
        Err(WorldError::DuplicateFactorId(id)) if id == "factor.a"
    ));
}

#[test]
fn rejects_factors_whose_inputs_nothing_provides() {
    let facts = json!([{ "id": "fact.a", "provides": "a", "value": 1, "scope": {} }]);
    let factors = json!([
        { "id": "factor.f", "inputs": ["a", "ghost"], "outputs": ["z"], "kind": "rule" }
    ]);
    match World::from_json(minimal_world(facts, factors, json!([]))) {
        Err(WorldError::UnknownFactorInputs { factor, missing }) => {
            assert_eq!(factor, "factor.f");
            assert_eq!(missing, vec!["ghost".to_string()]);
        }
        other => panic!("expected UnknownFactorInputs, got {other:?}"),
    }
}

#[test]
fn a_factor_input_may_be_produced_by_another_factor() {
    let facts = json!([{ "id": "fact.a", "provides": "a", "value": 1, "scope": {} }]);
    let factors = json!([
        { "id": "factor.one", "inputs": ["a"], "outputs": ["mid"], "kind": "rule" },
        { "id": "factor.two", "inputs": ["mid"], "outputs": ["out"], "kind": "rule" }
    ]);
    assert!(World::from_json(minimal_world(facts, factors, json!([]))).is_ok());
}

#[test]
fn malformed_members_name_the_offending_subject() {
    let facts = json!([{ "id": "fact.a", "provides": "a", "scope": {} }]);
    match World::from_json(minimal_world(facts, json!([]), json!([]))) {
        Err(WorldError::MissingField { field, subject }) => {
            assert_eq!(field, "value");
            assert_eq!(subject, "fact fact.a");
        }
        other => panic!("expected a located MissingField, got {other:?}"),
    }

    let events = json!([{ "id": "event.e", "event_time": "nope", "availability_time": "2025-01-01T00:00:00Z" }]);
    match World::from_json(minimal_world(json!([]), json!([]), events)) {
        Err(WorldError::Timestamp { subject, .. }) => {
            assert_eq!(subject, "event event.e.event_time")
        }
        other => panic!("expected a located Timestamp error, got {other:?}"),
    }
}

#[test]
fn golden_world_has_no_structural_errors() {
    let world = World::from_json(golden_world_json()).unwrap();
    let report = validate(&world, &DimensionRegistry::default());
    let errors: Vec<_> = report
        .diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .collect();
    assert!(
        errors.is_empty(),
        "golden world reported errors: {errors:#?}"
    );
}

#[test]
fn shadowed_variables_are_reported_as_errors() {
    let facts = json!([
        { "id": "fact.a", "provides": "same", "value": 1, "scope": {} },
        { "id": "fact.b", "provides": "same", "value": 2, "scope": {} }
    ]);
    let world = World::from_json(minimal_world(facts, json!([]), json!([]))).unwrap();
    let report = validate(&world, &DimensionRegistry::default());
    assert_eq!(report.with_code("shadowed_variable").count(), 1);
    assert!(report.has_errors());
}

#[test]
fn dangling_causal_parents_and_backdated_events_are_reported() {
    let events = json!([
        {
            "id": "event.child",
            "event_time": "2025-06-01T00:00:00Z",
            "availability_time": "2025-01-01T00:00:00Z",
            "causal_parents": ["event.missing"],
            "produces": []
        }
    ]);
    let world = World::from_json(minimal_world(json!([]), json!([]), events)).unwrap();
    let report = validate(&world, &DimensionRegistry::default());
    assert_eq!(report.with_code("dangling_causal_parent").count(), 1);
    assert_eq!(report.with_code("backdated_event").count(), 1);
}

#[test]
fn unclassified_scope_dimensions_are_reported() {
    let facts = json!([
        { "id": "fact.a", "provides": "a", "value": 1, "scope": { "wobble": "w" } }
    ]);
    let world = World::from_json(minimal_world(facts, json!([]), json!([]))).unwrap();
    let report = validate(&world, &DimensionRegistry::default());
    assert_eq!(report.with_code("unclassified_scope_dimension").count(), 1);
}

#[test]
fn factor_cycles_are_reported_without_hanging() {
    let facts = json!([{ "id": "fact.seed", "provides": "seed", "value": 1, "scope": {} }]);
    let factors = json!([
        { "id": "factor.a", "inputs": ["seed", "y"], "outputs": ["x"], "kind": "rule" },
        { "id": "factor.b", "inputs": ["x"], "outputs": ["y"], "kind": "rule" }
    ]);
    let world = World::from_json(minimal_world(facts, factors, json!([]))).unwrap();
    let report = validate(&world, &DimensionRegistry::default());
    assert!(report.with_code("factor_cycle").count() >= 1);
}

/// The loser of a shadowing race stays reachable, because the compiler has to classify it.
///
/// `fact_providing` keeps the last provider, matching the reference runtime's dict semantics, and
/// before the index kept the displaced ones there was no way to ask which facts had been dropped
/// that way. That gap is what let a shadowed omission be published as provably irrelevant: the
/// compiler could see that a variable was needed and that one fact supplied it, and could not see
/// that another fact supplied it differently.
#[test]
fn a_shadowed_fact_stays_reachable_through_the_index_rather_than_being_lost() {
    let facts = json!([
        { "id": "fact.first", "provides": "risk_score", "value": "low", "scope": {} },
        { "id": "fact.second", "provides": "risk_score", "value": "high", "scope": {} },
        { "id": "fact.only", "provides": "cohort_id", "value": "C-1", "scope": {} }
    ]);
    let world = World::from_json(minimal_world(facts, json!([]), json!([]))).unwrap();

    assert_eq!(
        world
            .fact_providing("risk_score")
            .map(|f| f.id.as_str().to_string()),
        Some("fact.second".to_string()),
        "the later fact still wins, because the reference runtime's semantics are the contract"
    );
    assert_eq!(
        world.shadowed_provider_ids("risk_score"),
        vec!["fact.first".to_string()],
        "and the displaced one is now nameable"
    );
    assert!(
        world.shadowed_provider_ids("cohort_id").is_empty(),
        "an unshadowed variable has no losers"
    );
    assert!(world.shadowed_provider_ids("no_such_variable").is_empty());

    let report = validate(&world, &DimensionRegistry::default());
    assert_eq!(
        report.with_code("shadowed_variable").count(),
        1,
        "the advisory diagnostic still fires; the index addition did not replace it"
    );
}
