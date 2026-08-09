#![allow(dead_code)]

//! Hand-built fixtures.
//!
//! Every number below was written by hand so that nothing in this crate's tests can be mistaken for
//! a measurement. There is no dataset, no fixture pack and no shipped grid, for the reason
//! `bioprism-metrics` gives for the same absence.

use bioprism_atlas::{
    CapabilityId, CausalChain, Detectability, EvidenceStatus, FailureAxes, FailureLabel,
    FailureMechanism, FailureRecord, Inducement, LabelDistribution, Reversibility, Severity,
    UnmeasuredReason,
};
use bioprism_ids::RunId;
use bioprism_metrics::{
    CapabilityGrid, GridCell, MeasurementConditions, NoIntervalReason, ScoringRule, Subject,
};

pub fn capability(name: &str) -> CapabilityId {
    CapabilityId::parse(name).expect("fixture capability id")
}

pub fn conditions(label: &str) -> MeasurementConditions {
    MeasurementConditions::new(Subject::grid(label), ScoringRule::atlas_pass_rate())
}

/// A grid with two measured capabilities and one hole.
pub fn grid(label: &str) -> CapabilityGrid {
    CapabilityGrid::new(label, conditions(label))
        .with_cell(
            capability("identity.lineage"),
            GridCell::point(0.8, NoIntervalReason::EstimatorNotAvailable, 4).expect("fixture cell"),
        )
        .with_cell(
            capability("cohort.statistics"),
            GridCell::point(0.6, NoIntervalReason::EstimatorNotAvailable, 3).expect("fixture cell"),
        )
        .with_cell(
            capability("causal.interpretation"),
            GridCell::unmeasured(UnmeasuredReason::NotAttempted),
        )
}

pub fn grid_with_holes(label: &str, holes: &[(&str, UnmeasuredReason)]) -> CapabilityGrid {
    let mut built = CapabilityGrid::new(label, conditions(label));
    for (name, reason) in holes {
        built = built.with_cell(capability(name), GridCell::unmeasured(*reason));
    }
    built
}

pub fn axes(severity: Severity, inducement: Inducement) -> FailureAxes {
    FailureAxes::new(
        EvidenceStatus::Preserved,
        Reversibility::Reversible,
        Detectability::DetectedByReview,
        severity,
        inducement,
    )
}

/// A four-label chain whose first divergence is at step 2.
pub fn chain(failure_id: &str, mechanism: FailureMechanism) -> CausalChain {
    CausalChain::new(
        failure_id,
        FailureLabel::new(FailureMechanism::RelevantEvidenceNotAcquired, 1),
        FailureLabel::new(mechanism, 2),
        vec![FailureLabel::new(
            FailureMechanism::UncertaintyMisreportedToCaller,
            3,
        )],
        FailureLabel::new(FailureMechanism::SuccessfulCommandMistakenForTaskSuccess, 4),
    )
    .expect("fixture chain")
}

pub const TAXONOMY: &str = "bioprism-failure-taxonomy/0.1";

pub fn record(
    failure_id: &str,
    implicates: &str,
    mechanism: FailureMechanism,
    axes: FailureAxes,
) -> FailureRecord {
    FailureRecord::new(
        failure_id,
        RunId::parse(format!("run-{failure_id}")).expect("fixture run id"),
        capability(implicates),
        TAXONOMY,
        chain(failure_id, mechanism),
        axes,
        LabelDistribution::certain(mechanism, "fixture diagnosis"),
    )
}

/// A record whose reviewers disagree.
pub fn contested_record(failure_id: &str, implicates: &str) -> FailureRecord {
    FailureRecord::new(
        failure_id,
        RunId::parse(format!("run-{failure_id}")).expect("fixture run id"),
        capability(implicates),
        TAXONOMY,
        chain(failure_id, FailureMechanism::StaleEvidenceTrusted),
        axes(Severity::Degraded, Inducement::ModelInduced),
        LabelDistribution::contested(
            failure_id,
            [
                (FailureMechanism::StaleEvidenceTrusted, 0.5),
                (FailureMechanism::HypothesisCollapsedTooEarly, 0.5),
            ],
            "two reviewers, no adjudication",
        )
        .expect("fixture distribution"),
    )
}
