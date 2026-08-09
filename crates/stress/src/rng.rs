//! Deterministic pseudo-randomness, and noise that provably cannot move the mean.
//!
//! Blueprint 32.01 requires every transformation to carry a seed and reproduce from it, and 32.23
//! requires descendants to be deduplicated by content — both are meaningless if the generator
//! reads a system clock or a system RNG. The bit generator is [`SplitMix64`], reused from
//! `bioprism-worldgen` rather than copied so the two crates cannot drift apart.
//!
//! Two deliberate choices about the *shape* of the noise:
//!
//! Box–Muller is not used. It needs `ln` and `cos`, which Rust delegates to the platform libm and
//! which are therefore not guaranteed bit-identical across machines. Every value produced here
//! comes from addition, subtraction, multiplication and division of exactly representable
//! quantities, so a cohort perturbed on one machine is byte-identical to the same cohort perturbed
//! on another. Approximate normality is a fair price for reproducible content addresses.
//!
//! [`centred_orthogonal_noise`] is the load-bearing one. 32.03 names *"noise generator changes
//! mean biology"* as a failure risk of assay stress: if widening the error bars also nudges the
//! group mean, any downstream change is unattributable. Projecting the noise onto the orthogonal
//! complement of the signal makes the sample mean and the signal-noise covariance both exactly
//! zero, so variance adds and location does not move — the postcondition becomes arithmetic
//! rather than a hope about large samples.

use bioprism_worldgen::rng::SplitMix64;

/// A float layer over the workspace bit generator.
pub struct StressRng {
    inner: SplitMix64,
}

impl StressRng {
    pub fn new(seed: u64) -> Self {
        StressRng {
            inner: SplitMix64::new(seed),
        }
    }

    /// Uniform on `[0, 1)`, by exact division of a 53-bit mantissa.
    pub fn unit(&mut self) -> f64 {
        const SCALE: f64 = 1.0 / (1u64 << 53) as f64;
        ((self.inner.next_u64() >> 11) as f64) * SCALE
    }

    /// Uniform on `[-1, 1)`.
    ///
    /// Bounded on purpose: [`crate::perturb`] multiplies this by a stated reproducibility
    /// coefficient, and a bounded draw is what lets the postcondition "no volume moved further
    /// than the assay's own reproducibility" be checked exactly instead of in probability.
    pub fn symmetric(&mut self) -> f64 {
        self.unit() * 2.0 - 1.0
    }

    /// Zero-mean, unit-variance draw by the Irwin–Hall construction.
    ///
    /// Twelve uniforms minus six. Kurtosis is wrong in the far tails; nothing in this crate reads
    /// the tails, and the alternative costs cross-platform reproducibility.
    pub fn unit_variance(&mut self) -> f64 {
        let mut total = 0.0;
        for _ in 0..12 {
            total += self.unit();
        }
        total - 6.0
    }

    pub fn draws(&mut self, count: usize) -> Vec<f64> {
        (0..count).map(|_| self.unit_variance()).collect()
    }
}

fn mean(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.iter().sum::<f64>() / values.len() as f64
}

/// Noise with sample mean zero and sample covariance with `signal` zero.
///
/// The returned vector added to `signal` leaves the sample mean of `signal` unchanged and raises
/// its sample variance by exactly the variance of the noise. Both properties are what
/// [`crate::invariant::CohortInvariant::ClassMeansUnchanged`] and
/// [`crate::invariant::CohortInvariant::DispersionIncreased`] assert, so the assay stress
/// satisfies its own postcondition by construction rather than by luck.
///
/// When `signal` is constant the projection step is skipped: there is no signal direction to be
/// orthogonal to, and centring alone already gives both properties.
pub fn centred_orthogonal_noise(signal: &[f64], seed: u64) -> Vec<f64> {
    let mut rng = StressRng::new(seed);
    let mut noise = rng.draws(signal.len());
    if noise.is_empty() {
        return noise;
    }

    let noise_mean = mean(&noise);
    for value in noise.iter_mut() {
        *value -= noise_mean;
    }

    let signal_mean = mean(signal);
    let centred: Vec<f64> = signal.iter().map(|value| value - signal_mean).collect();
    let denominator: f64 = centred.iter().map(|value| value * value).sum();
    if denominator > 0.0 {
        let numerator: f64 = centred
            .iter()
            .zip(noise.iter())
            .map(|(signal, noise)| signal * noise)
            .sum();
        let coefficient = numerator / denominator;
        for (value, centred) in noise.iter_mut().zip(centred.iter()) {
            *value -= coefficient * centred;
        }
        let residual_mean = mean(&noise);
        for value in noise.iter_mut() {
            *value -= residual_mean;
        }
    }
    noise
}
