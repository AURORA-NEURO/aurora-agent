//! Reading a `bioprism-atlas` atlas into a grid preserves the atlas's own refusals and adds none
//! of its own.

mod common;

use bioprism_atlas::{
    Atlas, CapabilityDimension, CapabilityFamily, CapabilityNode, CapabilityOntology,
    EvidenceRecord, EvidenceTier, OracleTier, TrialOutcome, UnmeasuredReason,
};
use bioprism_metrics::{
    CapabilityGrid, CoveredAggregate, GatePredicate, GateVerdict, MetricsError, NoIntervalReason,
    ReleaseGate,
};
use common::{cap, recorded};

const VERSION: &str = "bridge-ontology/1";

fn ontology() -> CapabilityOntology {
    CapabilityOntology::from_nodes(
        VERSION,
        vec![
            CapabilityNode::new(
                cap("verify.oracle"),
                "verify a tool result against a deterministic oracle",
                CapabilityFamily::Verification,
                CapabilityDimension::Competence,
            ),
            CapabilityNode::new(
                cap("safety.escalation"),
                "escalate to a human at a clinical boundary",
                CapabilityFamily::PrivacyAndSafety,
                CapabilityDimension::Safety,
            ),
            CapabilityNode::new(
                cap("memory.recall"),
                "recall a prior decision state",
                CapabilityFamily::Memory,
                CapabilityDimension::Reliability,
            ),
        ],
    )
    .expect("valid ontology")
}

fn record(trial: &str, capability: &str, outcome: TrialOutcome) -> EvidenceRecord {
    EvidenceRecord::new(
        trial,
        cap(capability),
        VERSION,
        EvidenceTier::PublicObservedWorld,
        OracleTier::Executable,
        outcome,
    )
}

fn atlas() -> Atlas {
    Atlas::builder(ontology())
        .evidence(record("t1", "verify.oracle", TrialOutcome::Pass))
        .evidence(record("t2", "verify.oracle", TrialOutcome::Pass))
        .evidence(record("t3", "verify.oracle", TrialOutcome::Fail))
        .evidence(record("t4", "memory.recall", TrialOutcome::Fail))
        .evidence(record("t5", "memory.recall", TrialOutcome::Fail))
        .declare_unmeasured(
            cap("safety.escalation"),
            UnmeasuredReason::DeferredAcquisition,
        )
        .build()
        .expect("valid atlas")
}

#[test]
fn an_atlas_hole_becomes_a_grid_hole_carrying_the_same_reason() {
    let grid = CapabilityGrid::from_atlas("system-a", &atlas(), recorded("system-a"));
    let cell = grid.cell(&cap("safety.escalation")).expect("present");
    assert_eq!(
        cell.unmeasured_reason(),
        Some(UnmeasuredReason::DeferredAcquisition)
    );
    assert!(cell.value().is_none());
}

#[test]
fn a_measured_and_poor_capability_stays_measured_with_a_score_of_zero() {
    let grid = CapabilityGrid::from_atlas("system-a", &atlas(), recorded("system-a"));
    let cell = grid.cell(&cap("memory.recall")).expect("present");
    assert_eq!(cell.value(), Some(0.0));
    assert!(cell.is_measured());
    assert!(cell.unmeasured_reason().is_none());
}

#[test]
fn a_grid_hole_serializes_with_no_numeric_field_for_a_renderer_to_coerce() {
    let grid = CapabilityGrid::from_atlas("system-a", &atlas(), recorded("system-a"));
    let cell = grid.cell(&cap("safety.escalation")).expect("present");
    let encoded = serde_json::to_value(cell).expect("serializable");
    let object = encoded.as_object().expect("object");
    assert_eq!(object["state"], "unmeasured");
    assert!(object.get("estimate").is_none());
    assert!(object.get("effective_size").is_none());
}

#[test]
fn the_grid_takes_its_ontology_version_from_the_atlas_rather_than_from_the_caller() {
    let grid = CapabilityGrid::from_atlas(
        "system-a",
        &atlas(),
        recorded("system-a").with_ontology_version("a-version-the-caller-made-up"),
    );
    assert_eq!(
        grid.conditions
            .ontology_version
            .recorded_value()
            .map(String::as_str),
        Some(VERSION)
    );
}

#[test]
fn a_grid_from_an_atlas_carries_no_intervals_and_states_why() {
    let grid = CapabilityGrid::from_atlas("system-a", &atlas(), recorded("system-a"));
    let cell = grid.cell(&cap("verify.oracle")).expect("present");
    assert!(cell.interval().is_none());
    assert_eq!(
        cell.estimate()
            .and_then(bioprism_metrics::Estimate::no_interval_reason),
        Some(NoIntervalReason::EstimatorNotAvailable)
    );
}

#[test]
fn an_atlas_measurement_of_one_evaluable_trial_states_single_trial_as_its_reason() {
    let atlas = Atlas::builder(ontology())
        .evidence(record("t1", "verify.oracle", TrialOutcome::Pass))
        .build()
        .expect("valid atlas");
    let grid = CapabilityGrid::from_atlas("system-a", &atlas, recorded("system-a"));
    assert_eq!(
        grid.cell(&cap("verify.oracle"))
            .and_then(bioprism_metrics::GridCell::estimate)
            .and_then(bioprism_metrics::Estimate::no_interval_reason),
        Some(NoIntervalReason::SingleTrial)
    );
}

#[test]
fn an_uncertainty_gate_over_an_atlas_grid_is_unevaluable_which_is_the_truthful_verdict() {
    let grid = CapabilityGrid::from_atlas("system-a", &atlas(), recorded("system-a"));
    let gate = ReleaseGate::new("uncertainty").requiring(GatePredicate::MaximumIntervalWidth {
        capability: cap("verify.oracle"),
        ceiling: 0.05,
    });

    let report = gate.evaluate(&grid);
    assert_eq!(report.verdict, GateVerdict::NotEvaluable);
    assert!(!report.verdict.permits_release());
}

#[test]
fn a_grid_from_an_atlas_with_a_hole_cannot_produce_a_complete_mean() {
    let grid = CapabilityGrid::from_atlas("system-a", &atlas(), recorded("system-a"));
    match CoveredAggregate::complete_mean(&grid) {
        Err(MetricsError::IncompleteGrid { unmeasured, .. }) => {
            assert_eq!(unmeasured, vec!["safety.escalation".to_string()]);
        }
        other => panic!("expected a refusal naming the hole, got {other:?}"),
    }
}

#[test]
fn a_mean_over_an_atlas_grid_reports_the_hole_alongside_the_number() {
    let grid = CapabilityGrid::from_atlas("system-a", &atlas(), recorded("system-a"));
    let aggregate = CoveredAggregate::mean(&grid).expect("two measured cells");
    assert!((aggregate.value().get() - (2.0 / 3.0 + 0.0) / 2.0).abs() < 1e-9);
    assert_eq!(aggregate.coverage().blocking_holes.len(), 1);
    assert_eq!(aggregate.coverage().contributed.len(), 2);
    assert_eq!(aggregate.coverage().cells, 3);
}

#[test]
fn an_atlas_grid_restricted_to_a_subset_becomes_a_different_subject() {
    let grid = CapabilityGrid::from_atlas("system-a", &atlas(), recorded("system-a"));
    let restricted = grid.restricted_to("system-a/verification", &[cap("verify.oracle")]);

    assert_eq!(restricted.len(), 1);
    assert!(bioprism_metrics::comparable(&grid.conditions, &restricted.conditions).is_err());
}

#[test]
fn the_grid_effective_size_is_the_atlas_clustering_unit_not_the_trial_count() {
    let atlas = Atlas::builder(ontology())
        .evidence_all((0..50).map(|index| {
            record(&format!("t{index}"), "verify.oracle", TrialOutcome::Pass)
                .with_parent_world(bioprism_ids::WorldId::parse("world-1").expect("valid world"))
                .generated_instance()
        }))
        .build()
        .expect("valid atlas");
    let grid = CapabilityGrid::from_atlas("system-a", &atlas, recorded("system-a"));

    assert_eq!(
        grid.cell(&cap("verify.oracle"))
            .and_then(bioprism_metrics::GridCell::effective_size),
        Some(1)
    );

    let gate = ReleaseGate::new("independence").requiring(GatePredicate::MinimumEffectiveSize {
        capability: cap("verify.oracle"),
        floor: 30,
    });
    assert_eq!(gate.evaluate(&grid).verdict, GateVerdict::Blocked);
}
