//! Blueprint 35.01, 35.09, 35.14, 35.17 and 35.18 — the gates around the content pipeline.

use bioprism_scale::adaptive::{AdaptivePlan, Stratum, StoppingDecision};
use bioprism_scale::audit::{Auditor, QualityGate, ReleaseAudit};
use bioprism_scale::error::{AdaptiveError, AuditError, ReleaseError};
use bioprism_scale::portfolio::{Cell, PortfolioPlan};
use bioprism_scale::release::{ReleaseLedger, ReleaseState, ReleaseVersion};
use bioprism_scale::schedule::{schedule, HiddenSeed, MutationBudget, ParentEligibility};
use std::collections::{BTreeMap, BTreeSet};

fn families(names: &[&str]) -> BTreeSet<String> {
    names.iter().map(|name| name.to_string()).collect()
}

fn eligible_parents(count: usize) -> Vec<ParentEligibility> {
    (0..count)
        .map(|index| ParentEligibility {
            parent: format!("p{index}"),
            decisions: vec![format!("d{index}-a"), format!("d{index}-b")],
            compatible_families: families(&["rename", "distract", "camouflage"]),
        })
        .collect()
}

#[test]
fn a_schedule_reports_its_class_ceiling_before_anything_is_generated() {
    let parents = eligible_parents(4);
    let plan = schedule(
        &parents,
        MutationBudget { instances: 200 },
        HiddenSeed::new(0xB10),
    );

    assert_eq!(plan.entries.len(), 200);
    assert_eq!(
        plan.class_ceiling(),
        24,
        "8 decisions × 3 families is every class this schedule can ever reach"
    );
    assert!(plan.projected_inflation() > 8.0);
    assert_eq!(plan.repeat_parameterizations, 200 - 24);
}

#[test]
fn a_schedule_covers_every_pair_before_it_repeats_one() {
    let parents = eligible_parents(4);
    let plan = schedule(
        &parents,
        MutationBudget { instances: 24 },
        HiddenSeed::new(7),
    );

    assert_eq!(plan.covered_pairs, 24);
    assert_eq!(plan.repeat_parameterizations, 0);
    assert!((plan.projected_inflation() - 1.0).abs() < f64::EPSILON);
}

#[test]
fn a_schedule_is_deterministic_under_its_seed() {
    let parents = eligible_parents(3);
    let budget = MutationBudget { instances: 30 };
    let a = schedule(&parents, budget, HiddenSeed::new(42));
    let b = schedule(&parents, budget, HiddenSeed::new(42));
    let c = schedule(&parents, budget, HiddenSeed::new(43));

    assert_eq!(a.entries, b.entries);
    assert_ne!(a.entries, c.entries);
}

#[test]
fn a_hidden_seed_serializes_to_its_digest_and_never_to_its_value() {
    let seed = HiddenSeed::new(0xDEAD_BEEF_CAFE_1234);
    let encoded = serde_json::to_string(&seed).unwrap();

    assert!(!encoded.contains("deadbeef"));
    assert!(!encoded.contains(&0xDEAD_BEEF_CAFE_1234u64.to_string()));
    assert_eq!(encoded, format!("\"{}\"", seed.digest()));
    assert_eq!(seed.digest(), HiddenSeed::new(0xDEAD_BEEF_CAFE_1234).digest());
    assert_ne!(seed.digest(), HiddenSeed::new(1).digest());
}

#[test]
fn a_budget_below_the_mandatory_safety_floor_is_refused() {
    let plan = AdaptivePlan::new(vec![
        Stratum::mandatory("dual-use-screen", 400),
        Stratum::mandatory("dosing-safety", 300),
        Stratum::discretionary("capability-probe", 5_000, 0.9),
    ]);

    match plan.select(500) {
        Err(AdaptiveError::BudgetBelowMandatoryFloor {
            budget,
            floor,
            strata,
        }) => {
            assert_eq!(budget, 500);
            assert_eq!(floor, 700);
            assert!(strata.contains("dual-use-screen"));
            assert!(strata.contains("dosing-safety"));
        }
        other => panic!("safety strata are not tradeable for information gain: {other:?}"),
    }
}

#[test]
fn mandatory_strata_are_taken_before_information_gain_is_consulted() {
    let plan = AdaptivePlan::new(vec![
        Stratum::mandatory("dual-use-screen", 400),
        Stratum::discretionary("high-gain", 300, 0.95),
        Stratum::discretionary("low-gain", 300, 0.10),
    ]);

    let selection = plan.select(800).unwrap();
    assert!(selection.selected_strata.contains(&"dual-use-screen".to_string()));
    assert!(selection.selected_strata.contains(&"high-gain".to_string()));
    assert_eq!(selection.skipped_strata, vec!["low-gain".to_string()]);
    assert_eq!(selection.instances, 700);
    assert_eq!(selection.mandatory_instances, 400);
    assert!(selection.cost_reduction_versus_exhaustive(1_000_000) > 0.99);
}

#[test]
fn a_stratum_that_does_not_fit_is_skipped_whole_and_named() {
    let plan = AdaptivePlan::new(vec![
        Stratum::mandatory("safety", 10),
        Stratum::discretionary("large", 1_000, 0.99),
    ]);
    let selection = plan.select(100).unwrap();

    assert_eq!(selection.instances, 10, "half a stratum has none of its coverage property");
    assert_eq!(selection.skipped_strata, vec!["large".to_string()]);
}

#[test]
fn stopping_uses_the_effective_sample_size_and_not_the_instance_count() {
    let on_instances = StoppingDecision::evaluate(50_000, 100_000.0, 0.01).unwrap();
    let on_effective = StoppingDecision::evaluate(3_718, 7_435.0, 0.01).unwrap();

    assert!(on_instances.stop, "the inflated count declares victory");
    assert!(
        !on_effective.stop,
        "the same data, honestly counted, has not reached the target half-width"
    );
    assert!(on_effective.half_width > on_instances.half_width);
    assert_eq!(on_effective.effective_sample_size, 7_435.0);
    assert!(on_effective.reason.contains("normal approximation"));
}

#[test]
fn a_non_positive_stopping_target_is_refused() {
    assert!(matches!(
        StoppingDecision::evaluate(1, 10.0, 0.0),
        Err(AdaptiveError::NonPositiveTarget(_))
    ));
    let no_data = StoppingDecision::evaluate(0, 0.0, 0.01).unwrap();
    assert!(!no_data.stop);
    assert!(no_data.half_width.is_infinite());
}

#[test]
fn a_portfolio_report_always_names_its_blind_spots() {
    let mut plan = PortfolioPlan::new();
    plan.target(Cell::new("molecular", "omics", "target-selection"), 2)
        .target(Cell::new("cellular", "imaging", "phenotype-call"), 2)
        .target(Cell::new("clinical", "ehr", "treatment-choice"), 2)
        .author(Cell::new("molecular", "omics", "target-selection"), "site-a")
        .author(Cell::new("molecular", "omics", "target-selection"), "site-a")
        .author(Cell::new("cellular", "imaging", "phenotype-call"), "site-b");

    let report = plan.report(600.0);
    assert_eq!(report.blind_spots.len(), 1);
    assert_eq!(report.blind_spots[0].label(), "clinical/ehr/treatment-choice");
    assert_eq!(report.authored_worlds, 3);
    assert_eq!(report.targeted_worlds, 6);
    assert_eq!(report.distinct_sites, 2);
    assert!((report.site_concentration - 2.0 / 3.0).abs() < 1e-9);
    assert!(report.headline().contains("clinical/ehr/treatment-choice"));
    assert_eq!(report.expert_minutes, 1_800.0);
}

#[test]
fn a_fully_covered_portfolio_still_reports_an_explicit_empty_blind_spot_list() {
    let mut plan = PortfolioPlan::new();
    plan.target(Cell::new("molecular", "omics", "target-selection"), 1)
        .author(Cell::new("molecular", "omics", "target-selection"), "site-a");

    let report = plan.report(600.0);
    assert!(report.blind_spots.is_empty());
    assert_eq!(report.coverage, 1.0);
    assert!(report.headline().contains("none"));
}

#[test]
fn a_republished_version_with_different_content_is_refused() {
    let mut ledger = ReleaseLedger::new();
    let version = ReleaseVersion::new(1, 0, 0);
    ledger.publish(version, "digest-a", BTreeMap::new()).unwrap();
    ledger.publish(version, "digest-a", BTreeMap::new()).unwrap();

    match ledger.publish(version, "digest-b", BTreeMap::new()) {
        Err(ReleaseError::ImmutableReleaseModified {
            version,
            published,
            attempted,
        }) => {
            assert_eq!(version, "1.0.0");
            assert_eq!(published, "digest-a");
            assert_eq!(attempted, "digest-b");
        }
        other => panic!("republishing invalidates every result ever reported: {other:?}"),
    }
}

#[test]
fn supersession_cannot_go_backwards() {
    let mut ledger = ReleaseLedger::new();
    ledger
        .publish(ReleaseVersion::new(1, 0, 0), "a", BTreeMap::new())
        .unwrap();
    ledger
        .publish(ReleaseVersion::new(2, 0, 0), "b", BTreeMap::new())
        .unwrap();

    assert!(matches!(
        ledger.supersede(ReleaseVersion::new(2, 0, 0), ReleaseVersion::new(1, 0, 0)),
        Err(ReleaseError::SupersessionGoesBackwards { .. })
    ));

    let notice = ledger
        .supersede(ReleaseVersion::new(1, 0, 0), ReleaseVersion::new(2, 0, 0))
        .unwrap();
    assert!(notice.notice.contains("not comparable"));
    assert_eq!(
        ledger.status_of(ReleaseVersion::new(1, 0, 0)).unwrap().state,
        ReleaseState::Superseded {
            by: ReleaseVersion::new(2, 0, 0)
        }
    );
}

#[test]
fn a_withdrawn_release_is_marked_and_stays_retrievable() {
    let mut ledger = ReleaseLedger::new();
    let version = ReleaseVersion::new(1, 2, 3);
    ledger.publish(version, "digest", BTreeMap::new()).unwrap();
    ledger.withdraw(version, "oracle defect found post-release").unwrap();

    let release = ledger.status_of(version).expect("entries are never deleted");
    assert!(matches!(release.state, ReleaseState::Withdrawn { .. }));
    assert_eq!(release.content_digest, "digest");
    assert!(matches!(
        ledger.withdraw(version, "again"),
        Err(ReleaseError::AlreadyWithdrawn(_))
    ));
    assert_eq!(ledger.all().len(), 1);
}

#[test]
fn deprecation_records_a_support_window_and_a_reason() {
    let mut ledger = ReleaseLedger::new();
    let version = ReleaseVersion::new(1, 0, 0);
    ledger.publish(version, "d", BTreeMap::new()).unwrap();
    ledger.deprecate(version, 12, "superseded oracle pins").unwrap();

    match &ledger.status_of(version).unwrap().state {
        ReleaseState::Deprecated {
            support_epochs,
            reason,
        } => {
            assert_eq!(*support_epochs, 12);
            assert_eq!(reason, "superseded oracle pins");
        }
        other => panic!("expected a deprecation, got {other:?}"),
    }
    assert!(matches!(
        ledger.deprecate(ReleaseVersion::new(9, 9, 9), 1, "x"),
        Err(ReleaseError::UnknownRelease { .. })
    ));
}

#[test]
fn an_auditor_who_produced_the_artefact_is_refused() {
    match ReleaseAudit::open("pack-1.0.0", "factory-node-7", Auditor::new("factory-node-7")) {
        Err(AuditError::SelfAudit { auditor }) => assert_eq!(auditor, "factory-node-7"),
        other => panic!("independent reproduction means a different party: {other:?}"),
    }
    assert!(ReleaseAudit::open("pack-1.0.0", "factory-node-7", Auditor::new("site-b")).is_ok());
}

#[test]
fn an_unevaluated_gate_is_not_a_passed_gate() {
    let mut audit =
        ReleaseAudit::open("pack-1.0.0", "factory-node-7", Auditor::new("site-b")).unwrap();
    for gate in QualityGate::ALL.into_iter().take(7) {
        audit.record(gate, true, "checked");
    }

    match audit.finish() {
        Err(AuditError::GateUnevaluated { gate }) => {
            assert_eq!(
                gate,
                QualityGate::IndependentReproduction.as_str(),
                "the eighth gate is the one an eager pipeline is most likely to skip"
            );
        }
        other => panic!("nobody-checked is not a pass: {other:?}"),
    }
}

#[test]
fn a_failed_gate_blocks_the_release_and_names_it() {
    let mut audit =
        ReleaseAudit::open("pack-1.0.0", "factory-node-7", Auditor::new("site-b")).unwrap();
    for gate in QualityGate::ALL {
        let passed = gate != QualityGate::DuplicateAndContaminationScans;
        audit.record(gate, passed, if passed { "ok" } else { "canary reproduced" });
    }

    let report = audit.finish().unwrap();
    assert!(!report.may_release());
    assert_eq!(report.release_blocking_defects, 1);
    assert!(report
        .blocking_reason
        .expect("a blocked release states why")
        .contains("duplicate_and_contamination_scans"));
    assert_eq!(report.outcomes.len(), 8);
}

#[test]
fn a_clean_audit_by_an_independent_party_may_release() {
    let mut audit =
        ReleaseAudit::open("pack-1.0.0", "factory-node-7", Auditor::new("site-b")).unwrap();
    for gate in QualityGate::ALL {
        audit.record(gate, true, "reproduced independently");
    }

    let report = audit.finish().unwrap();
    assert!(report.may_release());
    assert!(report.blocking_reason.is_none());
    assert_eq!(report.auditor, "site-b");
    assert_ne!(report.auditor, report.produced_by);
}
