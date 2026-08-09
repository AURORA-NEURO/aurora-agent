//! 23.25: the capability rule, composition checks, provider fidelity, and supply-chain admission.

use bioprism_fabric::effect::{Effect, EffectKind, EffectSet, Scope};
use bioprism_fabric::flow::Sensitivity;
use bioprism_fabric::molecule::Version;
use bioprism_interweave::component::{
    admit, compose, permits, Admission, CheckOutcome, ComponentContract, ComponentRole,
    CompositionCheck, Denial, Determinism, HostService, Interface, IsolationFidelity,
    PackageManifest, Provider, SupplyChainControl,
};
use std::collections::BTreeSet;

fn deterministic() -> Determinism {
    Determinism::Deterministic {
        pinned_inputs: BTreeSet::from(["schema@1.0.0".to_string()]),
        content_addressed_outputs: true,
    }
}

fn scorer(version: Version) -> Interface {
    Interface::new("aurora:weave-evidence/scorer", version)
}

#[test]
fn a_component_declaring_filesystem_write_without_a_filesystem_import_is_refused() {
    let contract = ComponentContract::new(
        "transform",
        ComponentRole::DataTransform,
        Provider::WasmComponent,
        deterministic(),
    )
    .with_effects(EffectSet::new().with(Effect::new(
        EffectKind::FilesystemWrite,
        Scope::Undeclared,
    )));
    assert_eq!(
        permits(&contract),
        vec![Denial::MissingHostImport {
            component: "transform".into(),
            kind: EffectKind::FilesystemWrite,
            service: HostService::RestrictedFilesystem,
        }]
    );
}

#[test]
fn the_same_component_with_the_import_granted_is_permitted() {
    let contract = ComponentContract::new(
        "transform",
        ComponentRole::DataTransform,
        Provider::WasmComponent,
        deterministic(),
    )
    .with_host(HostService::RestrictedFilesystem)
    .with_effects(EffectSet::new().with(Effect::new(
        EffectKind::FilesystemWrite,
        Scope::Undeclared,
    )));
    assert!(permits(&contract).is_empty());
}

#[test]
fn a_pure_component_needs_no_host_import_at_all() {
    let contract = ComponentContract::new(
        "parser",
        ComponentRole::Parser,
        Provider::WasmComponent,
        deterministic(),
    )
    .with_effects(EffectSet::new().with(Effect::new(EffectKind::PureCompute, Scope::Undeclared)));
    assert!(permits(&contract).is_empty());
}

#[test]
fn every_denial_is_reported_rather_than_only_the_first() {
    let contract = ComponentContract::new(
        "greedy",
        ComponentRole::LocalSpecialistModel,
        Provider::ContainerSandbox,
        Determinism::Nondeterministic {
            reason: "remote model".into(),
        },
    )
    .with_effects(
        EffectSet::new()
            .with(Effect::new(EffectKind::FilesystemRead, Scope::Undeclared))
            .with(Effect::new(EffectKind::NetworkWrite, Scope::Undeclared))
            .with(Effect::new(EffectKind::SecretUse, Scope::Undeclared)),
    );
    assert_eq!(permits(&contract).len(), 3);
}

#[test]
fn a_component_claiming_determinism_while_importing_a_clock_contradicts_itself() {
    let contract = ComponentContract::new(
        "scorer",
        ComponentRole::ContextScorer,
        Provider::WasmComponent,
        deterministic(),
    )
    .with_host(HostService::ClocksAndRandomness);
    assert_eq!(
        permits(&contract),
        vec![Denial::DeterminismContradictedByImport {
            component: "scorer".into(),
            service: HostService::ClocksAndRandomness,
        }]
    );
}

#[test]
fn a_deterministic_component_without_content_addressed_outputs_is_not_replayable() {
    let determinism = Determinism::Deterministic {
        pinned_inputs: BTreeSet::new(),
        content_addressed_outputs: false,
    };
    assert!(!determinism.replayable());
    assert!(deterministic().replayable());
    assert!(!Determinism::Nondeterministic {
        reason: "model".into()
    }
    .replayable());
}

#[test]
fn every_effect_kind_that_touches_a_host_resource_names_the_import_it_needs() {
    for kind in EffectKind::TAXONOMY {
        let required = HostService::required_for(kind);
        let touches_host = !matches!(
            kind,
            EffectKind::PureCompute
                | EffectKind::AgentSpawn
                | EffectKind::AgentDelegate
                | EffectKind::PolicyChange
                | EffectKind::ClinicalOutput
                | EffectKind::IrreversibleEffect
        );
        assert_eq!(required.is_some(), touches_host, "{kind:?}");
    }
}

fn producer() -> ComponentContract {
    ComponentContract::new(
        "producer",
        ComponentRole::DeterministicEvaluator,
        Provider::WasmComponent,
        deterministic(),
    )
    .exporting(scorer(Version::new(0, 2, 0)))
    .with_effects(EffectSet::new().with(Effect::new(EffectKind::ArtifactRead, Scope::Undeclared)))
    .cleared_to(Sensitivity::Internal)
}

fn consumer() -> ComponentContract {
    ComponentContract::new(
        "consumer",
        ComponentRole::ArtifactValidator,
        Provider::WasmComponent,
        deterministic(),
    )
    .importing(scorer(Version::new(0, 2, 0)))
    .with_effects(EffectSet::new().with(Effect::new(EffectKind::ArtifactRead, Scope::Undeclared)))
    .cleared_to(Sensitivity::Internal)
}

#[test]
fn a_compatible_pair_still_leaves_two_checks_undecided_rather_than_passing_them() {
    let report = compose(&producer(), &consumer());
    assert!(report.failing().is_empty());
    assert_eq!(
        report.undecided(),
        BTreeSet::from([
            CompositionCheck::AsyncAndStreamBehaviour,
            CompositionCheck::ResourceOwnership,
        ])
    );
    assert!(!report.admits());
}

#[test]
fn an_undecided_check_is_not_a_holding_one() {
    let report = compose(&producer(), &consumer());
    assert!(!report
        .outcomes
        .get(&CompositionCheck::ResourceOwnership)
        .expect("check present")
        .holds());
    assert!(report
        .outcomes
        .get(&CompositionCheck::TypeCompatibility)
        .expect("check present")
        .holds());
}

#[test]
fn a_consumer_importing_an_interface_the_producer_does_not_export_fails_type_compatibility() {
    let consumer = ComponentContract::new(
        "consumer",
        ComponentRole::ArtifactValidator,
        Provider::WasmComponent,
        deterministic(),
    )
    .importing(Interface::new("aurora:weave/other", Version::new(1, 0, 0)));
    let report = compose(&producer(), &consumer);
    assert!(report.failing().contains(&CompositionCheck::TypeCompatibility));
}

#[test]
fn an_older_minor_version_on_the_producer_side_fails_version_constraints() {
    let older = ComponentContract::new(
        "producer",
        ComponentRole::DeterministicEvaluator,
        Provider::WasmComponent,
        deterministic(),
    )
    .exporting(scorer(Version::new(0, 1, 0)));
    let report = compose(&older, &consumer());
    assert!(report.failing().contains(&CompositionCheck::VersionConstraints));
    assert!(!report.failing().contains(&CompositionCheck::TypeCompatibility));
}

#[test]
fn a_consumer_performing_an_effect_outside_the_producer_envelope_fails_effect_compatibility() {
    let escaping = consumer().with_effects(
        EffectSet::new().with(Effect::new(EffectKind::NetworkWrite, Scope::Undeclared)),
    );
    let report = compose(&producer(), &escaping);
    assert!(report.failing().contains(&CompositionCheck::EffectCompatibility));
}

#[test]
fn a_consumer_cleared_below_the_producer_fails_security_labels() {
    let under_cleared = consumer().cleared_to(Sensitivity::Public);
    let report = compose(&producer(), &under_cleared);
    assert!(report.failing().contains(&CompositionCheck::SecurityLabels));
}

#[test]
fn remote_providers_declare_the_weakest_isolation_fidelity() {
    assert_eq!(
        Provider::RemoteService.fidelity(),
        IsolationFidelity::Declared
    );
    assert_eq!(Provider::A2AAgent.fidelity(), IsolationFidelity::Declared);
    assert_eq!(Provider::McpServer.fidelity(), IsolationFidelity::Declared);
    assert!(Provider::WasmComponent.fidelity() > IsolationFidelity::Declared);
}

#[test]
fn wasm_is_the_only_provider_at_capability_scoped_fidelity() {
    let capability_scoped: Vec<Provider> = Provider::ALL
        .into_iter()
        .filter(|p| p.fidelity() == IsolationFidelity::CapabilityScoped)
        .collect();
    assert_eq!(capability_scoped, vec![Provider::WasmComponent]);
}

#[test]
fn a_package_missing_an_unconditional_control_is_refused() {
    let manifest = PackageManifest {
        component: "scorer".into(),
        satisfied: SupplyChainControl::ALL
            .into_iter()
            .filter(|c| *c != SupplyChainControl::DependencyLock)
            .collect(),
    };
    assert_eq!(
        admit(&manifest),
        Admission::Refused {
            unmet: BTreeSet::from([SupplyChainControl::DependencyLock])
        }
    );
}

#[test]
fn a_package_missing_only_the_qualified_control_is_admitted_with_the_gap_named() {
    let manifest = PackageManifest {
        component: "scorer".into(),
        satisfied: SupplyChainControl::ALL
            .into_iter()
            .filter(|c| *c != SupplyChainControl::ReproducibleBuild)
            .collect(),
    };
    assert_eq!(
        admit(&manifest),
        Admission::AdmittedWithQualifiedGap {
            unmet: BTreeSet::from([SupplyChainControl::ReproducibleBuild])
        }
    );
}

#[test]
fn a_complete_manifest_is_admitted_outright() {
    let manifest = PackageManifest {
        component: "scorer".into(),
        satisfied: SupplyChainControl::ALL.into_iter().collect(),
    };
    assert_eq!(admit(&manifest), Admission::Admitted);
}

#[test]
fn reproducible_build_is_the_only_qualified_supply_chain_control() {
    let qualified: Vec<SupplyChainControl> = SupplyChainControl::ALL
        .into_iter()
        .filter(|c| !c.unconditional())
        .collect();
    assert_eq!(qualified, vec![SupplyChainControl::ReproducibleBuild]);
}

#[test]
fn a_composition_report_names_its_two_participants_so_a_failure_can_be_attributed() {
    let report = compose(&producer(), &consumer());
    assert_eq!(report.producer, "producer");
    assert_eq!(report.consumer, "consumer");
    assert_eq!(report.outcomes.len(), CompositionCheck::ALL.len());
    assert!(matches!(
        report
            .outcomes
            .get(&CompositionCheck::AsyncAndStreamBehaviour)
            .expect("present"),
        CheckOutcome::Undecided { .. }
    ));
}
