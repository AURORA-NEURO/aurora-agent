//! 23.47: request conformance, decision capsules over kernel projections, attention, approvals.

use bioprism_fabric::effect::Irreversibility;
use bioprism_interweave::human::{
    attention_tracking, detect, required_layer, Approval, ApprovalRefusal, Basis, CapsuleDefect,
    Consequences, DecisionCapsule, FailureMode, HumanRequest, InteractionStep, OrgIdentity, OrgRole,
    OrganizationalCommitment, ParticipantKind, RequestDefect, Requested, ReviewOption,
    ReviewRefusal, Reviewer, SuccessionRefusal, Urgency, EVALUATION_MEASURES,
    MIXED_INITIATIVE_CONTROLS,
};
use bioprism_section::{
    DecisionSection, EvidenceCapsule, Layer, OracleVerdict, RenderContext,
};
use bioprism_weave::{Capability, ContextCapsule, Recipient};
use serde_json::json;

fn section(tags: Vec<&str>) -> DecisionSection {
    DecisionSection {
        world_id: "w1".into(),
        query_id: "q1".into(),
        decision_time: "2026-01-01T00:00:00Z".into(),
        goal: "decide whether the patch may be merged".into(),
        selected_evidence: vec![EvidenceCapsule {
            id: "test:race-17".into(),
            provides: "race-condition".into(),
            value: json!(true),
            scope: json!({}),
            tags: tags.into_iter().map(str::to_string).collect(),
            provenance: vec!["trace:commit-order".into()],
        }],
        selected_factors: Vec::new(),
        oracle: OracleVerdict::new("deterministic-test", Vec::new()),
        unresolved_obligations: Vec::new(),
        refinement_frontier: Vec::new(),
    }
}

fn context(sufficient: bool) -> RenderContext {
    RenderContext {
        omitted_facts: 0,
        total_facts: 1,
        supports_sufficiency_claim: sufficient,
        protected_closure_satisfied: true,
        certificate_sha256: None,
    }
}

fn capsule(tags: Vec<&str>, clearance: Vec<&str>, sufficient: bool, layer: Layer) -> ContextCapsule {
    let recipient = clearance
        .into_iter()
        .fold(Recipient::new("reviewer", "approver").up_to(layer), |r, c| {
            r.cleared_for(c)
        });
    ContextCapsule::project(&section(tags), &context(sufficient), &recipient, layer)
}

fn request(subject: &str) -> HumanRequest {
    HumanRequest {
        requested: Requested::Decision {
            subject: subject.into(),
            question: "may this patch be merged".into(),
        },
        why_human: "merge authority is not delegated to agents".into(),
        decision_rights: Capability::BranchWrite,
        deadline_tick: 100,
        urgency: Urgency::Elevated,
        estimated_minutes: 8,
        uncertainty: vec!["database-isolation-in-production".into()],
        alternatives: vec!["request targeted evidence".into()],
        consequences: Consequences {
            of_approval: "the patch merges to main".into(),
            of_rejection: "the branch stays open".into(),
            of_no_response: "the change window closes and the branch stays open".into(),
        },
        reversibility: Irreversibility::E2,
        dissent_recording: "dissent is appended to the adjudication record".into(),
    }
}

#[test]
fn a_fully_specified_request_is_conformant() {
    assert!(request("patch-4417").conformant());
    assert!(request("patch-4417").defects().is_empty());
}

#[test]
fn please_review_everything_fails_conformance_on_scope() {
    let vague = request("everything");
    assert!(vague.defects().contains(&RequestDefect::UnboundedScope {
        subject: "everything".into()
    }));
    assert!(!vague.conformant());
}

#[test]
fn a_request_with_no_attention_estimate_fails_conformance() {
    let mut bad = request("patch-4417");
    bad.estimated_minutes = 0;
    assert!(bad.defects().contains(&RequestDefect::NoAttentionEstimate));
}

#[test]
fn a_request_that_does_not_say_what_silence_means_fails_conformance() {
    let mut bad = request("patch-4417");
    bad.consequences.of_no_response = "   ".into();
    assert!(bad.defects().contains(&RequestDefect::NoSilenceConsequence));
}

#[test]
fn a_request_with_no_alternatives_and_no_dissent_policy_reports_both_defects() {
    let mut bad = request("patch-4417");
    bad.alternatives.clear();
    bad.dissent_recording = String::new();
    let defects = bad.defects();
    assert!(defects.contains(&RequestDefect::NoAlternatives));
    assert!(defects.contains(&RequestDefect::NoDissentRecording));
    assert_eq!(defects.len(), 2);
}

#[test]
fn an_empty_subject_is_reported_as_missing_rather_than_as_unbounded() {
    let mut bad = request("   ");
    bad.why_human = String::new();
    let defects = bad.defects();
    assert!(defects.contains(&RequestDefect::NoSubject));
    assert!(defects.contains(&RequestDefect::NoJustification));
}

#[test]
fn a_capsule_offering_one_option_is_a_notification_and_is_refused() {
    let defect = DecisionCapsule::draft("merge?", "race", capsule(vec![], vec![], true, Layer::L2))
        .at_class(Irreversibility::E2)
        .build()
        .unwrap_err();
    assert_eq!(defect, CapsuleDefect::TooFewOptions);
}

#[test]
fn a_capsule_recommending_an_action_it_does_not_offer_is_refused() {
    let defect = DecisionCapsule::draft("merge?", "race", capsule(vec![], vec![], true, Layer::L2))
        .at_class(Irreversibility::E2)
        .offering(ReviewOption::Reject)
        .offering(ReviewOption::Approve)
        .recommending(ReviewOption::DelegateToOwner)
        .build()
        .unwrap_err();
    assert_eq!(
        defect,
        CapsuleDefect::RecommendationNotOffered(ReviewOption::DelegateToOwner)
    );
}

#[test]
fn a_capsule_offering_all_four_of_the_blueprints_options_builds() {
    let built = ReviewOption::ALL
        .into_iter()
        .fold(
            DecisionCapsule::draft("merge?", "race", capsule(vec![], vec![], true, Layer::L2))
                .at_class(Irreversibility::E2),
            |draft, option| draft.offering(option),
        )
        .recommending(ReviewOption::RequestTargetedEvidence)
        .estimated_minutes(8)
        .requesting(Capability::BranchWrite)
        .build()
        .expect("four options at L2");
    assert_eq!(built.options.len(), 4);
    assert_eq!(built.estimated_review_minutes, 8);
    assert_eq!(built.authority_requested, Capability::BranchWrite);
}

#[test]
fn an_irreversible_decision_shown_at_l2_is_refused_for_being_too_shallow() {
    let defect = DecisionCapsule::draft("publish?", "irreversible", capsule(vec![], vec![], true, Layer::L2))
        .at_class(Irreversibility::E4)
        .offering(ReviewOption::Approve)
        .offering(ReviewOption::Reject)
        .recommending(ReviewOption::Reject)
        .build()
        .unwrap_err();
    assert_eq!(
        defect,
        CapsuleDefect::LayerTooShallow {
            class: Irreversibility::E4,
            required: Layer::L4,
            actual: Layer::L2,
        }
    );
}

#[test]
fn the_same_irreversible_decision_shown_at_l4_is_accepted() {
    let built = DecisionCapsule::draft("publish?", "irreversible", capsule(vec![], vec![], true, Layer::L4))
        .at_class(Irreversibility::E4)
        .offering(ReviewOption::Approve)
        .offering(ReviewOption::Reject)
        .recommending(ReviewOption::Reject)
        .build();
    assert!(built.is_ok());
}

#[test]
fn the_required_layer_rises_with_the_irreversibility_class() {
    assert_eq!(required_layer(Irreversibility::E0), Layer::L2);
    assert_eq!(required_layer(Irreversibility::E2), Layer::L2);
    assert_eq!(required_layer(Irreversibility::E3), Layer::L3);
    assert_eq!(required_layer(Irreversibility::E4), Layer::L4);
}

#[test]
fn a_capsule_over_a_projection_that_withheld_evidence_reports_partial_evidence() {
    let withheld = capsule(vec!["phi"], vec![], true, Layer::L2);
    let built = DecisionCapsule::draft("merge?", "race", withheld)
        .at_class(Irreversibility::E2)
        .offering(ReviewOption::Approve)
        .offering(ReviewOption::Reject)
        .recommending(ReviewOption::Reject)
        .build()
        .expect("two options at L2");
    match built.basis() {
        Basis::PartialEvidence { withheld } => assert_eq!(withheld, vec!["test:race-17".to_string()]),
        other => panic!("expected partial evidence, got {other:?}"),
    }
    assert!(!built.basis().informed());
}

#[test]
fn a_capsule_over_a_complete_projection_with_an_insufficient_upstream_says_so_distinctly() {
    let built = DecisionCapsule::draft("merge?", "race", capsule(vec![], vec![], false, Layer::L2))
        .at_class(Irreversibility::E2)
        .offering(ReviewOption::Approve)
        .offering(ReviewOption::Reject)
        .recommending(ReviewOption::Reject)
        .build()
        .expect("two options at L2");
    assert_eq!(built.basis(), Basis::UpstreamInsufficient);
    assert!(!built.basis().informed());
}

#[test]
fn a_capsule_over_a_complete_and_sufficient_projection_is_informed() {
    let built = DecisionCapsule::draft("merge?", "race", capsule(vec![], vec![], true, Layer::L2))
        .at_class(Irreversibility::E2)
        .offering(ReviewOption::Approve)
        .offering(ReviewOption::Reject)
        .recommending(ReviewOption::Reject)
        .build()
        .expect("two options at L2")
        .supported_by("test:race-17")
        .opposed_by("agent:patcher-claim-4")
        .unresolved("database-isolation-in-production");
    assert_eq!(built.basis(), Basis::CompleteEvidence);
    assert!(built.basis().informed());
    assert_eq!(built.supporting.len(), 1);
    assert_eq!(built.opposing.len(), 1);
    assert!(built.context().is_complete());
}

#[test]
fn a_reviewer_without_the_required_expertise_is_refused_before_any_attention_is_spent() {
    let mut reviewer = Reviewer::new("dana", ParticipantKind::IndividualHuman, 60);
    let refusal = reviewer
        .accept(&request("patch-4417"), Some("database-internals"))
        .unwrap_err();
    assert!(matches!(refusal, ReviewRefusal::MissingExpertise { .. }));
    assert_eq!(reviewer.remaining_minutes(), 60);
    assert_eq!(reviewer.requests_taken(), 0);
}

#[test]
fn a_reviewer_at_their_request_limit_is_overloaded_rather_than_exhausted() {
    let mut reviewer = Reviewer::new("dana", ParticipantKind::IndividualHuman, 600).limited_to(1);
    reviewer
        .accept(&request("patch-1"), None)
        .expect("first request fits");
    let refusal = reviewer.accept(&request("patch-2"), None).unwrap_err();
    assert_eq!(
        refusal,
        ReviewRefusal::Overloaded {
            reviewer: "dana".into(),
            taken: 1,
            limit: 1,
        }
    );
}

#[test]
fn attention_is_spent_and_cannot_be_spent_twice() {
    let mut reviewer = Reviewer::new("dana", ParticipantKind::IndividualHuman, 10);
    reviewer
        .accept(&request("patch-1"), None)
        .expect("eight minutes fits in ten");
    assert_eq!(reviewer.remaining_minutes(), 2);
    let refusal = reviewer.accept(&request("patch-2"), None).unwrap_err();
    assert_eq!(
        refusal,
        ReviewRefusal::AttentionExhausted {
            reviewer: "dana".into(),
            requested_minutes: 8,
            available_minutes: 2,
        }
    );
}

#[test]
fn a_reviewer_with_the_expertise_and_the_budget_accepts() {
    let mut reviewer =
        Reviewer::new("dana", ParticipantKind::ExpertPanel, 60).expert_in("database-internals");
    assert!(reviewer
        .accept(&request("patch-4417"), Some("database-internals"))
        .is_ok());
    assert_eq!(reviewer.requests_taken(), 1);
}

fn approval(kind: ParticipantKind, granted: Capability) -> Approval {
    Approval {
        approver: "dana".into(),
        approver_kind: kind,
        granted,
        basis: Basis::CompleteEvidence,
        conflicts_disclosed: true,
        consent_informed: true,
        duration_ticks: 50,
        further_approval_required: Vec::new(),
    }
}

#[test]
fn an_approval_does_not_authorize_a_capability_it_did_not_grant() {
    let refusal = approval(ParticipantKind::IndividualHuman, Capability::BranchWrite)
        .authorizes(Capability::PublishResult, 0, 10)
        .unwrap_err();
    assert_eq!(
        refusal,
        ApprovalRefusal::OutsideScope {
            approver: "dana".into(),
            granted: Capability::BranchWrite,
            attempted: Capability::PublishResult,
        }
    );
}

#[test]
fn an_approval_expires_at_the_end_of_its_stated_duration() {
    let refusal = approval(ParticipantKind::IndividualHuman, Capability::BranchWrite)
        .authorizes(Capability::BranchWrite, 0, 51)
        .unwrap_err();
    assert_eq!(refusal, ApprovalRefusal::Expired { expires_at: 50, now: 51 });
}

#[test]
fn an_approval_within_scope_and_duration_authorizes_the_action() {
    assert!(approval(ParticipantKind::IndividualHuman, Capability::BranchWrite)
        .authorizes(Capability::BranchWrite, 0, 50)
        .is_ok());
}

#[test]
fn an_approval_attributed_to_a_mixed_molecule_does_not_carry_a_human_judgement() {
    let refusal = approval(
        ParticipantKind::MixedHumanAgentMolecule,
        Capability::BranchWrite,
    )
    .authorizes(Capability::BranchWrite, 0, 10)
    .unwrap_err();
    assert_eq!(
        refusal,
        ApprovalRefusal::NotAHumanJudgement {
            approver: "dana".into(),
            kind: ParticipantKind::MixedHumanAgentMolecule,
        }
    );
}

#[test]
fn an_approval_with_an_outstanding_institutional_review_does_not_authorize_yet() {
    let mut pending = approval(ParticipantKind::IndividualHuman, Capability::BranchWrite);
    pending.further_approval_required = vec!["data steward".into()];
    let refusal = pending
        .authorizes(Capability::BranchWrite, 0, 10)
        .unwrap_err();
    assert!(matches!(
        refusal,
        ApprovalRefusal::FurtherApprovalRequired { .. }
    ));
}

#[test]
fn approval_requested_after_an_irreversible_action_is_detected_from_the_ordering_alone() {
    let steps = vec![
        InteractionStep::ActionPerformed {
            class: Irreversibility::E4,
        },
        InteractionStep::ApprovalRequested {
            class: Irreversibility::E4,
        },
    ];
    assert!(detect(&steps, false)
        .contains(&FailureMode::ApprovalRequestedAfterIrreversibleAction));
}

#[test]
fn the_same_two_events_in_the_right_order_are_not_a_failure() {
    let steps = vec![
        InteractionStep::ApprovalRequested {
            class: Irreversibility::E4,
        },
        InteractionStep::ActionPerformed {
            class: Irreversibility::E4,
        },
    ];
    assert!(detect(&steps, false).is_empty());
}

#[test]
fn an_approval_on_partial_evidence_is_detected_as_a_rubber_stamp() {
    let steps = vec![InteractionStep::ApprovalGiven {
        basis: Basis::PartialEvidence {
            withheld: vec!["test:race-17".into()],
        },
        minutes_spent: 9,
    }];
    assert!(detect(&steps, false).contains(&FailureMode::RubberStampFromPoorContext));
}

#[test]
fn an_instant_approval_on_complete_evidence_is_also_detected_as_a_rubber_stamp() {
    let steps = vec![InteractionStep::ApprovalGiven {
        basis: Basis::CompleteEvidence,
        minutes_spent: 0,
    }];
    assert!(detect(&steps, false).contains(&FailureMode::RubberStampFromPoorContext));
}

#[test]
fn a_considered_approval_on_complete_evidence_raises_nothing() {
    let steps = vec![InteractionStep::ApprovalGiven {
        basis: Basis::CompleteEvidence,
        minutes_spent: 9,
    }];
    assert!(detect(&steps, false).is_empty());
}

#[test]
fn five_of_the_eight_failure_modes_are_decidable_from_the_records_this_crate_keeps() {
    let detectable: Vec<FailureMode> = FailureMode::ALL
        .into_iter()
        .filter(|mode| mode.detectable())
        .collect();
    assert_eq!(detectable.len(), 5);
    assert!(!FailureMode::AmbiguousResponsibility.detectable());
    assert!(!FailureMode::HumanInterventionAsUnmeasuredEscapeHatch.detectable());
}

#[test]
fn an_organizational_commitment_survives_a_session_by_moving_to_its_successor() {
    let commitment = OrganizationalCommitment {
        identity: OrgIdentity::attested("lab-a", "att-9", "registry"),
        role: OrgRole::Debtor,
        obligation: "preserve the dataset for ten years".into(),
        responsible_party: "curator-1".into(),
        succession: vec!["curator-1".into(), "curator-2".into()],
    };
    let carried = commitment.end_session().expect("a successor exists");
    assert_eq!(carried.responsible_party, "curator-2");
    assert_eq!(carried.obligation, "preserve the dataset for ten years");
}

#[test]
fn an_organizational_commitment_with_no_successor_refuses_rather_than_being_orphaned() {
    let commitment = OrganizationalCommitment {
        identity: OrgIdentity::attested("lab-a", "att-9", "registry"),
        role: OrgRole::Approver,
        obligation: "adjudicate disputed artifacts".into(),
        responsible_party: "chair".into(),
        succession: vec!["chair".into()],
    };
    assert_eq!(
        commitment.end_session(),
        Err(SuccessionRefusal::NoSuccessor {
            organization: "lab-a".into(),
            obligation: "adjudicate disputed artifacts".into(),
        })
    );
}

#[test]
fn an_organizational_identity_requires_an_attestation_and_its_issuer() {
    let identity = OrgIdentity::attested("lab-a", "att-9", "registry");
    assert_eq!(identity.attestation_id, "att-9");
    assert_eq!(identity.issued_by, "registry");
}

#[test]
fn only_three_participant_kinds_guarantee_a_human_judgement() {
    let guaranteeing: Vec<ParticipantKind> = ParticipantKind::ALL
        .into_iter()
        .filter(|kind| kind.guarantees_human_judgement())
        .collect();
    assert_eq!(
        guaranteeing,
        vec![
            ParticipantKind::IndividualHuman,
            ParticipantKind::OnCallHumanPool,
            ParticipantKind::ExpertPanel,
        ]
    );
}

#[test]
fn four_of_the_seven_attention_quantities_are_tracked_here_and_the_rest_are_named() {
    let tracking = attention_tracking();
    assert_eq!(tracking.len(), 7);
    assert_eq!(tracking.values().filter(|tracked| **tracked).count(), 4);
    assert_eq!(MIXED_INITIATIVE_CONTROLS.len(), 9);
    assert_eq!(EVALUATION_MEASURES.len(), 8);
}
