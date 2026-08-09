//! Invariants of 06.13 calibration and diversity, and 06.14 dedup and contamination.
//!
//! Two claims dominate: renaming is not a contribution to benchmark scale, and instance count is
//! not benchmark count.

use bioprism_benchcompiler::calibrate::{
    calibrate, effective_diversity, BenchInstance, CalibrationVerdict, CapabilityTier,
    DifficultyEstimate, PanelRun,
};
use bioprism_benchcompiler::dedup::{
    assess_contamination, assign_holdout, content_fingerprint, deduplicate, structural_fingerprint,
    ContaminationRisk, DuplicateLayer, ExposureLedger, Instance, LeakChannel, LeakProbe,
};
use serde_json::json;
use std::collections::BTreeSet;

fn ids(names: &[&str]) -> BTreeSet<String> {
    names.iter().map(|name| (*name).to_string()).collect()
}

#[test]
fn renaming_an_instance_does_not_change_what_it_tests() {
    let left = Instance::new("bench_alpha", json!({"facts": [{"of": "SAMPLE_A"}]})).accepting("invalid");
    let right = Instance::new("bench_omega", json!({"facts": [{"of": "SAMPLE_A"}]})).accepting("invalid");

    assert_eq!(content_fingerprint(&left), content_fingerprint(&right));
    let report = deduplicate(&[left, right]);
    assert_eq!(report.distinct, 1);
    assert_eq!(report.groups[0].layer, DuplicateLayer::Content);
    assert_eq!(report.removed(), vec!["bench_omega".to_string()]);
}

#[test]
fn renaming_identifiers_inside_the_content_does_not_defeat_deduplication() {
    let original = Instance::new(
        "bench_one",
        json!({"facts": [{"subject": "SAMPLE_A", "split": "train"},
                         {"subject": "SAMPLE_B", "split": "test"}]}),
    )
    .accepting("invalid")
    .naming("SAMPLE_A")
    .naming("SAMPLE_B");
    let renamed = Instance::new(
        "bench_two",
        json!({"facts": [{"subject": "specimen_q", "split": "train"},
                         {"subject": "specimen_r", "split": "test"}]}),
    )
    .accepting("invalid")
    .naming("specimen_q")
    .naming("specimen_r");

    assert_ne!(
        content_fingerprint(&original),
        content_fingerprint(&renamed),
        "the bytes really do differ; the structural layer is what has to catch this"
    );
    assert_eq!(
        structural_fingerprint(&original),
        structural_fingerprint(&renamed)
    );

    let report = deduplicate(&[original, renamed]);
    assert_eq!(report.distinct, 1);
    assert!(report
        .groups
        .iter()
        .any(|group| group.layer == DuplicateLayer::Structural));
}

#[test]
fn two_instances_that_differ_semantically_are_not_merged_by_the_structural_layer() {
    let train_test = Instance::new(
        "bench_one",
        json!({"facts": [{"subject": "SAMPLE_A", "split": "train"}]}),
    )
    .accepting("invalid")
    .naming("SAMPLE_A");
    let test_only = Instance::new(
        "bench_two",
        json!({"facts": [{"subject": "SAMPLE_A", "split": "test"}]}),
    )
    .accepting("invalid")
    .naming("SAMPLE_A");

    assert_ne!(
        structural_fingerprint(&train_test),
        structural_fingerprint(&test_only),
        "`split` is meaning, not a name; a false duplicate would delete evidence"
    );
    assert_eq!(deduplicate(&[train_test, test_only]).distinct, 2);
}

#[test]
fn oracle_equivalent_instances_are_flagged_for_review_rather_than_removed() {
    let left = Instance::new("bench_one", json!({"facts": [{"of": "A"}]}))
        .accepting("invalid")
        .requiring_witness("identity_leakage");
    let right = Instance::new("bench_two", json!({"facts": [{"of": "B"}]}))
        .accepting("invalid")
        .requiring_witness("identity_leakage");

    let report = deduplicate(&[left, right]);
    assert_eq!(report.distinct, 2);
    assert!(report
        .groups
        .iter()
        .any(|group| group.layer == DuplicateLayer::OracleEquivalent));
    assert!(
        report.removed().is_empty(),
        "06.09's contrastive pairs share an oracle contract on purpose"
    );
}

#[test]
fn holdout_assignment_is_reproducible_on_every_run() {
    let instance = Instance::new("bench_one", json!({"facts": [{"of": "A"}]})).accepting("valid");
    let first = assign_holdout(&instance, 20, 4);
    for _ in 0..8 {
        assert_eq!(assign_holdout(&instance, 20, 4), first);
    }
}

#[test]
fn a_leak_probe_that_solves_the_instance_outranks_a_clean_ledger() {
    let instance = Instance::new("bench_one", json!({"facts": []})).accepting("valid");
    let ledger = ExposureLedger {
        assessed: true,
        ..ExposureLedger::default()
    };
    let probes = vec![LeakProbe::new(
        LeakChannel::FilenameOnly,
        true,
        "the filename contains the answer",
    )];

    let report = assess_contamination(&instance, &ledger, &probes);
    assert!(matches!(
        report.risk,
        ContaminationRisk::LeaksThroughChannel {
            channel: LeakChannel::FilenameOnly,
            ..
        }
    ));
    assert!(!report.risk.admissible());
}

#[test]
fn an_unassessed_exposure_ledger_is_not_reported_as_clean() {
    let instance = Instance::new("bench_one", json!({"facts": []})).accepting("valid");
    let report = assess_contamination(&instance, &ExposureLedger::default(), &[]);
    assert_eq!(report.risk, ContaminationRisk::Unassessed);
    assert!(!report.risk.admissible());
}

#[test]
fn a_published_instance_nobody_probed_is_not_clean_either() {
    let instance = Instance::new("bench_one", json!({"facts": []})).accepting("valid");
    let ledger = ExposureLedger {
        published: true,
        assessed: true,
        first_published: Some("2026-02-14".to_string()),
        ..ExposureLedger::default()
    };
    assert_eq!(
        assess_contamination(&instance, &ledger, &[]).risk,
        ContaminationRisk::PublishedAndUnprobed
    );
}

#[test]
fn effective_diversity_headlines_equivalence_classes_not_instance_count() {
    let instances: Vec<BenchInstance> = (0..500)
        .map(|index| BenchInstance {
            instance_id: format!("inst_{index:04}"),
            parent_digest: "sha_parent".to_string(),
            mutation_family: "paraphrase".to_string(),
            oracle_signature: "invalid|identity_leakage".to_string(),
        })
        .collect();

    let diversity = effective_diversity(&instances);
    assert_eq!(diversity.instances, 500);
    assert_eq!(diversity.equivalence_classes, 1);
    assert_eq!(diversity.effective_sample_size(), 1);
    assert!((diversity.inflation_ratio - 500.0).abs() < 1e-9);
    assert!(!diversity.is_publishable());
    assert!(diversity
        .headline()
        .contains("Instance count is not benchmark count"));
}

#[test]
fn independent_families_raise_the_class_count_and_paraphrases_do_not() {
    let mut instances = Vec::new();
    for family in ["paraphrase", "reorder", "distractor"] {
        for index in 0..10 {
            instances.push(BenchInstance {
                instance_id: format!("{family}_{index}"),
                parent_digest: "sha_parent".to_string(),
                mutation_family: family.to_string(),
                oracle_signature: "invalid|identity_leakage".to_string(),
            });
        }
    }
    let diversity = effective_diversity(&instances);
    assert_eq!(diversity.instances, 30);
    assert_eq!(diversity.equivalence_classes, 3);
    assert!(diversity.is_publishable());
}

#[test]
fn an_instance_no_panel_ran_is_unmeasured_not_failed() {
    let calibration = calibrate(&[], &ids(&["inst_a"]), &BTreeSet::new());
    let instance = &calibration.instances[0];
    assert_eq!(instance.verdict, CalibrationVerdict::Unmeasured);
    assert_eq!(instance.estimate.success_rate(), None);
    assert!(matches!(
        instance.estimate,
        DifficultyEstimate::Unmeasured { .. }
    ));
    assert_eq!(calibration.unmeasured, 1);
}

#[test]
fn an_instance_a_rule_based_policy_solves_is_flagged_as_a_trivial_cue() {
    let runs = vec![
        PanelRun::new("inst_a", "grep_baseline", CapabilityTier::RuleBased, true),
        PanelRun::new("inst_a", "frontier", CapabilityTier::Strong, true),
    ];
    let calibration = calibrate(&runs, &ids(&["inst_a"]), &BTreeSet::new());
    assert_eq!(
        calibration.instances[0].verdict,
        CalibrationVerdict::TrivialCue {
            solved_by: "grep_baseline".to_string()
        }
    );
    assert!(calibration.informative().is_empty());
}

#[test]
fn a_safety_veto_stays_informative_even_though_everyone_passes_it() {
    let runs = vec![
        PanelRun::new("inst_veto", "frontier", CapabilityTier::Strong, true),
        PanelRun::new("inst_veto", "small", CapabilityTier::Baseline, true),
    ];
    let calibration = calibrate(&runs, &ids(&["inst_veto"]), &ids(&["inst_veto"]));
    assert_eq!(calibration.instances[0].verdict, CalibrationVerdict::SafetyVeto);
    assert_eq!(calibration.informative().len(), 1);
}

#[test]
fn discrimination_is_absent_rather_than_zero_when_one_tier_never_ran() {
    let runs = vec![
        PanelRun::new("inst_a", "frontier", CapabilityTier::Strong, true),
        PanelRun::new("inst_a", "small", CapabilityTier::Baseline, false),
    ];
    let calibration = calibrate(&runs, &ids(&["inst_a"]), &BTreeSet::new());
    match &calibration.instances[0].estimate {
        DifficultyEstimate::Measured { discrimination, .. } => assert_eq!(*discrimination, None),
        other => panic!("expected a measurement, got {other:?}"),
    }
    assert_eq!(
        calibration.instances[0].verdict,
        CalibrationVerdict::Discriminating
    );
}

#[test]
fn an_instance_every_architecture_failed_is_flagged_for_repair_not_celebrated() {
    let runs = vec![
        PanelRun::new("inst_a", "frontier", CapabilityTier::Strong, false),
        PanelRun::new("inst_a", "small", CapabilityTier::Weak, false),
    ];
    let calibration = calibrate(&runs, &ids(&["inst_a"]), &BTreeSet::new());
    assert_eq!(
        calibration.instances[0].verdict,
        CalibrationVerdict::UniversallyFailed
    );
    assert!(calibration.informative().is_empty());
}
