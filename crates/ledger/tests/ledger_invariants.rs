//! Invariant tests for the infrastructure event ledger.
//!
//! Every test name states the claim it proves. The five headings mirror 40.09's verification
//! plan — time-travel replay, duplicate append, correction and supersession, projection rebuild
//! equality, causal cycle rejection — plus the lifecycle requirements 40.06 and 12.22 add.

use bioprism_ids::EventId;
use bioprism_ledger::{
    Actor, ChainStatus, ClassCounts, Event, EventClass, EventKind, EventLedger, EventTimes,
    LedgerError, Projection, RecordTime, ReleaseTime, RetentionPolicy, SchemaCatalog, SubjectKey,
    SubjectLatest, TemporalCut, ValidTime,
};
use serde_json::{json, Value};
use std::collections::BTreeMap;

fn valid(day: &str) -> ValidTime {
    ValidTime::parse(&format!("2021-{day}T00:00:00Z")).expect("fixture instant")
}

fn record(day: &str) -> RecordTime {
    RecordTime::parse(&format!("2021-{day}T00:00:00Z")).expect("fixture instant")
}

fn release(day: &str) -> ReleaseTime {
    ReleaseTime::parse(&format!("2021-{day}T00:00:00Z")).expect("fixture instant")
}

fn curator() -> Actor {
    Actor::new("registry-core", "curator").expect("fixture actor")
}

fn evt(seq: u64) -> EventId {
    EventId::parse(format!("evt-{seq:012}")).expect("generated id shape")
}

fn observation(subject: &str, valid_day: &str, record_day: &str, payload: Value) -> Event {
    Event::new(
        EventClass::Material,
        EventKind::parse("lesion.measured").expect("fixture kind"),
        curator(),
        SubjectKey::parse(subject).expect("fixture subject"),
        EventTimes::published_on_record(valid(valid_day), record(record_day)),
        payload,
    )
    .expect("fixture event")
}

fn embargoed(
    subject: &str,
    valid_day: &str,
    record_day: &str,
    release_day: &str,
    payload: Value,
) -> Event {
    Event::new(
        EventClass::Evaluation,
        EventKind::parse("cohort.scored").expect("fixture kind"),
        curator(),
        SubjectKey::parse(subject).expect("fixture subject"),
        EventTimes::new(valid(valid_day), record(record_day), release(release_day))
            .expect("fixture times"),
        payload,
    )
    .expect("fixture event")
}

fn subject(name: &str) -> SubjectKey {
    SubjectKey::parse(name).expect("fixture subject")
}

fn record_once(ledger: &mut EventLedger, event: Event) -> EventId {
    ledger
        .append(event)
        .expect("append succeeds")
        .admission
        .recorded_id()
        .expect("event was recorded, not quarantined")
        .clone()
}

fn state_ids(ledger: &EventLedger, cut: &TemporalCut) -> BTreeMap<SubjectKey, EventId> {
    ledger
        .latest_by_subject(cut)
        .expect("cut is inside the retained window")
        .into_iter()
        .map(|(key, entry)| (key, entry.id.clone()))
        .collect()
}

/// Twenty entries about four subjects, one per day through January.
///
/// Several entries per subject inside any plausible compaction bound, which is what makes the
/// retention tests test something: a log with one entry per subject would be entirely
/// carry-forward and nothing would ever be removed.
fn busy_ledger() -> EventLedger {
    let mut ledger = EventLedger::new();
    for index in 0..20u32 {
        let day = format!("01-{:02}", index + 1);
        ledger
            .append(observation(
                &format!("patient-{}", index % 4),
                &day,
                &day,
                json!({ "diameter_mm": index }),
            ))
            .expect("append succeeds");
    }
    ledger
}

// --- Three time axes -------------------------------------------------------------------------

#[test]
fn a_release_that_precedes_its_record_is_refused() {
    let refusal = EventTimes::new(valid("01-01"), record("03-01"), release("02-01"))
        .expect_err("publishing before learning is impossible");
    assert!(matches!(refusal, LedgerError::ReleaseBeforeRecord { .. }));
}

#[test]
fn valid_time_may_precede_or_follow_record_time_because_backfills_and_forecasts_are_both_real() {
    EventTimes::new(valid("01-01"), record("06-01"), release("06-01")).expect("backfill");
    EventTimes::new(valid("12-01"), record("06-01"), release("06-01")).expect("forecast");
}

#[test]
fn a_temporal_cut_admits_an_event_only_when_every_bound_it_sets_is_satisfied() {
    let times = EventTimes::new(valid("03-01"), record("04-01"), release("05-01")).expect("times");
    assert!(TemporalCut::EVERYTHING.admits(&times));
    assert!(TemporalCut::known_at(record("04-01")).admits(&times));
    assert!(!TemporalCut::known_at(record("03-31")).admits(&times));
    assert!(!TemporalCut::readable_at(release("04-15")).admits(&times));
    assert!(!TemporalCut::believed_at(valid("02-01"), record("06-01")).admits(&times));
}

// --- Append, chaining and tamper evidence ----------------------------------------------------

#[test]
fn an_empty_ledger_has_an_intact_chain_and_no_head() {
    let ledger = EventLedger::new();
    assert!(ledger.is_empty());
    assert_eq!(ledger.head(), "");
    assert_eq!(ledger.verify_chain(), ChainStatus::Intact);
}

#[test]
fn the_chain_verifies_over_a_non_trivial_log() {
    let ledger = busy_ledger();
    assert_eq!(ledger.len(), 20);
    assert_eq!(ledger.verify_chain(), ChainStatus::Intact);
    assert_eq!(ledger.verify_causal_acyclicity(), ChainStatus::Intact);
}

#[test]
fn editing_a_recorded_entry_invalidates_its_digest() {
    let ledger = busy_ledger();
    let mut forged = ledger.entries()[4].clone();
    forged.event.subject = subject("patient-somebody-else");
    assert_ne!(forged.recompute_digest(), forged.digest);
}

#[test]
fn every_entry_chains_to_the_digest_of_the_one_before_it() {
    let ledger = busy_ledger();
    for pair in ledger.entries().windows(2) {
        assert_eq!(pair[1].previous, pair[0].digest);
    }
    assert_eq!(ledger.head(), ledger.entries()[19].digest);
}

#[test]
fn sequence_numbers_are_monotonic_and_ids_are_derived_from_them() {
    let ledger = busy_ledger();
    for (index, entry) in ledger.entries().iter().enumerate() {
        assert_eq!(entry.seq, index as u64);
        assert_eq!(entry.id, evt(index as u64));
    }
    assert_eq!(ledger.next_seq(), 20);
}

// --- Idempotency -----------------------------------------------------------------------------

#[test]
fn the_same_event_submitted_twice_is_recorded_once() {
    let mut ledger = EventLedger::new();
    let first = ledger
        .append(observation(
            "patient-1",
            "01-01",
            "01-02",
            json!({ "n": 1 }),
        ))
        .expect("first append");
    let second = ledger
        .append(observation(
            "patient-1",
            "01-01",
            "01-02",
            json!({ "n": 1 }),
        ))
        .expect("replay append");

    assert_eq!(ledger.len(), 1);
    assert!(second.admission.is_duplicate());
    assert_eq!(
        second.admission.recorded_id(),
        first.admission.recorded_id()
    );
}

#[test]
fn a_different_body_under_the_same_idempotency_key_is_refused_rather_than_silently_dropped() {
    let key = bioprism_ledger::IdempotencyKey::parse("intake-batch-9").expect("key");
    let mut ledger = EventLedger::new();
    ledger
        .append(
            observation("patient-1", "01-01", "01-02", json!({ "n": 1 }))
                .with_idempotency_key(key.clone()),
        )
        .expect("first append");

    let refusal = ledger
        .append(
            observation("patient-1", "01-01", "01-02", json!({ "n": 2 })).with_idempotency_key(key),
        )
        .expect_err("a conflicting body under a used key is a declared failure");
    assert!(matches!(refusal, LedgerError::IdempotencyConflict { .. }));
    assert_eq!(ledger.len(), 1);
}

#[test]
fn resubmitting_a_quarantined_event_does_not_queue_it_twice() {
    let mut ledger = EventLedger::new();
    let blocked =
        || observation("patient-1", "01-01", "01-02", json!({ "n": 1 })).caused_by([evt(0)]);
    ledger.append(blocked()).expect("first submission");
    let repeat = ledger.append(blocked()).expect("second submission");

    assert!(repeat.admission.is_quarantined());
    assert_eq!(ledger.quarantined().len(), 1);
}

// --- Causal parents and quarantine -----------------------------------------------------------

#[test]
fn an_event_whose_causal_parent_is_absent_is_quarantined_not_rejected() {
    let mut ledger = EventLedger::new();
    let receipt = ledger
        .append(observation("patient-1", "01-02", "01-02", json!({ "n": 1 })).caused_by([evt(0)]))
        .expect("quarantine is not an error");

    assert!(receipt.admission.is_quarantined());
    assert!(ledger.is_empty());
    assert_eq!(ledger.quarantined()[0].missing, vec![evt(0)]);
}

#[test]
fn a_quarantined_event_is_admitted_as_soon_as_its_parent_arrives() {
    let mut ledger = EventLedger::new();
    ledger
        .append(observation("patient-1", "01-02", "01-02", json!({ "n": 2 })).caused_by([evt(0)]))
        .expect("quarantined");
    let receipt = ledger
        .append(observation(
            "patient-1",
            "01-01",
            "01-01",
            json!({ "n": 1 }),
        ))
        .expect("parent append");

    assert_eq!(receipt.released, vec![evt(1)]);
    assert!(ledger.quarantined().is_empty());
    assert_eq!(ledger.len(), 2);
    assert_eq!(ledger.verify_chain(), ChainStatus::Intact);
}

#[test]
fn a_chain_of_out_of_order_arrivals_resolves_within_a_single_append() {
    let mut ledger = EventLedger::new();
    ledger
        .append(observation("patient-1", "01-03", "01-03", json!({ "n": 3 })).caused_by([evt(1)]))
        .expect("grandchild quarantined");
    ledger
        .append(observation("patient-1", "01-02", "01-02", json!({ "n": 2 })).caused_by([evt(0)]))
        .expect("child quarantined");
    let receipt = ledger
        .append(observation(
            "patient-1",
            "01-01",
            "01-01",
            json!({ "n": 1 }),
        ))
        .expect("root append");

    assert_eq!(receipt.released, vec![evt(1), evt(2)]);
    assert_eq!(ledger.len(), 3);
    assert_eq!(ledger.verify_causal_acyclicity(), ChainStatus::Intact);
}

#[test]
fn mutually_dependent_events_stay_quarantined_because_neither_parent_can_ever_arrive() {
    let mut ledger = EventLedger::new();
    ledger
        .append(observation("patient-1", "01-01", "01-01", json!({ "n": 1 })).caused_by([evt(9)]))
        .expect("quarantined");
    ledger
        .append(observation("patient-2", "01-01", "01-01", json!({ "n": 2 })).caused_by([evt(8)]))
        .expect("quarantined");
    ledger
        .append(observation(
            "patient-3",
            "01-01",
            "01-01",
            json!({ "n": 3 }),
        ))
        .expect("unrelated append");

    assert_eq!(ledger.len(), 1);
    assert_eq!(ledger.quarantined().len(), 2);
}

#[test]
fn a_causal_cycle_cannot_be_constructed_because_a_parent_must_already_precede_its_child() {
    let mut ledger = EventLedger::new();
    let root = record_once(
        &mut ledger,
        observation("patient-1", "01-01", "01-01", json!({ "n": 1 })),
    );
    ledger
        .append(observation("patient-1", "01-02", "01-02", json!({ "n": 2 })).caused_by([root]))
        .expect("child append");

    assert_eq!(ledger.verify_causal_acyclicity(), ChainStatus::Intact);
    for entry in ledger.entries() {
        for parent in entry.event.causal_parents() {
            assert!(ledger.get(parent).expect("parent retained").seq < entry.seq);
        }
    }
}

// --- Correction and supersession -------------------------------------------------------------

#[test]
fn a_correction_supersedes_the_original_without_removing_it() {
    let mut ledger = EventLedger::new();
    let original = record_once(
        &mut ledger,
        observation("patient-1", "01-01", "01-02", json!({ "diameter_mm": 14 })),
    );
    let correction = record_once(
        &mut ledger,
        observation("patient-1", "01-01", "06-01", json!({ "diameter_mm": 41 }))
            .superseding(original.clone()),
    );

    assert_eq!(ledger.len(), 2);
    assert!(ledger.get(&original).is_some());
    assert_eq!(ledger.superseded_by(&original), Some(&correction));
    assert_eq!(
        ledger.current_version_of(&original).expect("known event"),
        correction
    );
}

#[test]
fn two_corrections_of_the_same_entry_are_refused_as_a_fork() {
    let mut ledger = EventLedger::new();
    let original = record_once(
        &mut ledger,
        observation("patient-1", "01-01", "01-02", json!({ "diameter_mm": 14 })),
    );
    ledger
        .append(
            observation("patient-1", "01-01", "06-01", json!({ "diameter_mm": 41 }))
                .superseding(original.clone()),
        )
        .expect("first correction");

    let refusal = ledger
        .append(
            observation("patient-1", "01-01", "07-01", json!({ "diameter_mm": 42 }))
                .superseding(original.clone()),
        )
        .expect_err("a second correction of the same entry has no single current version");
    assert_eq!(
        refusal,
        LedgerError::AlreadySuperseded {
            target: original,
            by: evt(1)
        }
    );
}

#[test]
fn a_correction_recorded_before_its_original_is_refused() {
    let mut ledger = EventLedger::new();
    let original = record_once(
        &mut ledger,
        observation("patient-1", "01-01", "06-01", json!({ "diameter_mm": 14 })),
    );
    let refusal = ledger
        .append(
            observation("patient-1", "01-01", "02-01", json!({ "diameter_mm": 41 }))
                .superseding(original),
        )
        .expect_err("nothing can correct a fact it predates");
    assert!(matches!(
        refusal,
        LedgerError::CorrectionPrecedesOriginal { .. }
    ));
}

#[test]
fn superseding_an_unknown_event_is_refused() {
    let mut ledger = EventLedger::new();
    let refusal = ledger
        .append(observation("patient-1", "01-01", "01-02", json!({})).superseding(evt(77)))
        .expect_err("a correction to nothing is malformed");
    assert_eq!(refusal, LedgerError::UnknownEvent(evt(77)));
}

#[test]
fn current_version_follows_a_chain_of_successive_corrections() {
    let mut ledger = EventLedger::new();
    let first = record_once(
        &mut ledger,
        observation("patient-1", "01-01", "01-02", json!({ "n": 1 })),
    );
    let second = record_once(
        &mut ledger,
        observation("patient-1", "01-01", "02-02", json!({ "n": 2 })).superseding(first.clone()),
    );
    let third = record_once(
        &mut ledger,
        observation("patient-1", "01-01", "03-02", json!({ "n": 3 })).superseding(second),
    );

    assert_eq!(ledger.current_version_of(&first).expect("known"), third);
}

// --- As-of queries ---------------------------------------------------------------------------

/// The test the whole crate exists for. A fact is observed as true on 1 January and recorded on
/// 2 January; on 1 June a correction is recorded, still about 1 January. The two questions —
/// what did we know in March, and what is true about January — have different answers on the
/// same log, and a ledger that returns the same answer to both has lost the distinction.
#[test]
fn what_was_known_then_and_what_is_true_now_differ_on_a_log_with_a_late_correction() {
    let mut ledger = EventLedger::new();
    let original = record_once(
        &mut ledger,
        observation("patient-1", "01-01", "01-02", json!({ "diameter_mm": 14 })),
    );
    let correction = record_once(
        &mut ledger,
        observation("patient-1", "01-01", "06-01", json!({ "diameter_mm": 41 }))
            .superseding(original.clone()),
    );

    let in_march = ledger
        .state_of(
            &subject("patient-1"),
            &TemporalCut::known_at(record("03-01")),
        )
        .expect("inside the window")
        .expect("a fact was known in March");
    assert_eq!(in_march.id, original);
    assert_eq!(
        in_march.event.payload.value(&in_march.id).expect("present"),
        &json!({ "diameter_mm": 14 })
    );

    let today = ledger
        .state_of(&subject("patient-1"), &TemporalCut::true_at(valid("01-01")))
        .expect("inside the window")
        .expect("a fact is true about January");
    assert_eq!(today.id, correction);
    assert_eq!(
        today.event.payload.value(&today.id).expect("present"),
        &json!({ "diameter_mm": 41 })
    );
}

#[test]
fn a_correction_does_not_retroactively_hide_the_original_from_an_earlier_record_cut() {
    let mut ledger = EventLedger::new();
    let original = record_once(
        &mut ledger,
        observation("patient-1", "01-01", "01-02", json!({ "n": 1 })),
    );
    record_once(
        &mut ledger,
        observation("patient-1", "01-01", "06-01", json!({ "n": 2 })).superseding(original.clone()),
    );

    let early = ledger
        .cut(&TemporalCut::known_at(record("03-01")))
        .expect("inside the window");
    assert_eq!(early.len(), 1);
    assert_eq!(early[0].id, original);

    let late = ledger
        .cut(&TemporalCut::known_at(record("07-01")))
        .expect("inside the window");
    assert_eq!(late.len(), 1);
    assert_eq!(late[0].id, evt(1));
}

#[test]
fn a_valid_time_bound_and_a_record_time_bound_select_different_entries() {
    let mut ledger = EventLedger::new();
    record_once(
        &mut ledger,
        observation(
            "patient-1",
            "01-01",
            "09-01",
            json!({ "n": "old fact, learned late" }),
        ),
    );
    record_once(
        &mut ledger,
        observation("patient-2", "09-01", "09-02", json!({ "n": "recent fact" })),
    );

    let true_in_february = ledger
        .cut(&TemporalCut::true_at(valid("02-01")))
        .expect("inside the window");
    assert_eq!(true_in_february.len(), 1);
    assert_eq!(true_in_february[0].event.subject, subject("patient-1"));

    let known_in_february = ledger
        .cut(&TemporalCut::known_at(record("02-01")))
        .expect("inside the window");
    assert!(known_in_february.is_empty());
}

#[test]
fn a_release_cut_hides_an_embargoed_fact_that_a_record_cut_reveals() {
    let mut ledger = EventLedger::new();
    record_once(
        &mut ledger,
        embargoed(
            "cohort-a",
            "01-01",
            "01-02",
            "09-01",
            json!({ "auc": 0.81 }),
        ),
    );

    let curator_view = ledger
        .cut(&TemporalCut::known_at(record("03-01")))
        .expect("inside the window");
    assert_eq!(curator_view.len(), 1);

    let agent_view = ledger
        .cut(&TemporalCut::readable_at(release("03-01")))
        .expect("inside the window");
    assert!(
        agent_view.is_empty(),
        "an agent scored in March must not see a result unblinded in September"
    );
}

#[test]
fn the_bitemporal_cut_asks_what_we_then_believed_was_then_true() {
    let mut ledger = EventLedger::new();
    record_once(
        &mut ledger,
        observation("patient-1", "01-01", "01-02", json!({ "n": 1 })),
    );
    record_once(
        &mut ledger,
        observation("patient-1", "05-01", "05-02", json!({ "n": 2 })),
    );

    let believed = ledger
        .cut(&TemporalCut::believed_at(valid("03-01"), record("06-01")))
        .expect("inside the window");
    assert_eq!(believed.len(), 1);
    assert_eq!(believed[0].id, evt(0));
}

#[test]
fn record_time_regression_is_reported_as_a_clock_anomaly_rather_than_refused() {
    let mut ledger = EventLedger::new();
    record_once(
        &mut ledger,
        observation("patient-1", "01-01", "06-01", json!({ "n": 1 })),
    );
    record_once(
        &mut ledger,
        observation("patient-2", "01-01", "02-01", json!({ "n": 2 })),
    );

    let anomalies = ledger.clock_consistency();
    assert_eq!(anomalies.len(), 1);
    assert_eq!(anomalies[0].seq, 1);
    assert_eq!(ledger.len(), 2);
}

// --- Projections and checkpoints -------------------------------------------------------------

#[test]
fn a_projection_rebuilt_from_a_checkpoint_equals_one_rebuilt_from_genesis() {
    let mut ledger = busy_ledger();
    let checkpoint = ledger
        .project(&SubjectLatest)
        .checkpoint()
        .expect("state serializes")
        .expect("something was folded");
    assert_eq!(checkpoint.through_seq, 19);

    for index in 0..15u32 {
        let day = format!("03-{:02}", index + 1);
        ledger
            .append(observation(
                &format!("patient-{}", index % 7),
                &day,
                &day,
                json!({ "diameter_mm": index + 200 }),
            ))
            .expect("append succeeds");
    }
    let correction = observation("patient-3", "03-04", "12-01", json!({ "diameter_mm": 999 }))
        .superseding(evt(3));
    ledger.append(correction).expect("correction appends");

    let from_genesis = ledger.project(&SubjectLatest);
    let from_checkpoint = ledger
        .resume(&SubjectLatest, &checkpoint)
        .expect("checkpoint belongs to this log");

    assert_eq!(from_genesis.state, from_checkpoint.state);
    assert_eq!(from_genesis.through_seq, from_checkpoint.through_seq);
    assert_eq!(from_genesis.head_digest, from_checkpoint.head_digest);
    assert_eq!(from_genesis.applied, 36);
    assert_eq!(from_checkpoint.applied, 16);
}

#[test]
fn checkpoint_equality_holds_for_a_second_independent_projection() {
    let mut ledger = busy_ledger();
    let checkpoint = ledger
        .project(&ClassCounts)
        .checkpoint()
        .expect("state serializes")
        .expect("something was folded");
    ledger
        .append(embargoed("cohort-a", "04-01", "04-02", "09-01", json!({})))
        .expect("append succeeds");

    let from_genesis = ledger.project(&ClassCounts);
    let resumed = ledger
        .resume(&ClassCounts, &checkpoint)
        .expect("checkpoint belongs to this log");
    assert_eq!(from_genesis.state, resumed.state);
    assert_eq!(from_genesis.state[&EventClass::Material], 20);
    assert_eq!(from_genesis.state[&EventClass::Evaluation], 1);
}

#[test]
fn a_checkpoint_taken_against_a_different_log_is_refused() {
    let mut other = EventLedger::new();
    record_once(
        &mut other,
        observation("patient-x", "01-01", "01-01", json!({ "n": 0 })),
    );
    let foreign = other
        .project(&SubjectLatest)
        .checkpoint()
        .expect("state serializes")
        .expect("something was folded");

    let ledger = busy_ledger();
    let refusal = ledger
        .resume(&SubjectLatest, &foreign)
        .expect_err("a checkpoint commits to one specific history");
    assert!(matches!(refusal, LedgerError::CheckpointDivergence { .. }));
}

#[test]
fn a_checkpoint_whose_state_was_edited_is_refused() {
    let ledger = busy_ledger();
    let mut checkpoint = ledger
        .project(&SubjectLatest)
        .checkpoint()
        .expect("state serializes")
        .expect("something was folded");
    checkpoint.state.remove(&subject("patient-2"));

    let refusal = checkpoint
        .verify_state()
        .expect_err("the state no longer hashes to its recorded digest");
    assert!(matches!(
        refusal,
        LedgerError::CheckpointStateMismatch { .. }
    ));
    assert!(ledger.resume(&SubjectLatest, &checkpoint).is_err());
}

#[test]
fn projection_lag_counts_the_entries_a_checkpoint_has_not_absorbed() {
    let mut ledger = busy_ledger();
    let checkpoint = ledger
        .project(&ClassCounts)
        .checkpoint()
        .expect("state serializes")
        .expect("something was folded");
    assert_eq!(ledger.projection_lag(&checkpoint), 0);

    for index in 0..4u32 {
        let day = format!("05-{:02}", index + 1);
        ledger
            .append(observation(
                "patient-lag",
                &day,
                &day,
                json!({ "n": index }),
            ))
            .expect("append succeeds");
    }
    assert_eq!(ledger.projection_lag(&checkpoint), 4);
}

#[test]
fn the_subject_latest_projection_agrees_with_the_as_of_query_at_the_open_cut() {
    let mut ledger = busy_ledger();
    record_once(
        &mut ledger,
        observation("patient-1", "02-03", "12-01", json!({ "diameter_mm": 7 }))
            .superseding(evt(13)),
    );

    let projected: BTreeMap<SubjectKey, EventId> = ledger
        .project(&SubjectLatest)
        .state
        .into_iter()
        .map(|(key, fact)| (key, fact.event))
        .collect();
    assert_eq!(projected, state_ids(&ledger, &TemporalCut::EVERYTHING));
}

#[test]
fn an_as_of_projection_carries_no_resume_point_because_a_cut_is_a_filter_not_a_prefix() {
    let ledger = busy_ledger();
    let run = ledger
        .project_cut(&ClassCounts, &TemporalCut::known_at(record("01-05")))
        .expect("inside the window");

    assert_eq!(run.applied, 5);
    assert_eq!(run.through_seq, None);
    assert!(run.checkpoint().expect("state serializes").is_none());
}

// --- Retention, compaction and deletion ------------------------------------------------------

#[test]
fn compaction_is_a_dry_run_unless_it_is_asked_to_apply() {
    let mut ledger = busy_ledger();
    let report = ledger
        .compact(&RetentionPolicy::before(record("01-11")))
        .expect("dry run");

    assert!(report.dry_run);
    assert!(report.anchor.is_none());
    assert_eq!(ledger.len(), 20);
    assert!(ledger.window().is_unrestricted());
    assert!(
        report.removed > 0,
        "the dry run still says what it would do"
    );
}

#[test]
fn a_compacted_ledger_still_verifies_its_chain_and_declares_what_it_removed() {
    let mut ledger = busy_ledger();
    let before = ledger.len();
    let report = ledger
        .compact(&RetentionPolicy::before(record("01-11")).applying())
        .expect("compaction applies");

    assert_eq!(ledger.len() as u64, before as u64 - report.removed);
    assert_eq!(ledger.verify_chain(), ChainStatus::Intact);
    assert_eq!(ledger.verify_causal_acyclicity(), ChainStatus::Intact);
    let anchor = report
        .anchor
        .expect("an applied compaction commits to what it destroyed");
    assert_eq!(anchor.removed_count, report.removed);
    assert_eq!(anchor.recompute_digest(), anchor.digest);
    assert_eq!(ledger.compactions().len(), 1);
}

#[test]
fn a_compacted_ledger_answers_state_queries_inside_its_retained_window_identically() {
    let mut ledger = busy_ledger();
    let cuts = [
        TemporalCut::known_at(record("01-11")),
        TemporalCut::known_at(record("01-15")),
        TemporalCut::known_at(record("01-20")),
        TemporalCut::EVERYTHING,
    ];
    let before: Vec<BTreeMap<SubjectKey, EventId>> =
        cuts.iter().map(|cut| state_ids(&ledger, cut)).collect();

    ledger
        .compact(&RetentionPolicy::before(record("01-11")).applying())
        .expect("compaction applies");

    let after: Vec<BTreeMap<SubjectKey, EventId>> =
        cuts.iter().map(|cut| state_ids(&ledger, cut)).collect();
    assert_eq!(before, after);
}

#[test]
fn a_query_behind_the_compaction_boundary_is_refused_rather_than_answered_from_what_is_left() {
    let mut ledger = busy_ledger();
    ledger
        .compact(&RetentionPolicy::before(record("01-11")).applying())
        .expect("compaction applies");

    let refusal = ledger
        .cut(&TemporalCut::known_at(record("01-03")))
        .expect_err("the ledger no longer holds that period");
    match refusal {
        LedgerError::OutsideRetainedWindow { axis, .. } => assert_eq!(axis, "record"),
        other => panic!("expected a retained-window refusal, got {other}"),
    }
    assert_eq!(
        ledger.window().answerable_from_record,
        Some(record("01-11"))
    );
}

#[test]
fn compaction_retains_the_causal_parents_of_survivors_so_no_edge_dangles() {
    let mut ledger = EventLedger::new();
    let ancestor = record_once(
        &mut ledger,
        observation("patient-1", "01-01", "01-01", json!({ "n": 1 })),
    );
    record_once(
        &mut ledger,
        observation("patient-1", "01-02", "01-02", json!({ "n": 2 })),
    );
    record_once(
        &mut ledger,
        observation("patient-9", "09-01", "09-01", json!({ "n": 3 })).caused_by([ancestor.clone()]),
    );

    let report = ledger
        .compact(&RetentionPolicy::before(record("06-01")).applying())
        .expect("compaction applies");
    assert!(report.retained_by_causal_closure.contains(&ancestor));
    assert!(ledger.get(&ancestor).is_some());
    assert_eq!(ledger.verify_causal_acyclicity(), ChainStatus::Intact);
}

#[test]
fn compaction_retains_the_original_of_a_surviving_correction() {
    let mut ledger = EventLedger::new();
    let original = record_once(
        &mut ledger,
        observation("patient-1", "01-01", "01-01", json!({ "n": 1 })),
    );
    record_once(
        &mut ledger,
        observation("patient-1", "01-02", "01-02", json!({ "n": 2 })),
    );
    record_once(
        &mut ledger,
        observation("patient-1", "01-01", "09-01", json!({ "n": 3 })).superseding(original.clone()),
    );

    let report = ledger
        .compact(&RetentionPolicy::before(record("06-01")).applying())
        .expect("compaction applies");
    assert!(report.retained_by_supersession_closure.contains(&original));
    assert!(ledger.get(&original).is_some());
    assert_eq!(ledger.superseded_by(&original), Some(&evt(2)));
}

#[test]
fn a_pinned_event_survives_compaction_that_would_otherwise_remove_it() {
    let mut ledger = busy_ledger();
    let held = evt(2);
    let report = ledger
        .compact(
            &RetentionPolicy::before(record("01-11"))
                .pinning([held.clone()])
                .applying(),
        )
        .expect("compaction applies");

    assert!(report.retained_by_pin.contains(&held));
    assert!(ledger.get(&held).is_some());
}

#[test]
fn appending_into_a_period_the_ledger_has_already_compacted_is_refused() {
    let mut ledger = busy_ledger();
    ledger
        .compact(&RetentionPolicy::before(record("01-11")).applying())
        .expect("compaction applies");

    let refusal = ledger
        .append(observation(
            "patient-late",
            "01-02",
            "01-02",
            json!({ "n": 0 }),
        ))
        .expect_err("a compacted period cannot be written into");
    assert!(matches!(refusal, LedgerError::OutsideRetainedWindow { .. }));
}

#[test]
fn a_checkpoint_older_than_the_retained_window_cannot_be_resumed() {
    let mut ledger = busy_ledger();
    let early = ledger
        .project_through(&ClassCounts, 2)
        .checkpoint()
        .expect("state serializes")
        .expect("something was folded");
    ledger
        .compact(&RetentionPolicy::before(record("01-11")).applying())
        .expect("compaction applies");

    let refusal = ledger
        .resume(&ClassCounts, &early)
        .expect_err("the entries after that point are gone");
    assert!(matches!(
        refusal,
        LedgerError::CheckpointOutsideRetention { .. }
    ));
}

#[test]
fn compaction_of_a_ledger_with_nothing_old_enough_removes_nothing() {
    let mut ledger = busy_ledger();
    let report = ledger
        .compact(&RetentionPolicy::before(record("01-01")).applying())
        .expect("compaction applies");

    assert_eq!(report.examined, 0);
    assert_eq!(report.removed, 0);
    assert!(report.anchor.is_none());
    assert!(ledger.window().is_unrestricted());
    assert_eq!(ledger.len(), 20);
}

#[test]
fn redaction_destroys_the_payload_and_leaves_the_chain_intact() {
    let mut ledger = busy_ledger();
    let target = evt(4);
    let before = ledger.get(&target).expect("entry exists").digest.clone();
    let redaction = ledger
        .redact(&target, "subject withdrew consent")
        .expect("entry exists");

    assert_eq!(redaction.entry_digest, before);
    assert_eq!(ledger.verify_chain(), ChainStatus::Intact);
    let entry = ledger.get(&target).expect("entry survives");
    assert!(entry.event.payload.is_redacted());
    assert_eq!(
        entry.event.payload.digest().as_str(),
        redaction.payload_digest
    );
}

#[test]
fn reading_a_redacted_payload_fails_with_the_reason_it_was_destroyed() {
    let mut ledger = busy_ledger();
    let target = evt(4);
    ledger
        .redact(&target, "subject withdrew consent")
        .expect("entry exists");

    let entry = ledger.get(&target).expect("entry survives");
    let refusal = entry
        .event
        .payload
        .value(&entry.id)
        .expect_err("the bytes are gone");
    assert_eq!(
        refusal,
        LedgerError::PayloadRedacted {
            id: target,
            reason: "subject withdrew consent".to_string()
        }
    );
}

#[test]
fn redacting_an_unknown_event_is_refused() {
    let mut ledger = EventLedger::new();
    let refusal = ledger
        .redact(&evt(3), "any reason")
        .expect_err("no such entry");
    assert_eq!(refusal, LedgerError::UnknownEvent(evt(3)));
}

// --- Schema and serialization ----------------------------------------------------------------

#[test]
fn a_closed_catalog_refuses_an_unknown_event_kind() {
    let mut ledger = EventLedger::with_schemas(SchemaCatalog::closed(["cohort.scored"]));
    let refusal = ledger
        .append(observation("patient-1", "01-01", "01-01", json!({})))
        .expect_err("lesion.measured is not in the catalog");
    assert_eq!(
        refusal,
        LedgerError::UnknownSchema {
            kind: "lesion.measured".to_string()
        }
    );
    ledger
        .append(embargoed("cohort-a", "01-01", "01-01", "01-01", json!({})))
        .expect("a declared kind is admitted");
}

#[test]
fn an_open_catalog_admits_any_well_formed_kind_but_still_rejects_a_blank_one() {
    assert!(EventKind::parse("").is_err());
    assert!(SubjectKey::parse("bad\u{0}subject").is_err());
    assert!(Actor::new("someone", "").is_err());
}

#[test]
fn a_recorded_entry_round_trips_through_json_without_changing_its_digest() {
    let ledger = busy_ledger();
    let entry = &ledger.entries()[7];
    let text = serde_json::to_string(entry).expect("entry serializes");
    let restored: bioprism_ledger::LedgerEntry =
        serde_json::from_str(&text).expect("entry deserializes");

    assert_eq!(&restored, entry);
    assert_eq!(restored.recompute_digest(), entry.digest);
}

#[test]
fn a_checkpoint_round_trips_through_json_and_still_verifies() {
    let ledger = busy_ledger();
    let checkpoint = ledger
        .project(&SubjectLatest)
        .checkpoint()
        .expect("state serializes")
        .expect("something was folded");
    let text = serde_json::to_string(&checkpoint).expect("checkpoint serializes");
    let restored: bioprism_ledger::Checkpoint<<SubjectLatest as Projection>::State> =
        serde_json::from_str(&text).expect("checkpoint deserializes");

    restored.verify_state().expect("state digest still matches");
    assert_eq!(
        ledger
            .resume(&SubjectLatest, &restored)
            .expect("resumes")
            .state,
        ledger.project(&SubjectLatest).state
    );
}

#[test]
fn the_class_histogram_counts_every_transition_family_the_log_holds() {
    let mut ledger = busy_ledger();
    ledger
        .append(embargoed("cohort-a", "04-01", "04-02", "09-01", json!({})))
        .expect("append succeeds");

    let histogram = ledger.class_histogram();
    assert_eq!(histogram[&EventClass::Material], 20);
    assert_eq!(histogram[&EventClass::Evaluation], 1);
    assert!(!histogram.contains_key(&EventClass::Policy));
}
