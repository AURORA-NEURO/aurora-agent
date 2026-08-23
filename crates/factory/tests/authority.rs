//! Shared authority tests exercise the cross-process boundary, not just a JSON round trip.

use bioprism_factory::{
    AuthorityLockInfo, AuthorityMutation, ExecutionAuthoritySnapshot, ExecutionOperation,
    Idempotency, Job, JobStore, QueueAdmissionPolicy, ResourceClass, SharedExecutionAuthority,
};
use bioprism_scope::Timestamp;
use serde_json::json;
use std::path::PathBuf;
use std::sync::{Arc, Barrier};
use std::thread;

const SECOND: i128 = 1_000_000_000;

fn at(seconds: i128) -> Timestamp {
    Timestamp::from_nanos_utc(seconds * SECOND)
}

fn path(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "bioprism-authority-{label}-{}-{}-{}.json",
        std::process::id(),
        thread::current().name().unwrap_or("test"),
        randless_id()
    ))
}

fn randless_id() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(1);
    NEXT.fetch_add(1, Ordering::Relaxed)
}

fn mutation(key: &str, job_id: &str, at: Timestamp) -> AuthorityMutation {
    AuthorityMutation::new(
        ExecutionOperation::EnqueueAndLease,
        key,
        Some(job_id.into()),
        Some("test-worker".into()),
        Some(1),
        at,
        json!({ "resource_class": "compile", "test": true }),
    )
}

fn enqueue_job(store: &mut JobStore, id: &str) -> Result<String, bioprism_factory::FactoryError> {
    store.enqueue(Job::new(
        id,
        ResourceClass::Compile,
        Idempotency::Idempotent,
        json!({ "id": id }),
    ))
}

#[test]
fn authority_replays_legacy_queue_checkpoint_without_erasing_work() {
    let checkpoint = path("legacy-migration");
    let mut queue = JobStore::new();
    enqueue_job(&mut queue, "legacy-job").unwrap();
    queue.checkpoint_to_path(&checkpoint).unwrap();

    let authority = SharedExecutionAuthority::open(Some(checkpoint.clone())).unwrap();
    let snapshot = authority.snapshot().unwrap();
    assert_eq!(snapshot.queue.jobs[0].id, "legacy-job");
    assert_eq!(snapshot.events.len(), 0);
    assert_eq!(snapshot.schema_version, 1);
    authority.flush().unwrap();
    let persisted = ExecutionAuthoritySnapshot::load_from_path(&checkpoint).unwrap();
    assert_eq!(persisted.queue.jobs[0].id, "legacy-job");
    assert_eq!(persisted.events.len(), 0);
    let _ = std::fs::remove_file(checkpoint);
}

#[test]
fn identical_transition_retries_do_not_duplicate_the_authority_history() {
    let checkpoint = path("idempotent");
    let authority = SharedExecutionAuthority::open(Some(checkpoint.clone())).unwrap();
    let policy = QueueAdmissionPolicy::new(10, 10);
    let first = mutation("enqueue:one", "one", at(0));
    authority
        .mutate(first.clone(), |queue| enqueue_job(queue, "one"))
        .unwrap();
    authority
        .mutate(first, |queue| enqueue_job(queue, "one"))
        .unwrap();
    let snapshot = authority.snapshot().unwrap();
    assert_eq!(snapshot.revision, 1);
    assert_eq!(snapshot.events.len(), 1);
    assert_eq!(snapshot.queue.jobs.len(), 1);
    assert_eq!(policy.max_jobs, 10);
    let _ = std::fs::remove_file(checkpoint);
}

#[test]
fn cooperating_authorities_serialize_updates_without_losing_jobs() {
    let checkpoint = path("concurrent");
    let workers = 12;
    let barrier = Arc::new(Barrier::new(workers));
    let mut handles = Vec::new();
    for index in 0..workers {
        let barrier = Arc::clone(&barrier);
        let checkpoint = checkpoint.clone();
        handles.push(thread::spawn(move || {
            let authority = SharedExecutionAuthority::open(Some(checkpoint)).unwrap();
            let id = format!("job-{index}");
            barrier.wait();
            authority
                .mutate(
                    mutation(&format!("enqueue:{id}"), &id, at(index as i128)),
                    |queue| enqueue_job(queue, &id),
                )
                .unwrap();
        }));
    }
    for handle in handles {
        handle.join().unwrap();
    }

    let restored = ExecutionAuthoritySnapshot::load_from_path(&checkpoint).unwrap();
    assert_eq!(restored.queue.jobs.len(), workers);
    assert_eq!(restored.events.len(), workers);
    restored.verify().unwrap();
    let _ = std::fs::remove_file(checkpoint);
}

#[test]
fn tampering_with_a_transition_is_rejected_before_projection() {
    let checkpoint = path("tamper");
    let authority = SharedExecutionAuthority::open(Some(checkpoint.clone())).unwrap();
    authority
        .mutate(mutation("enqueue:tamper", "tamper", at(0)), |queue| {
            enqueue_job(queue, "tamper")
        })
        .unwrap();
    let mut document: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&checkpoint).unwrap()).unwrap();
    document["events"][0]["details"]["test"] = json!(false);
    std::fs::write(&checkpoint, serde_json::to_vec(&document).unwrap()).unwrap();
    assert!(matches!(
        ExecutionAuthoritySnapshot::load_from_path(&checkpoint),
        Err(bioprism_factory::FactoryError::AuthorityDigestMismatch { .. })
            | Err(bioprism_factory::FactoryError::InvalidAuthoritySnapshot { .. })
    ));
    let _ = std::fs::remove_file(checkpoint);
}

#[test]
fn releasing_an_orphaned_lock_requires_attribution_and_is_journaled() {
    let checkpoint = path("unlock");
    let authority = SharedExecutionAuthority::open(Some(checkpoint.clone())).unwrap();
    let lock = checkpoint.with_file_name(format!(
        ".{}.authority-lock",
        checkpoint.file_name().unwrap().to_string_lossy()
    ));
    std::fs::create_dir_all(&lock).unwrap();
    std::fs::write(
        lock.join("owner.json"),
        serde_json::to_vec(&AuthorityLockInfo {
            owner_id: "crashed-worker".into(),
            acquired_unix_nanos: 12,
        })
        .unwrap(),
    )
    .unwrap();

    let release = authority
        .release_orphaned_lock("operator-1", "confirmed process terminated", at(9))
        .unwrap();
    assert_eq!(release.previous_owner.owner_id, "crashed-worker");
    assert_eq!(authority.status().unwrap().revision, 1);
    assert!(!authority.status().unwrap().lock_present);
    assert!(authority
        .release_orphaned_lock("", "missing attribution", at(10))
        .is_err());
    let _ = std::fs::remove_file(checkpoint);
}
