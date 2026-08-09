//! The million-scale accounting example, worked and run.
//!
//! Blueprint 35.16: "provide a transparent numerical design for reaching one million valid
//! instances without overstating independence." [`million_scale_example`] is that design executed
//! rather than described, and `tests/million_scale.rs` asserts every number below.
//!
//! ## The finding
//!
//! The blueprint's own numbers do not reconcile, in two places, and the honest effective size is
//! roughly two and a half orders of magnitude below the headline.
//!
//! **First reconciliation failure.** 35.16 specifies 12,000 parent decisions and 35 validated
//! mutation families, then 1.2 million enumerable descendants. Applying every family to every
//! decision yields 420,000. Reaching 1,200,000 requires 2.857… parameterizations per family, which
//! is not an integer: no whole number of parameterizations reproduces the stated figure. Two gives
//! 840,000 (30% short); three gives 1,260,000 (5% over). The parameterization axis is never
//! mentioned in 35.16's required components at all, so the descendant count is the only place it
//! appears — implicitly, and inconsistently.
//!
//! **Second reconciliation failure.** The purpose states one million *valid* instances; the
//! components state 1.2 million *enumerable* descendants. That implies a validated yield of at
//! least 83.3%, which 35.16 never states, and which 35.09 lists "valid instance yield" as a metric
//! to be *measured* rather than assumed. A yield below 83.3% misses the million entirely.
//!
//! **The effective size.** 1,200,000 enumerable descendants cannot exceed 420,000 equivalence
//! classes, because two parameterizations of one family over one decision differ in surface form
//! and that is exactly what the relation refuses to count. Of those, the release suite executes
//! 100,000 — and 35's own constraint says "results name the subset actually executed", so
//! unexecuted classes are not evidence. Clustering those 100,000 on their 400 parent worlds at an
//! assumed intra-parent correlation of 0.05 leaves about 7,400 independent observations. The floor,
//! under the relation 35 itself calls the unit of scientific breadth, is **400**.
//!
//! So the honest ladder is 1,200,000 → 420,000 → 100,000 → ~7,400 → 400. The headline is roughly
//! 160 times the information the release actually delivers, and 3,000 times the number of
//! independent worlds. That is the finding, and it ships.
//!
//! None of this makes the design bad. 400 audited parent worlds is a serious benchmark. It is only
//! bad if it is reported as a million.

use crate::cost::{Capacity, CostForecast, CostModel, FactoryPlan};
use crate::effective::{hierarchical_effective_size, EffectiveSize, SimilarityRelation};
use crate::error::ScaleError;
use serde::{Deserialize, Serialize};

/// The intra-parent correlation the headline figure assumes.
///
/// An assumption, not a measurement, and the report carries [`RHO_RANGE`] beside it because the
/// answer moves by an order of magnitude across plausible values. Quoting one value without the
/// range would be the same error as quoting the nominal count without the effective one.
pub const REFERENCE_RHO: f64 = 0.05;

/// The range reported alongside the headline: near-independent, moderate, strongly clustered.
pub const RHO_RANGE: [f64; 3] = [0.01, 0.05, 0.20];

/// 35.16's stated design, verbatim.
pub fn blueprint_plan() -> FactoryPlan {
    FactoryPlan {
        parent_worlds: 400,
        parent_decisions: 12_000,
        mutation_families: 35,
        parameterizations_per_family: 1,
        enumerable_descendants: 1_200_000,
        release_suite_instances: 100_000,
        ci_suite_instances: 3_000,
    }
}

/// The rates the worked example assumes. Stated so a reader can disagree with them numerically.
pub fn assumed_cost_model() -> CostModel {
    CostModel {
        parent_authoring_expert_minutes: 480.0,
        parent_review_expert_minutes: 120.0,
        descendant_generation_compute_units: 0.05,
        instance_execution_tokens: 8_000,
        oracle_evaluation_compute_units: 0.2,
        bytes_per_parent: 2_000_000,
        bytes_per_descendant: 20_000,
        replay_cache_hit_rate: 0.4,
        transfer_bytes_per_instance: 20_000,
    }
}

/// Where the blueprint's stated counts disagree with its stated structure.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Reconciliation {
    pub stated_descendants: usize,
    /// decisions × families × 1 parameterization.
    pub implied_descendants_single_parameterization: usize,
    /// The non-integer this crate refuses to round.
    pub required_parameterizations: f64,
    pub descendants_at_two_parameterizations: usize,
    pub descendants_at_three_parameterizations: usize,
    /// The purpose's target, against the components' enumerable count.
    pub stated_valid_instance_target: usize,
    pub implied_minimum_yield: f64,
    pub reconciles: bool,
    pub notes: Vec<String>,
}

impl Reconciliation {
    pub fn of(plan: &FactoryPlan, valid_instance_target: usize) -> Self {
        let single = plan.class_ceiling();
        let required = if single == 0 {
            0.0
        } else {
            plan.enumerable_descendants as f64 / single as f64
        };
        let integral = (required - required.round()).abs() < 1e-9;
        let implied_minimum_yield = if plan.enumerable_descendants == 0 {
            0.0
        } else {
            valid_instance_target as f64 / plan.enumerable_descendants as f64
        };

        Reconciliation {
            stated_descendants: plan.enumerable_descendants,
            implied_descendants_single_parameterization: single,
            required_parameterizations: required,
            descendants_at_two_parameterizations: single.saturating_mul(2),
            descendants_at_three_parameterizations: single.saturating_mul(3),
            stated_valid_instance_target: valid_instance_target,
            implied_minimum_yield,
            reconciles: integral,
            notes: vec![
                format!(
                    "{} decisions × {} families = {} descendants at one parameterization each; \
                     the stated {} needs {:.3} parameterizations per family, which is not an \
                     integer",
                    plan.parent_decisions,
                    plan.mutation_families,
                    single,
                    plan.enumerable_descendants,
                    required
                ),
                format!(
                    "reaching {} valid instances from {} enumerable descendants requires a \
                     validated yield of at least {:.1}%, which 35.16 does not state and 35.09 \
                     lists as a quantity to be measured",
                    valid_instance_target,
                    plan.enumerable_descendants,
                    implied_minimum_yield * 100.0
                ),
            ],
        }
    }
}

/// One point on the clustering curve.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HierarchicalPoint {
    /// Assumed intra-parent correlation.
    pub rho: f64,
    /// Executed instances deflated by the clustered design effect.
    pub effective_sample_size: f64,
    /// Nominal enumerable descendants over this effective size.
    pub inflation_versus_nominal: f64,
}

/// The whole worked example: the ladder, the cost, and the two reconciliation failures.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MillionScaleAccounting {
    pub plan: FactoryPlan,
    pub reconciliation: Reconciliation,
    /// 1.2 M enumerable descendants against their 420,000-class ceiling.
    pub enumerable: EffectiveSize,
    /// The subset actually executed, which is the only subset that is evidence.
    pub executed_release: EffectiveSize,
    /// The headline, clustered on parent worlds at [`REFERENCE_RHO`].
    pub headline: EffectiveSize,
    pub hierarchical: Vec<HierarchicalPoint>,
    /// 35's own floor: parent worlds are the unit of scientific breadth.
    pub parent_world_floor: usize,
    pub forecast: CostForecast,
    pub capacity: Capacity,
    pub findings: Vec<String>,
}

impl MillionScaleAccounting {
    /// The sentence that must appear next to any "one million instances" claim.
    pub fn headline_sentence(&self) -> String {
        format!(
            "{} enumerable descendants → {} equivalence classes → {} executed → ~{:.0} independent \
             observations at rho={} → {} independent parent worlds. The headline is ×{:.0} the \
             information the release delivers.",
            self.plan.enumerable_descendants,
            self.enumerable.effective,
            self.plan.release_suite_instances,
            self.headline.effective,
            REFERENCE_RHO,
            self.parent_world_floor,
            self.headline.inflation_ratio
        )
    }
}

/// Runs 35.16's design and reports what it is actually worth.
pub fn million_scale_example() -> Result<MillionScaleAccounting, ScaleError> {
    let plan = blueprint_plan();
    let model = assumed_cost_model();
    let reconciliation = Reconciliation::of(&plan, 1_000_000);

    let class_ceiling = plan.class_ceiling();
    let enumerable = EffectiveSize::from_counts(
        SimilarityRelation::EquivalenceClass,
        plan.enumerable_descendants,
        class_ceiling,
    );
    let executed_classes = plan.release_suite_instances.min(class_ceiling);
    let executed_release = EffectiveSize::from_counts(
        SimilarityRelation::EquivalenceClass,
        plan.release_suite_instances,
        executed_classes,
    );

    let mut hierarchical = Vec::new();
    for rho in RHO_RANGE {
        let effective =
            hierarchical_effective_size(executed_classes, plan.parent_worlds, rho)?;
        hierarchical.push(HierarchicalPoint {
            rho,
            effective_sample_size: effective,
            inflation_versus_nominal: if effective <= 0.0 {
                0.0
            } else {
                plan.enumerable_descendants as f64 / effective
            },
        });
    }

    let reference =
        hierarchical_effective_size(executed_classes, plan.parent_worlds, REFERENCE_RHO)?;
    let headline = EffectiveSize::from_counts(
        SimilarityRelation::EquivalenceClass,
        plan.enumerable_descendants,
        reference.round() as usize,
    );

    let forecast = model.forecast(&plan, headline.clone());
    let capacity = CostModel::capacity(&forecast, 20_000.0, 2_400.0);

    let mut findings = reconciliation.notes.clone();
    findings.push(format!(
        "effective size ladder: {} enumerable → {} classes → {} executed → {:.0} clustered at \
         rho={} → {} parent worlds",
        plan.enumerable_descendants,
        class_ceiling,
        plan.release_suite_instances,
        reference,
        REFERENCE_RHO,
        plan.parent_worlds
    ));
    findings.push(format!(
        "cost per nominal instance is {:.4} compute units; cost per effective class is {:.2}, a \
         factor of {:.0} — the first number is the one a paraphrase factory optimizes",
        forecast.compute_per_nominal_instance,
        forecast.compute_per_effective_class,
        forecast.compute_per_effective_class / forecast.compute_per_nominal_instance.max(f64::MIN_POSITIVE)
    ));
    findings.push(format!(
        "{} is the binding resource: {:.1} epochs of expert review against {:.1} epochs of \
         execution",
        capacity.binding_resource, capacity.expert_epochs, capacity.execution_epochs
    ));

    Ok(MillionScaleAccounting {
        parent_world_floor: plan.parent_worlds,
        plan,
        reconciliation,
        enumerable,
        executed_release,
        headline,
        hierarchical,
        forecast,
        capacity,
        findings,
    })
}
