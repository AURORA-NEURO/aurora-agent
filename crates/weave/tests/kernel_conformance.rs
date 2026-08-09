//! Kernel conformance.
//!
//! Blueprint 23.49 lists what a kernel implementation must pass. Each of its items has a test
//! here, named after it, so the claim "this is a conforming microkernel" is checkable rather than
//! asserted.

use bioprism_section::{
    DecisionSection, EvidenceCapsule, Layer, OracleVerdict, RenderContext,
};
use bioprism_weave::{
    Act, ActKind, Budget, Capability, ChainStatus, ContextCapsule, ContinuationHandle, Fidelity,
    Kernel, KernelError, Participant, Recipient, Resource, ResumeError,
};
use serde_json::json;

fn kernel_with_two_participants() -> Kernel {
    let mut kernel = Kernel::new();

    let analyst_grant = kernel.authority_mut().issue(
        "analyst",
        [
            Capability::ReadWorld,
            Capability::ReadEvidence,
            Capability::CompileContext,
            Capability::ExecuteTests,
        ],
    );
    let reviewer_grant = kernel
        .authority_mut()
        .issue("reviewer", [Capability::ReadWorld, Capability::ReadEvidence]);

    kernel.admit(
        Participant {
            name: "analyst".into(),
            role: "analyst".into(),
            grant: analyst_grant,
            abi_grade: 3,
        },
        Budget::new().with(Resource::ToolCalls, 100),
    );
    kernel.admit(
        Participant {
            name: "reviewer".into(),
            role: "reviewer".into(),
            grant: reviewer_grant,
            abi_grade: 2,
        },
        Budget::new().with(Resource::ToolCalls, 100),
    );
    kernel
}

#[test]
fn rejects_invalid_act_order() {
    let mut kernel = kernel_with_two_participants();

    // Accepting something nobody proposed.
    let orphan = Act::new(ActKind::Accept, "reviewer", "analyst", json!({}));
    assert!(matches!(
        kernel.submit(orphan),
        Err(KernelError::MissingAntecedent { .. })
    ));

    let claim = kernel
        .submit(Act::new(
            ActKind::Claim,
            "analyst",
            "reviewer",
            json!({ "split": "valid" }),
        ))
        .expect("claim is legal");

    // Accepting a claim is not a legal transition; only proposals may be accepted.
    let wrong = Act::new(ActKind::Accept, "reviewer", "analyst", json!({}))
        .replying_to(claim.id.clone());
    match kernel.submit(wrong) {
        Err(KernelError::IllegalTransition { kind, found, .. }) => {
            assert_eq!(kind, "accept");
            assert_eq!(found, "claim");
        }
        other => panic!("expected an illegal transition, got {other:?}"),
    }

    // Challenging a claim is legal.
    let challenge = Act::new(ActKind::Challenge, "reviewer", "analyst", json!({ "why": "alias" }))
        .replying_to(claim.id);
    assert!(kernel.submit(challenge).is_ok());
    assert_eq!(kernel.contested_claims().len(), 1);
}

#[test]
fn a_rejected_act_does_not_enter_the_ledger() {
    let mut kernel = kernel_with_two_participants();
    let before = kernel.ledger().len();

    let _ = kernel.submit(Act::new(ActKind::Accept, "reviewer", "analyst", json!({})));
    let _ = kernel.submit(Act::new(ActKind::Claim, "stranger", "analyst", json!({})));

    assert_eq!(kernel.ledger().len(), before, "rejected acts must not be recorded");
    assert_eq!(
        kernel.rejected_acts(),
        2,
        "both the orphan accept and the unknown participant are counted"
    );
}

#[test]
fn a_participant_without_the_capability_cannot_act() {
    let mut kernel = kernel_with_two_participants();
    // The reviewer holds ReadEvidence but not ExecuteTests, which discharge requires.
    let proposal = kernel
        .submit(Act::new(ActKind::Propose, "reviewer", "analyst", json!({ "task": "re-run" })))
        .unwrap();
    let accept = kernel
        .submit(Act::new(ActKind::Accept, "analyst", "reviewer", json!({})).replying_to(proposal.id))
        .unwrap();

    let discharge = Act::new(ActKind::Discharge, "reviewer", "analyst", json!({}))
        .replying_to(accept.id.clone());
    assert!(matches!(kernel.submit(discharge), Err(KernelError::Authority(_))));

    let by_analyst = Act::new(ActKind::Discharge, "analyst", "reviewer", json!({}))
        .replying_to(accept.id);
    assert!(kernel.submit(by_analyst).is_ok());
    assert!(kernel.open_commitments().is_empty());
}

#[test]
fn a_commitment_cannot_be_discharged_twice() {
    let mut kernel = kernel_with_two_participants();
    let proposal = kernel
        .submit(Act::new(ActKind::Propose, "reviewer", "analyst", json!({})))
        .unwrap();
    let accept = kernel
        .submit(Act::new(ActKind::Accept, "analyst", "reviewer", json!({})).replying_to(proposal.id))
        .unwrap();

    assert!(kernel
        .submit(Act::new(ActKind::Discharge, "analyst", "reviewer", json!({})).replying_to(accept.id.clone()))
        .is_ok());
    assert!(matches!(
        kernel.submit(
            Act::new(ActKind::Discharge, "analyst", "reviewer", json!({})).replying_to(accept.id)
        ),
        Err(KernelError::AlreadyDischarged { .. })
    ));
}

#[test]
fn authority_attenuation_and_transitive_revocation() {
    let mut kernel = Kernel::new();
    let table = kernel.authority_mut();

    let root = table.issue(
        "lead",
        [
            Capability::ReadWorld,
            Capability::ReadEvidence,
            Capability::BranchWrite,
        ],
    );

    // Cannot delegate what you do not hold.
    assert!(matches!(
        table.delegate(&root, "helper", [Capability::PublishResult]),
        Err(bioprism_weave::AuthorityError::Amplification { .. })
    ));

    let child = table
        .delegate(&root, "helper", [Capability::ReadWorld, Capability::ReadEvidence])
        .expect("narrowing is allowed");
    let grandchild = table
        .delegate(&child, "sub-helper", [Capability::ReadWorld])
        .expect("narrowing again is allowed");

    assert!(table.check(&grandchild, Capability::ReadWorld).is_ok());
    assert!(table.check(&grandchild, Capability::BranchWrite).is_err());

    let revoked = table.revoke(&child).expect("revocation succeeds");
    assert_eq!(revoked.len(), 2, "revoking a grant revokes its descendants");
    assert!(table.check(&grandchild, Capability::ReadWorld).is_err());
    assert!(
        table.check(&root, Capability::ReadWorld).is_ok(),
        "revoking a child must not affect the parent"
    );
}

#[test]
fn budgets_are_affine_and_never_duplicated() {
    let mut parent = Budget::new().with(Resource::Tokens, 1000);

    let child = parent.split(Resource::Tokens, 400).expect("split succeeds");
    assert_eq!(parent.remaining(Resource::Tokens), 600);
    assert_eq!(child.remaining(Resource::Tokens), 400);
    assert_eq!(
        parent.remaining(Resource::Tokens) + child.remaining(Resource::Tokens),
        1000,
        "splitting must conserve the total"
    );

    assert!(
        parent.split(Resource::Tokens, 700).is_err(),
        "cannot hand out more than remains"
    );

    parent.absorb(child);
    assert_eq!(
        parent.remaining(Resource::Tokens),
        1000,
        "an unused child allowance returns to the parent"
    );
}

#[test]
fn budget_exhaustion_stops_a_participant() {
    let mut kernel = Kernel::new();
    let grant = kernel
        .authority_mut()
        .issue("tight", [Capability::ReadEvidence]);
    kernel.admit(
        Participant {
            name: "tight".into(),
            role: "analyst".into(),
            grant,
            abi_grade: 1,
        },
        Budget::new().with(Resource::ToolCalls, 5),
    );

    // Each claim costs 2.
    assert!(kernel.submit(Act::new(ActKind::Claim, "tight", "tight", json!({}))).is_ok());
    assert!(kernel.submit(Act::new(ActKind::Claim, "tight", "tight", json!({}))).is_ok());
    assert_eq!(kernel.remaining("tight", Resource::ToolCalls), 1);
    assert!(matches!(
        kernel.submit(Act::new(ActKind::Claim, "tight", "tight", json!({}))),
        Err(KernelError::Budget(_))
    ));
}

#[test]
fn the_ledger_is_hash_chained_and_tamper_evident() {
    let mut kernel = kernel_with_two_participants();
    kernel
        .submit(Act::new(ActKind::Claim, "analyst", "reviewer", json!({ "a": 1 })))
        .unwrap();
    kernel
        .submit(Act::new(ActKind::Claim, "analyst", "reviewer", json!({ "a": 2 })))
        .unwrap();

    assert!(kernel.verify_chain().is_intact());

    let mut events = kernel.ledger().events().to_vec();
    events[0].act.payload = json!({ "a": 99 });
    let mut forged = bioprism_weave::Ledger::new();
    for event in &events {
        forged.append(event.act.clone());
    }
    // Re-appending produces a *different* chain, which is the point: the original digests cannot
    // be reproduced from altered contents.
    assert_ne!(forged.head(), kernel.ledger().head());
}

#[test]
fn a_broken_chain_is_detected_at_the_right_entry() {
    let mut ledger = bioprism_weave::Ledger::new();
    ledger.append(Act::new(ActKind::Ask, "a", "b", json!({})));
    ledger.append(Act::new(ActKind::Ask, "a", "b", json!({})));
    assert!(ledger.verify_chain().is_intact());

    let mut events = ledger.events().to_vec();
    events[1].hash = "0".repeat(64);
    let broken = rebuild(&events);
    match broken.verify_chain() {
        ChainStatus::Broken { at_seq, .. } => assert_eq!(at_seq, 1),
        ChainStatus::Intact => panic!("an altered digest must break the chain"),
    }
}

/// Reconstructs a ledger from raw events without recomputing digests, to simulate a tampered file.
fn rebuild(events: &[bioprism_weave::LedgerEvent]) -> TamperedLedger {
    TamperedLedger {
        events: events.to_vec(),
    }
}

struct TamperedLedger {
    events: Vec<bioprism_weave::LedgerEvent>,
}

impl TamperedLedger {
    fn verify_chain(&self) -> ChainStatus {
        let mut expected = String::new();
        for event in &self.events {
            if event.previous != expected {
                return ChainStatus::Broken {
                    at_seq: event.seq,
                    reason: "previous mismatch".into(),
                };
            }
            let body = json!({
                "seq": event.seq, "id": event.id, "act": event.act, "previous": event.previous,
            });
            let recomputed = bioprism_ids::ContentHash::of_value(&body).unwrap();
            if recomputed.as_str() != event.hash {
                return ChainStatus::Broken {
                    at_seq: event.seq,
                    reason: "digest mismatch".into(),
                };
            }
            expected = event.hash.clone();
        }
        ChainStatus::Intact
    }
}

#[test]
fn independent_events_reach_the_same_derived_state_in_either_order() {
    let build = |reversed: bool| {
        let mut kernel = kernel_with_two_participants();
        let acts = vec![
            Act::new(ActKind::Claim, "analyst", "reviewer", json!({ "topic": "identity" })),
            Act::new(ActKind::Claim, "reviewer", "analyst", json!({ "topic": "site" })),
        ];
        let acts = if reversed {
            acts.into_iter().rev().collect::<Vec<_>>()
        } else {
            acts
        };
        for act in acts {
            kernel.submit(act).expect("independent claims are legal");
        }
        kernel
    };

    let forward = build(false);
    let backward = build(true);

    // Ledger order differs, derived state does not.
    assert_ne!(forward.ledger().head(), backward.ledger().head());
    assert_eq!(forward.open_commitments(), backward.open_commitments());
    assert_eq!(
        forward.contested_claims().len(),
        backward.contested_claims().len()
    );
}

fn sample_section() -> (DecisionSection, RenderContext) {
    let section = DecisionSection {
        world_id: "w".into(),
        query_id: "q".into(),
        decision_time: "2025-01-01T00:00:00Z".into(),
        goal: "assess split integrity".into(),
        selected_evidence: vec![
            EvidenceCapsule::from_raw_fact(&json!({
                "id": "fact.split", "provides": "split_assignment",
                "value": { "S001": "train" }, "scope": {}, "tags": ["split", "public"]
            })),
            EvidenceCapsule::from_raw_fact(&json!({
                "id": "fact.identity", "provides": "subject_aliases",
                "value": { "S001": ["ALT-77"] }, "scope": {}, "tags": ["identity", "restricted"]
            })),
        ],
        selected_factors: vec![json!({ "id": "factor.check" })],
        oracle: OracleVerdict::new("deterministic_split_integrity_v1", vec![]),
        unresolved_obligations: vec![],
        refinement_frontier: vec![],
    };
    let context = RenderContext {
        omitted_facts: 700,
        total_facts: 702,
        supports_sufficiency_claim: true,
        protected_closure_satisfied: true,
        certificate_sha256: Some("ab".repeat(32)),
    };
    (section, context)
}

#[test]
fn context_projection_is_label_aware_and_reports_what_it_withheld() {
    let (section, context) = sample_section();

    let cleared = Recipient::new("lead", "analyst")
        .cleared_for("split")
        .cleared_for("public")
        .cleared_for("identity")
        .cleared_for("restricted");
    let full = ContextCapsule::project(&section, &context, &cleared, Layer::L2);
    assert!(full.is_complete());
    assert_eq!(full.content["evidence"].as_array().unwrap().len(), 2);
    assert!(full.supports_sufficiency_claim());

    let limited = Recipient::new("outside", "statistician")
        .cleared_for("split")
        .cleared_for("public");
    let projected = ContextCapsule::project(&section, &context, &limited, Layer::L2);

    assert_eq!(projected.withheld.len(), 1);
    assert_eq!(projected.withheld[0].id, "fact.identity");
    assert_eq!(projected.content["evidence"].as_array().unwrap().len(), 1);
    assert_eq!(projected.content["withheld_by_projection"]["count"], json!(1));
    assert!(
        !projected.supports_sufficiency_claim(),
        "a filtered capsule cannot vouch for completeness it did not observe"
    );
}

#[test]
fn a_recipients_layer_ceiling_is_enforced() {
    let (section, context) = sample_section();
    let shallow = Recipient::new("junior", "assistant")
        .cleared_for("split")
        .cleared_for("public")
        .cleared_for("identity")
        .cleared_for("restricted")
        .up_to(Layer::L1);

    let capsule = ContextCapsule::project(&section, &context, &shallow, Layer::L4);
    assert_eq!(capsule.layer, Layer::L1, "the ceiling wins over the request");
    assert!(capsule.content.get("evidence").is_none());
    assert!(capsule.content.get("raw_section").is_none());
    // Omissions survive the ceiling.
    assert_eq!(capsule.content["omissions"]["omitted_facts"], json!(700));
}

#[test]
fn continuation_integrity_and_stale_state_detection() {
    let mut kernel = kernel_with_two_participants();
    kernel
        .submit(Act::new(ActKind::Claim, "analyst", "reviewer", json!({ "n": 1 })))
        .unwrap();

    let handle = ContinuationHandle::take(
        "cont-1",
        "analyst",
        kernel.ledger().head(),
        Fidelity::Exact,
        kernel.open_commitments(),
        json!({ "cursor": 7 }),
    );
    assert!(handle.is_intact());
    assert!(handle.resume(&kernel.ledger().head()).is_ok());

    // The world moves on.
    kernel
        .submit(Act::new(ActKind::Claim, "reviewer", "analyst", json!({ "n": 2 })))
        .unwrap();
    match handle.resume(&kernel.ledger().head()) {
        Err(ResumeError::Stale { .. }) => {}
        other => panic!("a superseded handle must not resume, got {other:?}"),
    }

    // Forking from the superseded point is always allowed.
    let branch = handle.fork("cont-2", "reviewer");
    assert!(branch.is_intact());
    assert_eq!(branch.taken_at_head, handle.taken_at_head);

    let mut altered = handle.clone();
    altered.state = json!({ "cursor": 999 });
    assert!(!altered.is_intact());
    assert!(matches!(
        altered.resume(&altered.taken_at_head),
        Err(ResumeError::Tampered(_))
    ));

    let frozen = ContinuationHandle::take(
        "cont-3",
        "analyst",
        kernel.ledger().head(),
        Fidelity::Frozen,
        vec![],
        json!({}),
    );
    assert!(matches!(
        frozen.resume(&kernel.ledger().head()),
        Err(ResumeError::NotResumable { .. })
    ));
}

#[test]
fn the_kernel_emits_a_checkpoint_for_prism_extraction() {
    let mut kernel = kernel_with_two_participants();
    let proposal = kernel
        .submit(Act::new(ActKind::Propose, "reviewer", "analyst", json!({ "task": "audit" })))
        .unwrap();
    kernel
        .submit(Act::new(ActKind::Accept, "analyst", "reviewer", json!({})).replying_to(proposal.id))
        .unwrap();
    let claim = kernel
        .submit(Act::new(ActKind::Claim, "analyst", "reviewer", json!({ "split": "valid" })))
        .unwrap();
    kernel
        .submit(Act::new(ActKind::Challenge, "reviewer", "analyst", json!({})).replying_to(claim.id))
        .unwrap();

    let checkpoint = kernel.checkpoint();
    assert_eq!(checkpoint["events"], json!(4));
    assert_eq!(checkpoint["open_commitments"].as_array().unwrap().len(), 1);
    assert_eq!(checkpoint["contested_claims"].as_array().unwrap().len(), 1);
    assert_eq!(checkpoint["head"], json!(kernel.ledger().head()));
    assert!(kernel.verify_chain().is_intact());
}

#[test]
fn the_kernel_refuses_to_judge_content() {
    // The kernel accepts contradictory claims without preferring either: deciding scientific truth
    // is explicitly outside the trusted computing base (23.49).
    let mut kernel = kernel_with_two_participants();
    kernel
        .submit(Act::new(ActKind::Claim, "analyst", "reviewer", json!({ "split": "valid" })))
        .unwrap();
    kernel
        .submit(Act::new(ActKind::Claim, "reviewer", "analyst", json!({ "split": "invalid" })))
        .unwrap();

    assert_eq!(kernel.ledger().len(), 2, "both claims stand in the record");
    assert!(kernel.contested_claims().is_empty(), "no challenge was made, so neither is contested");
}
