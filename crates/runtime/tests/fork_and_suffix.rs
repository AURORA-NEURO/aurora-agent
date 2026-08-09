//! Fork, replay and suffix execution (blueprint 05.05).
//!
//! What makes a counterfactual *matched* is that both branches start from the same state, and what
//! makes that checkable is that the state is a digest rather than a resemblance. These tests hold
//! the fork to three things: it lands on the parent's exact digest, it never re-performs the
//! prefix, and it declares whatever makes the two branches less than exactly comparable.

use bioprism_ids::RunId;
use bioprism_runtime::{
    cache_reuse, compare_suffixes, fork_tape, observable_state, open_suffix, CachePrefixKey, Clock,
    EffectKind, EffectPolicy, ExternalActions, Host, InProcessWorld, MaterializationPolicy, Network,
    NetworkMode, Provenance, RecordingHost, ReuseStatus, RuntimeError, Sandbox, WorldTape,
    OBSERVABLE_STATE_VERSION,
};

fn run(id: &str) -> RunId {
    RunId::parse(id).expect("well-formed run id")
}

fn policy() -> EffectPolicy {
    EffectPolicy::evaluation_default()
        .declaring([
            EffectKind::ClockNow,
            EffectKind::ClockSleep,
            EffectKind::FileRead,
            EffectKind::FileWrite,
            EffectKind::NetworkFetch,
            EffectKind::ModelCall,
            EffectKind::Payment,
        ])
        .allowing_path("/work/")
        .with_network(NetworkMode::RecordedFixture)
}

/// The world the parent ran against: it has the fixture the prefix needs.
fn parent_world() -> InProcessWorld {
    InProcessWorld::new()
        .with_seed(3)
        .with_base_file("/work/in.txt", "input")
        .with_fixture("GET", "https://fixtures.test/prefix", "prefix-body")
}

/// The world a branch runs against. It deliberately lacks the prefix fixture, so any attempt to
/// re-perform the inherited steps would fail rather than pass unnoticed.
fn branch_world() -> InProcessWorld {
    InProcessWorld::new()
        .with_seed(3)
        .with_base_file("/work/in.txt", "input")
        .with_fixture("GET", "https://fixtures.test/suffix", "suffix-body")
}

fn prefix_program(host: &mut dyn Host) -> Result<(), RuntimeError> {
    host.sleep(100)?;
    host.get_body("https://fixtures.test/prefix")?;
    host.read_file("/work/in.txt")?;
    Ok(())
}

fn parent_tape() -> WorldTape {
    let mut host = RecordingHost::new(run("run-parent"), parent_world(), policy());
    prefix_program(&mut host).expect("declared");
    host.write_file("/work/parent.txt", "parent-only")
        .expect("allowed");
    host.into_tape()
}

#[test]
fn a_fork_shares_the_parents_state_digest_at_the_fork_point() {
    let parent = parent_tape();
    let child = fork_tape(&parent, 3, run("run-child")).expect("within range");

    assert_eq!(
        child.state_digest_at(3).expect("in range"),
        parent.state_digest_at(3).expect("in range"),
        "a fork must land on the parent's exact state, not on a similar one"
    );
    assert_eq!(child.inherited_steps(), 3);
    let lineage = child.lineage().expect("a fork has a parent");
    assert_eq!(lineage.parent_run.as_str(), "run-parent");
    assert_eq!(lineage.forked_at_step, 3);
    child.verify_chain().expect("the inherited chain is intact");
}

#[test]
fn forking_past_the_end_of_a_tape_is_refused() {
    let parent = parent_tape();
    let error = fork_tape(&parent, parent.len() + 1, run("run-child"))
        .expect_err("there is no state after the end");
    assert!(matches!(error, RuntimeError::ForkBeyondEnd { .. }));
}

#[test]
fn a_fork_does_not_re_perform_the_prefix() {
    let parent = parent_tape();
    let mut suffix = open_suffix(&parent, 3, run("run-child"), branch_world(), policy())
        .expect("within range");

    suffix.resume_at_fork();
    suffix
        .get_body("https://fixtures.test/suffix")
        .expect("the branch's own fixture");
    suffix.write_file("/work/child.txt", "child").expect("allowed");

    assert_eq!(
        suffix.source().calls(),
        2,
        "the branch's world was asked only about the branch's own steps"
    );
    let tape = suffix.finish().expect("the fork point was reached");
    assert_eq!(tape.len(), 5, "three inherited steps plus two new ones");
}

#[test]
fn a_forked_suffix_diverges_from_the_parent_only_after_the_fork_point() {
    let parent = parent_tape();
    let mut suffix = open_suffix(&parent, 3, run("run-child"), branch_world(), policy())
        .expect("within range");
    suffix.resume_at_fork();
    suffix
        .write_file("/work/child.txt", "child-only")
        .expect("allowed");
    let child = suffix.finish().expect("reached the fork");

    assert_eq!(
        parent.first_divergence(&child),
        Some(3),
        "everything before the fork point is shared byte for byte"
    );
    let comparison = compare_suffixes(&parent, &child);
    assert_eq!(comparison.common_ancestor_step, 3);
    assert_eq!(comparison.first_divergence, Some(3));
    assert_eq!(comparison.left_steps, 4);
    assert_eq!(comparison.right_steps, 4);
}

#[test]
fn re_walking_the_prefix_with_a_different_program_is_caught_at_the_fork_point() {
    let parent = parent_tape();
    let mut suffix = open_suffix(&parent, 3, run("run-child"), branch_world(), policy())
        .expect("within range");

    suffix.sleep(100).expect("step 0 matches the parent");
    let error = suffix
        .sleep(100)
        .expect_err("the parent fetched at step 1, it did not sleep again");
    match error {
        RuntimeError::DivergentRequest { step, recorded, .. } => {
            assert_eq!(step, 1);
            assert_eq!(recorded, "network_fetch(GET https://fixtures.test/prefix)");
        }
        other => panic!("expected a divergence, got {other}"),
    }
    assert_eq!(
        suffix.source().calls(),
        0,
        "a re-walked prefix is checked against the tape, never against the world"
    );
}

#[test]
fn a_suffix_that_never_reached_the_fork_point_is_refused() {
    let parent = parent_tape();
    let mut suffix = open_suffix(&parent, 3, run("run-child"), branch_world(), policy())
        .expect("within range");
    suffix.sleep(100).expect("step 0 matches");

    assert!(suffix.is_replaying());
    let error = suffix
        .finish()
        .expect_err("a branch that stopped inside the prefix has no suffix to compare");
    assert!(matches!(error, RuntimeError::SuffixNotReached { .. }));
}

#[test]
fn a_fork_cannot_repeat_an_irreversible_effect_and_simulates_it_instead() {
    let parent = parent_tape();
    let fork_policy = policy().with_materialization(MaterializationPolicy::Simulate);
    let mut suffix = open_suffix(&parent, 3, run("run-child"), branch_world(), fork_policy)
        .expect("within range");
    suffix.resume_at_fork();

    let outcome = suffix
        .pay("vendor-1", 4_200)
        .expect("a counterfactual may propose a payment; it may not make one");
    assert_eq!(outcome.field("simulated"), Some(&serde_json::json!(true)));
    assert_eq!(
        suffix.source().calls(),
        0,
        "no world was touched by the simulated payment"
    );

    let child = suffix.finish().expect("reached the fork");
    assert_eq!(child.simulated_steps(), vec![3]);
    assert_eq!(child.entries()[3].effect.provenance, Provenance::Simulated);
}

#[test]
fn a_comparison_declares_that_a_branch_simulated_rather_than_performed() {
    let parent = parent_tape();
    let fork_policy = policy().with_materialization(MaterializationPolicy::Simulate);
    let mut suffix = open_suffix(&parent, 3, run("run-child"), branch_world(), fork_policy)
        .expect("within range");
    suffix.resume_at_fork();
    suffix.pay("vendor-1", 4_200).expect("simulated");
    let child = suffix.finish().expect("reached the fork");

    let comparison = compare_suffixes(&parent, &child);
    assert_eq!(comparison.right_simulated, vec![3]);
    assert!(
        comparison
            .reconstruction_differences
            .iter()
            .any(|note| note.contains("simulated")),
        "a simulated outcome must be declared, not folded into the result: {:?}",
        comparison.reconstruction_differences
    );
}

#[test]
fn a_comparison_declares_when_branches_inherited_different_prefixes() {
    let parent = parent_tape();
    let short = fork_tape(&parent, 1, run("run-short")).expect("in range");
    let long = fork_tape(&parent, 3, run("run-long")).expect("in range");

    let comparison = compare_suffixes(&short, &long);
    assert!(
        comparison
            .reconstruction_differences
            .iter()
            .any(|note| note.contains("different prefixes")),
        "{:?}",
        comparison.reconstruction_differences
    );
}

#[test]
fn observable_state_hands_a_continuation_the_prefix_it_did_not_run() {
    let parent = parent_tape();
    let state = observable_state(&parent, 3).expect("in range");

    assert_eq!(state.version, OBSERVABLE_STATE_VERSION);
    assert_eq!(state.fork_step, 3);
    assert_eq!(
        state.state_digest,
        parent.state_digest_at(3).expect("in range")
    );
    assert_eq!(state.steps.len(), 3);
    assert!(
        state
            .steps
            .iter()
            .all(|step| step.provenance == Provenance::Performed),
        "a candidate must be able to tell an observed answer from an invented one"
    );
}

#[test]
fn two_branches_from_the_same_point_are_compared_at_that_point() {
    let parent = parent_tape();

    let mut left = open_suffix(&parent, 2, run("run-left"), branch_world(), policy())
        .expect("in range");
    left.resume_at_fork();
    left.write_file("/work/left.txt", "left").expect("allowed");
    let left = left.finish().expect("reached the fork");

    let mut right = open_suffix(&parent, 2, run("run-right"), branch_world(), policy())
        .expect("in range");
    right.resume_at_fork();
    right.write_file("/work/right.txt", "right").expect("allowed");
    let right = right.finish().expect("reached the fork");

    let comparison = compare_suffixes(&left, &right);
    assert_eq!(comparison.common_ancestor_step, 2);
    assert_eq!(comparison.first_divergence, Some(2));
    assert!(
        comparison.reconstruction_differences.is_empty(),
        "two branches performed from the same digest are exactly comparable: {:?}",
        comparison.reconstruction_differences
    );
}

#[test]
fn a_forked_tape_still_loads_after_a_json_round_trip() {
    let parent = parent_tape();
    let child = fork_tape(&parent, 2, run("run-child")).expect("in range");
    let reloaded =
        WorldTape::from_json(&child.to_json().expect("serializes")).expect("reloads intact");

    assert_eq!(reloaded, child);
    assert_eq!(reloaded.inherited_steps(), 2);
}

#[test]
fn cache_reuse_is_rejected_when_the_tool_schema_changes() {
    let parent = parent_tape();
    let schema = serde_json::json!({ "tools": ["read", "write"] });
    let recorded = CachePrefixKey::for_fork(&parent, 3, "m1", &schema, "policy-v1")
        .expect("in range");

    assert_eq!(
        cache_reuse(&recorded, &recorded.clone()),
        ReuseStatus::Reused
    );

    let widened = serde_json::json!({ "tools": ["read", "write", "delete"] });
    let candidate = CachePrefixKey::for_fork(&parent, 3, "m1", &widened, "policy-v1")
        .expect("in range");
    match cache_reuse(&recorded, &candidate) {
        ReuseStatus::Rejected { reason } => assert_eq!(reason, "tool schema differs"),
        ReuseStatus::Reused => panic!("a different tool schema is a different prefix"),
    }
}

#[test]
fn cache_reuse_is_rejected_when_the_fork_point_moves() {
    let parent = parent_tape();
    let schema = serde_json::json!({ "tools": [] });
    let at_three = CachePrefixKey::for_fork(&parent, 3, "m1", &schema, "p").expect("in range");
    let at_two = CachePrefixKey::for_fork(&parent, 2, "m1", &schema, "p").expect("in range");

    match cache_reuse(&at_three, &at_two) {
        ReuseStatus::Rejected { reason } => assert_eq!(reason, "prefix differs"),
        ReuseStatus::Reused => panic!("a different prefix is not the same prefix"),
    }
}
