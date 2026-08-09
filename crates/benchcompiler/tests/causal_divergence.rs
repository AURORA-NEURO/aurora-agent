//! Invariants of 06.05 first causal divergence.
//!
//! The distinction these tests defend is the one `bioprism_trace` established and this crate
//! inherits: a difference the agent produced and a difference that happened to it are not the same
//! finding, and only the first can host a cell.

use bioprism_benchcompiler::causal::{analyse, CausalVerdict};
use bioprism_benchcompiler::CausalError;
use bioprism_trace::{Divergence, Event, EventKind, Trace};
use serde_json::json;

fn goal(step: usize) -> Event {
    Event::new(step, EventKind::Goal, json!({"summary": "rank the candidates"}))
}

fn action(step: usize, tool: &str, caused_by: usize) -> Event {
    Event::new(step, EventKind::Action, json!({"tool": tool})).caused_by(caused_by)
}

fn irreversible_action(step: usize, tool: &str, caused_by: usize) -> Event {
    Event::new(
        step,
        EventKind::Action,
        json!({"tool": tool, "irreversible": true}),
    )
    .caused_by(caused_by)
}

fn result(step: usize, summary: &str, caused_by: usize) -> Event {
    Event::new(step, EventKind::Result, json!({"summary": summary})).caused_by(caused_by)
}

fn observation(step: usize, summary: &str, caused_by: usize) -> Event {
    Event::new(step, EventKind::Observation, json!({"summary": summary})).caused_by(caused_by)
}

fn termination(step: usize, caused_by: usize) -> Event {
    Event::new(step, EventKind::Termination, json!({"summary": "gave up"})).caused_by(caused_by)
}

/// A chain where the runs part company at step 3, an action, and the failure surfaces at step 5.
fn chain(tool_at_three: &str) -> Vec<Event> {
    vec![
        goal(0),
        irreversible_action(1, "choose_assay", 0),
        result(2, "assay selected", 1),
        action(3, tool_at_three, 2),
        Event::new(4, EventKind::Claim, json!({"summary": "reported a hit"})).caused_by(3),
        termination(5, 4),
    ]
}

#[test]
fn a_divergence_that_lands_on_an_observation_is_not_localised_to_the_agent() {
    let failing = Trace::new(
        "run_fail",
        vec![
            goal(0),
            action(1, "search", 0),
            observation(2, "the index returned 4 hits", 1),
            action(3, "summarise", 2),
            termination(4, 3),
        ],
        false,
    );
    let passing = Trace::new(
        "run_pass",
        vec![
            goal(0),
            action(1, "search", 0),
            observation(2, "the index returned 91 hits", 1),
            action(3, "summarise", 2),
            termination(4, 3),
        ],
        true,
    );

    let analysis = analyse(&failing, Some(&passing)).expect("both traces are analysable");

    assert!(!analysis.textual_is_actionable);
    assert!(analysis.refuses_to_localise());
    assert_eq!(analysis.first_causal_step(), None);
    match analysis.verdict {
        CausalVerdict::EnvironmentDivergence { at_step, kind, .. } => {
            assert_eq!(at_step, 2);
            assert_eq!(kind, "observation");
        }
        other => panic!("expected the environment to be named, got {other:?}"),
    }
}

#[test]
fn localise_to_refuses_a_step_the_agent_did_not_control() {
    let failing = Trace::new("run_fail", chain("run_wrong_panel"), false);
    let passing = Trace::new("run_pass", chain("run_right_panel"), true);
    let analysis = analyse(&failing, Some(&passing)).expect("analysable");

    let error = analysis
        .localise_to(&failing, 2)
        .expect_err("step 2 is a tool result; the agent had no alternative there");
    assert!(matches!(
        error,
        CausalError::NotAgentControlled {
            step: 2,
            kind: "result"
        }
    ));
    assert_eq!(analysis.localise_to(&failing, 3), Ok(3));
}

#[test]
fn the_cell_is_placed_at_the_causal_step_not_where_the_failure_became_visible() {
    let failing = Trace::new("run_fail", chain("run_wrong_panel"), false);
    let passing = Trace::new("run_pass", chain("run_right_panel"), true);
    let analysis = analyse(&failing, Some(&passing)).expect("analysable");

    assert_eq!(analysis.terminal_step, 5);
    assert_eq!(analysis.first_causal_step(), Some(3));
    assert!(matches!(
        analysis.verdict,
        CausalVerdict::FirstCausal { step: 3, .. }
    ));
}

#[test]
fn a_candidate_downstream_of_an_earlier_divergent_decision_is_not_named_first() {
    let failing = Trace::new(
        "run_fail",
        vec![
            goal(0),
            irreversible_action(1, "pick_cohort_a", 0),
            result(2, "cohort loaded", 1),
            irreversible_action(3, "run_panel", 2),
            Event::new(4, EventKind::Claim, json!({"summary": "reported"})).caused_by(3),
            termination(5, 4),
        ],
        false,
    );
    let passing = Trace::new(
        "run_pass",
        vec![
            goal(0),
            irreversible_action(1, "pick_cohort_b", 0),
            result(2, "cohort loaded", 1),
            irreversible_action(3, "run_other_panel", 2),
            Event::new(4, EventKind::Claim, json!({"summary": "reported"})).caused_by(3),
            termination(5, 4),
        ],
        true,
    );

    let analysis = analyse(&failing, Some(&passing)).expect("analysable");

    assert_eq!(analysis.first_causal_step(), Some(1));
    let downstream = analysis
        .candidates
        .iter()
        .find(|candidate| candidate.step == 3)
        .expect("step 3 is still reported as a candidate");
    assert_eq!(downstream.upstream_unresolved, Some(1));
}

#[test]
fn two_causes_neither_upstream_of_the_other_are_reported_as_a_conjunction() {
    let build = |left: &str, right: &str| {
        vec![
            goal(0),
            irreversible_action(1, left, 0),
            Event::new(2, EventKind::Observation, json!({"summary": "panel ready"}))
                .caused_by(1)
                .seeing(["panel"]),
            irreversible_action(3, right, 0),
            Event::new(4, EventKind::Claim, json!({"summary": "reported"}))
                .caused_by(3)
                .seeing(["panel"]),
            termination(5, 4),
        ]
    };
    let failing = Trace::new("run_fail", build("pick_wrong_panel", "wrong_threshold"), false);
    let passing = Trace::new("run_pass", build("pick_right_panel", "right_threshold"), true);

    let analysis = analyse(&failing, Some(&passing)).expect("analysable");

    match &analysis.verdict {
        CausalVerdict::Conjunction { steps } => assert_eq!(steps, &vec![1, 3]),
        other => panic!("evidence supports two causes; forcing one would be wrong: {other:?}"),
    }
}

#[test]
fn with_no_reference_trajectory_the_analysis_declines_to_name_a_divergence() {
    let failing = Trace::new("run_fail", chain("run_wrong_panel"), false);
    let analysis = analyse(&failing, None).expect("a single trace is still analysable");

    assert_eq!(analysis.reference, None);
    assert!(analysis
        .candidates
        .iter()
        .all(|candidate| candidate.score.counterfactual_effect == 0.0));
    assert!(matches!(
        analysis.verdict,
        CausalVerdict::Unlocalizable { .. }
    ));
}

#[test]
fn identical_runs_report_no_divergence_rather_than_a_weakest_candidate() {
    let failing = Trace::new("run_a", chain("run_panel"), false);
    let passing = Trace::new("run_b", chain("run_panel"), true);
    let analysis = analyse(&failing, Some(&passing)).expect("analysable");

    assert_eq!(analysis.textual, Divergence::Identical);
    assert!(matches!(analysis.verdict, CausalVerdict::NoDivergence));
    assert_eq!(analysis.first_causal_step(), None);
}

#[test]
fn the_textual_divergence_is_carried_through_exactly_as_the_trace_crate_reported_it() {
    let failing = Trace::new("run_fail", chain("run_wrong_panel"), false);
    let passing = Trace::new("run_pass", chain("run_right_panel"), true);
    let analysis = analyse(&failing, Some(&passing)).expect("analysable");

    assert_eq!(
        analysis.textual,
        bioprism_trace::first_divergence(&failing, &passing),
        "a reader must be able to see where the causal answer departs from the diff"
    );
}

#[test]
fn an_empty_failing_trace_has_no_terminal_failure_to_explain() {
    let failing = Trace::new("run_fail", vec![], false);
    assert_eq!(
        analyse(&failing, None),
        Err(CausalError::NoTerminalFailure)
    );
}

#[test]
fn a_trace_of_pure_observation_offers_no_step_a_cell_could_sit_on() {
    let failing = Trace::new(
        "run_fail",
        vec![goal(0), observation(1, "saw something", 0), termination(2, 1)],
        false,
    );
    assert_eq!(
        analyse(&failing, None),
        Err(CausalError::NoDecisionBearingStep)
    );
}

#[test]
fn each_rank_component_is_reported_separately_so_a_reviewer_can_reject_one() {
    let failing = Trace::new("run_fail", chain("run_wrong_panel"), false);
    let passing = Trace::new("run_pass", chain("run_right_panel"), true);
    let analysis = analyse(&failing, Some(&passing)).expect("analysable");

    let top = analysis.candidates.first().expect("at least one candidate");
    let recomputed = top.score.necessity * bioprism_benchcompiler::causal::WEIGHT_NECESSITY
        + top.score.counterfactual_effect * bioprism_benchcompiler::causal::WEIGHT_COUNTERFACTUAL
        + top.score.irreversibility * bioprism_benchcompiler::causal::WEIGHT_IRREVERSIBILITY
        + top.score.explanatory_simplicity * bioprism_benchcompiler::causal::WEIGHT_SIMPLICITY;
    assert!((recomputed - top.score.total).abs() < 1e-12);
    assert!(
        !top.score.irreversibility_declared,
        "step 3 declares nothing, so its irreversibility is this crate's default and must say so"
    );
}

#[test]
fn analysis_round_trips_through_json_without_losing_the_verdict() {
    let failing = Trace::new("run_fail", chain("run_wrong_panel"), false);
    let passing = Trace::new("run_pass", chain("run_right_panel"), true);
    let analysis = analyse(&failing, Some(&passing)).expect("analysable");

    let encoded = serde_json::to_string(&analysis).expect("serialisable");
    let decoded: bioprism_benchcompiler::CausalAnalysis =
        serde_json::from_str(&encoded).expect("round-trips");
    assert_eq!(decoded, analysis);
}
