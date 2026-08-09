//! Objectives, providers and placement: the three modules that run the platform rather than
//! store it.

use bioprism_dataops::{
    accept_result, attribute_timeout, burn_rate_alert, declared_objective_names,
    local_path_is_self_contained, plan_shards, reference_local, AdmissionPolicy, AlertDecision,
    AttemptRecord, Attested, Attribution, Basis, BudgetPolicy, BurnThreshold, Capability,
    Confidence, Conformance, ConformanceLevel, ContentHash, Coverage, Epoch, FailureDomain, Fleet,
    Indeterminate, IsolationStrength, JobRequirements, Observations, PartyId, PlacementDecision,
    PlacementError, PlacementPolicy, ProviderCatalog, ProviderError, ProviderId, ProviderKind,
    ProviderProfile, Refusal, Region, Repeatability, ResultEvidence, ServiceObjective, SloError,
    TaskId, TaskLease, Target, ThreatLevel, TimeoutEvidence, TrustDomain, UnitId, WarmPool,
    Window, WorkerDeclaration, WorkerId,
};
use std::collections::BTreeSet;

fn objective(good: u64, per: u64) -> ServiceObjective {
    ServiceObjective::new(
        "api-read-availability",
        Target::new(good, per, "api-read-availability").expect("a reachable target"),
        Window::new(Epoch::new(0), Epoch::new(30)).expect("a forward window"),
    )
    .expect("a plain objective name")
}

fn platform_failure() -> Attribution {
    Attribution::classified(
        FailureDomain::PlatformInfrastructure,
        ["scheduler process restarted".to_string()],
        Confidence::Certain,
    )
    .expect("evidence was supplied")
}

fn agent_failure() -> Attribution {
    Attribution::classified(
        FailureDomain::Agent,
        ["agent emitted an unparsable action".to_string()],
        Confidence::Certain,
    )
    .expect("evidence was supplied")
}

#[test]
fn an_objective_cannot_be_met_when_part_of_the_population_was_never_observed() {
    let report = objective(999, 1000).evaluate(
        &Observations::new(
            100,
            [],
            Coverage::Partial {
                observed: 100,
                expected: 1000,
            },
        ),
        &BudgetPolicy::platform_default(),
    );

    assert!(!report.conformance.is_met());
    assert!(matches!(
        report.conformance,
        Conformance::Indeterminate(Indeterminate::PartialCoverage {
            observed: 100,
            expected: 1000
        })
    ));
}

#[test]
fn a_breach_is_still_provable_when_the_failures_already_seen_exceed_the_whole_allowance() {
    let report = objective(999, 1000).evaluate(
        &Observations::new(
            10,
            [platform_failure(), platform_failure()],
            Coverage::Partial {
                observed: 12,
                expected: 1000,
            },
        ),
        &BudgetPolicy::platform_default(),
    );

    assert!(report.conformance.is_breached());
    assert_eq!(report.charged, 2);
}

#[test]
fn an_objective_over_an_unknown_population_is_indeterminate_rather_than_perfect() {
    let report = objective(999, 1000).evaluate(
        &Observations::new(
            5_000,
            [],
            Coverage::NoDenominator {
                observed: 5_000,
                reason: "the collector restarted mid-window".to_string(),
            },
        ),
        &BudgetPolicy::platform_default(),
    );

    assert!(matches!(
        report.conformance,
        Conformance::Indeterminate(Indeterminate::NoDenominator { .. })
    ));
}

#[test]
fn an_unclassified_failure_charges_the_budget() {
    let report = objective(999, 1000).evaluate(
        &Observations::new(
            999,
            [Attribution::unclassified("no telemetry survived the window")],
            Coverage::of(1000, 1000).expect("a complete window"),
        ),
        &BudgetPolicy::platform_default(),
    );

    assert_eq!(report.charged, 1);
    assert_eq!(report.unclassified, 1);
}

#[test]
fn an_objective_cannot_be_improved_by_declining_to_classify_a_failure() {
    let complete = Coverage::of(1000, 1000).expect("a complete window");
    let policy = BudgetPolicy::platform_default();

    let named = objective(999, 1000).evaluate(
        &Observations::new(999, [platform_failure()], complete.clone()),
        &policy,
    );
    let unnamed = objective(999, 1000).evaluate(
        &Observations::new(
            999,
            [Attribution::unclassified("cause unknown")],
            complete,
        ),
        &policy,
    );

    assert_eq!(named.charged, unnamed.charged);
}

#[test]
fn an_agent_failure_does_not_charge_the_platform_budget() {
    let report = objective(999, 1000).evaluate(
        &Observations::new(
            999,
            [agent_failure()],
            Coverage::of(1000, 1000).expect("a complete window"),
        ),
        &BudgetPolicy::platform_default(),
    );

    assert_eq!(report.charged, 0);
    assert_eq!(report.excluded.get(&FailureDomain::Agent), Some(&1));
    assert!(report.conformance.is_met());
}

#[test]
fn classifying_a_failure_without_evidence_is_refused() {
    let error = Attribution::classified(FailureDomain::Provider, [], Confidence::Weak)
        .expect_err("a classification with no evidence is an opinion");

    assert!(matches!(error, SloError::MalformedField { .. }));
}

#[test]
fn an_alert_on_an_unmeasurable_window_is_neither_fire_nor_quiet() {
    let report = objective(999, 1000).evaluate(
        &Observations::new(
            100,
            [],
            Coverage::NoDenominator {
                observed: 100,
                reason: "the collector restarted".to_string(),
            },
        ),
        &BudgetPolicy::platform_default(),
    );

    let decision = burn_rate_alert(&report, BurnThreshold::new(1, 1).expect("a real threshold"));

    assert!(!decision.is_fire());
    assert!(!decision.is_quiet());
    assert!(matches!(decision, AlertDecision::CannotEvaluate { .. }));
}

#[test]
fn a_budget_burned_past_the_threshold_fires() {
    let report = objective(990, 1000).evaluate(
        &Observations::new(
            980,
            [
                platform_failure(),
                platform_failure(),
                platform_failure(),
                platform_failure(),
                platform_failure(),
                platform_failure(),
                platform_failure(),
                platform_failure(),
                platform_failure(),
                platform_failure(),
                platform_failure(),
            ],
            Coverage::of(991, 991).expect("a complete window"),
        ),
        &BudgetPolicy::platform_default(),
    );

    let decision = burn_rate_alert(&report, BurnThreshold::new(1, 1).expect("a real threshold"));

    assert!(decision.is_fire());
}

#[test]
fn the_allowance_rounds_down_so_the_target_is_never_quietly_weakened() {
    let target = Target::new(999, 1000, "api-read-availability").expect("a reachable target");

    assert_eq!(target.allowance(1500), 1);
    assert_eq!(target.allowance(1999), 1);
    assert_eq!(target.allowance(2000), 2);
}

#[test]
fn a_target_permitting_more_successes_than_attempts_is_refused() {
    let error = Target::new(1001, 1000, "impossible").expect_err("that target cannot be reached");

    assert!(matches!(error, SloError::ImpossibleTarget { .. }));
}

#[test]
fn a_window_that_ends_before_it_starts_is_refused() {
    let error = Window::new(Epoch::new(9), Epoch::new(2)).expect_err("time does not run backwards");

    assert!(matches!(error, SloError::WindowInverted { start: 9, end: 2 }));
}

#[test]
fn the_section_declares_eight_objective_names_and_not_one_target() {
    let names = declared_objective_names();

    assert_eq!(names.len(), 8);
    assert!(names.contains(&"artifact-durability"));
}

fn provider_id(name: &str) -> ProviderId {
    ProviderId::parse(name).expect("a plain provider id")
}

fn trust(name: &str) -> TrustDomain {
    TrustDomain::parse(name).expect("a plain trust domain")
}

fn region(name: &str) -> Region {
    Region::parse(name).expect("a plain region")
}

fn capability(name: &str) -> Capability {
    Capability::parse(name).expect("a plain capability")
}

fn provider(id: &str, isolation: IsolationStrength, verified: bool) -> ProviderProfile {
    let id = provider_id(id);
    let conformance = if verified {
        Attested::first_hand(ConformanceLevel::Full, Epoch::new(1))
    } else {
        ProviderProfile::self_declared_conformance(&id, ConformanceLevel::Full, Epoch::new(1))
            .expect("a plain provider id")
    };
    ProviderProfile::new(
        id,
        ProviderKind::ExternalSandbox,
        region("eu-west"),
        7,
        trust("tenant-a"),
        isolation,
        [capability("gpu")],
        conformance,
    )
}

#[test]
fn a_shared_namespace_never_isolates_hostile_work() {
    let mut catalog = ProviderCatalog::new();
    catalog
        .declare(provider("cheap", IsolationStrength::SharedNamespace, true))
        .expect("the first declaration");

    let error = catalog
        .admit(
            &provider_id("cheap"),
            ThreatLevel::Hostile,
            &AdmissionPolicy {
                minimum_conformance: ConformanceLevel::None,
                require_verified_conformance: false,
            },
        )
        .expect_err("a namespace is not a boundary");

    assert!(matches!(error, ProviderError::IsolationInadequate { .. }));
}

#[test]
fn the_same_conformance_level_verified_and_declared_are_not_the_same_value() {
    let measured = provider("a", IsolationStrength::MicroVm, true);
    let claimed = provider("a", IsolationStrength::MicroVm, false);

    assert_eq!(
        measured.conformance().value(),
        claimed.conformance().value()
    );
    assert_ne!(measured.conformance(), claimed.conformance());
}

#[test]
fn a_provider_that_only_declared_its_conformance_is_refused_when_a_measurement_is_required() {
    let mut catalog = ProviderCatalog::new();
    catalog
        .declare(provider("vendor", IsolationStrength::MicroVm, false))
        .expect("the declaration");

    let error = catalog
        .admit(
            &provider_id("vendor"),
            ThreatLevel::Hostile,
            &AdmissionPolicy {
                minimum_conformance: ConformanceLevel::Full,
                require_verified_conformance: true,
            },
        )
        .expect_err("a vendor claim is not a test result");

    assert!(matches!(
        error,
        ProviderError::ConformanceNotVerified { .. }
    ));
}

#[test]
fn the_same_provider_is_admitted_once_the_platform_has_run_the_suite_itself() {
    let mut catalog = ProviderCatalog::new();
    catalog
        .declare(provider("vendor", IsolationStrength::MicroVm, true))
        .expect("the declaration");

    catalog
        .admit(
            &provider_id("vendor"),
            ThreatLevel::Hostile,
            &AdmissionPolicy {
                minimum_conformance: ConformanceLevel::Full,
                require_verified_conformance: true,
            },
        )
        .expect("a first-hand conformance result satisfies the policy");
}

#[test]
fn a_warm_pool_refuses_work_from_another_trust_domain() {
    let pool = WarmPool::new(provider_id("sandbox"), trust("tenant-a"), 4);

    let error = pool
        .admits(&trust("tenant-b"))
        .expect_err("a warm worker has already run somebody's code");

    assert!(matches!(error, ProviderError::TrustDomainMismatch { .. }));
}

#[test]
fn the_reference_local_path_names_none_of_the_four_forbidden_services() {
    let topology = reference_local().expect("the reference local topology checks");

    local_path_is_self_contained(&topology, &ProviderCatalog::new())
        .expect("the local path is self-contained");
}

#[test]
fn a_kubernetes_provider_disqualifies_the_local_path() {
    let topology = reference_local().expect("the reference local topology checks");
    let mut catalog = ProviderCatalog::new();
    catalog
        .declare(ProviderProfile::new(
            provider_id("cluster"),
            ProviderKind::SelfHostedKubernetes,
            region("eu-west"),
            1,
            trust("tenant-a"),
            IsolationStrength::DedicatedNode,
            [],
            Attested::first_hand(ConformanceLevel::Full, Epoch::new(1)),
        ))
        .expect("the declaration");

    let error = local_path_is_self_contained(&topology, &catalog)
        .expect_err("a cluster is not a local path");

    assert!(matches!(
        error,
        ProviderError::LocalPathNeedsExternalService { .. }
    ));
}

fn worker(id: &str, warm: &[&str]) -> WorkerDeclaration {
    WorkerDeclaration {
        id: WorkerId::parse(id).expect("a plain worker id"),
        provider: provider_id("sandbox"),
        trust_domain: trust("tenant-a"),
        isolation: IsolationStrength::MicroVm,
        capabilities: [capability("gpu")].into_iter().collect(),
        data_regions: [region("eu-west")].into_iter().collect(),
        warm_keys: warm.iter().map(|key| key.to_string()).collect(),
    }
}

fn job(affinity: Option<&str>, sensitive: bool) -> JobRequirements {
    JobRequirements {
        task: TaskId::parse("task-1").expect("a plain task id"),
        needs: [capability("gpu")].into_iter().collect(),
        permitted_regions: [region("eu-west")].into_iter().collect(),
        trust_domain: trust("tenant-a"),
        threat: ThreatLevel::Untrusted,
        sensitive,
        affinity_key: affinity.map(ToString::to_string),
    }
}

fn permissive_policy() -> PlacementPolicy {
    PlacementPolicy {
        require_verified_declaration: false,
        approved_pools: [trust("tenant-a")].into_iter().collect(),
    }
}

#[test]
fn a_placement_onto_a_self_declared_worker_carries_a_declared_basis() {
    let mut fleet = Fleet::new();
    fleet
        .declare(
            worker("w-1", &[]),
            PartyId::parse("w-1").expect("a plain party id"),
            Epoch::new(1),
        )
        .expect("the declaration");

    let decision = fleet.place(&job(None, false), &permissive_policy());

    assert_eq!(decision.worker().map(ToString::to_string).as_deref(), Some("w-1"));
    assert!(matches!(decision.basis(), Some(Basis::Declared { .. })));
}

#[test]
fn a_placement_onto_a_probed_worker_carries_a_first_hand_basis() {
    let mut fleet = Fleet::new();
    fleet
        .probe(worker("w-1", &[]), Epoch::new(1))
        .expect("the probe");

    let decision = fleet.place(&job(None, false), &permissive_policy());

    assert!(decision.basis().is_some_and(Basis::is_first_hand));
}

#[test]
fn a_policy_requiring_verified_declarations_refuses_a_fleet_that_only_declared() {
    let mut fleet = Fleet::new();
    fleet
        .declare(
            worker("w-1", &[]),
            PartyId::parse("w-1").expect("a plain party id"),
            Epoch::new(1),
        )
        .expect("the declaration");
    let policy = PlacementPolicy {
        require_verified_declaration: true,
        approved_pools: [trust("tenant-a")].into_iter().collect(),
    };

    let decision = fleet.place(&job(None, false), &policy);

    assert!(matches!(
        decision,
        PlacementDecision::Refused {
            refusal: Refusal::DeclarationsUnverified { .. }
        }
    ));
}

#[test]
fn a_refusal_names_the_capability_that_eliminated_every_candidate() {
    let mut fleet = Fleet::new();
    fleet
        .probe(worker("w-1", &[]), Epoch::new(1))
        .expect("the probe");
    let mut demanding = job(None, false);
    demanding.needs = [capability("fpga")].into_iter().collect();

    let decision = fleet.place(&demanding, &permissive_policy());

    assert_eq!(
        decision,
        PlacementDecision::Refused {
            refusal: Refusal::CapabilityUnavailable {
                capability: "fpga".to_string()
            }
        }
    );
}

#[test]
fn an_empty_fleet_refuses_rather_than_returning_nothing_ambiguous() {
    let decision = Fleet::new().place(&job(None, false), &permissive_policy());

    assert_eq!(
        decision,
        PlacementDecision::Refused {
            refusal: Refusal::NoWorkers
        }
    );
}

#[test]
fn placement_prefers_a_worker_already_warm_for_the_affinity_key() {
    let mut fleet = Fleet::new();
    fleet
        .probe(worker("w-1", &[]), Epoch::new(1))
        .expect("the first probe");
    fleet
        .probe(worker("w-2", &["cohort-7"]), Epoch::new(1))
        .expect("the second probe");

    let decision = fleet.place(&job(Some("cohort-7"), false), &permissive_policy());

    assert_eq!(decision.worker().map(ToString::to_string).as_deref(), Some("w-2"));
}

#[test]
fn placement_without_an_affinity_key_is_deterministic_across_repeats() {
    let mut fleet = Fleet::new();
    fleet.probe(worker("w-2", &[]), Epoch::new(1)).expect("w-2");
    fleet.probe(worker("w-1", &[]), Epoch::new(1)).expect("w-1");

    let first = fleet.place(&job(None, false), &permissive_policy());
    let second = fleet.place(&job(None, false), &permissive_policy());

    assert_eq!(first, second);
    assert_eq!(first.worker().map(ToString::to_string).as_deref(), Some("w-1"));
}

#[test]
fn a_sensitive_job_will_not_run_outside_an_approved_pool() {
    let mut fleet = Fleet::new();
    fleet
        .probe(worker("w-1", &[]), Epoch::new(1))
        .expect("the probe");
    let policy = PlacementPolicy {
        require_verified_declaration: false,
        approved_pools: BTreeSet::new(),
    };

    let decision = fleet.place(&job(None, true), &policy);

    assert!(matches!(
        decision,
        PlacementDecision::Refused {
            refusal: Refusal::PoolNotApproved { .. }
        }
    ));
}

#[test]
fn a_worker_declared_twice_is_refused() {
    let mut fleet = Fleet::new();
    fleet.probe(worker("w-1", &[]), Epoch::new(1)).expect("once");

    let error = fleet
        .probe(worker("w-1", &[]), Epoch::new(2))
        .expect_err("two records for one worker");

    assert!(matches!(error, PlacementError::DuplicateWorker { .. }));
}

fn units(count: u64) -> Vec<UnitId> {
    (0..count)
        .map(|index| UnitId::parse(format!("parent-{index}")).expect("a plain unit id"))
        .collect()
}

#[test]
fn sharding_places_every_unit_exactly_once() {
    let plan = plan_shards(units(50), 4, "release-3").expect("four shards is a legal plan");

    let total: usize = plan.shards.values().map(BTreeSet::len).sum();
    assert_eq!(total, 50);
    assert_eq!(plan.assigned().len(), 50);
    assert_eq!(plan.seeds.len(), 50);
}

#[test]
fn sharding_is_reproducible_for_the_same_salt_and_moves_for_a_different_one() {
    let first = plan_shards(units(50), 4, "release-3").expect("a plan");
    let same = plan_shards(units(50), 4, "release-3").expect("the same plan");
    let other = plan_shards(units(50), 4, "release-4").expect("a different plan");

    assert_eq!(first, same);
    assert_ne!(first, other);
}

#[test]
fn adding_a_unit_does_not_reshuffle_the_units_already_placed() {
    let before = plan_shards(units(20), 4, "release-3").expect("a plan");
    let after = plan_shards(units(21), 4, "release-3").expect("a larger plan");

    for unit in units(20) {
        assert_eq!(before.shard_of(&unit), after.shard_of(&unit));
    }
}

#[test]
fn zero_shards_is_refused_rather_than_silently_making_one() {
    let error = plan_shards(units(3), 0, "release-3").expect_err("zero shards holds nothing");

    assert!(matches!(
        error,
        PlacementError::ImpossibleShardCount { shards: 0, units: 3 }
    ));
}

#[test]
fn a_side_effecting_task_cannot_be_speculatively_duplicated() {
    let error = AttemptRecord::speculate(
        TaskId::parse("task-1").expect("a plain task id"),
        Repeatability::SideEffecting,
        WorkerId::parse("w-1").expect("a plain worker id"),
        [WorkerId::parse("w-2").expect("a plain worker id")],
    )
    .expect_err("running it twice may run its effect twice");

    assert!(matches!(error, PlacementError::SpeculationUnsafe { .. }));
}

#[test]
fn a_speculative_race_keeps_the_attempts_that_lost() {
    let record = AttemptRecord::speculate(
        TaskId::parse("task-1").expect("a plain task id"),
        Repeatability::Idempotent,
        WorkerId::parse("w-1").expect("a plain worker id"),
        [WorkerId::parse("w-2").expect("a plain worker id")],
    )
    .expect("an idempotent task may be duplicated");

    assert_eq!(record.attempts(), 2);
    assert_eq!(record.losers().len(), 1);
}

#[test]
fn a_timeout_whose_infrastructure_health_was_never_established_is_not_an_agent_failure() {
    let attribution = attribute_timeout(&TimeoutEvidence {
        infrastructure_healthy: None,
        agent_made_progress: Some(true),
    });

    assert_eq!(attribution.domain(), None);
    assert!(matches!(attribution, Attribution::Unclassified { .. }));
}

#[test]
fn a_timeout_during_a_confirmed_infrastructure_failure_belongs_to_the_platform() {
    let attribution = attribute_timeout(&TimeoutEvidence {
        infrastructure_healthy: Some(false),
        agent_made_progress: None,
    });

    assert_eq!(
        attribution.domain(),
        Some(FailureDomain::PlatformInfrastructure)
    );
}

#[test]
fn a_timeout_with_healthy_infrastructure_and_an_observed_agent_is_an_evaluation_outcome() {
    let attribution = attribute_timeout(&TimeoutEvidence {
        infrastructure_healthy: Some(true),
        agent_made_progress: Some(true),
    });

    assert_eq!(attribution.domain(), Some(FailureDomain::Agent));
    assert!(attribution
        .domain()
        .is_some_and(FailureDomain::is_evaluation_outcome));
}

fn lease() -> TaskLease {
    TaskLease {
        task: TaskId::parse("task-1").expect("a plain task id"),
        worker: WorkerId::parse("w-1").expect("a plain worker id"),
        issued_at: Epoch::new(1),
        expires_at: Epoch::new(5),
        input_manifest: ContentHash::of_bytes(b"inputs"),
    }
}

#[test]
fn a_result_arriving_after_its_lease_expired_is_refused() {
    let error = accept_result(
        &lease(),
        ResultEvidence::DigestClaimed {
            by: WorkerId::parse("w-1").expect("a plain worker id"),
            digest: ContentHash::of_bytes(b"outputs"),
        },
        Epoch::new(6),
    )
    .expect_err("the lease had already gone");

    assert!(matches!(error, PlacementError::LeaseExpired { .. }));
}

#[test]
fn a_result_without_a_digest_is_unobserved_rather_than_accepted_quietly() {
    let accepted = accept_result(
        &lease(),
        ResultEvidence::Unattested {
            by: WorkerId::parse("w-1").expect("a plain worker id"),
            reason: "the worker returned no digest".to_string(),
        },
        Epoch::new(4),
    )
    .expect("an unattested result is still a result");

    assert!(matches!(accepted.basis(), Basis::Unobserved { .. }));
    assert!(!accepted.value().has_claimed_digest());
}

#[test]
fn a_claimed_digest_is_a_declaration_by_the_worker_and_not_a_verification() {
    let accepted = accept_result(
        &lease(),
        ResultEvidence::DigestClaimed {
            by: WorkerId::parse("w-1").expect("a plain worker id"),
            digest: ContentHash::of_bytes(b"outputs"),
        },
        Epoch::new(4),
    )
    .expect("a claimed digest is accepted");

    assert!(matches!(accepted.basis(), Basis::Declared { .. }));
    assert!(!accepted.basis().is_first_hand());
}
