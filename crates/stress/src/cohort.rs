//! The thing that gets stressed.
//!
//! Blueprint 32.07 changes "population composition and sampling mechanisms while preserving or
//! deliberately altering the estimand", which presupposes a unit of population to compose. That
//! unit is [`Cohort`]: subjects carrying a latent truth, a measurement, a batch label, a
//! segmentation-derived volume, and a sampling weight.
//!
//! Two modelling decisions are worth stating because they are what make the postconditions in
//! [`crate::invariant`] executable rather than rhetorical.
//!
//! **Prevalence is moved by weight, not by resampling.** 32.07 offers *"reweighted or resampled
//! parent units"* and separately names *"repeated samples inflate n"* as a failure risk. Duplicating
//! positives to raise prevalence walks straight into that risk; reweighting does not touch a single
//! measurement, keeps every subject exactly once, and pays its cost visibly through
//! [`Cohort::effective_n`]. The cost is real — a reweighted cohort carries less information than
//! its nominal size suggests — and the blueprint's scoring section demands the *"effective—not
//! nominal—test size"* be reported, so it is reported.
//!
//! **Non-detection is a third state, not a zero.** 32.03 names *"nondetection treated as absence"*
//! as the characteristic assay-stress failure. A subject whose degraded measurement falls below the
//! assay's limit of detection is marked [`Subject::resolved`]`= false` and is excluded from every
//! statistic, with the exclusion count carried on the conclusion. Coercing it to a number would
//! silently convert "we could not measure this" into "this was low".
//!
//! Deliberately not modelled: time, treatment, and modality. Temporal and evidence-availability
//! stress is blueprint 32.08 and is already served by the temporal leakage mechanism in
//! `bioprism-mutation`; adding a second, weaker version of it here would create two answers to one
//! question.

use crate::error::StressError;
use crate::rng::StressRng;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// One participant.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Subject {
    pub id: String,
    /// Site, batch, reagent lot or platform — the technical domain of 32.06.
    pub batch: String,
    /// The latent truth. Stress never touches this: a perturbation that edits the answer is not a
    /// robustness probe, it is a different benchmark.
    pub condition: bool,
    /// The assay readout.
    pub marker: f64,
    /// A segmentation-derived geometric quantity, per the geometric evidence of 30.05.
    pub volume_mm3: f64,
    /// Sampling weight. Prevalence shift moves this and nothing else.
    pub weight: f64,
    /// False once the measurement has fallen below the assay's limit of detection.
    pub resolved: bool,
}

impl Subject {
    pub fn new(
        id: impl Into<String>,
        batch: impl Into<String>,
        condition: bool,
        marker: f64,
        volume_mm3: f64,
    ) -> Self {
        Subject {
            id: id.into(),
            batch: batch.into(),
            condition,
            marker,
            volume_mm3,
            weight: 1.0,
            resolved: true,
        }
    }
}

/// One position in an ordering, carrying the quantity that put it there.
///
/// The value travels with the id because the strongest statement available about an ordering under
/// bounded perturbation is conditional on the gap: two subjects separated by more than the
/// perturbation can move cannot swap. Discarding the value would leave only the weak claim that
/// nothing anywhere changed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Ranked {
    pub id: String,
    pub value: f64,
}

/// A population of subjects, with the statistics a decision procedure reads.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Cohort {
    pub id: String,
    pub subjects: Vec<Subject>,
}

impl Cohort {
    pub fn new(id: impl Into<String>, subjects: Vec<Subject>) -> Self {
        Cohort {
            id: id.into(),
            subjects,
        }
    }

    /// Rejects a cohort no statistic is defined on.
    ///
    /// Called at the head of every perturbation, so a malformed cohort is reported as a malformed
    /// cohort rather than surfacing later as a `NaN` in a robustness profile.
    pub fn validate(&self) -> Result<(), StressError> {
        if self.subjects.is_empty() {
            return Err(StressError::EmptyCohort {
                cohort: self.id.clone(),
            });
        }
        let mut seen = BTreeSet::new();
        for subject in &self.subjects {
            if !seen.insert(subject.id.as_str()) {
                return Err(StressError::DuplicateSubject {
                    cohort: self.id.clone(),
                    subject: subject.id.clone(),
                });
            }
            for (field, value) in [
                ("marker", subject.marker),
                ("volume_mm3", subject.volume_mm3),
                ("weight", subject.weight),
            ] {
                if !value.is_finite() {
                    return Err(StressError::NonFinite {
                        subject: subject.id.clone(),
                        field: field.into(),
                    });
                }
            }
            if subject.weight <= 0.0 {
                return Err(StressError::NonPositiveWeight {
                    subject: subject.id.clone(),
                    weight: format!("{}", subject.weight),
                });
            }
            if subject.volume_mm3 <= 0.0 {
                return Err(StressError::NonPositiveVolume {
                    subject: subject.id.clone(),
                    volume: format!("{}", subject.volume_mm3),
                });
            }
        }
        for (class, present) in [
            ("positive", self.subjects.iter().any(|s| s.condition)),
            ("negative", self.subjects.iter().any(|s| !s.condition)),
        ] {
            if !present {
                return Err(StressError::ClassAbsent {
                    cohort: self.id.clone(),
                    class: class.into(),
                });
            }
        }
        Ok(())
    }

    pub fn len(&self) -> usize {
        self.subjects.len()
    }

    pub fn is_empty(&self) -> bool {
        self.subjects.is_empty()
    }

    pub fn get(&self, id: &str) -> Option<&Subject> {
        self.subjects.iter().find(|subject| subject.id == id)
    }

    /// Subjects whose measurement is above the limit of detection.
    pub fn resolved(&self) -> impl Iterator<Item = &Subject> {
        self.subjects.iter().filter(|subject| subject.resolved)
    }

    pub fn unresolved_count(&self) -> usize {
        self.subjects.iter().filter(|s| !s.resolved).count()
    }

    /// Weighted base rate over resolved subjects.
    pub fn prevalence(&self) -> f64 {
        let total: f64 = self.resolved().map(|s| s.weight).sum();
        if total <= 0.0 {
            return 0.0;
        }
        self.resolved()
            .filter(|s| s.condition)
            .map(|s| s.weight)
            .sum::<f64>()
            / total
    }

    /// Kish effective sample size, `(Σw)² / Σw²`.
    ///
    /// The honest denominator after reweighting. A cohort of two hundred subjects reweighted hard
    /// enough to reach a deployment base rate may carry the information of forty; reporting `200`
    /// is the inflation 32.07 warns about, in a different costume.
    pub fn effective_n(&self) -> f64 {
        let sum: f64 = self.resolved().map(|s| s.weight).sum();
        let sum_squares: f64 = self.resolved().map(|s| s.weight * s.weight).sum();
        if sum_squares <= 0.0 {
            0.0
        } else {
            sum * sum / sum_squares
        }
    }

    /// Weighted mean marker within one class.
    pub fn class_mean(&self, condition: bool) -> Option<f64> {
        let total: f64 = self
            .resolved()
            .filter(|s| s.condition == condition)
            .map(|s| s.weight)
            .sum();
        if total <= 0.0 {
            return None;
        }
        Some(
            self.resolved()
                .filter(|s| s.condition == condition)
                .map(|s| s.weight * s.marker)
                .sum::<f64>()
                / total,
        )
    }

    /// Weighted population standard deviation of the marker within one class.
    pub fn class_sd(&self, condition: bool) -> Option<f64> {
        let mean = self.class_mean(condition)?;
        let total: f64 = self
            .resolved()
            .filter(|s| s.condition == condition)
            .map(|s| s.weight)
            .sum();
        if total <= 0.0 {
            return None;
        }
        let variance = self
            .resolved()
            .filter(|s| s.condition == condition)
            .map(|s| s.weight * (s.marker - mean) * (s.marker - mean))
            .sum::<f64>()
            / total;
        Some(variance.sqrt())
    }

    /// Pooled within-class spread, the unit batch offsets are expressed in.
    ///
    /// Expressing an offset in raw marker units would make a stress magnitude meaningless across
    /// cohorts measured on different scales; expressing it in pooled within-class standard
    /// deviations makes "half a standard deviation of batch shift" comparable everywhere.
    pub fn pooled_within_sd(&self) -> Option<f64> {
        let positive = self.class_sd(true)?;
        let negative = self.class_sd(false)?;
        Some(((positive * positive + negative * negative) / 2.0).sqrt())
    }

    /// Subjects ordered by marker, descending, ties broken by id.
    ///
    /// Total and deterministic: a ranking that depends on input order would report spurious
    /// instability under perturbations that only reorder.
    pub fn ranking(&self) -> Vec<Ranked> {
        self.rank_by(|subject| subject.marker)
    }

    /// Subjects ordered by segmentation-derived volume, descending.
    pub fn volume_ranking(&self) -> Vec<Ranked> {
        self.rank_by(|subject| subject.volume_mm3)
    }

    fn rank_by(&self, key: impl Fn(&Subject) -> f64) -> Vec<Ranked> {
        let mut ordered: Vec<&Subject> = self.resolved().collect();
        ordered.sort_by(|left, right| {
            key(right)
                .partial_cmp(&key(left))
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| left.id.cmp(&right.id))
        });
        ordered
            .iter()
            .map(|subject| Ranked {
                id: subject.id.clone(),
                value: key(subject),
            })
            .collect()
    }

    /// Weighted rank concordance of marker against condition, ties counted as a half.
    ///
    /// Exactly invariant under class-uniform reweighting: every positive-negative pair's weight
    /// product and the normaliser scale by the same factor. That is the algebraic reason a
    /// discriminative summary is entitled to declare invariance under prevalence shift, and it is
    /// checked in the test suite rather than asserted here.
    pub fn separation(&self) -> Option<f64> {
        let positives: Vec<&Subject> = self.resolved().filter(|s| s.condition).collect();
        let negatives: Vec<&Subject> = self.resolved().filter(|s| !s.condition).collect();
        if positives.is_empty() || negatives.is_empty() {
            return None;
        }
        let mut concordant = 0.0;
        let mut total = 0.0;
        for positive in &positives {
            for negative in &negatives {
                let pair = positive.weight * negative.weight;
                total += pair;
                if positive.marker > negative.marker {
                    concordant += pair;
                } else if (positive.marker - negative.marker).abs() == 0.0 {
                    concordant += pair * 0.5;
                }
            }
        }
        if total <= 0.0 {
            None
        } else {
            Some(concordant / total)
        }
    }

    pub fn batches(&self) -> BTreeSet<String> {
        self.subjects.iter().map(|s| s.batch.clone()).collect()
    }

    /// Fraction of each batch that is class-minority, minimised over batches.
    ///
    /// Zero means at least one batch contains only one class, so no perturbation of that batch can
    /// be attributed to the batch rather than to the biology. 32.06 names *"batch and condition are
    /// perfectly confounded without acknowledgement"* as a failure risk; this is the number the
    /// acknowledgement is made of.
    pub fn batch_condition_overlap(&self) -> f64 {
        let mut counts: BTreeMap<&str, (usize, usize)> = BTreeMap::new();
        for subject in &self.subjects {
            let entry = counts.entry(subject.batch.as_str()).or_insert((0, 0));
            if subject.condition {
                entry.0 += 1;
            } else {
                entry.1 += 1;
            }
        }
        counts
            .values()
            .map(|(positive, negative)| {
                let total = positive + negative;
                if total == 0 {
                    0.0
                } else {
                    (*positive).min(*negative) as f64 / total as f64
                }
            })
            .fold(f64::INFINITY, f64::min)
            .min(1.0)
    }

    /// Content address over the cohort's data, excluding its label.
    ///
    /// The same reasoning as `bioprism-mutation`'s content digest: two cohorts with identical
    /// subjects are the same cohort however they are named, and hashing the name would let a
    /// generator defeat deduplication by relabelling.
    pub fn digest(&self) -> Result<ContentHash, StressError> {
        let value =
            serde_json::to_value(&self.subjects).map_err(|error| StressError::NotAddressable {
                cohort: self.id.clone(),
                detail: error.to_string(),
            })?;
        ContentHash::of_value(&value).map_err(|error| StressError::NotAddressable {
            cohort: self.id.clone(),
            detail: error.to_string(),
        })
    }

    /// A deterministic cohort for exercising the program.
    ///
    /// Batches are assigned by index and the condition by a prefix cut, so batch membership and
    /// condition are close to orthogonal — the configuration under which batch stress is
    /// identifiable at all. [`Cohort::synthetic_confounded`] is the deliberate opposite.
    pub fn synthetic(
        id: impl Into<String>,
        seed: u64,
        subjects: usize,
        prevalence: f64,
        separation_sd: f64,
    ) -> Self {
        const BATCHES: [&str; 2] = ["site-a", "site-b"];
        let mut rng = StressRng::new(seed);
        let positives = (prevalence * subjects as f64).round() as usize;
        let built = (0..subjects)
            .map(|index| {
                let condition = index < positives;
                let shift = if condition { separation_sd } else { 0.0 };
                Subject::new(
                    format!("SUBJ-{index:04}"),
                    BATCHES[index % BATCHES.len()],
                    condition,
                    rng.unit_variance() + shift,
                    1_000.0 + rng.unit_variance() * 120.0,
                )
            })
            .collect();
        Cohort::new(id, built)
    }

    /// A cohort in which batch membership is a function of the condition.
    ///
    /// Exists so the confounding check has something to fail on. A batch stress run against this
    /// cohort must report non-identifiability, not a clean pass.
    pub fn synthetic_confounded(
        id: impl Into<String>,
        seed: u64,
        subjects: usize,
        prevalence: f64,
        separation_sd: f64,
    ) -> Self {
        let mut cohort = Cohort::synthetic(id, seed, subjects, prevalence, separation_sd);
        for subject in cohort.subjects.iter_mut() {
            subject.batch = if subject.condition {
                "site-b".into()
            } else {
                "site-a".into()
            };
        }
        cohort
    }
}
