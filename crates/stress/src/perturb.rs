//! Applying a stress, and checking that it did what it declared.
//!
//! The two are separate functions on purpose, following `bioprism-mutation`: [`apply`] never
//! evaluates its own postconditions, because a transformation that marks its own homework will
//! always pass. [`validate`] runs them, and [`perturb`] does both and hands back a
//! [`Perturbed`] carrying the evidence.
//!
//! Each family's implementation is one paragraph of arithmetic and one paragraph of restraint.
//! What each deliberately does *not* do is the interesting part:
//!
//! - Prevalence shift does not resample. 32.07's failure risk *"repeated samples inflate n"* is
//!   avoided by moving weight instead of copying subjects, and the price is paid openly through
//!   [`crate::cohort::Cohort::effective_n`].
//! - Batch effect does not touch the latent truth, and does not rebalance batches. If batch and
//!   condition are confounded in the parent cohort they stay confounded, and
//!   [`crate::profile`] reports the result as non-identifiable rather than as a pass.
//! - Assay degradation does not draw independent noise. Independent noise moves the class mean by
//!   `O(σ/√n)`, which is exactly the artefact 32.03 warns about; the noise here is projected onto
//!   the orthogonal complement of the signal so the mean cannot move at all and the spread scales
//!   by exactly the declared factor.
//! - Segmentation jitter does not draw from an unbounded distribution. The bound is what lets the
//!   postcondition "nothing moved further than the segmentation's stated reproducibility" be an
//!   arithmetic fact instead of a confidence statement.

use crate::cohort::Cohort;
use crate::error::StressError;
use crate::family::{Knob, Stress};
use crate::invariant::{CohortInvariant, InvariantCheck};
use crate::rng::{centred_orthogonal_noise, StressRng};
use serde::Serialize;

/// Absolute tolerance for postconditions that hold exactly in the reals.
///
/// The constructions below are exact algebraically; the slack is for double rounding only, and is
/// some seven orders of magnitude tighter than any effect a robustness sweep resolves.
pub const ARITHMETIC_TOLERANCE: f64 = 1e-9;

/// A stressed cohort together with the postcondition evidence.
///
/// Fields are private and there is no `Deserialize`, so the only way to hold one is to have run
/// [`perturb`] — which means the checks travelling with the cohort were actually executed against
/// it. A stressed cohort whose postconditions were never run is not representable, which is the
/// same guarantee the workspace makes for a provenance `View`. Use [`apply`] when the unchecked
/// cohort is what you want; that path is explicit rather than reachable by omission.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Perturbed {
    stress: Stress,
    cohort: Cohort,
    checks: Vec<InvariantCheck>,
}

impl Perturbed {
    pub fn stress(&self) -> &Stress {
        &self.stress
    }

    pub fn cohort(&self) -> &Cohort {
        &self.cohort
    }

    pub fn checks(&self) -> &[InvariantCheck] {
        &self.checks
    }

    /// Consumes the evidence to hand back the cohort, so a caller that wants the data must first
    /// have had the checks in hand.
    pub fn into_cohort(self) -> Cohort {
        self.cohort
    }

    /// Whether every declared postcondition held.
    pub fn is_valid(&self) -> bool {
        self.checks.iter().all(InvariantCheck::held)
    }

    /// The postconditions that failed. A non-empty list means the generator is defective and its
    /// output must not be read as a robustness finding.
    pub fn defects(&self) -> Vec<&InvariantCheck> {
        self.checks.iter().filter(|check| !check.held()).collect()
    }
}

/// The base rate this stress is asking for at its magnitude.
///
/// Interpolates from the cohort's own prevalence at magnitude zero to the declared target at
/// magnitude one, so a sweep visits a monotone path rather than jumping.
pub fn interpolated_target(observed: f64, target: f64, fraction: f64) -> f64 {
    observed + fraction * (target - observed)
}

/// The postconditions a stress promises to satisfy on this cohort.
///
/// Computed against the parent because several of them quantify the change — "prevalence reaches
/// 0.041", "spread scales by 2.5" — and those numbers depend on where the cohort started.
pub fn postconditions(
    cohort: &Cohort,
    stress: &Stress,
) -> Result<Vec<CohortInvariant>, StressError> {
    cohort.validate()?;
    let fraction = stress.magnitude.fraction();

    let mut declared = vec![
        CohortInvariant::SubjectSetUnchanged,
        CohortInvariant::LatentTruthUnchanged,
    ];

    match &stress.knob {
        Knob::PrevalenceShift { target_prevalence } => {
            check_prevalence_target(*target_prevalence)?;
            let observed = cohort.prevalence();
            reweighting_mass(cohort)?;
            declared.push(CohortInvariant::MeasurementsUnchanged);
            declared.push(CohortInvariant::VolumesUnchanged);
            declared.push(CohortInvariant::BatchMembershipUnchanged);
            declared.push(CohortInvariant::PrevalenceMovedTo {
                target: interpolated_target(observed, *target_prevalence, fraction),
                tolerance: ARITHMETIC_TOLERANCE,
            });
        }
        Knob::BatchEffect { batch, offset_sd } => {
            require_batch(cohort, batch)?;
            let spread = pooled_spread(cohort)?;
            declared.push(CohortInvariant::WeightsUnchanged);
            declared.push(CohortInvariant::VolumesUnchanged);
            declared.push(CohortInvariant::BatchMembershipUnchanged);
            declared.push(CohortInvariant::OffsetConfinedToBatch {
                batch: batch.clone(),
                offset: fraction * offset_sd * spread,
                tolerance: ARITHMETIC_TOLERANCE,
            });
        }
        Knob::AssayDegradation {
            sd_multiplier,
            limit_of_detection,
        } => {
            check_multiplier(*sd_multiplier)?;
            declared.push(CohortInvariant::WeightsUnchanged);
            declared.push(CohortInvariant::VolumesUnchanged);
            declared.push(CohortInvariant::BatchMembershipUnchanged);
            declared.push(CohortInvariant::ClassMeansUnchanged {
                tolerance: ARITHMETIC_TOLERANCE,
            });
            declared.push(CohortInvariant::DispersionScaledBy {
                factor: 1.0 + fraction * (sd_multiplier - 1.0),
                tolerance: ARITHMETIC_TOLERANCE,
            });
            if let Some(limit) = limit_of_detection {
                declared.push(CohortInvariant::NonDetectionMarkedUnresolved { limit: *limit });
            }
        }
        Knob::SegmentationJitter { reproducibility_cv } => {
            check_reproducibility(*reproducibility_cv)?;
            declared.push(CohortInvariant::MeasurementsUnchanged);
            declared.push(CohortInvariant::WeightsUnchanged);
            declared.push(CohortInvariant::BatchMembershipUnchanged);
            declared.push(CohortInvariant::VolumesWithinReproducibility {
                cv: fraction * reproducibility_cv,
            });
        }
    }
    Ok(declared)
}

/// Applies the stress. Does not check anything it promised.
pub fn apply(cohort: &Cohort, stress: &Stress) -> Result<Cohort, StressError> {
    cohort.validate()?;
    let fraction = stress.magnitude.fraction();
    let mut stressed = cohort.clone();

    match &stress.knob {
        Knob::PrevalenceShift { target_prevalence } => {
            check_prevalence_target(*target_prevalence)?;
            let (positive_mass, negative_mass) = reweighting_mass(cohort)?;
            let total = positive_mass + negative_mass;
            let target = interpolated_target(cohort.prevalence(), *target_prevalence, fraction);
            let positive_factor = target * total / positive_mass;
            let negative_factor = (1.0 - target) * total / negative_mass;
            for subject in stressed.subjects.iter_mut() {
                subject.weight *= if subject.condition {
                    positive_factor
                } else {
                    negative_factor
                };
            }
        }
        Knob::BatchEffect { batch, offset_sd } => {
            require_batch(cohort, batch)?;
            let spread = pooled_spread(cohort)?;
            let offset = fraction * offset_sd * spread;
            for subject in stressed.subjects.iter_mut() {
                if &subject.batch == batch {
                    subject.marker += offset;
                }
            }
        }
        Knob::AssayDegradation {
            sd_multiplier,
            limit_of_detection,
        } => {
            check_multiplier(*sd_multiplier)?;
            let factor = 1.0 + fraction * (sd_multiplier - 1.0);
            widen(&mut stressed, factor, stress.seed);
            if let Some(limit) = limit_of_detection {
                for subject in stressed.subjects.iter_mut() {
                    subject.resolved = subject.marker >= *limit;
                }
            }
        }
        Knob::SegmentationJitter { reproducibility_cv } => {
            check_reproducibility(*reproducibility_cv)?;
            let cv = fraction * reproducibility_cv;
            let mut rng = StressRng::new(stress.seed);
            for subject in stressed.subjects.iter_mut() {
                subject.volume_mm3 *= 1.0 + cv * rng.symmetric();
            }
        }
    }

    stressed.id = format!("{}+{}", cohort.id, stress.id);
    Ok(stressed)
}

/// Runs the declared postconditions against a parent and its stressed descendant.
pub fn validate(
    before: &Cohort,
    after: &Cohort,
    stress: &Stress,
) -> Result<Vec<InvariantCheck>, StressError> {
    Ok(postconditions(before, stress)?
        .into_iter()
        .map(|invariant| {
            let result = invariant.check(before, after);
            InvariantCheck { invariant, result }
        })
        .collect())
}

/// Applies a stress and validates it in one step.
pub fn perturb(cohort: &Cohort, stress: &Stress) -> Result<Perturbed, StressError> {
    let stressed = apply(cohort, stress)?;
    let checks = validate(cohort, &stressed, stress)?;
    Ok(Perturbed {
        stress: stress.clone(),
        cohort: stressed,
        checks,
    })
}

/// Scales within-class spread by exactly `factor` without moving either class mean.
///
/// Per class: draw noise, strip its mean and its component along the centred signal, then rescale
/// so its sample standard deviation is exactly `σ√(f²−1)`. Because the residual is orthogonal to
/// the signal, variances add and the result has spread exactly `σf`.
///
/// A class with fewer than three subjects has a zero-dimensional orthogonal complement, so the
/// residual is the zero vector and no widening is possible. Nothing is silently substituted: the
/// spread stays put and [`CohortInvariant::DispersionScaledBy`] reports the generator as defective
/// for that cohort, which is the truthful outcome.
fn widen(cohort: &mut Cohort, factor: f64, seed: u64) {
    for (class_index, condition) in [true, false].into_iter().enumerate() {
        let indices: Vec<usize> = cohort
            .subjects
            .iter()
            .enumerate()
            .filter(|(_, subject)| subject.condition == condition)
            .map(|(index, _)| index)
            .collect();
        let markers: Vec<f64> = indices
            .iter()
            .map(|index| cohort.subjects[*index].marker)
            .collect();

        let mean = markers.iter().sum::<f64>() / markers.len() as f64;
        let spread = (markers
            .iter()
            .map(|value| (value - mean) * (value - mean))
            .sum::<f64>()
            / markers.len() as f64)
            .sqrt();
        let target = spread * (factor * factor - 1.0).max(0.0).sqrt();
        if target <= 0.0 {
            continue;
        }

        let mut noise =
            centred_orthogonal_noise(&markers, seed.wrapping_add(class_index as u64 + 1));
        let noise_mean = noise.iter().sum::<f64>() / noise.len() as f64;
        let noise_spread = (noise
            .iter()
            .map(|value| (value - noise_mean) * (value - noise_mean))
            .sum::<f64>()
            / noise.len() as f64)
            .sqrt();
        if noise_spread <= 0.0 {
            continue;
        }
        let scale = target / noise_spread;
        for value in noise.iter_mut() {
            *value *= scale;
        }
        for (index, value) in indices.iter().zip(noise.iter()) {
            cohort.subjects[*index].marker += value;
        }
    }
}

fn check_prevalence_target(target: f64) -> Result<(), StressError> {
    if target > 0.0 && target < 1.0 {
        Ok(())
    } else {
        Err(StressError::PrevalenceOutOfRange {
            target: format!("{target}"),
        })
    }
}

fn check_multiplier(multiplier: f64) -> Result<(), StressError> {
    if multiplier >= 1.0 {
        Ok(())
    } else {
        Err(StressError::NarrowingMultiplier {
            multiplier: format!("{multiplier}"),
        })
    }
}

fn check_reproducibility(cv: f64) -> Result<(), StressError> {
    if (0.0..1.0).contains(&cv) {
        Ok(())
    } else {
        Err(StressError::ReproducibilityOutOfRange {
            cv: format!("{cv}"),
        })
    }
}

/// The pooled within-class spread a batch offset is expressed in, or a refusal naming the class
/// that has no resolved subjects.
///
/// [`Cohort::pooled_within_sd`] is an `Option` because a class whose measurements all fell below
/// the limit of detection has no estimable spread. Reading that as zero scales the offset to zero,
/// so a stress that could not be applied is applied as nothing and then declares
/// `OffsetConfinedToBatch { offset: 0.0 }` — an invariant that holds trivially. An unmeasurable
/// spread is not a spread of zero, and the family reports it rather than running a null stress.
fn pooled_spread(cohort: &Cohort) -> Result<f64, StressError> {
    match cohort.pooled_within_sd() {
        Some(spread) => Ok(spread),
        None => {
            let class = if cohort.class_sd(true).is_none() {
                "resolved positive"
            } else {
                "resolved negative"
            };
            Err(StressError::ClassAbsent {
                cohort: cohort.id.clone(),
                class: class.to_string(),
            })
        }
    }
}

fn require_batch(cohort: &Cohort, batch: &str) -> Result<(), StressError> {
    if cohort.subjects.iter().any(|subject| subject.batch == batch) {
        Ok(())
    } else {
        Err(StressError::BatchAbsent {
            cohort: cohort.id.clone(),
            batch: batch.to_string(),
        })
    }
}

/// Weight currently sitting on each class, over analysable subjects.
///
/// Reweighting can move mass between classes that exist; it cannot conjure a class that does not.
/// A cohort with no analysable positives has no path to a higher base rate, and saying so is more
/// useful than dividing by zero.
fn reweighting_mass(cohort: &Cohort) -> Result<(f64, f64), StressError> {
    let positive: f64 = cohort
        .resolved()
        .filter(|subject| subject.condition)
        .map(|subject| subject.weight)
        .sum();
    let negative: f64 = cohort
        .resolved()
        .filter(|subject| !subject.condition)
        .map(|subject| subject.weight)
        .sum();
    if positive <= 0.0 || negative <= 0.0 {
        return Err(StressError::PrevalenceUnreachable {
            target: "any".into(),
            observed: format!("{:.6}", cohort.prevalence()),
        });
    }
    Ok((positive, negative))
}
