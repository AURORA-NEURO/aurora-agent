//! Local-first and federated deployment: the offline contract, the doctor, and imports from hubs
//! somebody else governs.

use bioprism_dataops::{
    import_record, minimize_cross_region, plan_replication, private_worker_link, reference_local,
    reference_team, ArtifactSensitivity, Basis, ConnectionDirection, DataClass, Demand,
    Deployment, DeploymentPlan, Detail, DoctorReport, Epoch, FederationError, FederationPolicy,
    LocalError, Mutability, OfflineContract, ParityClaim, PartyId, Plane, PlanePlacement,
    ProbeOutcome, Readiness, Region, Replication, Requirement, Resolution, ResourceEnvelope,
    SignedRecord, Source, StoreProfile, TenantPattern, TopologyDraft, Unsatisfiable,
};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};

fn embedded(name: &str, technology: &str) -> Requirement {
    Requirement {
        name: name.to_string(),
        source: Source::Embedded {
            technology: technology.to_string(),
        },
    }
}

fn networked(name: &str, host: &str) -> Requirement {
    Requirement {
        name: name.to_string(),
        source: Source::Network {
            host: host.to_string(),
        },
    }
}

#[test]
fn a_closed_contract_refuses_a_network_requirement_and_names_the_host() {
    let resolution = OfflineContract::closed().resolve(&networked("registry", "hub.example"));

    assert_eq!(
        resolution,
        Resolution::Refused {
            reason: Unsatisfiable::NetworkDenied {
                host: "hub.example".to_string()
            }
        }
    );
}

#[test]
fn the_core_demo_resolves_entirely_without_a_network() {
    let contract = OfflineContract::closed();
    let demo = [
        embedded("catalog", "sqlite"),
        embedded("artifacts", "filesystem-cas"),
        Requirement {
            name: "model".to_string(),
            source: Source::Fixture {
                name: "recorded-completions".to_string(),
            },
        },
    ];

    assert_eq!(contract.admit_all(demo.iter()).expect("all three resolve"), 3);
}

#[test]
fn an_open_contract_still_refuses_a_host_that_is_not_on_the_allow_list() {
    let contract = OfflineContract::open_to(["hub.example".to_string()]);

    let error = contract
        .admit_all([&networked("registry", "elsewhere.example")])
        .expect_err("an allow list is not a switch");

    assert!(matches!(error, LocalError::HostNotAllowed { .. }));
}

#[test]
fn an_open_contract_admits_the_host_it_named() {
    let contract = OfflineContract::open_to(["hub.example".to_string()]);

    assert!(contract
        .resolve(&networked("registry", "hub.example"))
        .is_satisfied());
}

#[test]
fn a_skipped_probe_leaves_the_deployment_undetermined_rather_than_ready() {
    let report = DoctorReport::new()
        .with(
            "disk",
            ProbeOutcome::Ok {
                detail: Detail::plain("41 GB free"),
            },
        )
        .expect("the first probe")
        .with(
            "container-runtime",
            ProbeOutcome::NotChecked {
                reason: "no runtime binary on PATH to interrogate".to_string(),
            },
        )
        .expect("the second probe");

    let readiness = report.readiness();

    assert!(!readiness.is_ready());
    assert!(matches!(readiness, Readiness::Undetermined { .. }));
}

#[test]
fn a_report_whose_probes_all_passed_is_ready_and_says_how_many_ran() {
    let report = DoctorReport::new()
        .with(
            "disk",
            ProbeOutcome::Ok {
                detail: Detail::plain("41 GB free"),
            },
        )
        .expect("the probe");

    assert_eq!(report.readiness(), Readiness::Ready { checked: 1 });
}

#[test]
fn a_definite_problem_outranks_a_skipped_probe() {
    let report = DoctorReport::new()
        .with(
            "disk",
            ProbeOutcome::Problem {
                detail: Detail::plain("0 GB free"),
                remedy: "free space or point the store elsewhere".to_string(),
            },
        )
        .expect("the first probe")
        .with(
            "ports",
            ProbeOutcome::NotChecked {
                reason: "skipped".to_string(),
            },
        )
        .expect("the second probe");

    assert!(matches!(report.readiness(), Readiness::NotReady { .. }));
}

#[test]
fn redaction_replaces_the_value_and_keeps_the_probe_and_its_outcome() {
    let report = DoctorReport::new()
        .with(
            "provider-key",
            ProbeOutcome::Problem {
                detail: Detail::sensitive("sk-live-9f2a"),
                remedy: "rotate the key".to_string(),
            },
        )
        .expect("the probe");

    let shareable = report.redacted().expect("redaction succeeds");

    assert!(report.discloses_sensitive());
    assert!(!shareable.discloses_sensitive());
    assert!(shareable.probes().contains_key("provider-key"));
    assert!(matches!(
        shareable.probes()["provider-key"],
        ProbeOutcome::Problem { .. }
    ));
    assert_eq!(report.readiness(), shareable.readiness());
}

#[test]
fn redaction_is_idempotent_and_two_machines_holding_the_same_value_agree() {
    let one = DoctorReport::new()
        .with(
            "provider-key",
            ProbeOutcome::Ok {
                detail: Detail::sensitive("sk-live-9f2a"),
            },
        )
        .expect("the probe");
    let other = DoctorReport::new()
        .with(
            "provider-key",
            ProbeOutcome::Ok {
                detail: Detail::sensitive("sk-live-9f2a"),
            },
        )
        .expect("the probe");

    let once = one.redacted().expect("redaction succeeds");
    let twice = once.redacted().expect("redaction is idempotent");

    assert_eq!(once, twice);
    assert_eq!(once, other.redacted().expect("redaction succeeds"));
}

#[test]
fn a_probe_declared_twice_is_refused() {
    let error = DoctorReport::new()
        .with(
            "disk",
            ProbeOutcome::Ok {
                detail: Detail::plain("ok"),
            },
        )
        .expect("the first probe")
        .with(
            "disk",
            ProbeOutcome::NotChecked {
                reason: "skipped".to_string(),
            },
        )
        .expect_err("two answers for one probe");

    assert!(matches!(error, LocalError::DuplicateProbe { .. }));
}

#[test]
fn an_envelope_refusal_names_the_resource_and_both_numbers() {
    let envelope = ResourceEnvelope {
        memory_mb: 8_192,
        disk_mb: 40_960,
    };

    let error = envelope
        .admit(Demand {
            memory_mb: 32_768,
            disk_mb: 1_024,
        })
        .expect_err("the machine is too small");

    assert_eq!(
        error,
        LocalError::EnvelopeExceeded {
            resource: "memory_mb",
            needed: 32_768,
            available: 8_192
        }
    );
}

#[test]
fn a_parity_claim_can_only_be_established_from_an_actual_comparison() {
    let asserted = ParityClaim::unverified("nobody compared the two deployments");
    let compared = ParityClaim::between(
        &reference_local().expect("the local topology"),
        &reference_team().expect("the team topology"),
    );

    assert!(!asserted.is_established());
    assert!(compared.is_established());
}

#[test]
fn an_established_parity_claim_says_how_many_classes_it_compared() {
    let local = reference_local().expect("the local topology");

    let claim = ParityClaim::between(&local, &local);

    assert!(matches!(claim, ParityClaim::Established { compared: 5 }));
}

#[test]
fn a_parity_claim_over_diverging_promises_names_the_class_that_diverged() {
    let local = reference_local().expect("the local topology");
    let divergent = TopologyDraft::new()
        .with_store(
            StoreProfile::canonical("catalog", "postgresql", Mutability::Mutable)
                .expect("a plain store"),
        )
        .with_store(
            StoreProfile::canonical("cas", "s3-compatible", Mutability::AppendOnly)
                .expect("a plain store"),
        )
        .with_store(
            StoreProfile::canonical("eventlog", "object-store-segments", Mutability::AppendOnly)
                .expect("a plain store"),
        )
        .with_store(
            StoreProfile::rebuildable(
                "results",
                "analytical-engine",
                Mutability::Mutable,
                [DataClass::Event, DataClass::Metadata],
            )
            .expect("a plain store"),
        )
        .with_store(
            StoreProfile::rebuildable(
                "index",
                "structured-index",
                Mutability::Mutable,
                [DataClass::Metadata, DataClass::Artifact],
            )
            .expect("a plain store"),
        )
        .assign(DataClass::Metadata, "catalog")
        .assign(DataClass::Artifact, "cas")
        .assign(DataClass::Event, "eventlog")
        .assign(DataClass::Analytics, "results")
        .assign(DataClass::Search, "index")
        .check(Deployment::Team)
        .expect("an append-only artifact store still preserves evidence");

    let claim = ParityClaim::between(&local, &divergent);

    match claim {
        ParityClaim::Broken { differences } => {
            assert_eq!(differences.len(), 1);
            assert_eq!(differences[0].class, DataClass::Artifact);
        }
        other => panic!("expected a broken claim, got {other:?}"),
    }
}

fn full_plan(execution: PlanePlacement) -> DeploymentPlan {
    let mut plan = DeploymentPlan::new();
    for plane in Plane::ALL {
        plan = plan.place(plane, PlanePlacement::HubOperated);
    }
    plan.place(Plane::ExecutionPool, execution)
}

#[test]
fn a_plan_missing_a_plane_is_refused_before_any_compatibility_question() {
    let plan = DeploymentPlan::new().place(Plane::ControlApi, PlanePlacement::HubOperated);

    let error = plan
        .validate(TenantPattern::SharedControl)
        .expect_err("a plan with a hole gets completed by whoever deploys it");

    assert!(matches!(error, FederationError::PlaneUnplaced { .. }));
}

#[test]
fn an_execution_pool_inside_a_customer_network_is_permitted() {
    full_plan(PlanePlacement::CustomerNetwork)
        .validate(TenantPattern::SharedControl)
        .expect("the section says execution pools may live in customer networks");
}

#[test]
fn a_control_plane_inside_a_customer_network_is_refused_under_shared_control() {
    let plan = full_plan(PlanePlacement::HubOperated)
        .place(Plane::Signing, PlanePlacement::CustomerNetwork);

    let error = plan
        .validate(TenantPattern::SharedControl)
        .expect_err("shared control does not put signing in a tenant network");

    assert_eq!(
        error,
        FederationError::PlaneMisplaced {
            plane: "signing",
            pattern: "shared-control"
        }
    );
}

#[test]
fn the_same_placement_is_correct_under_a_dedicated_installation() {
    full_plan(PlanePlacement::HubOperated)
        .place(Plane::Signing, PlanePlacement::CustomerNetwork)
        .validate(TenantPattern::DedicatedInstallation)
        .expect("a dedicated installation is entirely the customer's");
}

#[test]
fn an_air_gapped_registry_requires_every_plane_air_gapped() {
    let mut plan = DeploymentPlan::new();
    for plane in Plane::ALL {
        plan = plan.place(plane, PlanePlacement::AirGapped);
    }
    let leaky = plan
        .clone()
        .place(Plane::Observability, PlanePlacement::HubOperated);

    plan.validate(TenantPattern::AirGappedRegistry)
        .expect("all nine planes are air-gapped");
    assert!(leaky.validate(TenantPattern::AirGappedRegistry).is_err());
}

#[test]
fn an_inbound_link_into_a_customer_network_is_refused() {
    let plan = full_plan(PlanePlacement::CustomerNetwork);

    let error = private_worker_link(
        &plan,
        Plane::ExecutionPool,
        ConnectionDirection::InboundToCustomer,
    )
    .expect_err("a private worker dials out");

    assert_eq!(
        error,
        FederationError::InboundRequired {
            plane: "execution-pool"
        }
    );
}

#[test]
fn an_outbound_link_from_a_customer_network_is_accepted() {
    private_worker_link(
        &full_plan(PlanePlacement::CustomerNetwork),
        Plane::ExecutionPool,
        ConnectionDirection::OutboundFromCustomer,
    )
    .expect("outbound is the requirement");
}

fn region(name: &str) -> Region {
    Region::parse(name).expect("a plain region")
}

#[test]
fn a_sensitive_artifact_cannot_be_replicated_out_of_its_region() {
    let error = plan_replication(
        "trial-42-trace",
        ArtifactSensitivity::Sensitive,
        &region("eu-west"),
        [region("us-east")],
    )
    .expect_err("sensitive artifacts are pinned");

    assert!(matches!(
        error,
        FederationError::SensitiveArtifactReplication { .. }
    ));
}

#[test]
fn a_sensitive_artifact_with_no_targets_is_pinned_to_its_home_region() {
    let plan = plan_replication(
        "trial-42-trace",
        ArtifactSensitivity::Sensitive,
        &region("eu-west"),
        [],
    )
    .expect("pinning is legal");

    assert_eq!(
        plan,
        Replication::Pinned {
            region: region("eu-west")
        }
    );
}

#[test]
fn a_public_artifact_replicates_to_every_region_but_its_own() {
    let plan = plan_replication(
        "pack-a",
        ArtifactSensitivity::Public,
        &region("eu-west"),
        [region("eu-west"), region("us-east")],
    )
    .expect("public artifacts replicate");

    assert_eq!(
        plan,
        Replication::Replicated {
            to: [region("us-east")].into_iter().collect()
        }
    );
}

#[test]
fn cross_region_metadata_keeps_only_the_allow_listed_fields() {
    let record: BTreeMap<String, serde_json::Value> = [
        ("id".to_string(), json!("pack-a")),
        ("digest".to_string(), json!("abcd")),
        ("submitter_email".to_string(), json!("someone@example.org")),
    ]
    .into_iter()
    .collect();
    let allowed: BTreeSet<String> = ["id".to_string(), "digest".to_string()].into_iter().collect();

    let minimized = minimize_cross_region(&record, &allowed);

    assert_eq!(minimized.len(), 2);
    assert!(!minimized.contains_key("submitter_email"));
}

fn hub(name: &str) -> PartyId {
    PartyId::parse(name).expect("a plain hub id")
}

fn record(attestation: Option<&str>) -> SignedRecord {
    SignedRecord {
        hub: hub("hub-b"),
        payload: json!({ "pack": "pack-a" }),
        attestation: attestation.map(ToString::to_string),
        origin_epoch: Epoch::new(4),
    }
}

#[test]
fn a_record_from_an_untrusted_publisher_is_not_imported() {
    let policy = FederationPolicy::new([hub("hub-c")], true);

    let error = import_record(&record(Some("sig")), &policy, Epoch::new(9))
        .expect_err("local policy decides which publishers to trust");

    assert!(matches!(error, FederationError::UntrustedPublisher { .. }));
}

#[test]
fn an_unattested_record_is_refused_unless_the_policy_accepts_one() {
    let strict = FederationPolicy::new([hub("hub-b")], false);
    let lenient = FederationPolicy::new([hub("hub-b")], true);

    assert!(matches!(
        import_record(&record(None), &strict, Epoch::new(9)),
        Err(FederationError::UnattestedImport { .. })
    ));
    assert!(import_record(&record(None), &lenient, Epoch::new(9)).is_ok());
}

#[test]
fn an_imported_record_is_replicated_and_carries_both_epochs() {
    let policy = FederationPolicy::new([hub("hub-b")], false);

    let imported =
        import_record(&record(Some("sig")), &policy, Epoch::new(9)).expect("the import succeeds");

    assert!(!imported.basis().is_first_hand());
    assert_eq!(
        imported.basis(),
        &Basis::Replicated {
            origin: hub("hub-b"),
            origin_epoch: Epoch::new(4),
            received_at: Epoch::new(9)
        }
    );
}

#[test]
fn a_locally_published_value_and_the_same_value_imported_are_not_equal() {
    let policy = FederationPolicy::new([hub("hub-b")], false);
    let imported =
        import_record(&record(Some("sig")), &policy, Epoch::new(9)).expect("the import succeeds");
    let local = bioprism_dataops::Attested::first_hand(json!({ "pack": "pack-a" }), Epoch::new(9));

    assert_eq!(imported.value(), local.value());
    assert_ne!(imported, local);
}
