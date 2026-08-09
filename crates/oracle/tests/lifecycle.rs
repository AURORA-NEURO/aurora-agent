//! Versioning, drift, and expiration (31.16), and the instant representation they rest on.

mod common;

use bioprism_oracle::{
    Admissibility, EvidenceTier, Judgement, MeshPolicy, OracleError, OracleManifest, Plane,
    Position, UtcTimestamp, ValidityWindow, VerdictBasis,
};
use bioprism_section::OracleStatus;
use common::{confidence, judgement, now, oracle_ref, ts};

fn windowed(name: &str, window: ValidityWindow) -> OracleManifest {
    OracleManifest::new(
        oracle_ref(name, 1),
        EvidenceTier::Deterministic,
        [Plane::Artifact],
        [],
        window,
    )
    .expect("fixture manifest is well formed")
    .disclaiming_the_rest()
}

#[test]
fn an_expired_oracle_judgement_says_it_expired_rather_than_being_silently_used() {
    let manifest = windowed(
        "stale_schema",
        ValidityWindow::new(ts("2020-01-01T00:00:00Z"), Some(ts("2021-01-01T00:00:00Z")))
            .expect("the window opens before it closes"),
    );
    let judged =
        Judgement::from_manifest(&manifest, &now(), Position::Contradicted, confidence(1.0));

    assert!(!judged.is_admissible());
    assert_eq!(
        judged.admissibility,
        Admissibility::Expired {
            at: now(),
            valid_until: ts("2021-01-01T00:00:00Z"),
        }
    );
    assert!(judged.admissibility.reason().contains("expired"));

    let verdict = MeshPolicy::default().combine("bundle", &now(), vec![judged]);
    assert_eq!(verdict.basis, VerdictBasis::NoAdmissibleOracle);
    assert_eq!(
        verdict.inadmissible.len(),
        1,
        "the expired judgement is retained, because 'we ran it and discarded it' differs from \
         'we never ran it'"
    );
    assert!(verdict.contributing.is_empty());
}

#[test]
fn a_judgement_made_before_the_validity_window_opens_is_inadmissible() {
    let manifest = windowed(
        "future_schema",
        ValidityWindow::open_ended(ts("2030-01-01T00:00:00Z")),
    );
    let judged = Judgement::from_manifest(&manifest, &now(), Position::Supported, confidence(1.0));

    assert_eq!(
        judged.admissibility,
        Admissibility::NotYetValid {
            at: now(),
            valid_from: ts("2030-01-01T00:00:00Z"),
        }
    );
}

#[test]
fn a_superseded_oracle_names_its_successor_and_is_inadmissible() {
    let successor = oracle_ref("schema", 2);
    let manifest = windowed(
        "schema",
        ValidityWindow::open_ended(ts("2000-01-01T00:00:00Z")),
    )
    .superseded_by(successor.clone());
    let judged = Judgement::from_manifest(&manifest, &now(), Position::Supported, confidence(1.0));

    assert_eq!(
        judged.admissibility,
        Admissibility::Superseded { by: successor }
    );
    assert!(judged
        .admissibility
        .reason()
        .contains("biooracle:test:schema:2.0.0"));
}

#[test]
fn an_expired_deterministic_oracle_stops_suppressing_a_judge_that_is_still_valid() {
    let expired = windowed(
        "stale_schema",
        ValidityWindow::new(ts("2020-01-01T00:00:00Z"), Some(ts("2021-01-01T00:00:00Z")))
            .expect("the window opens before it closes"),
    );
    let verdict = MeshPolicy::default().combine(
        "bundle",
        &now(),
        vec![
            Judgement::from_manifest(&expired, &now(), Position::Contradicted, confidence(1.0)),
            judgement("judge", EvidenceTier::Judge, Position::Supported, 0.9),
        ],
    );

    assert_eq!(
        verdict.status(),
        OracleStatus::Valid,
        "expiry removes the oracle from the ladder entirely; it cannot half-count"
    );
    assert!(verdict.suppressed.is_empty());
    assert_eq!(verdict.inadmissible.len(), 1);
}

#[test]
fn a_validity_window_that_closes_before_it_opens_is_rejected() {
    let error = ValidityWindow::new(ts("2026-01-01T00:00:00Z"), Some(ts("2025-01-01T00:00:00Z")))
        .expect_err("an inverted window admits nothing");

    assert!(matches!(error, OracleError::InvertedValidityWindow { .. }));
}

#[test]
fn a_timestamp_with_a_non_zulu_offset_is_rejected() {
    for bad in [
        "2026-08-08T12:00:00+01:00",
        "2026-08-08T12:00:00",
        "2026-08-08 12:00:00Z",
        "2026-08-08T12:00:00.500Z",
    ] {
        assert!(
            UtcTimestamp::parse(bad).is_err(),
            "{bad:?} would break the guarantee that byte order is chronological order"
        );
    }
}

#[test]
fn lexical_order_on_utc_timestamps_is_chronological_order() {
    let earlier = ts("2026-08-08T09:59:59Z");
    let later = ts("2026-08-08T10:00:00Z");
    assert!(earlier < later);

    let year_end = ts("2026-12-31T23:59:59Z");
    let year_start = ts("2027-01-01T00:00:00Z");
    assert!(year_end < year_start);
}

#[test]
fn an_impossible_calendar_date_is_rejected() {
    assert!(UtcTimestamp::parse("2026-02-30T00:00:00Z").is_err());
    assert!(UtcTimestamp::parse("2026-02-29T00:00:00Z").is_err());
    assert!(
        UtcTimestamp::parse("2024-02-29T00:00:00Z").is_ok(),
        "2024 is a leap year"
    );
    assert!(UtcTimestamp::parse("2026-13-01T00:00:00Z").is_err());
    assert!(UtcTimestamp::parse("2026-08-08T24:00:00Z").is_err());
}
