//! The Beta-Bernoulli capability posterior, written out longhand.
//!
//! Blueprint 08.02 asks for a hierarchical logistic or ordinal item-response model and then
//! immediately qualifies it: "Use simpler empirical estimates until data supports complexity."
//! This module is the simpler empirical estimate. One capability, one success probability, a
//! conjugate Beta prior, Bernoulli trials.
//!
//! **What is deliberately not modelled here.** Item difficulty and discrimination (the 2PL item
//! model of 08.02) are absent: every trial of a capability is treated as an exchangeable draw
//! from the same Bernoulli. So is the multidimensional loading of one cell onto several
//! capabilities. Both would change the *point estimate*, not just its uncertainty, and neither
//! can be fitted honestly from the handful of trials a 500–2,000-instance panel affords per
//! capability. The clustering correction lives in [`crate::cluster`] and is applied on top of
//! this module rather than inside it, so the two approximations stay separable and separately
//! falsifiable.
//!
//! Everything numerical is written here rather than pulled from a statistics crate: the
//! Lanczos log-gamma, the continued-fraction regularized incomplete beta, quantiles by bisection
//! on the CDF, and a substituted Simpson rule for the probability that one posterior exceeds
//! another. Accuracy claims are stated with each function and tested against closed forms.

use crate::error::AdaptiveError;
use serde::{Deserialize, Serialize};
use std::f64::consts::PI;

/// Log-gamma by the Lanczos approximation, `g = 7` with nine coefficients.
///
/// Relative error below `1e-13` over the range this crate uses (arguments are posterior masses,
/// so `>= 1`). Defined only for strictly positive arguments; the reflection formula for negative
/// arguments is not implemented because no caller can reach it.
pub fn ln_gamma(x: f64) -> f64 {
    debug_assert!(x > 0.0, "ln_gamma is only defined here for x > 0");
    const LANCZOS: [f64; 9] = [
        0.999_999_999_999_809_9,
        676.520_368_121_885_1,
        -1_259.139_216_722_402_8,
        771.323_428_777_653_1,
        -176.615_029_162_140_6,
        12.507_343_278_686_905,
        -0.138_571_095_265_720_1,
        9.984_369_578_019_572e-6,
        1.505_632_735_149_311_6e-7,
    ];
    let z = x - 1.0;
    let mut series = LANCZOS[0];
    for (i, coefficient) in LANCZOS.iter().enumerate().skip(1) {
        series += coefficient / (z + i as f64);
    }
    let t = z + 7.5;
    0.5 * (2.0 * PI).ln() + (z + 0.5) * t.ln() - t + series.ln()
}

/// `ln B(a, b)`, the log of the beta function.
pub fn ln_beta(a: f64, b: f64) -> f64 {
    ln_gamma(a) + ln_gamma(b) - ln_gamma(a + b)
}

/// The regularized incomplete beta function `I_x(a, b)`, i.e. the Beta CDF.
///
/// Evaluated by the modified-Lentz continued fraction, using the `I_x(a,b) = 1 - I_{1-x}(b,a)`
/// reflection on the slowly converging side. Absolute error below `1e-14` for the parameter
/// ranges here.
pub fn regularized_incomplete_beta(x: f64, a: f64, b: f64) -> f64 {
    if !x.is_finite() || x <= 0.0 {
        return 0.0;
    }
    if x >= 1.0 {
        return 1.0;
    }
    let ln_front = a * x.ln() + b * (1.0 - x).ln() - ln_beta(a, b);
    if x < (a + 1.0) / (a + b + 2.0) {
        ln_front.exp() * continued_fraction(x, a, b) / a
    } else {
        1.0 - ln_front.exp() * continued_fraction(1.0 - x, b, a) / b
    }
}

fn continued_fraction(x: f64, a: f64, b: f64) -> f64 {
    const MAX_ITERATIONS: usize = 400;
    const EPSILON: f64 = 3.0e-16;
    const TINY: f64 = 1.0e-300;

    let qab = a + b;
    let qap = a + 1.0;
    let qam = a - 1.0;
    let mut c = 1.0;
    let mut d = 1.0 - qab * x / qap;
    if d.abs() < TINY {
        d = TINY;
    }
    d = 1.0 / d;
    let mut h = d;

    for m in 1..=MAX_ITERATIONS {
        let m = m as f64;
        let m2 = 2.0 * m;

        let even = m * (b - m) * x / ((qam + m2) * (a + m2));
        d = 1.0 + even * d;
        if d.abs() < TINY {
            d = TINY;
        }
        c = 1.0 + even / c;
        if c.abs() < TINY {
            c = TINY;
        }
        d = 1.0 / d;
        h *= d * c;

        let odd = -(a + m) * (qab + m) * x / ((a + m2) * (qap + m2));
        d = 1.0 + odd * d;
        if d.abs() < TINY {
            d = TINY;
        }
        c = 1.0 + odd / c;
        if c.abs() < TINY {
            c = TINY;
        }
        d = 1.0 / d;
        let delta = d * c;
        h *= delta;

        if (delta - 1.0).abs() < EPSILON {
            break;
        }
    }
    h
}

/// An equal-tailed credible interval.
///
/// Equal-tailed rather than highest-posterior-density: for a U-shaped Beta (which the Jeffreys
/// prior produces before any data arrives) the HPD region is two disjoint intervals, and a
/// reporting surface that can only draw one bar would silently show the wrong thing.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CredibleInterval {
    pub lo: f64,
    pub hi: f64,
    /// The mass the interval carries, e.g. `0.95`.
    pub credibility: f64,
}

impl CredibleInterval {
    pub fn width(&self) -> f64 {
        self.hi - self.lo
    }

    pub fn contains(&self, value: f64) -> bool {
        value >= self.lo && value <= self.hi
    }

    /// Whether two intervals are disjoint.
    ///
    /// A conservative separation test, not a posterior probability: disjoint intervals imply a
    /// difference, overlapping intervals do not imply its absence. Use
    /// [`probability_first_exceeds_second`] when the question is "how sure are we".
    pub fn disjoint_from(&self, other: &CredibleInterval) -> bool {
        self.hi < other.lo || other.hi < self.lo
    }
}

/// Prior pseudo-counts for a capability's success probability.
///
/// Defaults to Jeffreys, `Beta(0.5, 0.5)`. Chosen over the uniform `Beta(1, 1)` because it adds
/// one pseudo-observation in total rather than two, and because it is invariant to
/// reparameterisation of the success probability — a capability's posterior should not depend on
/// whether the panel reports pass-rate or log-odds. It is still a *choice*, and it still matters
/// at the small effective sample sizes a clustered panel produces: at `n_eff = 4` the prior is a
/// fifth of the evidence.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BetaPrior {
    pub alpha: f64,
    pub beta: f64,
}

impl Default for BetaPrior {
    fn default() -> Self {
        BetaPrior {
            alpha: 0.5,
            beta: 0.5,
        }
    }
}

impl BetaPrior {
    pub fn new(alpha: f64, beta: f64) -> Result<Self, AdaptiveError> {
        BetaPosterior::new(alpha, beta).map(|p| BetaPrior {
            alpha: p.alpha,
            beta: p.beta,
        })
    }

    pub fn mass(&self) -> f64 {
        self.alpha + self.beta
    }

    /// The posterior after `successes` successes and `failures` failures.
    ///
    /// Counts are `f64` because the clustered path feeds fractional effective counts through the
    /// same conjugate update.
    pub fn update(&self, successes: f64, failures: f64) -> Result<BetaPosterior, AdaptiveError> {
        BetaPosterior::new(self.alpha + successes, self.beta + failures)
    }
}

/// A Beta distribution over one capability's success probability.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BetaPosterior {
    pub alpha: f64,
    pub beta: f64,
}

impl BetaPosterior {
    pub fn new(alpha: f64, beta: f64) -> Result<Self, AdaptiveError> {
        if !alpha.is_finite() || !beta.is_finite() || alpha <= 0.0 || beta <= 0.0 {
            return Err(AdaptiveError::InvalidBetaParameters { alpha, beta });
        }
        Ok(BetaPosterior { alpha, beta })
    }

    pub fn mean(&self) -> f64 {
        self.alpha / (self.alpha + self.beta)
    }

    /// Total pseudo-count, prior plus evidence. The quantity the clustered path deflates.
    pub fn mass(&self) -> f64 {
        self.alpha + self.beta
    }

    pub fn variance(&self) -> f64 {
        let m = self.mass();
        self.alpha * self.beta / (m * m * (m + 1.0))
    }

    pub fn cdf(&self, x: f64) -> f64 {
        regularized_incomplete_beta(x, self.alpha, self.beta)
    }

    pub fn pdf(&self, x: f64) -> f64 {
        if x <= 0.0 || x >= 1.0 {
            return 0.0;
        }
        ((self.alpha - 1.0) * x.ln() + (self.beta - 1.0) * (1.0 - x).ln() - ln_beta(self.alpha, self.beta))
            .exp()
    }

    /// The `q`-quantile, by bisection on the CDF.
    ///
    /// Bisection rather than Newton: the CDF is monotone by construction so bisection cannot
    /// diverge, and 200 halvings of `[0, 1]` reach the limit of `f64` resolution. Determinism
    /// matters more here than iteration count — a suite that reports a different interval on a
    /// different machine is not auditable.
    pub fn quantile(&self, q: f64) -> Result<f64, AdaptiveError> {
        if !q.is_finite() || !(0.0..=1.0).contains(&q) {
            return Err(AdaptiveError::InvalidProbability(q));
        }
        if q == 0.0 {
            return Ok(0.0);
        }
        if q == 1.0 {
            return Ok(1.0);
        }
        let mut lo = 0.0f64;
        let mut hi = 1.0f64;
        for _ in 0..200 {
            let mid = 0.5 * (lo + hi);
            if mid <= lo || mid >= hi {
                break;
            }
            if self.cdf(mid) < q {
                lo = mid;
            } else {
                hi = mid;
            }
            if hi - lo < 1.0e-15 {
                break;
            }
        }
        Ok(0.5 * (lo + hi))
    }

    pub fn interval(&self, credibility: f64) -> Result<CredibleInterval, AdaptiveError> {
        if !credibility.is_finite() || credibility <= 0.0 || credibility >= 1.0 {
            return Err(AdaptiveError::InvalidCredibility(credibility));
        }
        let tail = 0.5 * (1.0 - credibility);
        Ok(CredibleInterval {
            lo: self.quantile(tail)?,
            hi: self.quantile(1.0 - tail)?,
            credibility,
        })
    }

    /// The expected reduction in posterior variance from one more Bernoulli trial.
    ///
    /// Closed form, no approximation. With `n = alpha + beta`, observing one trial gives
    /// `Beta(alpha+1, beta)` with probability `alpha/n` and `Beta(alpha, beta+1)` otherwise; the
    /// expected posterior variance collapses to `alpha*beta / (n (n+1)^2)`, and subtracting it
    /// from the current variance `alpha*beta / (n^2 (n+1))` leaves
    ///
    /// ```text
    ///     alpha * beta / (n^2 (n+1)^2)  =  variance / (n + 1).
    /// ```
    ///
    /// This is the base term of the acquisition score in [`crate::select`]. It is strictly
    /// positive, it shrinks like `1/n^3`, and it is largest where the posterior is both diffuse
    /// and near one half — which is the behaviour an information-directed policy should have
    /// (43.15) without any tuning constant.
    pub fn expected_variance_reduction(&self) -> f64 {
        self.variance() / (self.mass() + 1.0)
    }

    /// The same distribution's mean carried onto a different total pseudo-count.
    ///
    /// This is how the clustered interval is formed: keep the point estimate the naive posterior
    /// reports, and replace its confidence with the confidence the *effective* sample size
    /// supports. Because the mean is held fixed and Beta variance at fixed mean is
    /// `mean(1-mean)/(mass+1)`, deflating the mass strictly increases the variance — which is
    /// what makes "the clustered interval is never narrower" a theorem rather than a hope.
    pub fn with_mass(&self, mass: f64) -> Result<Self, AdaptiveError> {
        if !mass.is_finite() || mass <= 0.0 {
            return Err(AdaptiveError::InvalidBetaParameters {
                alpha: f64::NAN,
                beta: mass,
            });
        }
        let mean = self.mean();
        BetaPosterior::new(mean * mass, (1.0 - mean) * mass)
    }
}

/// `P(X > Y)` for independent Beta-distributed `X` and `Y`.
///
/// Computed as `∫ f_X(x) F_Y(x) dx` under the substitution `x = 3u² - 2u³`, whose derivative
/// `6u(1-u)` vanishes at both endpoints fast enough to cancel the integrable endpoint
/// singularities a `Beta(a, b)` density has when `a < 1` or `b < 1` — including the Jeffreys
/// prior's `Beta(0.5, 0.5)`. Composite Simpson with 4096 panels over `u`; absolute error below
/// `1e-9` in the tests below, which include two closed forms and the exact symmetry
/// `P(X>Y) + P(Y>X) = 1`.
///
/// Two numerical details are load-bearing rather than defensive. The endpoints are sampled a
/// hair inside `[0, 1]`, because the substituted integrand has a finite but *non-zero* limit at
/// `u = 1` whenever `X` has `beta < 1`, and evaluating exactly at the pole yields a guarded
/// zero — a one-panel error of order `1e-4`. And `1 - x` is formed as `(1-u)^2 (1+2u)` rather
/// than by subtraction, because near `u = 1` the subtraction cancels to exactly `1.0` in `f64`
/// and destroys precisely the quantity the pole needs.
///
/// Independence of `X` and `Y` is an assumption, not a fact: two architectures evaluated on the
/// *same* instances share the instance-level noise, and their posteriors are positively
/// correlated. This function therefore *overstates* separation for paired designs. A paired
/// estimator is not implemented.
pub fn probability_first_exceeds_second(x: &BetaPosterior, y: &BetaPosterior) -> f64 {
    const PANELS: usize = 4096;
    const INSET: f64 = 1.0e-10;
    let ln_norm = ln_beta(x.alpha, x.beta);
    let h = 1.0 / PANELS as f64;
    let mut total = 0.0;
    for i in 0..=PANELS {
        let u = (i as f64 * h).clamp(INSET, 1.0 - INSET);
        let v = 1.0 - u;
        let t = u * u * (3.0 - 2.0 * u);
        let one_minus_t = v * v * (1.0 + 2.0 * u);
        let ln_density = (x.alpha - 1.0) * t.ln() + (x.beta - 1.0) * one_minus_t.ln() - ln_norm;
        let value = ln_density.exp() * y.cdf(t) * 6.0 * u * v;
        let value = if value.is_finite() { value } else { 0.0 };
        let weight = if i == 0 || i == PANELS {
            1.0
        } else if i % 2 == 1 {
            4.0
        } else {
            2.0
        };
        total += weight * value;
    }
    (total * h / 3.0).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64, tolerance: f64) -> bool {
        (a - b).abs() <= tolerance
    }

    #[test]
    fn ln_gamma_matches_known_values() {
        assert!(close(ln_gamma(1.0), 0.0, 1e-12));
        assert!(close(ln_gamma(2.0), 0.0, 1e-12));
        assert!(close(ln_gamma(5.0), 24.0f64.ln(), 1e-11));
        assert!(close(ln_gamma(0.5), PI.sqrt().ln(), 1e-12));
        assert!(close(ln_gamma(100.0), 359.134_205_369_575_4, 1e-9));
    }

    #[test]
    fn the_incomplete_beta_reproduces_the_uniform_and_binomial_closed_forms() {
        for k in 0..=10 {
            let x = k as f64 / 10.0;
            assert!(close(regularized_incomplete_beta(x, 1.0, 1.0), x, 1e-13));
        }
        // I_x(2,1) = x^2 and I_x(1,2) = 1 - (1-x)^2.
        assert!(close(regularized_incomplete_beta(0.3, 2.0, 1.0), 0.09, 1e-13));
        assert!(close(regularized_incomplete_beta(0.3, 1.0, 2.0), 0.51, 1e-13));
        // Symmetry: I_x(a,b) = 1 - I_{1-x}(b,a).
        let direct = regularized_incomplete_beta(0.37, 4.5, 9.25);
        let mirrored = 1.0 - regularized_incomplete_beta(0.63, 9.25, 4.5);
        assert!(close(direct, mirrored, 1e-13));
    }

    #[test]
    fn the_quantile_inverts_the_cdf() {
        let posterior = BetaPosterior::new(7.5, 3.5).unwrap();
        for q in [0.01, 0.025, 0.25, 0.5, 0.75, 0.975, 0.99] {
            let x = posterior.quantile(q).unwrap();
            assert!(close(posterior.cdf(x), q, 1e-9), "q={q} x={x}");
        }
    }

    #[test]
    fn the_credible_interval_carries_the_mass_it_claims() {
        let posterior = BetaPosterior::new(12.0, 30.0).unwrap();
        let interval = posterior.interval(0.95).unwrap();
        let mass = posterior.cdf(interval.hi) - posterior.cdf(interval.lo);
        assert!(close(mass, 0.95, 1e-9));
        assert!(interval.contains(posterior.mean()));
    }

    #[test]
    fn the_expected_variance_reduction_equals_the_simulated_one() {
        let posterior = BetaPosterior::new(3.0, 5.0).unwrap();
        let p = posterior.mean();
        let after_success = BetaPosterior::new(4.0, 5.0).unwrap().variance();
        let after_failure = BetaPosterior::new(3.0, 6.0).unwrap().variance();
        let expected = posterior.variance() - (p * after_success + (1.0 - p) * after_failure);
        assert!(close(posterior.expected_variance_reduction(), expected, 1e-15));
        assert!(posterior.expected_variance_reduction() > 0.0);
    }

    #[test]
    fn deflating_the_mass_preserves_the_mean_and_widens_the_interval() {
        let posterior = BetaPosterior::new(60.0, 40.0).unwrap();
        let deflated = posterior.with_mass(10.0).unwrap();
        assert!(close(deflated.mean(), posterior.mean(), 1e-13));
        assert!(deflated.variance() > posterior.variance());
        let wide = deflated.interval(0.95).unwrap();
        let narrow = posterior.interval(0.95).unwrap();
        assert!(wide.width() > narrow.width());
    }

    #[test]
    fn interval_width_decreases_monotonically_in_effective_mass() {
        let base = BetaPosterior::new(0.5 + 30.0, 0.5 + 20.0).unwrap();
        let mut previous = f64::INFINITY;
        for mass in [2.0, 4.0, 8.0, 16.0, 32.0, 64.0, 128.0, 512.0] {
            let width = base.with_mass(mass).unwrap().interval(0.95).unwrap().width();
            assert!(width < previous, "width did not shrink at mass {mass}");
            previous = width;
        }
    }

    #[test]
    fn the_density_matches_its_closed_forms_and_integrates_to_one() {
        let rising = BetaPosterior::new(2.0, 1.0).unwrap();
        assert!(close(rising.pdf(0.25), 0.5, 1e-12));
        assert!(close(rising.pdf(0.75), 1.5, 1e-12));
        assert_eq!(rising.pdf(0.0), 0.0);
        assert_eq!(rising.pdf(1.0), 0.0);

        // Midpoint rule, which never touches the endpoints where a Beta density may be singular.
        let posterior = BetaPosterior::new(6.0, 3.0).unwrap();
        let panels = 200_000;
        let h = 1.0 / panels as f64;
        let mass: f64 = (0..panels)
            .map(|i| posterior.pdf((i as f64 + 0.5) * h) * h)
            .sum();
        assert!(close(mass, 1.0, 1e-6), "density integrated to {mass}");
    }

    #[test]
    fn probability_of_superiority_matches_its_closed_forms() {
        let uniform = BetaPosterior::new(1.0, 1.0).unwrap();
        let rising = BetaPosterior::new(2.0, 1.0).unwrap();
        assert!(close(
            probability_first_exceeds_second(&rising, &uniform),
            2.0 / 3.0,
            1e-9
        ));
        assert!(close(
            probability_first_exceeds_second(&uniform, &rising),
            1.0 / 3.0,
            1e-9
        ));
    }

    #[test]
    fn probability_of_superiority_is_one_half_against_an_identical_posterior() {
        for (a, b) in [(0.5, 0.5), (1.0, 1.0), (12.0, 4.0), (200.0, 300.0)] {
            let posterior = BetaPosterior::new(a, b).unwrap();
            let p = probability_first_exceeds_second(&posterior, &posterior);
            assert!(close(p, 0.5, 1e-6), "Beta({a},{b}) gave {p}");
        }
    }

    #[test]
    fn probability_of_superiority_complements_to_one() {
        let a = BetaPosterior::new(18.5, 6.5).unwrap();
        let b = BetaPosterior::new(9.5, 11.5).unwrap();
        let forward = probability_first_exceeds_second(&a, &b);
        let backward = probability_first_exceeds_second(&b, &a);
        assert!(close(forward + backward, 1.0, 1e-7));
        assert!(forward > backward);
    }

    #[test]
    fn degenerate_beta_parameters_are_rejected_rather_than_producing_nan() {
        assert!(matches!(
            BetaPosterior::new(0.0, 1.0),
            Err(AdaptiveError::InvalidBetaParameters { .. })
        ));
        assert!(matches!(
            BetaPosterior::new(1.0, f64::NAN),
            Err(AdaptiveError::InvalidBetaParameters { .. })
        ));
        assert!(matches!(
            BetaPosterior::new(4.0, 4.0).unwrap().interval(1.0),
            Err(AdaptiveError::InvalidCredibility(_))
        ));
    }
}
