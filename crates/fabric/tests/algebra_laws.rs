//! 23.41's operators, laws and substitution relation as executable properties.

mod common;

use bioprism_fabric::algebra::{
    check_affine_non_duplication, check_authority_attenuation, check_commitment_conservation,
    check_epistemic_monotonicity, check_identity, check_parallel_commutativity,
    check_sequential_associativity, choose, compare, delegate, fallback, fuse, par, race_verified,
    seq, shield, substitutable, substitute, CommitmentDisposition, CompositionError,
    DimensionVerdict, EquivalenceDimension, LawOutcome, LeasePlan, Operator,
    ParallelJustification, ProvenanceState, SubstitutionContext, SubstitutionObjection,
    SubstitutionVerdict, TerminalPath, Violation,
};
use bioprism_fabric::contract::{
    identity, identity_relabelling, AssuranceProfile, ComponentId, DeclaredCommitment,
    FailureContract, InterfaceType, PartialResultPolicy, ResourceEnvelope,
};
use bioprism_fabric::effect::{Effect, EffectKind, EffectSet, Irreversibility, Scope};
use bioprism_fabric::flow::{FlowLabel, Labelling, Sensitivity};
use bioprism_fabric::reputation::EvidenceLayer;
use bioprism_choreography::{GlobalType, Role as ChoreoRole};
use bioprism_weave::{Capability, Resource};

use common::{
    analysis, cheaper_verifier, effects, extractor, permissive_envelope, report, thin_report,
    verifier,
};

fn context() -> SubstitutionContext {
    SubstitutionContext {
        allowed_envelope: permissive_envelope(),
        minimum_assurance: AssuranceProfile::at(EvidenceLayer::SelfDeclared),
    }
}

#[test]
fn sequential_composition_refuses_when_the_producers_output_does_not_satisfy_the_consumers_input() {
    let producer = common::verifier("thin").with_effects(EffectSet::new());
    let consumer = bioprism_fabric::contract::AgentContract::new(
        "needs-more",
        common::record(
            "Report",
            &[
                ("verdict", bioprism_fabric::contract::FieldType::Text),
                ("evidence", bioprism_fabric::contract::FieldType::Text),
                ("citations", bioprism_fabric::contract::FieldType::Text),
            ],
        ),
        thin_report(),
        bioprism_fabric::contract::EpistemicContract::new(
            bioprism_fabric::contract::UncertaintySemantics::Calibrated,
        ),
        AssuranceProfile::at(EvidenceLayer::PrismEvaluated),
    );
    let error = seq(&producer, &consumer).unwrap_err();
    match error {
        CompositionError::InterfaceMismatch { missing, .. } => {
            assert_eq!(missing, vec!["citations".to_string()]);
        }
        other => panic!("expected an interface mismatch, got {other:?}"),
    }
}

#[test]
fn a_composite_effect_set_is_the_union_of_its_parts_and_never_larger() {
    let composition = seq(&extractor("x"), &verifier("v")).expect("well-typed");
    let union = extractor("x").effects.union(&verifier("v").effects);
    assert_eq!(composition.contract.effects, union);
    assert!(composition.contract.effects.escalation_over(&union).is_empty());
}

#[test]
fn parallel_composition_refuses_overlapping_write_sets_and_names_the_overlap() {
    let left = verifier("left").with_effects(effects(&[(EffectKind::FilesystemWrite, "repo/**")]));
    let right =
        verifier("right").with_effects(effects(&[(EffectKind::FilesystemWrite, "repo/src/a")]));
    let error = par(&left, &right, ParallelJustification::DisjointWriteSets).unwrap_err();
    match error {
        CompositionError::WriteSetsOverlap { overlap, .. } => assert!(!overlap.is_empty()),
        other => panic!("expected a write-set overlap, got {other:?}"),
    }
}

#[test]
fn parallel_composition_accepts_an_overlap_when_a_merge_contract_is_declared() {
    let left = verifier("left").with_effects(effects(&[(EffectKind::FilesystemWrite, "repo/**")]));
    let right =
        verifier("right").with_effects(effects(&[(EffectKind::FilesystemWrite, "repo/src/a")]));
    let composition = par(
        &left,
        &right,
        ParallelJustification::MergeContract {
            contract: "three-way-merge@1".to_string(),
        },
    )
    .expect("a declared merge contract makes it legal");
    assert!(matches!(
        composition.operator,
        Operator::Parallel {
            justification: ParallelJustification::MergeContract { .. }
        }
    ));
}

#[test]
fn parallel_commutativity_fails_exactly_when_the_write_sets_overlap() {
    let disjoint_a =
        verifier("a").with_effects(effects(&[(EffectKind::FilesystemWrite, "repo/a/**")]));
    let disjoint_b =
        verifier("b").with_effects(effects(&[(EffectKind::FilesystemWrite, "repo/b/**")]));
    assert!(check_parallel_commutativity(&disjoint_a, &disjoint_b)
        .outcome
        .holds());

    let clashing_b =
        verifier("b").with_effects(effects(&[(EffectKind::FilesystemWrite, "repo/a/x")]));
    let report = check_parallel_commutativity(&disjoint_a, &clashing_b);
    match report.outcome {
        LawOutcome::Fails { violations } => {
            assert!(matches!(violations[0], Violation::WriteSetsOverlap { .. }))
        }
        other => panic!("expected the law to fail, got {other:?}"),
    }
}

#[test]
fn an_undeclared_write_scope_counts_as_overlapping_because_disjointness_cannot_be_shown() {
    let bounded =
        verifier("a").with_effects(effects(&[(EffectKind::FilesystemWrite, "repo/a/**")]));
    let unscoped = verifier("b").with_effects(EffectSet::new().with(Effect::new(
        EffectKind::FilesystemWrite,
        Scope::Undeclared,
    )));
    assert!(!check_parallel_commutativity(&bounded, &unscoped)
        .outcome
        .holds());
}

#[test]
fn sequential_associativity_holds_for_reversible_deadline_insensitive_components() {
    let a = extractor("a");
    let b = verifier("b").with_effects(EffectSet::new());
    let c = verifier("c").with_effects(EffectSet::new());
    let b = bioprism_fabric::contract::AgentContract {
        output: analysis(),
        ..b
    };
    let report = check_sequential_associativity(&a, &b, &c);
    assert!(report.outcome.holds(), "{report:?}");
}

#[test]
fn sequential_associativity_fails_across_an_uncompensated_irreversible_effect() {
    let publish = Effect::at_class(
        EffectKind::ExternalPublish,
        Scope::resource("github.pull-request/42").unwrap(),
        Irreversibility::E4,
    )
    .unwrap();
    let a = extractor("a");
    let b = bioprism_fabric::contract::AgentContract {
        output: analysis(),
        ..verifier("b")
    }
    .with_effects(EffectSet::new().with(publish.clone()));
    let c = verifier("c").with_effects(EffectSet::new());
    let report = check_sequential_associativity(&a, &b, &c);
    match report.outcome {
        LawOutcome::Fails { violations } => assert!(violations.iter().any(|v| matches!(
            v,
            Violation::IrreversibleEffectCrossesBoundary { effect, .. } if *effect == publish
        ))),
        other => panic!("expected the law to fail, got {other:?}"),
    }
}

#[test]
fn sequential_associativity_fails_for_a_deadline_sensitive_component() {
    let a = extractor("a");
    let b = bioprism_fabric::contract::AgentContract {
        output: analysis(),
        ..verifier("b")
    }
    .with_effects(EffectSet::new())
    .with_failure(FailureContract::new().deadline_sensitive());
    let c = verifier("c").with_effects(EffectSet::new());
    match check_sequential_associativity(&a, &b, &c).outcome {
        LawOutcome::Fails { violations } => assert!(violations
            .iter()
            .any(|v| matches!(v, Violation::DeadlineSensitiveComponent { .. }))),
        other => panic!("expected the law to fail, got {other:?}"),
    }
}

#[test]
fn sequential_associativity_is_inapplicable_rather_than_false_when_a_grouping_is_ill_typed() {
    let a = extractor("a");
    let b = verifier("b");
    let c = extractor("c");
    assert!(matches!(
        check_sequential_associativity(&a, &b, &c).outcome,
        LawOutcome::Inapplicable { .. }
    ));
}

#[test]
fn a_pure_identity_satisfies_the_identity_law() {
    let a = verifier("a");
    let id = identity(&report());
    assert!(check_identity(&a, &id).outcome.holds());
}

#[test]
fn an_identity_that_changes_the_security_label_breaks_the_identity_law() {
    let a = verifier("a").emitting_at(Labelling::Labelled(FlowLabel::open_at(Sensitivity::Public)));
    let relabelling = identity_relabelling(
        &report(),
        Labelling::Labelled(FlowLabel::open_at(Sensitivity::Restricted)),
    );
    match check_identity(&a, &relabelling).outcome {
        LawOutcome::Fails { violations } => assert!(violations
            .iter()
            .any(|v| matches!(v, Violation::IdentityChangesLabel { .. }))),
        other => panic!("expected the law to fail, got {other:?}"),
    }
}

#[test]
fn an_identity_that_consumes_budget_breaks_the_identity_law() {
    let a = verifier("a");
    let costly = identity(&report()).with_envelope(ResourceEnvelope::new().tokens(1));
    match check_identity(&a, &costly).outcome {
        LawOutcome::Fails { violations } => assert!(violations
            .iter()
            .any(|v| matches!(v, Violation::IdentityChangesEnvelope { .. }))),
        other => panic!("expected the law to fail, got {other:?}"),
    }
}

#[test]
fn delegation_may_not_grant_authority_the_delegator_does_not_hold() {
    let delegator = verifier("lead").with_authority([Capability::ReadEvidence]);
    let worker = bioprism_fabric::contract::AgentContract {
        input: report(),
        ..verifier("worker")
    };
    let error = delegate(
        &delegator,
        [Capability::BranchWrite].into_iter().collect(),
        &worker,
    )
    .unwrap_err();
    match error {
        CompositionError::AuthorityAmplified { missing, .. } => {
            assert!(missing.contains(&Capability::BranchWrite))
        }
        other => panic!("expected amplification, got {other:?}"),
    }
}

#[test]
fn no_operator_creates_authority_absent_from_its_inputs() {
    let a = extractor("a").with_authority([Capability::ReadEvidence]);
    let b = verifier("b").with_authority([Capability::ReadWorld]);
    let composition = seq(&a, &b).expect("well-typed");
    assert!(check_authority_attenuation(&composition).outcome.holds());

    let mut forged = composition.clone();
    forged.contract.authority.insert(Capability::PublishResult);
    match check_authority_attenuation(&forged).outcome {
        LawOutcome::Fails { violations } => assert!(violations.iter().any(|v| matches!(
            v,
            Violation::AuthorityCreated { capabilities } if capabilities.contains(&Capability::PublishResult)
        ))),
        other => panic!("expected the law to fail, got {other:?}"),
    }
}

#[test]
fn an_affine_budget_cannot_be_subdivided_past_its_total() {
    let plan = LeasePlan::new(Resource::Tokens, 100)
        .allocating(&ComponentId::new("a"), 60)
        .allocating(&ComponentId::new("b"), 60);
    match check_affine_non_duplication(&plan).outcome {
        LawOutcome::Fails { violations } => assert!(violations
            .iter()
            .any(|v| matches!(v, Violation::BudgetOversubscribed { .. }))),
        other => panic!("expected oversubscription, got {other:?}"),
    }
}

#[test]
fn an_affine_budget_that_fits_upholds_the_law() {
    let plan = LeasePlan::new(Resource::Tokens, 100)
        .allocating(&ComponentId::new("a"), 60)
        .allocating(&ComponentId::new("b"), 40);
    assert!(check_affine_non_duplication(&plan).outcome.holds());
}

#[test]
fn an_exclusive_lock_held_by_two_components_violates_affine_non_duplication() {
    let plan = LeasePlan::new(Resource::Tokens, 100)
        .holding("deploy-key", &ComponentId::new("a"))
        .holding("deploy-key", &ComponentId::new("b"));
    match check_affine_non_duplication(&plan).outcome {
        LawOutcome::Fails { violations } => assert!(violations.iter().any(|v| matches!(
            v,
            Violation::AffineResourceDuplicated { resource, .. } if resource == "deploy-key"
        ))),
        other => panic!("expected duplication, got {other:?}"),
    }
}

#[test]
fn a_terminal_path_that_omits_a_mandatory_commitment_violates_conservation() {
    let a = extractor("a").committing(DeclaredCommitment::mandatory("deliver-report"));
    let b = verifier("b").committing(DeclaredCommitment::discretionary("send-summary"));
    let composition = seq(&a, &b).expect("well-typed");

    let complete = TerminalPath::new("success")
        .disposing("deliver-report", CommitmentDisposition::Closed);
    assert!(
        check_commitment_conservation(&composition, &[complete])
            .outcome
            .holds()
    );

    let incomplete = TerminalPath::new("abort");
    match check_commitment_conservation(&composition, &[incomplete]).outcome {
        LawOutcome::Fails { violations } => assert!(violations.iter().any(|v| matches!(
            v,
            Violation::MandatoryCommitmentUnaccounted { commitment, .. } if commitment == "deliver-report"
        ))),
        other => panic!("expected the law to fail, got {other:?}"),
    }
}

#[test]
fn commitment_conservation_is_inapplicable_when_no_terminal_path_is_supplied() {
    let composition = seq(&extractor("a"), &verifier("b")).expect("well-typed");
    assert!(matches!(
        check_commitment_conservation(&composition, &[]).outcome,
        LawOutcome::Inapplicable { .. }
    ));
}

#[test]
fn a_retraction_adds_to_the_record_and_erasing_a_claim_violates_monotonicity() {
    let before = ProvenanceState::default().asserting("c1", &["e1", "e2"]);
    let retracted = before.clone().retracting("c1");
    assert!(check_epistemic_monotonicity(&before, &retracted)
        .outcome
        .holds());

    let erased = ProvenanceState::default();
    match check_epistemic_monotonicity(&before, &erased).outcome {
        LawOutcome::Fails { violations } => {
            assert!(violations
                .iter()
                .any(|v| matches!(v, Violation::ClaimErased { claim } if claim == "c1")));
            assert!(violations
                .iter()
                .any(|v| matches!(v, Violation::EvidenceLineageRemoved { .. })));
        }
        other => panic!("expected the law to fail, got {other:?}"),
    }
}

#[test]
fn a_replacement_with_identical_output_but_weaker_calibration_is_not_substitutable() {
    let verdict = substitutable(&cheaper_verifier("cheap"), &verifier("good"), &context());
    match verdict {
        SubstitutionVerdict::Refused { objections } => {
            assert!(objections
                .iter()
                .any(|o| matches!(o, SubstitutionObjection::EpistemicWeakened { .. })));
            assert!(!objections
                .iter()
                .any(|o| matches!(o, SubstitutionObjection::EffectsNotSubset { .. })));
        }
        other => panic!("expected a refusal, got {other:?}"),
    }
}

#[test]
fn a_replacement_whose_effects_are_a_strict_subset_and_guarantees_a_superset_refines() {
    let original = verifier("original");
    let replacement = bioprism_fabric::contract::AgentContract {
        id: ComponentId::new("replacement"),
        ..verifier("replacement")
    }
    .with_effects(EffectSet::new());
    assert!(substitutable(&replacement, &original, &context()).admitted());
}

#[test]
fn a_replacement_needing_broader_authority_is_refused_naming_the_capability() {
    let original = verifier("original").with_authority([Capability::ReadEvidence]);
    let replacement = verifier("replacement")
        .with_authority([Capability::ReadEvidence, Capability::PublishResult]);
    match substitutable(&replacement, &original, &context()) {
        SubstitutionVerdict::Refused { objections } => assert!(objections.iter().any(|o| matches!(
            o,
            SubstitutionObjection::AuthorityBroadened { extra } if extra.contains(&Capability::PublishResult)
        ))),
        other => panic!("expected a refusal, got {other:?}"),
    }
}

#[test]
fn a_replacement_that_omits_an_output_field_does_not_refine() {
    let original = verifier("original");
    let replacement = bioprism_fabric::contract::AgentContract {
        output: thin_report(),
        ..verifier("replacement")
    };
    match substitutable(&replacement, &original, &context()) {
        SubstitutionVerdict::Refused { objections } => assert!(objections.iter().any(|o| matches!(
            o,
            SubstitutionObjection::OutputDoesNotRefine { missing } if missing.contains(&"evidence".to_string())
        ))),
        other => panic!("expected a refusal, got {other:?}"),
    }
}

#[test]
fn substitution_is_undecided_rather_than_permitted_when_a_scope_is_undeclared() {
    let original = verifier("original");
    let replacement = verifier("replacement").with_effects(
        EffectSet::new().with(Effect::new(EffectKind::ArtifactRead, Scope::Undeclared)),
    );
    match substitutable(&replacement, &original, &context()) {
        SubstitutionVerdict::Undecided { objections } => assert!(objections
            .iter()
            .any(|o| matches!(o, SubstitutionObjection::EffectsUndecided { .. }))),
        other => panic!("expected undecided, got {other:?}"),
    }
}

#[test]
fn an_unmeasured_assurance_never_meets_a_stated_minimum() {
    let strict = SubstitutionContext {
        allowed_envelope: permissive_envelope(),
        minimum_assurance: AssuranceProfile::at(EvidenceLayer::SelfDeclared)
            .with_lower_bound_bp(5_000),
    };
    let unmeasured = bioprism_fabric::contract::AgentContract {
        assurance: AssuranceProfile::at(EvidenceLayer::PrismEvaluated),
        ..verifier("unmeasured")
    };
    match substitutable(&unmeasured, &verifier("original"), &strict) {
        SubstitutionVerdict::Refused { objections } => assert!(objections.iter().any(|o| matches!(
            o,
            SubstitutionObjection::AssuranceInsufficient { shortfalls }
                if shortfalls.iter().any(|s| matches!(
                    s,
                    bioprism_fabric::contract::AssuranceShortfall::Unmeasured { .. }
                ))
        ))),
        other => panic!("expected a refusal, got {other:?}"),
    }
}

#[test]
fn a_replacement_that_drops_cancellation_weakens_failure_semantics() {
    let original = verifier("original").with_failure(
        FailureContract::new()
            .cancellable()
            .returning(PartialResultPolicy::ReturnMarked),
    );
    let replacement = verifier("replacement").with_failure(FailureContract::new());
    match substitutable(&replacement, &original, &context()) {
        SubstitutionVerdict::Refused { objections } => assert!(objections
            .iter()
            .any(|o| matches!(o, SubstitutionObjection::FailureSemanticsWeakened { .. }))),
        other => panic!("expected a refusal, got {other:?}"),
    }
}

#[test]
fn substituting_into_a_composition_refuses_before_rebuilding_it() {
    let composition = seq(&extractor("x"), &verifier("v")).expect("well-typed");
    let error = substitute(
        &composition,
        &ComponentId::new("v"),
        &cheaper_verifier("cheap"),
        &context(),
    )
    .unwrap_err();
    assert!(matches!(
        error,
        CompositionError::SubstitutionRefused { .. }
    ));
}

#[test]
fn a_permitted_substitution_rebuilds_the_composite_effect_account_from_the_operator() {
    let composition = seq(&extractor("x"), &verifier("v")).expect("well-typed");
    let leaner = bioprism_fabric::contract::AgentContract {
        id: ComponentId::new("v"),
        ..verifier("v")
    }
    .with_effects(EffectSet::new());
    let rebuilt = substitute(&composition, &ComponentId::new("v"), &leaner, &context())
        .expect("a strictly smaller effect set refines");
    assert_eq!(rebuilt.contract.effects, extractor("x").effects);
}

#[test]
fn equivalence_is_reported_per_dimension_and_never_collapsed_to_a_boolean() {
    let report = compare(&verifier("a"), &cheaper_verifier("b"));
    assert_eq!(report.dimensions.len(), EquivalenceDimension::ALL.len());
    assert!(report
        .dimensions_differing()
        .contains(&EquivalenceDimension::Evidence));
    assert!(matches!(
        report.dimensions[&EquivalenceDimension::CostDistribution],
        DimensionVerdict::NotComparable { .. }
    ));
}

#[test]
fn a_race_without_a_verifier_is_refused_because_speed_alone_cannot_pick_a_winner() {
    let branches = [verifier("a"), verifier("b")];
    assert!(matches!(
        race_verified(&branches, "").unwrap_err(),
        CompositionError::UnverifiedRace
    ));
    assert!(race_verified(&branches, "deterministic-tests").is_ok());
}

#[test]
fn a_fallback_without_a_declared_predicate_is_refused() {
    assert!(matches!(
        fallback(&verifier("a"), &verifier("b"), "").unwrap_err(),
        CompositionError::UndeclaredFailurePredicate
    ));
    assert!(fallback(&verifier("a"), &verifier("b"), "timeout").is_ok());
}

#[test]
fn a_shield_that_cannot_contain_its_component_is_refused_and_a_shield_that_can_is_visible() {
    let component = verifier("worker")
        .with_effects(effects(&[(EffectKind::FilesystemWrite, "repo/**")]));
    let too_narrow = effects(&[(EffectKind::ArtifactRead, "corpus/**")]);
    assert!(matches!(
        shield(&component, "monitor", too_narrow).unwrap_err(),
        CompositionError::ShieldCannotContain { .. }
    ));

    let adequate = effects(&[(EffectKind::FilesystemWrite, "repo/**")]);
    let shielded = shield(&component, "policy-monitor@1", adequate).expect("contains");
    assert_eq!(
        shielded.contract.assurance.shielded_by.as_deref(),
        Some("policy-monitor@1")
    );
}

#[test]
fn policy_choice_refuses_branches_that_accept_unrelated_inputs() {
    assert!(matches!(
        choose(&extractor("a"), &verifier("b"), "router").unwrap_err(),
        CompositionError::BranchesNotInterchangeable { .. }
    ));
}

#[test]
fn a_fusion_refuses_a_choreography_role_no_participant_fills() {
    let choreography = GlobalType::message(
        ChoreoRole::new("x"),
        ChoreoRole::new("v"),
        bioprism_choreography::Label::new("analysis"),
        GlobalType::End,
    )
    .well_formed()
    .expect("well-formed");
    let error = fuse(&[extractor("x")], &choreography, &report()).unwrap_err();
    match error {
        CompositionError::UnfilledRoles { roles } => assert!(roles.contains("v")),
        other => panic!("expected an unfilled role, got {other:?}"),
    }
}

#[test]
fn a_fusion_exports_one_interface_and_carries_the_choreography_digest() {
    let choreography = GlobalType::message(
        ChoreoRole::new("x"),
        ChoreoRole::new("v"),
        bioprism_choreography::Label::new("analysis"),
        GlobalType::End,
    )
    .well_formed()
    .expect("well-formed");
    let digest = choreography.digest().expect("digest");
    let molecule = fuse(&[extractor("x"), verifier("v")], &choreography, &report())
        .expect("both roles filled");
    assert_eq!(molecule.contract.output, report());
    assert!(matches!(
        &molecule.operator,
        Operator::ChoreographedFusion { choreography_digest } if *choreography_digest == digest
    ));
    assert!(check_authority_attenuation(&molecule).outcome.holds());
}

#[test]
fn a_replacement_that_emits_at_a_wider_label_than_the_original_is_refused() {
    let original =
        verifier("original").emitting_at(Labelling::Labelled(FlowLabel::open_at(Sensitivity::Public)));
    let replacement = verifier("replacement").emitting_at(Labelling::Labelled(
        FlowLabel::open_at(Sensitivity::Restricted).in_compartment("patient-42"),
    ));
    match substitutable(&replacement, &original, &context()) {
        SubstitutionVerdict::Refused { objections } => assert!(objections
            .iter()
            .any(|o| matches!(o, SubstitutionObjection::OutputLabelWidened { .. }))),
        other => panic!("expected a refusal, got {other:?}"),
    }
}

#[test]
fn a_replacement_emitting_at_a_narrower_label_is_accepted_on_the_flow_clause() {
    let original = verifier("original").emitting_at(Labelling::Labelled(
        FlowLabel::open_at(Sensitivity::Restricted).in_compartment("patient-42"),
    ));
    let replacement = bioprism_fabric::contract::AgentContract {
        id: ComponentId::new("replacement"),
        ..verifier("replacement")
    }
    .emitting_at(Labelling::Labelled(FlowLabel::open_at(Sensitivity::Public)));
    match substitutable(&replacement, &original, &context()) {
        SubstitutionVerdict::Refines => {}
        other => panic!("expected refinement, got {other:?}"),
    }
}

#[test]
fn every_law_report_and_verdict_round_trips_through_json_unchanged() {
    let composition = seq(&extractor("x"), &verifier("v")).expect("well-typed");
    let report = check_authority_attenuation(&composition);
    let encoded = serde_json::to_string(&report).unwrap();
    assert_eq!(
        serde_json::from_str::<bioprism_fabric::algebra::LawReport>(&encoded).unwrap(),
        report
    );

    let verdict = substitutable(&cheaper_verifier("cheap"), &verifier("good"), &context());
    let encoded = serde_json::to_string(&verdict).unwrap();
    assert_eq!(
        serde_json::from_str::<SubstitutionVerdict>(&encoded).unwrap(),
        verdict
    );
}

#[test]
fn composing_the_same_contracts_twice_yields_byte_identical_composites() {
    let first = seq(&extractor("x"), &verifier("v")).unwrap();
    let second = seq(&extractor("x"), &verifier("v")).unwrap();
    assert_eq!(
        serde_json::to_string(&first).unwrap(),
        serde_json::to_string(&second).unwrap()
    );
}

#[test]
fn an_interface_with_more_fields_is_a_subtype_of_one_with_fewer() {
    assert!(report().subtypes(&thin_report()));
    assert!(!thin_report().subtypes(&report()));
    assert!(InterfaceType::new("Anything").subtypes(&InterfaceType::new("Other")));
}
