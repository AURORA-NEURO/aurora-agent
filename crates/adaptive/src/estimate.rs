//! The reported capability estimate, with both intervals side by side.
//!
//! The naive interval is kept and published, not discarded. That is the point: a reader who has
//! only ever seen `n = 1000` needs to see what that read would have claimed next to what the
//! evidence supports, or the correction is an unfalsifiable assertion. The ratio of the two
//! widths — [`CapabilityEstimate::inflation`] — is the single number that says how much of the
//! panel's apparent precision came from the mutation generator rather than from evidence.
//!
//! Implements the reporting half of blueprint 08.02 and 08.05, and the Gate 4 claim that a small
//! adaptive panel can "estimate the same meaningful capability differences as a much larger
//! exhaustive run": [`probability_of_superiority`] is where "meaningful difference" is given a
//! number, and it is computed on the clustered posteriors, so the answer is allowed to be "we
//! cannot tell".

use crate::beta::{self, BetaPosterior, BetaPrior, CredibleInterval};
use crate::cluster::{cluster_bootstrap_interval, BootstrapConfig, ClusterSummary, Icc};
use crate::error::AdaptiveError;
use crate::id::CapabilityId;
use serde::{Deserialize, Serialize};

/// One capability's posterior, reported honestly.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CapabilityEstimate {
    pub capability: CapabilityId,
    /// Scored trials. The number a naive report would headline.
    pub trials: usize,
    pub successes: usize,
    /// Trials that were run and could not be scored. Not evidence, but not free either.
    pub abstentions: usize,
    /// Distinct parent worlds the trials descend from.
    pub parents: usize,
    /// Shared by both intervals; see [`ClusterSummary::clustered_posterior`].
    pub posterior_mean: f64,
    pub icc: Icc,
    pub design_effect: f64,
    /// `trials / design_effect`. What the panel actually learned.
    pub effective_trials: f64,
    pub naive_posterior: BetaPosterior,
    pub clustered_posterior: BetaPosterior,
    /// The interval a report that ignored parent structure would print.
    pub naive_interval: CredibleInterval,
    /// The interval the evidence supports. Never narrower than `naive_interval`.
    pub clustered_interval: CredibleInterval,
    /// A second opinion that assumes nothing about the shape of the dependence, when there were
    /// enough parents to resample.
    pub bootstrap_interval: Option<CredibleInterval>,
    /// `clustered_interval.width() / naive_interval.width()`.
    pub inflation: f64,
    pub caveat: String,
}

impl CapabilityEstimate {
    /// Builds the estimate. Coverage is *not* checked here; see [`crate::panel::AdaptivePanel`].
    pub fn from_summary(
        capability: CapabilityId,
        summary: &ClusterSummary,
        abstentions: usize,
        prior: &BetaPrior,
        credibility: f64,
        bootstrap: Option<&BootstrapConfig>,
    ) -> Result<Self, AdaptiveError> {
        let naive_posterior = summary.naive_posterior(prior)?;
        let clustered_posterior = summary.clustered_posterior(prior)?;
        let naive_interval = naive_posterior.interval(credibility)?;
        let clustered_interval = clustered_posterior.interval(credibility)?;
        let bootstrap_interval = match bootstrap {
            Some(config) if summary.parents() >= 2 => {
                Some(cluster_bootstrap_interval(summary, config, credibility)?)
            }
            _ => None,
        };
        let inflation = if naive_interval.width() > 0.0 {
            clustered_interval.width() / naive_interval.width()
        } else {
            f64::INFINITY
        };
        Ok(CapabilityEstimate {
            capability,
            trials: summary.trials(),
            successes: summary.successes(),
            abstentions,
            parents: summary.parents(),
            posterior_mean: naive_posterior.mean(),
            icc: summary.icc(),
            design_effect: summary.design_effect(),
            effective_trials: summary.effective_trials(),
            naive_posterior,
            clustered_posterior,
            naive_interval,
            clustered_interval,
            bootstrap_interval,
            inflation,
            caveat: CAVEAT.to_string(),
        })
    }

    /// The sentence a report should lead with.
    pub fn headline(&self) -> String {
        format!(
            "{}: {:.3} [{:.3}, {:.3}] at {:.0}% from {} scored trials across {} parent(s) — \
             worth {:.1} independent trials (design effect {:.1}, rho {:.2}). Ignoring parent \
             structure would have claimed [{:.3}, {:.3}], an interval {:.1}x too narrow.",
            self.capability,
            self.posterior_mean,
            self.clustered_interval.lo,
            self.clustered_interval.hi,
            self.clustered_interval.credibility * 100.0,
            self.trials,
            self.parents,
            self.effective_trials,
            self.design_effect,
            self.icc.rho(),
            self.naive_interval.lo,
            self.naive_interval.hi,
            self.inflation
        )
    }

    /// Whether the two intervals overlap. Conservative; see
    /// [`CredibleInterval::disjoint_from`].
    pub fn separated_from(&self, other: &CapabilityEstimate) -> bool {
        self.clustered_interval
            .disjoint_from(&other.clustered_interval)
    }
}

const CAVEAT: &str = "The clustered interval assumes one level of dependence (the parent world), a \
                      common intraclass correlation across parents, and exchangeable instances \
                      within a parent. Item difficulty and discrimination are not modelled, so \
                      the point estimate is the panel's pass rate on the instances actually run, \
                      not a difficulty-adjusted ability. The effective sample size is an \
                      approximation from the design effect, not a random-effects posterior.";

/// `P(capability a > capability b)` on the **clustered** posteriors.
///
/// This is the number Gate 4 turns on. A naive panel will report near-certainty about a
/// difference that ten parents cannot support; running the same comparison through the effective
/// sample size is what lets a 500–2,000-instance panel agree with an exhaustive sweep about which
/// differences are real instead of merely agreeing about their sign.
///
/// Assumes the two posteriors are independent. Two architectures scored on the *same* instances
/// are not, and for that design this overstates separation; a paired estimator is not
/// implemented.
pub fn probability_of_superiority(a: &CapabilityEstimate, b: &CapabilityEstimate) -> f64 {
    beta::probability_first_exceeds_second(&a.clustered_posterior, &b.clustered_posterior)
}

/// The same probability computed as if every instance were independent.
///
/// Exposed only so a report can show the two next to each other. It is the wrong number; it is
/// also the number almost every benchmark leaderboard publishes.
pub fn naive_probability_of_superiority(a: &CapabilityEstimate, b: &CapabilityEstimate) -> f64 {
    beta::probability_first_exceeds_second(&a.naive_posterior, &b.naive_posterior)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cluster::Cluster;
    use crate::id::ParentId;

    fn summary(spec: &[(usize, usize)]) -> ClusterSummary {
        ClusterSummary::new(
            spec.iter()
                .enumerate()
                .map(|(i, (trials, successes))| Cluster {
                    parent: ParentId::parse(format!("p{i:03}")).unwrap(),
                    trials: *trials,
                    successes: *successes,
                })
                .collect(),
        )
    }

    fn estimate(name: &str, spec: &[(usize, usize)]) -> CapabilityEstimate {
        CapabilityEstimate::from_summary(
            CapabilityId::parse(name).unwrap(),
            &summary(spec),
            0,
            &BetaPrior::default(),
            0.95,
            Some(&BootstrapConfig::default()),
        )
        .unwrap()
    }

    #[test]
    fn the_estimate_reports_both_intervals_and_the_inflation_between_them() {
        let e = estimate("reasoning", &[(50, 48), (50, 47), (50, 8), (50, 6), (50, 45)]);
        assert_eq!(e.trials, 250);
        assert_eq!(e.parents, 5);
        assert!(e.clustered_interval.width() > e.naive_interval.width());
        assert!(e.inflation > 1.0);
        assert!(e.effective_trials < e.trials as f64);
        assert!(e.bootstrap_interval.is_some());
        assert!(e.headline().contains("too narrow"));
    }

    #[test]
    fn the_naive_comparison_is_far_more_certain_than_the_clustered_one_on_clustered_data() {
        // Two architectures, 400 instances each, six parents each. The observed rates differ by
        // four points; the parents disagree with each other by fifty.
        let a = estimate(
            "arch-a",
            &[(80, 76), (80, 12), (80, 74), (80, 14), (80, 70)],
        );
        let b = estimate(
            "arch-b",
            &[(80, 70), (80, 10), (80, 72), (80, 12), (80, 66)],
        );
        let naive = naive_probability_of_superiority(&a, &b);
        let clustered = probability_of_superiority(&a, &b);
        assert!(naive > 0.75, "naive comparison was {naive}");
        assert!(
            clustered < naive,
            "clustered {clustered} was not less certain than naive {naive}"
        );
        assert!(clustered < 0.7, "clustered comparison was {clustered}");
    }

    #[test]
    fn a_difference_that_is_real_across_parents_survives_the_clustering_correction() {
        // Every parent agrees, so the correction has little to take away.
        let a = estimate("strong", &[(40, 36), (40, 35), (40, 37), (40, 36), (40, 35)]);
        let b = estimate("weak", &[(40, 12), (40, 11), (40, 13), (40, 10), (40, 12)]);
        assert!(probability_of_superiority(&a, &b) > 0.99);
        assert!(a.separated_from(&b));
    }

    #[test]
    fn the_bootstrap_is_omitted_rather_than_faked_when_there_is_one_parent() {
        let e = estimate("single", &[(60, 30)]);
        assert!(e.bootstrap_interval.is_none());
        assert!(matches!(e.icc, Icc::Unidentifiable { .. }));
    }

    #[test]
    fn the_estimate_round_trips_through_json() {
        let e = estimate("cap", &[(20, 15), (20, 8), (20, 17)]);
        let json = serde_json::to_string(&e).unwrap();
        let back: CapabilityEstimate = serde_json::from_str(&json).unwrap();
        assert_eq!(back.capability, e.capability);
        assert_eq!(back.trials, e.trials);
        assert_eq!(back.successes, e.successes);
        assert_eq!(back.parents, e.parents);
        assert_eq!(back.icc, e.icc);
        assert!((back.posterior_mean - e.posterior_mean).abs() < 1e-12);
        assert!((back.effective_trials - e.effective_trials).abs() < 1e-9);
        assert!((back.clustered_interval.lo - e.clustered_interval.lo).abs() < 1e-12);
        assert!((back.clustered_interval.hi - e.clustered_interval.hi).abs() < 1e-12);
    }
}
