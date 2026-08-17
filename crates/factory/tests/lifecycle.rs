//! The four non-negotiable invariants of blueprint 40.30, each asserted by name.

use bioprism_factory::{
    FactoryError, Idempotency, Job, JobState, JobStore, QueueAdmissionPolicy, Recovery,
    ResourceClass, WorkerCapability,
};
use bioprism_scope::Timestamp;
use serde_json::json;

const SECOND: i128 = 1_000_000_000;

fn at(seconds: i128) -> Timestamp {
    Timestamp::from_nanos_utc(seconds * SECOND)
}

fn worker(id: &str) -> WorkerCapability {
    WorkerCapability::new(
        id,
        vec![ResourceClass::Compile, ResourceClass::Evaluate, ResourceClass::Ingest],
    )
    .with_lease_duration_nanos(30 * SECOND)
}

fn job(id: &str, idempotency: Idempotency) -> Job {
    Job::new(id, ResourceClass::Compile, idempotency, json!({ "world": id }))
}

#[test]
fn a_job_runs_enqueue_lease_stage_commit() {
    let mut store = JobStore::new();
    store.enqueue(job("j1", Idempotency::Idempotent)).unwrap();

    let lease = store.lease(&worker("w1"), at(0)).unwrap().expect("a job was available");
    assert_eq!(lease.job_id, "j1");
    assert_eq!(lease.attempt, 1);
    assert_eq!(store.job("j1").unwrap().state, JobState::Leased);

    store.stage("j1", "w1", 1, at(5), json!({ "certificate": "abc" })).unwrap();
    assert_eq!(store.job("j1").unwrap().state, JobState::Staged);

    store.commit("j1", "w1", 1, at(6)).unwrap();
    assert_eq!(store.job("j1").unwrap().state, JobState::Succeeded);
    assert_eq!(store.result("j1"), Some(&json!({ "certificate": "abc" })));
}

/// Invariant 1.
#[test]
fn one_active_lease_per_attempt() {
    let mut store = JobStore::new();
    store.enqueue(job("j1", Idempotency::Idempotent)).unwrap();

    assert!(store.lease(&worker("w1"), at(0)).unwrap().is_some());
    assert!(
        store.lease(&worker("w2"), at(1)).unwrap().is_none(),
        "a leased job must not be claimable by a second worker"
    );

    assert!(matches!(
        store.commit("j1", "w2", 1, at(2)),
        Err(FactoryError::LeaseHeldByAnother { .. })
    ));
    assert!(matches!(
        store.heartbeat("j1", "w2", 1, at(2), 30 * SECOND),
        Err(FactoryError::LeaseHeldByAnother { .. })
    ));
}

/// Invariant 3: staged output is invisible until committed.
#[test]
fn staged_output_is_not_readable_before_commit() {
    let mut store = JobStore::new();
    store.enqueue(job("j1", Idempotency::Idempotent)).unwrap();
    store.lease(&worker("w1"), at(0)).unwrap();
    store.stage("j1", "w1", 1, at(1), json!({ "partial": true })).unwrap();

    assert_eq!(store.result("j1"), None, "a reader must not see uncommitted work");
    store.commit("j1", "w1", 1, at(2)).unwrap();
    assert!(store.result("j1").is_some());
}

#[test]
fn a_success_with_nothing_staged_is_refused() {
    let mut store = JobStore::new();
    store.enqueue(job("j1", Idempotency::Idempotent)).unwrap();
    store.lease(&worker("w1"), at(0)).unwrap();

    assert!(matches!(
        store.commit("j1", "w1", 1, at(1)),
        Err(FactoryError::NothingStaged { .. })
    ));
}

/// Invariant 2, the idempotent branch.
#[test]
fn an_expired_lease_on_idempotent_work_requeues() {
    let mut store = JobStore::new();
    store.enqueue(job("j1", Idempotency::Idempotent)).unwrap();
    store.lease(&worker("w1"), at(0)).unwrap();

    let recoveries = store.recover_expired(at(31));
    assert_eq!(
        recoveries,
        vec![Recovery::Requeued { job_id: "j1".into(), attempt: 1 }]
    );
    assert_eq!(store.job("j1").unwrap().state, JobState::Queued);
    assert!(store.lease(&worker("w2"), at(32)).unwrap().is_some());
}

/// Lease attempts are fencing tokens: a stale worker with the same identity cannot commit after
/// recovery has handed the job to a later attempt.
#[test]
fn a_stale_attempt_is_fenced_even_when_the_worker_identity_is_reused() {
    let mut store = JobStore::new();
    store.enqueue(job("j1", Idempotency::Idempotent)).unwrap();
    let first = store.lease(&worker("w1"), at(0)).unwrap().unwrap();
    assert_eq!(first.attempt, 1);
    store.recover_expired(at(31));
    let second = store.lease(&worker("w1"), at(32)).unwrap().unwrap();
    assert_eq!(second.attempt, 2);

    assert!(matches!(
        store.stage("j1", "w1", first.attempt, at(33), json!({ "stale": true })),
        Err(FactoryError::StaleLease {
            expected_attempt: 1,
            active_attempt: 2,
            ..
        })
    ));
    store
        .stage("j1", "w1", second.attempt, at(33), json!({ "fresh": true }))
        .unwrap();
    store.commit("j1", "w1", second.attempt, at(34)).unwrap();
    assert_eq!(store.result("j1"), Some(&json!({ "fresh": true })));
}

/// Invariant 2, the case a naive queue gets wrong.
#[test]
fn an_expired_lease_on_non_idempotent_work_is_quarantined_not_retried() {
    let mut store = JobStore::new();
    store.enqueue(job("j1", Idempotency::NonIdempotent)).unwrap();
    store.lease(&worker("w1"), at(0)).unwrap();

    let recoveries = store.recover_expired(at(31));
    assert!(matches!(recoveries[0], Recovery::Quarantined { .. }));
    assert_eq!(store.job("j1").unwrap().state, JobState::Quarantined);
    assert!(
        store.lease(&worker("w2"), at(32)).unwrap().is_none(),
        "a quarantined job must not be handed to another worker"
    );

    let reason = store.job("j1").unwrap().reason.clone().unwrap();
    assert!(
        reason.contains("may or may not have landed"),
        "the quarantine must say why a missed heartbeat is not evidence of non-execution"
    );
}

#[test]
fn compensable_work_waits_for_compensation_before_becoming_eligible() {
    let mut store = JobStore::new();
    store.enqueue(job("j1", Idempotency::Compensable)).unwrap();
    store.lease(&worker("w1"), at(0)).unwrap();

    assert!(matches!(
        store.recover_expired(at(31))[0],
        Recovery::AwaitingCompensation { .. }
    ));
    assert!(store.lease(&worker("w2"), at(32)).unwrap().is_none());

    store.compensate("j1").unwrap();
    assert_eq!(store.job("j1").unwrap().state, JobState::Queued);
    assert!(store.lease(&worker("w2"), at(33)).unwrap().is_some());
}

#[test]
fn compensation_is_refused_for_jobs_that_are_not_compensable() {
    let mut store = JobStore::new();
    store.enqueue(job("j1", Idempotency::NonIdempotent)).unwrap();
    store.lease(&worker("w1"), at(0)).unwrap();
    store.recover_expired(at(31));

    assert!(matches!(
        store.compensate("j1"),
        Err(FactoryError::NotCompensable { .. })
    ));
}

/// A reported failure is unambiguous, so even non-idempotent work may retry.
#[test]
fn a_reported_failure_retries_where_an_expired_lease_would_not() {
    let mut store = JobStore::new();
    store.enqueue(job("j1", Idempotency::NonIdempotent)).unwrap();
    store.lease(&worker("w1"), at(0)).unwrap();

    let recovery = store.fail("j1", "w1", 1, at(5), "upstream returned 503").unwrap();
    assert!(matches!(recovery, Recovery::Requeued { .. }));
    assert_eq!(
        store.job("j1").unwrap().state,
        JobState::Queued,
        "the worker was alive and says the effect did not land; that is not ambiguous"
    );
}

#[test]
fn releasing_a_quarantine_requires_a_named_operator() {
    let mut store = JobStore::new();
    store.enqueue(job("j1", Idempotency::NonIdempotent)).unwrap();
    store.lease(&worker("w1"), at(0)).unwrap();
    store.recover_expired(at(31));

    assert!(matches!(
        store.release_quarantine("j1", "  "),
        Err(FactoryError::UnattributedRelease { .. })
    ));
    store.release_quarantine("j1", "m.ambati").unwrap();
    assert_eq!(store.job("j1").unwrap().state, JobState::Queued);
    assert!(store.job("j1").unwrap().reason.clone().unwrap().contains("m.ambati"));
}

#[test]
fn a_job_out_of_attempts_is_dead_lettered_rather_than_looping() {
    let mut store = JobStore::new();
    store
        .enqueue(job("j1", Idempotency::Idempotent).with_max_attempts(2))
        .unwrap();

    store.lease(&worker("w1"), at(0)).unwrap();
    store.recover_expired(at(31));
    store.lease(&worker("w1"), at(32)).unwrap();
    let recoveries = store.recover_expired(at(63));

    assert!(matches!(recoveries[0], Recovery::DeadLettered { attempts: 2, .. }));
    assert_eq!(store.job("j1").unwrap().state, JobState::DeadLettered);
    assert_eq!(store.dead_lettered().len(), 1);
    assert!(store.lease(&worker("w1"), at(64)).unwrap().is_none());
}

/// Invariant 4.
#[test]
fn cancellation_is_explicit_and_drops_staged_work() {
    let mut store = JobStore::new();
    store.enqueue(job("j1", Idempotency::Idempotent)).unwrap();
    store.lease(&worker("w1"), at(0)).unwrap();
    store.stage("j1", "w1", 1, at(1), json!({ "partial": true })).unwrap();

    store.cancel("j1", "superseded by a newer world").unwrap();
    assert_eq!(store.job("j1").unwrap().state, JobState::Cancelled);
    assert_eq!(store.result("j1"), None);
    assert!(matches!(
        store.cancel("j1", "again"),
        Err(FactoryError::AlreadyTerminal { .. })
    ));
}

#[test]
fn a_heartbeat_extends_a_lease_but_cannot_resurrect_an_expired_one() {
    let mut store = JobStore::new();
    store.enqueue(job("j1", Idempotency::Idempotent)).unwrap();
    store.lease(&worker("w1"), at(0)).unwrap();

    store.heartbeat("j1", "w1", 1, at(20), 30 * SECOND).unwrap();
    assert!(
        store.recover_expired(at(31)).is_empty(),
        "a heartbeat at t=20 extends the lease past t=31"
    );

    assert!(matches!(
        store.heartbeat("j1", "w1", 1, at(100), 30 * SECOND),
        Err(FactoryError::LeaseExpired { .. })
    ));
}

#[test]
fn a_worker_cannot_commit_after_its_lease_expired() {
    let mut store = JobStore::new();
    store.enqueue(job("j1", Idempotency::Idempotent)).unwrap();
    store.lease(&worker("w1"), at(0)).unwrap();
    store.stage("j1", "w1", 1, at(1), json!({ "x": 1 })).unwrap();

    assert!(matches!(
        store.commit("j1", "w1", 1, at(31)),
        Err(FactoryError::LeaseExpired { .. })
    ));
}

#[test]
fn identical_work_submitted_twice_is_deduplicated() {
    let mut store = JobStore::new();
    let first = store.enqueue(job("j1", Idempotency::Idempotent)).unwrap();
    let second = store
        .enqueue(Job::new(
            "j2",
            ResourceClass::Compile,
            Idempotency::Idempotent,
            json!({ "world": "j1" }),
        ))
        .unwrap();

    assert_eq!(first, second, "the same spec is the same work whatever it is named");
    assert_eq!(store.len(), 1);
}

#[test]
fn a_worker_is_only_offered_classes_it_declares() {
    let mut store = JobStore::new();
    store
        .enqueue(Job::new(
            "j1",
            ResourceClass::Sandbox,
            Idempotency::Idempotent,
            json!({}),
        ))
        .unwrap();

    assert!(
        store.lease(&worker("w1"), at(0)).unwrap().is_none(),
        "worker w1 does not declare Sandbox"
    );
    let sandboxed = WorkerCapability::new("w2", vec![ResourceClass::Sandbox]);
    assert!(store.lease(&sandboxed, at(0)).unwrap().is_some());
}

#[test]
fn admission_policy_refuses_total_class_and_active_lease_overflow() {
    let policy = QueueAdmissionPolicy::new(2, 1)
        .with_resource_class_limit(ResourceClass::Compile, 1, 1);
    let mut store = JobStore::new();
    store
        .enqueue_with_policy(job("j1", Idempotency::Idempotent), &policy)
        .unwrap();
    assert!(matches!(
        store.enqueue_with_policy(
            Job::new("j2", ResourceClass::Compile, Idempotency::Idempotent, json!({})),
            &policy,
        ),
        Err(FactoryError::AdmissionLimit { dimension, .. }) if dimension == "jobs_by_class:Compile"
    ));
    store
        .enqueue_with_policy(
            Job::new("j3", ResourceClass::Evaluate, Idempotency::Idempotent, json!({})),
            &policy,
        )
        .unwrap();
    store.lease_with_policy(&worker("w1"), at(0), &policy).unwrap();
    assert!(matches!(
        store.lease_with_policy(&worker("w2"), at(1), &policy),
        Err(FactoryError::AdmissionLimit { dimension, .. }) if dimension == "active_leases"
    ));
}

#[test]
fn higher_priority_work_is_leased_first() {
    let mut store = JobStore::new();
    store.enqueue(job("low", Idempotency::Idempotent).with_priority(1)).unwrap();
    store.enqueue(job("high", Idempotency::Idempotent).with_priority(9)).unwrap();

    assert_eq!(store.lease(&worker("w1"), at(0)).unwrap().unwrap().job_id, "high");
    assert_eq!(store.lease(&worker("w2"), at(0)).unwrap().unwrap().job_id, "low");
}

#[test]
fn the_store_reports_its_quarantine_and_dead_letter_queues() {
    let mut store = JobStore::new();
    store.enqueue(job("bad", Idempotency::NonIdempotent)).unwrap();
    store.enqueue(job("ok", Idempotency::Idempotent)).unwrap();

    store.lease(&worker("w1"), at(0)).unwrap();
    store.lease(&worker("w2"), at(0)).unwrap();
    store.recover_expired(at(31));

    assert_eq!(store.quarantined().len(), 1);
    assert_eq!(store.quarantined()[0].id, "bad");
    assert_eq!(store.counts_by_class()[&ResourceClass::Compile], 2);
}
