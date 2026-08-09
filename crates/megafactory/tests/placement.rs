//! Invariants of distributed placement, fencing, and execution accounting, blueprint 35.13.

use bioprism_factory::{Idempotency, Job, ResourceClass, WorkerCapability};
use bioprism_ids::ContentHash;
use bioprism_megafactory::{
    place, AccessTier, Attestation, ExecutionLedger, FenceRegistry, Locale, PlacementError,
    TrustDomain, WorkRequest, WorkerProfile,
};

fn job(id: &str) -> Job {
    Job::new(
        id,
        ResourceClass::Evaluate,
        Idempotency::Idempotent,
        serde_json::json!({ "suite": "release" }),
    )
}

fn attested() -> Attestation {
    Attestation::Attested {
        measurement: ContentHash::of_value(&serde_json::json!({ "image": "runner-1" }))
            .expect("finite json"),
        vouched_by: "site-attestation-service".into(),
    }
}

fn worker(id: &str, domain: &str, locale: &str, attestation: Attestation) -> WorkerProfile {
    WorkerProfile::new(
        WorkerCapability::new(id, vec![ResourceClass::Evaluate]),
        TrustDomain::new(domain),
        Locale::new(locale),
        attestation,
    )
}

fn request(tier: AccessTier, locale: &str, oracle_domain: &str) -> WorkRequest {
    WorkRequest {
        data_locale: Locale::new(locale),
        access_tier: tier,
        oracle_domain: TrustDomain::new(oracle_domain),
        input_bytes: 4_096,
    }
}

#[test]
fn a_worker_that_does_not_declare_the_resource_class_is_refused() {
    let mut profile = worker("w1", "site-a", "eu", attested());
    profile.capability = WorkerCapability::new("w1", vec![ResourceClass::Index]);
    let error = place(
        &job("j1"),
        &request(AccessTier::Open, "eu", "site-b"),
        &profile,
    )
    .expect_err("capability is declared, not inferred");
    assert!(matches!(error, PlacementError::ClassNotDeclared { .. }));
}

#[test]
fn an_unattested_worker_may_not_take_restricted_work() {
    let error = place(
        &job("j1"),
        &request(AccessTier::Restricted, "eu", "site-b"),
        &worker("w1", "site-a", "eu", Attestation::Unattested),
    )
    .expect_err("restricted inputs need a vouched worker");
    assert!(matches!(error, PlacementError::UnattestedWorker { .. }));
}

#[test]
fn an_unattested_worker_may_take_open_work() {
    let placement = place(
        &job("j1"),
        &request(AccessTier::Open, "eu", "site-b"),
        &worker("w1", "site-a", "eu", Attestation::Unattested),
    )
    .expect("open work needs no attestation");
    assert!(placement.data_local);
}

#[test]
fn a_worker_in_the_judging_oracles_trust_domain_is_refused() {
    let error = place(
        &job("j1"),
        &request(AccessTier::Open, "eu", "site-a"),
        &worker("w1", "site-a", "eu", attested()),
    )
    .expect_err("oracle independence is preserved in distributed and federated execution");
    assert!(matches!(
        error,
        PlacementError::OracleDomainCollision { .. }
    ));
}

#[test]
fn enclave_data_does_not_leave_its_locale() {
    let error = place(
        &job("j1"),
        &request(AccessTier::Enclave, "eu", "site-b"),
        &worker("w1", "site-a", "us", attested()),
    )
    .expect_err("enclave data may not be transferred");
    assert!(matches!(error, PlacementError::EnclaveTransfer { .. }));
}

#[test]
fn enclave_work_placed_where_its_data_lives_is_accepted() {
    let placement = place(
        &job("j1"),
        &request(AccessTier::Enclave, "eu", "site-b"),
        &worker("w1", "site-a", "eu", attested()),
    )
    .expect("a local enclave placement is fine");
    assert!(placement.data_local);
    assert_eq!(placement.transfer_bytes, 0);
}

#[test]
fn a_non_local_placement_succeeds_and_records_what_it_cost() {
    let placement = place(
        &job("j1"),
        &request(AccessTier::Restricted, "eu", "site-b"),
        &worker("w1", "site-a", "us", attested()),
    )
    .expect("data locality is a preference, not a safety rule");
    assert!(!placement.data_local);
    assert_eq!(
        placement.transfer_bytes, 4_096,
        "moving the data is allowed; moving it silently is not"
    );
}

#[test]
fn access_tiers_order_from_open_to_enclave() {
    assert!(!AccessTier::Open.requires_attestation());
    assert!(AccessTier::Restricted.requires_attestation());
    assert!(AccessTier::Enclave.requires_attestation());
    assert!(AccessTier::Open.permits_transfer());
    assert!(AccessTier::Restricted.permits_transfer());
    assert!(!AccessTier::Enclave.permits_transfer());
}

#[test]
fn an_attestation_has_no_state_between_vouched_for_and_unchecked() {
    assert!(attested().is_attested());
    assert!(!Attestation::Unattested.is_attested());
    let json = serde_json::to_string(&Attestation::Unattested).expect("serialisable");
    assert_eq!(json, r#"{"attestation":"unattested"}"#);
}

#[test]
fn the_current_fence_is_admitted_and_a_superseded_one_is_not() {
    let mut registry = FenceRegistry::new();
    let first = registry.issue("j1");
    assert!(registry.admit("j1", first).is_ok());

    let second = registry.issue("j1");
    assert!(registry.admit("j1", second).is_ok());
    let error = registry
        .admit("j1", first)
        .expect_err("a resurrected worker still holds the old token");
    assert_eq!(
        error,
        PlacementError::StaleFence {
            job: "j1".into(),
            presented: 1,
            current: 2
        }
    );
}

#[test]
fn fences_are_issued_per_job_and_do_not_interfere() {
    let mut registry = FenceRegistry::new();
    let first = registry.issue("j1");
    registry.issue("j2");
    registry.issue("j2");
    assert!(registry.admit("j1", first).is_ok());
    assert_eq!(registry.current("j1").map(|fence| fence.get()), Some(1));
    assert_eq!(registry.current("j2").map(|fence| fence.get()), Some(2));
}

#[test]
fn a_commit_for_a_job_that_was_never_fenced_is_refused() {
    let mut registry = FenceRegistry::new();
    let fence = registry.issue("j1");
    let mut ledger = ExecutionLedger::new();
    assert_eq!(
        ledger.commit(&registry, "j2", "item-1", fence, Idempotency::Idempotent),
        Err(PlacementError::NoFenceIssued("j2".into()))
    );
    assert!(ledger.commits().is_empty());
}

#[test]
fn a_commit_under_a_stale_fence_never_reaches_the_ledger() {
    let mut registry = FenceRegistry::new();
    let stale = registry.issue("j1");
    registry.issue("j1");
    let mut ledger = ExecutionLedger::new();
    assert!(ledger
        .commit(&registry, "j1", "item-1", stale, Idempotency::Idempotent)
        .is_err());
    assert!(
        ledger.executed_items().is_empty(),
        "a rejected write must not appear in the record of what ran"
    );
}

#[test]
fn a_repeated_non_idempotent_commit_is_an_incident_and_not_wasted_compute() {
    let mut registry = FenceRegistry::new();
    let mut ledger = ExecutionLedger::new();
    let fence = registry.issue("j1");
    ledger
        .commit(&registry, "j1", "item-1", fence, Idempotency::NonIdempotent)
        .expect("first commit");
    ledger
        .commit(&registry, "j1", "item-1", fence, Idempotency::NonIdempotent)
        .expect("the same fence still admits, which is what makes this a real hazard");

    let duplicates = ledger.duplicates();
    assert_eq!(duplicates.repeated_effect_incidents, 1);
    assert_eq!(duplicates.wasted_idempotent_commits, 0);
    assert!(duplicates.has_incidents());
    assert_eq!(duplicates.jobs_committed_more_than_once, vec!["j1"]);
}

#[test]
fn many_wasted_idempotent_commits_never_add_up_to_an_incident() {
    let mut registry = FenceRegistry::new();
    let mut ledger = ExecutionLedger::new();
    let fence = registry.issue("j1");
    for _ in 0..100 {
        ledger
            .commit(&registry, "j1", "item-1", fence, Idempotency::Idempotent)
            .expect("idempotent commits are safe to repeat");
    }
    let duplicates = ledger.duplicates();
    assert_eq!(duplicates.wasted_idempotent_commits, 99);
    assert_eq!(duplicates.repeated_effect_incidents, 0);
    assert!(
        !duplicates.has_incidents(),
        "a hundred wasted re-runs must never be summed with one double-applied effect"
    );
}

#[test]
fn a_repeated_compensable_commit_is_tracked_in_its_own_column() {
    let mut registry = FenceRegistry::new();
    let mut ledger = ExecutionLedger::new();
    let fence = registry.issue("j1");
    ledger
        .commit(&registry, "j1", "item-1", fence, Idempotency::Compensable)
        .expect("first");
    ledger
        .commit(&registry, "j1", "item-1", fence, Idempotency::Compensable)
        .expect("second");
    let duplicates = ledger.duplicates();
    assert_eq!(duplicates.compensable_repeat_commits, 1);
    assert_eq!(duplicates.wasted_idempotent_commits, 0);
    assert_eq!(duplicates.repeated_effect_incidents, 0);
    assert!(duplicates.has_incidents());
}

#[test]
fn the_ledger_reports_executed_items_once_each_in_id_order() {
    let mut registry = FenceRegistry::new();
    let mut ledger = ExecutionLedger::new();
    for (job, item) in [("j2", "item-b"), ("j1", "item-a"), ("j3", "item-a")] {
        let fence = registry.issue(job);
        ledger
            .commit(&registry, job, item, fence, Idempotency::Idempotent)
            .expect("committed");
    }
    assert_eq!(ledger.executed_items(), vec!["item-a", "item-b"]);
}
