//! WorldTape and state ledger (blueprint 05.04).
//!
//! Each test names the property it defends. The tape is the crate's spine: if a tape can be edited
//! without detection, or two runs can reach "the same state" without agreeing on a digest, then
//! every guarantee built on top of it — replay, forking, matched comparison — is decoration.

use bioprism_ids::RunId;
use bioprism_runtime::{
    Clock, Effect, EffectClass, EffectKind, EffectOutcome, EffectPolicy, EffectRequest,
    InProcessWorld, NetworkMode, Provenance, RecordingHost, RestorationDeclaration, RuntimeError,
    Sandbox, WorldTape,
};
use serde_json::Value;

fn run(id: &str) -> RunId {
    RunId::parse(id).expect("well-formed run id")
}

fn permissive_policy() -> EffectPolicy {
    EffectPolicy::evaluation_default()
        .declaring([
            EffectKind::ClockNow,
            EffectKind::ClockSleep,
            EffectKind::RandomBytes,
            EffectKind::FileRead,
            EffectKind::FileWrite,
            EffectKind::NetworkFetch,
            EffectKind::ModelCall,
            EffectKind::ProcessSpawn,
        ])
        .allowing_path("/work/")
        .with_network(NetworkMode::RecordedFixture)
}

fn world() -> InProcessWorld {
    InProcessWorld::new()
        .with_seed(11)
        .with_base_file("/work/in.txt", "input")
        .with_fixture("GET", "https://fixtures.test/a", "body-a")
}

fn recorded(id: &str) -> WorldTape {
    let mut host = RecordingHost::new(run(id), world(), permissive_policy());
    host.sleep(50).expect("clock is declared");
    host.read_file("/work/in.txt").expect("path is allowed");
    host.write_file("/work/out.txt", "alpha")
        .expect("path is allowed");
    host.into_tape()
}

#[test]
fn a_tape_entry_chains_the_previous_digest_so_an_edit_breaks_the_chain() {
    let tape = recorded("run-chain");
    tape.verify_chain().expect("an untouched tape verifies");

    assert_eq!(tape.entries()[0].previous, "");
    assert_eq!(tape.entries()[1].previous, tape.entries()[0].digest);
    assert_eq!(tape.entries()[2].previous, tape.entries()[1].digest);
}

#[test]
fn a_tampered_tape_fails_to_load_rather_than_loading_quietly() {
    let tape = recorded("run-tamper");
    let json = tape.to_json().expect("tape serializes");
    assert!(
        json.contains("alpha"),
        "the write is in the serialized form"
    );

    // Same length, so nothing but the content changes.
    let tampered = json.replace("alpha", "gamma");
    let error = WorldTape::from_json(&tampered).expect_err("a tampered tape must not load");
    assert!(
        matches!(error, RuntimeError::BrokenChain { .. }),
        "expected a broken chain, got {error}"
    );
}

#[test]
fn a_tape_round_trips_through_json_unchanged() {
    let tape = recorded("run-round-trip");
    let json = tape.to_json().expect("tape serializes");
    let reloaded = WorldTape::from_json(&json).expect("an untouched tape reloads");

    assert_eq!(reloaded, tape);
    assert_eq!(reloaded.to_json().expect("reserializes"), json);
}

#[test]
fn the_state_digest_at_a_step_commits_to_exactly_that_prefix() {
    let tape = recorded("run-digest");

    assert_eq!(
        tape.state_digest_at(0).expect("zero is always in range"),
        "",
        "a run that has done nothing has committed to nothing"
    );
    assert_eq!(
        tape.state_digest_at(1).expect("in range"),
        tape.entries()[0].digest
    );
    assert_eq!(
        tape.state_digest_at(tape.len()).expect("in range"),
        tape.head()
    );
}

#[test]
fn a_step_past_the_end_of_a_tape_is_out_of_range() {
    let tape = recorded("run-range");
    let error = tape
        .state_digest_at(tape.len() + 1)
        .expect_err("past the end is not a state");
    assert!(matches!(error, RuntimeError::StepOutOfRange { .. }));
}

#[test]
fn two_runs_that_did_the_same_things_agree_digest_for_digest() {
    let left = recorded("run-twin-a");
    let right = recorded("run-twin-b");

    assert_eq!(left.head(), right.head());
    assert_eq!(
        left.first_divergence(&right),
        None,
        "the run id is not part of an entry's digest, so identical work agrees"
    );
}

#[test]
fn first_divergence_reports_the_earliest_step_two_tapes_disagree() {
    let left = recorded("run-diverge-a");

    let mut host = RecordingHost::new(run("run-diverge-b"), world(), permissive_policy());
    host.sleep(50).expect("declared");
    host.read_file("/work/in.txt").expect("allowed");
    host.write_file("/work/out.txt", "beta").expect("allowed");
    let right = host.into_tape();

    assert_eq!(left.first_divergence(&right), Some(2));
}

#[test]
fn a_shorter_tape_diverges_at_the_step_it_stopped() {
    let long = recorded("run-long");

    let mut host = RecordingHost::new(run("run-short"), world(), permissive_policy());
    host.sleep(50).expect("declared");
    let short = host.into_tape();

    assert_eq!(long.first_divergence(&short), Some(1));
    assert_eq!(short.first_divergence(&long), Some(1));
}

#[test]
fn artifacts_separate_what_a_run_consumed_from_what_it_created() {
    let tape = recorded("run-artifacts");
    let artifacts = tape.artifacts();

    assert!(artifacts.consumed.contains("/work/in.txt"));
    assert!(!artifacts.consumed.contains("/work/out.txt"));
    assert!(artifacts.created.contains_key("/work/out.txt"));
    assert!(!artifacts.created.contains_key("/work/in.txt"));
}

#[test]
fn a_checkpoint_commits_to_the_tape_head_it_was_taken_from() {
    let mut tape = recorded("run-checkpoint");
    let checkpoint = tape
        .checkpoint("in_process", RestorationDeclaration::portable())
        .expect("checkpoint is within the tape bound");

    assert_eq!(checkpoint.step, tape.len());
    assert_eq!(checkpoint.tape_head, tape.head());
    tape.verify_checkpoint(&checkpoint)
        .expect("its own checkpoint verifies");
}

#[test]
fn a_checkpoint_from_another_world_line_is_refused() {
    let mut left = recorded("run-ckpt-left");
    let checkpoint = left
        .checkpoint("in_process", RestorationDeclaration::portable())
        .expect("checkpoint is within the tape bound");

    let mut host = RecordingHost::new(run("run-ckpt-right"), world(), permissive_policy());
    host.sleep(50).expect("declared");
    host.read_file("/work/in.txt").expect("allowed");
    host.write_file("/work/out.txt", "different")
        .expect("allowed");
    let right = host.into_tape();

    let error = right
        .verify_checkpoint(&checkpoint)
        .expect_err("a checkpoint from another world-line does not describe this one");
    assert!(matches!(error, RuntimeError::CorruptCheckpoint { .. }));
}

#[test]
fn a_tampered_fork_lineage_is_rejected_before_use() {
    let mut host = RecordingHost::new(
        run("run-lineage-parent"),
        InProcessWorld::new(),
        permissive_policy(),
    );
    host.sleep(50).expect("declared");
    let child = bioprism_runtime::fork_tape(&host.into_tape(), 1, run("run-lineage-child"))
        .expect("fork point is in range");
    let mut document: Value = serde_json::from_str(&child.to_json().expect("serializes")).unwrap();
    document["lineage"]["parent_head"] = Value::String("tampered".into());

    let error = WorldTape::from_json(&document.to_string()).expect_err("lineage is authenticated");
    assert!(matches!(error, RuntimeError::LineageMismatch { .. }));
}

#[test]
fn an_oversized_tape_document_is_refused_before_deserialization() {
    let oversized = "x".repeat(bioprism_runtime::MAX_TAPE_JSON_BYTES + 1);
    let error = WorldTape::from_json(&oversized).expect_err("the input bound is enforced first");
    assert!(matches!(error, RuntimeError::TapeTooLarge { .. }));
}

#[test]
fn an_empty_tape_has_an_empty_head_and_no_state_to_report() {
    let tape = WorldTape::new(run("run-empty"));
    assert!(tape.is_empty());
    assert_eq!(tape.head(), "");
    assert_eq!(tape.inherited_steps(), 0);
    assert!(tape.simulated_steps().is_empty());
    tape.verify_chain().expect("an empty chain is intact");
}

#[test]
fn an_effect_cannot_relabel_its_request_class_or_simulated_outcome() {
    let mut class_drift = Effect::performed(
        EffectRequest::FileWrite {
            path: "/work/out.txt".into(),
            content: "content".into(),
        },
        EffectOutcome::new(Value::String("ok".into())),
    );
    class_drift.class = EffectClass::Pure;
    assert!(matches!(
        class_drift.validate(),
        Err(RuntimeError::InvariantViolation { .. })
    ));

    let mut simulated_drift = Effect::simulated(EffectRequest::ClockNow);
    simulated_drift.outcome = EffectOutcome::new(Value::Bool(false));
    assert_eq!(simulated_drift.provenance, Provenance::Simulated);
    assert!(matches!(
        simulated_drift.validate(),
        Err(RuntimeError::InvariantViolation { .. })
    ));
}
