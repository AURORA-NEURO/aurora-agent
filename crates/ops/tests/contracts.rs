//! Invariants that span more than one module of `bioprism-ops`, and one that spans two crates.
//!
//! The unit tests hold each module against its own contract. These hold the joins, which is where a
//! crate built module by module usually stops being coherent: the identity of a run is decided by
//! `config` and `flags` together, the honesty of a report is decided by `alpha` and `error`
//! together, and the claim about how much of the section is template is decided here and in
//! `bioprism-services` independently.

use bioprism_infra::Epoch;
use bioprism_ops::alpha::{self, Criterion, Verdict};
use bioprism_ops::capacity::{
    ArtifactHandling, Assumption, Bound, CapacityModel, Concession, DegradationPlan, Demand,
    Operation, Workload,
};
use bioprism_ops::config::{
    Binding, ConfigStack, DeploymentProfile, EffectiveConfig, Influence, Layer, Schema, SecretRef,
    SecretSource, SettingKey, SettingSpec, SettingValue, Source, ValueType,
};
use bioprism_ops::error::OpsError;
use bioprism_ops::flags::{Flag, FlagId, FlagRegistry, PinnedRun};
use bioprism_ops::hardening::{audit_credentials, coverage, Effect, EffectDeclaration};
use bioprism_ops::telemetry::{
    Derivation, DomainEvent, Field, MetricDefinition, Observations, RedactionPolicy, Sample,
    SignalId, TraceId, Treatment,
};
use bioprism_scope::ScopeClass;
use bioprism_services::Invalidates;
use serde_json::Value;

fn key(name: &str) -> SettingKey {
    SettingKey::parse(name).expect("well-formed")
}

fn flag_id(name: &str) -> FlagId {
    FlagId::parse(name).expect("well-formed")
}

fn schema() -> Schema {
    Schema::new()
        .with(SettingSpec::value(
            key("compile.max_hops"),
            ValueType::Integer,
            Influence::Emitted,
        ))
        .unwrap()
        .with(SettingSpec::value(
            key("store.root"),
            ValueType::Text,
            Influence::Operational,
        ))
        .unwrap()
        .with(SettingSpec::secret(key("hub.token")))
        .unwrap()
}

fn defaults() -> Source {
    Source::new(Layer::Defaults, "built-in")
        .bind(
            key("compile.max_hops"),
            Binding::Value(SettingValue::Integer(3)),
        )
        .bind(
            key("store.root"),
            Binding::Value(SettingValue::Text("./.bioprism".into())),
        )
}

fn registry() -> FlagRegistry {
    FlagRegistry::new()
        .with(Flag::toggle(flag_id("log.verbose"), false, "40.10"))
        .unwrap()
        .with(Flag::variant(flag_id("compile.ranking"), "ranking-v2", false, "40.10").unwrap())
        .unwrap()
}

/// The pair that identifies what a run computed, as opposed to how it was operated.
fn computation_identity(config: &EffectiveConfig, registry: &FlagRegistry) -> (String, String) {
    let pin = registry.pin([]).expect("pins");
    (
        config.emitted_fingerprint().expect("hashes").to_string(),
        pin.digest().to_string(),
    )
}

#[test]
fn two_runs_differing_only_in_how_they_were_operated_have_the_same_computation_identity() {
    let local = DeploymentProfile::new("local").bind(
        key("store.root"),
        Binding::Value(SettingValue::Text("./local".into())),
    );
    let hosted = DeploymentProfile::new("hosted").bind(
        key("store.root"),
        Binding::Value(SettingValue::Text("s3://bucket".into())),
    );

    let under = |profile: &DeploymentProfile| {
        ConfigStack::new(schema())
            .push(defaults())
            .with_profile(profile)
            .resolve()
            .expect("resolves")
    };

    let a = under(&local);
    let b = under(&hosted);
    assert_ne!(
        a.fingerprint().unwrap(),
        b.fingerprint().unwrap(),
        "the two runs were operated differently and the redacted fingerprint must show it"
    );
    assert_eq!(
        computation_identity(&a, &registry()),
        computation_identity(&b, &registry()),
        "a deployment profile and an operational toggle change providers, not what was computed"
    );
}

#[test]
fn a_variant_flag_and_an_emitted_setting_each_move_the_computation_identity_alone() {
    let base = ConfigStack::new(schema())
        .push(defaults())
        .resolve()
        .unwrap();
    let baseline = computation_identity(&base, &registry());

    let deeper = ConfigStack::new(schema())
        .push(defaults())
        .push(Source::new(Layer::CommandLine, "--max-hops").bind(
            key("compile.max_hops"),
            Binding::Value(SettingValue::Integer(5)),
        ))
        .resolve()
        .unwrap();
    let by_config = computation_identity(&deeper, &registry());

    let rebranded = FlagRegistry::new()
        .with(Flag::toggle(flag_id("log.verbose"), false, "40.10"))
        .unwrap()
        .with(Flag::variant(flag_id("compile.ranking"), "ranking-v3", false, "40.10").unwrap())
        .unwrap();
    let by_flag = computation_identity(&base, &rebranded);

    assert_ne!(baseline.0, by_config.0);
    assert_eq!(baseline.1, by_config.1);
    assert_eq!(baseline.0, by_flag.0);
    assert_ne!(baseline.1, by_flag.1);
}

#[test]
fn every_failure_this_crate_can_raise_names_one_of_the_six_contracts_it_implements() {
    let cited = ["40.10", "40.34", "40.35", "40.38", "40.39", "40.42"];
    let samples = [
        OpsError::UnknownSetting {
            key: "k".into(),
            origin: "o".into(),
        },
        OpsError::ProfileChangesSemantics {
            profile: "p".into(),
            key: "k".into(),
        },
        OpsError::UnsupportedMetric {
            metric: "m".into(),
            missing: vec![],
        },
        OpsError::UnclassifiedEmission {
            policy: "v1".into(),
        },
        OpsError::UnboundedWorkload {
            workload: "w".into(),
            operation: "o".into(),
        },
        OpsError::AmbientCredential {
            reference: "env:t".into(),
        },
        OpsError::UndeclaredEffect {
            run: "r".into(),
            effect: "network:x".into(),
        },
        OpsError::AssertedAcceptance {
            criterion: "c".into(),
            basis: "author".into(),
        },
    ];
    for error in samples {
        assert!(
            cited.contains(&error.module_id()),
            "{error} named {}, which this crate does not implement",
            error.module_id()
        );
    }
}

#[test]
fn a_run_that_leases_its_secrets_and_declares_its_effects_passes_both_hardening_predicates() {
    let config = ConfigStack::new(schema())
        .push(defaults())
        .push(Source::new(Layer::Environment, "BIOPRISM_HUB_TOKEN").bind(
            key("hub.token"),
            Binding::Secret(SecretRef::new(SecretSource::Environment, "HUB_TOKEN").unwrap()),
        ))
        .resolve()
        .expect("resolves");

    assert!(
        !audit_credentials(&config, &[]).is_clean(),
        "an environment secret with no lease is ambient"
    );

    let lease = config
        .lease(&key("hub.token"), "publish-boundary", Epoch::new(1), 5)
        .expect("leases");
    assert!(audit_credentials(&config, std::slice::from_ref(&lease)).is_clean());

    let effects = EffectDeclaration::new("publish").declaring(Effect::Network {
        host: "hub.example".into(),
    });
    assert!(effects
        .permits(&Effect::Network {
            host: "hub.example".into()
        })
        .is_ok());
    assert!(effects
        .permits(&Effect::Filesystem {
            path: "/etc/shadow".into(),
            write: false
        })
        .is_err());
}

#[test]
fn the_hardening_table_and_the_failure_taxonomy_agree_on_which_invariants_are_decided_here() {
    let decided: Vec<&str> = coverage()
        .iter()
        .filter(|row| row.decided_here)
        .map(|row| row.invariant)
        .collect();
    assert_eq!(decided.len(), 2);

    let raised = [
        OpsError::AmbientCredential {
            reference: "env:t".into(),
        },
        OpsError::UndeclaredEffect {
            run: "r".into(),
            effect: "clock".into(),
        },
    ];
    for error in raised {
        assert_eq!(error.module_id(), "40.39");
    }
}

#[test]
fn no_telemetry_failure_can_invalidate_a_result_however_the_projection_fails() {
    let policy = RedactionPolicy::new("v1")
        .declare(ScopeClass::Policy, Treatment::Emit)
        .unwrap();
    let event = DomainEvent::new("evt-1", "compile.finished", 1).with_field(
        "subject",
        Field::new(Value::String("PT-1".into()), ScopeClass::Identity),
    );
    let error = policy
        .project(&event, TraceId::new("trace-a"))
        .expect_err("identity has no declared treatment");
    assert_eq!(error.invalidates(), Invalidates::Projection);

    let unsupported = MetricDefinition::new(
        "export_drop_rate",
        Derivation::Ratio {
            numerator: SignalId::parse("dropped").unwrap(),
            denominator: SignalId::parse("offered").unwrap(),
        },
        "ratio",
    )
    .unwrap()
    .evaluate(&Observations::new())
    .expect_err("nothing was observed");
    assert_eq!(unsupported.invalidates(), Invalidates::Projection);
}

#[test]
fn a_capacity_headline_and_a_telemetry_metric_both_refuse_to_stand_on_nothing() {
    let projection = CapacityModel::new(
        Assumption::assumed(
            "work_units_per_epoch",
            1000.0,
            "work-units/epoch",
            "sized by hand",
        )
        .unwrap(),
        1024,
    )
    .project(
        &Workload::new(
            "compile",
            Assumption::measured("calls_per_epoch", 10.0, "calls/epoch", "counter").unwrap(),
        )
        .unwrap()
        .with(
            Operation::new(
                "closure_walk",
                Bound::Bounded { steps: 4 },
                ArtifactHandling::Streamed,
                Assumption::measured("cost_per_step", 1.0, "work-units/step", "bench").unwrap(),
            )
            .unwrap(),
        ),
    )
    .expect("projects");
    assert!(!projection.utilisation().is_fully_measured());
    assert!(projection
        .utilisation()
        .to_string()
        .contains("work_units_per_epoch"));

    let metric = MetricDefinition::new(
        "queue_age",
        Derivation::Passthrough {
            signal: SignalId::parse("oldest_lease_age").unwrap(),
        },
        "epochs",
    )
    .unwrap();
    assert!(metric.evaluate(&Observations::new()).is_err());
    assert!(metric
        .evaluate(&Observations::new().record(Sample::observed(
            SignalId::parse("oldest_lease_age").unwrap(),
            4.0,
            "lease-table",
            1,
        )))
        .is_ok());
}

#[test]
fn saturation_concedes_only_from_a_closed_set_that_contains_no_evidence() {
    let projection = CapacityModel::new(
        Assumption::measured("work_units_per_epoch", 10.0, "work-units/epoch", "bench").unwrap(),
        1024,
    )
    .project(
        &Workload::new(
            "compile",
            Assumption::measured("calls_per_epoch", 100.0, "calls/epoch", "counter").unwrap(),
        )
        .unwrap()
        .with(
            Operation::new(
                "walk",
                Bound::Bounded { steps: 10 },
                ArtifactHandling::Streamed,
                Assumption::measured("cost_per_step", 1.0, "work-units/step", "bench").unwrap(),
            )
            .unwrap(),
        ),
    )
    .expect("projects");

    let plan = DegradationPlan::declare("shed", [Concession::Throughput], "queue_age").unwrap();
    let saturation = projection.under(
        &Demand {
            calls_per_epoch: 50.0,
        },
        &plan,
    );
    assert!(saturation.is_saturated());
    for concession in plan.concessions() {
        assert!(Concession::ALL.contains(concession));
    }
}

#[test]
fn a_pinned_run_and_its_configuration_produce_a_reproducible_pair_of_digests() {
    let build = || {
        let config = ConfigStack::new(schema())
            .push(defaults())
            .resolve()
            .unwrap();
        let registry = registry();
        let pin = registry.pin([]).unwrap();
        let mut run = PinnedRun::new(pin);
        run.decide(&registry, &flag_id("compile.ranking"), false, Epoch::new(1))
            .unwrap();
        (
            config.fingerprint().unwrap().to_string(),
            run.pin().digest().to_string(),
        )
    };
    assert_eq!(build(), build());
}

#[test]
fn the_alpha_report_agrees_with_the_table_the_crate_documentation_prints() {
    let summary = alpha::summary();
    let table = alpha::markdown_table();
    let refuted_rows = table
        .lines()
        .filter(|line| line.contains("| refuted |"))
        .count();
    let unverifiable_rows = table
        .lines()
        .filter(|line| line.contains("| unverifiable |"))
        .count();
    assert_eq!(refuted_rows, summary.refuted);
    assert_eq!(unverifiable_rows, summary.unverifiable);
    assert_eq!(summary.met, 0);
}

#[test]
fn the_acceptance_criterion_about_signing_is_refuted_by_a_linked_type_and_not_by_an_opinion() {
    let finding = alpha::report()
        .into_iter()
        .find(|finding| finding.criterion == Criterion::VerifySignedResultBundle)
        .expect("in the report");
    assert_eq!(finding.verdict, Verdict::Refuted);
    assert!(finding.basis.entails());
    assert!(finding.detail.contains("NotChecked"));
}

#[test]
fn this_crate_reproduces_the_section_40_boilerplate_measurement_bioprism_services_published() {
    assert_eq!(
        bioprism_ops::SECTION_40_BOILERPLATE_PERCENT,
        bioprism_services::SECTION_40_BOILERPLATE_PERCENT,
        "two crates measured §40 independently; a disagreement means one of the two methods is \
         under-specified and both numbers should be withdrawn"
    );
}
