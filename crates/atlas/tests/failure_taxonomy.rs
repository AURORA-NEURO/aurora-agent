//! Invariants of the failure taxonomy, blueprint 03.10 and 33.18.
//!
//! The claims under test: the mechanism set is closed and its escape hatch is itself reported;
//! a causal chain cannot be ordered so that the divergence follows what it explains; reviewer
//! disagreement survives; and a failure that names no known capability is refused.

use bioprism_atlas::{
    Atlas, AtlasError, CapabilityDimension, CapabilityFamily, CapabilityId, CapabilityNode,
    CapabilityOntology, CausalChain, Detectability, EvidenceRecord, EvidenceStatus, EvidenceTier,
    FailureAxes, FailureLabel, FailureMechanism, FailureRecord, FailureStage, Inconsistency,
    Inducement, LabelDistribution, OracleTier, Reversibility, Severity, TrialOutcome,
};
use bioprism_ids::RunId;

const VERSION: &str = "capability-ontology/2026-08-07";

fn cap(id: &str) -> CapabilityId {
    CapabilityId::parse(id).expect("valid capability identifier")
}

fn ontology() -> CapabilityOntology {
    CapabilityOntology::from_nodes(
        VERSION,
        [CapabilityNode::new(
            cap("analysis"),
            "executable analysis",
            CapabilityFamily::ToolUse,
            CapabilityDimension::Competence,
        )],
    )
    .unwrap()
}

fn axes() -> FailureAxes {
    FailureAxes::new(
        EvidenceStatus::Preserved,
        Reversibility::Reversible,
        Detectability::DetectedByDeterministicCheck,
        Severity::WrongConclusion,
        Inducement::ModelInduced,
    )
}

fn chain(id: &str) -> CausalChain {
    CausalChain::new(
        id,
        FailureLabel::new(FailureMechanism::RelevantEvidenceNotAcquired, 2),
        FailureLabel::new(FailureMechanism::HypothesisCollapsedTooEarly, 4),
        vec![FailureLabel::new(
            FailureMechanism::ToolSchemaMisunderstood,
            7,
        )],
        FailureLabel::new(
            FailureMechanism::SuccessfulCommandMistakenForTaskSuccess,
            11,
        ),
    )
    .expect("an ordered chain")
}

fn failure(id: &str, capability: &str, labels: LabelDistribution) -> FailureRecord {
    FailureRecord::new(
        id,
        RunId::parse("run-1").unwrap(),
        cap(capability),
        VERSION,
        chain(id),
        axes(),
        labels,
    )
}

fn failed_trial(trial: &str) -> EvidenceRecord {
    EvidenceRecord::new(
        trial,
        cap("analysis"),
        VERSION,
        EvidenceTier::PublicObservedWorld,
        OracleTier::Deterministic,
        TrialOutcome::Fail,
    )
}

#[test]
fn a_failure_record_naming_a_capability_outside_the_ontology_is_refused() {
    let built = Atlas::builder(ontology())
        .failure(failure(
            "f1",
            "analysis",
            LabelDistribution::certain(FailureMechanism::ToolSchemaMisunderstood, "schema drift"),
        ))
        .build();
    assert!(built.is_ok());

    let mut ontology = CapabilityOntology::new(VERSION);
    ontology
        .insert(CapabilityNode::new(
            cap("other"),
            "other",
            CapabilityFamily::Memory,
            CapabilityDimension::Competence,
        ))
        .unwrap();
    let orphaned = Atlas::builder(ontology)
        .failure(failure(
            "f2",
            "analysis",
            LabelDistribution::certain(FailureMechanism::ToolSchemaMisunderstood, "schema drift"),
        ))
        .build();
    assert!(matches!(
        orphaned,
        Err(AtlasError::UnknownCapability { .. })
    ));
}

#[test]
fn a_causal_chain_whose_terminal_failure_precedes_its_first_divergence_is_refused() {
    let built = CausalChain::new(
        "f3",
        FailureLabel::new(FailureMechanism::RelevantEvidenceNotAcquired, 1),
        FailureLabel::new(FailureMechanism::HypothesisCollapsedTooEarly, 9),
        Vec::new(),
        FailureLabel::new(FailureMechanism::SafetyBoundaryCrossed, 4),
    );
    assert!(matches!(built, Err(AtlasError::ChainOutOfOrder { .. })));
}

#[test]
fn a_manifestation_before_the_first_divergence_is_refused() {
    let built = CausalChain::new(
        "f4",
        FailureLabel::new(FailureMechanism::RelevantEvidenceNotAcquired, 1),
        FailureLabel::new(FailureMechanism::HypothesisCollapsedTooEarly, 5),
        vec![FailureLabel::new(
            FailureMechanism::ToolSchemaMisunderstood,
            3,
        )],
        FailureLabel::new(FailureMechanism::SafetyBoundaryCrossed, 9),
    );
    assert!(matches!(built, Err(AtlasError::ChainOutOfOrder { .. })));
}

#[test]
fn an_initiating_cause_after_the_divergence_it_caused_is_refused() {
    let built = CausalChain::new(
        "f5",
        FailureLabel::new(FailureMechanism::RelevantEvidenceNotAcquired, 8),
        FailureLabel::new(FailureMechanism::HypothesisCollapsedTooEarly, 2),
        Vec::new(),
        FailureLabel::new(FailureMechanism::SafetyBoundaryCrossed, 9),
    );
    assert!(matches!(built, Err(AtlasError::ChainOutOfOrder { .. })));
}

#[test]
fn the_stage_of_a_failure_is_derived_from_its_mechanism_and_cannot_disagree_with_it() {
    for mechanism in FailureMechanism::CLOSED_SET {
        let label = FailureLabel::new(mechanism, 0);
        assert_eq!(label.stage(), mechanism.stage());
        if mechanism.is_classified() {
            assert!(label.stage().is_some(), "{mechanism} should localise");
        } else {
            assert_eq!(label.stage(), None);
        }
    }
    assert_eq!(
        FailureMechanism::ToolSchemaMisunderstood.stage(),
        Some(FailureStage::Tools)
    );
}

#[test]
fn an_unclassified_failure_is_reported_as_taxonomy_debt_rather_than_silently_bucketed() {
    let atlas = Atlas::builder(ontology())
        .evidence(failed_trial("t1"))
        .failure(failure(
            "f6",
            "analysis",
            LabelDistribution::certain(FailureMechanism::Unclassified, "reviewers found no fit"),
        ))
        .build()
        .unwrap();

    assert!(atlas.inconsistencies().iter().any(|i| matches!(
        i,
        Inconsistency::UnclassifiedFailure { .. }
    )));
    assert_eq!(
        bioprism_atlas::CoverageReport::of(&atlas)
            .debt
            .unclassified_failures,
        1
    );
}

#[test]
fn reviewer_disagreement_is_retained_rather_than_collapsed_to_a_majority_label() {
    let contested = LabelDistribution::contested(
        "f7",
        [
            (FailureMechanism::ToolSchemaMisunderstood, 0.5),
            (FailureMechanism::StaleEvidenceTrusted, 0.5),
        ],
        "two reviewers, two readings of the trace",
    )
    .unwrap();

    assert!(contested.is_contested());
    assert_eq!(contested.modal(), None);
    assert_eq!(
        contested.weight_of(FailureMechanism::StaleEvidenceTrusted),
        0.5
    );

    let atlas = Atlas::builder(ontology())
        .evidence(failed_trial("t1"))
        .failure(failure("f7", "analysis", contested))
        .build()
        .unwrap();
    assert!(atlas
        .inconsistencies()
        .iter()
        .any(|i| matches!(i, Inconsistency::ContestedDiagnosis { .. })));
}

#[test]
fn a_clear_majority_still_yields_a_modal_mechanism() {
    let leaning = LabelDistribution::contested(
        "f8",
        [
            (FailureMechanism::ToolSchemaMisunderstood, 0.75),
            (FailureMechanism::StaleEvidenceTrusted, 0.25),
        ],
        "three of four reviewers agreed",
    )
    .unwrap();
    assert_eq!(
        leaning.modal(),
        Some(FailureMechanism::ToolSchemaMisunderstood)
    );
}

#[test]
fn a_label_distribution_whose_weights_do_not_sum_to_one_is_refused() {
    assert!(matches!(
        LabelDistribution::contested(
            "f9",
            [
                (FailureMechanism::ToolSchemaMisunderstood, 0.5),
                (FailureMechanism::StaleEvidenceTrusted, 0.2),
            ],
            "incomplete"
        ),
        Err(AtlasError::MalformedLabelDistribution { .. })
    ));
    assert!(matches!(
        LabelDistribution::contested(
            "f9",
            [(FailureMechanism::ToolSchemaMisunderstood, 0.0)],
            "zero weight"
        ),
        Err(AtlasError::MalformedLabelWeight { .. })
    ));
    assert!(matches!(
        LabelDistribution::contested("f9", [], "nothing"),
        Err(AtlasError::EmptyLabelDistribution { .. })
    ));
}

#[test]
fn a_flattened_causal_chain_is_recorded_but_does_not_count_as_a_diagnosis() {
    let flat = CausalChain::new(
        "f10",
        FailureLabel::new(FailureMechanism::SafetyBoundaryCrossed, 3),
        FailureLabel::new(FailureMechanism::SafetyBoundaryCrossed, 3),
        Vec::new(),
        FailureLabel::new(FailureMechanism::SafetyBoundaryCrossed, 3),
    )
    .unwrap();
    assert!(flat.is_flattened());

    let record = FailureRecord::new(
        "f10",
        RunId::parse("run-1").unwrap(),
        cap("analysis"),
        VERSION,
        flat,
        axes(),
        LabelDistribution::certain(FailureMechanism::SafetyBoundaryCrossed, "one label, one step"),
    );
    assert!(!record.is_diagnosed());
}

#[test]
fn a_failure_whose_evidence_was_lost_is_not_counted_as_diagnosed() {
    let record = FailureRecord::new(
        "f11",
        RunId::parse("run-1").unwrap(),
        cap("analysis"),
        VERSION,
        chain("f11"),
        FailureAxes::new(
            EvidenceStatus::Lost,
            Reversibility::Irreversible,
            Detectability::Undetected,
            Severity::UnsafeAction,
            Inducement::ModelInduced,
        ),
        LabelDistribution::certain(FailureMechanism::SafetyBoundaryCrossed, "trace gone"),
    );
    assert!(!record.is_diagnosed());
}

#[test]
fn an_evaluator_induced_failure_is_not_charged_to_the_system_under_test() {
    let benchmark_defect = FailureAxes::new(
        EvidenceStatus::Preserved,
        Reversibility::Reversible,
        Detectability::DetectedByReview,
        Severity::Degraded,
        Inducement::EvaluatorInduced,
    );
    assert!(!benchmark_defect.charges_the_system());
    assert!(axes().charges_the_system());
}

#[test]
fn the_first_divergence_histogram_localises_failures_by_derived_stage() {
    let atlas = Atlas::builder(ontology())
        .evidence(failed_trial("t1"))
        .evidence(failed_trial("t2"))
        .failure(failure(
            "f12",
            "analysis",
            LabelDistribution::certain(FailureMechanism::HypothesisCollapsedTooEarly, "early"),
        ))
        .failure(failure(
            "f13",
            "analysis",
            LabelDistribution::certain(FailureMechanism::HypothesisCollapsedTooEarly, "early"),
        ))
        .build()
        .unwrap();

    let report = bioprism_atlas::CoverageReport::of(&atlas);
    let reasoning = report
        .first_divergence_histogram
        .iter()
        .find(|s| s.stage == FailureStage::Reasoning)
        .expect("the chain diverges in reasoning");
    assert_eq!(reasoning.count, 2);
    assert_eq!(report.debt.unclassified_failures, 0);
}

#[test]
fn a_causal_chain_reports_how_far_the_error_propagated() {
    let chain = chain("f14");
    assert_eq!(chain.propagation_span(), 7);
    assert_eq!(chain.labels().len(), 4);
    assert!(!chain.is_flattened());
}
