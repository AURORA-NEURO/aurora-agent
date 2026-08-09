//! Blueprint 23.06: a global type is well formed exactly when every role has a local type.

use bioprism_choreography::{
    GlobalType, Label, LocalBranch, LocalType, ProtocolError, Role, WellFormedGlobal,
};

fn plan_execute_verify() -> GlobalType {
    GlobalType::message(
        "lead",
        "worker",
        "task",
        GlobalType::message(
            "worker",
            "verifier",
            "artifact",
            GlobalType::message("verifier", "lead", "pass", GlobalType::End),
        ),
    )
}

#[test]
fn a_well_formed_global_type_projects_onto_every_role_it_names() {
    let checked = plan_execute_verify().well_formed().expect("well formed");
    let roles: Vec<&Role> = checked.roles().collect();
    assert_eq!(
        roles,
        vec![
            &Role::new("lead"),
            &Role::new("verifier"),
            &Role::new("worker")
        ]
    );
}

#[test]
fn a_projection_contains_only_the_transitions_of_its_own_role() {
    let checked = plan_execute_verify().well_formed().expect("well formed");
    let worker = checked.project(&Role::new("worker")).expect("worker");

    let LocalType::Offer { from, branches } = worker else {
        panic!("worker starts by waiting for the lead, got {worker:?}");
    };
    assert_eq!(from, &Role::new("lead"));
    assert_eq!(branches.len(), 1);
    let LocalType::Select { to, .. } = &branches[0].continuation else {
        panic!("worker then sends to the verifier");
    };
    assert_eq!(to, &Role::new("verifier"));
}

#[test]
fn a_role_that_never_acts_is_absent_from_the_projection_rather_than_idle() {
    let checked = plan_execute_verify().well_formed().expect("well formed");
    let error = checked
        .project(&Role::new("statistician"))
        .expect_err("no such role");
    assert_eq!(
        error,
        ProtocolError::RoleNotInProtocol {
            role: Role::new("statistician")
        }
    );
}

#[test]
fn self_communication_is_rejected_at_projection_not_discovered_at_runtime() {
    let error = GlobalType::message("lead", "lead", "note", GlobalType::End)
        .well_formed()
        .expect_err("a role cannot send to itself");
    assert_eq!(
        error,
        ProtocolError::SelfCommunication {
            role: Role::new("lead")
        }
    );
}

#[test]
fn two_branches_with_the_same_label_are_rejected() {
    let error = GlobalType::choice(
        "verifier",
        "lead",
        vec![
            (Label::new("pass"), GlobalType::End),
            (Label::new("pass"), GlobalType::End),
        ],
    )
    .well_formed()
    .expect_err("the receiver could not tell them apart");
    assert_eq!(
        error,
        ProtocolError::DuplicateLabel {
            from: Role::new("verifier"),
            to: Role::new("lead"),
            label: Label::new("pass"),
        }
    );
}

#[test]
fn a_choice_with_no_branches_is_rejected() {
    let error = GlobalType::choice("verifier", "lead", vec![])
        .well_formed()
        .expect_err("nothing can ever be sent");
    assert_eq!(
        error,
        ProtocolError::EmptyChoice {
            from: Role::new("verifier"),
            to: Role::new("lead"),
        }
    );
}

#[test]
fn a_free_recursion_variable_is_rejected() {
    let error = GlobalType::message("lead", "worker", "task", GlobalType::var("loop"))
        .well_formed()
        .expect_err("loop is not bound");
    assert_eq!(
        error,
        ProtocolError::UnboundRecursionVariable {
            var: "loop".to_string()
        }
    );
}

#[test]
fn an_unguarded_recursion_is_rejected_because_it_makes_no_progress() {
    let error = GlobalType::rec("loop", GlobalType::var("loop"))
        .well_formed()
        .expect_err("rec X. X has no transition");
    assert_eq!(
        error,
        ProtocolError::UnguardedRecursion {
            var: "loop".to_string()
        }
    );
}

#[test]
fn a_role_not_informed_of_a_choice_is_a_typed_error_and_not_a_local_type() {
    let global = GlobalType::choice(
        "verifier",
        "lead",
        vec![
            (
                Label::new("pass"),
                GlobalType::message("lead", "worker", "release", GlobalType::End),
            ),
            (Label::new("fail"), GlobalType::End),
        ],
    );

    let error = global.well_formed().expect_err("the worker is never told");
    assert_eq!(
        error,
        ProtocolError::UninformedOfChoice {
            role: Role::new("worker"),
            from: Role::new("verifier"),
            to: Role::new("lead"),
            left: Label::new("pass"),
            right: Label::new("fail"),
        }
    );
}

#[test]
fn an_uninformed_role_error_names_the_role_and_both_labels() {
    let global = GlobalType::choice(
        "verifier",
        "lead",
        vec![
            (
                Label::new("pass"),
                GlobalType::message("lead", "worker", "release", GlobalType::End),
            ),
            (Label::new("fail"), GlobalType::End),
        ],
    );
    let message = global.well_formed().expect_err("uninformed").to_string();
    assert!(message.contains("worker"), "{message}");
    assert!(message.contains("pass"), "{message}");
    assert!(message.contains("fail"), "{message}");
    assert!(message.contains("not informed"), "{message}");
}

#[test]
fn external_choices_from_the_same_peer_merge_into_one_local_type() {
    let global = GlobalType::choice(
        "verifier",
        "lead",
        vec![
            (
                Label::new("pass"),
                GlobalType::message("lead", "worker", "release", GlobalType::End),
            ),
            (
                Label::new("fail"),
                GlobalType::message("lead", "worker", "revise", GlobalType::End),
            ),
        ],
    );

    let checked = global.well_formed().expect("the worker learns from the label");
    let worker = checked.project(&Role::new("worker")).expect("worker");
    assert_eq!(
        worker,
        &LocalType::Offer {
            from: Role::new("lead"),
            branches: vec![
                LocalBranch {
                    label: Label::new("release"),
                    continuation: LocalType::End,
                },
                LocalBranch {
                    label: Label::new("revise"),
                    continuation: LocalType::End,
                },
            ],
        }
    );
}

#[test]
fn two_internal_choices_do_not_merge_because_the_role_would_have_to_guess() {
    let global = GlobalType::choice(
        "verifier",
        "lead",
        vec![
            (
                Label::new("pass"),
                GlobalType::message("worker", "lead", "done", GlobalType::End),
            ),
            (
                Label::new("fail"),
                GlobalType::message("worker", "lead", "abort", GlobalType::End),
            ),
        ],
    );

    let error = global
        .well_formed()
        .expect_err("the worker would have to know which label to send");
    assert!(matches!(
        error,
        ProtocolError::UninformedOfChoice { ref role, .. } if role == &Role::new("worker")
    ));
}

#[test]
fn a_role_outside_a_loop_projects_to_end_rather_than_an_unproductive_recursion() {
    let global = GlobalType::message(
        "lead",
        "worker",
        "start",
        GlobalType::rec(
            "loop",
            GlobalType::message("worker", "ledger", "observation", GlobalType::var("loop")),
        ),
    );

    let checked = global.well_formed().expect("well formed");
    let lead = checked.project(&Role::new("lead")).expect("lead");
    let LocalType::Select { branches, .. } = lead else {
        panic!("the lead sends start");
    };
    assert_eq!(
        branches[0].continuation,
        LocalType::End,
        "the lead takes no part in the loop, so it finishes instead of looping silently"
    );
}

#[test]
fn unfolding_a_recursive_local_type_returns_to_a_syntactically_identical_term() {
    let global = GlobalType::rec(
        "loop",
        GlobalType::message("worker", "ledger", "observation", GlobalType::var("loop")),
    );
    let checked = global.well_formed().expect("well formed");
    let worker = checked.project(&Role::new("worker")).expect("worker").clone();

    let LocalType::Select { branches, .. } = worker.unfold() else {
        panic!("the worker sends inside the loop");
    };
    assert_eq!(
        branches[0].continuation, worker,
        "one round of the loop returns to the same term, which is what keeps the state space finite"
    );
}

#[test]
fn the_well_formed_witness_cannot_be_deserialised_from_an_ill_formed_global() {
    let ill_formed = GlobalType::message("lead", "lead", "note", GlobalType::End);
    let encoded = serde_json::to_string(&ill_formed).expect("global types serialise");

    let decoded: Result<WellFormedGlobal, _> = serde_json::from_str(&encoded);
    assert!(
        decoded.is_err(),
        "serde must re-run the check rather than trust the bytes"
    );
}

#[test]
fn a_well_formed_witness_survives_a_round_trip_with_the_same_digest() {
    let checked = plan_execute_verify().well_formed().expect("well formed");
    let encoded = serde_json::to_string(&checked).expect("serialises");
    let decoded: WellFormedGlobal = serde_json::from_str(&encoded).expect("round trips");

    assert_eq!(decoded.digest().unwrap(), checked.digest().unwrap());
    assert_eq!(decoded.projections(), checked.projections());
}

#[test]
fn the_digest_changes_when_the_protocol_changes() {
    let original = plan_execute_verify().well_formed().expect("well formed");
    let altered = GlobalType::message(
        "lead",
        "worker",
        "task",
        GlobalType::message("worker", "verifier", "artifact", GlobalType::End),
    )
    .well_formed()
    .expect("well formed");

    assert_ne!(original.digest().unwrap(), altered.digest().unwrap());
}
