//! A hit must be provably the same computation, and a miss must say which kind of miss it is.

use bioprism_infra::{
    Cache, CacheEntry, CacheError, CodeIdentity, ComputationKey, DependencyDeclaration,
    EntryStatus, Epoch, KeySchema, Lookup, MissReason, ReuseRule,
};
use serde_json::json;

fn schema(reuse: ReuseRule) -> KeySchema {
    KeySchema::declare(
        "compile-decision-section",
        ["inputs", "code", "environment", "oracle", "policy"],
        reuse,
    )
    .expect("five named components is a well-formed schema")
}

fn key(schema: &KeySchema, oracle: &str) -> ComputationKey {
    ComputationKey::build(
        schema,
        [
            ("inputs", "world@3"),
            ("code", "fiber@1.2"),
            ("environment", "linux-x86_64"),
            ("oracle", oracle),
            ("policy", "strict"),
        ],
    )
    .expect("every declared component is supplied")
}

fn build(name: &str) -> CodeIdentity {
    CodeIdentity::parse(name).expect("build identity is non-empty")
}

#[test]
fn a_key_missing_a_declared_component_is_refused_rather_than_hashed_without_it() {
    let schema = schema(ReuseRule::SameBuildOnly);
    let error = ComputationKey::build(
        &schema,
        [
            ("inputs", "world@3"),
            ("code", "fiber@1.2"),
            ("environment", "linux-x86_64"),
            ("oracle", "gpt"),
        ],
    )
    .expect_err("policy was not supplied");
    assert_eq!(
        error,
        CacheError::IncompleteKey {
            schema: "compile-decision-section".to_string(),
            component: "policy".to_string(),
        }
    );
}

#[test]
fn an_empty_component_value_is_refused_because_it_is_indistinguishable_from_absence_after_hashing()
{
    let schema = schema(ReuseRule::SameBuildOnly);
    let error = ComputationKey::build(
        &schema,
        [
            ("inputs", "world@3"),
            ("code", "fiber@1.2"),
            ("environment", ""),
            ("oracle", "gpt"),
            ("policy", "strict"),
        ],
    )
    .expect_err("an empty environment is not an environment");
    assert_eq!(
        error,
        CacheError::EmptyComponent {
            schema: "compile-decision-section".to_string(),
            component: "environment".to_string(),
        }
    );
}

#[test]
fn a_component_the_schema_never_declared_is_refused_rather_than_silently_ignored() {
    let schema = schema(ReuseRule::SameBuildOnly);
    let error = ComputationKey::build(
        &schema,
        [
            ("inputs", "world@3"),
            ("code", "fiber@1.2"),
            ("environment", "linux"),
            ("oracle", "gpt"),
            ("policy", "strict"),
            ("tenant", "acme"),
        ],
    )
    .expect_err("tenant is not part of this schema");
    assert_eq!(
        error,
        CacheError::UndeclaredComponent {
            schema: "compile-decision-section".to_string(),
            component: "tenant".to_string(),
        }
    );
}

#[test]
fn a_schema_with_no_components_is_refused_because_it_maps_every_computation_to_one_address() {
    let error = KeySchema::declare("empty", Vec::<String>::new(), ReuseRule::SameBuildOnly)
        .expect_err("a schema must declare what determines the result");
    assert_eq!(
        error,
        CacheError::SchemaWithoutComponents {
            schema: "empty".to_string()
        }
    );
}

#[test]
fn there_is_no_way_to_build_a_key_from_a_digest_alone() {
    let schema = schema(ReuseRule::SameBuildOnly);
    let digest = key(&schema, "gpt").digest();
    let attempt: Result<ComputationKey, _> = serde_json::from_value(json!(digest));
    assert!(
        attempt.is_err(),
        "a bare digest must not deserialize into a key"
    );
}

#[test]
fn deserialization_runs_the_same_validation_as_construction() {
    let attempt: Result<ComputationKey, _> = serde_json::from_value(json!({
        "schema_name": "compile-decision-section",
        "schema_digest": "abc",
        "components": { "inputs": "world@3", "environment": "" },
    }));
    assert!(
        attempt.is_err(),
        "an empty component must not survive a round trip through serde"
    );
}

#[test]
fn a_key_that_round_trips_through_json_still_hashes_to_the_same_address() {
    let schema = schema(ReuseRule::SameBuildOnly);
    let original = key(&schema, "gpt");
    let text = serde_json::to_string(&original).expect("a key serializes");
    let restored: ComputationKey = serde_json::from_str(&text).expect("and deserializes");
    assert_eq!(original.digest(), restored.digest());
    assert_eq!(original, restored);
}

#[test]
fn a_hit_carries_a_proof_naming_every_component_that_matched() {
    let schema = schema(ReuseRule::SameBuildOnly);
    let mut cache = Cache::new(schema.clone());
    let builder = build("build-a");
    cache
        .insert(
            key(&schema, "gpt"),
            json!({ "section": "L0" }),
            builder.clone(),
            Epoch::new(4),
            DependencyDeclaration::on([]),
        )
        .expect("insert accepts a valid key");

    let lookup = cache
        .lookup(&key(&schema, "gpt"), &builder)
        .expect("no collision");
    let hit = lookup.hit().expect("the same computation hits");
    assert_eq!(hit.value, json!({ "section": "L0" }));
    let names: Vec<&str> = hit.proof.matched.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(
        names,
        vec!["code", "environment", "inputs", "oracle", "policy"]
    );
    assert_eq!(hit.proof.written_at, Epoch::new(4));
    assert!(!hit.proof.is_cross_build());
}

#[test]
fn changing_one_component_misses_rather_than_hitting_a_neighbour() {
    let schema = schema(ReuseRule::SameBuildOnly);
    let mut cache = Cache::new(schema.clone());
    let builder = build("build-a");
    cache
        .insert(
            key(&schema, "gpt"),
            json!(1),
            builder.clone(),
            Epoch::ZERO,
            DependencyDeclaration::on([]),
        )
        .expect("insert");

    let lookup = cache
        .lookup(&key(&schema, "claude"), &builder)
        .expect("no collision");
    assert_eq!(lookup.miss_reason(), Some(&MissReason::NoEntry));
}

#[test]
fn a_digest_that_maps_to_a_different_computation_is_a_typed_collision_naming_the_component() {
    let schema = schema(ReuseRule::SameBuildOnly);
    let honest = key(&schema, "gpt");
    let impostor = key(&schema, "claude");

    let mut cache = Cache::restore(
        schema.clone(),
        [(
            honest.digest(),
            CacheEntry {
                key: impostor,
                value: json!("answer for a different oracle"),
                produced_by: build("build-a"),
                written_at: Epoch::ZERO,
                dependencies: DependencyDeclaration::on([]),
                status: EntryStatus::Proven,
            },
        )],
    );

    let error = cache
        .lookup(&honest, &build("build-a"))
        .expect_err("the digest matches but the computation does not");
    match error {
        CacheError::KeyCollision {
            component,
            stored,
            presented,
            ..
        } => {
            assert_eq!(component, "oracle");
            assert_eq!(stored, "claude");
            assert_eq!(presented, "gpt");
        }
        other => panic!("expected a key collision, got {other:?}"),
    }
}

#[test]
fn a_collision_is_never_downgraded_to_a_miss() {
    let schema = schema(ReuseRule::SameBuildOnly);
    let honest = key(&schema, "gpt");
    let mut cache = Cache::restore(
        schema.clone(),
        [(
            honest.digest(),
            CacheEntry {
                key: key(&schema, "claude"),
                value: json!(0),
                produced_by: build("build-a"),
                written_at: Epoch::ZERO,
                dependencies: DependencyDeclaration::on([]),
                status: EntryStatus::Proven,
            },
        )],
    );
    let result = cache.lookup(&honest, &build("build-a"));
    assert!(result.is_err());
    assert_eq!(
        cache.misses_by_reason().values().sum::<u64>(),
        0,
        "a collision must not be counted as an ordinary miss"
    );
}

#[test]
fn restore_keeps_the_persisted_address_so_a_changed_digest_function_surfaces_as_a_collision() {
    let schema = schema(ReuseRule::SameBuildOnly);
    let stored_key = key(&schema, "gpt");
    let cache = Cache::restore(
        schema,
        [(
            "not-the-digest-this-build-would-compute".to_string(),
            CacheEntry {
                key: stored_key.clone(),
                value: json!(1),
                produced_by: build("build-a"),
                written_at: Epoch::ZERO,
                dependencies: DependencyDeclaration::on([]),
                status: EntryStatus::Proven,
            },
        )],
    );
    assert!(
        cache
            .get("not-the-digest-this-build-would-compute")
            .is_some(),
        "the persisted address is kept exactly as written"
    );
    assert!(cache.get(&stored_key.digest()).is_none());
}

#[test]
fn adding_a_component_to_a_schema_produces_a_miss_and_never_a_collision() {
    let old = schema(ReuseRule::SameBuildOnly);
    let new = KeySchema::declare(
        "compile-decision-section",
        ["inputs", "code", "environment", "oracle", "policy", "seed"],
        ReuseRule::SameBuildOnly,
    )
    .expect("schema");

    let old_key = key(&old, "gpt");
    let new_key = ComputationKey::build(
        &new,
        [
            ("inputs", "world@3"),
            ("code", "fiber@1.2"),
            ("environment", "linux-x86_64"),
            ("oracle", "gpt"),
            ("policy", "strict"),
            ("seed", "7"),
        ],
    )
    .expect("key");

    assert_ne!(old.digest(), new.digest());
    assert_ne!(
        old_key.digest(),
        new_key.digest(),
        "the schema digest is folded into the key address"
    );

    let mut cache = Cache::new(new.clone());
    cache
        .insert(
            new_key.clone(),
            json!(1),
            build("build-a"),
            Epoch::ZERO,
            DependencyDeclaration::on([]),
        )
        .expect("insert");
    let error = cache
        .lookup(&old_key, &build("build-a"))
        .expect_err("a key from the old schema does not belong to this cache");
    assert!(matches!(error, CacheError::ForeignSchema { .. }));
}

#[test]
fn a_value_from_another_build_misses_when_the_schema_forbids_cross_build_reuse() {
    let schema = schema(ReuseRule::SameBuildOnly);
    let mut cache = Cache::new(schema.clone());
    cache
        .insert(
            key(&schema, "gpt"),
            json!(1),
            build("build-a"),
            Epoch::ZERO,
            DependencyDeclaration::on([]),
        )
        .expect("insert");

    let lookup = cache
        .lookup(&key(&schema, "gpt"), &build("build-b"))
        .expect("no collision");
    assert_eq!(
        lookup.miss_reason(),
        Some(&MissReason::CrossBuild {
            produced_by: build("build-a"),
            requested_by: build("build-b"),
        })
    );
}

#[test]
fn a_value_from_another_build_hits_when_the_schema_declares_the_computation_reproducible() {
    let schema = schema(ReuseRule::AcrossBuilds);
    let mut cache = Cache::new(schema.clone());
    cache
        .insert(
            key(&schema, "gpt"),
            json!(1),
            build("build-a"),
            Epoch::ZERO,
            DependencyDeclaration::on([]),
        )
        .expect("insert");

    let lookup = cache
        .lookup(&key(&schema, "gpt"), &build("build-b"))
        .expect("no collision");
    let hit = lookup.hit().expect("cross-build reuse is permitted here");
    assert!(
        hit.proof.is_cross_build(),
        "the proof must record that the value came from another build"
    );
    assert_eq!(hit.proof.produced_by, build("build-a"));
    assert_eq!(hit.proof.reuse, ReuseRule::AcrossBuilds);
}

#[test]
fn the_conservative_reuse_rule_is_the_default() {
    assert_eq!(ReuseRule::default(), ReuseRule::SameBuildOnly);
}

#[test]
fn misses_are_counted_by_reason_so_a_cold_cache_and_a_refusing_one_do_not_look_alike() {
    let schema = schema(ReuseRule::SameBuildOnly);
    let mut cache = Cache::new(schema.clone());
    cache
        .insert(
            key(&schema, "gpt"),
            json!(1),
            build("build-a"),
            Epoch::ZERO,
            DependencyDeclaration::on([]),
        )
        .expect("insert");

    let _ = cache.lookup(&key(&schema, "claude"), &build("build-a"));
    let _ = cache.lookup(&key(&schema, "gpt"), &build("build-b"));
    let _ = cache.lookup(&key(&schema, "gpt"), &build("build-a"));

    assert_eq!(cache.hits(), 1);
    assert_eq!(cache.misses_by_reason().get("no-entry"), Some(&1));
    assert_eq!(cache.misses_by_reason().get("cross-build"), Some(&1));
    assert!((cache.hit_rate() - 1.0 / 3.0).abs() < 1e-9);
}

#[test]
fn a_hit_proof_serializes_so_a_caller_can_record_why_it_did_not_recompute() {
    let schema = schema(ReuseRule::SameBuildOnly);
    let mut cache = Cache::new(schema.clone());
    let builder = build("build-a");
    cache
        .insert(
            key(&schema, "gpt"),
            json!(1),
            builder.clone(),
            Epoch::new(9),
            DependencyDeclaration::on([]),
        )
        .expect("insert");
    let lookup = cache
        .lookup(&key(&schema, "gpt"), &builder)
        .expect("lookup");
    let Lookup::Hit(hit) = lookup else {
        panic!("expected a hit");
    };
    let text = serde_json::to_string(&hit.proof).expect("the proof serializes");
    assert!(text.contains("\"oracle\""));
    assert!(text.contains("build-a"));
}

#[test]
fn a_blank_build_identity_is_refused() {
    assert!(CodeIdentity::parse("  ").is_err());
    assert!(CodeIdentity::parse("build\u{7}a").is_err());
}
