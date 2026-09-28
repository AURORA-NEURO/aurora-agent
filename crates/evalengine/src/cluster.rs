//! Parent-clustered statistics.
//!
//! Blueprint 07.06 states the sampling hierarchy — "trials are nested in instances, mutations,
//! cells, parents, packs, domains, architectures, and models" — and the rule that follows from it:
//! "analyses must not treat correlated descendants as independent." 06.15 is the reason it matters
//! here: the mutation engine generates instances at million scale from a much smaller set of
//! parents, and 06.13's failure mode is "generated instances inflate scale while adding little
//! independent diagnostic information."
//!
//! Nine hundred instances mutated from one parent are not nine hundred observations. They are
//! closer to one, and an interval computed as though they were nine hundred is wrong by a factor
//! of thirty in width.
//!
//! # The maths, and its assumption
//!
//! One-way random-effects ANOVA on the parent factor, then a design effect. For `k` parents with
//! sizes `n_i` summing to `N`:
//!
//! ```text
//! MSB  = Σ nᵢ(ȳᵢ − ȳ)² / (k − 1)
//! MSW  = Σ Σ (yᵢⱼ − ȳᵢ)² / (N − k)
//! m₀   = (N − Σnᵢ²/N) / (k − 1)
//! ICC  = (MSB − MSW) / (MSB + (m₀ − 1)·MSW)          clamped to [0, 1]
//! deff = 1 + (N/k − 1)·ICC
//! nₑff = N / deff                                     clamped to [k, N]
//! ```
//!
//! The assumption, stated plainly: **instances are exchangeable within a parent and independent
//! across parents, with one variance component.** That is a simplification. Real descendants are
//! nested further (mutation family inside parent, trial inside instance), difficulty is not
//! constant within a parent, and the ICC point estimate carries its own uncertainty which is not
//! propagated. A model with a second level would give a different, usually smaller, effective `n`.
//! The estimate here is therefore a **ceiling on independent information, not a measurement of
//! it**, and every consumer should read it as "no more independent than this".
//!
//! # Where it refuses
//!
//! With one parent there is no between-parent variation to estimate from, so the ICC is
//! [`IccEstimate::Undefined`] and the effective sample size falls back to the number of parents.
//! That fallback is the conservative reading — it assumes descendants of a parent are perfectly
//! correlated — and it is chosen over the alternative (assume independence, report `N`) because
//! being wrong in that direction manufactures significance.
//!
//! # Not implemented here
//!
//! Bootstrap or Bayesian intervals (07.06 asks for hierarchical bootstrap for pack estimates),
//! multi-level nesting, paired intervals for matched comparisons, and multiplicity control. This
//! module supplies the effective sample size those methods would need and stops; it does not
//! publish a confidence interval, because a normal-approximation interval on a clustered pass rate
//! would be the kind of false precision 07.06 explicitly warns against.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::error::EvalError;

/// What could be estimated about within-parent correlation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "icc", rename_all = "snake_case")]
pub enum IccEstimate {
    /// Estimable, clamped to `[0, 1]`.
    Estimated { value: f64 },
    /// Every parent contributed exactly one instance, so within-parent correlation cannot arise
    /// and the design effect is one by construction.
    NotApplicable,
    /// Not estimable: too few parents, or no variance at all in the sample.
    Undefined { reason: String },
    /// The sample is malformed or floating-point arithmetic could not produce a finite estimate.
    Invalid { reason: String },
}

impl IccEstimate {
    pub fn value(&self) -> Option<f64> {
        match self {
            IccEstimate::Estimated { value } => Some(*value),
            IccEstimate::NotApplicable => Some(0.0),
            IccEstimate::Undefined { .. } | IccEstimate::Invalid { .. } => None,
        }
    }

    pub fn is_undefined(&self) -> bool {
        matches!(self, IccEstimate::Undefined { .. })
    }

    pub fn is_invalid(&self) -> bool {
        matches!(self, IccEstimate::Invalid { .. })
    }
}

/// Observations grouped by the parent task they descend from.
///
/// Values are expected in `[0, 1]` — a pass indicator or a credit fraction — but nothing here
/// requires it; the arithmetic is the same for any bounded score.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ClusteredSample {
    pub label: String,
    clusters: BTreeMap<String, Vec<f64>>,
    /// Instances whose value could not be determined, kept by parent so the unknown share is
    /// reported rather than dropped.
    unknown: BTreeMap<String, usize>,
}

impl ClusteredSample {
    pub fn new(label: impl Into<String>) -> Self {
        ClusteredSample {
            label: label.into(),
            clusters: BTreeMap::new(),
            unknown: BTreeMap::new(),
        }
    }

    pub fn push(&mut self, parent: impl Into<String>, value: f64) {
        self.clusters.entry(parent.into()).or_default().push(value);
    }

    /// Record an instance whose value is unknown.
    ///
    /// It counts toward the unknown share and toward nothing else. Pushing `0.0` instead would be
    /// the coercion `bioprism-section`'s omission manifest refuses, and it is refused here too.
    pub fn push_unknown(&mut self, parent: impl Into<String>) {
        *self.unknown.entry(parent.into()).or_insert(0) += 1;
    }

    pub fn n_instances(&self) -> usize {
        self.clusters.values().map(Vec::len).sum()
    }

    pub fn n_clusters(&self) -> usize {
        self.clusters.len()
    }

    pub fn n_unknown(&self) -> usize {
        self.unknown.values().sum()
    }

    pub fn parents(&self) -> impl Iterator<Item = &str> {
        self.clusters.keys().map(String::as_str)
    }

    /// The mean over instances. Named for what it does wrong: a parent with nine hundred
    /// descendants moves this nine hundred times as much as a parent with one. Returns `None`
    /// for an empty parent or a non-finite input or result.
    pub fn naive_instance_mean(&self) -> Option<f64> {
        let n = self.n_instances();
        if n == 0
            || self
                .clusters
                .values()
                .any(|values| values.is_empty() || values.iter().any(|value| !value.is_finite()))
        {
            return None;
        }
        let total: f64 = self.clusters.values().flatten().sum();
        let mean = total / n as f64;
        mean.is_finite().then_some(mean)
    }

    /// The unweighted mean of per-parent means, which treats the parent as the sampling unit.
    /// Returns `None` for an empty parent or a non-finite input or result.
    pub fn cluster_balanced_mean(&self) -> Option<f64> {
        if self.clusters.is_empty()
            || self
                .clusters
                .values()
                .any(|values| values.is_empty() || values.iter().any(|value| !value.is_finite()))
        {
            return None;
        }
        let sum: f64 = self
            .clusters
            .values()
            .map(|values| values.iter().sum::<f64>() / values.len() as f64)
            .sum();
        let mean = sum / self.clusters.len() as f64;
        mean.is_finite().then_some(mean)
    }

    /// One-way random-effects ICC over the parent factor.
    pub fn icc(&self) -> IccEstimate {
        let k = self.clusters.len();
        let n_total = self.n_instances();
        if let Some((parent, _)) = self.clusters.iter().find(|(_, values)| values.is_empty()) {
            return IccEstimate::Invalid {
                reason: format!("parent `{parent}` has no observations"),
            };
        }
        if let Some((parent, value)) = self.clusters.iter().find_map(|(parent, values)| {
            values
                .iter()
                .find(|value| !value.is_finite())
                .map(|value| (parent, value))
        }) {
            return IccEstimate::Invalid {
                reason: format!("parent `{parent}` contains non-finite score `{value}`"),
            };
        }
        if k < 2 {
            return IccEstimate::Undefined {
                reason: format!("{k} parent cluster(s); between-parent variance is not estimable"),
            };
        }
        if n_total == k {
            return IccEstimate::NotApplicable;
        }

        let grand: f64 = self.clusters.values().flatten().sum::<f64>() / n_total as f64;
        if !grand.is_finite() {
            return IccEstimate::Invalid {
                reason: "grand mean overflowed floating-point range".to_string(),
            };
        }

        let mut ssb = 0.0;
        let mut ssw = 0.0;
        let mut sum_sq_sizes = 0.0;
        for values in self.clusters.values() {
            if values.is_empty() {
                continue;
            }
            let size = values.len() as f64;
            sum_sq_sizes += size * size;
            let mean = values.iter().sum::<f64>() / size;
            ssb += size * (mean - grand).powi(2);
            ssw += values.iter().map(|v| (v - mean).powi(2)).sum::<f64>();
        }

        let msb = ssb / (k as f64 - 1.0);
        let msw = ssw / (n_total as f64 - k as f64);
        let m0 = (n_total as f64 - sum_sq_sizes / n_total as f64) / (k as f64 - 1.0);
        let denominator = msb + (m0 - 1.0) * msw;

        if !msb.is_finite() || !msw.is_finite() || !m0.is_finite() || !denominator.is_finite() {
            return IccEstimate::Invalid {
                reason: "variance calculation overflowed floating-point range".to_string(),
            };
        }

        if denominator <= f64::EPSILON {
            return IccEstimate::Undefined {
                reason: "no variance in the sample; correlation is not estimable".to_string(),
            };
        }

        let raw = (msb - msw) / denominator;
        if !raw.is_finite() {
            return IccEstimate::Invalid {
                reason: "correlation calculation was non-finite".to_string(),
            };
        }
        IccEstimate::Estimated {
            value: raw.clamp(0.0, 1.0),
        }
    }

    /// Independent observations the sample is worth. Returns `None` for no observations, an
    /// invalid sample, or arithmetic that cannot produce a finite estimate.
    ///
    /// Never above the instance count and never below the parent count. The lower clamp is the
    /// statement that parents are independent of one another; the upper is arithmetic.
    pub fn effective_sample_size(&self) -> Option<f64> {
        let n = self.n_instances() as f64;
        let k = self.n_clusters() as f64;
        if n == 0.0 {
            return None;
        }
        match self.icc() {
            IccEstimate::NotApplicable => Some(n),
            IccEstimate::Undefined { .. } => Some(k),
            IccEstimate::Invalid { .. } => None,
            IccEstimate::Estimated { value } => {
                let mean_size = n / k;
                let design_effect = 1.0 + (mean_size - 1.0) * value;
                let estimate = (n / design_effect).clamp(k, n);
                estimate.is_finite().then_some(estimate)
            }
        }
    }

    /// The aggregate, with its effective sample size attached.
    ///
    /// There is deliberately no method on this type that returns a headline mean as a bare `f64`.
    /// [`ClusteredSample::naive_instance_mean`] exists to be compared against, and is named so that
    /// quoting it takes a decision.
    pub fn estimate(&self) -> Result<ClusteredEstimate, EvalError> {
        if self.clusters.is_empty() {
            return Err(EvalError::EmptySample {
                label: self.label.clone(),
            });
        }
        if let Some((parent, _)) = self.clusters.iter().find(|(_, values)| values.is_empty()) {
            return Err(EvalError::EmptyCluster {
                label: self.label.clone(),
                parent: parent.clone(),
            });
        }
        if let Some((parent, value)) = self.clusters.iter().find_map(|(parent, values)| {
            values
                .iter()
                .find(|value| !value.is_finite())
                .map(|value| (parent, value))
        }) {
            return Err(EvalError::NonFiniteClusterValue {
                label: self.label.clone(),
                parent: parent.clone(),
                value: value.to_string(),
            });
        }

        let mean =
            self.cluster_balanced_mean()
                .ok_or_else(|| EvalError::NonFiniteClusterAggregate {
                    label: self.label.clone(),
                    statistic: "cluster-balanced mean".to_string(),
                })?;
        let naive_instance_mean =
            self.naive_instance_mean()
                .ok_or_else(|| EvalError::NonFiniteClusterAggregate {
                    label: self.label.clone(),
                    statistic: "instance mean".to_string(),
                })?;
        let effective_sample_size =
            self.effective_sample_size()
                .ok_or_else(|| EvalError::NonFiniteClusterAggregate {
                    label: self.label.clone(),
                    statistic: "effective sample size".to_string(),
                })?;

        let instances = self.n_instances();
        let unknown = self.n_unknown();
        let observed = instances + unknown;

        Ok(ClusteredEstimate {
            label: self.label.clone(),
            mean,
            naive_instance_mean,
            instances,
            clusters: self.n_clusters(),
            largest_cluster: self
                .clusters
                .values()
                .map(Vec::len)
                .max()
                .unwrap_or_default(),
            icc: self.icc(),
            effective_sample_size,
            unknown_instances: unknown,
            unknown_fraction: if observed == 0 {
                0.0
            } else {
                unknown as f64 / observed as f64
            },
        })
    }
}

/// An aggregate that cannot be quoted without its effective sample size, because the two travel in
/// the same struct and both serialize.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClusteredEstimate {
    pub label: String,
    /// The cluster-balanced mean: parents weigh equally, whatever their descendant count.
    pub mean: f64,
    /// The instance mean, published beside it. A large gap between the two is the signal that one
    /// parent's descendants dominate the pack.
    pub naive_instance_mean: f64,
    pub instances: usize,
    pub clusters: usize,
    pub largest_cluster: usize,
    pub icc: IccEstimate,
    /// Independent observations this is worth. Always read this before the mean.
    pub effective_sample_size: f64,
    pub unknown_instances: usize,
    pub unknown_fraction: f64,
}

impl ClusteredEstimate {
    /// Check that a persisted or caller-constructed aggregate could have come from a sample.
    ///
    /// The fields are public for report interoperability, so every consumer that makes a claim
    /// from an estimate must validate it before trusting the attached effective sample size.
    pub fn validate(&self) -> Result<(), EvalError> {
        let invalid = |detail: &str| EvalError::InvalidClusteredEstimate {
            label: self.label.clone(),
            detail: detail.to_string(),
        };

        if !self.mean.is_finite() || !self.naive_instance_mean.is_finite() {
            return Err(invalid("means must be finite"));
        }
        if self.instances == 0 || self.clusters == 0 {
            return Err(invalid(
                "known instances and parent clusters must both be nonzero",
            ));
        }
        if self.clusters > self.instances {
            return Err(invalid("parent cluster count exceeds known instance count"));
        }
        if self.largest_cluster == 0 || self.largest_cluster > self.instances {
            return Err(invalid(
                "largest cluster is outside the known instance count",
            ));
        }
        let smallest_possible_largest =
            self.instances / self.clusters + usize::from(self.instances % self.clusters != 0);
        if self.largest_cluster < smallest_possible_largest {
            return Err(invalid(
                "largest cluster is smaller than the average cluster size",
            ));
        }
        let Some(total_instances) = self.instances.checked_add(self.unknown_instances) else {
            return Err(invalid("known and unknown instance counts overflow"));
        };
        let expected_unknown_fraction = self.unknown_instances as f64 / total_instances as f64;
        if !self.unknown_fraction.is_finite()
            || !(0.0..=1.0).contains(&self.unknown_fraction)
            || (self.unknown_fraction - expected_unknown_fraction).abs() > 1e-12
        {
            return Err(invalid(
                "unknown fraction does not match the known and unknown counts",
            ));
        }
        if !self.effective_sample_size.is_finite()
            || self.effective_sample_size < self.clusters as f64
            || self.effective_sample_size > self.instances as f64
        {
            return Err(invalid(
                "effective sample size must be finite and between parent and instance counts",
            ));
        }

        let expected_effective = match &self.icc {
            IccEstimate::Estimated { value } => {
                if !value.is_finite() || !(0.0..=1.0).contains(value) {
                    return Err(invalid("estimated ICC must be finite and in [0, 1]"));
                }
                if self.clusters < 2 || self.instances == self.clusters {
                    return Err(invalid(
                        "estimated ICC requires repeated observations in at least two clusters",
                    ));
                }
                let n = self.instances as f64;
                let k = self.clusters as f64;
                Some((n / (1.0 + (n / k - 1.0) * value)).clamp(k, n))
            }
            IccEstimate::NotApplicable => {
                if self.instances != self.clusters {
                    return Err(invalid(
                        "ICC is not applicable only when every cluster is a singleton",
                    ));
                }
                Some(self.instances as f64)
            }
            IccEstimate::Undefined { .. } => {
                if self.clusters > 1 && self.instances == self.clusters {
                    return Err(invalid(
                        "singleton clusters have a defined, not undefined, ICC",
                    ));
                }
                Some(self.clusters as f64)
            }
            IccEstimate::Invalid { .. } => {
                return Err(invalid(
                    "an invalid ICC cannot support a clustered estimate",
                ));
            }
        };
        if let Some(expected) = expected_effective {
            if (self.effective_sample_size - expected).abs() > 1e-9 * expected.max(1.0) {
                return Err(invalid(
                    "effective sample size is inconsistent with the ICC",
                ));
            }
        }
        Ok(())
    }

    /// How much the instance count overstates the independent information.
    pub fn inflation_factor(&self) -> f64 {
        if self.effective_sample_size <= 0.0 {
            return f64::INFINITY;
        }
        self.instances as f64 / self.effective_sample_size
    }

    /// Whether the two means disagree enough that parent imbalance is driving the headline.
    pub fn is_parent_dominated(&self, tolerance: f64) -> bool {
        (self.mean - self.naive_instance_mean).abs() > tolerance
    }

    /// A one-line summary that always carries the caveat.
    pub fn explain(&self) -> String {
        format!(
            "{}: {:.3} over {} instances from {} parent(s), effective n = {:.2}{}",
            self.label,
            self.mean,
            self.instances,
            self.clusters,
            self.effective_sample_size,
            if self.icc.is_undefined() {
                " (correlation not estimable; effective n falls back to the parent count)"
            } else {
                ""
            }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(label: &str, groups: &[(&str, &[f64])]) -> ClusteredSample {
        let mut sample = ClusteredSample::new(label);
        for (parent, values) in groups {
            for value in *values {
                sample.push(*parent, *value);
            }
        }
        sample
    }

    #[test]
    fn nine_hundred_instances_from_one_parent_are_worth_one_observation() {
        let mut sample = ClusteredSample::new("pass_rate");
        for index in 0..900 {
            sample.push("parent-1", f64::from(index % 2));
        }
        let estimate = sample.estimate().expect("non-empty");
        assert_eq!(estimate.instances, 900);
        assert_eq!(estimate.clusters, 1);
        assert!(estimate.icc.is_undefined());
        assert_eq!(estimate.effective_sample_size, 1.0);
        assert!(estimate.inflation_factor() > 800.0);
    }

    #[test]
    fn perfectly_correlated_descendants_collapse_to_the_parent_count() {
        let sample = sample(
            "pass_rate",
            &[
                ("p1", &[1.0, 1.0, 1.0, 1.0]),
                ("p2", &[0.0, 0.0, 0.0, 0.0]),
                ("p3", &[1.0, 1.0, 1.0, 1.0]),
            ],
        );
        let estimate = sample.estimate().expect("non-empty");
        assert_eq!(estimate.instances, 12);
        assert_eq!(estimate.icc.value(), Some(1.0));
        assert!((estimate.effective_sample_size - 3.0).abs() < 1e-9);
    }

    #[test]
    fn independent_singleton_parents_keep_their_full_sample_size() {
        let sample = sample(
            "pass_rate",
            &[
                ("p1", &[1.0]),
                ("p2", &[0.0]),
                ("p3", &[1.0]),
                ("p4", &[0.0]),
            ],
        );
        let estimate = sample.estimate().expect("non-empty");
        assert_eq!(estimate.icc, IccEstimate::NotApplicable);
        assert_eq!(estimate.effective_sample_size, 4.0);
        assert_eq!(estimate.inflation_factor(), 1.0);
    }

    #[test]
    fn effective_sample_size_never_exceeds_the_instance_count() {
        let sample = sample(
            "pass_rate",
            &[("p1", &[1.0, 0.0, 1.0]), ("p2", &[0.0, 1.0, 0.0])],
        );
        let estimate = sample.estimate().expect("non-empty");
        assert!(estimate.effective_sample_size <= estimate.instances as f64);
        assert!(estimate.effective_sample_size >= estimate.clusters as f64);
    }

    #[test]
    fn a_dominant_parent_moves_the_instance_mean_but_not_the_cluster_balanced_one() {
        let mut sample = ClusteredSample::new("pass_rate");
        for _ in 0..100 {
            sample.push("prolific", 1.0);
        }
        sample.push("sparse", 0.0);
        let estimate = sample.estimate().expect("non-empty");
        assert!((estimate.mean - 0.5).abs() < 1e-9);
        assert!(estimate.naive_instance_mean > 0.98);
        assert!(estimate.is_parent_dominated(0.1));
    }

    #[test]
    fn unknown_instances_are_reported_rather_than_averaged_in_as_zero() {
        let mut sample = ClusteredSample::new("pass_rate");
        sample.push("p1", 1.0);
        sample.push("p2", 1.0);
        sample.push_unknown("p1");
        sample.push_unknown("p1");
        let estimate = sample.estimate().expect("non-empty");
        assert!((estimate.mean - 1.0).abs() < 1e-9);
        assert_eq!(estimate.unknown_instances, 2);
        assert!((estimate.unknown_fraction - 0.5).abs() < 1e-9);
    }

    #[test]
    fn an_empty_sample_is_an_error_rather_than_a_mean_of_zero() {
        let err = ClusteredSample::new("pass_rate").estimate().unwrap_err();
        assert_eq!(
            err,
            EvalError::EmptySample {
                label: "pass_rate".to_string()
            }
        );
    }

    #[test]
    fn a_non_finite_score_cannot_escape_as_a_clustered_estimate() {
        let mut sample = ClusteredSample::new("pass_rate");
        sample.push("p1", 1.0);
        sample.push("p2", f64::NAN);

        assert_eq!(sample.naive_instance_mean(), None);
        assert_eq!(sample.cluster_balanced_mean(), None);
        assert!(sample.icc().is_invalid());
        assert_eq!(sample.effective_sample_size(), None);
        assert!(matches!(
            sample.estimate(),
            Err(EvalError::NonFiniteClusterValue { parent, .. }) if parent == "p2"
        ));
    }

    #[test]
    fn finite_scores_that_overflow_an_aggregate_are_rejected() {
        let sample = sample(
            "large_finite_score",
            &[("p1", &[f64::MAX, f64::MAX]), ("p2", &[f64::MAX, f64::MAX])],
        );

        assert_eq!(sample.naive_instance_mean(), None);
        assert_eq!(sample.cluster_balanced_mean(), None);
        assert!(sample.icc().is_invalid());
        assert!(matches!(
            sample.estimate(),
            Err(EvalError::NonFiniteClusterAggregate { .. })
        ));
    }

    #[test]
    fn a_sample_with_no_variance_reports_the_conservative_parent_count() {
        let sample = sample("pass_rate", &[("p1", &[1.0, 1.0]), ("p2", &[1.0, 1.0])]);
        let estimate = sample.estimate().expect("non-empty");
        assert!(estimate.icc.is_undefined());
        assert_eq!(estimate.effective_sample_size, 2.0);
    }

    #[test]
    fn a_clustered_estimate_round_trips_through_json() {
        let estimate = sample("pass_rate", &[("p1", &[1.0, 0.0]), ("p2", &[1.0, 1.0])])
            .estimate()
            .expect("non-empty");
        let text = serde_json::to_string(&estimate).expect("serialize");
        let back: ClusteredEstimate = serde_json::from_str(&text).expect("deserialize");
        assert_eq!(estimate, back);
        back.validate().expect("valid aggregate");
    }

    #[test]
    fn a_deserialized_effective_sample_size_cannot_exceed_its_instance_count() {
        let estimate = sample("pass_rate", &[("p1", &[1.0]), ("p2", &[0.0])])
            .estimate()
            .expect("non-empty");
        let mut json = serde_json::to_value(&estimate).expect("serialize");
        json["effective_sample_size"] = serde_json::json!(1000.0);
        let tampered: ClusteredEstimate = serde_json::from_value(json).expect("deserialize");
        assert!(matches!(
            tampered.validate(),
            Err(EvalError::InvalidClusteredEstimate { .. })
        ));
    }
}
