//! Structural estimates, hard budgets, and the cost model.
//!
//! Blueprint 43.18 gives the objective the optimizer minimises,
//! `Ĉ(σ,b) = Σ_j T̂_{b,j} + λ_m · max_j M̂_{b,j} + λ_r · R̂_{b,j}`,
//! and separates it from the constraints: "memory limits are hard constraints". That separation is
//! reproduced here. A [`Budget`] is checked before a plan is admitted at all and produces a
//! [`crate::Declined`]; a [`CostModel`] only ranks plans that already passed.
//!
//! ## What the numbers are and are not
//!
//! `predicted_ops` counts semiring applications, not seconds. Nothing in this crate measures wall
//! clock: a hardware-conditioned time model would need calibration data that does not exist yet,
//! and 43.48 requires phase boundaries to state their hardware context. An operation count is
//! hardware-free and reproducible, and it is the right unit for comparing two schedules over the
//! same kernel. It is the wrong unit for predicting latency, and no part of this crate claims
//! otherwise.
//!
//! `λ_m` and `λ_r` below are declared defaults, not calibrated ones. 43.18 asks for a "cost model
//! and calibration service"; the calibration half is not implemented, so the weights are stated
//! openly and a caller who has measurements should replace them.

use crate::order::{Bound, WidthMetric};
use bioprism_section::Backend;
use serde::{Deserialize, Serialize};

/// A backend's structural prediction for one region.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Estimate {
    pub backend: Backend,
    pub method: String,
    /// Which width this estimate is about. 43.18: width claims state their metric.
    pub width_metric: WidthMetric,
    pub induced_width: Option<usize>,
    /// Whether the width is the optimum or an upper bound on it.
    pub bound: Bound,
    /// Bound variables in elimination order, empty for backends that do not eliminate.
    pub order: Vec<String>,
    pub predicted_multiply_ops: f64,
    pub predicted_aggregate_ops: f64,
    /// Largest single intermediate the plan holds.
    pub predicted_peak_entries: f64,
    pub predicted_total_entries: f64,
    /// Fraction of the estimate resting on defaulted rather than observed cardinalities.
    pub uncertainty: f64,
    pub assumptions: Vec<String>,
}

impl Estimate {
    pub fn predicted_ops(&self) -> f64 {
        self.predicted_multiply_ops + self.predicted_aggregate_ops
    }

    /// Validates the numeric and representation invariants consumed by the portfolio.
    ///
    /// Estimates are also produced by third-party [`crate::QueryBackend`] implementations, so the
    /// portfolio cannot assume that a public `Estimate` is well formed. In particular, a NaN cost
    /// would make comparisons silently false and could turn an invalid candidate into a selected
    /// plan.
    pub fn validate(&self) -> Result<(), String> {
        for (field, value) in [
            ("predicted_multiply_ops", self.predicted_multiply_ops),
            ("predicted_aggregate_ops", self.predicted_aggregate_ops),
            ("predicted_peak_entries", self.predicted_peak_entries),
            ("predicted_total_entries", self.predicted_total_entries),
        ] {
            if !value.is_finite() || value < 0.0 {
                return Err(format!(
                    "{field} must be finite and non-negative, got {value}"
                ));
            }
        }
        if !self.uncertainty.is_finite() || !(0.0..=1.0).contains(&self.uncertainty) {
            return Err(format!(
                "uncertainty must be finite and within [0, 1], got {}",
                self.uncertainty
            ));
        }
        let predicted_ops = self.predicted_ops();
        if !predicted_ops.is_finite() || predicted_ops < self.predicted_multiply_ops {
            return Err("predicted operation count overflows a finite value".to_string());
        }
        if !self.risk().is_finite() {
            return Err("risk-adjusted operation count overflows a finite value".to_string());
        }
        if self.predicted_total_entries < self.predicted_peak_entries {
            return Err(format!(
                "predicted_total_entries {} is below predicted_peak_entries {}",
                self.predicted_total_entries, self.predicted_peak_entries
            ));
        }
        match (self.width_metric, self.induced_width) {
            (WidthMetric::InducedWidth, None) => {
                return Err("induced-width estimates must report induced_width".to_string())
            }
            (WidthMetric::NotApplicable, Some(_)) => {
                return Err("non-eliminating estimates must not report induced_width".to_string())
            }
            _ => {}
        }
        if self.method.trim().is_empty() {
            return Err("method must not be blank".to_string());
        }
        Ok(())
    }

    /// The risk term of 43.18's objective.
    ///
    /// Defined as the share of predicted work that rests on an assumed cardinality. A region whose
    /// domains are all observed carries no risk under this definition; one whose domains are all
    /// guessed is charged its full predicted cost again. It is a proxy, not a distribution — 43.18
    /// asks for `ρ` as "a risk functional over uncertain cost estimates" and this crate has no
    /// cost distribution to take a quantile of.
    pub fn risk(&self) -> f64 {
        self.predicted_ops() * self.uncertainty
    }
}

/// Hard constraints. A plan that violates one is not costed, it is refused.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Budget {
    pub max_induced_width: usize,
    pub max_peak_entries: f64,
    pub max_ops: f64,
}

impl Default for Budget {
    fn default() -> Self {
        Budget {
            max_induced_width: 24,
            max_peak_entries: (1u64 << 24) as f64,
            max_ops: 1.0e10,
        }
    }
}

impl Budget {
    /// Validates hard limits. Positive infinity is the explicit unbounded value used by phase
    /// sweeps; NaN and negative values are never meaningful limits.
    pub fn validate(&self) -> Result<(), String> {
        for (field, value) in [
            ("max_peak_entries", self.max_peak_entries),
            ("max_ops", self.max_ops),
        ] {
            if value.is_nan() || value < 0.0 {
                return Err(format!(
                    "{field} must be non-negative or positive infinity, got {value}"
                ));
            }
        }
        Ok(())
    }

    pub fn with_max_peak_entries(mut self, entries: f64) -> Self {
        self.max_peak_entries = entries;
        self
    }

    pub fn with_max_induced_width(mut self, width: usize) -> Self {
        self.max_induced_width = width;
        self
    }

    pub fn with_max_ops(mut self, ops: f64) -> Self {
        self.max_ops = ops;
        self
    }
}

/// The ranking objective, applied only to plans that already satisfy their [`Budget`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CostModel {
    /// `λ_m`. How many semiring operations one held table entry is worth.
    pub memory_weight: f64,
    /// `λ_r`. How heavily to charge estimate uncertainty.
    pub risk_weight: f64,
    /// How much cheaper an optimizing backend must be before the portfolio leaves the conservative
    /// plan. 43.18 asks for hysteresis so that plan choice does not thrash on estimate noise; this
    /// is that hysteresis, expressed as a margin rather than as a history.
    pub minimum_speedup: f64,
}

impl Default for CostModel {
    fn default() -> Self {
        CostModel {
            memory_weight: 4.0,
            risk_weight: 1.0,
            minimum_speedup: 1.25,
        }
    }
}

impl CostModel {
    /// Validates the ranking weights before they participate in candidate comparisons.
    pub fn validate(&self) -> Result<(), String> {
        for (field, value) in [
            ("memory_weight", self.memory_weight),
            ("risk_weight", self.risk_weight),
        ] {
            if !value.is_finite() || value < 0.0 {
                return Err(format!(
                    "{field} must be finite and non-negative, got {value}"
                ));
            }
        }
        if !self.minimum_speedup.is_finite() || self.minimum_speedup <= 0.0 {
            return Err(format!(
                "minimum_speedup must be finite and positive, got {}",
                self.minimum_speedup
            ));
        }
        Ok(())
    }

    pub fn cost(&self, estimate: &Estimate) -> f64 {
        if self.validate().is_err() || estimate.validate().is_err() {
            return f64::INFINITY;
        }
        let cost = estimate.predicted_ops()
            + self.memory_weight * estimate.predicted_peak_entries
            + self.risk_weight * estimate.risk();
        if cost.is_finite() && cost >= 0.0 {
            cost
        } else {
            f64::INFINITY
        }
    }

    pub fn with_minimum_speedup(mut self, speedup: f64) -> Self {
        self.minimum_speedup = speedup;
        self
    }

    pub fn with_memory_weight(mut self, weight: f64) -> Self {
        self.memory_weight = weight;
        self
    }
}
