//! 23.31: the stability ladder, dimensional claims, version pinning, namespaces, compatibility.

use bioprism_fabric::molecule::Version;
use bioprism_interweave::conformance::{
    check_change, core_act_named, ChangeRefusal, ConformanceClaim, Dimension, ExtensionRegistry,
    FamilyState, GovernanceError, Guarantee, MigrationRefusal, NegotiatedProfile, PinnedVersions,
    ProposedChange, ProtocolTransition, SpecFamily, Stability, StateMapping, UnmetPrerequisite,
    PROPOSAL_SECTIONS, TEST_SUITES,
};
use std::collections::BTreeSet;

#[test]
fn retired_is_terminal_so_a_published_result_cannot_be_reinterpreted_later() {
    for next in Stability::ALL {
        assert!(
            !Stability::Retired.may_transition_to(next),
            "retired must not reach {next:?}"
        );
    }
}

#[test]
fn a_candidate_may_unfreeze_back_to_experimental_but_a_stable_family_may_not() {
    assert!(Stability::Candidate.may_transition_to(Stability::Experimental));
    assert!(!Stability::Stable.may_transition_to(Stability::Experimental));
    assert!(!Stability::Stable.may_transition_to(Stability::Candidate));
}

#[test]
fn legacy_is_reachable_only_from_stable() {
    let sources: Vec<Stability> = Stability::ALL
        .into_iter()
        .filter(|from| from.may_transition_to(Stability::Legacy))
        .collect();
    assert_eq!(sources, vec![Stability::Stable]);
}

#[test]
fn no_level_may_transition_to_itself() {
    for level in Stability::ALL {
        assert!(!level.may_transition_to(level), "{level:?} loops");
    }
}

#[test]
fn a_family_transition_the_ladder_forbids_is_refused_with_both_endpoints_named() {
    let state = FamilyState::new(
        SpecFamily::CommunicativeActs,
        Version::new(1, 0, 0),
        Stability::Stable,
    );
    let refusal = state.transition(Stability::Experimental).unwrap_err();
    assert_eq!(refusal.family, SpecFamily::CommunicativeActs);
    assert_eq!(refusal.from, Stability::Stable);
    assert_eq!(refusal.to, Stability::Experimental);
}

#[test]
fn a_permitted_transition_preserves_the_version_and_changes_only_the_stability() {
    let state = FamilyState::new(
        SpecFamily::AdapterProfiles,
        Version::new(0, 3, 1),
        Stability::Candidate,
    );
    let promoted = state.transition(Stability::Stable).expect("permitted");
    assert_eq!(promoted.version, Version::new(0, 3, 1));
    assert_eq!(promoted.stability, Stability::Stable);
}

#[test]
fn the_three_levels_that_promise_compatibility_are_stable_legacy_and_deprecated() {
    let promising: Vec<Stability> = Stability::ALL
        .into_iter()
        .filter(|s| s.promises_compatibility())
        .collect();
    assert_eq!(
        promising,
        vec![Stability::Stable, Stability::Legacy, Stability::Deprecated]
    );
}

#[test]
fn a_c1_through_c4_claim_covers_neither_continuations_nor_authority() {
    let claim = ConformanceClaim::through("impl", Dimension::C4);
    assert!(claim.covers(Dimension::C4));
    assert!(!claim.covers(Dimension::C6));
    assert!(!claim.covers(Dimension::C9));
    assert!(claim.is_prefix());
    assert!(claim.unmet_prerequisites().is_empty());
}

#[test]
fn a_claim_of_continuations_without_capsules_reports_the_prerequisite_without_rejecting_it() {
    let claim = ConformanceClaim::new("impl")
        .claiming(Dimension::C3)
        .claiming(Dimension::C5)
        .claiming(Dimension::C9);
    let unmet = claim.unmet_prerequisites();
    assert!(unmet.contains(&UnmetPrerequisite {
        claimed: Dimension::C9,
        requires: Dimension::C8,
    }));
    assert!(claim.covers(Dimension::C9));
}

#[test]
fn a_gapped_claim_is_not_a_prefix() {
    let claim = ConformanceClaim::new("impl")
        .claiming(Dimension::C1)
        .claiming(Dimension::C4);
    assert!(!claim.is_prefix());
}

#[test]
fn an_empty_claim_is_vacuously_a_prefix_and_claims_nothing() {
    let claim = ConformanceClaim::new("impl");
    assert!(claim.is_prefix());
    assert!(claim.claimed().is_empty());
    for dimension in Dimension::ALL {
        assert!(!claim.covers(dimension));
    }
}

#[test]
fn the_prerequisite_relation_is_acyclic_when_followed_from_every_dimension() {
    for start in Dimension::ALL {
        let mut frontier = vec![start];
        let mut seen: BTreeSet<Dimension> = BTreeSet::new();
        while let Some(current) = frontier.pop() {
            for required in current.prerequisites() {
                assert_ne!(*required, start, "{start:?} is its own prerequisite");
                if seen.insert(*required) {
                    frontier.push(*required);
                }
            }
        }
    }
}

#[test]
fn every_dimension_carries_the_blueprints_label() {
    assert_eq!(Dimension::ALL.len(), 12);
    for dimension in Dimension::ALL {
        assert!(!dimension.label().is_empty());
    }
}

fn profile(major: u32) -> NegotiatedProfile {
    NegotiatedProfile {
        weave_ir_major: major,
        act_packages: BTreeSet::from(["core".to_string()]),
        type_and_ontology_packages: BTreeSet::new(),
        security_profile: "default".into(),
        replay_profile: "strict".into(),
        adapter_profile: "a2a".into(),
    }
}

#[test]
fn a_running_thread_cannot_migrate_without_a_mapping_for_every_live_state() {
    let pinned = PinnedVersions::pin("thread-1", profile(1));
    let transition = ProtocolTransition {
        reason: "act package v2".into(),
        mappings: vec![StateMapping {
            from_state: "awaiting-accept".into(),
            to_state: "awaiting-accept".into(),
        }],
    };
    let live = BTreeSet::from(["awaiting-accept".to_string(), "open-commitment".to_string()]);
    assert_eq!(
        pinned.migrate(profile(2), &transition, &live),
        Err(MigrationRefusal::UnmappedState {
            state: "open-commitment".into()
        })
    );
}

#[test]
fn a_migration_with_every_state_mapped_repins_the_thread() {
    let pinned = PinnedVersions::pin("thread-1", profile(1));
    let transition = ProtocolTransition {
        reason: "act package v2".into(),
        mappings: vec![StateMapping {
            from_state: "awaiting-accept".into(),
            to_state: "awaiting-acceptance".into(),
        }],
    };
    let live = BTreeSet::from(["awaiting-accept".to_string()]);
    let migrated = pinned
        .migrate(profile(2), &transition, &live)
        .expect("mapped");
    assert_eq!(migrated.profile().weave_ir_major, 2);
    assert_eq!(migrated.thread, "thread-1");
}

#[test]
fn a_migration_with_no_stated_reason_is_refused_even_when_every_state_maps() {
    let pinned = PinnedVersions::pin("thread-1", profile(1));
    let transition = ProtocolTransition {
        reason: "  ".into(),
        mappings: Vec::new(),
    };
    assert_eq!(
        pinned.migrate(profile(2), &transition, &BTreeSet::new()),
        Err(MigrationRefusal::NoReason)
    );
}

#[test]
fn an_extension_cannot_redefine_a_core_act_under_another_meaning() {
    let mut registry = ExtensionRegistry::new();
    let error = registry.register("org.example.challenge", "example").unwrap_err();
    assert_eq!(
        error,
        GovernanceError::RedefinesCoreAct {
            name: "org.example.challenge".into(),
            act: "challenge".into(),
        }
    );
}

#[test]
fn the_reserved_act_names_come_from_the_kernels_own_vocabulary() {
    assert_eq!(core_act_named("attest"), Some("attest"));
    assert_eq!(core_act_named("revoke"), Some("revoke"));
    assert_eq!(core_act_named("summarise"), None);
}

#[test]
fn an_unnamespaced_extension_is_refused() {
    let mut registry = ExtensionRegistry::new();
    assert_eq!(
        registry.register("summarise", "example"),
        Err(GovernanceError::UnnamespacedExtension {
            name: "summarise".into()
        })
    );
    assert!(registry.is_empty());
}

#[test]
fn a_reverse_domain_extension_that_shadows_nothing_registers() {
    let mut registry = ExtensionRegistry::new();
    assert!(registry.register("org.example.summarise", "example").is_ok());
    assert_eq!(registry.owner("org.example.summarise"), Some("example"));
    assert_eq!(registry.len(), 1);
}

#[test]
fn a_registry_owned_namespace_is_accepted_without_a_reverse_domain() {
    let mut registry = ExtensionRegistry::new();
    assert!(registry.register("weave.ext.escalate", "registry").is_ok());
}

#[test]
fn registering_the_same_name_twice_names_the_existing_owner() {
    let mut registry = ExtensionRegistry::new();
    registry
        .register("org.example.summarise", "example")
        .expect("first registration succeeds");
    assert_eq!(
        registry.register("org.example.summarise", "other"),
        Err(GovernanceError::AlreadyRegistered {
            name: "org.example.summarise".into(),
            owner: "example".into(),
        })
    );
}

#[test]
fn a_change_that_would_rewrite_published_results_is_refused_even_with_a_major_bump() {
    let state = FamilyState::new(
        SpecFamily::PrismInstrumentationProfile,
        Version::new(1, 0, 0),
        Stability::Stable,
    );
    let change = ProposedChange {
        family: SpecFamily::PrismInstrumentationProfile,
        from: Version::new(1, 0, 0),
        to: Version::new(2, 0, 0),
        breaks: BTreeSet::from([Guarantee::PublishedResultSemantics]),
    };
    assert_eq!(
        check_change(&state, &change),
        Err(ChangeRefusal::RewritesPublishedResults {
            family: SpecFamily::PrismInstrumentationProfile
        })
    );
}

#[test]
fn a_breaking_change_inside_a_major_version_is_refused_on_a_stable_family() {
    let state = FamilyState::new(
        SpecFamily::CommunicativeActs,
        Version::new(1, 0, 0),
        Stability::Stable,
    );
    let change = ProposedChange {
        family: SpecFamily::CommunicativeActs,
        from: Version::new(1, 0, 0),
        to: Version::new(1, 1, 0),
        breaks: BTreeSet::from([Guarantee::CoreActConsequences]),
    };
    assert!(matches!(
        check_change(&state, &change),
        Err(ChangeRefusal::BreaksWithinMajor { .. })
    ));
}

#[test]
fn the_same_breaking_change_is_permitted_across_a_major_boundary() {
    let state = FamilyState::new(
        SpecFamily::CommunicativeActs,
        Version::new(1, 0, 0),
        Stability::Stable,
    );
    let change = ProposedChange {
        family: SpecFamily::CommunicativeActs,
        from: Version::new(1, 0, 0),
        to: Version::new(2, 0, 0),
        breaks: BTreeSet::from([Guarantee::CoreActConsequences]),
    };
    assert!(check_change(&state, &change).is_ok());
}

#[test]
fn an_experimental_family_may_break_anything_except_published_results() {
    let state = FamilyState::new(
        SpecFamily::WeaveLangSyntaxAndSemantics,
        Version::new(0, 1, 0),
        Stability::Experimental,
    );
    let change = ProposedChange {
        family: SpecFamily::WeaveLangSyntaxAndSemantics,
        from: Version::new(0, 1, 0),
        to: Version::new(0, 2, 0),
        breaks: BTreeSet::from([
            Guarantee::CoreActConsequences,
            Guarantee::ReplayOfHistoricalBundles,
        ]),
    };
    assert!(check_change(&state, &change).is_ok());
}

#[test]
fn the_eleven_specification_families_and_their_checklists_are_recorded_in_full() {
    assert_eq!(SpecFamily::ALL.len(), 11);
    assert_eq!(PROPOSAL_SECTIONS.len(), 10);
    assert_eq!(TEST_SUITES.len(), 11);
    assert_eq!(Guarantee::ALL.len(), 5);
}
