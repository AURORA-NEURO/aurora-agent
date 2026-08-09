//! Blueprint 23.45: the difference between "no defect found within the bound" and "no defect".

use bioprism_choreography::{
    BoundHit, Event, ExplorationBound, GlobalType, Label, LocalBranch, LocalType, Property, Role,
    RoleStatus, System, Verdict,
};
use bioprism_section::InfluenceClass;
use std::collections::BTreeMap;

fn offer(from: &str, label: &str, continuation: LocalType) -> LocalType {
    LocalType::Offer {
        from: Role::new(from),
        branches: vec![LocalBranch {
            label: Label::new(label),
            continuation,
        }],
    }
}

fn select(to: &str, label: &str, continuation: LocalType) -> LocalType {
    LocalType::Select {
        to: Role::new(to),
        branches: vec![LocalBranch {
            label: Label::new(label),
            continuation,
        }],
    }
}

/// The authority deadlock of 23.45's own counterexample: the lead will not grant write access
/// until it sees a patch, and the worker will not produce a patch until it has write access.
fn grant_before_patch_deadlock() -> System {
    let lead = offer(
        "worker",
        "request-grant",
        offer("worker", "patch", select("worker", "grant", LocalType::End)),
    );
    let worker = select(
        "lead",
        "request-grant",
        offer("lead", "grant", select("lead", "patch", LocalType::End)),
    );
    System::from_locals([(Role::new("lead"), lead), (Role::new("worker"), worker)])
}

fn plan_execute_verify() -> System {
    let global = GlobalType::message(
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
    .well_formed()
    .expect("well formed");
    System::from_projection(&global)
}

#[test]
fn the_projection_of_a_well_formed_global_type_cannot_deadlock() {
    let report = plan_execute_verify().check(ExplorationBound::default());

    assert!(
        report.deadlock.is_proof(),
        "expected a proof, got {:?}",
        report.deadlock
    );
    assert!(report.liveness.is_proof());
    assert!(report.orphan.is_proof());
    assert!(report.unexpected_message.is_proof());
    assert!(report.all_proved());
    assert_eq!(report.truncated, None);
}

#[test]
fn a_protocol_that_genuinely_deadlocks_yields_a_counterexample_trace() {
    let report = grant_before_patch_deadlock().check(ExplorationBound::default());

    let counterexample = report
        .deadlock
        .counterexample()
        .expect("a deadlock without a trace is not usable");
    assert_eq!(counterexample.property, Property::Deadlock);
    assert_eq!(
        counterexample
            .steps
            .iter()
            .map(|step| step.event.clone())
            .collect::<Vec<Event>>(),
        vec![
            Event::Send {
                from: Role::new("worker"),
                to: Role::new("lead"),
                label: Label::new("request-grant"),
            },
            Event::Receive {
                from: Role::new("worker"),
                to: Role::new("lead"),
                label: Label::new("request-grant"),
            },
        ]
    );
}

#[test]
fn the_counterexample_is_the_shortest_path_to_the_offending_state() {
    let report = grant_before_patch_deadlock().check(ExplorationBound::default());
    let counterexample = report.deadlock.counterexample().expect("trace");
    assert_eq!(
        counterexample.steps.len(),
        2,
        "breadth-first search must not pad the trace with events the defect did not need"
    );
    assert_eq!(counterexample.steps[0].index, 0);
    assert_eq!(counterexample.steps[1].index, 1);
}

#[test]
fn the_counterexample_names_what_each_role_is_waiting_for() {
    let report = grant_before_patch_deadlock().check(ExplorationBound::default());
    let counterexample = report.deadlock.counterexample().expect("trace");

    let waiting: BTreeMap<String, Vec<String>> = counterexample
        .state
        .roles
        .iter()
        .filter_map(|role| match &role.status {
            RoleStatus::WaitingFor { peer, expecting } => Some((
                role.role.to_string(),
                vec![
                    peer.to_string(),
                    expecting
                        .iter()
                        .map(Label::as_str)
                        .collect::<Vec<&str>>()
                        .join(","),
                ],
            )),
            _ => None,
        })
        .collect();

    assert_eq!(
        waiting.get("lead"),
        Some(&vec!["worker".to_string(), "patch".to_string()])
    );
    assert_eq!(
        waiting.get("worker"),
        Some(&vec!["lead".to_string(), "grant".to_string()])
    );
}

#[test]
fn the_counterexample_renders_the_trace_format_the_blueprint_specifies() {
    let report = grant_before_patch_deadlock().check(ExplorationBound::default());
    let rendered = report.deadlock.counterexample().expect("trace").to_string();

    assert!(rendered.starts_with("state 0: worker sends request-grant to lead"));
    assert!(rendered.contains("state 1: lead receives request-grant from worker"));
    assert!(rendered.contains("DEADLOCK"));
}

#[test]
fn no_deadlock_within_the_bound_is_not_reported_as_no_deadlock() {
    let global = GlobalType::rec(
        "loop",
        GlobalType::message("worker", "ledger", "observation", GlobalType::var("loop")),
    )
    .well_formed()
    .expect("well formed");
    let report = System::from_projection(&global).check(ExplorationBound::default());

    assert!(
        !report.deadlock.is_proof(),
        "an unbounded stream has an infinite state space; claiming a proof would be a lie"
    );
    assert!(!report.deadlock.is_refuted());
    assert!(matches!(
        report.deadlock,
        Verdict::Inconclusive {
            hit: BoundHit::ChannelCapacity,
            ..
        }
    ));
    assert!(!report.all_proved());
}

#[test]
fn an_inconclusive_verdict_names_the_bound_that_stopped_the_search() {
    let long = GlobalType::message(
        "a",
        "b",
        "one",
        GlobalType::message(
            "b",
            "a",
            "two",
            GlobalType::message(
                "a",
                "b",
                "three",
                GlobalType::message("b", "a", "four", GlobalType::End),
            ),
        ),
    )
    .well_formed()
    .expect("well formed");

    let bound = ExplorationBound::new(3, 400, 2);
    let report = System::from_projection(&long).check(bound);

    match &report.deadlock {
        Verdict::Inconclusive {
            hit,
            bound: reported,
            states_explored,
        } => {
            assert_eq!(*hit, BoundHit::MaxStates);
            assert_eq!(*reported, bound);
            assert!(*states_explored <= 3);
        }
        other => panic!("expected an inconclusive verdict, got {other:?}"),
    }
}

#[test]
fn a_depth_bound_that_stops_the_search_is_reported_as_a_depth_bound() {
    let long = GlobalType::message(
        "a",
        "b",
        "one",
        GlobalType::message(
            "b",
            "a",
            "two",
            GlobalType::message("a", "b", "three", GlobalType::End),
        ),
    )
    .well_formed()
    .expect("well formed");

    let report = System::from_projection(&long).check(ExplorationBound::new(20_000, 2, 2));
    assert!(matches!(
        report.deadlock,
        Verdict::Inconclusive {
            hit: BoundHit::MaxDepth,
            ..
        }
    ));
}

#[test]
fn a_message_nobody_ever_receives_is_reported_as_an_orphan() {
    let system = System::from_locals([
        (
            Role::new("runner"),
            select("ledger", "observation", LocalType::End),
        ),
        (Role::new("ledger"), LocalType::End),
    ]);

    let report = system.check(ExplorationBound::default());
    let counterexample = report
        .orphan
        .counterexample()
        .expect("an orphan without a trace is not usable");
    assert_eq!(counterexample.property, Property::OrphanMessage);
    assert_eq!(counterexample.steps.len(), 1);
    assert!(counterexample.explanation.contains("never read"));
    assert_eq!(counterexample.state.channels.len(), 1);
    assert_eq!(
        counterexample.state.channels[0].queued,
        vec![Label::new("observation")]
    );
}

#[test]
fn an_orphan_terminal_state_is_not_also_called_a_deadlock() {
    let system = System::from_locals([
        (
            Role::new("runner"),
            select("ledger", "observation", LocalType::End),
        ),
        (Role::new("ledger"), LocalType::End),
    ]);

    let report = system.check(ExplorationBound::default());
    assert!(
        report.deadlock.is_proof(),
        "every role finished; the defect is the unread message, not a stall"
    );
    assert!(report.orphan.is_refuted());
}

#[test]
fn a_label_the_receiver_does_not_offer_is_an_unexpected_message() {
    let system = System::from_locals([
        (Role::new("sender"), select("receiver", "surprise", LocalType::End)),
        (
            Role::new("receiver"),
            offer("sender", "expected", LocalType::End),
        ),
    ]);

    let report = system.check(ExplorationBound::default());
    let counterexample = report.unexpected_message.counterexample().expect("trace");
    assert_eq!(counterexample.property, Property::UnexpectedMessage);
    assert!(counterexample.explanation.contains("surprise"));
}

#[test]
fn a_full_channel_is_not_reported_as_a_deadlock() {
    let global = GlobalType::message(
        "a",
        "b",
        "one",
        GlobalType::message("a", "b", "two", GlobalType::End),
    )
    .well_formed()
    .expect("well formed");

    let report = System::from_projection(&global).check(ExplorationBound::new(20_000, 400, 1));

    assert!(
        !report.deadlock.is_refuted(),
        "a queue-full stall is an artifact of the analysis bound, not a protocol defect"
    );
    assert!(matches!(
        report.deadlock,
        Verdict::Inconclusive {
            hit: BoundHit::ChannelCapacity,
            ..
        }
    ));
}

#[test]
fn the_same_protocol_is_proved_once_the_channel_is_wide_enough_to_hold_it() {
    let global = GlobalType::message(
        "a",
        "b",
        "one",
        GlobalType::message("a", "b", "two", GlobalType::End),
    )
    .well_formed()
    .expect("well formed");

    let report = System::from_projection(&global).check(ExplorationBound::new(20_000, 400, 2));
    assert!(report.all_proved());
}

#[test]
fn liveness_is_inconclusive_rather_than_refuted_when_the_search_was_truncated() {
    let global = GlobalType::rec(
        "loop",
        GlobalType::message("worker", "ledger", "observation", GlobalType::var("loop")),
    )
    .well_formed()
    .expect("well formed");

    let report = System::from_projection(&global).check(ExplorationBound::default());

    assert!(
        report.liveness.counterexample().is_none(),
        "the absence of a path to termination cannot be witnessed inside a truncated search"
    );
    assert!(matches!(report.liveness, Verdict::Inconclusive { .. }));
}

#[test]
fn a_protocol_that_can_never_terminate_fails_liveness_when_the_search_is_complete() {
    let system = System::from_locals([
        (
            Role::new("runner"),
            select("ledger", "observation", LocalType::End),
        ),
        (Role::new("ledger"), LocalType::End),
    ]);

    let report = system.check(ExplorationBound::default());
    let counterexample = report
        .liveness
        .counterexample()
        .expect("no state can reach a clean terminal, and the search was exhaustive");
    assert_eq!(counterexample.property, Property::Liveness);
}

#[test]
fn an_inconclusive_report_does_not_support_a_sufficiency_claim() {
    let global = GlobalType::rec(
        "loop",
        GlobalType::message("worker", "ledger", "observation", GlobalType::var("loop")),
    )
    .well_formed()
    .expect("well formed");

    let report = System::from_projection(&global).check(ExplorationBound::default());
    let omissions = report.omissions();

    assert!(!omissions.groups.is_empty());
    assert!(
        !omissions.supports_sufficiency_claim(),
        "an unexplored state space is InfluenceClass::Unknown, and unknown never counts toward sufficiency"
    );
    assert_eq!(report.deadlock.influence(), InfluenceClass::Unknown);
}

#[test]
fn a_fully_proved_report_omits_nothing() {
    let report = plan_execute_verify().check(ExplorationBound::default());
    let omissions = report.omissions();

    assert!(omissions.groups.is_empty());
    assert!(omissions.supports_sufficiency_claim());
    assert_eq!(report.deadlock.influence(), InfluenceClass::Zero);
}

#[test]
fn a_counterexample_found_inside_a_truncated_search_is_still_a_counterexample() {
    let report = grant_before_patch_deadlock().check(ExplorationBound::new(2, 400, 1));

    assert!(
        report.deadlock.is_refuted() || matches!(report.deadlock, Verdict::Inconclusive { .. }),
        "either shape is honest; what is forbidden is Holds"
    );
    assert!(!report.deadlock.is_proof());
}

#[test]
fn a_verdict_round_trips_through_serde_without_losing_the_distinction() {
    let inconclusive = System::from_projection(
        &GlobalType::rec(
            "loop",
            GlobalType::message("worker", "ledger", "observation", GlobalType::var("loop")),
        )
        .well_formed()
        .expect("well formed"),
    )
    .check(ExplorationBound::default());

    let encoded = serde_json::to_string(&inconclusive).expect("serialises");
    assert!(encoded.contains("inconclusive"));
    assert!(!encoded.contains("\"holds\""));

    let decoded: bioprism_choreography::ModelCheckReport =
        serde_json::from_str(&encoded).expect("round trips");
    assert!(!decoded.deadlock.is_proof());
    assert_eq!(decoded.truncated, Some(BoundHit::ChannelCapacity));
}
