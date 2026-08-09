//! Cost model, capacity planning and release sweeps.
//!
//! Blueprint 35.15. Forecast storage, compute, model tokens, expert review and data transfer for CI
//! and major releases.
//!
//! ## Cost is reported per effective class, not per instance
//!
//! [`CostForecast`] embeds an [`EffectiveSize`], and there is no constructor that omits it. Cost per
//! instance is the number that makes a paraphrase factory look efficient — halve the cost per
//! instance by generating twice as many near-identical items — and it is exactly the metric 35's
//! honest-scale definition exists to reject. Cost per *effective class* moves the right way: adding
//! items that carry no new information makes it worse.
//!
//! ## Units
//!
//! Abstract and named: expert minutes, compute units, model tokens, bytes. No currency. A currency
//! figure would need a price list this crate cannot know and would age badly; a caller with one can
//! multiply.
//!
//! ## Not implemented
//!
//! No queueing model, no contention, no tail latency, no per-site heterogeneity beyond a single
//! throughput figure. This forecasts totals and a completion horizon under a stated throughput; it
//! does not simulate a scheduler. The scheduler is `bioprism-factory`'s.

use crate::effective::EffectiveSize;
use serde::{Deserialize, Serialize};

/// Per-unit costs. Every field is a rate the caller measured or assumed, never a default.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CostModel {
    /// Expert minutes to author one parent world. The dominant term at small scale and the one
    /// that does not amortize: 35's scale constraint says parent worlds are the unit of breadth.
    pub parent_authoring_expert_minutes: f64,
    /// Expert minutes to review one parent world's compiled decision boundaries.
    pub parent_review_expert_minutes: f64,
    /// Compute to generate and validate one descendant.
    pub descendant_generation_compute_units: f64,
    /// Model tokens to execute one instance against a system under test.
    pub instance_execution_tokens: u64,
    /// Compute to run the oracle over one executed instance.
    pub oracle_evaluation_compute_units: f64,
    pub bytes_per_parent: u64,
    pub bytes_per_descendant: u64,
    /// Fraction of executions served from the replay cache, in `[0, 1]`. Applies only to repeated
    /// executions of an unchanged instance against an unchanged system — see
    /// [`crate::cas::ReplayCache`], which refuses to serve a hit under an incomplete key.
    pub replay_cache_hit_rate: f64,
    /// Bytes moved per instance in a federated sweep.
    pub transfer_bytes_per_instance: u64,
}

impl CostModel {
    /// Forecasts a plan's totals, carrying the effective size alongside.
    pub fn forecast(&self, plan: &FactoryPlan, effective: EffectiveSize) -> CostForecast {
        let hit_rate = self.replay_cache_hit_rate.clamp(0.0, 1.0);
        let executed = plan.release_suite_instances + plan.ci_suite_instances;
        let executed_cold = executed as f64 * (1.0 - hit_rate);

        let expert_minutes = plan.parent_worlds as f64
            * (self.parent_authoring_expert_minutes + self.parent_review_expert_minutes);
        let compute_units = plan.enumerable_descendants as f64
            * self.descendant_generation_compute_units
            + executed_cold * self.oracle_evaluation_compute_units;
        let model_tokens = executed_cold * self.instance_execution_tokens as f64;
        let storage_bytes = plan.parent_worlds as f64 * self.bytes_per_parent as f64
            + plan.enumerable_descendants as f64 * self.bytes_per_descendant as f64;
        let transfer_bytes = executed as f64 * self.transfer_bytes_per_instance as f64;

        let effective_classes = effective.effective.max(1) as f64;
        CostForecast {
            expert_minutes,
            compute_units,
            model_tokens,
            storage_bytes,
            transfer_bytes,
            executed_instances: executed,
            compute_per_nominal_instance: if plan.enumerable_descendants == 0 {
                0.0
            } else {
                compute_units / plan.enumerable_descendants as f64
            },
            compute_per_effective_class: compute_units / effective_classes,
            expert_minutes_per_effective_class: expert_minutes / effective_classes,
            effective_size: effective,
        }
    }

    /// Epochs to complete a sweep at a stated throughput, and which resource binds.
    ///
    /// "Epoch" rather than "day" because this crate reads no clock; the caller supplies whatever
    /// period their throughput figures are quoted in.
    pub fn capacity(
        forecast: &CostForecast,
        instances_per_epoch: f64,
        expert_minutes_per_epoch: f64,
    ) -> Capacity {
        let execution_epochs = if instances_per_epoch <= 0.0 {
            f64::INFINITY
        } else {
            forecast.executed_instances as f64 / instances_per_epoch
        };
        let expert_epochs = if expert_minutes_per_epoch <= 0.0 {
            f64::INFINITY
        } else {
            forecast.expert_minutes / expert_minutes_per_epoch
        };
        Capacity {
            execution_epochs,
            expert_epochs,
            epochs_to_complete: execution_epochs.max(expert_epochs),
            binding_resource: if expert_epochs >= execution_epochs {
                "expert review".to_string()
            } else {
                "execution".to_string()
            },
        }
    }
}

/// The design of a factory run. 35.16's required components, as a type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FactoryPlan {
    pub parent_worlds: usize,
    pub parent_decisions: usize,
    pub mutation_families: usize,
    /// Distinct parameterizations of each mutation family. 35.16 never states this and its
    /// descendant count cannot be reached without it — see [`crate::accounting::Reconciliation`].
    pub parameterizations_per_family: usize,
    /// The count the blueprint states. Compared against [`FactoryPlan::implied_descendants`].
    pub enumerable_descendants: usize,
    pub release_suite_instances: usize,
    pub ci_suite_instances: usize,
}

impl FactoryPlan {
    /// Descendants the stated structure actually produces: decisions × families × parameterizations.
    ///
    /// Every mutation family applies to every parent decision, which is the generous reading; a
    /// compatibility matrix (35.09's "mutation compatibility") only reduces it.
    pub fn implied_descendants(&self) -> usize {
        self.parent_decisions
            .saturating_mul(self.mutation_families)
            .saturating_mul(self.parameterizations_per_family)
    }

    /// Decisions per parent world.
    pub fn decisions_per_parent(&self) -> f64 {
        if self.parent_worlds == 0 {
            0.0
        } else {
            self.parent_decisions as f64 / self.parent_worlds as f64
        }
    }

    /// Upper bound on independent equivalence classes.
    ///
    /// A `(parent decision, mutation family)` pair is the finest grain at which an instance can
    /// carry new diagnostic information under the equivalence-class relation: two parameterizations
    /// of one family over one decision differ in surface form, which is precisely what the relation
    /// refuses to count. Parameterizations therefore multiply the instance count and not this
    /// bound, which is the arithmetic behind the whole section.
    pub fn class_ceiling(&self) -> usize {
        self.parent_decisions.saturating_mul(self.mutation_families)
    }
}

/// Forecast totals. The effective size travels with them and cannot be dropped.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CostForecast {
    pub expert_minutes: f64,
    pub compute_units: f64,
    pub model_tokens: f64,
    pub storage_bytes: f64,
    pub transfer_bytes: f64,
    pub executed_instances: usize,
    /// The flattering metric, published only beside the next one.
    pub compute_per_nominal_instance: f64,
    /// The honest one. Generating more near-duplicates makes this worse, as it should.
    pub compute_per_effective_class: f64,
    pub expert_minutes_per_effective_class: f64,
    pub effective_size: EffectiveSize,
}

/// What binds a release sweep.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Capacity {
    pub execution_epochs: f64,
    pub expert_epochs: f64,
    pub epochs_to_complete: f64,
    pub binding_resource: String,
}
