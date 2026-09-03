//! The capacity model: a projection that cannot be separated from what qualifies it.
//!
//! Implements blueprint 40.35 (Performance, Capacity and Load Model).
//!
//! # The shape is `crates/scale`'s, in a different unit
//!
//! `bioprism_scale::CostForecast` embeds an `EffectiveSize` and has no constructor that omits it,
//! so a cost-per-instance figure cannot travel without the relation that decides what counts as a
//! distinct instance. The same failure mode applies to a capacity number and is more common: "we
//! sustain 400 compiles per epoch" is meaningless without the per-compile cost it was divided by,
//! and that cost is usually a guess.
//!
//! So there is no bare `f64` on the way out of this module. [`Assumption`] has two constructors,
//! [`Assumption::measured`] and [`Assumption::assumed`], and no third that omits the basis; an
//! [`Operation`] holds its cost *as* an `Assumption`, so an operation with an unbacked cost is not
//! constructible; and every headline [`CapacityProjection`] produces is a [`Qualified`] carrying
//! the measured and assumed inputs it rests on. [`Qualified`]'s `Display` prints the qualification
//! beside the number, because the place a number loses its caveats is a paste into a summary.
//!
//! # The four invariants, and which of them a type can hold
//!
//! | 40.35 invariant | how it is held here |
//! |---|---|
//! | Correctness and policy do not degrade silently under load | [`Concession`] is closed and contains no variant a result depends on; [`DegradationPlan::declare`] refuses a plan that concedes nothing or is visible nowhere |
//! | Queries and actions are bounded | [`Bound::Unbounded`] is representable and unprojectable: [`CapacityModel::project`] refuses it with [`OpsError::UnboundedWorkload`] rather than assigning it an infinite cost |
//! | Large artifacts stream | [`ArtifactHandling::Materialised`] above the model's ceiling is [`OpsError::UnstreamedArtifact`] |
//! | Backpressure is visible | [`Saturation::Saturated`] carries a [`DegradationPlan`] and there is no variant that reports saturation without one |
//!
//! The second is worth dwelling on. An unbounded traversal is not an expensive operation, it is an
//! operation with no capacity at all, and a model that gave it a large number would let a plan be
//! made for it. It is refused instead.
//!
//! # What is deliberately not implemented
//!
//! * **No benchmark, no load generator, no profiler, no measurement of anything.** 40.35 names all
//!   four under Interfaces. Nothing here executes a workload; every number is a caller's, and
//!   [`Basis::Measured`] records the caller's claim about where theirs came from — it does not
//!   verify it, and [`Basis`] is not evidence.
//! * **No queueing theory.** No arrival distribution, no service-time distribution, no Little's
//!   law, no tail estimate. `bioprism-scale` makes the same exclusion for the same reason: a
//!   percentile from an unvalidated distribution is a decoration. [`CapacityProjection`] reports
//!   utilisation and headroom, which are ratios of stated quantities.
//! * **No clock, no timer, no wall-clock latency.** Work is counted in abstract units per epoch.
//!   Converting units to seconds needs a machine, and 40.35's own "hardware/environment manifests"
//!   are what would carry that.
//! * **No scheduler, no worker pool, no lease, no cache.** `bioprism-infra` owns caching and
//!   quota, `bioprism-factory` owns scheduling. Neither is imported or reimplemented; a
//!   [`DegradationPlan`] describes what would be conceded, and concedes nothing itself.
//! * **No cost in currency.** Same reason `bioprism-scale` gives: a price list this crate cannot
//!   know, ageing badly.

use crate::error::{well_formed_name, OpsError};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Where a number came from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "basis", rename_all = "snake_case")]
pub enum Basis {
    /// Somebody measured it, and `how` says with what. This crate does not check the claim; it
    /// makes the claim impossible to omit.
    Measured { how: String },
    /// Nobody measured it. `rationale` is why the number is the one chosen.
    Assumed { rationale: String },
}

impl Basis {
    pub fn is_measured(&self) -> bool {
        matches!(self, Basis::Measured { .. })
    }
}

/// A named quantity that cannot exist without a basis.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Assumption {
    name: String,
    value: f64,
    unit: String,
    basis: Basis,
}

impl Assumption {
    pub fn measured(
        name: impl Into<String>,
        value: f64,
        unit: impl Into<String>,
        how: impl Into<String>,
    ) -> Result<Self, OpsError> {
        Ok(Assumption {
            name: well_formed_name("assumption name", &name.into())?,
            value,
            unit: unit.into(),
            basis: Basis::Measured { how: how.into() },
        })
    }

    pub fn assumed(
        name: impl Into<String>,
        value: f64,
        unit: impl Into<String>,
        rationale: impl Into<String>,
    ) -> Result<Self, OpsError> {
        Ok(Assumption {
            name: well_formed_name("assumption name", &name.into())?,
            value,
            unit: unit.into(),
            basis: Basis::Assumed {
                rationale: rationale.into(),
            },
        })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn value(&self) -> f64 {
        self.value
    }

    pub fn unit(&self) -> &str {
        &self.unit
    }

    pub fn basis(&self) -> &Basis {
        &self.basis
    }
}

/// A number and everything that qualifies it.
///
/// The point is the `Display`: a headline that is printed carries its qualification into whatever
/// it is pasted into.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Qualified {
    value: f64,
    unit: String,
    measured: Vec<String>,
    assumed: Vec<String>,
}

impl Qualified {
    pub fn value(&self) -> f64 {
        self.value
    }

    pub fn unit(&self) -> &str {
        &self.unit
    }

    /// The assumptions behind the number that somebody measured.
    pub fn measured(&self) -> &[String] {
        &self.measured
    }

    /// The assumptions behind the number that nobody measured.
    pub fn assumed(&self) -> &[String] {
        &self.assumed
    }

    /// Whether every input to the number was measured.
    pub fn is_fully_measured(&self) -> bool {
        self.assumed.is_empty()
    }
}

impl fmt::Display for Qualified {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.value, self.unit)?;
        if self.assumed.is_empty() {
            write!(f, " (all {} inputs measured)", self.measured.len())
        } else {
            write!(f, " (assumed: {})", self.assumed.join(", "))
        }
    }
}

/// How much an operation may touch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "bound", rename_all = "snake_case")]
pub enum Bound {
    /// At most this many items per call.
    Bounded { steps: u64 },
    /// No stated limit. Representable so that a workload can be written down honestly, and
    /// unprojectable so that it cannot be planned for.
    Unbounded,
}

impl Bound {
    pub fn steps(self) -> Option<u64> {
        match self {
            Bound::Bounded { steps } => Some(steps),
            Bound::Unbounded => None,
        }
    }
}

/// Whether an artifact passes through memory.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "handling", rename_all = "snake_case")]
pub enum ArtifactHandling {
    /// Read or written incrementally; peak residency is not the artifact's size.
    Streamed,
    /// Held whole. 40.35's `memory copy of raw matrix/image`.
    Materialised { bytes: u64 },
}

impl ArtifactHandling {
    pub fn resident_bytes(self) -> u64 {
        match self {
            ArtifactHandling::Streamed => 0,
            ArtifactHandling::Materialised { bytes } => bytes,
        }
    }
}

/// One thing a workload does per call.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Operation {
    name: String,
    traversal: Bound,
    handling: ArtifactHandling,
    cost_per_step: Assumption,
}

impl Operation {
    /// The cost is an [`Assumption`], so an operation whose cost has no stated basis cannot be
    /// built.
    pub fn new(
        name: impl Into<String>,
        traversal: Bound,
        handling: ArtifactHandling,
        cost_per_step: Assumption,
    ) -> Result<Self, OpsError> {
        Ok(Operation {
            name: well_formed_name("operation name", &name.into())?,
            traversal,
            handling,
            cost_per_step,
        })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn traversal(&self) -> Bound {
        self.traversal
    }

    pub fn handling(&self) -> ArtifactHandling {
        self.handling
    }

    pub fn cost_per_step(&self) -> &Assumption {
        &self.cost_per_step
    }

    fn work_per_call(&self) -> Option<f64> {
        self.traversal
            .steps()
            .map(|steps| steps as f64 * self.cost_per_step.value)
    }
}

/// A class of work, as 40.35's "workload classes".
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Workload {
    name: String,
    calls_per_epoch: Assumption,
    operations: Vec<Operation>,
}

impl Workload {
    pub fn new(name: impl Into<String>, calls_per_epoch: Assumption) -> Result<Self, OpsError> {
        Ok(Workload {
            name: well_formed_name("workload name", &name.into())?,
            calls_per_epoch,
            operations: Vec::new(),
        })
    }

    pub fn with(mut self, operation: Operation) -> Self {
        self.operations.push(operation);
        self
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn operations(&self) -> &[Operation] {
        &self.operations
    }

    /// Every operation with no stated traversal bound, in declaration order.
    pub fn unbounded_operations(&self) -> Vec<&Operation> {
        self.operations
            .iter()
            .filter(|operation| operation.traversal == Bound::Unbounded)
            .collect()
    }
}

/// What a deployment supplies.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CapacityModel {
    work_units_per_epoch: Assumption,
    memory_ceiling_bytes: u64,
}

impl CapacityModel {
    pub fn new(work_units_per_epoch: Assumption, memory_ceiling_bytes: u64) -> Self {
        CapacityModel {
            work_units_per_epoch,
            memory_ceiling_bytes,
        }
    }

    pub fn memory_ceiling_bytes(&self) -> u64 {
        self.memory_ceiling_bytes
    }

    /// Projects a workload against this model.
    ///
    /// Refuses before it computes anything: an unbounded operation has no capacity rather than a
    /// large one, and a materialised artifact above the ceiling is a design defect the model must
    /// not absorb into a bigger number.
    pub fn project(&self, workload: &Workload) -> Result<CapacityProjection, OpsError> {
        if let Some(operation) = workload.unbounded_operations().first() {
            return Err(OpsError::UnboundedWorkload {
                workload: workload.name.clone(),
                operation: operation.name.clone(),
            });
        }
        for operation in &workload.operations {
            let bytes = operation.handling.resident_bytes();
            if bytes > self.memory_ceiling_bytes {
                return Err(OpsError::UnstreamedArtifact {
                    operation: operation.name.clone(),
                    bytes,
                    ceiling: self.memory_ceiling_bytes,
                });
            }
        }

        let mut work_per_call = 0.0;
        for operation in &workload.operations {
            let Some(work) = operation.work_per_call() else {
                return Err(OpsError::UnboundedWorkload {
                    workload: workload.name.clone(),
                    operation: operation.name.clone(),
                });
            };
            work_per_call += work;
        }
        let demand = work_per_call * workload.calls_per_epoch.value;
        let supply = self.work_units_per_epoch.value;

        let mut inputs: Vec<Assumption> = vec![
            self.work_units_per_epoch.clone(),
            workload.calls_per_epoch.clone(),
        ];
        inputs.extend(
            workload
                .operations
                .iter()
                .map(|operation| operation.cost_per_step.clone()),
        );

        Ok(CapacityProjection {
            workload: workload.name.clone(),
            work_per_call,
            demand_per_epoch: demand,
            supply_per_epoch: supply,
            peak_resident_bytes: workload
                .operations
                .iter()
                .map(|operation| operation.handling.resident_bytes())
                .max()
                .unwrap_or(0),
            inputs,
        })
    }
}

/// A capacity projection, inseparable from its assumptions.
///
/// Private fields and no public constructor: [`CapacityModel::project`] is the only route, and it
/// collects every [`Assumption`] the numbers rest on. There is no way to hold a projection whose
/// assumptions have been dropped.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CapacityProjection {
    workload: String,
    work_per_call: f64,
    demand_per_epoch: f64,
    supply_per_epoch: f64,
    peak_resident_bytes: u64,
    inputs: Vec<Assumption>,
}

impl CapacityProjection {
    pub fn workload(&self) -> &str {
        &self.workload
    }

    pub fn assumptions(&self) -> &[Assumption] {
        &self.inputs
    }

    /// Fraction of supply the stated demand consumes.
    pub fn utilisation(&self) -> Qualified {
        self.qualify(
            if self.supply_per_epoch == 0.0 {
                f64::INFINITY
            } else {
                self.demand_per_epoch / self.supply_per_epoch
            },
            "utilisation",
        )
    }

    /// Calls per epoch the model sustains at the stated per-call cost.
    pub fn sustainable_calls_per_epoch(&self) -> Qualified {
        self.qualify(
            if self.work_per_call == 0.0 {
                f64::INFINITY
            } else {
                self.supply_per_epoch / self.work_per_call
            },
            "calls/epoch",
        )
    }

    pub fn work_per_call(&self) -> Qualified {
        self.qualify(self.work_per_call, "work-units/call")
    }

    pub fn peak_resident_bytes(&self) -> u64 {
        self.peak_resident_bytes
    }

    /// Whether every number this projection reports rests only on measured inputs.
    pub fn is_fully_measured(&self) -> bool {
        self.inputs.iter().all(|input| input.basis.is_measured())
    }

    /// Where the projection stands against a stated demand, and what would be conceded if it does
    /// not fit.
    ///
    /// The plan is an argument rather than an option, so there is no path that reports saturation
    /// without saying what happens next.
    pub fn under(&self, demand: &Demand, plan: &DegradationPlan) -> Saturation {
        let sustainable = if self.work_per_call == 0.0 {
            f64::INFINITY
        } else {
            self.supply_per_epoch / self.work_per_call
        };
        if demand.calls_per_epoch <= sustainable {
            Saturation::Within {
                headroom_calls_per_epoch: sustainable - demand.calls_per_epoch,
            }
        } else {
            Saturation::Saturated {
                excess_calls_per_epoch: demand.calls_per_epoch - sustainable,
                plan: plan.clone(),
            }
        }
    }

    fn qualify(&self, value: f64, unit: &str) -> Qualified {
        let mut measured = Vec::new();
        let mut assumed = Vec::new();
        for input in &self.inputs {
            if input.basis.is_measured() {
                measured.push(input.name.clone());
            } else {
                assumed.push(input.name.clone());
            }
        }
        Qualified {
            value,
            unit: unit.to_string(),
            measured,
            assumed,
        }
    }
}

/// Offered load.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Demand {
    pub calls_per_epoch: f64,
}

/// What a system gives up under load.
///
/// The set is closed and contains nothing a result depends on. That closure is how 40.35's first
/// invariant — *correctness and policy do not degrade silently under load* — is enforced rather
/// than restated: there is no `Concession::SkipProtectedClosure`, no `Concession::RelaxPolicy`, no
/// `Concession::PartialEvidence`, and adding one would be visible in review as a change to this
/// enumeration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Concession {
    /// Requests take longer.
    Latency,
    /// Fewer requests are accepted, and the ones refused are refused explicitly.
    Throughput,
    /// Non-essential enrichment is skipped. Never evidence a result rests on.
    OptionalEnrichment,
    /// Projections are served staler. `bioprism_infra::index` already requires a projection to
    /// answer with a freshness, so staleness is reportable rather than invisible.
    Freshness,
}

impl Concession {
    pub const ALL: [Concession; 4] = [
        Concession::Latency,
        Concession::Throughput,
        Concession::OptionalEnrichment,
        Concession::Freshness,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Concession::Latency => "latency",
            Concession::Throughput => "throughput",
            Concession::OptionalEnrichment => "optional_enrichment",
            Concession::Freshness => "freshness",
        }
    }
}

impl fmt::Display for Concession {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// What is conceded under saturation, and where a reader would see it happen.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DegradationPlan {
    name: String,
    concessions: Vec<Concession>,
    visible_as: String,
}

impl DegradationPlan {
    /// Refuses a plan that concedes nothing, and a plan nobody could see happening.
    ///
    /// Both are the silent case. A plan with no concessions claims the system absorbs unbounded
    /// load for free; a plan with no visible signal degrades where nobody is looking, which is what
    /// 40.35's *backpressure is visible* forbids.
    pub fn declare(
        name: impl Into<String>,
        concessions: impl IntoIterator<Item = Concession>,
        visible_as: impl Into<String>,
    ) -> Result<Self, OpsError> {
        let name = name.into();
        let concessions: Vec<Concession> = concessions.into_iter().collect();
        let visible_as = visible_as.into();
        if concessions.is_empty() || visible_as.trim().is_empty() {
            return Err(OpsError::SilentDegradation { plan: name });
        }
        Ok(DegradationPlan {
            name,
            concessions,
            visible_as,
        })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn concessions(&self) -> &[Concession] {
        &self.concessions
    }

    /// The signal a reader watches to know degradation is happening.
    pub fn visible_as(&self) -> &str {
        &self.visible_as
    }
}

/// Where a projection stands against an offered load.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "saturation", rename_all = "snake_case")]
pub enum Saturation {
    Within {
        headroom_calls_per_epoch: f64,
    },
    /// Carries the plan. There is no saturated state without one.
    Saturated {
        excess_calls_per_epoch: f64,
        plan: DegradationPlan,
    },
}

impl Saturation {
    pub fn is_saturated(&self) -> bool {
        matches!(self, Saturation::Saturated { .. })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn measured(name: &str, value: f64, unit: &str) -> Assumption {
        Assumption::measured(name, value, unit, "microbenchmark").expect("well-formed")
    }

    fn assumed(name: &str, value: f64, unit: &str) -> Assumption {
        Assumption::assumed(name, value, unit, "no measurement exists").expect("well-formed")
    }

    fn compile_workload(cost: Assumption) -> Workload {
        Workload::new("compile", measured("calls_per_epoch", 100.0, "calls/epoch"))
            .unwrap()
            .with(
                Operation::new(
                    "closure_walk",
                    Bound::Bounded { steps: 500 },
                    ArtifactHandling::Streamed,
                    cost,
                )
                .unwrap(),
            )
    }

    fn model() -> CapacityModel {
        CapacityModel::new(
            measured("work_units_per_epoch", 1_000_000.0, "work-units/epoch"),
            64 * 1024 * 1024,
        )
    }

    #[test]
    fn a_headline_number_cannot_travel_without_what_qualifies_it() {
        let projection = model()
            .project(&compile_workload(assumed(
                "cost_per_step",
                2.0,
                "work-units/step",
            )))
            .expect("projects");
        let headline = projection.sustainable_calls_per_epoch();
        assert!(!headline.is_fully_measured());
        assert_eq!(headline.assumed(), ["cost_per_step".to_string()]);
        assert!(
            headline.to_string().contains("assumed: cost_per_step"),
            "printing the headline must print the qualification: {headline}"
        );
    }

    #[test]
    fn a_projection_over_measured_inputs_says_so_and_one_over_a_guess_does_not() {
        let all_measured = model()
            .project(&compile_workload(measured(
                "cost_per_step",
                2.0,
                "work-units/step",
            )))
            .unwrap();
        assert!(all_measured.is_fully_measured());
        assert!(all_measured.utilisation().to_string().contains("measured"));

        let one_guess = model()
            .project(&compile_workload(assumed(
                "cost_per_step",
                2.0,
                "work-units/step",
            )))
            .unwrap();
        assert!(!one_guess.is_fully_measured());
    }

    #[test]
    fn an_operation_with_no_traversal_bound_has_no_capacity_rather_than_a_large_one() {
        let workload = Workload::new(
            "graph_query",
            measured("calls_per_epoch", 10.0, "calls/epoch"),
        )
        .unwrap()
        .with(
            Operation::new(
                "transitive_closure",
                Bound::Unbounded,
                ArtifactHandling::Streamed,
                measured("cost_per_step", 1.0, "work-units/step"),
            )
            .unwrap(),
        );
        let error = model().project(&workload).unwrap_err();
        match error {
            OpsError::UnboundedWorkload { operation, .. } => {
                assert_eq!(operation, "transitive_closure")
            }
            other => panic!("expected an unbounded workload, got {other}"),
        }
    }

    #[test]
    fn an_artifact_held_whole_above_the_ceiling_is_refused_rather_than_absorbed() {
        let workload = Workload::new("ingest", measured("calls_per_epoch", 1.0, "calls/epoch"))
            .unwrap()
            .with(
                Operation::new(
                    "load_volume",
                    Bound::Bounded { steps: 1 },
                    ArtifactHandling::Materialised {
                        bytes: 512 * 1024 * 1024,
                    },
                    measured("cost_per_step", 1.0, "work-units/step"),
                )
                .unwrap(),
            );
        let error = model().project(&workload).unwrap_err();
        assert!(matches!(error, OpsError::UnstreamedArtifact { .. }));
    }

    #[test]
    fn a_streamed_artifact_of_any_size_projects_and_contributes_no_residency() {
        let workload = Workload::new("ingest", measured("calls_per_epoch", 1.0, "calls/epoch"))
            .unwrap()
            .with(
                Operation::new(
                    "stream_volume",
                    Bound::Bounded { steps: 1 },
                    ArtifactHandling::Streamed,
                    measured("cost_per_step", 1.0, "work-units/step"),
                )
                .unwrap(),
            );
        let projection = model().project(&workload).expect("streaming projects");
        assert_eq!(projection.peak_resident_bytes(), 0);
    }

    #[test]
    fn a_degradation_plan_that_concedes_nothing_is_the_silent_case_and_is_refused() {
        let error = DegradationPlan::declare("none", [], "queue_age").unwrap_err();
        assert!(matches!(error, OpsError::SilentDegradation { .. }));
    }

    #[test]
    fn a_degradation_nobody_could_see_happening_is_also_the_silent_case() {
        let error =
            DegradationPlan::declare("invisible", [Concession::Latency], "   ").unwrap_err();
        assert!(matches!(error, OpsError::SilentDegradation { .. }));
    }

    #[test]
    fn the_concession_set_is_closed_and_contains_nothing_a_result_depends_on() {
        assert_eq!(Concession::ALL.len(), 4);
        let names: Vec<&str> = Concession::ALL.iter().map(|c| c.as_str()).collect();
        assert_eq!(
            names,
            ["latency", "throughput", "optional_enrichment", "freshness"],
            "adding a concession that gives up evidence, closure or policy would change this list \
             and 40.35's first invariant with it"
        );
    }

    #[test]
    fn saturation_always_carries_the_plan_that_says_what_is_given_up() {
        let projection = model()
            .project(&compile_workload(measured(
                "cost_per_step",
                2.0,
                "work-units/step",
            )))
            .unwrap();
        let plan = DegradationPlan::declare(
            "shed",
            [Concession::Throughput, Concession::Latency],
            "queue_age",
        )
        .unwrap();

        let sustainable = projection.sustainable_calls_per_epoch().value();
        let within = projection.under(
            &Demand {
                calls_per_epoch: sustainable / 2.0,
            },
            &plan,
        );
        assert!(!within.is_saturated());

        let over = projection.under(
            &Demand {
                calls_per_epoch: sustainable * 2.0,
            },
            &plan,
        );
        match over {
            Saturation::Saturated { plan, .. } => {
                assert_eq!(plan.visible_as(), "queue_age");
                assert!(!plan.concessions().is_empty());
            }
            Saturation::Within { .. } => panic!("expected saturation"),
        }
    }

    #[test]
    fn utilisation_and_headroom_are_read_off_the_same_stated_quantities() {
        let projection = model()
            .project(&compile_workload(measured(
                "cost_per_step",
                2.0,
                "work-units/step",
            )))
            .unwrap();
        assert!((projection.work_per_call().value() - 1000.0).abs() < 1e-9);
        assert!((projection.sustainable_calls_per_epoch().value() - 1000.0).abs() < 1e-9);
        assert!((projection.utilisation().value() - 0.1).abs() < 1e-9);
    }

    #[test]
    fn every_assumption_the_projection_rests_on_travels_with_it() {
        let projection = model()
            .project(&compile_workload(assumed(
                "cost_per_step",
                2.0,
                "work-units/step",
            )))
            .unwrap();
        let names: Vec<&str> = projection
            .assumptions()
            .iter()
            .map(Assumption::name)
            .collect();
        assert!(names.contains(&"work_units_per_epoch"));
        assert!(names.contains(&"calls_per_epoch"));
        assert!(names.contains(&"cost_per_step"));
    }
}
