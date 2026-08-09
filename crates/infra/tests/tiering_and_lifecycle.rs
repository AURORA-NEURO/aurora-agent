//! Tiering plans before it acts, and deletion leaves a tombstone rather than rewriting history.

use bioprism_ids::ContentHash;
use bioprism_infra::{
    AccessRecord, Classification, DeletionBasis, Durability, Epoch, GcPolicy, Lifecycle,
    LifecycleError, LocalLayout, ObjectId, ObjectRecord, Residency, RetentionWindow, StorageArea,
    Tier, TieringPolicy,
};
use bioprism_ledger::RecordTime;

fn digest(seed: &str) -> ContentHash {
    ContentHash::of_bytes(seed.as_bytes())
}

fn record(id: &str, created: &str, bytes: u64) -> ObjectRecord {
    ObjectRecord {
        id: ObjectId::parse(id).expect("object id"),
        digest: digest(id),
        bytes,
        classification: Classification::Internal,
        residency: Residency::parse("eu-west").expect("residency"),
        created: RecordTime::parse(created).expect("record time"),
        pinned: false,
    }
}

fn window_from(record_time: &str) -> RetentionWindow {
    RetentionWindow {
        answerable_from_record: Some(RecordTime::parse(record_time).expect("record time")),
        ..RetentionWindow::unrestricted()
    }
}

#[test]
fn a_policy_whose_cold_threshold_is_not_after_its_warm_threshold_is_refused() {
    let error = TieringPolicy::new(10, 10, 3, 2).expect_err("cold must be strictly later");
    assert_eq!(
        error,
        LifecycleError::IncoherentTieringPolicy { warm: 10, cold: 10 }
    );
}

#[test]
fn an_idle_object_is_demoted_and_the_transition_states_the_threshold_it_crossed() {
    let policy = TieringPolicy::new(5, 20, 3, 2).expect("policy");
    let records = vec![AccessRecord::new("obj-1", Tier::Hot, Epoch::new(0))];
    let plan = policy.plan(&records, Epoch::new(7)).expect("plan");

    assert_eq!(plan.len(), 1);
    let transition = &plan.transitions[0];
    assert_eq!(transition.from, Tier::Hot);
    assert_eq!(transition.to, Tier::Warm);
    assert!(!transition.skipped_a_tier);
}

#[test]
fn an_object_idle_past_the_cold_threshold_skips_a_tier_and_says_so() {
    let policy = TieringPolicy::new(5, 20, 3, 2).expect("policy");
    let records = vec![AccessRecord::new("obj-1", Tier::Hot, Epoch::new(0))];
    let plan = policy.plan(&records, Epoch::new(40)).expect("plan");

    let transition = &plan.transitions[0];
    assert_eq!(transition.to, Tier::Cold);
    assert!(transition.skipped_a_tier);
}

#[test]
fn a_pinned_object_is_never_demoted_below_warm() {
    let policy = TieringPolicy::new(5, 20, 3, 2).expect("policy");
    let records = vec![AccessRecord::new("obj-1", Tier::Hot, Epoch::new(0)).pinned()];
    let plan = policy.plan(&records, Epoch::new(100)).expect("plan");

    assert_eq!(plan.transitions[0].to, Tier::Warm);
}

#[test]
fn promotion_needs_both_enough_accesses_and_enough_recency() {
    let policy = TieringPolicy::new(5, 20, 3, 2).expect("policy");

    let busy_but_stale =
        vec![AccessRecord::new("obj-1", Tier::Cold, Epoch::new(0)).with_recent_accesses(50)];
    let plan = policy.plan(&busy_but_stale, Epoch::new(30)).expect("plan");
    assert!(
        plan.transitions.iter().all(|t| t.to != Tier::Hot),
        "a count alone must not promote an object nobody has touched"
    );

    let recent_but_quiet =
        vec![AccessRecord::new("obj-2", Tier::Cold, Epoch::new(29)).with_recent_accesses(1)];
    let plan = policy
        .plan(&recent_but_quiet, Epoch::new(30))
        .expect("plan");
    assert!(
        plan.transitions.iter().all(|t| t.to != Tier::Hot),
        "recency alone must not promote an object touched once"
    );

    let busy_and_recent =
        vec![AccessRecord::new("obj-3", Tier::Cold, Epoch::new(29)).with_recent_accesses(9)];
    let plan = policy.plan(&busy_and_recent, Epoch::new(30)).expect("plan");
    assert_eq!(plan.transitions[0].to, Tier::Hot);
}

#[test]
fn an_access_after_the_planning_epoch_is_refused_rather_than_read_as_zero_idle() {
    let policy = TieringPolicy::new(5, 20, 3, 2).expect("policy");
    let records = vec![AccessRecord::new("obj-1", Tier::Hot, Epoch::new(11))];
    let error = policy
        .plan(&records, Epoch::new(10))
        .expect_err("the caller's state is inconsistent");
    assert_eq!(
        error,
        LifecycleError::AccessInFuture {
            object: "obj-1".to_string(),
            last_access: Epoch::new(11),
            now: Epoch::new(10),
        }
    );
}

#[test]
fn planning_changes_nothing_until_the_plan_is_applied() {
    let policy = TieringPolicy::new(5, 20, 3, 2).expect("policy");
    let mut records = vec![AccessRecord::new("obj-1", Tier::Hot, Epoch::new(0))];
    let plan = policy.plan(&records, Epoch::new(7)).expect("plan");
    assert_eq!(records[0].tier, Tier::Hot);

    let (applied, absent) = plan.apply_to(&mut records);
    assert_eq!((applied, absent), (1, 0));
    assert_eq!(records[0].tier, Tier::Warm);
}

#[test]
fn a_plan_applied_to_a_population_that_moved_reports_the_transitions_it_could_not_perform() {
    let policy = TieringPolicy::new(5, 20, 3, 2).expect("policy");
    let records = vec![AccessRecord::new("obj-1", Tier::Hot, Epoch::new(0))];
    let plan = policy.plan(&records, Epoch::new(7)).expect("plan");

    let mut moved: Vec<AccessRecord> = Vec::new();
    let (applied, absent) = plan.apply_to(&mut moved);
    assert_eq!((applied, absent), (0, 1));
}

#[test]
fn the_layout_fans_object_paths_out_two_levels_under_the_digest() {
    let layout = LocalLayout::new(".bioprism").expect("layout");
    let hash = digest("obj-1");
    let path = layout.object_path(&hash);
    let text = hash.as_str();
    assert_eq!(
        path,
        format!(
            ".bioprism/objects/sha256/{}/{}/{}",
            &text[0..2],
            &text[2..4],
            &text[4..]
        )
    );
}

#[test]
fn every_area_of_the_layout_states_whether_losing_it_costs_evidence_or_only_time() {
    assert_eq!(StorageArea::Events.durability(), Durability::Canonical);
    assert_eq!(StorageArea::Indexes.durability(), Durability::Rebuildable);
    assert_eq!(StorageArea::Workspaces.durability(), Durability::Ephemeral);
    let canonical = LocalLayout::canonical_areas();
    assert!(canonical.contains(&StorageArea::Objects));
    assert!(!canonical.contains(&StorageArea::Analytics));
}

#[test]
fn storage_cannot_reclaim_while_the_ledger_has_never_compacted() {
    let mut lifecycle = Lifecycle::under(RetentionWindow::unrestricted());
    let id = lifecycle.register(record("obj-1", "2021-01-01T00:00:00Z", 100));

    let error = lifecycle
        .admits_reclamation(&id)
        .expect_err("an uncompacted ledger still answers everything");
    assert_eq!(
        error,
        LifecycleError::RetentionWindowUnrestricted {
            object: "obj-1".to_string()
        }
    );
}

#[test]
fn storage_cannot_reclaim_an_object_the_ledger_still_promises_to_answer_about() {
    let mut lifecycle = Lifecycle::under(window_from("2021-01-01T00:00:00Z"));
    let id = lifecycle.register(record("obj-1", "2021-06-01T00:00:00Z", 100));

    let error = lifecycle
        .admits_reclamation(&id)
        .expect_err("created inside the retained window");
    assert!(matches!(error, LifecycleError::WithinRetainedWindow { .. }));
}

#[test]
fn an_object_behind_the_compaction_boundary_may_be_reclaimed() {
    let mut lifecycle = Lifecycle::under(window_from("2021-06-01T00:00:00Z"));
    let id = lifecycle.register(record("obj-1", "2020-01-01T00:00:00Z", 100));
    assert!(lifecycle.admits_reclamation(&id).is_ok());
}

#[test]
fn adopting_a_window_takes_the_more_restrictive_of_the_two() {
    let mut lifecycle = Lifecycle::under(window_from("2021-06-01T00:00:00Z"));
    lifecycle.adopt_window(window_from("2020-01-01T00:00:00Z"));
    assert_eq!(
        lifecycle.window().answerable_from_record,
        Some(RecordTime::parse("2021-06-01T00:00:00Z").expect("record time")),
        "a ledger cannot recover history it already destroyed"
    );
}

#[test]
fn deleting_an_object_leaves_a_tombstone_and_a_later_resolve_returns_it_rather_than_not_found() {
    let mut lifecycle = Lifecycle::under(window_from("2021-06-01T00:00:00Z"));
    let id = lifecycle.register(record("obj-1", "2020-01-01T00:00:00Z", 100));

    let tombstone = lifecycle
        .delete(&id, DeletionBasis::Reclaim, "space", Epoch::new(3))
        .expect("outside the window and unreferenced");
    assert_eq!(tombstone.digest, digest("obj-1"));
    assert_eq!(tombstone.bytes_reclaimed, 100);

    let error = lifecycle.resolve(&id).expect_err("the object is gone");
    match error {
        LifecycleError::Tombstoned { at, reason, .. } => {
            assert_eq!(at, Epoch::new(3));
            assert_eq!(reason, "space");
        }
        other => panic!("expected a tombstone, got {other:?}"),
    }
}

#[test]
fn an_object_a_manifest_still_names_cannot_be_reclaimed() {
    let mut lifecycle = Lifecycle::under(window_from("2021-06-01T00:00:00Z"));
    let id = lifecycle.register(record("obj-1", "2020-01-01T00:00:00Z", 100));
    lifecycle
        .register_manifest("result-bundle-9", ["obj-1"])
        .expect("manifest");

    let error = lifecycle
        .delete(&id, DeletionBasis::Reclaim, "space", Epoch::new(3))
        .expect_err("a published result names it");
    assert_eq!(
        error,
        LifecycleError::StillReferenced {
            object: "obj-1".to_string(),
            by: "result-bundle-9".to_string()
        }
    );
}

#[test]
fn a_lawful_deletion_proceeds_inside_the_window_and_records_the_manifests_it_affects() {
    let mut lifecycle = Lifecycle::under(RetentionWindow::unrestricted());
    let id = lifecycle.register(record("obj-1", "2021-06-01T00:00:00Z", 100));
    lifecycle
        .register_manifest("result-bundle-9", ["obj-1"])
        .expect("manifest");

    let tombstone = lifecycle
        .delete(
            &id,
            DeletionBasis::Lawful,
            "erasure request",
            Epoch::new(11),
        )
        .expect("a compelled deletion is not subject to the retained window");
    assert_eq!(tombstone.basis, DeletionBasis::Lawful);
    assert!(tombstone.still_referenced_by.contains("result-bundle-9"));
}

#[test]
fn a_tombstone_carries_the_digest_and_the_classification_but_not_the_content() {
    let mut lifecycle = Lifecycle::under(RetentionWindow::unrestricted());
    let id = lifecycle.register(record("obj-1", "2021-06-01T00:00:00Z", 100));
    let tombstone = lifecycle
        .delete(&id, DeletionBasis::Lawful, "erasure", Epoch::new(1))
        .expect("lawful");
    let text = serde_json::to_string(&tombstone).expect("tombstone serializes");
    assert!(text.contains(tombstone.digest.as_str()));
    assert_eq!(tombstone.classification, Classification::Internal);
    assert_eq!(tombstone.residency.as_str(), "eu-west");
}

#[test]
fn garbage_collection_is_a_dry_run_by_default_and_changes_nothing() {
    let mut lifecycle = Lifecycle::under(window_from("2021-06-01T00:00:00Z"));
    lifecycle.register(record("obj-1", "2020-01-01T00:00:00Z", 100));
    lifecycle.register(record("obj-2", "2020-01-01T00:00:00Z", 200));
    lifecycle
        .register_manifest("root", ["obj-1"])
        .expect("manifest");

    let policy = GcPolicy::from_roots(["root"]);
    assert!(policy.dry_run);
    let report = lifecycle.garbage_collect(&policy);
    assert!(report
        .swept
        .contains(&ObjectId::parse("obj-2").expect("id")));
    assert_eq!(lifecycle.len(), 2, "a dry run destroys nothing");
}

#[test]
fn garbage_collection_states_why_each_survivor_survived() {
    let mut lifecycle = Lifecycle::under(window_from("2021-06-01T00:00:00Z"));
    lifecycle.register(record("root-member", "2020-01-01T00:00:00Z", 10));
    lifecycle.register(record("child-member", "2020-01-01T00:00:00Z", 10));
    let mut pinned = record("pinned", "2020-01-01T00:00:00Z", 10);
    pinned.pinned = true;
    lifecycle.register(pinned);
    lifecycle.register(record("in-window", "2022-01-01T00:00:00Z", 10));
    lifecycle.register(record("orphan", "2020-01-01T00:00:00Z", 10));

    lifecycle
        .register_manifest("root", ["root-member", "child"])
        .expect("manifest");
    lifecycle
        .register_manifest("child", ["child-member"])
        .expect("manifest");

    let report = lifecycle.garbage_collect(&GcPolicy::from_roots(["root"]).applying());

    assert!(report
        .retained_by_root
        .contains(&ObjectId::parse("root-member").expect("id")));
    assert!(report
        .retained_by_closure
        .contains(&ObjectId::parse("child-member").expect("id")));
    assert!(report
        .retained_by_pin
        .contains(&ObjectId::parse("pinned").expect("id")));
    assert!(report
        .retained_by_retention_window
        .contains(&ObjectId::parse("in-window").expect("id")));
    assert_eq!(
        report.swept,
        [ObjectId::parse("orphan").expect("id")]
            .into_iter()
            .collect()
    );
    assert_eq!(report.bytes_swept, 10);
    assert_eq!(lifecycle.len(), 4);
}

#[test]
fn a_manifest_naming_an_object_nobody_holds_is_reported_rather_than_credited_as_a_clean_closure() {
    let mut lifecycle = Lifecycle::under(window_from("2021-06-01T00:00:00Z"));
    lifecycle.register(record("obj-1", "2020-01-01T00:00:00Z", 10));
    lifecycle
        .register_manifest("root", ["obj-1", "vanished"])
        .expect("manifest");

    let report = lifecycle.garbage_collect(&GcPolicy::from_roots(["root"]));
    assert!(!report.closure_intact());
    assert!(report.dangling_references["root"].contains("vanished"));
}

#[test]
fn a_tombstoned_object_does_not_count_as_a_broken_reference() {
    let mut lifecycle = Lifecycle::under(RetentionWindow::unrestricted());
    let id = lifecycle.register(record("obj-1", "2021-06-01T00:00:00Z", 10));
    lifecycle
        .register_manifest("root", ["obj-1"])
        .expect("manifest");
    lifecycle
        .delete(&id, DeletionBasis::Lawful, "erasure", Epoch::new(1))
        .expect("lawful");

    let report = lifecycle.garbage_collect(&GcPolicy::from_roots(["root"]));
    assert!(
        report.closure_intact(),
        "the reference resolves to a tombstone, which is an answer"
    );
}

#[test]
fn a_public_classification_is_the_only_one_that_permits_global_deduplication() {
    assert!(Classification::Public.permits_global_deduplication());
    assert!(!Classification::Internal.permits_global_deduplication());
    assert!(!Classification::Controlled.permits_global_deduplication());
}

#[test]
fn a_blank_object_id_residency_or_reason_is_refused() {
    assert!(ObjectId::parse(" ").is_err());
    assert!(Residency::parse("").is_err());
    let mut lifecycle = Lifecycle::under(RetentionWindow::unrestricted());
    let id = lifecycle.register(record("obj-1", "2021-06-01T00:00:00Z", 1));
    assert!(lifecycle
        .delete(&id, DeletionBasis::Lawful, "  ", Epoch::ZERO)
        .is_err());
}
