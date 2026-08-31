//! Network, time and randomness virtualization (blueprint 05.07).
//!
//! The claim under test is narrow and absolute: replaying a tape reproduces byte-identical results,
//! and a replay that meets an effect the tape does not record fails loudly instead of reaching for
//! the live world. Everything else in this crate — forking, matched comparison, published evidence
//! — is only worth as much as these tests are.

use bioprism_ids::RunId;
use bioprism_runtime::{
    Clock, EffectKind, EffectPolicy, EffectRequest, Fault, Host, InProcessWorld, Network,
    NetworkMode, Randomness, RecordingHost, ReplayHost, RuntimeError, Sandbox, WorldTape,
};

fn run(id: &str) -> RunId {
    RunId::parse(id).expect("well-formed run id")
}

fn policy() -> EffectPolicy {
    EffectPolicy::evaluation_default()
        .declaring([
            EffectKind::ClockNow,
            EffectKind::ClockSleep,
            EffectKind::RandomBytes,
            EffectKind::FileRead,
            EffectKind::FileWrite,
            EffectKind::NetworkFetch,
            EffectKind::ModelCall,
        ])
        .allowing_path("/work/")
        .with_network(NetworkMode::RecordedFixture)
}

fn world_saying(body: &str) -> InProcessWorld {
    InProcessWorld::new()
        .with_seed(0x5eed)
        .with_base_file("/work/in.txt", "input")
        .with_fixture("GET", "https://fixtures.test/a", body)
}

/// The program under test. Written against `&mut dyn Host` so the identical text runs recording,
/// replaying, or as a forked suffix — which is the only way "the same program" means anything.
fn program(host: &mut dyn Host) -> Result<Vec<String>, RuntimeError> {
    let mut out = Vec::new();
    out.push(host.now_millis()?.to_string());
    host.sleep(120)?;
    out.push(host.now_millis()?.to_string());
    out.push(host.random_hex(6)?);
    out.push(host.get_body("https://fixtures.test/a")?);
    out.push(host.call_model("m1", "hello world")?);
    out.push(format!("{:?}", host.read_file("/work/in.txt")?));
    out.push(host.write_file("/work/out.txt", "alpha")?.to_string());
    Ok(out)
}

fn record(id: &str, world: InProcessWorld) -> (Vec<String>, WorldTape) {
    let mut host = RecordingHost::new(run(id), world, policy());
    let output = program(&mut host).expect("the program is fully declared");
    (output, host.into_tape())
}

#[test]
fn replaying_a_tape_reproduces_byte_identical_results() {
    let (live, tape) = record("run-replay", world_saying("body-a"));

    let mut replay = ReplayHost::new(tape);
    let replayed = program(&mut replay).expect("the tape records every step");

    assert_eq!(
        serde_json::to_string(&replayed).expect("outputs serialize"),
        serde_json::to_string(&live).expect("outputs serialize"),
        "a replay must reproduce the recorded run byte for byte"
    );
    replay.finish().expect("the whole tape was consumed");
}

#[test]
fn a_replay_survives_the_tape_going_through_json() {
    let (live, tape) = record("run-json", world_saying("body-a"));
    let reloaded =
        WorldTape::from_json(&tape.to_json().expect("serializes")).expect("reloads intact");

    let mut replay = ReplayHost::new(reloaded);
    assert_eq!(program(&mut replay).expect("records every step"), live);
}

#[test]
fn a_replay_that_meets_an_unrecorded_effect_fails_rather_than_going_live() {
    let (_, tape) = record("run-unrecorded", world_saying("body-a"));
    let recorded_steps = tape.len();

    let mut replay = ReplayHost::new(tape);
    program(&mut replay).expect("the recorded prefix replays");

    let error = replay
        .perform(EffectRequest::ClockNow)
        .expect_err("one step past the end of the tape has no recorded answer");
    match error {
        RuntimeError::UnrecordedEffect { step, request } => {
            assert_eq!(step, recorded_steps);
            assert_eq!(request, "clock_now");
        }
        other => panic!("expected an unrecorded effect, got {other}"),
    }
}

#[test]
fn a_replay_whose_request_diverges_names_the_step_and_both_requests() {
    let (_, tape) = record("run-diverged", world_saying("body-a"));

    let mut replay = ReplayHost::new(tape);
    replay.now_millis().expect("step 0 matches");
    replay.sleep(120).expect("step 1 matches");

    let error = replay
        .sleep(999)
        .expect_err("step 2 was a clock read, not another sleep");
    match error {
        RuntimeError::DivergentRequest {
            step,
            recorded,
            requested,
        } => {
            assert_eq!(step, 2);
            assert_eq!(recorded, "clock_now");
            assert_eq!(requested, "clock_sleep(999ms)");
        }
        other => panic!("expected a divergence, got {other}"),
    }
}

#[test]
fn a_replay_ignores_a_live_world_that_has_since_changed() {
    let (live, tape) = record("run-stable", world_saying("body-original"));

    // The same program against a world whose fixture now answers differently.
    let (changed, _) = record("run-changed", world_saying("body-rewritten"));
    assert_ne!(
        changed, live,
        "the contrast only means something if the world really did change the answer"
    );

    let mut replay = ReplayHost::new(tape);
    let replayed = program(&mut replay).expect("the tape records every step");
    assert_eq!(
        replayed, live,
        "a replay answers from the tape, not from whatever the world says today"
    );
}

#[test]
fn a_replay_that_stops_early_leaves_the_tape_unaccounted_for() {
    let (_, tape) = record("run-early-stop", world_saying("body-a"));
    let total = tape.len();

    let mut replay = ReplayHost::new(tape);
    replay.now_millis().expect("step 0 replays");

    let error = replay
        .finish()
        .expect_err("a program that stops early is not the program that was recorded");
    match error {
        RuntimeError::ReplayIncomplete { consumed, total: t } => {
            assert_eq!(consumed, 1);
            assert_eq!(t, total);
        }
        other => panic!("expected an incomplete replay, got {other}"),
    }
}

#[test]
fn two_recordings_of_the_same_program_and_world_produce_identical_tapes() {
    let (_, left) = record("run-same", world_saying("body-a"));
    let (_, right) = record("run-same", world_saying("body-a"));

    assert_eq!(
        left.to_json().expect("serializes"),
        right.to_json().expect("serializes"),
        "the recording path must contribute no nondeterminism of its own"
    );
}

#[test]
fn the_task_clock_advances_only_when_the_program_asks_it_to() {
    let mut host = RecordingHost::new(run("run-clock"), world_saying("body-a"), policy());

    assert_eq!(host.now_millis().expect("declared"), 0);
    assert_eq!(
        host.now_millis().expect("declared"),
        0,
        "reading a virtual clock is not an event in the task's timeline"
    );
    assert_eq!(host.sleep(750).expect("declared"), 750);
    assert_eq!(host.now_millis().expect("declared"), 750);
}

#[test]
fn a_clock_that_ticks_on_every_read_still_replays_identically() {
    let world = world_saying("body-a").with_clock_tick(5);
    let mut host = RecordingHost::new(run("run-tick"), world, policy());
    let live = program(&mut host).expect("declared");
    let tape = host.into_tape();

    let mut replay = ReplayHost::new(tape);
    assert_eq!(program(&mut replay).expect("records every step"), live);
}

#[test]
fn recorded_entropy_is_reproduced_exactly_rather_than_redrawn() {
    let mut host = RecordingHost::new(run("run-entropy"), world_saying("body-a"), policy());
    let first = host.random_hex(16).expect("declared");
    let second = host.random_hex(16).expect("declared");
    assert_ne!(first, second, "a generator that repeats is not a generator");
    let tape = host.into_tape();

    // The replay draws from a world seeded differently only in the sense that it has none at all.
    let mut replay = ReplayHost::new(tape);
    assert_eq!(replay.random_hex(16).expect("recorded"), first);
    assert_eq!(replay.random_hex(16).expect("recorded"), second);
}

#[test]
fn a_deterministically_injected_fault_reproduces_exactly() {
    let faulty = || {
        world_saying("body-a")
            .with_seed(1)
            .with_fault_at(1, Fault::Timeout)
    };

    let mut first = RecordingHost::new(run("run-fault-1"), faulty(), policy());
    first.now_millis().expect("call 0 is clean");
    let error_one = first
        .sleep(10)
        .expect_err("call 1 is scheduled to time out");

    let mut second = RecordingHost::new(run("run-fault-2"), faulty(), policy());
    second.now_millis().expect("call 0 is clean");
    let error_two = second
        .sleep(10)
        .expect_err("call 1 is scheduled to time out");

    assert_eq!(error_one, error_two);
    assert!(matches!(
        error_one,
        RuntimeError::InjectedFault { call: 1, .. }
    ));
    assert_eq!(
        first.into_tape().len(),
        1,
        "a faulted effect produced no outcome, so nothing was sealed into the tape"
    );
}

#[test]
fn a_truncated_response_is_what_the_tape_records_and_what_a_replay_returns() {
    let world = world_saying("a-long-body").with_fault_at(0, Fault::Truncated { keep_bytes: 6 });
    let mut host = RecordingHost::new(run("run-truncated"), world, policy());
    let live = host
        .get_body("https://fixtures.test/a")
        .expect("the fixture exists");
    assert_eq!(live, "a-long");

    let mut replay = ReplayHost::new(host.into_tape());
    assert_eq!(
        replay
            .get_body("https://fixtures.test/a")
            .expect("recorded"),
        "a-long"
    );
}

#[test]
fn a_fixture_miss_fails_closed_instead_of_reaching_the_network() {
    let mut host = RecordingHost::new(run("run-miss"), world_saying("body-a"), policy());

    let error = host
        .get_body("https://fixtures.test/never-recorded")
        .expect_err("recorded-fixture mode has nowhere else to look");
    match error {
        RuntimeError::SourceFailure { reason, .. } => {
            assert!(reason.contains("no recorded fixture"), "got {reason}");
        }
        other => panic!("expected a source failure, got {other}"),
    }
    assert!(
        host.into_tape().is_empty(),
        "a request that was never answered is not an effect"
    );
}
