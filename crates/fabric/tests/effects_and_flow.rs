//! 23.14: the effect calculus and the agent-to-agent information-flow case.

mod common;

use bioprism_fabric::effect::{
    ComposedPolicy, Containment, Effect, EffectError, EffectKind, EffectParameter, EffectPolicy,
    EffectSet, Gate, Inclusion, Irreversibility, PolicySource, ResourcePattern, Scope,
};
use bioprism_fabric::flow::{
    declassify, project_for, taint_join, Declassifier, DisclosureMatrix, FlowDecision, FlowError,
    FlowLabel, FlowRefusal, Item, Labelling, Principal, PurposeRestriction, Residual,
    ResidualPolicy, Sensitivity, VerifierResult,
};

use common::effect as eff;

#[test]
fn the_effect_taxonomy_is_closed_and_round_trips_through_its_dotted_names() {
    for kind in EffectKind::TAXONOMY {
        assert_eq!(EffectKind::parse(kind.as_str()).unwrap(), kind);
    }
    assert!(matches!(
        EffectKind::parse("database.drop").unwrap_err(),
        EffectError::UnknownEffectKind(_)
    ));
}

#[test]
fn an_effect_may_be_declared_above_its_irreversibility_floor_and_never_below_it() {
    assert!(Effect::at_class(
        EffectKind::ArtifactWrite,
        Scope::Undeclared,
        Irreversibility::E3
    )
    .is_ok());
    assert!(matches!(
        Effect::at_class(
            EffectKind::ClinicalOutput,
            Scope::Undeclared,
            Irreversibility::E1
        )
        .unwrap_err(),
        EffectError::ClassBelowFloor { .. }
    ));
}

#[test]
fn a_recursive_wildcard_is_only_legal_as_the_last_segment() {
    assert!(ResourcePattern::parse("repo/**").is_ok());
    assert!(matches!(
        ResourcePattern::parse("repo/**/src").unwrap_err(),
        EffectError::InteriorRecursiveWildcard(_)
    ));
}

#[test]
fn resource_containment_is_structural_over_patterns() {
    let outer = ResourcePattern::parse("repo/branch/**").unwrap();
    assert!(outer.contains(&ResourcePattern::parse("repo/branch/42/src").unwrap()));
    assert!(!outer.contains(&ResourcePattern::parse("repo/main").unwrap()));

    let single = ResourcePattern::parse("repo/*/config").unwrap();
    assert!(single.contains(&ResourcePattern::parse("repo/a/config").unwrap()));
    assert!(!single.contains(&ResourcePattern::parse("repo/a/b/config").unwrap()));
}

#[test]
fn an_undeclared_scope_is_neither_contained_by_nor_containing_a_bounded_one() {
    let bounded = Scope::resource("repo/**").unwrap();
    assert_eq!(
        bounded.contains(&Scope::Undeclared),
        Containment::Undecided
    );
    assert_eq!(
        Scope::Undeclared.contains(&bounded),
        Containment::Undecided
    );
    assert_eq!(
        Scope::Undeclared.contains(&Scope::Undeclared),
        Containment::Contains
    );
}

#[test]
fn effect_inclusion_is_three_valued_and_undecided_is_not_admitted() {
    let outer = EffectSet::new().with(eff(EffectKind::ArtifactRead, "corpus/**"));
    let inner_ok = EffectSet::new().with(eff(EffectKind::ArtifactRead, "corpus/a"));
    let inner_bad = EffectSet::new().with(eff(EffectKind::ArtifactWrite, "corpus/a"));
    let inner_unknown =
        EffectSet::new().with(Effect::new(EffectKind::ArtifactRead, Scope::Undeclared));

    assert!(matches!(outer.includes(&inner_ok), Inclusion::Holds));
    assert!(matches!(
        outer.includes(&inner_bad),
        Inclusion::Fails { .. }
    ));
    let undecided = outer.includes(&inner_unknown);
    assert!(matches!(undecided, Inclusion::Undecided { .. }));
    assert!(!undecided.admitted());
}

#[test]
fn policy_composition_is_deny_by_default_with_union_of_prohibitions() {
    let write = eff(EffectKind::FilesystemWrite, "repo/**");
    let permissive = EffectPolicy::new(PolicySource::User).allowing(write.clone());
    let restrictive = EffectPolicy::new(PolicySource::Organization);
    let composed = ComposedPolicy::compose([permissive.clone(), restrictive]);
    assert!(matches!(
        composed.gate(&write),
        Gate::NotAllowed {
            by: PolicySource::Organization
        }
    ));

    let prohibiting = EffectPolicy::new(PolicySource::DataOwner)
        .allowing(write.clone())
        .prohibiting(write.clone());
    let composed = ComposedPolicy::compose([permissive, prohibiting]);
    assert!(matches!(composed.gate(&write), Gate::Prohibited { .. }));
}

#[test]
fn a_class_at_or_above_the_approval_threshold_needs_an_approval_transition() {
    let publish = Effect::new(EffectKind::ExternalPublish, Scope::Undeclared);
    let policy = EffectPolicy::new(PolicySource::Runtime)
        .allowing(publish.clone())
        .requiring_approval_from(Irreversibility::E4);
    let composed = ComposedPolicy::compose([policy]);
    assert!(matches!(
        composed.gate(&publish),
        Gate::ApprovalRequired { .. }
    ));
}

#[test]
fn an_effect_parameter_cannot_be_instantiated_outside_its_bound() {
    let parameter = EffectParameter::new(
        "R",
        ResourcePattern::parse("corpus/**").unwrap(),
    )
    .permitting(EffectKind::ArtifactRead);
    let template = EffectSet::new().with(eff(EffectKind::ArtifactRead, "$R"));

    let inside = parameter
        .instantiate(&template, &ResourcePattern::parse("corpus/pubmed").unwrap())
        .unwrap();
    assert!(inside
        .iter()
        .any(|e| matches!(&e.scope, Scope::Resource(p) if p.as_str() == "corpus/pubmed")));

    assert!(matches!(
        parameter
            .instantiate(&template, &ResourcePattern::parse("secrets/keys").unwrap())
            .unwrap_err(),
        EffectError::ArgumentOutsideBound { .. }
    ));
}

#[test]
fn an_effect_parameter_refuses_a_kind_it_does_not_permit() {
    let parameter = EffectParameter::new("R", ResourcePattern::parse("corpus/**").unwrap())
        .permitting(EffectKind::ArtifactRead);
    let template = EffectSet::new().with(eff(EffectKind::ArtifactWrite, "$R"));
    assert!(matches!(
        parameter
            .instantiate(&template, &ResourcePattern::parse("corpus/a").unwrap())
            .unwrap_err(),
        EffectError::KindOutsideParameter { .. }
    ));
}

fn restricted() -> FlowLabel {
    FlowLabel::open_at(Sensitivity::Restricted)
        .in_compartment("patient-42")
        .for_purpose("research-validation")
        .resident_in("US")
        .retained_for(30)
}

#[test]
fn an_unlabelled_value_is_not_a_public_one_and_flows_nowhere() {
    let public = Labelling::Labelled(FlowLabel::open_at(Sensitivity::Public));
    let decision = Labelling::Unlabelled.flows_to(&public);
    match decision {
        FlowDecision::Refused { refusals } => {
            assert!(refusals.contains(&FlowRefusal::SourceUnlabelled))
        }
        other => panic!("expected a refusal, got {other:?}"),
    }
}

#[test]
fn a_participant_with_no_established_clearance_receives_nothing() {
    let public = Labelling::Labelled(FlowLabel::open_at(Sensitivity::Public));
    match public.flows_to(&Principal::uncleared("stranger").clearance) {
        FlowDecision::Refused { refusals } => {
            assert!(refusals.contains(&FlowRefusal::DestinationUnlabelled))
        }
        other => panic!("expected a refusal, got {other:?}"),
    }
}

#[test]
fn an_empty_purpose_restriction_permits_no_purpose_and_is_not_unrestricted() {
    let nothing = PurposeRestriction::only(Vec::<String>::new());
    assert!(!nothing.permits_all_of(&PurposeRestriction::only(["research"])));
    assert!(PurposeRestriction::Unrestricted.permits_all_of(&nothing));
    assert!(!nothing.permits_all_of(&PurposeRestriction::Unrestricted));
}

#[test]
fn the_flow_rule_refuses_a_destination_missing_a_compartment() {
    let source = restricted();
    let destination = FlowLabel::open_at(Sensitivity::Restricted)
        .for_purpose("research-validation")
        .resident_in("US")
        .retained_for(30);
    match source.flows_to(&destination) {
        FlowDecision::Refused { refusals } => assert!(refusals.iter().any(|r| matches!(
            r,
            FlowRefusal::CompartmentsMissing { missing } if missing.contains("patient-42")
        ))),
        other => panic!("expected a refusal, got {other:?}"),
    }
}

#[test]
fn the_flow_rule_refuses_a_destination_that_would_retain_longer() {
    let source = restricted();
    let destination = restricted().retained_for(365);
    match source.flows_to(&destination) {
        FlowDecision::Refused { refusals } => assert!(refusals
            .iter()
            .any(|r| matches!(r, FlowRefusal::RetentionExceeded { .. }))),
        other => panic!("expected a refusal, got {other:?}"),
    }
}

#[test]
fn a_value_derived_from_two_sources_carries_the_join_of_their_labels() {
    let a = FlowLabel::open_at(Sensitivity::Internal).in_compartment("site-7");
    let b = FlowLabel::open_at(Sensitivity::Restricted).in_compartment("patient-42");
    let joined = a.join(&b);
    assert_eq!(joined.sensitivity, Sensitivity::Restricted);
    assert!(joined.compartments.contains("site-7"));
    assert!(joined.compartments.contains("patient-42"));
}

#[test]
fn generation_from_an_unlabelled_input_does_not_declassify() {
    let joined = taint_join(&[
        Labelling::Labelled(FlowLabel::open_at(Sensitivity::Public)),
        Labelling::Unlabelled,
    ]);
    assert_eq!(joined, Labelling::Unlabelled);
    assert_eq!(taint_join(&[]), Labelling::Unlabelled);
}

#[test]
fn an_aggregate_declassifier_refuses_below_its_cohort_and_emits_provenance_above_it() {
    let value = Labelling::Labelled(restricted());
    let declassifier = Declassifier::Aggregate { min_cohort: 20 };
    assert!(matches!(
        declassify(
            &value,
            &declassifier,
            3,
            VerifierResult::Passed {
                method: "k-anon".into()
            }
        )
        .unwrap_err(),
        FlowError::CohortTooSmall { .. }
    ));

    let (lowered, record) = declassify(
        &value,
        &declassifier,
        50,
        VerifierResult::Passed {
            method: "k-anon".into(),
        },
    )
    .expect("cohort is large enough");
    assert_eq!(
        lowered.label().unwrap().sensitivity,
        Sensitivity::Confidential
    );
    assert_eq!(record.source.sensitivity, Sensitivity::Restricted);
    assert!(record.retained_compartments.contains("patient-42"));
}

#[test]
fn an_unlabelled_value_cannot_be_declassified_at_all() {
    assert!(matches!(
        declassify(
            &Labelling::Unlabelled,
            &Declassifier::Tokenize,
            100,
            VerifierResult::Unverified {
                reason: "none".into()
            }
        )
        .unwrap_err(),
        FlowError::DeclassifyUnlabelled
    ));
}

fn items() -> Vec<Item> {
    vec![
        Item {
            id: "public-summary".into(),
            labelling: Labelling::Labelled(FlowLabel::open_at(Sensitivity::Public)),
            shape: "Summary@1".into(),
        },
        Item {
            id: "patient-record".into(),
            labelling: Labelling::Labelled(restricted()),
            shape: "Record@3".into(),
        },
        Item {
            id: "orphan".into(),
            labelling: Labelling::Unlabelled,
            shape: "Unknown".into(),
        },
    ]
}

#[test]
fn a_silent_projection_leaves_the_recipient_unable_to_detect_that_anything_was_withheld() {
    let recipient = Principal::new("reader", FlowLabel::open_at(Sensitivity::Public));
    let projection = project_for(
        &recipient,
        &items(),
        ResidualPolicy::new(Residual::Silent),
    );
    assert_eq!(projection.released.len(), 1);
    assert_eq!(projection.withheld_count(), 2);
    assert!(projection.withheld.is_empty());
    assert!(!projection.recipient_can_detect_omission());
}

#[test]
fn disclosing_the_reason_for_a_withholding_is_itself_a_channel_and_is_recorded_as_one() {
    let recipient = Principal::new("reader", FlowLabel::open_at(Sensitivity::Public));
    let projection = project_for(
        &recipient,
        &items(),
        ResidualPolicy::new(Residual::ReasonDisclosed),
    );
    assert_eq!(projection.withheld.len(), 2);
    assert!(projection.recipient_can_detect_omission());
    assert!(projection.withheld[0].shape.is_some());
    assert!(projection.withheld[0].refusals.is_some());
}

#[test]
fn what_one_participant_may_learn_from_another_is_not_symmetric() {
    let all = items();
    let investigator = Principal::new("investigator", restricted());
    let reporter = Principal::new("reporter", FlowLabel::open_at(Sensitivity::Public));
    let matrix = DisclosureMatrix::compute(&[
        (investigator, all.clone()),
        (reporter, vec![all[0].clone()]),
    ]);
    assert_eq!(
        matrix.may_learn("investigator", "reporter"),
        ["public-summary".to_string()]
    );
    assert_eq!(
        matrix.may_learn("reporter", "investigator"),
        ["public-summary".to_string()]
    );
    assert!(matrix.is_symmetric_for("investigator", "reporter"));
}

#[test]
fn a_holder_cleared_for_something_it_does_not_hold_can_disclose_nothing_about_it() {
    let all = items();
    let investigator = Principal::new("investigator", restricted());
    let auditor = Principal::new("auditor", restricted());
    let matrix = DisclosureMatrix::compute(&[
        (investigator, all.clone()),
        (auditor, Vec::new()),
    ]);
    assert!(matrix
        .may_learn("investigator", "auditor")
        .contains(&"patient-record".to_string()));
    assert!(matrix.may_learn("auditor", "investigator").is_empty());
    assert!(!matrix.is_symmetric_for("investigator", "auditor"));
}

#[test]
fn a_holder_may_not_pass_on_an_item_it_is_not_itself_cleared_to_read() {
    let all = items();
    let courier = Principal::new("courier", FlowLabel::open_at(Sensitivity::Public));
    let investigator = Principal::new("investigator", restricted());
    let matrix =
        DisclosureMatrix::compute(&[(courier, all.clone()), (investigator, Vec::new())]);
    assert_eq!(
        matrix.may_learn("courier", "investigator"),
        ["public-summary".to_string()]
    );
}

#[test]
fn rendering_a_flow_label_into_the_kernels_flat_clearance_set_reports_what_it_loses() {
    let principal = Principal::new("investigator", restricted());
    let clearance = principal.capsule_clearance();
    assert!(clearance.contains("compartment:patient-42"));
    let loss = principal.capsule_clearance_loss();
    assert!(loss.contains(&"purpose-restriction"));
    assert!(loss.contains(&"residency"));
    assert!(loss.contains(&"retention"));
}

#[test]
fn an_uncleared_principal_renders_an_empty_clearance_and_says_the_clearance_is_absent() {
    let principal = Principal::uncleared("stranger");
    assert!(principal.capsule_clearance().is_empty());
    assert_eq!(principal.capsule_clearance_loss(), vec!["clearance-absent"]);
}
