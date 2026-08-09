//! Parent-aware uncertainty: the part of adaptive evaluation that is easy to get wrong.
//!
//! This is the crate's actual contribution, so it is worth stating the problem plainly.
//! `crates/mutation` generates instances by mutating an audited parent world, and
//! `crates/mutation/src/diversity.rs` already refuses to call the instance count a benchmark
//! count. The same refusal has to be made a second time, in the *statistics*: a thousand
//! instances descended from ten parents are not a thousand independent Bernoulli trials. They
//! share the parent's structure, its vocabulary, its difficulty, and whatever quirk of it the
//! architecture under test happens to handle well or badly. Treating them as `n = 1000` produces
//! a credible interval roughly `sqrt(deff)` times too narrow, and at a realistic intraclass
//! correlation that factor is not a rounding error — it is an order of magnitude.
//!
//! Blueprint 08.02 names the fix in one line ("Random effects or cluster bootstrap account for
//! parent and mutation dependence. Generated descendants do not create artificial confidence")
//! and 08.05 makes it a stopping precondition ("Even decisive early results must satisfy minimum
//! independent-parent ... counts"). Both routes are implemented here, and both the naive and the
//! corrected interval are reported, because a correction nobody can see is a correction nobody
//! believes.
//!
//! # What is estimated and what is assumed
//!
//! * The intraclass correlation `rho` is **estimated** from the data, by the one-way ANOVA
//!   moment estimator for binary outcomes with unequal cluster sizes. It is a moment estimator,
//!   not a likelihood fit; it is noisy when the number of parents is small, which is exactly when
//!   it matters most.
//! * The design effect `1 + (m_A - 1) rho` with `m_A = sum(m^2)/sum(m)` is the standard Kish
//!   form for unequal cluster sizes. It **assumes a common `rho` across parents** and a common
//!   success probability within the capability; neither is checked.
//! * The effective sample size `n / deff` is then fed to a Beta with the naive posterior mean.
//!   That is an approximation, not a posterior derived from a random-effects likelihood. It is
//!   reported as such.
//! * The cluster bootstrap resamples **parents**, not instances, and makes no `rho` assumption at
//!   all — but it needs enough parents to have something to resample, and its percentile interval
//!   is coarse below roughly twenty clusters.
//!
//! # What is not modelled
//!
//! Nested levels below the parent (repeated trials of the *same instance*, or mutation-family
//! sub-clusters within a parent) are not modelled. The ledger in [`crate::ledger`] therefore
//! *refuses* a second scored trial on an instance, so the one-level model is never silently
//! applied to two-level data. Cross-capability correlation is not modelled either: capabilities
//! are treated as separate estimation problems even when they share parents.

use crate::beta::{BetaPosterior, BetaPrior, CredibleInterval};
use crate::error::AdaptiveError;
use crate::id::ParentId;
use crate::rng::SplitMix64;
use serde::{Deserialize, Serialize};

/// Scored trials from one parent, for one capability.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cluster {
    pub parent: ParentId,
    pub trials: usize,
    pub successes: usize,
}

impl Cluster {
    pub fn rate(&self) -> f64 {
        if self.trials == 0 {
            0.0
        } else {
            self.successes as f64 / self.trials as f64
        }
    }
}

/// What the intraclass correlation estimate actually is, in each case.
///
/// The variants exist because the three situations demand different honesty. Distinct parents
/// everywhere means clustering provably cannot inflate anything. A single parent, or outcomes
/// with no variance at all, means `rho` is *unidentifiable* — and the convenient assumption there
/// is `rho = 0`, which is precisely the assumption that manufactures confidence out of nothing.
/// This crate assumes the opposite.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Icc {
    /// Every scored trial came from a distinct parent, so the adjusted cluster size is one and
    /// the design effect is one whatever `rho` happens to be.
    NoClustering,
    /// Estimated by the one-way ANOVA moment estimator.
    Estimated {
        rho: f64,
        /// The raw moment estimate before clamping to `[0, 1]`. A negative raw value means the
        /// within-parent variance exceeded the between-parent variance, which happens by chance
        /// with few parents; it is clamped to zero rather than treated as evidence of negative
        /// dependence.
        raw: f64,
    },
    /// `rho` cannot be identified from these data. The worst case is assumed.
    ///
    /// The reason is carried as text because it is the part a reader needs: "this panel assumed
    /// total dependence" is only actionable alongside *why* it had to.
    Unidentifiable { assumed: f64, reason: String },
}

impl Icc {
    pub fn rho(&self) -> f64 {
        match self {
            Icc::NoClustering => 0.0,
            Icc::Estimated { rho, .. } => *rho,
            Icc::Unidentifiable { assumed, .. } => *assumed,
        }
    }

    pub fn is_estimated(&self) -> bool {
        matches!(self, Icc::Estimated { .. })
    }
}

/// Trials for one capability, grouped by the parent they descend from.
///
/// Clusters are held in `ParentId` order so that every derived quantity — the estimate, the
/// bootstrap, the digest of an audit record — is a function of the data and not of insertion
/// order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ClusterSummary {
    pub clusters: Vec<Cluster>,
}

impl ClusterSummary {
    pub fn new(mut clusters: Vec<Cluster>) -> Self {
        clusters.sort_by(|a, b| a.parent.cmp(&b.parent));
        clusters.retain(|c| c.trials > 0);
        ClusterSummary { clusters }
    }

    pub fn parents(&self) -> usize {
        self.clusters.len()
    }

    pub fn trials(&self) -> usize {
        self.clusters.iter().map(|c| c.trials).sum()
    }

    pub fn successes(&self) -> usize {
        self.clusters.iter().map(|c| c.successes).sum()
    }

    pub fn failures(&self) -> usize {
        self.trials() - self.successes()
    }

    pub fn is_empty(&self) -> bool {
        self.clusters.is_empty()
    }

    /// The observed success rate, ignoring clustering entirely.
    pub fn raw_rate(&self) -> f64 {
        let n = self.trials();
        if n == 0 {
            0.0
        } else {
            self.successes() as f64 / n as f64
        }
    }

    /// Kish's adjusted cluster size, `sum(m^2) / sum(m)`.
    ///
    /// Equal to the mean cluster size inflated by its coefficient of variation,
    /// `m_bar (1 + cv^2)`. Unequal parent contributions make clustering *worse* than the mean
    /// size alone suggests, which is why a scheduler that piles trials onto one convenient parent
    /// damages the interval faster than it improves it.
    pub fn adjusted_cluster_size(&self) -> f64 {
        let n = self.trials();
        if n == 0 {
            return 0.0;
        }
        let sum_squares: f64 = self.clusters.iter().map(|c| (c.trials * c.trials) as f64).sum();
        sum_squares / n as f64
    }

    /// The one-way ANOVA moment estimator of the intraclass correlation.
    ///
    /// With `K` parents, sizes `m_k`, within-parent rates `p_k` and overall rate `p`:
    ///
    /// ```text
    ///   MSB = sum m_k (p_k - p)^2 / (K - 1)
    ///   MSW = sum m_k p_k (1 - p_k) / (n - K)
    ///   m_0 = (n - sum m_k^2 / n) / (K - 1)
    ///   rho = (MSB - MSW) / (MSB + (m_0 - 1) MSW)
    /// ```
    ///
    /// This is the estimator usually attributed to Fleiss and Cuzick for binary data with unequal
    /// cluster sizes. It is cheap, it needs no iteration, and it is *biased downward* when the
    /// number of parents is small — meaning the correction below is, if anything, too gentle.
    ///
    /// It also has a degeneracy that has to be guarded rather than trusted. `MSW` carries
    /// `n - K` degrees of freedom, and a singleton parent contributes exactly zero to the within
    /// sum by construction. A panel that has just started — most parents sampled once, one
    /// sampled twice — therefore has one degree of freedom or none, and if that single
    /// informative pair happens to agree with itself the estimator returns `rho = 1`. That is
    /// not a finding about the data; it is the estimator running out of information, and the
    /// consequence is severe: at `rho = 1` the marginal weight of any repeat is zero (see
    /// [`marginal_independent_weight`]), so every candidate from an already-touched parent
    /// scores zero and the capability stops being selectable at all. It was found exactly that
    /// way, by a panel that abandoned two of its three capabilities after twenty-five trials
    /// each. Below two multi-trial parents the answer is [`Icc::Unidentifiable`] instead.
    pub fn icc(&self) -> Icc {
        let k = self.parents();
        let n = self.trials();
        if n == 0 {
            return Icc::Unidentifiable {
                assumed: 1.0,
                reason: "no scored trials".into(),
            };
        }
        if self.clusters.iter().all(|c| c.trials == 1) {
            return Icc::NoClustering;
        }
        if k < 2 {
            return Icc::Unidentifiable {
                assumed: 1.0,
                reason: "a single parent supplies every trial, so between-parent variance is \
                         unobservable and the trials could all be one fact repeated"
                    .into(),
            };
        }

        let multi_trial_parents = self.clusters.iter().filter(|c| c.trials >= 2).count();
        if multi_trial_parents < 2 || n - k < 2 {
            return Icc::Unidentifiable {
                assumed: 1.0,
                reason: "fewer than two parents contributed more than one trial, so the \
                         within-parent variance has almost no degrees of freedom and dependence \
                         cannot be separated from difficulty"
                    .into(),
            };
        }

        let p = self.raw_rate();
        let between: f64 = self
            .clusters
            .iter()
            .map(|c| {
                let d = c.rate() - p;
                c.trials as f64 * d * d
            })
            .sum::<f64>()
            / (k - 1) as f64;

        let within_denominator = (n - k) as f64;
        let within = if within_denominator <= 0.0 {
            0.0
        } else {
            self.clusters
                .iter()
                .map(|c| c.trials as f64 * c.rate() * (1.0 - c.rate()))
                .sum::<f64>()
                / within_denominator
        };

        let m0 = (n as f64 - self.adjusted_cluster_size()) / (k - 1) as f64;
        let denominator = between + (m0 - 1.0) * within;
        if denominator <= 0.0 {
            return Icc::Unidentifiable {
                assumed: 1.0,
                reason: "outcomes carry no variance within or between parents, so the data cannot \
                         distinguish independent trials from one parent-level fact repeated"
                    .into(),
            };
        }
        let raw = (between - within) / denominator;
        Icc::Estimated {
            rho: raw.clamp(0.0, 1.0),
            raw,
        }
    }

    /// `1 + (m_A - 1) rho`, never below one.
    pub fn design_effect(&self) -> f64 {
        let rho = self.icc().rho();
        (1.0 + (self.adjusted_cluster_size() - 1.0) * rho).max(1.0)
    }

    /// The number of independent trials the clustered evidence is actually worth.
    ///
    /// This, not the raw count, is what a suite should report. Blueprint 08 lists
    /// "Generated instances inflate scale while adding little independent diagnostic
    /// information" as a named failure mode; this number is the one that makes it visible.
    pub fn effective_trials(&self) -> f64 {
        let n = self.trials() as f64;
        if n == 0.0 {
            0.0
        } else {
            n / self.design_effect()
        }
    }

    /// The naive posterior: every trial counted as one independent observation.
    ///
    /// Kept and reported precisely so the reader can see how much confidence it invents.
    pub fn naive_posterior(&self, prior: &BetaPrior) -> Result<BetaPosterior, AdaptiveError> {
        prior.update(self.successes() as f64, self.failures() as f64)
    }

    /// The clustered posterior: the naive mean carried onto the effective sample size.
    ///
    /// The mean is held identical to the naive posterior's on purpose. Clustering is a statement
    /// about how much you know, not about what you believe; moving the point estimate as well
    /// would make the two intervals incomparable and let a reader attribute the widening to a
    /// shift in the estimate.
    pub fn clustered_posterior(&self, prior: &BetaPrior) -> Result<BetaPosterior, AdaptiveError> {
        let naive = self.naive_posterior(prior)?;
        naive.with_mass(prior.mass() + self.effective_trials())
    }
}

/// Seed and draw count for the cluster bootstrap.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BootstrapConfig {
    pub seed: u64,
    pub draws: usize,
}

impl Default for BootstrapConfig {
    fn default() -> Self {
        BootstrapConfig {
            seed: 0x0B10_9815,
            draws: 2000,
        }
    }
}

/// A percentile interval from resampling **parents** with replacement.
///
/// The second of the two routes 08.02 names. It assumes nothing about the shape of the
/// within-parent dependence — a parent is drawn whole or not at all — which makes it the better
/// check when the common-`rho` assumption behind the design effect is doubtful.
///
/// Its weaknesses are equally real and are not hidden: with `K` parents the resampled mean can
/// take at most a lattice of values, the percentile interval has no small-sample correction (no
/// BCa, no studentisation), and with few parents it can land *narrower* than the design-effect
/// interval. It is offered as a second opinion, never as the reported interval.
pub fn cluster_bootstrap_interval(
    summary: &ClusterSummary,
    config: &BootstrapConfig,
    credibility: f64,
) -> Result<CredibleInterval, AdaptiveError> {
    if summary.parents() < 2 {
        return Err(AdaptiveError::BootstrapNeedsClusters(summary.parents()));
    }
    if config.draws == 0 {
        return Err(AdaptiveError::BootstrapNeedsDraws);
    }
    if !credibility.is_finite() || credibility <= 0.0 || credibility >= 1.0 {
        return Err(AdaptiveError::InvalidCredibility(credibility));
    }

    let k = summary.parents();
    let mut rng = SplitMix64::new(config.seed);
    let mut rates = Vec::with_capacity(config.draws);
    for _ in 0..config.draws {
        let mut trials = 0usize;
        let mut successes = 0usize;
        for _ in 0..k {
            let cluster = &summary.clusters[rng.below(k)];
            trials += cluster.trials;
            successes += cluster.successes;
        }
        rates.push(successes as f64 / trials as f64);
    }
    rates.sort_by(|a, b| a.total_cmp(b));

    let tail = 0.5 * (1.0 - credibility);
    Ok(CredibleInterval {
        lo: percentile(&rates, tail),
        hi: percentile(&rates, 1.0 - tail),
        credibility,
    })
}

fn percentile(sorted: &[f64], q: f64) -> f64 {
    let n = sorted.len();
    if n == 1 {
        return sorted[0];
    }
    let position = q * (n - 1) as f64;
    let lower = position.floor() as usize;
    let upper = (lower + 1).min(n - 1);
    let fraction = position - lower as f64;
    sorted[lower] * (1.0 - fraction) + sorted[upper] * fraction
}

/// The independent information a *further* trial from a parent already sampled `m` times adds.
///
/// Derived directly from the design effect. A cluster of size `m` is worth `m / (1 + rho(m-1))`
/// independent observations, so the marginal worth of the next one is
///
/// ```text
///   (m+1)/(1 + rho m) - m/(1 + rho(m-1))  =  (1 - rho) / ((1 + rho m)(1 + rho(m-1)))
/// ```
///
/// which is exactly `1` at `m = 0`, falls off like `1/(rho m)^2`, and is `0` for every repeat
/// when `rho = 1`. It needs no tuning constant and no diversity bonus: the "penalize cells highly
/// correlated with already-run parents" requirement of 08.03 falls out of the same `rho` that
/// widens the interval, which is the only way the two can stay consistent with each other.
pub fn marginal_independent_weight(existing_trials: usize, rho: f64) -> f64 {
    let rho = rho.clamp(0.0, 1.0);
    let m = existing_trials as f64;
    if rho >= 1.0 {
        return if existing_trials == 0 { 1.0 } else { 0.0 };
    }
    (1.0 - rho) / ((1.0 + rho * m) * (1.0 + rho * (m - 1.0)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parent(name: &str) -> ParentId {
        ParentId::parse(name).unwrap()
    }

    fn summary(spec: &[(usize, usize)]) -> ClusterSummary {
        ClusterSummary::new(
            spec.iter()
                .enumerate()
                .map(|(i, (trials, successes))| Cluster {
                    parent: parent(&format!("parent-{i:03}")),
                    trials: *trials,
                    successes: *successes,
                })
                .collect(),
        )
    }

    #[test]
    fn distinct_parents_everywhere_means_no_clustering_correction_at_all() {
        let s = summary(&[(1, 1), (1, 0), (1, 1), (1, 0), (1, 1)]);
        assert_eq!(s.icc(), Icc::NoClustering);
        assert_eq!(s.design_effect(), 1.0);
        assert_eq!(s.effective_trials(), 5.0);
    }

    #[test]
    fn the_adjusted_cluster_size_exceeds_the_mean_when_parents_contribute_unequally() {
        let equal = summary(&[(10, 5), (10, 5), (10, 5), (10, 5)]);
        let lopsided = summary(&[(31, 15), (3, 2), (3, 1), (3, 2)]);
        assert!((equal.adjusted_cluster_size() - 10.0).abs() < 1e-12);
        assert_eq!(equal.trials(), lopsided.trials());
        assert!(lopsided.adjusted_cluster_size() > equal.adjusted_cluster_size());
    }

    #[test]
    fn perfectly_homogeneous_parents_collapse_to_one_effective_trial_per_parent() {
        // Ten parents, a hundred instances each. Every instance of a parent gives the same
        // answer, so the panel has learned ten facts, not a thousand.
        let mut clusters = Vec::new();
        for i in 0..10 {
            clusters.push(Cluster {
                parent: parent(&format!("p{i}")),
                trials: 100,
                successes: if i < 6 { 100 } else { 0 },
            });
        }
        let s = ClusterSummary::new(clusters);
        assert_eq!(s.trials(), 1000);
        let icc = s.icc();
        assert!(icc.is_estimated(), "{icc:?}");
        let rho = icc.rho();
        assert!(rho > 0.99, "rho was {rho}");
        assert!((s.effective_trials() - 10.0).abs() < 0.2);
    }

    #[test]
    fn a_single_parent_is_treated_as_unidentifiable_and_assumed_worst_case() {
        let s = summary(&[(64, 40)]);
        let icc = s.icc();
        assert!(matches!(icc, Icc::Unidentifiable { assumed, .. } if assumed == 1.0));
        assert!((s.effective_trials() - 1.0).abs() < 1e-9);
    }

    #[test]
    fn outcomes_with_no_variance_are_unidentifiable_rather_than_conveniently_independent() {
        let s = summary(&[(20, 20), (20, 20), (20, 20)]);
        assert!(matches!(s.icc(), Icc::Unidentifiable { .. }));
        assert!(s.design_effect() > 19.0);
        assert!(s.effective_trials() <= 3.0 + 1e-9);
    }

    #[test]
    fn a_negative_raw_icc_is_clamped_to_zero_rather_than_credited() {
        // Parents alternate internally, so within-parent variance is maximal and between-parent
        // variance is nil; the moment estimator goes negative.
        let s = summary(&[(20, 10), (20, 10), (20, 10), (20, 10)]);
        match s.icc() {
            Icc::Estimated { rho, raw } => {
                assert!(raw < 0.0, "expected a negative raw estimate, got {raw}");
                assert_eq!(rho, 0.0);
            }
            other => panic!("expected an estimate, got {other:?}"),
        }
        assert_eq!(s.design_effect(), 1.0);
    }

    #[test]
    fn the_parent_clustered_interval_is_wider_than_the_naive_one() {
        let prior = BetaPrior::default();
        // Ten parents, fifty instances each, strongly parent-driven outcomes.
        let mut clusters = Vec::new();
        for i in 0..10 {
            clusters.push(Cluster {
                parent: parent(&format!("p{i:02}")),
                trials: 50,
                successes: if i < 7 { 46 } else { 6 },
            });
        }
        let s = ClusterSummary::new(clusters);
        let naive = s.naive_posterior(&prior).unwrap().interval(0.95).unwrap();
        let clustered = s.clustered_posterior(&prior).unwrap().interval(0.95).unwrap();
        assert!(
            clustered.width() > naive.width(),
            "clustered {clustered:?} was not wider than naive {naive:?}"
        );
        assert!(clustered.width() / naive.width() > 3.0);
    }

    #[test]
    fn a_naive_interval_materially_understates_uncertainty_on_clustered_data() {
        // The concrete failure this crate exists to prevent: 1,000 instances from 10 parents.
        // The naive read says the capability is pinned to within a couple of points; the truth is
        // that ten parents were observed and the panel cannot resolve it nearly that finely.
        let prior = BetaPrior::default();
        let mut clusters = Vec::new();
        for i in 0..10 {
            clusters.push(Cluster {
                parent: parent(&format!("p{i:02}")),
                trials: 100,
                successes: if i < 6 { 95 } else { 10 },
            });
        }
        let s = ClusterSummary::new(clusters);
        let naive = s.naive_posterior(&prior).unwrap().interval(0.95).unwrap();
        let clustered = s.clustered_posterior(&prior).unwrap().interval(0.95).unwrap();

        assert!(naive.width() < 0.07, "naive width was {}", naive.width());
        assert!(
            clustered.width() > 0.45,
            "clustered width was {}",
            clustered.width()
        );
        // A thousand instances worth roughly fourteen trials. That factor of seventy is the
        // entire argument for this module.
        assert!(s.effective_trials() < 15.0, "{}", s.effective_trials());
        assert_eq!(s.trials(), 1000);
    }

    #[test]
    fn the_clustered_interval_is_never_narrower_across_a_grid_of_configurations() {
        let prior = BetaPrior::default();
        for parents in [2usize, 3, 5, 9, 17] {
            for size in [1usize, 2, 7, 40] {
                for rate_numerator in [0usize, 1, 3, 4] {
                    let clusters: Vec<Cluster> = (0..parents)
                        .map(|i| Cluster {
                            parent: parent(&format!("p{i:03}")),
                            trials: size,
                            successes: (size * (rate_numerator + i % 2)) / 5,
                        })
                        .collect();
                    let s = ClusterSummary::new(clusters);
                    let naive = s.naive_posterior(&prior).unwrap().interval(0.9).unwrap();
                    let clustered = s.clustered_posterior(&prior).unwrap().interval(0.9).unwrap();
                    assert!(
                        clustered.width() >= naive.width() - 1e-12,
                        "parents={parents} size={size} num={rate_numerator}: \
                         clustered {clustered:?} narrower than naive {naive:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn the_cluster_bootstrap_is_reproducible_from_its_seed_and_moves_with_it() {
        let s = summary(&[(20, 19), (20, 18), (20, 3), (20, 2), (20, 17), (20, 4)]);
        let config = BootstrapConfig {
            seed: 12345,
            draws: 1000,
        };
        let first = cluster_bootstrap_interval(&s, &config, 0.95).unwrap();
        let second = cluster_bootstrap_interval(&s, &config, 0.95).unwrap();
        assert_eq!(first, second);
        let other = cluster_bootstrap_interval(
            &s,
            &BootstrapConfig {
                seed: 999,
                draws: 1000,
            },
            0.95,
        )
        .unwrap();
        assert_ne!(first, other);
    }

    #[test]
    fn the_cluster_bootstrap_also_rejects_the_naive_interval_on_clustered_data() {
        let prior = BetaPrior::default();
        let mut clusters = Vec::new();
        for i in 0..12 {
            clusters.push(Cluster {
                parent: parent(&format!("p{i:02}")),
                trials: 40,
                successes: if i < 7 { 38 } else { 3 },
            });
        }
        let s = ClusterSummary::new(clusters);
        let naive = s.naive_posterior(&prior).unwrap().interval(0.95).unwrap();
        let bootstrap = cluster_bootstrap_interval(&s, &BootstrapConfig::default(), 0.95).unwrap();
        assert!(
            bootstrap.width() > naive.width() * 3.0,
            "bootstrap {bootstrap:?} vs naive {naive:?}"
        );
    }

    #[test]
    fn the_bootstrap_refuses_to_run_on_a_single_parent() {
        let s = summary(&[(50, 25)]);
        assert!(matches!(
            cluster_bootstrap_interval(&s, &BootstrapConfig::default(), 0.95),
            Err(AdaptiveError::BootstrapNeedsClusters(1))
        ));
    }

    #[test]
    fn marginal_independent_weight_is_one_for_a_fresh_parent_and_decays_thereafter() {
        for rho in [0.0, 0.1, 0.5, 0.9] {
            assert!((marginal_independent_weight(0, rho) - 1.0).abs() < 1e-12);
        }
        let mut previous = f64::INFINITY;
        for m in 0..12 {
            let w = marginal_independent_weight(m, 0.4);
            assert!(w < previous, "weight did not decay at m={m}");
            assert!(w > 0.0);
            previous = w;
        }
        assert_eq!(marginal_independent_weight(3, 1.0), 0.0);
        assert_eq!(marginal_independent_weight(0, 1.0), 1.0);
        for m in 0..8 {
            assert_eq!(marginal_independent_weight(m, 0.0), 1.0);
        }
    }

    #[test]
    fn marginal_weights_telescope_to_the_clusters_effective_size() {
        // The weights are marginal differences of m / (1 + rho(m-1)); summing them must return
        // the cluster's own effective size, or selection and inference disagree about what a
        // trial is worth.
        let rho = 0.35;
        let m = 9usize;
        let summed: f64 = (0..m).map(|k| marginal_independent_weight(k, rho)).sum();
        let closed = m as f64 / (1.0 + rho * (m as f64 - 1.0));
        assert!((summed - closed).abs() < 1e-12, "{summed} vs {closed}");
    }
}
