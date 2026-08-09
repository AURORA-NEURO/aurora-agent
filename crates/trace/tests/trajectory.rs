//! Ingestion, divergence localization, segmentation and the Gate 2 approval requirement.

use bioprism_prism::InputRef;
use bioprism_section::OracleStatus;
use bioprism_trace::{
    excluded, first_divergence, from_jsonl, is_actionable, review_reduction, segment, validate,
    CellProposal, Divergence, Event, EventKind, Trace, TraceError,
};
use serde_json::json;

fn line(step: usize, kind: &str, payload: serde_json::Value, visible: &[&str]) -> String {
    json!({ "step": step, "kind": kind, "payload": payload, "visible": visible }).to_string()
}

fn failing_trace() -> Trace {
    Trace::new(
        "run-failing",
        vec![
            Event::new(0, EventKind::Goal, json!({ "summary": "audit the split" })),
            Event::new(1, EventKind::Observation, json!({ "summary": "read manifest" }))
                .seeing(["cohort_id"]),
            Event::new(
                2,
                EventKind::Action,
                json!({ "tool": "grep", "summary": "search for duplicates", "alternatives": ["grep", "load-aliases"] }),
            )
            .seeing(["cohort_id"]),
            Event::new(3, EventKind::Result, json!({ "summary": "no matches" })),
            Event::new(4, EventKind::Claim, json!({ "summary": "split is valid" })),
            Event::new(5, EventKind::Termination, json!({ "summary": "done" })),
        ],
        false,
    )
}

fn passing_trace() -> Trace {
    Trace::new(
        "run-passing",
        vec![
            Event::new(0, EventKind::Goal, json!({ "summary": "audit the split" })),
            Event::new(1, EventKind::Observation, json!({ "summary": "read manifest" }))
                .seeing(["cohort_id"]),
            Event::new(
                2,
                EventKind::Action,
                json!({ "tool": "load-aliases", "summary": "load the alias table", "alternatives": ["grep", "load-aliases"] }),
            )
            .seeing(["cohort_id", "subject_aliases"]),
            Event::new(3, EventKind::Result, json!({ "summary": "ALT-77 shared" })),
            Event::new(4, EventKind::Claim, json!({ "summary": "split leaks identity" })),
            Event::new(5, EventKind::Termination, json!({ "summary": "done" })),
        ],
        true,
    )
}

#[test]
fn jsonl_ingestion_reports_what_it_could_not_carry() {
    let text = [
        line(0, "goal", json!({ "summary": "start" }), &[]),
        line(1, "action", json!({ "tool": "read" }), &["a"]),
        "{ not json at all".to_string(),
        json!({ "step": 3, "kind": "telepathy", "payload": {} }).to_string(),
        json!({ "step": 4, "kind": "claim", "payload": {}, "latency_ms": 12 }).to_string(),
    ]
    .join("\n");

    let ingestion = from_jsonl("t", &text, false);
    let loss = ingestion.loss();

    assert_eq!(loss.unparsed_lines, vec![3]);
    assert_eq!(loss.untyped_events.len(), 1);
    assert_eq!(loss.untyped_events[0].found, "telepathy");
    assert_eq!(loss.unmapped_fields.len(), 1);
    assert_eq!(loss.unmapped_fields[0].field, "latency_ms");
    assert!(!loss.is_lossless());
    assert_eq!(ingestion.trace().len(), 3);
}

#[test]
fn an_unrecognised_kind_is_dropped_rather_than_guessed() {
    let text = json!({ "step": 0, "kind": "wibble", "payload": {} }).to_string();
    let ingestion = from_jsonl("t", &text, false);
    assert!(ingestion.trace().is_empty(), "a mistyped event must not be invented into an observation");
    assert_eq!(ingestion.loss().dropped_events(), 1);
}

#[test]
fn a_trace_that_dropped_events_is_not_compilable() {
    let clean = from_jsonl("t", &line(0, "action", json!({}), &[]), false);
    assert!(clean.is_compilable());

    let lossy = from_jsonl("t", "{ broken", false);
    assert!(
        !lossy.is_compilable(),
        "segmenting a sequence with holes would freeze a state the agent never occupied"
    );
}

#[test]
fn a_lossless_import_asserts_completeness_rather_than_saying_nothing() {
    let text = [
        line(0, "goal", json!({}), &[]),
        line(1, "action", json!({ "tool": "x" }), &[]),
    ]
    .join("\n");
    let ingestion = from_jsonl("t", &text, true);
    assert!(ingestion.loss().is_lossless());
    assert_eq!(ingestion.loss().dropped_events(), 0);
}

#[test]
fn structural_defects_in_ordering_are_rejected() {
    let duplicated = Trace::new(
        "t",
        vec![
            Event::new(1, EventKind::Action, json!({})),
            Event::new(1, EventKind::Action, json!({})),
        ],
        false,
    );
    assert!(matches!(
        validate(&duplicated),
        Err(TraceError::DuplicateStep { step: 1 })
    ));

    let backwards = Trace::new(
        "t",
        vec![
            Event::new(5, EventKind::Action, json!({})),
            Event::new(2, EventKind::Action, json!({})),
        ],
        false,
    );
    assert!(matches!(
        validate(&backwards),
        Err(TraceError::NonMonotonicStep { .. })
    ));

    let impossible = Trace::new(
        "t",
        vec![Event::new(2, EventKind::Action, json!({})).caused_by(7)],
        false,
    );
    assert!(matches!(
        validate(&impossible),
        Err(TraceError::CausalParentNotEarlier { .. })
    ));

    assert!(validate(&failing_trace()).is_ok());
}

#[test]
fn divergence_localises_the_step_where_the_runs_parted() {
    let failing = failing_trace();
    let passing = passing_trace();

    match first_divergence(&failing, &passing) {
        Divergence::Diverged {
            failing_step,
            common_prefix,
            visibility_gap,
            failing_did,
            passing_did,
            ..
        } => {
            assert_eq!(failing_step, 2, "the tool choice, not the claim it led to");
            assert_eq!(common_prefix, 2);
            assert_eq!(visibility_gap, vec!["subject_aliases".to_string()]);
            assert!(failing_did.contains("grep"));
            assert!(passing_did.contains("load-aliases"));
        }
        other => panic!("expected a localised divergence, got {other:?}"),
    }
}

/// The failure is *visible* at the claim in step 4, but the decision was made at step 2.
#[test]
fn divergence_points_at_the_decision_not_where_the_failure_surfaced() {
    let divergence = first_divergence(&failing_trace(), &passing_trace());
    assert_eq!(divergence.failing_step(), Some(2));
    assert_ne!(
        divergence.failing_step(),
        Some(4),
        "step 4 is where the wrong claim appears; step 2 is where it became inevitable"
    );
}

/// Content-based alignment, so one insertion is one difference.
#[test]
fn an_inserted_event_does_not_report_the_whole_remainder_as_divergent() {
    let base = failing_trace();
    let mut shifted = base.clone();
    shifted.events.insert(
        1,
        Event::new(99, EventKind::Observation, json!({ "summary": "extra log line" })),
    );

    match first_divergence(&shifted, &base) {
        Divergence::Diverged { common_prefix, .. } => {
            assert_eq!(common_prefix, 1, "divergence begins at the insertion, not before it");
        }
        other => panic!("expected divergence, got {other:?}"),
    }
}

#[test]
fn identical_runs_report_identical_rather_than_a_false_divergence() {
    let trace = failing_trace();
    assert_eq!(first_divergence(&trace, &trace.clone()), Divergence::Identical);
    assert!(!first_divergence(&trace, &trace).is_localised());
}

#[test]
fn a_run_that_stopped_early_is_reported_as_such() {
    let full = failing_trace();
    let mut truncated = full.clone();
    truncated.events.truncate(3);
    truncated.trace_id = "run-short".into();

    match first_divergence(&truncated, &full) {
        Divergence::EarlyTermination {
            shorter,
            longer_continued_for,
            ..
        } => {
            assert_eq!(shorter, "run-short");
            assert_eq!(longer_continued_for, 3);
        }
        other => panic!("expected early termination, got {other:?}"),
    }
}

#[test]
fn only_decision_bearing_steps_can_host_a_cell() {
    let trace = failing_trace();
    let candidates = segment(&trace, Some(2));

    assert!(candidates.iter().all(|c| c.kind == "action" || c.kind == "choice"));
    assert!(
        !candidates.iter().any(|c| c.step == 3),
        "a tool result is something that happened to the agent, not a decision"
    );

    let skipped = excluded(&trace);
    assert!(skipped.iter().any(|(step, _)| *step == 3));
    assert!(skipped
        .iter()
        .any(|(_, reason)| reason.contains("no alternative was available")));
}

#[test]
fn a_divergence_at_a_result_step_is_not_actionable() {
    let mut failing = failing_trace();
    let mut passing = passing_trace();
    // Make the runs agree at the action and differ only at the result the tool returned.
    passing.events[2] = failing.events[2].clone();
    failing.events[3] = Event::new(3, EventKind::Result, json!({ "summary": "flaky timeout" }));

    let divergence = first_divergence(&failing, &passing);
    assert_eq!(divergence.failing_step(), Some(3));
    assert!(
        !is_actionable(&divergence, &failing),
        "no architecture had a decision to make there, so a cell would measure nothing"
    );
}

#[test]
fn the_divergence_step_outranks_structurally_similar_candidates() {
    let trace = failing_trace();
    let ranked = segment(&trace, Some(2));
    assert_eq!(ranked[0].step, 2);
    assert!(ranked[0].score.is_divergence);
    assert!(ranked[0].score.total > 0.5);
}

#[test]
fn every_score_component_is_visible_to_a_reviewer() {
    let candidate = &segment(&failing_trace(), Some(2))[0];
    assert_eq!(candidate.score.alternatives, 2);
    assert_eq!(candidate.score.downstream_steps, 3);
    assert!(candidate.score.is_divergence);
    assert!(
        candidate.score.total > 0.0,
        "a ranker a reviewer cannot audit is exactly what Gate 2 forbids"
    );
}

#[test]
fn segmentation_reduces_what_a_reviewer_must_read() {
    let trace = failing_trace();
    let candidates = segment(&trace, Some(2));
    let reduction = review_reduction(&trace, &candidates);
    assert!(reduction > 0.5, "6 steps to 1 candidate, got {reduction}");

    let all_actions = Trace::new(
        "t",
        (0..4)
            .map(|n| Event::new(n, EventKind::Action, json!({})))
            .collect(),
        false,
    );
    assert_eq!(
        review_reduction(&all_actions, &segment(&all_actions, None)),
        0.0,
        "a trace that is all decisions offers no reduction, and must not claim one"
    );
}

fn refs() -> (InputRef, InputRef) {
    (
        InputRef::new("w.json", &json!({ "world": 1 })),
        InputRef::new("q.json", &json!({ "query": 1 })),
    )
}

/// Gate 2, enforced by the type system.
#[test]
fn a_proposal_becomes_a_cell_only_through_a_named_reviewer() {
    let trace = failing_trace();
    let divergence = first_divergence(&trace, &passing_trace());
    let candidate = &segment(&trace, divergence.failing_step())[0];
    let proposal = CellProposal::from_candidate(&trace, candidate, Some(&divergence)).unwrap();

    let (world, query) = refs();
    assert!(matches!(
        proposal.clone().approve("", world.clone(), query.clone(), OracleStatus::Invalid),
        Err(TraceError::UnattributedApproval)
    ));
    assert!(matches!(
        proposal.clone().approve("   ", world.clone(), query.clone(), OracleStatus::Invalid),
        Err(TraceError::UnattributedApproval)
    ));

    let approved = proposal
        .approve("m.ambati", world, query, OracleStatus::Invalid)
        .expect("a named reviewer may approve");
    assert_eq!(approved.reviewer, "m.ambati");
    assert!(approved.cell.cell_id.starts_with("dc_"));
}

#[test]
fn an_approved_cell_carries_the_trajectory_it_came_from() {
    let trace = failing_trace();
    let divergence = first_divergence(&trace, &passing_trace());
    let candidate = &segment(&trace, divergence.failing_step())[0];
    let (world, query) = refs();

    let approved = CellProposal::from_candidate(&trace, candidate, Some(&divergence))
        .unwrap()
        .approve("reviewer", world, query, OracleStatus::Invalid)
        .unwrap();

    let provenance = approved.provenance();
    assert_eq!(provenance["from_trace"], json!("run-failing"));
    assert_eq!(provenance["at_step"], json!(2));
    assert_eq!(
        provenance["trace_digest"],
        json!(trace.digest().as_str()),
        "the cell must name the exact trajectory revision it was cut from"
    );
    assert_eq!(provenance["approval_digest"].as_str().unwrap().len(), 64);
}

#[test]
fn a_proposal_explains_itself_to_the_reviewer() {
    let trace = failing_trace();
    let divergence = first_divergence(&trace, &passing_trace());
    let candidate = &segment(&trace, divergence.failing_step())[0];
    let proposal = CellProposal::from_candidate(&trace, candidate, Some(&divergence)).unwrap();

    assert!(proposal.rationale.contains("first divergence"));
    assert!(proposal.what_the_run_did.contains("search for duplicates"));
    assert!(proposal
        .what_a_passing_run_did
        .as_deref()
        .unwrap_or_default()
        .contains("load-aliases"));
    assert_eq!(proposal.visibility_gap, vec!["subject_aliases".to_string()]);
}

#[test]
fn a_proposal_cannot_name_a_step_outside_its_trace() {
    let trace = failing_trace();
    let mut candidate = segment(&trace, None)[0].clone();
    candidate.step = 999;
    assert!(matches!(
        CellProposal::from_candidate(&trace, &candidate, None),
        Err(TraceError::StepNotInTrace { step: 999 })
    ));
}

#[test]
fn the_whole_pipeline_runs_from_jsonl_to_an_approved_cell() {
    let text = [
        line(0, "goal", json!({ "summary": "audit the split" }), &[]),
        line(1, "observation", json!({ "summary": "read manifest" }), &["cohort_id"]),
        line(2, "action", json!({ "tool": "grep", "summary": "search", "alternatives": ["grep", "load"] }), &["cohort_id"]),
        line(3, "result", json!({ "summary": "none" }), &["cohort_id"]),
        line(4, "claim", json!({ "summary": "valid" }), &["cohort_id"]),
    ]
    .join("\n");

    let ingestion = from_jsonl("run-1", &text, false);
    assert!(ingestion.is_compilable());
    let (trace, _) = ingestion.into_parts();
    validate(&trace).unwrap();

    let divergence = first_divergence(&trace, &passing_trace());
    assert!(divergence.is_localised());
    assert_eq!(
        divergence.failing_step(),
        Some(2),
        "the prefix must align exactly, or divergence lands on an observation and is not actionable"
    );
    assert!(is_actionable(&divergence, &trace));

    let candidates = segment(&trace, divergence.failing_step());
    let proposal = CellProposal::from_candidate(&trace, &candidates[0], Some(&divergence)).unwrap();
    let (world, query) = refs();
    let approved = proposal
        .approve("reviewer", world, query, OracleStatus::Invalid)
        .unwrap();

    assert_eq!(approved.proposal.step, 2);
    assert!(approved.cell.acceptable_verdicts.contains("invalid"));
}
