//! 23.46: identity layers, the evidence ladder, attested versus claimed, contextual reputation.

use bioprism_fabric::effect::Irreversibility;
use bioprism_fabric::flow::Sensitivity;
use bioprism_fabric::reputation::{
    bind, independent_subjects, Attestation, AttestationStatus, BindingDecision, CapabilityAssertion,
    CapabilityCard, ContextLookup, ContextualScore, DeploymentContext, EvidenceLayer, FailedGate,
    IdentityLayer, IdentityLayers, LogicalTime, Reputation, ReputationContext, ReputationError,
    RevocationEvent, RevocationReason, RevocationResponse, RungStatus, SelfDeclaration,
    UnmeasuredReason, ValidityWindow, WindowKey,
};

fn context(capability: &str, risk: Irreversibility) -> ReputationContext {
    ReputationContext {
        capability: capability.to_string(),
        task_domain: "software".to_string(),
        effect_risk: risk,
        data_classification: Sensitivity::Internal,
        evaluator: "prism".to_string(),
        benchmark_version: "weavebench@0.4".to_string(),
        window: WindowKey::new(0, 100),
        software_version: "build-1".to_string(),
        deployment: DeploymentContext::PublicBenchmark,
    }
}

fn attestation(subject: &str, claim: &str, layer: EvidenceLayer) -> Attestation {
    Attestation::new(subject, claim, "registry", layer, ValidityWindow::new(0, 100))
        .expect("issuer differs from subject")
}

#[test]
fn a_reputation_earned_in_one_context_does_not_transfer_to_another() {
    let mut reputation = Reputation::new("agent:investigator-2");
    reputation.record(
        context("literature.retrieve", Irreversibility::E0),
        ContextualScore::new(48, 50).unwrap(),
    );

    assert!(matches!(
        reputation.lookup(&context("literature.retrieve", Irreversibility::E0)),
        ContextLookup::Measured { .. }
    ));
    let elsewhere = reputation.lookup(&context("code.modify", Irreversibility::E3));
    assert!(matches!(elsewhere, ContextLookup::Unmeasured { .. }));
    assert!(elsewhere.score().is_none());
}

#[test]
fn past_performance_under_read_only_access_does_not_establish_safety_with_production_credentials() {
    let mut reputation = Reputation::new("agent:patcher");
    reputation.record(
        context("code.modify", Irreversibility::E0),
        ContextualScore::new(40, 40).unwrap(),
    );
    let production = reputation.lookup(&context("code.modify", Irreversibility::E4));
    match production {
        ContextLookup::Unmeasured { reason } => assert!(matches!(
            reason,
            UnmeasuredReason::MeasuredElsewhere { .. }
        )),
        other => panic!("expected unmeasured, got {other:?}"),
    }
}

#[test]
fn a_never_probed_context_is_distinguishable_from_one_measured_under_a_different_key() {
    let mut reputation = Reputation::new("agent:x");
    reputation.record(
        context("a", Irreversibility::E0),
        ContextualScore::new(1, 2).unwrap(),
    );
    assert!(matches!(
        reputation.lookup(&context("a", Irreversibility::E1)),
        ContextLookup::Unmeasured {
            reason: UnmeasuredReason::MeasuredElsewhere { .. }
        }
    ));
    assert!(matches!(
        reputation.lookup(&context("z", Irreversibility::E0)),
        ContextLookup::Unmeasured {
            reason: UnmeasuredReason::NeverProbed
        }
    ));
}

#[test]
fn a_contextual_score_cannot_be_constructed_with_a_zero_denominator() {
    assert!(matches!(
        ContextualScore::new(0, 0).unwrap_err(),
        ReputationError::EmptyDenominator
    ));
    assert!(matches!(
        ContextualScore::new(5, 3).unwrap_err(),
        ReputationError::SuccessesExceedTrials { .. }
    ));
}

#[test]
fn a_single_success_does_not_read_as_certainty() {
    let one = ContextualScore::new(1, 1).unwrap();
    assert_eq!(one.rate_bp(), 10_000);
    assert_eq!(one.lower_bound_bp(), 5_000);
    let many = ContextualScore::new(100, 100).unwrap();
    assert!(many.lower_bound_bp() > one.lower_bound_bp());
}

#[test]
fn an_agent_cannot_attest_to_itself() {
    assert!(matches!(
        Attestation::new(
            "agent:x",
            "capability/y",
            "agent:x",
            EvidenceLayer::PrismEvaluated,
            ValidityWindow::new(0, 10)
        )
        .unwrap_err(),
        ReputationError::SelfIssuedAttestation { .. }
    ));
}

#[test]
fn a_claimed_capability_and_an_attested_one_are_different_values() {
    let claimed = CapabilityAssertion::Claimed(SelfDeclaration {
        subject: "agent:x".into(),
        claim: "capability/verify".into(),
    });
    let attested = CapabilityAssertion::Attested(attestation(
        "agent:x",
        "capability/verify",
        EvidenceLayer::PrismEvaluated,
    ));
    assert_eq!(claimed.claim(), attested.claim());
    assert_ne!(claimed.layer(), attested.layer());
    assert_eq!(claimed.layer(), EvidenceLayer::SelfDeclared);
    assert!(!claimed.layer().is_third_party());
}

#[test]
fn a_card_records_each_rung_independently_rather_than_only_its_maximum() {
    let card = CapabilityCard::new("agent:x")
        .declaring("capability/verify")
        .attesting(attestation(
            "agent:x",
            "capability/verify",
            EvidenceLayer::FixtureVerified,
        ))
        .unwrap()
        .attesting(attestation(
            "agent:x",
            "capability/verify",
            EvidenceLayer::IndependentlyAttested,
        ))
        .unwrap();
    let ladder = card.ladder("capability/verify", LogicalTime(5));
    assert_eq!(ladder[&EvidenceLayer::FixtureVerified], RungStatus::Valid);
    assert_eq!(
        ladder[&EvidenceLayer::IndependentlyAttested],
        RungStatus::Valid
    );
    assert_eq!(ladder[&EvidenceLayer::PrismEvaluated], RungStatus::NoEvidence);
    assert_eq!(
        ladder[&EvidenceLayer::SelfDeclared],
        RungStatus::SelfDeclaredOnly
    );
}

#[test]
fn a_self_declaration_never_reaches_a_rung_above_self_declared() {
    let card = CapabilityCard::new("agent:x").declaring("capability/verify");
    assert!(card
        .highest_valid_rung("capability/verify", LogicalTime(1))
        .is_none());
}

#[test]
fn an_attestation_outside_its_validity_window_is_expired_rather_than_valid() {
    let attestation = attestation("agent:x", "c", EvidenceLayer::PrismEvaluated);
    assert_eq!(attestation.status(LogicalTime(50)), AttestationStatus::Valid);
    assert!(matches!(
        attestation.status(LogicalTime(200)),
        AttestationStatus::Expired { .. }
    ));
}

#[test]
fn a_revoked_attestation_is_revoked_even_inside_its_window() {
    let attestation = attestation("agent:x", "c", EvidenceLayer::PrismEvaluated)
        .revoked_for(RevocationReason::BenchmarkExploit);
    assert!(matches!(
        attestation.status(LogicalTime(10)),
        AttestationStatus::Revoked(RevocationReason::BenchmarkExploit)
    ));
}

#[test]
fn a_card_refuses_an_attestation_about_a_different_subject() {
    assert!(CapabilityCard::new("agent:x")
        .attesting(attestation("agent:y", "c", EvidenceLayer::PrismEvaluated))
        .is_err());
}

#[test]
fn identity_layer_drift_is_visible_when_only_the_model_changes_behind_a_stable_endpoint() {
    let before = IdentityLayers::new()
        .endpoint("https://agents.example/verify")
        .model("model-a@1");
    let after = IdentityLayers::new()
        .endpoint("https://agents.example/verify")
        .model("model-b@2");
    let drifted = before.drifted(&after);
    assert!(drifted.contains(&IdentityLayer::Model));
    assert!(!drifted.contains(&IdentityLayer::Endpoint));
}

fn requirement(capability: &str) -> bioprism_fabric::reputation::RoleRequirement {
    bioprism_fabric::reputation::RoleRequirement {
        capability: capability.to_string(),
        minimum_rung: EvidenceLayer::PrismEvaluated,
        minimum_lower_bound_bp: Some(6_000),
        context: context(capability, Irreversibility::E0),
        required_effects: ["artifact.read".to_string()].into_iter().collect(),
        permitted_organizations: Default::default(),
        bound_lineages: Default::default(),
    }
}

#[test]
fn a_self_advertised_capability_produces_a_candidate_and_never_a_binding() {
    let card = CapabilityCard::new("agent:x").declaring("capability/verify");
    let reputation = Reputation::new("agent:x");
    match bind(&requirement("capability/verify"), &card, &reputation, LogicalTime(1)) {
        BindingDecision::Candidate { failed } => {
            assert!(failed
                .iter()
                .any(|g| matches!(g, FailedGate::NoAttestationForCapability { .. })));
        }
        other => panic!("expected a candidate, got {other:?}"),
    }
}

#[test]
fn a_candidate_with_no_declaration_and_no_evidence_is_rejected_outright() {
    let card = CapabilityCard::new("agent:x");
    let reputation = Reputation::new("agent:x");
    assert!(matches!(
        bind(&requirement("capability/verify"), &card, &reputation, LogicalTime(1)),
        BindingDecision::Rejected { .. }
    ));
}

#[test]
fn an_unmeasured_candidate_fails_the_lower_bound_gate_as_unmeasured_not_as_low_scoring() {
    let card = CapabilityCard::new("agent:x")
        .declaring("capability/verify")
        .attesting(
            attestation("agent:x", "capability/verify", EvidenceLayer::PrismEvaluated)
                .scoped_to_effect("artifact.read"),
        )
        .unwrap();
    let reputation = Reputation::new("agent:x");
    match bind(&requirement("capability/verify"), &card, &reputation, LogicalTime(1)) {
        BindingDecision::Candidate { failed } | BindingDecision::Rejected { failed } => {
            assert!(failed.iter().any(|g| matches!(g, FailedGate::Unmeasured { .. })));
            assert!(!failed
                .iter()
                .any(|g| matches!(g, FailedGate::BelowLowerBound { .. })));
        }
        other => panic!("expected a failed gate, got {other:?}"),
    }
}

#[test]
fn a_fully_evidenced_candidate_binds() {
    let card = CapabilityCard::new("agent:x")
        .declaring("capability/verify")
        .attesting(
            attestation("agent:x", "capability/verify", EvidenceLayer::PrismEvaluated)
                .scoped_to_effect("artifact.read"),
        )
        .unwrap();
    let mut reputation = Reputation::new("agent:x");
    reputation.record(
        context("capability/verify", Irreversibility::E0),
        ContextualScore::new(90, 100).unwrap(),
    );
    match bind(&requirement("capability/verify"), &card, &reputation, LogicalTime(1)) {
        BindingDecision::Bound { rung, lower_bound_bp } => {
            assert_eq!(rung, EvidenceLayer::PrismEvaluated);
            assert!(lower_bound_bp >= 6_000);
        }
        other => panic!("expected a binding, got {other:?}"),
    }
}

#[test]
fn an_effect_outside_the_attested_scope_fails_a_gate_that_self_declaration_cannot_override() {
    let card = CapabilityCard::new("agent:x")
        .declaring("capability/verify")
        .attesting(
            attestation("agent:x", "capability/verify", EvidenceLayer::PrismEvaluated)
                .scoped_to_effect("search.query"),
        )
        .unwrap();
    let mut reputation = Reputation::new("agent:x");
    reputation.record(
        context("capability/verify", Irreversibility::E0),
        ContextualScore::new(99, 100).unwrap(),
    );
    match bind(&requirement("capability/verify"), &card, &reputation, LogicalTime(1)) {
        BindingDecision::Candidate { failed } => assert!(failed
            .iter()
            .any(|g| matches!(g, FailedGate::EffectOutsideAttestedScope { .. }))),
        other => panic!("expected a candidate with a failed gate, got {other:?}"),
    }
}

#[test]
fn a_candidate_sharing_a_bound_lineage_fails_the_correlated_failure_gate() {
    let card = CapabilityCard::new("agent:x")
        .declaring("capability/verify")
        .with_lineage("runtime:alpha")
        .attesting(
            attestation("agent:x", "capability/verify", EvidenceLayer::PrismEvaluated)
                .scoped_to_effect("artifact.read"),
        )
        .unwrap();
    let mut reputation = Reputation::new("agent:x");
    reputation.record(
        context("capability/verify", Irreversibility::E0),
        ContextualScore::new(99, 100).unwrap(),
    );
    let mut requirement = requirement("capability/verify");
    requirement.bound_lineages.insert("runtime:alpha".into());
    match bind(&requirement, &card, &reputation, LogicalTime(1)) {
        BindingDecision::Candidate { failed } => assert!(failed.iter().any(|g| matches!(
            g,
            FailedGate::LineageAlreadyBound { lineage } if lineage == "runtime:alpha"
        ))),
        other => panic!("expected a candidate with a failed gate, got {other:?}"),
    }
}

#[test]
fn five_endpoints_sharing_a_runtime_lineage_count_as_one_independent_source() {
    let cards: Vec<CapabilityCard> = (0..5)
        .map(|i| CapabilityCard::new(format!("agent:{i}")).with_lineage("runtime:alpha"))
        .collect();
    assert_eq!(independent_subjects(&cards).len(), 1);

    let mut mixed = cards.clone();
    mixed.push(CapabilityCard::new("agent:other").with_lineage("runtime:beta"));
    assert_eq!(independent_subjects(&mixed).len(), 2);
}

#[test]
fn a_card_with_no_recorded_lineage_is_its_own_source_because_unknown_is_not_shared() {
    let cards = vec![
        CapabilityCard::new("agent:a"),
        CapabilityCard::new("agent:b"),
    ];
    assert_eq!(independent_subjects(&cards).len(), 2);
}

#[test]
fn a_security_incident_stops_a_thread_and_a_stale_evaluation_only_rebinds_it() {
    let stop = RevocationEvent {
        subject: "agent:x".into(),
        claim: "c".into(),
        reason: RevocationReason::SecurityIncident,
    };
    assert_eq!(stop.recommended_response(), RevocationResponse::Stop);

    let rebind = RevocationEvent {
        reason: RevocationReason::StaleEvaluation,
        ..stop.clone()
    };
    assert_eq!(rebind.recommended_response(), RevocationResponse::Rebind);

    let reduce = RevocationEvent {
        reason: RevocationReason::CalibrationDrift,
        ..stop
    };
    assert_eq!(
        reduce.recommended_response(),
        RevocationResponse::ReduceAuthority
    );
}

#[test]
fn reputation_exposes_no_aggregate_across_contexts() {
    let mut reputation = Reputation::new("agent:x");
    reputation.record(
        context("a", Irreversibility::E0),
        ContextualScore::new(10, 10).unwrap(),
    );
    reputation.record(
        context("b", Irreversibility::E4),
        ContextualScore::new(0, 10).unwrap(),
    );
    assert_eq!(reputation.measured_context_count(), 2);
    for (context, score) in reputation.measured() {
        assert!(!context.capability.is_empty());
        assert!(score.trials() > 0);
    }
}
