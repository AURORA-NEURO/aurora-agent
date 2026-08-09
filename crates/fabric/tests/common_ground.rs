//! 23.44: the epistemic CRDT's merge laws and the three levels of common ground.

use bioprism_fabric::effect::Irreversibility;
use bioprism_fabric::flow::{FlowLabel, Labelling, Principal, Sensitivity};
use bioprism_fabric::ground::{
    common_ground, conflicts, interpret, may_proceed_under_partition, ConsistencyMode, DerivedState,
    EpistemicOp, GroundError, GroundLevel, InterpretationPolicy, OpKind, Proposition, Replica,
    Scope, Weighting,
};
use bioprism_fabric::reputation::LogicalTime;

fn public() -> Labelling {
    Labelling::Labelled(FlowLabel::open_at(Sensitivity::Public))
}

fn confidential() -> Labelling {
    Labelling::Labelled(FlowLabel::open_at(Sensitivity::Confidential).in_compartment("site-7"))
}

fn assert_op(
    author: &str,
    proposition: &str,
    value: &str,
    clock: u64,
    parents: Vec<bioprism_fabric::ground::OpId>,
    label: Labelling,
) -> EpistemicOp {
    EpistemicOp::new(
        author,
        parents,
        LogicalTime(clock),
        label,
        OpKind::Assert {
            proposition: Proposition::new(proposition),
            value: value.to_string(),
            confidence_bp: 8_000,
        },
    )
    .expect("encodable")
}

/// A non-trivial event set: two contradictory assertions, an attestation, a retraction and a
/// supersession, wired together by causal parents.
fn scenario() -> Vec<EpistemicOp> {
    let a1 = assert_op("alice", "commit-order", "a-first", 1, vec![], public());
    let b1 = assert_op("bob", "commit-order", "b-first", 2, vec![], public());
    let attest = EpistemicOp::new(
        "carol",
        vec![a1.id.clone()],
        LogicalTime(3),
        public(),
        OpKind::Attest {
            target: a1.id.clone(),
            method: "transaction-log".into(),
            passed: true,
        },
    )
    .unwrap();
    let retract = EpistemicOp::new(
        "bob",
        vec![b1.id.clone(), attest.id.clone()],
        LogicalTime(4),
        public(),
        OpKind::Retract {
            target: b1.id.clone(),
            reason: "log disagrees".into(),
        },
    )
    .unwrap();
    let supersede = EpistemicOp::new(
        "alice",
        vec![a1.id.clone(), retract.id.clone()],
        LogicalTime(5),
        public(),
        OpKind::Supersede {
            old: a1.id.clone(),
            relation: "refines".into(),
            proposition: Proposition::new("commit-order"),
            value: "a-first".into(),
        },
    )
    .unwrap();
    vec![a1, b1, attest, retract, supersede]
}

fn replica_from(order: &[usize], ops: &[EpistemicOp]) -> Replica {
    let mut replica = Replica::new();
    for index in order {
        replica.append(ops[*index].clone());
    }
    replica
}

#[test]
fn merge_is_commutative_on_a_case_with_a_conflict_a_retraction_and_a_supersession() {
    let ops = scenario();
    let left = replica_from(&[0, 1, 2], &ops);
    let right = replica_from(&[3, 4], &ops);

    let mut a = left.clone();
    a.merge(&right);
    let mut b = right.clone();
    b.merge(&left);

    assert_eq!(a.admitted_count(), ops.len());
    assert_eq!(a.digest().unwrap(), b.digest().unwrap());
}

#[test]
fn merge_is_associative() {
    let ops = scenario();
    let x = replica_from(&[0], &ops);
    let y = replica_from(&[1, 2], &ops);
    let z = replica_from(&[3, 4], &ops);

    let mut left = x.clone();
    left.merge(&y);
    left.merge(&z);

    let mut yz = y.clone();
    yz.merge(&z);
    let mut right = x.clone();
    right.merge(&yz);

    assert_eq!(left.digest().unwrap(), right.digest().unwrap());
}

#[test]
fn merge_is_idempotent_and_a_duplicate_delivery_changes_nothing() {
    let ops = scenario();
    let mut replica = replica_from(&[0, 1, 2, 3, 4], &ops);
    let before = replica.digest().unwrap();
    let copy = replica.clone();
    let report = replica.merge(&copy);
    assert!(report.admitted.is_empty());
    assert_eq!(replica.digest().unwrap(), before);
}

#[test]
fn reordered_delivery_converges_because_an_op_with_absent_parents_is_held_not_dropped() {
    let ops = scenario();
    let forward = replica_from(&[0, 1, 2, 3, 4], &ops);
    let backward = replica_from(&[4, 3, 2, 1, 0], &ops);
    assert_eq!(backward.pending_count(), 0);
    assert_eq!(forward.digest().unwrap(), backward.digest().unwrap());
}

#[test]
fn an_op_whose_parents_never_arrive_stays_pending_and_is_never_admitted() {
    let ops = scenario();
    let replica = replica_from(&[4], &ops);
    assert_eq!(replica.admitted_count(), 0);
    assert_eq!(replica.pending_count(), 1);
}

#[test]
fn a_retraction_adds_an_event_and_never_removes_the_original_claim() {
    let ops = scenario();
    let replica = replica_from(&[0, 1, 2, 3, 4], &ops);
    assert!(replica.get(&ops[1].id).is_some());
    let states = interpret(&replica, InterpretationPolicy::new(100), LogicalTime(10));
    assert!(states.contains_key(&Proposition::new("commit-order")));
}

#[test]
fn two_replicas_asserting_incompatible_values_produce_a_conflict_a_summariser_cannot_collapse() {
    let ops = scenario();
    let replica = replica_from(&[0, 1, 2, 3, 4], &ops);
    let found = conflicts(&replica);
    assert_eq!(found.len(), 1);
    assert!(found[0].is_unresolved());
    assert!(found[0].alternatives.len() >= 2);
    assert!(!found[0].decisive_evidence_needed.is_empty());
}

#[test]
fn a_resolution_event_is_the_only_thing_that_moves_a_conflict_out_of_unresolved() {
    let ops = scenario();
    let mut replica = replica_from(&[0, 1, 2, 3, 4], &ops);
    let resolve = EpistemicOp::new(
        "arbiter",
        vec![ops[4].id.clone()],
        LogicalTime(6),
        public(),
        OpKind::Resolve {
            dispute: Proposition::new("commit-order"),
            decision: "a-first".into(),
            basis: "server-ack-timestamp".into(),
        },
    )
    .unwrap();
    replica.append(resolve);
    assert!(!conflicts(&replica)[0].is_unresolved());
}

#[test]
fn an_expired_assumption_derives_as_stale_rather_than_provisionally_accepted() {
    let mut replica = Replica::new();
    let assume = EpistemicOp::new(
        "alice",
        vec![],
        LogicalTime(1),
        public(),
        OpKind::Assume {
            proposition: Proposition::new("branch-plan"),
            expiry: LogicalTime(5),
        },
    )
    .unwrap();
    replica.append(assume);
    let policy = InterpretationPolicy::new(1_000);
    assert_eq!(
        interpret(&replica, policy, LogicalTime(2))[&Proposition::new("branch-plan")],
        DerivedState::ProvisionallyAccepted
    );
    assert_eq!(
        interpret(&replica, policy, LogicalTime(9))[&Proposition::new("branch-plan")],
        DerivedState::Stale
    );
}

#[test]
fn a_replica_projected_for_a_recipient_reports_how_many_operations_it_did_not_receive() {
    let mut replica = Replica::new();
    replica.append(assert_op("alice", "p", "v", 1, vec![], public()));
    replica.append(assert_op("alice", "q", "w", 2, vec![], confidential()));
    let reader = Principal::new("reader", FlowLabel::open_at(Sensitivity::Public));
    let redacted = replica.replicate_for(&reader);
    assert_eq!(redacted.replica.admitted_count(), 1);
    assert_eq!(redacted.omitted_count, 1);
    assert!(!redacted.view_is_complete());
}

#[test]
fn a_and_b_both_knowing_x_is_not_common_ground() {
    let mut replica = Replica::new();
    replica.append(assert_op("alice", "x", "true", 1, vec![], public()));
    replica.append(assert_op("bob", "x", "true", 2, vec![], public()));

    let scope = Scope {
        thread: "t".into(),
        members: vec![
            Principal::new("alice", FlowLabel::open_at(Sensitivity::Public)),
            Principal::new("bob", FlowLabel::open_at(Sensitivity::Public)),
        ],
        purpose: "plan".into(),
        as_of: LogicalTime(10),
    };
    let ground = common_ground(&replica, &scope);
    assert_eq!(
        ground.levels[&Proposition::new("x")],
        GroundLevel::Distributed
    );
    assert!(ground.distributed().contains(&Proposition::new("x")));
    assert!(ground
        .at_least(GroundLevel::Mutual { depth: 2 })
        .is_empty());
}

#[test]
fn two_independently_formed_beliefs_that_cross_reference_are_mutual_and_not_publicly_grounded() {
    let alice = assert_op("alice", "x", "true", 1, vec![], public());
    let bob = assert_op("bob", "x", "true", 2, vec![], public());
    let alice_ack = assert_op("alice", "x", "true", 3, vec![bob.id.clone()], public());
    let bob_ack = assert_op("bob", "x", "true", 4, vec![alice.id.clone()], public());

    let mut replica = Replica::new();
    for op in [alice, bob, alice_ack, bob_ack] {
        replica.append(op);
    }

    let scope = Scope {
        thread: "t".into(),
        members: vec![
            Principal::new("alice", FlowLabel::open_at(Sensitivity::Public)),
            Principal::new("bob", FlowLabel::open_at(Sensitivity::Public)),
        ],
        purpose: "plan".into(),
        as_of: LogicalTime(10),
    };
    assert_eq!(
        common_ground(&replica, &scope).levels[&Proposition::new("x")],
        GroundLevel::Mutual { depth: 2 }
    );
}

#[test]
fn seeing_an_operation_in_a_shared_log_is_not_evidence_that_the_other_party_saw_yours() {
    let alice = assert_op("alice", "x", "true", 1, vec![], public());
    let bob = assert_op("bob", "x", "true", 2, vec![alice.id.clone()], public());

    let mut replica = Replica::new();
    replica.append(alice);
    replica.append(bob);

    let scope = Scope {
        thread: "t".into(),
        members: vec![
            Principal::new("alice", FlowLabel::open_at(Sensitivity::Public)),
            Principal::new("bob", FlowLabel::open_at(Sensitivity::Public)),
        ],
        purpose: "plan".into(),
        as_of: LogicalTime(10),
    };
    assert_eq!(
        common_ground(&replica, &scope).levels[&Proposition::new("x")],
        GroundLevel::Distributed
    );
}

#[test]
fn a_publicly_grounded_proposition_is_stronger_than_a_merely_mutual_one() {
    let anchor = assert_op("alice", "x", "true", 1, vec![], public());
    let bob = assert_op("bob", "x", "true", 2, vec![anchor.id.clone()], public());
    let alice_ack = assert_op("alice", "x", "true", 3, vec![bob.id.clone()], public());

    let mut replica = Replica::new();
    replica.append(anchor);
    replica.append(bob);
    replica.append(alice_ack);

    let scope = Scope {
        thread: "t".into(),
        members: vec![
            Principal::new("alice", FlowLabel::open_at(Sensitivity::Public)),
            Principal::new("bob", FlowLabel::open_at(Sensitivity::Public)),
        ],
        purpose: "plan".into(),
        as_of: LogicalTime(10),
    };
    assert_eq!(
        common_ground(&replica, &scope).levels[&Proposition::new("x")],
        GroundLevel::PubliclyGrounded
    );
}

#[test]
fn a_proposition_one_member_cannot_see_is_asymmetric_and_not_common_ground_at_any_level() {
    let mut replica = Replica::new();
    replica.append(assert_op("alice", "secret", "v", 1, vec![], confidential()));

    let scope = Scope {
        thread: "t".into(),
        members: vec![
            Principal::new(
                "alice",
                FlowLabel::open_at(Sensitivity::Confidential).in_compartment("site-7"),
            ),
            Principal::new("bob", FlowLabel::open_at(Sensitivity::Public)),
        ],
        purpose: "plan".into(),
        as_of: LogicalTime(10),
    };
    let ground = common_ground(&replica, &scope);
    assert!(ground.levels.is_empty());
    assert!(ground
        .asymmetric
        .get(&Proposition::new("secret"))
        .unwrap()
        .contains("alice"));
}

#[test]
fn an_irreversible_effect_may_not_proceed_on_a_view_that_only_eventually_converges() {
    assert!(matches!(
        may_proceed_under_partition(Irreversibility::E4, ConsistencyMode::LocalTentative)
            .unwrap_err(),
        GroundError::EffectRequiresConvergedView { .. }
    ));
    assert!(
        may_proceed_under_partition(Irreversibility::E4, ConsistencyMode::StopAndSynchronize)
            .is_ok()
    );
    assert!(may_proceed_under_partition(Irreversibility::E1, ConsistencyMode::BoundedEffects).is_ok());
}

#[test]
fn reputation_never_substitutes_for_missing_evidence_in_the_weighting() {
    let weighting = Weighting {
        directness_bp: 9_000,
        method_bp: 9_000,
        freshness_bp: 9_000,
        independence_bp: 9_000,
        calibration_bp: 9_000,
        conflict_of_interest_penalty_bp: 0,
        custody_bp: 9_000,
        domain_validity_bp: 9_000,
    };
    assert!(weighting.weigh(0).is_none());
    assert!(weighting.weigh(1).is_some());
}

#[test]
fn an_operation_naming_itself_as_its_own_parent_is_rejected_as_malformed() {
    let op = assert_op("alice", "x", "v", 1, vec![], public());
    let self_parented = EpistemicOp {
        parents: [op.id.clone()].into_iter().collect(),
        ..op
    };
    let mut replica = Replica::new();
    let report = replica.append(self_parented);
    assert_eq!(report.rejected_cyclic.len(), 1);
    assert_eq!(replica.admitted_count(), 0);
}
