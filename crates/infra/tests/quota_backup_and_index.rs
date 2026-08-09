//! A budget that cannot be duplicated, a restore that will not claim what it did not check, and
//! an index that answers with candidates rather than evidence.

use bioprism_infra::{
    BackupClass, BackupError, BackupSet, CandidateId, Epoch, Freshness, IndexError, Projection,
    Purpose, QuotaError, RestoreVerdict, StorageClass, StorageQuota,
};
use std::collections::BTreeSet;

/// Detects `Clone` at the type level, by autoref specialization.
///
/// `&&Probe<T>` reaches the `ViaClone` impl on `&Probe<T>` in one autoref step when `T: Clone`,
/// and falls through to the blanket `ViaFallback` impl on `Probe<T>` otherwise. This asserts the
/// property itself rather than the spelling of a derive, which a later refactor could restore
/// without anyone noticing.
struct Probe<T>(std::marker::PhantomData<T>);

trait ViaClone {
    fn is_clone(&self) -> bool {
        true
    }
}

impl<T: Clone> ViaClone for &Probe<T> {}

trait ViaFallback {
    fn is_clone(&self) -> bool {
        false
    }
}

impl<T> ViaFallback for Probe<T> {}

#[test]
fn a_storage_quota_cannot_be_duplicated_by_copying() {
    let quota: &&Probe<StorageQuota> = &&Probe(std::marker::PhantomData);
    assert!(
        !quota.is_clone(),
        "copying a storage quota would authorize two subsystems to fill the same disk"
    );

    let report: &&Probe<bioprism_infra::RestoreReport> = &&Probe(std::marker::PhantomData);
    assert!(
        report.is_clone(),
        "a report is evidence about a budget, not a claim on one, and may be copied freely"
    );
}

#[test]
fn a_storage_quota_can_still_be_reported_even_though_it_cannot_be_copied() {
    fn assert_serializable<T: serde::Serialize>() {}
    assert_serializable::<StorageQuota>();
}

#[test]
fn a_charge_within_the_ordinary_allowance_succeeds_and_reports_what_is_left() {
    let mut quota = StorageQuota::new(1_000, 200).expect("reserve is smaller than the limit");
    let remaining = quota
        .charge(StorageClass::Objects, Purpose::Ingest, 300)
        .expect("well within the limit");
    assert_eq!(remaining, 500);
    assert_eq!(quota.used(), 300);
    assert_eq!(quota.remaining(), 700);
}

#[test]
fn ordinary_work_stops_at_the_reserve_rather_than_at_the_limit() {
    let mut quota = StorageQuota::new(1_000, 200).expect("quota");
    let error = quota
        .charge(StorageClass::Objects, Purpose::Ingest, 900)
        .expect_err("900 crosses into the 200-byte reserve");
    assert_eq!(
        error,
        QuotaError::ReserveIsProtected {
            purpose: "ingest",
            requested: 900,
            reserve: 200,
        }
    );
    assert_eq!(quota.used(), 0, "a refused charge charges nothing");
}

#[test]
fn cleanup_and_evidence_finalization_may_draw_on_the_reserve() {
    let mut quota = StorageQuota::new(1_000, 200).expect("quota");
    quota
        .charge(StorageClass::Results, Purpose::EvidenceFinalization, 900)
        .expect("finalizing evidence may use the reserve");
    assert_eq!(quota.used(), 900);
    assert!(Purpose::Cleanup.may_use_reserve());
    assert!(!Purpose::Ingest.may_use_reserve());
}

#[test]
fn crossing_the_hard_limit_is_a_different_error_from_crossing_into_the_reserve() {
    let mut quota = StorageQuota::new(1_000, 200).expect("quota");
    let error = quota
        .charge(StorageClass::Cache, Purpose::Cleanup, 1_001)
        .expect_err("past the hard limit");
    assert_eq!(
        error,
        QuotaError::Exceeded {
            class: "cache",
            requested: 1_001,
            available: 1_000,
            limit: 1_000,
        }
    );
}

#[test]
fn releasing_more_than_was_charged_is_refused_so_allowance_cannot_be_minted() {
    let mut quota = StorageQuota::new(1_000, 100).expect("quota");
    quota
        .charge(StorageClass::Objects, Purpose::Ingest, 50)
        .expect("charge");
    let error = quota
        .release(StorageClass::Objects, 80)
        .expect_err("only 50 are charged");
    assert_eq!(
        error,
        QuotaError::ReleaseExceedsCharge {
            class: "objects",
            requested: 80,
            charged: 50,
        }
    );
}

#[test]
fn delegation_subtracts_from_the_parent_so_the_two_always_sum_to_the_original() {
    let mut parent = StorageQuota::new(1_000, 100).expect("quota");
    let child = parent.delegate(400).expect("400 are unspent");
    assert_eq!(parent.limit(), 600);
    assert_eq!(child.limit(), 400);
    assert_eq!(parent.limit() + child.limit(), 1_000);
    assert_eq!(
        child.reserve(),
        0,
        "a delegate does not inherit the reserve"
    );
}

#[test]
fn a_delegation_larger_than_the_unspent_allowance_is_refused() {
    let mut parent = StorageQuota::new(1_000, 100).expect("quota");
    parent
        .charge(StorageClass::Objects, Purpose::Ingest, 700)
        .expect("charge");
    let error = parent.delegate(400).expect_err("only 300 are unspent");
    assert_eq!(
        error,
        QuotaError::DelegationExceedsRemaining {
            requested: 400,
            available: 300,
        }
    );
}

#[test]
fn absorbing_a_delegate_returns_its_allowance_and_its_realized_usage() {
    let mut parent = StorageQuota::new(1_000, 100).expect("quota");
    let mut child = parent.delegate(400).expect("delegate");
    child
        .charge(StorageClass::Cache, Purpose::Ingest, 250)
        .expect("charge");
    parent.absorb(child);

    assert_eq!(parent.limit(), 1_000);
    assert_eq!(parent.used(), 250);
    assert_eq!(
        parent.charged_by_class().get(&StorageClass::Cache),
        Some(&250)
    );
}

#[test]
fn a_reserve_that_is_not_smaller_than_the_limit_is_refused() {
    let error = StorageQuota::new(100, 100).expect_err("no ordinary allowance would remain");
    assert_eq!(
        error,
        QuotaError::ReserveExceedsLimit {
            reserve: 100,
            limit: 100
        }
    );
}

#[test]
fn a_quota_serializes_for_reporting_because_reporting_is_not_duplicating() {
    let mut quota = StorageQuota::new(1_000, 100).expect("quota");
    quota
        .charge(StorageClass::Events, Purpose::Ingest, 40)
        .expect("charge");
    let text = serde_json::to_string(&quota).expect("a quota can be reported");
    assert!(text.contains("1000"));
    assert!(text.contains("Events"));
}

#[test]
fn the_classes_that_can_be_rebuilt_are_named_so_a_planner_sheds_those_first() {
    assert!(StorageClass::Indexes.is_reconstructible());
    assert!(StorageClass::Cache.is_reconstructible());
    assert!(!StorageClass::Events.is_reconstructible());
    assert!(!StorageClass::Objects.is_reconstructible());
}

#[test]
fn a_restore_that_runs_classes_out_of_order_is_refused_rather_than_silently_sorted() {
    let backup = BackupSet::new();
    let error = backup
        .restore(&[BackupClass::Artifacts, BackupClass::TrustRoots])
        .expect_err("trust roots come first");
    assert_eq!(
        error,
        BackupError::OutOfOrder {
            earlier: "trust-roots",
            later: "artifacts",
        }
    );
}

#[test]
fn a_complete_restore_with_captured_content_is_verified() {
    let backup = BackupSet::new()
        .with_item(BackupClass::TrustRoots, "key-1", "d1", true)
        .expect("item")
        .with_item(BackupClass::Catalog, "catalog", "d2", true)
        .expect("item")
        .with_item(BackupClass::EventLog, "events", "d3", true)
        .expect("item")
        .with_item(BackupClass::Artifacts, "bundle-1", "d4", true)
        .expect("item")
        .with_item(BackupClass::Projections, "search", "d5", true)
        .expect("item")
        .requiring("bundle-1", ["catalog"]);

    let report = backup.restore(&BackupClass::ALL).expect("in order");
    assert_eq!(report.verdict, RestoreVerdict::Verified { objects: 5 });
    assert!(report.verdict.is_verified());
    assert_eq!(report.total_restored(), 5);
}

#[test]
fn a_restore_whose_manifest_names_a_missing_child_reports_a_broken_closure() {
    let backup = BackupSet::new()
        .with_item(BackupClass::Artifacts, "bundle-1", "d1", true)
        .expect("item")
        .requiring("bundle-1", ["child-that-was-not-backed-up"]);

    let report = backup
        .restore(&[BackupClass::Artifacts])
        .expect("order is fine");
    match &report.verdict {
        RestoreVerdict::ClosureBroken { manifests } => assert!(manifests.contains("bundle-1")),
        other => panic!("expected a broken closure, got {other:?}"),
    }
    assert!(report.missing_children["bundle-1"].contains("child-that-was-not-backed-up"));
}

#[test]
fn a_backup_of_digests_without_content_restores_unverified_rather_than_verified() {
    let backup = BackupSet::new()
        .with_item(BackupClass::TrustRoots, "key-1", "d1", true)
        .expect("item")
        .with_item(BackupClass::Catalog, "catalog", "d2", true)
        .expect("item")
        .with_item(BackupClass::EventLog, "events", "d3", true)
        .expect("item")
        .with_item(BackupClass::Artifacts, "bundle-1", "d4", false)
        .expect("item")
        .with_item(BackupClass::Projections, "search", "d5", true)
        .expect("item");

    let report = backup.restore(&BackupClass::ALL).expect("in order");
    match &report.verdict {
        RestoreVerdict::Unverified { items } => assert!(items.contains("bundle-1")),
        other => panic!("a pointer is not a backup; got {other:?}"),
    }
    assert!(report.unverifiable.contains("bundle-1"));
}

#[test]
fn skipping_a_rebuildable_class_is_a_task_while_skipping_evidence_is_an_incident() {
    let backup = BackupSet::new()
        .with_item(BackupClass::TrustRoots, "key-1", "d1", true)
        .expect("item")
        .with_item(BackupClass::Catalog, "catalog", "d2", true)
        .expect("item")
        .with_item(BackupClass::EventLog, "events", "d3", true)
        .expect("item")
        .with_item(BackupClass::Artifacts, "bundle-1", "d4", true)
        .expect("item");

    let report = backup
        .restore(&[
            BackupClass::TrustRoots,
            BackupClass::Catalog,
            BackupClass::EventLog,
            BackupClass::Artifacts,
        ])
        .expect("in order");
    assert!(report.rebuild_required.contains(&BackupClass::Projections));
    assert!(report.evidence_not_restored.is_empty());
    assert!(report.verdict.is_verified());
}

#[test]
fn a_restore_that_omits_an_irreproducible_class_is_not_verified() {
    let backup = BackupSet::new()
        .with_item(BackupClass::TrustRoots, "key-1", "d1", true)
        .expect("item");
    let report = backup
        .restore(&[BackupClass::TrustRoots])
        .expect("in order");
    assert!(!report.verdict.is_verified());
    assert!(report
        .evidence_not_restored
        .contains(&BackupClass::Artifacts));
}

#[test]
fn the_restore_order_follows_the_blueprint_sequence() {
    let mut order: Vec<BackupClass> = BackupClass::ALL.to_vec();
    order.sort_by_key(|class| class.restore_order());
    assert_eq!(order, BackupClass::ALL.to_vec());
    assert!(BackupClass::Projections.is_rebuildable());
    assert!(!BackupClass::Artifacts.is_rebuildable());
}

fn candidates(names: &[&str]) -> BTreeSet<CandidateId> {
    names
        .iter()
        .map(|name| CandidateId::parse(*name).expect("candidate"))
        .collect()
}

#[test]
fn an_index_answer_offers_places_to_look_and_no_way_to_read_a_value_from_it() {
    let mut projection = Projection::new("failures").expect("projection");
    projection
        .rebuild(
            [("kras".to_string(), candidates(&["fact-1", "fact-2"]))],
            Epoch::new(5),
        )
        .expect("rebuild");

    let answer = projection.query("kras", Some(Epoch::new(5)));
    assert_eq!(answer.candidates(), &candidates(&["fact-1", "fact-2"]));
    assert_eq!(answer.revision, 1);
    assert!(answer.freshness.is_up_to_date());
}

#[test]
fn every_answer_reports_the_revision_and_the_freshness_of_the_projection() {
    let mut projection = Projection::new("failures").expect("projection");
    projection
        .rebuild(
            [("kras".to_string(), candidates(&["fact-1"]))],
            Epoch::new(5),
        )
        .expect("rebuild");

    let answer = projection.query("kras", Some(Epoch::new(12)));
    assert_eq!(
        answer.freshness,
        Freshness::StaleBy {
            epochs: 7,
            through: Epoch::new(5)
        }
    );
}

#[test]
fn a_caller_who_cannot_state_the_canonical_epoch_gets_unknown_rather_than_an_optimistic_default() {
    let mut projection = Projection::new("failures").expect("projection");
    projection
        .rebuild(
            [("kras".to_string(), candidates(&["fact-1"]))],
            Epoch::new(5),
        )
        .expect("rebuild");

    let answer = projection.query("kras", None);
    assert!(!answer.freshness.is_known());
    assert!(!answer.freshness.is_up_to_date());
    assert_eq!(answer.freshness.name(), "unknown");
}

#[test]
fn a_rebuild_replaces_the_postings_so_a_deleted_entry_cannot_outlive_its_deletion() {
    let mut projection = Projection::new("failures").expect("projection");
    projection
        .rebuild(
            [("kras".to_string(), candidates(&["fact-1", "fact-2"]))],
            Epoch::new(1),
        )
        .expect("rebuild");
    projection
        .rebuild(
            [("kras".to_string(), candidates(&["fact-1"]))],
            Epoch::new(2),
        )
        .expect("rebuild");

    let answer = projection.query("kras", Some(Epoch::new(2)));
    assert_eq!(answer.candidates(), &candidates(&["fact-1"]));
    assert_eq!(answer.revision, 2);
}

#[test]
fn a_rebuild_covering_an_earlier_epoch_than_the_current_one_is_refused() {
    let mut projection = Projection::new("failures").expect("projection");
    projection
        .rebuild([("a".to_string(), candidates(&["x"]))], Epoch::new(9))
        .expect("rebuild");
    let error = projection
        .rebuild([("a".to_string(), candidates(&["x"]))], Epoch::new(4))
        .expect_err("coverage would go backwards while the revision went forwards");
    assert_eq!(
        error,
        IndexError::RebuildGoesBackwards {
            index: "failures".to_string(),
            through: Epoch::new(4),
            existing: Epoch::new(9),
        }
    );
}

#[test]
fn an_unknown_term_answers_with_no_candidates_and_still_reports_its_freshness() {
    let mut projection = Projection::new("failures").expect("projection");
    projection
        .rebuild([("a".to_string(), candidates(&["x"]))], Epoch::new(3))
        .expect("rebuild");
    let answer = projection.query("absent", Some(Epoch::new(3)));
    assert!(answer.is_empty());
    assert!(answer.freshness.is_up_to_date());
}

#[test]
fn an_epoch_never_reads_a_clock_and_reports_an_inverted_interval_as_absent() {
    assert_eq!(Epoch::new(9).elapsed_since(Epoch::new(4)), Some(5));
    assert_eq!(Epoch::new(4).elapsed_since(Epoch::new(9)), None);
    assert_eq!(Epoch::ZERO.next(), Epoch::new(1));
    assert_eq!(Epoch::new(3).to_string(), "e3");
}
