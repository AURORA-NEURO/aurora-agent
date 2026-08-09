//! 23.48: reachability, the closure protocol, and semantic compaction that refuses to orphan.

use bioprism_fabric::lifecycle::{
    check_shutdown, claim_global_deletion, close_thread, resume_permitted, ClaimId, ClosureCheck,
    ClosureEvidence, ClosureOutcome, CompactionPlan, CompactionRefusal, ContinuationTerms,
    DeletionAttestation, DerivationGraph, LifecycleError, LifecycleGraph, LifecycleObject,
    MustSurvive, ObjectId, ObjectKind, ObjectState, Root, ShutdownStep, VerificationStrength,
};
use bioprism_fabric::reputation::LogicalTime;
use std::collections::BTreeSet;

fn satisfied_evidence() -> ClosureEvidence {
    ClosureEvidence {
        irreversible_effects_recorded: true,
        compensations_settled: true,
        disputes_exported: true,
        retention_scheduled: true,
        terminal_result_emitted: true,
    }
}

#[test]
fn a_purged_object_cannot_transition_to_anything() {
    for state in ObjectState::ALL {
        assert!(!ObjectState::Purged.may_transition_to(state));
    }
}

#[test]
fn only_scheduled_for_deletion_precedes_purged() {
    for state in ObjectState::ALL {
        let permitted = state.may_transition_to(ObjectState::Purged);
        assert_eq!(permitted, state == ObjectState::ScheduledForDeletion);
    }
}

#[test]
fn an_illegal_transition_is_refused_and_names_both_states() {
    let mut graph = LifecycleGraph::new()
        .with(LifecycleObject::new("thread-1", ObjectKind::Thread));
    assert!(graph
        .transition(&ObjectId::new("thread-1"), ObjectState::Quiescent)
        .is_ok());
    match graph
        .transition(&ObjectId::new("thread-1"), ObjectState::Purged)
        .unwrap_err()
    {
        LifecycleError::IllegalTransition { from, to, .. } => {
            assert_eq!(from, ObjectState::Quiescent);
            assert_eq!(to, ObjectState::Purged);
        }
        other => panic!("expected an illegal transition, got {other:?}"),
    }
}

#[test]
fn state_is_live_when_reachable_from_a_root_and_unreachable_otherwise() {
    let graph = LifecycleGraph::new()
        .with(
            LifecycleObject::new("thread-1", ObjectKind::Thread)
                .rooted_by(Root::ActiveThread)
                .referencing("capsule-1"),
        )
        .with(LifecycleObject::new("capsule-1", ObjectKind::ContextCapsule))
        .with(LifecycleObject::new("orphan", ObjectKind::Artifact));
    let live = graph.live_set(LogicalTime(0));
    assert!(live.contains(&ObjectId::new("capsule-1")));
    assert!(graph
        .unreachable(LogicalTime(0))
        .contains(&ObjectId::new("orphan")));
}

#[test]
fn an_identical_content_hash_does_not_confer_retention_because_liveness_is_keyed_by_identity() {
    let graph = LifecycleGraph::new()
        .with(
            LifecycleObject::new("pinned", ObjectKind::Artifact)
                .rooted_by(Root::UserPinnedArtifact),
        )
        .with(LifecycleObject::new("duplicate", ObjectKind::Artifact));
    let live = graph.live_set(LogicalTime(0));
    assert!(live.contains(&ObjectId::new("pinned")));
    assert!(!live.contains(&ObjectId::new("duplicate")));
}

#[test]
fn an_expired_object_stops_the_reachability_walk() {
    let graph = LifecycleGraph::new()
        .with(
            LifecycleObject::new("thread-1", ObjectKind::Thread)
                .rooted_by(Root::ActiveThread)
                .referencing("lease-1"),
        )
        .with(
            LifecycleObject::new("lease-1", ObjectKind::BudgetLease)
                .expiring_at(5)
                .referencing("downstream"),
        )
        .with(LifecycleObject::new("downstream", ObjectKind::Artifact));
    assert!(graph.live_set(LogicalTime(1)).contains(&ObjectId::new("downstream")));
    assert!(!graph.live_set(LogicalTime(9)).contains(&ObjectId::new("downstream")));
}

#[test]
fn a_thread_holding_an_open_commitment_reaches_a_partial_terminal_state_that_names_it() {
    let graph = LifecycleGraph::new()
        .with(
            LifecycleObject::new("thread-1", ObjectKind::Thread)
                .rooted_by(Root::ActiveThread)
                .referencing("commitment-1"),
        )
        .with(LifecycleObject::new("commitment-1", ObjectKind::Commitment));
    match close_thread(&graph, &satisfied_evidence(), LogicalTime(0)) {
        ClosureOutcome::PartialTerminal { open } => {
            assert!(open.iter().any(|condition| condition.check
                == ClosureCheck::MandatoryCommitmentsAccountedFor
                && condition.blocking.contains(&ObjectId::new("commitment-1"))));
        }
        other => panic!("expected a partial terminal state, got {other:?}"),
    }
}

#[test]
fn a_thread_with_nothing_outstanding_closes_cleanly() {
    let graph = LifecycleGraph::new()
        .with(LifecycleObject::new("thread-1", ObjectKind::Thread).rooted_by(Root::ActiveThread));
    assert!(close_thread(&graph, &satisfied_evidence(), LogicalTime(0)).is_clean());
}

#[test]
fn an_unasserted_closure_check_blocks_closure_even_with_an_empty_object_graph() {
    let graph = LifecycleGraph::new();
    match close_thread(&graph, &ClosureEvidence::default(), LogicalTime(0)) {
        ClosureOutcome::PartialTerminal { open } => assert_eq!(open.len(), 5),
        other => panic!("expected a partial terminal state, got {other:?}"),
    }
}

#[test]
fn an_active_grant_or_lease_left_reachable_is_reported_as_a_leak() {
    let graph = LifecycleGraph::new()
        .with(
            LifecycleObject::new("thread-1", ObjectKind::Thread)
                .rooted_by(Root::ActiveThread)
                .referencing("grant-1")
                .referencing("sub-1"),
        )
        .with(LifecycleObject::new("grant-1", ObjectKind::Grant))
        .with(LifecycleObject::new("sub-1", ObjectKind::Subscription));
    let leaked = graph.leaked_handles(LogicalTime(0));
    assert!(leaked.contains(&ObjectId::new("grant-1")));
    assert!(leaked.contains(&ObjectId::new("sub-1")));
}

fn derivations() -> DerivationGraph {
    DerivationGraph::new()
        .deriving("conclusion:patch-is-correct", &["claim:test-passes", "claim:no-regression"])
        .deriving("claim:test-passes", &["observation:ci-run-88"])
        .live("conclusion:patch-is-correct")
}

#[test]
fn a_compaction_that_would_orphan_a_live_conclusion_is_refused_naming_the_conclusion() {
    let plan = CompactionPlan::new(0, 100).dropping("claim:test-passes");
    match plan.check(&derivations(), &MustSurvive::new()).unwrap_err() {
        CompactionRefusal::WouldOrphanConclusion {
            premise,
            conclusions,
        } => {
            assert_eq!(premise, ClaimId::new("claim:test-passes"));
            assert!(conclusions.contains(&ClaimId::new("conclusion:patch-is-correct")));
        }
        other => panic!("expected an orphan refusal, got {other:?}"),
    }
}

#[test]
fn a_transitively_supporting_observation_is_protected_as_well_as_the_direct_premise() {
    let plan = CompactionPlan::new(0, 100).dropping("observation:ci-run-88");
    assert!(matches!(
        plan.check(&derivations(), &MustSurvive::new()).unwrap_err(),
        CompactionRefusal::WouldOrphanConclusion { .. }
    ));
}

#[test]
fn a_claim_nothing_live_depends_on_may_be_compacted() {
    let plan = CompactionPlan::new(0, 100).dropping("claim:unrelated-note");
    let certificate = plan
        .check(&derivations(), &MustSurvive::new())
        .expect("nothing live rests on it");
    assert_eq!(
        certificate.verification_after,
        VerificationStrength::PrefixCheckpointOnly
    );
    assert!(certificate.dropped.contains(&ClaimId::new("claim:unrelated-note")));
}

#[test]
fn verification_after_compaction_is_never_reported_as_strong_as_before() {
    let certificate = CompactionPlan::new(0, 10)
        .dropping("claim:x")
        .check(&DerivationGraph::new(), &MustSurvive::new())
        .expect("nothing to orphan");
    assert_ne!(certificate.verification_after, VerificationStrength::Full);
}

#[test]
fn compaction_must_declare_a_non_empty_retained_window() {
    match CompactionPlan::new(10, 10)
        .check(&DerivationGraph::new(), &MustSurvive::new())
        .unwrap_err()
    {
        CompactionRefusal::EmptyRetainedWindow { .. } => {}
        other => panic!("expected an empty-window refusal, got {other:?}"),
    }
}

#[test]
fn an_alternative_in_an_unresolved_conflict_survives_compaction() {
    let plan = CompactionPlan::new(0, 100).dropping("claim:a");
    let must = MustSurvive::new().conflict("claim:a");
    assert!(matches!(
        plan.check(&DerivationGraph::new(), &must).unwrap_err(),
        CompactionRefusal::UnresolvedConflict { .. }
    ));
}

#[test]
fn retraction_lineage_and_legal_holds_survive_compaction() {
    let must = MustSurvive::new().lineage("claim:retracted").held("claim:frozen");
    assert!(matches!(
        CompactionPlan::new(0, 100)
            .dropping("claim:retracted")
            .check(&DerivationGraph::new(), &must)
            .unwrap_err(),
        CompactionRefusal::RetractionLineage { .. }
    ));
    assert!(matches!(
        CompactionPlan::new(0, 100)
            .dropping("claim:frozen")
            .check(&DerivationGraph::new(), &must)
            .unwrap_err(),
        CompactionRefusal::LegalHold { .. }
    ));
}

fn continuation() -> ContinuationTerms {
    ContinuationTerms {
        id: ObjectId::new("cont-1"),
        created_at: LogicalTime(10),
        max_age: 5,
        owner: "alice".into(),
        transferable: false,
        open_commitments: BTreeSet::new(),
    }
}

#[test]
fn an_expired_continuation_cannot_be_resumed_however_available_its_bytes_are() {
    assert!(resume_permitted(&continuation(), "alice", LogicalTime(12)).is_ok());
    assert!(matches!(
        resume_permitted(&continuation(), "alice", LogicalTime(100)).unwrap_err(),
        LifecycleError::ContinuationExpired { .. }
    ));
}

#[test]
fn a_non_transferable_continuation_refuses_a_requester_that_is_not_its_owner() {
    assert!(matches!(
        resume_permitted(&continuation(), "bob", LogicalTime(12)).unwrap_err(),
        LifecycleError::ContinuationNotTransferable { .. }
    ));
}

#[test]
fn a_molecule_shutdown_that_revokes_grants_before_draining_commitments_is_refused() {
    let out_of_order = [
        ShutdownStep::StopAdmissions,
        ShutdownStep::RevokeNestedGrants,
        ShutdownStep::DrainOrTransferCommitments,
    ];
    assert!(matches!(
        check_shutdown(&out_of_order).unwrap_err(),
        LifecycleError::ShutdownOutOfOrder { .. }
    ));
}

#[test]
fn a_shutdown_that_stops_early_is_incomplete_and_names_the_missing_step() {
    match check_shutdown(&ShutdownStep::SEQUENCE[..6]).unwrap_err() {
        LifecycleError::ShutdownIncomplete { missing } => {
            assert_eq!(missing, ShutdownStep::ArchiveBoundConfiguration)
        }
        other => panic!("expected an incomplete shutdown, got {other:?}"),
    }
    assert!(check_shutdown(&ShutdownStep::SEQUENCE).is_ok());
}

#[test]
fn global_deletion_cannot_be_claimed_without_an_attestation_from_every_participant() {
    let required: BTreeSet<String> = ["org-a".to_string(), "org-b".to_string()]
        .into_iter()
        .collect();
    match claim_global_deletion(
        &required,
        &[DeletionAttestation::Deleted {
            participant: "org-a".into(),
        }],
    )
    .unwrap_err()
    {
        LifecycleError::DeletionAttestationMissing { participants } => {
            assert!(participants.contains("org-b"))
        }
        other => panic!("expected a missing attestation, got {other:?}"),
    }
}

#[test]
fn a_legal_hold_is_a_different_failure_from_a_silent_participant() {
    let required: BTreeSet<String> = ["org-a".to_string(), "org-b".to_string()]
        .into_iter()
        .collect();
    let attestations = [
        DeletionAttestation::Deleted {
            participant: "org-a".into(),
        },
        DeletionAttestation::LegalHold {
            participant: "org-b".into(),
            basis: "litigation".into(),
        },
    ];
    assert!(matches!(
        claim_global_deletion(&required, &attestations).unwrap_err(),
        LifecycleError::DeletionNotUniversal { .. }
    ));

    let all_deleted = [
        DeletionAttestation::Deleted {
            participant: "org-a".into(),
        },
        DeletionAttestation::Deleted {
            participant: "org-b".into(),
        },
    ];
    assert!(claim_global_deletion(&required, &all_deleted).is_ok());
}
