//! Cognitive type system invariants (blueprint 23.04).
//!
//! Each test names the program that must not compile.

use bioprism_weavelang::contract::{
    check, check_abi_grades, CognitiveType, ParticipantContract, TypeError,
};
use bioprism_weavelang::diagnostic::Diagnostic;
use bioprism_weavelang::ir::{ContinuationIr, ResumeGrade};
use bioprism_weavelang::parser::parse;
use bioprism_weave::ActKind;
use std::collections::BTreeMap;

fn roster(contracts: Vec<ParticipantContract>) -> BTreeMap<String, ParticipantContract> {
    contracts
        .into_iter()
        .map(|contract| (contract.id.clone(), contract))
        .collect()
}

/// A program that sends a `challenge` to the reviewer.
const CHALLENGING_PROGRAM: &str = r#"
policy p { allow effects [a.read] budget tokens <= 100 }
role Lead { provides [plan@1] requires [a.read] }
role Reviewer { provides [verify@1] requires [a.read] }
weave w() -> Report using p {
  bind lead to role Lead
  bind reviewer to role Reviewer
  send challenge(claimed) from lead to reviewer
}
"#;

/// A program that forks the same checkpoint twice, each fork drawing a lease.
const DOUBLE_LEASED_CHECKPOINT: &str = r#"
policy p { allow effects [a.read] budget tokens <= 100000 }
role Worker { provides [go@1] requires [a.read] }
weave w() -> Report using p {
  bind worker to role Worker
  checkpoint c = current
  fork from c {
    branch first with budget tokens(10000) { let a = ask worker.go() }
  }
  fork from c {
    branch second with budget tokens(10000) { let b = ask worker.go() }
  }
  join using verified-best
}
"#;

#[test]
fn a_program_that_sends_an_act_the_recipient_does_not_accept_is_rejected() {
    let program = parse(CHALLENGING_PROGRAM).expect("parses");
    let contracts = roster(vec![
        ParticipantContract::new("lead", "Lead").accepting([ActKind::Accept]),
        // A reviewer that can accept and reject, but has no challenge handler.
        ParticipantContract::new("reviewer", "Reviewer")
            .accepting([ActKind::Accept, ActKind::Reject]),
    ]);

    let error = check(&program, &contracts).expect_err("`reviewer` accepts no challenge");
    let TypeError::ActNotAccepted {
        act,
        recipient,
        accepted,
        ..
    } = &error
    else {
        panic!("expected an act-not-accepted error, got {error:?}");
    };
    assert_eq!(act, "challenge");
    assert_eq!(recipient, "reviewer");
    assert_eq!(accepted, &vec!["accept".to_string(), "reject".to_string()]);
    assert_eq!(error.code(), "WEAVE-E4001");
}

#[test]
fn the_same_program_type_checks_against_a_reviewer_that_does_accept_challenges() {
    let program = parse(CHALLENGING_PROGRAM).expect("parses");
    let contracts = roster(vec![
        ParticipantContract::new("lead", "Lead").accepting([ActKind::Accept]),
        ParticipantContract::new("reviewer", "Reviewer")
            .accepting([ActKind::Accept, ActKind::Reject, ActKind::Challenge]),
    ]);
    let report = check(&program, &contracts).expect("the roster satisfies the program");
    assert_eq!(report.checked_acts, 1);
    assert!(report.unchecked_participants.is_empty());
}

#[test]
fn a_program_that_leases_one_checkpoints_budget_to_two_forks_is_rejected() {
    let program = parse(DOUBLE_LEASED_CHECKPOINT).expect("parses");
    let error =
        check(&program, &BTreeMap::new()).expect_err("a snapshot's allowance cannot be drawn twice");
    let TypeError::CheckpointLeasedTwice {
        checkpoint,
        first,
        second,
        ..
    } = &error
    else {
        panic!("expected a double-lease error, got {error:?}");
    };
    assert_eq!(checkpoint, "c");
    assert_eq!(first, "first");
    assert_eq!(second, "second");
    assert_eq!(error.code(), "WEAVE-E4002");
}

#[test]
fn the_same_leases_drawn_by_one_fork_are_accepted() {
    let source = DOUBLE_LEASED_CHECKPOINT.replace(
        "  }\n  fork from c {\n    branch second",
        "\n    branch second",
    );
    let program = parse(&source).expect("parses");
    let report = check(&program, &BTreeMap::new()).expect("one fork, two branches, one allowance");
    assert_eq!(
        report.leased_checkpoints.get("c"),
        Some(&vec!["first".to_string(), "second".to_string()])
    );
}

#[test]
fn a_program_whose_branch_leases_exceed_the_ceiling_is_rejected() {
    let source = DOUBLE_LEASED_CHECKPOINT
        .replace("budget tokens <= 100000", "budget tokens <= 12000")
        .replace(
            "  }\n  fork from c {\n    branch second",
            "\n    branch second",
        );
    let program = parse(&source).expect("parses");
    let error = check(&program, &BTreeMap::new()).expect_err("20000 does not fit under 12000");
    assert_eq!(error.code(), "WEAVE-E4003");
}

#[test]
fn a_program_that_resumes_an_exact_continuation_on_a_lossy_participant_is_rejected() {
    let continuation = ContinuationIr {
        continuation_id: "continuation:repair-42:decision-14".to_string(),
        fidelity: ResumeGrade::R3,
        world_snapshot: "sha256:world".to_string(),
        local_role_state: "sha256:role-state".to_string(),
        epistemic_checkpoint: "sha256:evidence-ledger".to_string(),
        commitment_checkpoint: "sha256:commitment-ledger".to_string(),
        open_obligations: vec!["obligation:commit-order".to_string()],
        grants: vec!["grant:repo-read".to_string()],
        budget_lease: "lease:investigation-42".to_string(),
        resume_input_schema: "aurora:weave/next-action@1".to_string(),
        expected_output_schema: "aurora:weave/world-delta@1".to_string(),
        invariants: vec!["working-tree-clean".to_string()],
    };

    let lossy = ParticipantContract::new("cheap-adapter", "Investigator").resuming(ResumeGrade::R1);
    let error = lossy
        .check_resume(&continuation)
        .expect_err("an R1 adapter cannot hold an R3 continuation");
    let TypeError::FidelityTooHigh {
        required, held, ..
    } = &error
    else {
        panic!("expected a fidelity error, got {error:?}");
    };
    assert_eq!(*required, ResumeGrade::R3);
    assert_eq!(*held, ResumeGrade::R1);
    assert_eq!(error.code(), "WEAVE-E4004");

    let exact = ParticipantContract::new("full-adapter", "Investigator").resuming(ResumeGrade::R3);
    exact.check_resume(&continuation).expect("R3 holds R3");
}

#[test]
fn a_resume_grade_maps_onto_the_kernels_fidelity_rather_than_a_second_vocabulary() {
    use bioprism_weave::Fidelity;
    assert_eq!(ResumeGrade::R0.to_fidelity(), Fidelity::Frozen);
    assert_eq!(ResumeGrade::R1.to_fidelity(), Fidelity::Lossy);
    assert_eq!(ResumeGrade::R2.to_fidelity(), Fidelity::Lossy);
    assert_eq!(ResumeGrade::R3.to_fidelity(), Fidelity::Exact);
}

#[test]
fn a_participant_below_the_programs_required_abi_grade_is_rejected() {
    let contracts = roster(vec![
        ParticipantContract::new("worker", "Worker").resuming(ResumeGrade::R1)
    ]);
    let error = check_abi_grades(3, &contracts).expect_err("a forking program needs grade 3");
    assert_eq!(error.code(), "WEAVE-E4005");
    check_abi_grades(1, &contracts).expect("grade 1 is met");
}

#[test]
fn a_program_that_substitutes_an_agent_with_broader_effects_is_rejected() {
    let source = r#"
policy p { allow effects [repo.read] budget tokens <= 10 }
role Reader { provides [read@1] requires [repo.read] }
weave w() -> Report using p {
  bind agent to role Reader
  let seen = ask agent.look()
}
"#;
    let program = parse(source).expect("parses");
    let contracts = roster(vec![ParticipantContract::new("agent", "Reader")
        .with_effects(["repo.read", "deploy.production"])]);

    let error = check(&program, &contracts).expect_err("the agent can deploy; the role cannot");
    let TypeError::EffectNotSubstitutable { extra, role, .. } = &error else {
        panic!("expected a substitution error, got {error:?}");
    };
    assert_eq!(extra, &vec!["deploy.production".to_string()]);
    assert_eq!(role, "Reader");
    assert_eq!(error.code(), "WEAVE-E4006");
}

#[test]
fn an_agent_with_narrower_effects_may_still_substitute() {
    let source = r#"
policy p { allow effects [repo.read, branch.write] budget tokens <= 10 }
role Writer { provides [edit@1] requires [repo.read, branch.write] }
weave w() -> Report using p {
  bind agent to role Writer
  let seen = ask agent.look()
}
"#;
    let program = parse(source).expect("parses");
    let contracts =
        roster(vec![ParticipantContract::new("agent", "Writer").with_effects(["repo.read"])]);
    check(&program, &contracts).expect("narrower is substitutable; broader is not");
}

#[test]
fn a_claim_cannot_satisfy_a_requirement_for_a_verified_proposition() {
    let claim = CognitiveType::Claim("p:idempotency-key-regenerated".to_string());
    let required = CognitiveType::Verified("p:idempotency-key-regenerated".to_string());

    assert!(!claim.satisfies(&required));
    let error = claim
        .check_satisfies(&required)
        .expect_err("promotion needs a verifier transition");
    let TypeError::UnverifiedValue { supplied, required } = &error else {
        panic!("expected an unverified-value error, got {error:?}");
    };
    assert_eq!(supplied, "Claim");
    assert_eq!(required, "Verified");
    assert_eq!(error.code(), "WEAVE-E4007");
}

#[test]
fn a_verified_proposition_does_satisfy_a_requirement_for_a_claim() {
    let verified = CognitiveType::Verified("p".to_string());
    verified
        .check_satisfies(&CognitiveType::Claim("p".to_string()))
        .expect("verification is stronger than assertion");
}

#[test]
fn absence_disagreement_and_failure_are_four_different_types() {
    let unknown = CognitiveType::Unknown {
        reason: "no evidence".to_string(),
    };
    let conflicted = CognitiveType::Conflicted {
        candidates: vec!["a".to_string(), "b".to_string()],
    };
    let partial = CognitiveType::Partial {
        of: "p".to_string(),
        missing: vec!["confidence".to_string()],
    };
    let blocked = CognitiveType::Blocked {
        requirement: "human approval".to_string(),
    };

    for (left, right) in [
        (&unknown, &conflicted),
        (&unknown, &partial),
        (&unknown, &blocked),
        (&conflicted, &partial),
        (&conflicted, &blocked),
        (&partial, &blocked),
    ] {
        assert!(
            !left.satisfies(right),
            "23.04 keeps {} and {} distinct",
            left.name(),
            right.name()
        );
    }
    assert_eq!(unknown.proposition(), None);
    assert_eq!(partial.proposition(), Some("p"));
}

#[test]
fn a_value_may_not_flow_to_a_participant_whose_clearance_does_not_dominate_it() {
    let program = parse(CHALLENGING_PROGRAM).expect("parses");
    let contracts = roster(vec![
        ParticipantContract::new("lead", "Lead")
            .accepting([ActKind::Accept])
            .cleared_for("confidential"),
        ParticipantContract::new("reviewer", "Reviewer")
            .accepting([ActKind::Challenge])
            .cleared_for("public"),
    ]);

    let error = check(&program, &contracts).expect_err("public does not dominate confidential");
    let TypeError::LabelEscalation {
        sender_label,
        recipient_label,
        ..
    } = &error
    else {
        panic!("expected a label-flow error, got {error:?}");
    };
    assert_eq!(sender_label, "confidential");
    assert_eq!(recipient_label, "public");
    assert_eq!(error.code(), "WEAVE-E4008");
}

#[test]
fn an_unrecognised_clearance_level_is_treated_as_the_most_restrictive() {
    use bioprism_weavelang::ir::SecurityLabel;
    let known = SecurityLabel::new("restricted");
    let typo = SecurityLabel::new("restrcited");
    assert!(
        !known.dominates(&typo),
        "a misspelt clearance must not widen access"
    );
    assert!(typo.dominates(&known));
}

#[test]
fn a_participant_with_no_contract_is_reported_as_unchecked_rather_than_assumed_safe() {
    let program = parse(CHALLENGING_PROGRAM).expect("parses");
    let report = check(&program, &BTreeMap::new()).expect("nothing to contradict");
    assert_eq!(report.checked_acts, 0);
    assert!(report.unchecked_participants.contains("reviewer"));
    assert!(report.unchecked_participants.contains("lead"));
}
