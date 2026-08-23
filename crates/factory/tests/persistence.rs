//! Persistence tests assert restart boundaries rather than merely JSON round trips.

use bioprism_factory::{
    FactoryError, Idempotency, Job, JobState, JobStore, ResourceClass, WorkerCapability,
};
use bioprism_scope::Timestamp;
use serde_json::json;

const SECOND: i128 = 1_000_000_000;

fn at(seconds: i128) -> Timestamp {
    Timestamp::from_nanos_utc(seconds * SECOND)
}

fn worker(id: &str) -> WorkerCapability {
    WorkerCapability::new(id, vec![ResourceClass::Compile]).with_lease_duration_nanos(30 * SECOND)
}

fn job(id: &str, spec: &str) -> Job {
    Job::new(
        id,
        ResourceClass::Compile,
        Idempotency::Idempotent,
        json!({ "spec": spec }),
    )
}

#[test]
fn snapshot_round_trip_preserves_leases_outputs_and_deduplication() {
    let mut store = JobStore::new();
    store
        .enqueue(job("staged", "one").with_priority(9))
        .unwrap();
    store.enqueue(job("done", "two")).unwrap();
    store.lease(&worker("w1"), at(0)).unwrap();
    store
        .stage("staged", "w1", 1, at(1), json!({ "partial": true }))
        .unwrap();
    store.lease(&worker("w2"), at(2)).unwrap();
    store
        .stage("done", "w2", 1, at(3), json!({ "answer": 42 }))
        .unwrap();
    store.commit("done", "w2", 1, at(4)).unwrap();

    let snapshot = store.snapshot().unwrap();
    assert_eq!(snapshot.schema_version, 1);
    assert_eq!(snapshot.digest().unwrap(), snapshot.state_digest);
    let mut restored = JobStore::from_snapshot(snapshot).unwrap();

    assert_eq!(restored.job("staged").unwrap().state, JobState::Staged);
    assert_eq!(restored.job("done").unwrap().state, JobState::Succeeded);
    assert_eq!(restored.result("staged"), None);
    assert_eq!(restored.result("done"), Some(&json!({ "answer": 42 })));
    assert_eq!(
        restored
            .enqueue(job("renamed", "one"))
            .expect("deduplication index survives restore"),
        "staged"
    );
}

#[test]
fn tampered_snapshot_is_rejected_before_restore() {
    let mut store = JobStore::new();
    store.enqueue(job("j1", "one")).unwrap();
    let mut snapshot = store.snapshot().unwrap();
    snapshot.jobs[0].priority = 255;

    assert!(matches!(
        JobStore::from_snapshot(snapshot),
        Err(FactoryError::SnapshotDigestMismatch { .. })
    ));
}

#[test]
fn structurally_invalid_snapshot_is_rejected_even_with_a_valid_digest() {
    let mut store = JobStore::new();
    store.enqueue(job("j1", "one")).unwrap();
    let mut snapshot = store.snapshot().unwrap();
    snapshot.jobs.push(snapshot.jobs[0].clone());
    snapshot.state_digest = snapshot.digest().unwrap();

    assert!(matches!(
        JobStore::from_snapshot(snapshot),
        Err(FactoryError::InvalidSnapshot { .. })
    ));
}

#[test]
fn checkpoint_recovery_persists_the_idempotency_branch_after_restart() {
    let path = std::env::temp_dir().join(format!(
        "bioprism-factory-checkpoint-{}-{}.json",
        std::process::id(),
        std::thread::current().name().unwrap_or("test")
    ));
    let mut store = JobStore::new();
    store.enqueue(job("j1", "one")).unwrap();
    store.lease(&worker("w1"), at(0)).unwrap();
    store.checkpoint_to_path(&path).unwrap();

    let recoveries = JobStore::recover_expired_at_path(&path, at(31)).unwrap();
    assert_eq!(recoveries.len(), 1);
    assert_eq!(
        JobStore::load_from_path(&path)
            .unwrap()
            .job("j1")
            .unwrap()
            .state,
        JobState::Queued
    );
    std::fs::remove_file(path).unwrap();
}

#[test]
fn malformed_checkpoint_is_not_treated_as_an_empty_queue() {
    let path = std::env::temp_dir().join(format!(
        "bioprism-factory-malformed-{}.json",
        std::process::id()
    ));
    std::fs::write(&path, b"{ definitely not json").unwrap();
    assert!(matches!(
        JobStore::load_from_path(&path),
        Err(FactoryError::InvalidSnapshot { .. })
    ));
    std::fs::remove_file(path).unwrap();
}
