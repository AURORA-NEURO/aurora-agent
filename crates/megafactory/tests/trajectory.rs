//! Invariants of trajectory capture, redaction, and completeness, blueprint 35.06.

use bioprism_megafactory::{
    CaptureError, CaptureSession, Completeness, Field, RedactionPolicy, Span, SpanKind,
};
use serde_json::json;

const SECRET: &str = "MRN-4471-QX";

fn session_with(secret: &str) -> CaptureSession {
    let mut session = CaptureSession::new("notebook-run-1");
    session
        .append(
            Span::new(1, SpanKind::Execution)
                .with("cell", Field::recorded("load cohort"))
                .with("subject", Field::recorded(secret)),
        )
        .expect("first span");
    session
        .append(
            Span::new(2, SpanKind::Tool)
                .with("tool", Field::recorded("aligner"))
                .with("subject", Field::recorded(secret))
                .with("stderr", Field::NotCaptured),
        )
        .expect("second span");
    session
}

fn policy() -> RedactionPolicy {
    RedactionPolicy::new(
        ["subject"],
        "direct identifier removed under the data use agreement",
    )
}

#[test]
fn a_field_the_instrumentation_never_saw_is_not_a_redacted_one() {
    let not_captured = Field::NotCaptured;
    let redacted = Field::redacted("policy");
    assert_ne!(not_captured, redacted);
    assert_eq!(not_captured.value(), None);
    assert_eq!(redacted.value(), None);
    assert!(redacted.is_redacted());
    assert!(!not_captured.is_redacted());
}

#[test]
fn a_recorded_field_that_is_empty_is_still_not_a_missing_one() {
    let empty = Field::recorded("");
    assert_eq!(empty.value(), Some(""));
    assert_ne!(empty, Field::NotCaptured);
}

#[test]
fn the_released_digest_cannot_confirm_a_guess_at_the_redacted_value() {
    let first = session_with(SECRET)
        .redact(&policy())
        .expect("the policy names a field the session carries");
    let second = session_with("MRN-0000-AA")
        .redact(&policy())
        .expect("the policy names a field the session carries");

    assert_eq!(
        first.digest(),
        second.digest(),
        "two sessions differing only in the redacted value must be indistinguishable by digest, \
         or the digest is a confirmation oracle for the value the redaction removed"
    );
}

#[test]
fn the_released_digest_still_changes_when_unredacted_content_changes() {
    let baseline = session_with(SECRET)
        .redact(&policy())
        .expect("policy applies");
    let mut altered_source = CaptureSession::new("notebook-run-1");
    altered_source
        .append(
            Span::new(1, SpanKind::Execution)
                .with("cell", Field::recorded("load a different cohort"))
                .with("subject", Field::recorded(SECRET)),
        )
        .expect("first span");
    altered_source
        .append(
            Span::new(2, SpanKind::Tool)
                .with("tool", Field::recorded("aligner"))
                .with("subject", Field::recorded(SECRET))
                .with("stderr", Field::NotCaptured),
        )
        .expect("second span");
    let altered = altered_source.redact(&policy()).expect("policy applies");

    assert_ne!(
        baseline.digest(),
        altered.digest(),
        "redaction must not flatten the content that was kept"
    );
}

#[test]
fn a_leakage_scan_finds_a_secret_the_policy_did_not_cover() {
    let released = session_with(SECRET)
        .redact(&RedactionPolicy::new(["tool"], "vendor name withheld"))
        .expect("policy applies");
    let scan = released.scan_for_leakage(&[SECRET.to_string()]);
    assert!(!scan.found_nothing());
    assert_eq!(scan.leaked, vec![SECRET.to_string()]);
}

#[test]
fn a_leakage_scan_finds_nothing_when_the_policy_covers_the_field() {
    let released = session_with(SECRET)
        .redact(&policy())
        .expect("policy applies");
    let scan = released.scan_for_leakage(&[SECRET.to_string()]);
    assert!(scan.found_nothing());
}

#[test]
fn a_scan_that_found_nothing_carries_how_many_secrets_it_was_told_about() {
    let released = session_with(SECRET)
        .redact(&policy())
        .expect("policy applies");
    let scan = released.scan_for_leakage(&[]);
    assert!(scan.found_nothing());
    assert_eq!(
        scan.probes, 0,
        "a scan over no probes finds nothing and proves nothing, so the probe count travels"
    );
}

#[test]
fn a_redaction_policy_naming_a_field_no_span_carries_is_refused() {
    let error = session_with(SECRET)
        .redact(&RedactionPolicy::new(["password"], "not present"))
        .expect_err("a policy that matches nothing is a policy nobody checked");
    assert_eq!(
        error,
        CaptureError::RedactionTargetAbsent("password".into())
    );
}

#[test]
fn redaction_replaces_values_and_leaves_uncaptured_fields_alone() {
    let released = session_with(SECRET)
        .redact(&policy())
        .expect("policy applies");
    let second = released
        .spans()
        .iter()
        .find(|span| span.seq == 2)
        .expect("second span survives");
    assert!(second.field("subject").expect("present").is_redacted());
    assert_eq!(second.field("stderr"), Some(&Field::NotCaptured));
    assert_eq!(second.field("tool").and_then(Field::value), Some("aligner"));
}

#[test]
fn the_redaction_share_counts_withheld_fields_and_is_not_capture_overhead() {
    let released = session_with(SECRET)
        .redact(&policy())
        .expect("policy applies");
    assert_eq!(released.redacted_fields, 2);
    assert_eq!(released.total_fields, 5);
    assert_eq!(
        released.redaction_share(),
        released.redacted_fields as f64 / released.total_fields as f64
    );
}

#[test]
fn an_out_of_order_span_is_refused_rather_than_quietly_sorted() {
    let mut session = CaptureSession::new("s");
    session
        .append(Span::new(5, SpanKind::Execution))
        .expect("first");
    assert_eq!(
        session.append(Span::new(3, SpanKind::Execution)),
        Err(CaptureError::OutOfOrder {
            session: "s".into(),
            seq: 3,
            previous: 5
        })
    );
}

#[test]
fn a_repeated_sequence_number_is_refused() {
    let mut session = CaptureSession::new("s");
    session
        .append(Span::new(1, SpanKind::Execution))
        .expect("first");
    assert_eq!(
        session.append(Span::new(1, SpanKind::Tool)),
        Err(CaptureError::DuplicateSequence {
            session: "s".into(),
            seq: 1
        })
    );
}

#[test]
fn a_gapped_session_names_the_sequence_numbers_that_are_missing() {
    let mut session = CaptureSession::new("s");
    session
        .append(Span::new(1, SpanKind::Execution))
        .expect("ok");
    session.append(Span::new(4, SpanKind::Tool)).expect("ok");
    assert_eq!(
        session.completeness(),
        Completeness::Gapped {
            missing: vec![2, 3]
        }
    );
    assert!(!session.completeness().is_complete());
}

#[test]
fn a_gapped_session_is_refused_by_the_completeness_gate() {
    let mut session = CaptureSession::new("s");
    session
        .append(Span::new(1, SpanKind::Execution))
        .expect("ok");
    session.append(Span::new(9, SpanKind::Tool)).expect("ok");
    assert!(matches!(
        session.require_complete(),
        Err(CaptureError::GappedSession { .. })
    ));
}

#[test]
fn capture_that_started_late_is_not_reported_as_a_gap() {
    let mut session = CaptureSession::new("s");
    session
        .append(Span::new(100, SpanKind::Execution))
        .expect("ok");
    session.append(Span::new(101, SpanKind::Tool)).expect("ok");
    assert_eq!(session.completeness(), Completeness::Complete { spans: 2 });
    assert!(
        session.require_complete().is_ok(),
        "nothing here can know whether capture started late, and guessing would invent evidence \
         of loss"
    );
}

#[test]
fn an_empty_session_is_complete_and_says_so_with_zero_spans() {
    let session = CaptureSession::new("s");
    assert!(session.is_empty());
    assert_eq!(session.completeness(), Completeness::Complete { spans: 0 });
}

#[test]
fn redaction_removes_values_and_never_removes_spans() {
    let mut session = CaptureSession::new("s");
    session
        .append(Span::new(1, SpanKind::Execution).with("subject", Field::recorded(SECRET)))
        .expect("ok");
    session
        .append(Span::new(3, SpanKind::Tool).with("subject", Field::recorded(SECRET)))
        .expect("ok");
    let released = session.redact(&policy()).expect("policy applies");
    assert_eq!(released.len(), 2);
    assert_eq!(
        released.completeness(),
        Completeness::Gapped { missing: vec![2] },
        "a gap survives redaction; withholding a value is not the same as losing a span"
    );
}

#[test]
fn deserializing_an_out_of_order_capture_session_is_refused() {
    let session = session_with(SECRET);
    let mut document = serde_json::to_value(&session).expect("serialisable");
    let spans = document["spans"].as_array().expect("spans array");
    document["spans"] = json!([spans[1].clone(), spans[0].clone()]);

    let error = serde_json::from_value::<CaptureSession>(document).expect_err("must refuse");
    assert!(error.to_string().contains("follows"), "{error}");
}

#[test]
fn deserializing_redacted_session_with_forged_accounting_is_refused() {
    let released = session_with(SECRET)
        .redact(&policy())
        .expect("policy applies");
    let mut document = serde_json::to_value(&released).expect("serialisable");
    document["redacted_fields"] = json!(released.redacted_fields + 1);

    let error = serde_json::from_value::<bioprism_megafactory::RedactedSession>(document)
        .expect_err("must refuse");
    assert!(
        error
            .to_string()
            .contains("inconsistent redaction accounting"),
        "{error}"
    );
}

#[test]
fn the_span_kinds_describe_instrumentation_rather_than_decisions() {
    assert_eq!(SpanKind::ALL.len(), 5);
    let names: Vec<&str> = SpanKind::ALL.iter().map(|kind| kind.as_str()).collect();
    assert!(names.contains(&"annotation"));
    assert!(names.contains(&"snapshot"));
}
