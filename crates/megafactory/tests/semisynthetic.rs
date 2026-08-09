//! Invariants of semi-synthetic world construction, blueprint 35.03.
//!
//! The seed search below is the module's central claim made reproducible: randomising which
//! backgrounds receive an insertion does not make the batch confound impossible, and at a realistic
//! panel size it happens often enough to expect it.

use bioprism_megafactory::{
    assign_insertions, Background, ConfoundReport, Insertion, InsertionError, LatentRecovery,
    Panel, TransferClaim,
};
use bioprism_scale::audit::{Auditor, ReleaseAudit};
use bioprism_scope::{Interval, Timestamp};

fn at(nanos: i128) -> Timestamp {
    Timestamp::from_nanos_utc(nanos)
}

fn window() -> Interval {
    Interval {
        start: Some(at(0)),
        end: Some(at(10_000)),
    }
}

/// Twelve backgrounds in four batches of three: the shape the seed search runs over.
fn twelve_backgrounds() -> Vec<Background> {
    (0..12)
        .map(|index| {
            Background::new(
                format!("bg-{index:02}"),
                format!("batch-{}", index / 3),
                window(),
            )
        })
        .collect()
}

fn inserted(background: &str) -> Insertion {
    Insertion::new(background, "lesion", at(100), 1.0)
}

#[test]
fn a_batch_that_perfectly_predicts_insertion_makes_the_panel_unusable() {
    let backgrounds = vec![
        Background::new("a1", "batch-a", window()),
        Background::new("a2", "batch-a", window()),
        Background::new("b1", "batch-b", window()),
        Background::new("b2", "batch-b", window()),
    ];
    let panel = Panel::new(backgrounds, vec![inserted("a1"), inserted("a2")]);
    let report = ConfoundReport::measure(&panel).expect("consistent panel");

    assert!(report.batch_determines_label);
    assert!(!report.is_usable_as_an_oracle());
    assert_eq!(report.pure_batches, vec!["batch-a", "batch-b"]);
    assert!(report.headline().contains("batch bookkeeping"));
}

#[test]
fn a_mixed_panel_with_impure_batches_is_usable_as_an_oracle() {
    let backgrounds = vec![
        Background::new("a1", "batch-a", window()),
        Background::new("a2", "batch-a", window()),
        Background::new("b1", "batch-b", window()),
        Background::new("b2", "batch-b", window()),
    ];
    let panel = Panel::new(backgrounds, vec![inserted("a1"), inserted("b1")]);
    let report = ConfoundReport::measure(&panel).expect("consistent panel");

    assert!(!report.batch_determines_label);
    assert!(report.is_usable_as_an_oracle());
    assert_eq!(report.worst_batch_imbalance, 0.0);
}

#[test]
fn a_panel_with_one_label_only_has_no_discrimination_to_measure() {
    let backgrounds = vec![
        Background::new("a1", "batch-a", window()),
        Background::new("b1", "batch-b", window()),
    ];
    let none = Panel::new(backgrounds.clone(), Vec::new());
    let report = ConfoundReport::measure(&none).expect("consistent panel");
    assert!(!report.is_mixed());
    assert!(!report.batch_determines_label);
    assert!(!report.is_usable_as_an_oracle());

    let all = Panel::new(backgrounds, vec![inserted("a1"), inserted("b1")]);
    let report = ConfoundReport::measure(&all).expect("consistent panel");
    assert!(!report.is_mixed());
    assert!(!report.is_usable_as_an_oracle());
}

#[test]
fn an_insertion_into_an_unknown_background_is_refused() {
    let panel = Panel::new(
        vec![Background::new("a1", "batch-a", window())],
        vec![inserted("ghost")],
    );
    assert_eq!(
        panel.check_consistency(),
        Err(InsertionError::UnknownBackground("ghost".into()))
    );
}

#[test]
fn two_insertions_into_one_background_are_refused() {
    let panel = Panel::new(
        vec![Background::new("a1", "batch-a", window())],
        vec![inserted("a1"), inserted("a1")],
    );
    assert_eq!(
        panel.check_consistency(),
        Err(InsertionError::DoubleInsertion("a1".into()))
    );
}

#[test]
fn a_duplicate_background_is_refused() {
    let panel = Panel::new(
        vec![
            Background::new("a1", "batch-a", window()),
            Background::new("a1", "batch-b", window()),
        ],
        Vec::new(),
    );
    assert_eq!(
        panel.check_consistency(),
        Err(InsertionError::DuplicateBackground("a1".into()))
    );
}

#[test]
fn an_onset_outside_the_backgrounds_observation_window_is_refused() {
    let panel = Panel::new(
        vec![Background::new("a1", "batch-a", window())],
        vec![Insertion::new("a1", "lesion", at(99_999), 1.0)],
    );
    assert_eq!(
        panel.check_consistency(),
        Err(InsertionError::OnsetOutsideBackground {
            background: "a1".into()
        })
    );
}

#[test]
fn seeded_assignment_is_reproducible_and_lands_inside_every_window() {
    let backgrounds = twelve_backgrounds();
    let first = assign_insertions(&backgrounds, 7, 6, "lesion", 1.0);
    let second = assign_insertions(&backgrounds, 7, 6, "lesion", 1.0);
    assert_eq!(first, second, "the same seed must give the same assignment");
    assert_eq!(first.len(), 6);

    let panel = Panel::new(backgrounds, first);
    panel
        .check_consistency()
        .expect("assignment lands inside every observation window by construction");
}

#[test]
fn a_different_seed_gives_a_different_assignment() {
    let backgrounds = twelve_backgrounds();
    let first = assign_insertions(&backgrounds, 0, 6, "lesion", 1.0);
    let other = assign_insertions(&backgrounds, 1, 6, "lesion", 1.0);
    assert_ne!(first, other);
}

#[test]
fn asking_for_more_insertions_than_backgrounds_inserts_into_all_of_them() {
    let backgrounds = twelve_backgrounds();
    let all = assign_insertions(&backgrounds, 3, 99, "lesion", 1.0);
    assert_eq!(all.len(), backgrounds.len());
}

#[test]
fn randomised_assignment_still_produces_a_batch_determined_panel_at_seed_399() {
    let backgrounds = twelve_backgrounds();
    let insertions = assign_insertions(&backgrounds, 399, 6, "lesion", 1.0);
    let panel = Panel::new(backgrounds, insertions);
    let report = ConfoundReport::measure(&panel).expect("consistent panel");

    assert!(
        report.batch_determines_label,
        "seed 399 is the lowest seed at which every batch comes out pure: {}",
        report.headline()
    );
    assert!(!report.is_usable_as_an_oracle());
}

#[test]
fn seed_399_is_the_lowest_seed_that_confounds_this_panel() {
    let backgrounds = twelve_backgrounds();
    let first_bad = (0..400u64).find(|seed| {
        let insertions = assign_insertions(&backgrounds, *seed, 6, "lesion", 1.0);
        let panel = Panel::new(backgrounds.clone(), insertions);
        ConfoundReport::measure(&panel)
            .expect("consistent panel")
            .batch_determines_label
    });
    assert_eq!(first_bad, Some(399));
}

#[test]
fn the_batch_confound_is_common_enough_to_expect_rather_than_rare_enough_to_ignore() {
    let backgrounds = twelve_backgrounds();
    let confounded = (0..10_000u64)
        .filter(|seed| {
            let insertions = assign_insertions(&backgrounds, *seed, 6, "lesion", 1.0);
            let panel = Panel::new(backgrounds.clone(), insertions);
            ConfoundReport::measure(&panel)
                .expect("consistent panel")
                .batch_determines_label
        })
        .count();
    assert_eq!(
        confounded, 66,
        "66 of the first 10,000 seeds confound this panel outright, which is the combinatorial \
         rate: 6 of the 924 ways to choose 6 of 12 take two whole batches"
    );
}

#[test]
fn perfect_recall_on_a_confounded_panel_is_still_not_an_oracle() {
    let backgrounds = vec![
        Background::new("a1", "batch-a", window()),
        Background::new("a2", "batch-a", window()),
        Background::new("b1", "batch-b", window()),
        Background::new("b2", "batch-b", window()),
    ];
    let panel = Panel::new(backgrounds, vec![inserted("a1"), inserted("a2")]);
    let recovery = LatentRecovery::measure(&panel, &["a1".into(), "a2".into()])
        .expect("calls name known backgrounds");

    assert_eq!(recovery.recall, 1.0);
    assert_eq!(recovery.precision, 1.0);
    assert!(
        !recovery.panel_usable_as_an_oracle,
        "a perfect recovery figure travels with the verdict that the panel does not license it"
    );
}

#[test]
fn a_recovery_figure_counts_false_positives_against_precision() {
    let backgrounds = vec![
        Background::new("a1", "batch-a", window()),
        Background::new("a2", "batch-a", window()),
        Background::new("b1", "batch-b", window()),
        Background::new("b2", "batch-b", window()),
    ];
    let panel = Panel::new(backgrounds, vec![inserted("a1"), inserted("b1")]);
    let recovery = LatentRecovery::measure(&panel, &["a1".into(), "a2".into(), "b1".into()])
        .expect("calls name known backgrounds");
    assert_eq!(recovery.true_positives, 2);
    assert_eq!(recovery.false_positives, 1);
    assert_eq!(recovery.recall, 1.0);
    assert!(recovery.panel_usable_as_an_oracle);
}

#[test]
fn a_detector_call_naming_an_unknown_background_is_refused() {
    let panel = Panel::new(
        vec![Background::new("a1", "batch-a", window())],
        vec![inserted("a1")],
    );
    assert_eq!(
        LatentRecovery::measure(&panel, &["ghost".into()]),
        Err(InsertionError::UnknownDetectorCall("ghost".into()))
    );
}

#[test]
fn a_transfer_claim_must_be_written_down_in_one_state_or_the_other() {
    let unvalidated = TransferClaim::unvalidated("no observed cohort has been assembled yet");
    assert!(!unvalidated.is_validated());
    let json = serde_json::to_string(&unvalidated).expect("serialisable");
    assert!(json.contains(r#""transfer":"unvalidated""#), "{json}");

    let validated = TransferClaim::validated("cohort-x", 0.8);
    assert!(validated.is_validated());
}

#[test]
fn the_confound_report_contributes_the_non_llm_oracle_gate_and_only_that_one() {
    let backgrounds = vec![
        Background::new("a1", "batch-a", window()),
        Background::new("a2", "batch-a", window()),
        Background::new("b1", "batch-b", window()),
        Background::new("b2", "batch-b", window()),
    ];
    let panel = Panel::new(backgrounds, vec![inserted("a1"), inserted("b1")]);
    let report = ConfoundReport::measure(&panel).expect("consistent panel");

    let mut audit = ReleaseAudit::open("panel-1", "factory", Auditor::new("independent-site"))
        .expect("an independent auditor");
    report.contribute_to(&mut audit);
    assert!(
        audit.finish().is_err(),
        "one gate out of eight does not finish a release audit"
    );
}
