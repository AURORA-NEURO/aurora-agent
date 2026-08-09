//! Executable postconditions on the generator itself.
//!
//! Blueprint 32.22: *"Define executable relations between parent and descendant outputs instead of
//! trusting generated labels."* `bioprism-mutation` applies that to the oracle's verdict. This
//! module applies it one level lower, to the perturbation: before anyone asks what a decision
//! procedure did under stress, the stress must prove it did what it said it would.
//!
//! The distinction matters because the two failures look identical from downstream. If a
//! conclusion moves under assay degradation, either the conclusion was fragile — the finding — or
//! the noise generator quietly shifted the class mean, in which case the "finding" is an artefact
//! of a broken generator. 32.03 names that exact failure, and 32.07 names its sampling twin
//! (*"depth changes class prevalence through filtering"*). A stress whose postconditions fail is
//! quarantined as a defective generator, exactly as a mutation whose relation fails is rejected
//! rather than shipped.
//!
//! Class statistics here are **unweighted and computed over every subject, resolved or not**. That
//! is deliberate and it is not the same convention [`crate::cohort::Cohort`] uses for analysis.
//! The postcondition asks what the generator did to the measurements; weighting belongs to the
//! sampling family, and excluding censored subjects would let a generator hide a mean shift behind
//! its own limit of detection.

use crate::cohort::Cohort;
use serde::{Deserialize, Serialize};

/// The outcome of running a postcondition.
///
/// Mirrors `bioprism-mutation`'s result type on purpose: a violation must carry what was expected
/// and what happened, because "postcondition failed" is not a diagnosis.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum PostconditionResult {
    Held,
    Violated { expected: String, observed: String },
}

impl PostconditionResult {
    pub fn held(&self) -> bool {
        matches!(self, PostconditionResult::Held)
    }

    pub fn violated(expected: impl Into<String>, observed: impl Into<String>) -> Self {
        PostconditionResult::Violated {
            expected: expected.into(),
            observed: observed.into(),
        }
    }
}

/// What a stress promises about the cohort it produced.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "invariant", rename_all = "snake_case")]
pub enum CohortInvariant {
    /// Every subject id survives, exactly once, in order.
    SubjectSetUnchanged,
    /// No subject's latent truth was edited. The line between stress and a different benchmark.
    LatentTruthUnchanged,
    /// No measurement was touched. What lets prevalence shift claim it changed only composition.
    MeasurementsUnchanged,
    /// No sampling weight was touched.
    WeightsUnchanged,
    /// No subject changed batch.
    BatchMembershipUnchanged,
    /// No segmentation-derived volume was touched.
    VolumesUnchanged,
    /// Weighted base rate reached the declared target.
    PrevalenceMovedTo { target: f64, tolerance: f64 },
    /// Neither class mean moved. The postcondition that makes assay degradation attributable.
    ClassMeansUnchanged { tolerance: f64 },
    /// Within-class spread was multiplied by exactly this factor.
    DispersionScaledBy { factor: f64, tolerance: f64 },
    /// Exactly the named batch moved, by exactly the declared offset.
    OffsetConfinedToBatch {
        batch: String,
        offset: f64,
        tolerance: f64,
    },
    /// No volume moved further than the segmentation's own stated reproducibility.
    VolumesWithinReproducibility { cv: f64 },
    /// Everything the assay could no longer measure is marked unresolved, and nothing else is.
    NonDetectionMarkedUnresolved { limit: f64 },
}

fn class_markers(cohort: &Cohort, condition: bool) -> Vec<f64> {
    cohort
        .subjects
        .iter()
        .filter(|subject| subject.condition == condition)
        .map(|subject| subject.marker)
        .collect()
}

fn sample_mean(values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        None
    } else {
        Some(values.iter().sum::<f64>() / values.len() as f64)
    }
}

fn sample_sd(values: &[f64]) -> Option<f64> {
    let mean = sample_mean(values)?;
    let variance = values
        .iter()
        .map(|value| (value - mean) * (value - mean))
        .sum::<f64>()
        / values.len() as f64;
    Some(variance.sqrt())
}

fn class_name(condition: bool) -> &'static str {
    if condition {
        "positive"
    } else {
        "negative"
    }
}

impl CohortInvariant {
    /// A stable label, so a profile can name the postcondition that failed.
    pub fn label(&self) -> &'static str {
        match self {
            CohortInvariant::SubjectSetUnchanged => "subject_set_unchanged",
            CohortInvariant::LatentTruthUnchanged => "latent_truth_unchanged",
            CohortInvariant::MeasurementsUnchanged => "measurements_unchanged",
            CohortInvariant::WeightsUnchanged => "weights_unchanged",
            CohortInvariant::BatchMembershipUnchanged => "batch_membership_unchanged",
            CohortInvariant::VolumesUnchanged => "volumes_unchanged",
            CohortInvariant::PrevalenceMovedTo { .. } => "prevalence_moved_to",
            CohortInvariant::ClassMeansUnchanged { .. } => "class_means_unchanged",
            CohortInvariant::DispersionScaledBy { .. } => "dispersion_scaled_by",
            CohortInvariant::OffsetConfinedToBatch { .. } => "offset_confined_to_batch",
            CohortInvariant::VolumesWithinReproducibility { .. } => {
                "volumes_within_reproducibility"
            }
            CohortInvariant::NonDetectionMarkedUnresolved { .. } => {
                "non_detection_marked_unresolved"
            }
        }
    }

    /// Runs the postcondition against the cohort before and after the stress.
    pub fn check(&self, before: &Cohort, after: &Cohort) -> PostconditionResult {
        if before.len() != after.len() {
            return PostconditionResult::violated(
                format!("{} subjects", before.len()),
                format!("{} subjects", after.len()),
            );
        }
        let paired = || before.subjects.iter().zip(after.subjects.iter());

        match self {
            CohortInvariant::SubjectSetUnchanged => {
                match paired().find(|(before, after)| before.id != after.id) {
                    None => PostconditionResult::Held,
                    Some((before, after)) => PostconditionResult::violated(
                        format!("subject {} in position", before.id),
                        format!("subject {} in position", after.id),
                    ),
                }
            }
            CohortInvariant::LatentTruthUnchanged => {
                match paired().find(|(before, after)| before.condition != after.condition) {
                    None => PostconditionResult::Held,
                    Some((before, after)) => PostconditionResult::violated(
                        format!("{} is {}", before.id, class_name(before.condition)),
                        format!("{} is {}", after.id, class_name(after.condition)),
                    ),
                }
            }
            CohortInvariant::MeasurementsUnchanged => {
                match paired().find(|(before, after)| before.marker != after.marker) {
                    None => PostconditionResult::Held,
                    Some((before, after)) => PostconditionResult::violated(
                        format!("{} marker {}", before.id, before.marker),
                        format!("{} marker {}", after.id, after.marker),
                    ),
                }
            }
            CohortInvariant::WeightsUnchanged => {
                match paired().find(|(before, after)| before.weight != after.weight) {
                    None => PostconditionResult::Held,
                    Some((before, after)) => PostconditionResult::violated(
                        format!("{} weight {}", before.id, before.weight),
                        format!("{} weight {}", after.id, after.weight),
                    ),
                }
            }
            CohortInvariant::BatchMembershipUnchanged => {
                match paired().find(|(before, after)| before.batch != after.batch) {
                    None => PostconditionResult::Held,
                    Some((before, after)) => PostconditionResult::violated(
                        format!("{} in batch {}", before.id, before.batch),
                        format!("{} in batch {}", after.id, after.batch),
                    ),
                }
            }
            CohortInvariant::VolumesUnchanged => {
                match paired().find(|(before, after)| before.volume_mm3 != after.volume_mm3) {
                    None => PostconditionResult::Held,
                    Some((before, after)) => PostconditionResult::violated(
                        format!("{} volume {}", before.id, before.volume_mm3),
                        format!("{} volume {}", after.id, after.volume_mm3),
                    ),
                }
            }
            CohortInvariant::PrevalenceMovedTo { target, tolerance } => {
                let observed = after.prevalence();
                if (observed - target).abs() <= *tolerance {
                    PostconditionResult::Held
                } else {
                    PostconditionResult::violated(
                        format!("prevalence {target:.6}"),
                        format!("prevalence {observed:.6}"),
                    )
                }
            }
            CohortInvariant::ClassMeansUnchanged { tolerance } => {
                for condition in [true, false] {
                    let start = sample_mean(&class_markers(before, condition));
                    let end = sample_mean(&class_markers(after, condition));
                    match (start, end) {
                        (Some(start), Some(end)) if (start - end).abs() <= *tolerance => {}
                        (Some(start), Some(end)) => {
                            return PostconditionResult::violated(
                                format!("{} mean {start:.12}", class_name(condition)),
                                format!("{} mean {end:.12}", class_name(condition)),
                            );
                        }
                        _ => {
                            return PostconditionResult::violated(
                                format!("{} class present", class_name(condition)),
                                format!("{} class empty", class_name(condition)),
                            );
                        }
                    }
                }
                PostconditionResult::Held
            }
            CohortInvariant::DispersionScaledBy { factor, tolerance } => {
                for condition in [true, false] {
                    let start = sample_sd(&class_markers(before, condition));
                    let end = sample_sd(&class_markers(after, condition));
                    match (start, end) {
                        (Some(start), Some(end)) => {
                            let expected = start * factor;
                            if (end - expected).abs() > *tolerance {
                                return PostconditionResult::violated(
                                    format!(
                                        "{} spread {expected:.12} ({start:.12} x {factor:.6})",
                                        class_name(condition)
                                    ),
                                    format!("{} spread {end:.12}", class_name(condition)),
                                );
                            }
                        }
                        _ => {
                            return PostconditionResult::violated(
                                format!("{} class present", class_name(condition)),
                                format!("{} class empty", class_name(condition)),
                            );
                        }
                    }
                }
                PostconditionResult::Held
            }
            CohortInvariant::OffsetConfinedToBatch {
                batch,
                offset,
                tolerance,
            } => {
                for (before, after) in paired() {
                    let expected = if &before.batch == batch { *offset } else { 0.0 };
                    let observed = after.marker - before.marker;
                    if (observed - expected).abs() > *tolerance {
                        return PostconditionResult::violated(
                            format!("{} shifted by {expected:.12}", before.id),
                            format!("{} shifted by {observed:.12}", before.id),
                        );
                    }
                }
                PostconditionResult::Held
            }
            CohortInvariant::VolumesWithinReproducibility { cv } => {
                for (before, after) in paired() {
                    let bound = cv * before.volume_mm3;
                    let moved = (after.volume_mm3 - before.volume_mm3).abs();
                    if moved > bound + f64::EPSILON * before.volume_mm3 {
                        return PostconditionResult::violated(
                            format!("{} moved at most {bound:.9} mm3", before.id),
                            format!("{} moved {moved:.9} mm3", before.id),
                        );
                    }
                }
                PostconditionResult::Held
            }
            CohortInvariant::NonDetectionMarkedUnresolved { limit } => {
                for subject in &after.subjects {
                    let detectable = subject.marker >= *limit;
                    if detectable != subject.resolved {
                        return PostconditionResult::violated(
                            format!(
                                "{} marker {:.6} against limit {limit:.6} implies resolved={}",
                                subject.id, subject.marker, detectable
                            ),
                            format!("{} marked resolved={}", subject.id, subject.resolved),
                        );
                    }
                }
                PostconditionResult::Held
            }
        }
    }
}

/// One postcondition and what happened when it was run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InvariantCheck {
    pub invariant: CohortInvariant,
    pub result: PostconditionResult,
}

impl InvariantCheck {
    pub fn held(&self) -> bool {
        self.result.held()
    }
}
