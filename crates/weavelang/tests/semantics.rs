//! Operational-semantics invariants (blueprint 23.34).

use bioprism_weavelang::compile::compile;
use bioprism_weavelang::diagnostic::Diagnostic;
use bioprism_weavelang::ir::WeaveIr;
use bioprism_weavelang::reference::COMPLETE_PROGRAM;
use bioprism_weavelang::semantics::{
    ExecutionMode, Invariant, Machine, SemanticsError, Step, Trace,
};
use bioprism_weave::Resource;

const MUTATING_PROGRAM: &str = r#"
policy repair {
  allow effects [repo.read, tests.pass, branch.write]
  budget tokens <= 40000
  budget tool-calls <= 32
}
role Patcher { provides [patch@1] requires [branch.write, tests.pass] }
role Lead { provides [plan@1] requires [repo.read] }
weave repair-issue(issue: Issue) -> Report using repair {
  bind patcher to role Patcher
  bind lead to role Lead
  commit patcher to lead when task.accepted {
    deliver Patch before 15m
    satisfy with tests.pass
    compensate revert-branch on violation
  }
}
"#;

fn run(source: &str, mode: ExecutionMode) -> Result<Trace, SemanticsError> {
    let ir = compile(source).expect("the fixture must compile");
    Machine::load(ir, mode, "thread:test").run()
}

fn reference_ir() -> WeaveIr {
    compile(COMPLETE_PROGRAM).expect("the reference program compiles")
}

#[test]
fn the_same_program_and_the_same_inputs_produce_the_same_event_sequence_and_digest() {
    let first = run(COMPLETE_PROGRAM, ExecutionMode::Replay).expect("runs");
    let second = run(COMPLETE_PROGRAM, ExecutionMode::Replay).expect("runs");

    assert_eq!(first.digest, second.digest);
    assert_eq!(first.events, second.events);
    assert!(!first.events.is_empty(), "the reference program does something");
    assert_eq!(first.program_id, second.program_id);
}

#[test]
fn an_event_identifier_is_derived_from_content_and_never_minted() {
    let trace = run(COMPLETE_PROGRAM, ExecutionMode::Replay).expect("runs");
    for event in &trace.events {
        assert!(
            event.event_id.starts_with("urn:weave:event:sha256:"),
            "23.38 asks for urn:uuid; a UUID needs randomness, so identity is content-derived"
        );
        assert_eq!(
            event.time, None,
            "the compiler has no clock and must not invent a timestamp"
        );
    }
}

#[test]
fn every_event_names_its_causal_parent_and_carries_a_monotone_logical_clock() {
    let trace = run(COMPLETE_PROGRAM, ExecutionMode::Replay).expect("runs");
    assert!(trace.events[0].causal_parents.is_empty());
    for pair in trace.events.windows(2) {
        assert_eq!(pair[1].causal_parents, vec![pair[0].event_id.clone()]);
        assert_eq!(pair[1].logical_clock, pair[0].logical_clock + 1);
    }
}

#[test]
fn a_replay_refuses_a_transition_that_would_run_the_tests() {
    let error = run(MUTATING_PROGRAM, ExecutionMode::Replay)
        .expect_err("discharging the commitment runs tests");
    let SemanticsError::ReplayWouldMutateWorld {
        transition,
        effects,
    } = &error
    else {
        panic!("expected a replay-safety refusal, got {error:?}");
    };
    assert_eq!(transition, "commit-discharge-patcher");
    assert_eq!(effects, &vec!["tests.pass".to_string()]);
    assert_eq!(error.code(), "WEAVE-E5002");
}

#[test]
fn the_same_program_runs_when_live_mode_is_selected_by_name() {
    let trace = run(MUTATING_PROGRAM, ExecutionMode::Live).expect("live mode permits the effect");
    assert!(trace
        .events
        .iter()
        .any(|event| event.event_type == "aurora.weave.act.discharge.v1"));
}

#[test]
fn a_read_effect_is_not_a_production_side_effect_and_replays_freely() {
    let trace = run(COMPLETE_PROGRAM, ExecutionMode::Replay).expect("reads replay");
    assert!(trace
        .events
        .iter()
        .any(|event| event.event_type == "aurora.weave.act.ask.v1"));
}

#[test]
fn every_transition_is_charged_against_the_programs_budget() {
    let ir = reference_ir();
    let mut machine = Machine::load(ir, ExecutionMode::Replay, "thread:test");
    let before = machine.remaining(Resource::ToolCalls);
    let trace = machine.run().expect("runs");
    let after = machine.remaining(Resource::ToolCalls);
    assert_eq!(
        before - after,
        trace.events.len() as u64,
        "one tool call per transition, and no transition uncharged"
    );
}

#[test]
fn a_program_whose_budget_runs_out_mid_run_halts_rather_than_overdrawing() {
    let source = COMPLETE_PROGRAM.replace("budget tool-calls <= 64", "budget tool-calls <= 2");
    let error = run(&source, ExecutionMode::Replay).expect_err("two tool calls is not enough");
    let SemanticsError::BudgetRefused { transition, .. } = &error else {
        panic!("expected a budget refusal, got {error:?}");
    };
    assert!(!transition.is_empty());
    assert_eq!(error.code(), "WEAVE-E5004");
}

#[test]
fn a_program_with_no_tool_call_allowance_cannot_be_charged_and_says_so() {
    let source = COMPLETE_PROGRAM.replace("  budget tool-calls <= 64\n", "");
    let error = run(&source, ExecutionMode::Replay).expect_err("nothing to charge against");
    assert_eq!(error.code(), "WEAVE-E5003");
}

#[test]
fn every_safety_property_holds_over_a_completed_reference_run() {
    let ir = reference_ir();
    let mut machine = Machine::load(ir, ExecutionMode::Replay, "thread:test");
    machine.run().expect("runs");
    let violations = machine.check_invariants();
    assert!(
        violations.is_empty(),
        "23.34's safety properties must all hold: {violations:?}"
    );
}

#[test]
fn a_world_mutating_transition_with_no_covering_grant_violates_authority_safety() {
    let mut ir = compile(MUTATING_PROGRAM).expect("compiles");
    // Strip the grants without touching the transitions. The lowering cannot produce this state,
    // because it rejects an effect no role declared; the check exists so that a WeaveIR document
    // arriving from anywhere else cannot smuggle one past the evaluator either.
    for role in &mut ir.roles {
        role.requires_effects.clear();
    }

    let mut machine = Machine::load(ir, ExecutionMode::Live, "thread:test");
    let error = machine
        .run()
        .expect_err("discharging runs tests with no covering grant");
    let SemanticsError::InvariantViolated { violation } = &error else {
        panic!("expected an invariant violation, got {error:?}");
    };
    assert_eq!(violation.invariant, Invariant::AuthoritySafety);
    assert!(violation.detail.contains("tests.pass"));
    assert_eq!(error.code(), "WEAVE-E5001");
}

#[test]
fn a_delegated_grant_never_carries_more_than_the_grant_it_came_from() {
    let ir = compile(MUTATING_PROGRAM).expect("compiles");
    let mut machine = Machine::load(ir, ExecutionMode::Live, "thread:test");
    machine.run().expect("runs");
    assert!(
        !machine
            .check_invariants()
            .iter()
            .any(|violation| violation.invariant == Invariant::DelegationAttenuation),
        "a child grant is built by intersecting with its parent, so it cannot exceed it"
    );
}

#[test]
fn what_a_run_consumed_plus_what_remains_never_exceeds_what_was_issued() {
    let ir = compile(MUTATING_PROGRAM).expect("compiles");
    let issued = ir
        .policies
        .values()
        .flat_map(|policy| policy.budgets.iter())
        .find(|budget| budget.resource == Resource::ToolCalls)
        .map(|budget| budget.limit)
        .expect("the fixture allocates tool calls");

    let mut machine = Machine::load(ir, ExecutionMode::Live, "thread:test");
    let trace = machine.run().expect("runs");
    let consumed = trace.events.len() as u64;
    assert_eq!(consumed + machine.remaining(Resource::ToolCalls), issued);
    assert!(machine
        .check_invariants()
        .iter()
        .all(|violation| violation.invariant != Invariant::BudgetConservation));
}

#[test]
fn a_participant_cleared_below_the_threads_accumulated_label_violates_non_escalation() {
    let source = r#"
policy p {
  allow effects [notes.read]
  budget tokens <= 100
  budget tool-calls <= 8
}
role Secret { provides [hold@1] requires [notes.read] clearance restricted/patient-data }
role Public { provides [tell@1] requires [notes.read] clearance public }
weave leak() -> Report using p {
  bind secret to role Secret
  bind open to role Public
  let a = ask secret.look()
  let b = ask open.look()
}
"#;
    let ir = compile(source).expect("compiles");
    let mut machine = Machine::load(ir, ExecutionMode::Replay, "thread:test");
    let error = machine
        .run()
        .expect_err("a public role must not act after a restricted one");
    let SemanticsError::InvariantViolated { violation } = &error else {
        panic!("expected an invariant violation, got {error:?}");
    };
    assert_eq!(violation.invariant, Invariant::InformationNonEscalation);
}

#[test]
fn a_challenged_claim_stays_in_the_epistemic_ledger_alongside_its_challenge() {
    let source = r#"
policy p {
  allow effects [evidence.read]
  budget tokens <= 100
  budget tool-calls <= 8
}
role Claimant { provides [assert@1] requires [evidence.read] }
role Skeptic { provides [challenge@1] requires [evidence.read] }
weave dispute() -> Report using p {
  bind claimant to role Claimant
  bind skeptic to role Skeptic
  send claim(finding) from claimant to skeptic
  send challenge(finding) from skeptic to claimant
}
"#;
    let ir = compile(source).expect("compiles");
    let mut machine = Machine::load(ir, ExecutionMode::Replay, "thread:test");
    machine.run().expect("runs");

    assert_eq!(machine.epistemic().len(), 1, "the claim is not deleted");
    assert_eq!(
        machine.epistemic()[0].challenges.len(),
        1,
        "the challenge is recorded against it, not in place of it"
    );
}

#[test]
fn a_commitment_records_a_debtor_and_a_creditor_and_is_discharged_by_a_later_transition() {
    let ir = compile(MUTATING_PROGRAM).expect("compiles");
    let mut machine = Machine::load(ir, ExecutionMode::Live, "thread:test");
    machine.run().expect("runs");

    let commitment = machine
        .commitments()
        .values()
        .next()
        .expect("the commit statement creates one");
    assert!(!commitment.debtor.is_empty());
    assert!(!commitment.creditor.is_empty());
    assert!(commitment.discharged);
    assert_eq!(commitment.quality_predicates, vec!["tests.pass".to_string()]);
}

#[test]
fn a_run_halts_at_a_state_with_no_enabled_transition() {
    let ir = reference_ir();
    let mut machine = Machine::load(ir, ExecutionMode::Replay, "thread:test");
    loop {
        match machine.step().expect("steps") {
            Step::Fired { .. } => continue,
            Step::Halted { state } => {
                assert!(!state.is_empty());
                break;
            }
        }
    }
}

#[test]
fn the_liveness_report_states_what_it_cannot_prove_rather_than_asserting_it() {
    let ir = reference_ir();
    let mut machine = Machine::load(ir, ExecutionMode::Replay, "thread:test");
    machine.run().expect("runs");
    let report = machine.liveness();

    assert!(
        !report.deadlock_freedom_proven,
        "23.34 forbids claiming universal deadlock freedom for dynamic opaque agents"
    );
    assert!(report.commitments_left_open.is_empty());
    assert!(
        report.unreachable_states.is_empty(),
        "the lowering must not emit states nothing enters: {:?}",
        report.unreachable_states
    );
}

#[test]
fn an_open_commitment_is_reported_as_open_rather_than_quietly_closed() {
    let ir = compile(MUTATING_PROGRAM).expect("compiles");
    let mut machine = Machine::load(ir, ExecutionMode::Live, "thread:test");
    machine.step().expect("propose");
    machine.step().expect("accept");
    let report = machine.liveness();
    assert_eq!(report.commitments_left_open.len(), 1);
}

#[test]
fn the_topology_maps_every_bound_participant_to_its_role() {
    let ir = reference_ir();
    let machine = Machine::load(ir, ExecutionMode::Replay, "thread:test");
    assert_eq!(machine.topology().get("lead").map(String::as_str), Some("Lead"));
    assert_eq!(
        machine.topology().get("reviewer").map(String::as_str),
        Some("Reviewer")
    );
}

#[test]
fn a_trace_is_canonical_json_and_survives_a_round_trip() {
    let trace = run(COMPLETE_PROGRAM, ExecutionMode::Replay).expect("runs");
    let text = serde_json::to_string(&trace).expect("serialises");
    let restored: Trace = serde_json::from_str(&text).expect("round trips");
    assert_eq!(restored, trace);

    let value = serde_json::to_value(&trace.events).expect("serialises");
    let digest = bioprism_ids::sha256_hex_of_value(&value).expect("hashes");
    assert_eq!(digest, trace.digest);
}
