//! One host, several plugins, exercised only through the public API.
//!
//! The unit tests check one rule each. These check that the rules compose into the story 40.16
//! describes: a set of manifests arrives, some are refused, the survivors are ranked, and what the
//! host will actually *use* for a load-bearing result is narrower than what it registered.

use bioprism_sdk::{
    negotiate, AbiGrade, Capability, CapabilityCard, CapabilityKind, ConformanceEvidence,
    Determinism, Effect, EffectClass, EffectPolicy, HostAsk, IsolationClass, PluginId,
    PluginManifest, PluginRegistry, Priority, RegistrationError, RegistryPolicy,
    SandboxDeclaration, SchemaVersion, SelectionError, SemanticLossDeclaration, TrustLevel,
    VersionSet,
};

fn workspace_versions() -> VersionSet {
    RegistryPolicy::workspace_versions()
}

/// An adapter with a loss declaration and an independent conformance run against its exact content.
fn reviewed_adapter() -> PluginManifest {
    let base = PluginManifest::new("cbioportal-adapter", "2.1.0", "acme-bio")
        .speaking(workspace_versions())
        .providing(
            Capability::new(CapabilityKind::Adapter)
                .at(Priority(300))
                .handling(["text/csv", "application/x-cbioportal"]),
        )
        .claiming(Determinism::Deterministic)
        .needing(Effect::FilesystemRead {
            root: "/data/cbioportal".into(),
        })
        .at_grade(AbiGrade::P1)
        .requesting(SandboxDeclaration::requesting(
            IsolationClass::RestrictedProcess,
        ))
        .declaring_loss(SemanticLossDeclaration::admitting([
            "unmapped_column",
            "ontology_term_unmapped",
        ]));
    let core = base.core_digest().expect("digestible");
    base.attesting(
        ConformanceEvidence::new("adapter/normalize", CapabilityKind::Adapter, "bioprism-wg")
            .passing(41)
            .covering_core(core),
    )
}

/// An oracle whose only evidence is its own.
fn self_attested_oracle() -> PluginManifest {
    let base = PluginManifest::new("schema-oracle", "0.4.0", "acme-bio")
        .speaking(workspace_versions())
        .providing(Capability::new(CapabilityKind::Oracle).at(Priority(120)))
        .claiming(Determinism::Deterministic)
        .at_grade(AbiGrade::P3);
    let core = base.core_digest().expect("digestible");
    base.attesting(
        ConformanceEvidence::new("oracle/schema", CapabilityKind::Oracle, "acme-bio")
            .passing(18)
            .covering_core(core),
    )
}

/// A strategy nobody has checked, registered at the loudest priority in the set.
fn unverified_strategy(priority: Priority) -> PluginManifest {
    PluginManifest::new("greedy-retriever", "0.1.0", "someone")
        .speaking(workspace_versions())
        .providing(Capability::new(CapabilityKind::ContextStrategy).at(priority))
}

fn host() -> RegistryPolicy {
    RegistryPolicy::default()
        .granting(EffectPolicy::deny_all().granting_class(EffectClass::FilesystemRead))
}

#[test]
fn a_registered_set_resolves_to_one_winner_per_capability() {
    let registry = PluginRegistry::from_manifests(
        host(),
        [
            reviewed_adapter(),
            self_attested_oracle(),
            unverified_strategy(Priority(900)),
        ],
    )
    .expect("a clean set registers");

    let resolution = registry.resolution();
    assert_eq!(resolution.len(), 3);
    assert_eq!(
        resolution[&CapabilityKind::Adapter],
        PluginId::new("cbioportal-adapter", "2.1.0")
    );
    assert!(!resolution.contains_key(&CapabilityKind::QueryBackend));
}

#[test]
fn every_permutation_of_one_clean_set_yields_the_same_resolution_and_the_same_cards() {
    let manifests = [
        reviewed_adapter(),
        self_attested_oracle(),
        unverified_strategy(Priority(900)),
    ];

    let mut baseline = None;
    for rotation in 0..manifests.len() {
        let mut order = manifests.to_vec();
        order.rotate_left(rotation);
        order.reverse();
        let registry = PluginRegistry::from_manifests(host(), order).expect("registers");
        let observed = (registry.resolution(), registry.capability_cards());
        match &baseline {
            None => baseline = Some(observed),
            Some(expected) => assert_eq!(&observed, expected),
        }
    }
}

#[test]
fn an_unverified_plugin_wins_resolution_and_loses_load_bearing_selection() {
    let registry = PluginRegistry::from_manifests(
        host(),
        [self_attested_oracle(), unverified_strategy(Priority(900))],
    )
    .expect("registers");

    assert_eq!(
        registry
            .resolve(CapabilityKind::ContextStrategy)
            .expect("resolves")
            .id(),
        PluginId::new("greedy-retriever", "0.1.0")
    );
    assert!(matches!(
        registry.resolve_load_bearing(CapabilityKind::ContextStrategy),
        Err(SelectionError::NotSelectableForLoadBearing { .. })
    ));
    assert_eq!(
        registry
            .resolve_load_bearing(CapabilityKind::Oracle)
            .expect("the self-attested oracle clears the default floor")
            .trust
            .level,
        TrustLevel::SelfAttested
    );
}

#[test]
fn raising_the_trust_floor_narrows_what_the_same_set_may_be_used_for() {
    let strict = host().with_load_bearing_floor(TrustLevel::IndependentlyVerified);
    let registry =
        PluginRegistry::from_manifests(strict, [reviewed_adapter(), self_attested_oracle()])
            .expect("registers");

    assert!(registry
        .resolve_load_bearing(CapabilityKind::Adapter)
        .is_ok());
    match registry.resolve_load_bearing(CapabilityKind::Oracle) {
        Err(SelectionError::NotSelectableForLoadBearing { trust, reasons, .. }) => {
            assert_eq!(trust, "self-attested");
            assert!(reasons.contains("independent"), "{reasons}");
        }
        other => panic!("expected the self-attested oracle to be refused, got {other:?}"),
    }
}

#[test]
fn a_host_that_grants_nothing_refuses_a_plugin_that_needs_to_read_a_file() {
    let mut registry = PluginRegistry::new(RegistryPolicy::default());
    match registry.register(reviewed_adapter()) {
        Err(RegistrationError::EffectNotPermitted { effect, plugin }) => {
            assert!(effect.contains("/data/cbioportal"), "{effect}");
            assert!(plugin.contains("cbioportal-adapter"), "{plugin}");
        }
        other => panic!("expected an effect refusal, got {other:?}"),
    }
    assert!(registry.is_empty());
}

#[test]
fn the_abi_grade_a_plugin_declared_bounds_what_it_may_be_asked_after_registration() {
    let registry =
        PluginRegistry::from_manifests(host(), [reviewed_adapter(), self_attested_oracle()])
            .expect("registers");

    let adapter = PluginId::new("cbioportal-adapter", "2.1.0");
    let oracle = PluginId::new("schema-oracle", "0.4.0");

    assert!(registry
        .ask(&adapter, HostAsk::UseArtifactsAndEffects)
        .is_ok());
    match registry.ask(&adapter, HostAsk::ResumeContinuation) {
        Err(SelectionError::AbiGradeTooLow {
            declared, required, ..
        }) => {
            assert_eq!(declared, "ABI-P1");
            assert_eq!(required, "ABI-P3");
        }
        other => panic!("expected a grade refusal, got {other:?}"),
    }
    assert!(registry.ask(&oracle, HostAsk::ResumeContinuation).is_ok());
    assert!(matches!(
        registry.ask(&oracle, HostAsk::BindRoleDynamically),
        Err(SelectionError::AbiGradeTooLow { .. })
    ));
}

#[test]
fn a_capability_card_set_round_trips_through_json_and_keeps_the_enforcement_caveat() {
    let registry = PluginRegistry::from_manifests(host(), [reviewed_adapter()]).expect("registers");
    let cards = registry.capability_cards();

    let json = serde_json::to_string(&cards).expect("serialises");
    assert!(json.contains("declared_only"), "{json}");
    let back: Vec<CapabilityCard> = serde_json::from_str(&json).expect("deserialises");
    assert_eq!(back, cards);

    let adapter = back
        .iter()
        .find(|card| card.kind == CapabilityKind::Adapter)
        .and_then(CapabilityCard::provider)
        .expect("the adapter card has a provider");
    assert_eq!(adapter.trust, TrustLevel::IndependentlyVerified);
    assert!(adapter.load_bearing);
    assert!(adapter
        .semantic_loss
        .as_ref()
        .expect("an adapter card carries its loss declaration")
        .kinds
        .contains("ontology_term_unmapped"));
}

#[test]
fn a_registration_survives_a_json_round_trip_with_its_digests_intact() {
    let registry = PluginRegistry::from_manifests(host(), [reviewed_adapter()]).expect("registers");
    let registration = registry
        .get(&PluginId::new("cbioportal-adapter", "2.1.0"))
        .expect("registered");

    let json = serde_json::to_string(registration).expect("serialises");
    let back: bioprism_sdk::Registration = serde_json::from_str(&json).expect("deserialises");
    assert_eq!(&back, registration);
    assert_eq!(
        back.manifest.digest().expect("digestible"),
        registration.digest
    );
    assert_eq!(
        back.manifest.core_digest().expect("digestible"),
        registration.core_digest
    );
}

#[test]
fn a_plugin_and_a_host_that_share_no_schema_version_never_reach_registration() {
    let plugin_versions =
        VersionSet::new().with(SchemaVersion::parse("fiber-decision-section/9.0").expect("parses"));
    let error =
        negotiate(&workspace_versions(), &plugin_versions).expect_err("no version in common");
    assert!(error.to_string().contains("fiber-decision-section/9.0"));

    let mut registry = PluginRegistry::new(host());
    let manifest = PluginManifest::new("from-the-future", "1.0.0", "acme-bio")
        .speaking(plugin_versions)
        .providing(Capability::new(CapabilityKind::Oracle));
    assert!(matches!(
        registry.register(manifest),
        Err(RegistrationError::Negotiation(_))
    ));
}

#[test]
fn withdrawing_the_only_adapter_makes_the_capability_unavailable_without_hiding_it() {
    let mut registry =
        PluginRegistry::from_manifests(host(), [reviewed_adapter()]).expect("registers");
    let id = PluginId::new("cbioportal-adapter", "2.1.0");
    registry
        .revoke(
            &id,
            "declared loss surface did not match the emitted report",
        )
        .expect("revokes");

    assert!(matches!(
        registry.resolve_subject(CapabilityKind::Adapter, "text/csv"),
        Err(SelectionError::NoProviderFor { .. })
    ));
    let cards = registry.capability_cards();
    assert_eq!(cards.len(), CapabilityKind::ALL.len());
    assert!(cards.iter().all(|card| !card.is_available()));
    assert_eq!(registry.revocation_history().len(), 1);
    assert!(registry.revocation_history()[0]
        .reason
        .contains("did not match"));
}
