//! Blueprint 35.12. A cache hit must be provably the same computation, not merely the same key.

use bioprism_scale::cas::{
    put_world_release, ComputationKey, Delta, ObjectStore, ReplayCache,
};
use bioprism_scale::error::CacheError;
use serde_json::json;

fn key() -> ComputationKey {
    ComputationKey::new("inputs-digest", "code-digest", "env-digest", "oracle-digest").unwrap()
}

#[test]
fn a_cache_key_missing_any_component_is_refused() {
    for (index, name) in ["inputs", "code", "environment", "oracle"].into_iter().enumerate() {
        let mut parts = ["inputs-d", "code-d", "env-d", "oracle-d"];
        parts[index] = "";
        match ComputationKey::new(parts[0], parts[1], parts[2], parts[3]) {
            Err(CacheError::IncompleteKey(missing)) => assert_eq!(missing, name),
            other => panic!("an incomplete semantic key must be refused: {other:?}"),
        }
    }
}

#[test]
fn a_cache_hit_proves_every_component_matched() {
    let mut cache = ReplayCache::new();
    cache.insert(key(), json!({ "verdict": "Sufficient" }));

    let hit = cache.lookup(&key()).unwrap().expect("the same computation");
    assert_eq!(hit.value, json!({ "verdict": "Sufficient" }));
    assert_eq!(hit.proof.matched.len(), 4);
    let names: Vec<&str> = hit.proof.matched.iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(names, ["inputs", "code", "environment", "oracle"]);
    assert_eq!(hit.proof.digest, key().digest());
}

#[test]
fn changing_any_single_key_component_turns_a_hit_into_a_miss() {
    let base = ["inputs-digest", "code-digest", "env-digest", "oracle-digest"];
    for index in 0..4 {
        let mut cache = ReplayCache::new();
        cache.insert(key(), json!("stale"));

        let mut parts = base;
        parts[index] = "changed";
        let altered = ComputationKey::new(parts[0], parts[1], parts[2], parts[3]).unwrap();
        assert!(
            cache.lookup(&altered).unwrap().is_none(),
            "component {index} must participate in the key; a cache blind to the oracle serves \
             last month's answer"
        );
    }
}

#[test]
fn a_persisted_index_whose_digest_disagrees_refuses_to_serve() {
    let stored = ComputationKey::new("inputs-a", "code-a", "env-a", "oracle-a").unwrap();
    let presented = ComputationKey::new("inputs-a", "code-a", "env-a", "oracle-b").unwrap();

    let mut cache = ReplayCache::restore([(presented.digest(), stored, json!("wrong answer"))]);

    match cache.lookup(&presented) {
        Err(CacheError::KeyCollision {
            component,
            stored,
            presented,
            ..
        }) => {
            assert_eq!(component, "oracle");
            assert_eq!(stored, "oracle-a");
            assert_eq!(presented, "oracle-b");
        }
        other => panic!("the same address is not the same computation: {other:?}"),
    }
}

#[test]
fn the_hit_rate_counts_misses_and_never_counts_refusals() {
    let mut cache = ReplayCache::new();
    cache.insert(key(), json!(1));
    cache.lookup(&key()).unwrap();
    cache
        .lookup(&ComputationKey::new("other", "code-digest", "env-digest", "oracle-digest").unwrap())
        .unwrap();
    assert_eq!(cache.hit_rate(), 0.5);
    assert_eq!(cache.len(), 1);
}

#[test]
fn a_computation_key_round_trips_only_through_its_validating_constructor() {
    let encoded = serde_json::to_string(&key()).unwrap();
    let decoded: ComputationKey = serde_json::from_str(&encoded).unwrap();
    assert_eq!(decoded, key());

    let incomplete = r#"{"inputs":"a","code":"","environment":"c","oracle":"d"}"#;
    assert!(
        serde_json::from_str::<ComputationKey>(incomplete).is_err(),
        "deserialization must not be a back door around the completeness check"
    );
}

#[test]
fn an_object_is_addressed_by_its_content_so_storing_it_twice_is_free() {
    let mut store = ObjectStore::new();
    let first = store.put(b"the same bytes".to_vec());
    let second = store.put(b"the same bytes".to_vec());
    let other = store.put(b"different bytes".to_vec());

    assert_eq!(first, second);
    assert_ne!(first, other);
    assert_eq!(store.object_count(), 2);
    assert_eq!(store.get(&first).unwrap(), b"the same bytes");
    store.verify().expect("a healthy store rehashes to its own addresses");
}

#[test]
fn a_missing_object_is_named() {
    let store = ObjectStore::new();
    assert!(matches!(
        store.get("no-such-address"),
        Err(CacheError::MissingObject(address)) if address == "no-such-address"
    ));
}

#[test]
fn a_delta_chain_materializes_to_the_state_a_full_snapshot_would_hold() {
    let mut store = ObjectStore::new();
    let a = store.put(b"world-a".to_vec());
    let b = store.put(b"world-b".to_vec());
    let c = store.put(b"world-c".to_vec());

    store
        .snapshot(
            "base",
            None,
            Delta::new().add("alpha", &a).add("beta", &b),
        )
        .unwrap();
    store
        .snapshot(
            "fork",
            Some("base".into()),
            Delta::new().add("gamma", &c).remove("beta"),
        )
        .unwrap();

    let state = store.materialize("fork").unwrap();
    assert_eq!(state.len(), 2);
    assert_eq!(state["alpha"], a);
    assert_eq!(state["gamma"], c);
    assert!(!state.contains_key("beta"));

    let (stored, materialized) = store.delta_saving("fork").unwrap();
    assert_eq!(materialized, 2);
    assert_eq!(stored, 4, "two base entries plus one addition and one removal");
}

#[test]
fn a_snapshot_on_a_missing_base_is_refused() {
    let mut store = ObjectStore::new();
    assert!(matches!(
        store.snapshot("child", Some("ghost".into()), Delta::new()),
        Err(CacheError::MissingSnapshot(id)) if id == "ghost"
    ));
}

#[test]
fn a_snapshot_cycle_is_detected_rather_than_walked_forever() {
    let mut store = ObjectStore::new();
    store.snapshot("a", None, Delta::new()).unwrap();
    store.snapshot("b", Some("a".into()), Delta::new()).unwrap();
    store.snapshot("a", Some("b".into()), Delta::new()).unwrap();

    assert!(matches!(
        store.materialize("a"),
        Err(CacheError::SnapshotCycle(_))
    ));
    assert!(matches!(
        store.delta_saving("b"),
        Err(CacheError::SnapshotCycle(_))
    ));
}

#[test]
fn a_world_release_is_addressed_by_the_manifest_the_compiler_trusts() {
    let world = json!({
        "world_id": "w-release",
        "facts": [
            { "id": "f1", "provides": "dose", "value": 10, "tags": ["clinical"] },
            { "id": "f2", "provides": "weight", "value": 70, "tags": ["clinical"] }
        ],
        "factors": [],
        "events": [],
    });

    let directory = std::env::temp_dir().join("bioprism-scale-world-release-test");
    let _ = std::fs::remove_dir_all(&directory);

    let mut store = ObjectStore::new();
    let release = put_world_release(&mut store, &world, &directory).unwrap();

    assert_eq!(release.total_facts, 2);
    assert_eq!(release.world_sha256.len(), 64);
    assert_eq!(release.manifest_address.len(), 64);
    assert!(store.get(&release.manifest_address).is_ok());

    let repeat_directory = std::env::temp_dir().join("bioprism-scale-world-release-test-2");
    let _ = std::fs::remove_dir_all(&repeat_directory);
    let again = put_world_release(&mut store, &world, &repeat_directory).unwrap();
    assert_eq!(
        again.manifest_address, release.manifest_address,
        "the same world builds to the same manifest bytes in any directory"
    );
    assert_eq!(store.object_count(), 1);

    let _ = std::fs::remove_dir_all(&directory);
    let _ = std::fs::remove_dir_all(&repeat_directory);
}
