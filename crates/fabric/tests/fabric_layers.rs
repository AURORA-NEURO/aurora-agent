//! 23.11, 23.15, 23.17, 23.20, 23.42 and 23.01: the layers around the algebra.

mod common;

use bioprism_choreography::{GlobalType, Label as ChoreoLabel, Role as ChoreoRole};
use bioprism_fabric::algebra::{fuse, seq, SubstitutionContext};
use bioprism_fabric::blackboard::{
    flow_between_scopes, Blackboard, BlackboardError, Entry, EntryKind, LeaseTable, MemoryScope,
    Reducer, StorageLayer, Subscription, Topic,
};
use bioprism_fabric::contract::{AssuranceProfile, ComponentId, DeclaredCommitment};
use bioprism_fabric::effect::{EffectKind, EffectSet};
use bioprism_fabric::flow::{FlowLabel, Labelling, Principal, Sensitivity};
use bioprism_fabric::molecule::{
    admission_gaps, required_bump, split, Binding, Guarantees, Molecule, MoleculeCard,
    MoleculeError, Version, VersionBump,
};
use bioprism_fabric::negotiate::{
    discount, Calibration, Condition, ContractNet, DiscountedEstimate, NegotiationError, Offer,
    Rejection, Terms,
};
use bioprism_fabric::reputation::{
    Attestation, CapabilityCard, EvidenceLayer, LogicalTime, ValidityWindow,
};
use bioprism_fabric::stack::{
    fail_closed, layer_owner, negotiate_session, Adapter, AdapterRung, BridgeRequest,
    Compatibility, LossPolicy, SemanticFeature, SessionCapabilities, StackError, STACK,
};
use bioprism_fabric::synth::{
    correlated_failure_penalty, evaluate, pareto_frontier, synthesize, AssuranceRequirement,
    Candidacy, Candidate, Goal, RejectionReason, Role, RoleGraph, Score, SynthesisError,
};
use bioprism_fabric::topology::{
    Controller, DwellPolicy, Member, Pattern, ReasonCode, Topology, TopologyAction, TopologyError,
};
use bioprism_weave::Capability;
use std::collections::{BTreeMap, BTreeSet};

use common::{effects, extractor, permissive_envelope, report, verifier};

fn molecule() -> Molecule {
    let choreography = GlobalType::message(
        ChoreoRole::new("x"),
        ChoreoRole::new("v"),
        ChoreoLabel::new("analysis"),
        GlobalType::End,
    )
    .well_formed()
    .expect("well-formed");
    let composition =
        fuse(&[extractor("x"), verifier("v")], &choreography, &report()).expect("roles filled");
    let attribution: BTreeMap<String, String> = [
        ("verdict".to_string(), "reviewer".to_string()),
        ("evidence".to_string(), "reader".to_string()),
    ]
    .into_iter()
    .collect();
    Molecule::assemble(
        "scientific-reproducer",
        Version::new(1, 0, 0),
        vec![
            Binding {
                role: "reader".into(),
                participant: extractor("x"),
            },
            Binding {
                role: "reviewer".into(),
                participant: verifier("v"),
            },
        ],
        composition,
        report(),
        Guarantees::new(),
        attribution,
    )
    .expect("attributable")
}

#[test]
fn a_molecule_that_cannot_attribute_an_exported_output_is_refused_at_assembly() {
    let choreography = GlobalType::message(
        ChoreoRole::new("x"),
        ChoreoRole::new("v"),
        ChoreoLabel::new("analysis"),
        GlobalType::End,
    )
    .well_formed()
    .unwrap();
    let composition = fuse(&[extractor("x"), verifier("v")], &choreography, &report()).unwrap();
    let error = Molecule::assemble(
        "opaque",
        Version::new(1, 0, 0),
        vec![Binding {
            role: "reader".into(),
            participant: extractor("x"),
        }],
        composition,
        report(),
        Guarantees::new(),
        BTreeMap::new(),
    )
    .unwrap_err();
    assert!(matches!(error, MoleculeError::UnattributableOutput { .. }));
}

#[test]
fn a_composite_presents_one_interface_and_still_names_the_participant_behind_each_output() {
    let molecule = molecule();
    assert_eq!(molecule.attribute("verdict").unwrap().role, "reviewer");
    assert_eq!(molecule.attribute("evidence").unwrap().role, "reader");
    assert!(molecule.attribute("nonexistent").is_err());
    assert!(molecule.encapsulation().attribution.contains_key("verdict"));
}

#[test]
fn a_molecule_violating_its_declared_effect_ceiling_is_refused() {
    let choreography = GlobalType::message(
        ChoreoRole::new("x"),
        ChoreoRole::new("v"),
        ChoreoLabel::new("analysis"),
        GlobalType::End,
    )
    .well_formed()
    .unwrap();
    let composition = fuse(&[extractor("x"), verifier("v")], &choreography, &report()).unwrap();
    let guarantees =
        Guarantees::new().with_effect_ceiling(effects(&[(EffectKind::PureCompute, "local")]));
    let error = Molecule::assemble(
        "over-reaching",
        Version::new(1, 0, 0),
        vec![],
        composition,
        report(),
        guarantees,
        BTreeMap::new(),
    )
    .unwrap_err();
    assert!(matches!(error, MoleculeError::GuaranteeViolated { .. }));
}

#[test]
fn a_card_keeps_declared_and_verified_capabilities_in_separate_fields() {
    let card = MoleculeCard::of(&molecule())
        .unwrap()
        .declaring("capability/reproduce")
        .declaring("capability/adjudicate")
        .verified_by(
            Attestation::new(
                "scientific-reproducer",
                "capability/reproduce",
                "prism",
                EvidenceLayer::PrismEvaluated,
                ValidityWindow::new(0, 100),
            )
            .unwrap(),
        );
    assert_eq!(card.declared_capabilities.len(), 2);
    assert_eq!(card.verified_capabilities.len(), 1);
    let unverified = card.unverified_claims(LogicalTime(1));
    assert_eq!(unverified.len(), 1);
    assert!(unverified.contains("capability/adjudicate"));
}

#[test]
fn a_card_with_no_declared_effects_fails_the_one_admission_check_this_crate_can_run() {
    let mut card = MoleculeCard::of(&molecule()).unwrap();
    assert!(card.validate().is_ok());
    card.required_effects = EffectSet::new();
    assert!(matches!(
        card.validate().unwrap_err(),
        MoleculeError::CardIncomplete { .. }
    ));
    assert_eq!(admission_gaps().len(), 7);
}

#[test]
fn splitting_a_molecule_yields_the_participants_own_contract_and_no_wider_authority() {
    let molecule = molecule();
    let fragment = split(&molecule, "reviewer").unwrap();
    assert_eq!(fragment.authority, verifier("v").authority);
    assert!(split(&molecule, "nobody").is_err());
}

#[test]
fn changing_the_public_type_contract_forces_a_major_version() {
    let old = molecule();
    let mut new = molecule();
    new.exported = common::thin_report();
    let (bump, breaking) = required_bump(&old, &new);
    assert_eq!(bump, VersionBump::Major);
    assert!(breaking.contains(&bioprism_fabric::molecule::BreakingChange::PublicTypeContract));
}

#[test]
fn a_blackboard_has_no_setter_and_a_retraction_leaves_history_in_place() {
    let topic = Topic::new("claims/current");
    let mut board = Blackboard::new();
    board
        .publish(Entry::new(
            "e1",
            topic.clone(),
            "alice",
            EntryKind::Observation { value: "a".into() },
        ))
        .unwrap();
    board
        .publish(Entry::new(
            "e2",
            topic.clone(),
            "bob",
            EntryKind::Retraction {
                target: "e1".into(),
            },
        ))
        .unwrap();
    let view = board.project(&topic, LogicalTime(0));
    assert!(view.live.is_empty());
    assert!(view.retracted.contains("e1"));
    assert_eq!(view.history_length, 2);
    assert!(view.history_preserved());
}

#[test]
fn two_concurrent_conflicting_claims_both_survive_the_projection() {
    let topic = Topic::new("claims/current");
    let mut board = Blackboard::new();
    for (id, author, value) in [("e1", "alice", "a"), ("e2", "bob", "b")] {
        board
            .publish(Entry::new(
                id,
                topic.clone(),
                author,
                EntryKind::Observation {
                    value: value.into(),
                },
            ))
            .unwrap();
    }
    assert_eq!(board.project(&topic, LogicalTime(0)).live.len(), 2);
}

#[test]
fn a_duplicate_idempotency_key_is_refused_rather_than_appended_twice() {
    let mut board = Blackboard::new();
    let entry = Entry::new(
        "e1",
        Topic::new("artifacts/patches"),
        "alice",
        EntryKind::Observation {
            value: "patch".into(),
        },
    )
    .keyed("run-88");
    board.publish(entry.clone()).unwrap();
    assert!(matches!(
        board.publish(entry).unwrap_err(),
        BlackboardError::DuplicateIdempotencyKey { .. }
    ));
    assert_eq!(board.len(), 1);
}

#[test]
fn a_subscription_never_delivers_an_entry_outside_the_subscribers_clearance() {
    let topic = Topic::new("artifacts/patches");
    let mut board = Blackboard::new();
    board
        .publish(
            Entry::new(
                "public",
                topic.clone(),
                "alice",
                EntryKind::Observation { value: "v".into() },
            )
            .labelled(Labelling::Labelled(FlowLabel::open_at(Sensitivity::Public))),
        )
        .unwrap();
    board
        .publish(
            Entry::new(
                "secret",
                topic.clone(),
                "alice",
                EntryKind::Observation { value: "v".into() },
            )
            .labelled(Labelling::Labelled(
                FlowLabel::open_at(Sensitivity::Restricted).in_compartment("site-7"),
            )),
        )
        .unwrap();
    let subscriber = Principal::new("reader", FlowLabel::open_at(Sensitivity::Public));
    let subscription = Subscription::new(subscriber, "artifacts/");
    let delivered = board.watch(&subscription, LogicalTime(0));
    assert_eq!(delivered.len(), 1);
    assert_eq!(delivered[0].id, "public");
}

#[test]
fn a_local_note_is_not_promoted_to_verified_ground_without_independent_verification() {
    let mut board = Blackboard::new();
    board
        .publish(Entry::new(
            "note",
            Topic::new("hypotheses/x"),
            "alice",
            EntryKind::Observation { value: "v".into() },
        ))
        .unwrap();
    let self_only: BTreeSet<String> = ["alice".to_string()].into_iter().collect();
    assert!(matches!(
        board.promote("note", &self_only, 2).unwrap_err(),
        BlackboardError::InsufficientIndependentVerification { .. }
    ));
    let independent: BTreeSet<String> = ["bob".to_string(), "carol".to_string()]
        .into_iter()
        .collect();
    board.promote("note", &independent, 2).unwrap();
    assert!(board.entries()[0].is_verified());
}

#[test]
fn a_claim_topic_has_no_reducer_because_a_crdt_cannot_decide_which_claim_wins() {
    assert!(matches!(
        Reducer::for_topic(&Topic::new("claims/current")).unwrap_err(),
        BlackboardError::NoReducerForEpistemicTopic { .. }
    ));
    assert_eq!(
        Reducer::for_topic(&Topic::new("presence/agents")).unwrap(),
        Reducer::PerAuthorLatest
    );
    assert_eq!(
        Reducer::for_topic(&Topic::new("budgets/alerts")).unwrap(),
        Reducer::Counter
    );
}

#[test]
fn only_the_event_log_and_artifacts_are_canonical() {
    assert!(StorageLayer::CanonicalEventLog.is_canonical());
    assert!(StorageLayer::ArtifactStore.is_canonical());
    assert!(!StorageLayer::WorkspaceIndex.is_canonical());
    assert!(!StorageLayer::LocalAgentCache.is_canonical());
}

#[test]
fn widening_a_memory_scope_without_an_explicit_policy_is_refused() {
    assert!(matches!(
        flow_between_scopes(
            MemoryScope::ParticipantLocal,
            MemoryScope::PublicRegistry,
            None
        )
        .unwrap_err(),
        BlackboardError::ScopeWideningWithoutPolicy { .. }
    ));
    assert!(flow_between_scopes(
        MemoryScope::ParticipantLocal,
        MemoryScope::PublicRegistry,
        Some("publication-review@1")
    )
    .is_ok());
    assert!(flow_between_scopes(MemoryScope::PublicRegistry, MemoryScope::TurnLocal, None).is_ok());
}

#[test]
fn an_exclusive_lease_on_an_external_resource_refuses_a_second_holder_and_names_the_first() {
    let mut leases = LeaseTable::new();
    leases.acquire("repo/main", "alice").unwrap();
    match leases.acquire("repo/main", "bob").unwrap_err() {
        BlackboardError::ResourceAlreadyLeased { holder, .. } => assert_eq!(holder, "alice"),
        other => panic!("expected a lease clash, got {other:?}"),
    }
    leases.release("repo/main", "alice").unwrap();
    assert!(leases.acquire("repo/main", "bob").is_ok());
}

fn terms(cost: u64, success_bp: u32) -> Terms {
    Terms {
        capability: "statistics.verify@2".into(),
        input_contract: "analysis-bundle@3".into(),
        output_contract: "verification-report@2".into(),
        expected_cost_minor: cost,
        max_cost_minor: cost * 2,
        declared_latency_units: 90,
        estimated_success_bp: success_bp,
        conditions: BTreeSet::new(),
        subcontracting_allowed: false,
        cancellation_fee_minor: 0,
    }
}

#[test]
fn a_prose_side_promise_is_recorded_and_is_never_part_of_the_binding_terms() {
    let offer = Offer::new("offer:1", "agent:stats-17", terms(60, 8_800), 100)
        .saying("we will usually beat the p50 by a lot");
    let digest = offer.digest().unwrap();
    let amended = Offer::new("offer:1", "agent:stats-17", terms(60, 8_800), 100)
        .saying("entirely different prose");
    assert_eq!(digest, amended.digest().unwrap());
    assert_eq!(offer.unencoded_promises().len(), 1);

    let repriced = Offer::new("offer:1", "agent:stats-17", terms(90, 8_800), 100);
    assert_ne!(digest, repriced.digest().unwrap());
}

#[test]
fn a_provider_with_no_calibration_history_is_neither_discounted_nor_trusted_at_face_value() {
    let terms = terms(60, 8_800);
    assert!(matches!(
        discount(&terms, None),
        DiscountedEstimate::Unmeasured { claimed_bp: 8_800 }
    ));
    match discount(
        &terms,
        Some(Calibration {
            realisation_bp: 5_000,
            sample_size: 40,
        }),
    ) {
        DiscountedEstimate::Discounted { adjusted_bp, .. } => assert_eq!(adjusted_bp, 4_400),
        other => panic!("expected a discount, got {other:?}"),
    }
}

#[test]
fn an_infeasible_offer_is_eliminated_for_a_different_reason_than_a_losing_one() {
    let mut net = ContractNet::open("verify-analysis", 500).providing("analysis-code");
    let mut conditional = terms(60, 8_800);
    conditional.conditions.insert(Condition {
        requires: "raw-sample-counts".into(),
    });
    net.receive(Offer::new("offer:conditional", "a", conditional, 100));
    net.receive(Offer::new("offer:expensive", "b", terms(400, 9_000), 100));
    net.receive(Offer::new("offer:stale", "c", terms(60, 9_000), 1));

    let rejections = net.screen(LogicalTime(5));
    assert!(matches!(
        rejections["offer:conditional"],
        Rejection::ConditionsUnmet { .. }
    ));
    assert!(matches!(
        rejections["offer:expensive"],
        Rejection::OverBudget { .. }
    ));
    assert!(matches!(
        rejections["offer:stale"],
        Rejection::Expired { .. }
    ));
}

/// `forbidding_subcontracting` is the only writer of the flag `screen` reads.
///
/// The default is permissive, so the builder is what makes the refusal reachable at all. A reader
/// deleting it as unused would leave `Rejection::SubcontractingForbidden` unreachable and every
/// subcontracted offer silently feasible — which is why the same offer is screened twice here,
/// once under each setting, rather than once under the restrictive one.
#[test]
fn only_forbidding_subcontracting_makes_a_subcontracted_offer_infeasible() {
    let mut subcontracted = terms(60, 8_800);
    subcontracted.subcontracting_allowed = true;

    let mut permissive = ContractNet::open("verify-analysis", 500);
    permissive.receive(Offer::new("offer:1", "a", subcontracted.clone(), 100));
    assert!(
        permissive.screen(LogicalTime(5)).is_empty(),
        "the open default must accept a subcontracted offer, or the builder proves nothing"
    );

    let mut forbidding = ContractNet::open("verify-analysis", 500).forbidding_subcontracting();
    forbidding.receive(Offer::new("offer:1", "a", subcontracted, 100));
    assert!(matches!(
        forbidding.screen(LogicalTime(5))["offer:1"],
        Rejection::SubcontractingForbidden
    ));
}

#[test]
fn an_acceptance_must_hold_a_real_reservation_and_reservations_cannot_be_duplicated() {
    let mut net = ContractNet::open("verify", 500);
    net.receive(Offer::new("offer:1", "a", terms(100, 9_000), 100));
    assert!(matches!(
        net.accept("offer:1").unwrap_err(),
        NegotiationError::AcceptWithoutReservation { .. }
    ));
    net.reserve("offer:1").unwrap();
    assert!(matches!(
        net.reserve("offer:1").unwrap_err(),
        NegotiationError::DuplicateReservation { .. }
    ));
    let award = net.accept("offer:1").unwrap();
    assert_eq!(award.reserved_minor, 200);
}

#[test]
fn reservations_cannot_exceed_the_coordinators_ceiling_because_the_kernel_budget_refuses() {
    let mut net = ContractNet::open("verify", 300);
    net.receive(Offer::new("offer:1", "a", terms(100, 9_000), 100));
    net.receive(Offer::new("offer:2", "b", terms(100, 9_000), 100));
    net.reserve("offer:1").unwrap();
    assert!(matches!(
        net.reserve("offer:2").unwrap_err(),
        NegotiationError::ReservationRefused { .. }
    ));
    assert!(net.total_reserved() <= 300);
}

#[test]
fn settlement_releases_what_was_reserved_and_not_spent() {
    let mut net = ContractNet::open("verify", 500);
    net.receive(Offer::new("offer:1", "a", terms(100, 9_000), 100));
    net.reserve("offer:1").unwrap();
    net.accept("offer:1").unwrap();
    let settlement = net.settle(120).unwrap();
    assert_eq!(settlement.released_minor, 80);
    assert!(matches!(
        ContractNet::open("x", 10).settle(0).unwrap_err(),
        NegotiationError::SettleWithoutAward { .. }
    ));
}

fn team() -> Topology {
    Topology::new(Pattern::Star)
        .with(Member::new("lead", "coordinator", 0).holding(Capability::ReadEvidence))
        .with(
            Member::new("worker", "patcher", 0)
                .holding(Capability::BranchWrite)
                .owing("deliver-patch"),
        )
}

#[test]
fn a_participant_holding_an_open_commitment_cannot_retire_but_can_be_evicted() {
    let mut topology = team();
    let retire = TopologyAction::Retire {
        participant: ComponentId::new("worker"),
    };
    match topology.apply(&retire).unwrap_err() {
        TopologyError::RetireWithOpenCommitments { commitments, .. } => {
            assert!(commitments.contains("deliver-patch"))
        }
        other => panic!("expected a refusal, got {other:?}"),
    }

    let evict = TopologyAction::Evict {
        participant: ComponentId::new("worker"),
        reason: "unresponsive".into(),
    };
    let change = topology.apply(&evict).unwrap();
    assert!(change.stranded_commitments.contains("deliver-patch"));
    assert_eq!(topology.size(), 1);
}

#[test]
fn fusion_does_not_create_a_new_source_of_authority() {
    let mut topology = team();
    let members: BTreeSet<ComponentId> = ["lead", "worker"]
        .into_iter()
        .map(ComponentId::new)
        .collect();
    let overreaching = TopologyAction::Fuse {
        members: members.clone(),
        molecule: ComponentId::new("molecule"),
        requested_authority: [Capability::PublishResult].into_iter().collect(),
    };
    assert!(matches!(
        topology.apply(&overreaching).unwrap_err(),
        TopologyError::FusionCreatedAuthority { .. }
    ));

    let legal = TopologyAction::Fuse {
        members,
        molecule: ComponentId::new("molecule"),
        requested_authority: [Capability::ReadEvidence, Capability::BranchWrite]
            .into_iter()
            .collect(),
    };
    topology.apply(&legal).unwrap();
    assert_eq!(
        topology
            .molecule_members(&ComponentId::new("molecule"))
            .unwrap()
            .len(),
        2
    );
}

#[test]
fn a_promotion_may_not_grant_a_capability_nobody_in_the_topology_holds() {
    let mut topology = team();
    assert!(matches!(
        topology
            .apply(&TopologyAction::Promote {
                participant: ComponentId::new("lead"),
                capability: Capability::PublishResult,
            })
            .unwrap_err(),
        TopologyError::PromotionCreatesAuthority { .. }
    ));
    assert!(topology
        .apply(&TopologyAction::Promote {
            participant: ComponentId::new("lead"),
            capability: Capability::BranchWrite,
        })
        .is_ok());
}

#[test]
fn a_controller_refuses_a_removal_inside_the_minimum_dwell_window() {
    let mut topology = Topology::new(Pattern::Mesh).with(Member::new("newcomer", "skeptic", 0));
    let mut controller = Controller::new(DwellPolicy::new(10, 3, 10, 100));
    let evict = TopologyAction::Evict {
        participant: ComponentId::new("newcomer"),
        reason: "redundant".into(),
    };
    assert!(matches!(
        controller
            .propose(&topology, &evict, ReasonCode::RedundantWork)
            .unwrap_err(),
        TopologyError::MinimumDwellNotMet { .. }
    ));
    for _ in 0..12 {
        topology
            .apply(&TopologyAction::Rewire {
                from: ComponentId::new("newcomer"),
                to: ComponentId::new("newcomer"),
                connected: true,
            })
            .unwrap();
    }
    assert!(controller
        .propose(&topology, &evict, ReasonCode::RedundantWork)
        .is_ok());
}

#[test]
fn a_controller_enforces_a_cooldown_after_a_failed_spawn_and_a_change_budget() {
    let topology = Topology::new(Pattern::Swarm);
    let mut controller = Controller::new(DwellPolicy::new(0, 5, 2, 100));
    controller.note_failed_spawn(0);
    let spawn = TopologyAction::Spawn {
        role: "worker".into(),
        participant: ComponentId::new("w1"),
        authority: BTreeSet::new(),
    };
    assert!(matches!(
        controller
            .propose(&topology, &spawn, ReasonCode::ParallelisableWork)
            .unwrap_err(),
        TopologyError::SpawnCooldown { .. }
    ));

    let mut controller = Controller::new(DwellPolicy::new(0, 0, 2, 100));
    for _ in 0..2 {
        controller
            .propose(&topology, &spawn, ReasonCode::ParallelisableWork)
            .unwrap();
    }
    assert!(matches!(
        controller
            .propose(&topology, &spawn, ReasonCode::ParallelisableWork)
            .unwrap_err(),
        TopologyError::ChangeBudgetExhausted { .. }
    ));
    assert_eq!(Controller::unimplemented_mechanisms().len(), 3);
}

fn goal() -> Goal {
    Goal::new(
        "repair a duplicate-payment defect",
        "git-patch-with-evidence@1",
    )
    .unwrap()
    .allowing(effects(&[
        (EffectKind::ArtifactRead, "corpus/**"),
        (EffectKind::FilesystemWrite, "repo/branch/**"),
    ]))
    .within(1_000, 1_000)
    .labelled(Labelling::Labelled(FlowLabel::open_at(Sensitivity::Public)))
    .at_rung(EvidenceLayer::PrismEvaluated)
}

fn role_graph() -> RoleGraph {
    RoleGraph::new()
        .with(
            Role::new("producer", "patch.produce", EvidenceLayer::PrismEvaluated)
                .permitting(effects(&[(EffectKind::ArtifactRead, "corpus/**")])),
        )
        .with(
            Role::new("verifier", "test.run", EvidenceLayer::PrismEvaluated)
                .permitting(effects(&[(EffectKind::ArtifactRead, "corpus/**")])),
        )
        .verifying("verifier", "producer")
}

fn admissible_candidate(name: &str) -> Candidate {
    Candidate::new(name, role_graph())
        .binding(
            "producer",
            verifier("producer")
                .emitting_at(Labelling::Labelled(FlowLabel::open_at(Sensitivity::Public))),
        )
        .binding(
            "verifier",
            verifier("verifier")
                .emitting_at(Labelling::Labelled(FlowLabel::open_at(Sensitivity::Public))),
        )
        .terminating_at("delivered")
}

#[test]
fn a_goal_must_name_the_expected_artifact_and_not_only_a_natural_language_intent() {
    assert!(matches!(
        Goal::new("do the thing", "").unwrap_err(),
        SynthesisError::GoalWithoutArtifact
    ));
}

#[test]
fn a_candidate_with_no_terminal_state_is_rejected_with_its_commitments_named_as_orphans() {
    let candidate = Candidate::new("no-exit", role_graph())
        .binding(
            "producer",
            verifier("producer").committing(DeclaredCommitment::mandatory("c1")),
        )
        .binding("verifier", verifier("verifier"));
    let reasons = bioprism_fabric::synth::reject(&goal(), &candidate, None);
    assert!(reasons
        .iter()
        .any(|r| matches!(r, RejectionReason::MissingTerminalStates)));
    assert!(reasons
        .iter()
        .any(|r| matches!(r, RejectionReason::OrphanCommitments { .. })));
}

#[test]
fn a_hard_constraint_failure_yields_no_score_at_all_rather_than_a_small_penalty() {
    let candidate = Candidate::new("cheap-but-forbidden", role_graph())
        .binding("producer", verifier("producer"))
        .binding("verifier", verifier("verifier"));
    match evaluate(&goal(), &candidate, None) {
        Candidacy::Rejected { reasons } => assert!(!reasons.is_empty()),
        Candidacy::Admissible { .. } => {
            panic!("a candidate with no terminal state is not admissible")
        }
    }
}

#[test]
fn a_candidate_that_passes_every_hard_constraint_is_scored() {
    match evaluate(&goal(), &admissible_candidate("ok"), None) {
        Candidacy::Admissible { score } => assert_eq!(score.team_size, 2),
        Candidacy::Rejected { reasons } => panic!("unexpected rejection: {reasons:?}"),
    }
}

#[test]
fn a_role_graph_without_an_independent_verifier_fails_the_assurance_requirement() {
    let goal = goal().requiring(AssuranceRequirement::IndependentVerification);
    let mut candidate = admissible_candidate("no-verifier");
    candidate.graph.verifies.clear();
    let reasons = bioprism_fabric::synth::reject(&goal, &candidate, None);
    assert!(reasons
        .iter()
        .any(|r| matches!(r, RejectionReason::InsufficientVerifiedAssurance { .. })));
}

#[test]
fn a_deadlock_finding_from_the_choreography_crate_is_recorded_as_a_rejection_reason() {
    let reasons = bioprism_fabric::synth::reject(
        &goal(),
        &admissible_candidate("ok"),
        Some("role Reviewer waits forever"),
    );
    assert!(reasons
        .iter()
        .any(|r| matches!(r, RejectionReason::KnownProtocolDeadlock { .. })));
}

#[test]
fn five_clones_contribute_one_source_of_information_and_four_correlated_failures() {
    let clones: Vec<CapabilityCard> = (0..5)
        .map(|i| CapabilityCard::new(format!("agent:{i}")).with_lineage("runtime:alpha"))
        .collect();
    assert_eq!(correlated_failure_penalty(&clones), 4);
    assert_eq!(
        correlated_failure_penalty(&[CapabilityCard::new("solo")]),
        0
    );
}

#[test]
fn synthesis_reports_the_pareto_frontier_and_why_every_eliminated_candidate_was_eliminated() {
    let artifact = synthesize(
        &goal(),
        &[
            admissible_candidate("a"),
            Candidate::new("broken", role_graph()),
        ],
    );
    assert!(artifact.frontier.contains("a"));
    assert!(artifact.eliminated.contains_key("broken"));
    assert!(!artifact.eliminated["broken"].is_empty());
}

#[test]
fn a_dominated_candidate_is_off_the_frontier_and_incomparable_ones_stay_on_it() {
    let cheap_slow = Score {
        cost_minor: 10,
        latency_units: 100,
        privilege_surface: 0,
        correlated_failure: 0,
        team_size: 1,
    };
    let dear_fast = Score {
        cost_minor: 100,
        latency_units: 10,
        privilege_surface: 0,
        correlated_failure: 0,
        team_size: 1,
    };
    let dominated = Score {
        cost_minor: 200,
        latency_units: 200,
        privilege_surface: 1,
        correlated_failure: 1,
        team_size: 3,
    };
    let frontier = pareto_frontier(&[
        ("cheap-slow".into(), cheap_slow),
        ("dear-fast".into(), dear_fast),
        ("worse".into(), dominated),
    ]);
    assert!(frontier.contains("cheap-slow"));
    assert!(frontier.contains("dear-fast"));
    assert!(!frontier.contains("worse"));
}

#[test]
fn an_adapter_is_never_simply_compatible_and_a_lossless_one_names_its_surface() {
    let lossy = Adapter::new(
        "a2a-v1",
        "weave-ir-v0",
        AdapterRung::A2AWithWeaveExtension,
        LossPolicy::RejectOrWrap,
    )
    .supporting(SemanticFeature::Messages)
    .approximating(SemanticFeature::Commitments)
    .not_supporting(SemanticFeature::ContinuationTransfer);
    assert!(matches!(lossy.compatibility(), Compatibility::Lossy { .. }));

    let narrow = Adapter::new(
        "local",
        "weave-ir-v0",
        AdapterRung::NativeWeaveSdk,
        LossPolicy::Reject,
    )
    .supporting(SemanticFeature::Messages);
    match narrow.compatibility() {
        Compatibility::LosslessForDeclaredSurface { surface } => {
            assert_eq!(surface.len(), 1);
            assert!(!narrow.undeclared().is_empty());
        }
        other => panic!("expected a declared surface, got {other:?}"),
    }
}

#[test]
fn a_bridge_fails_closed_on_a_feature_the_adapter_never_declared() {
    let adapter = Adapter::new(
        "cli",
        "weave-ir-v0",
        AdapterRung::CliPtyDriver,
        LossPolicy::Reject,
    )
    .supporting(SemanticFeature::Messages);
    let request = BridgeRequest::default()
        .requiring(SemanticFeature::Messages)
        .requiring(SemanticFeature::AuthorityDelegation);
    match fail_closed(&adapter, &request).unwrap_err() {
        StackError::BridgeFailsClosed { features, .. } => {
            assert!(features.contains(&SemanticFeature::AuthorityDelegation))
        }
        other => panic!("expected a fail-closed refusal, got {other:?}"),
    }
    let approved = request.approving_downgrade(SemanticFeature::AuthorityDelegation);
    assert!(fail_closed(&adapter, &approved).is_ok());
}

#[test]
fn a_session_agreement_is_pinned_by_intersection_and_records_what_each_side_conceded() {
    let left = SessionCapabilities::new("alice")
        .speaking("0.1")
        .speaking("0.2")
        .acting("inform")
        .acting("commit")
        .resolving("audit")
        .with_payload_limit(1_000);
    let right = SessionCapabilities::new("bob")
        .speaking("0.2")
        .acting("inform")
        .resolving("audit")
        .resolving("summary")
        .with_payload_limit(500);
    let agreement = negotiate_session(&left, &right).unwrap();
    assert_eq!(agreement.weave_ir_version, "0.2");
    assert_eq!(agreement.acts.len(), 1);
    assert_eq!(agreement.max_payload_bytes, 500);
    assert_eq!(agreement.capsule_resolutions.len(), 1);
    let conceded = agreement.conceded();
    assert!(conceded
        .iter()
        .any(|(who, given_up)| who == "alice" && given_up.contains("commit")));
}

#[test]
fn two_participants_with_no_shared_ir_version_cannot_open_a_session() {
    let left = SessionCapabilities::new("alice")
        .speaking("0.1")
        .acting("inform");
    let right = SessionCapabilities::new("bob")
        .speaking("0.9")
        .acting("inform");
    assert!(matches!(
        negotiate_session(&left, &right).unwrap_err(),
        StackError::NoCommonIrVersion { .. }
    ));
}

#[test]
fn the_stack_is_twelve_layers_and_names_which_crate_discharges_each_implemented_one() {
    assert_eq!(STACK.len(), 12);
    for (index, layer) in STACK.iter().enumerate() {
        assert_eq!(layer.level as usize, index);
    }
    assert!(layer_owner(9).unwrap().contains("bioprism-fabric"));
    assert!(layer_owner(0).is_none());
}

#[test]
fn specializing_a_molecule_preserves_its_exported_interface_and_narrows_its_effect_account() {
    let molecule = molecule();
    let context = SubstitutionContext {
        allowed_envelope: permissive_envelope(),
        minimum_assurance: AssuranceProfile::at(EvidenceLayer::SelfDeclared),
    };
    let fixed = verifier("v").with_effects(EffectSet::new());
    let specialized = bioprism_fabric::molecule::specialize(&molecule, "reviewer", fixed, &context)
        .expect("a strictly smaller effect set refines");
    assert_eq!(specialized.exported, molecule.exported);
    assert_eq!(
        specialized.composition.contract.effects,
        extractor("x").effects
    );
    assert_eq!(specialized.version.minor, molecule.version.minor + 1);
}

#[test]
fn specializing_with_a_participant_that_does_not_refine_the_role_is_refused() {
    let context = SubstitutionContext {
        allowed_envelope: permissive_envelope(),
        minimum_assurance: AssuranceProfile::at(EvidenceLayer::SelfDeclared),
    };
    let error = bioprism_fabric::molecule::specialize(
        &molecule(),
        "reviewer",
        common::cheaper_verifier("v"),
        &context,
    )
    .unwrap_err();
    assert!(matches!(error, MoleculeError::SpecializationRefused { .. }));
}

#[test]
fn a_nested_molecule_enters_an_outer_composition_as_one_contract() {
    let molecule = molecule();
    let nested = bioprism_fabric::molecule::nest(&molecule);
    assert_eq!(nested.output, report());
    assert_eq!(nested.effects, molecule.composition.contract.effects);
    assert!(nested
        .id
        .as_str()
        .starts_with("scientific-reproducer@1.0.0"));

    let outer = seq(&nested, &verifier("downstream"));
    assert!(
        outer.is_err(),
        "Report does not satisfy the downstream verifier's Analysis input"
    );
}
