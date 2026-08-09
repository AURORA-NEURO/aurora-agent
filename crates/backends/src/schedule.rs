//! The costed plan both backends run.
//!
//! Blueprint 43.18 asks the optimizer to estimate cost and then "monitor actual intermediate
//! sizes" and "reoptimize at safe checkpoints when estimates fail". That comparison is only
//! meaningful if the estimate describes the computation that actually happens. A [`Schedule`] is
//! therefore not a summary statistic derived alongside the executor — it *is* the executor's
//! program. Cost estimation walks it without touching a table, execution walks it filling tables
//! in, and any divergence between predicted and observed work is a bug in one of the two rather
//! than modelling error.
//!
//! Both backends produce a schedule of the same shape. Variable elimination emits one step per
//! bound variable; direct materialisation emits a single step that aggregates every bound variable
//! at once. 43.38 requires equal engineering between a method and its baseline, and this is where
//! that is enforced structurally: the baseline is not a slower implementation, it is the same
//! implementation with a different schedule.

use crate::order::EliminationOrder;
use crate::region::QueryRegion;
use std::collections::BTreeSet;

/// Where a step reads one of its operands from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepInput {
    /// A factor of the region, by position.
    Factor(usize),
    /// The result of an earlier step, by position.
    Message(usize),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    /// Variables aggregated away by this step. Empty for the final combine.
    pub eliminated: Vec<String>,
    pub inputs: Vec<StepInput>,
    /// The assignment space the step iterates.
    pub scope: Vec<String>,
    /// The scope of the factor the step produces.
    pub result_scope: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Schedule {
    pub steps: Vec<Step>,
    /// Cells iterated, per step.
    pub cells: Vec<f64>,
    /// Entries written, per step.
    pub entries: Vec<f64>,
}

impl Schedule {
    /// Total ⊗ applications. Counted as one per operand per cell, which is what the kernel
    /// executes: the accumulator is seeded with the algebra's unit and multiplied by every input.
    pub fn multiply_ops(&self) -> f64 {
        self.steps
            .iter()
            .zip(&self.cells)
            .map(|(step, cells)| cells * step.inputs.len() as f64)
            .sum()
    }

    /// Total ⊕ applications. A step that eliminates nothing writes rather than accumulates.
    pub fn aggregate_ops(&self) -> f64 {
        self.steps
            .iter()
            .zip(&self.cells)
            .filter(|(step, _)| !step.eliminated.is_empty())
            .map(|(_, cells)| *cells)
            .sum()
    }

    pub fn total_ops(&self) -> f64 {
        self.multiply_ops() + self.aggregate_ops()
    }

    /// The largest intermediate the plan holds at once. 43.18 treats this as a hard constraint,
    /// not a cost term.
    pub fn peak_entries(&self) -> f64 {
        self.entries.iter().copied().fold(0.0, f64::max)
    }

    pub fn total_entries(&self) -> f64 {
        self.entries.iter().sum()
    }

    /// The largest assignment space any single step iterates.
    pub fn peak_cells(&self) -> f64 {
        self.cells.iter().copied().fold(0.0, f64::max)
    }
}

fn measure(region: &QueryRegion, steps: Vec<Step>) -> Schedule {
    let size = |names: &[String]| -> f64 {
        names
            .iter()
            .map(|name| region.cardinality_of(name).unwrap_or(1) as f64)
            .product()
    };
    let cells = steps.iter().map(|step| size(&step.scope)).collect();
    let entries = steps.iter().map(|step| size(&step.result_scope)).collect();
    Schedule {
        steps,
        cells,
        entries,
    }
}

/// Bucket elimination: one step per eliminated variable, then a final combine over the free scope.
///
/// This is the InsideOut schedule of 43.19. A factor joins the bucket of the first variable in the
/// order that its scope mentions; eliminating that variable replaces the whole bucket with a
/// single message over the variable's remaining neighbourhood, which is exactly the clique
/// [`crate::order::PrimalGraph::induced_width_of`] measured. Predicted peak memory and observed
/// peak memory agree because they are the same quantity computed twice.
pub fn bucket_schedule(region: &QueryRegion, order: &EliminationOrder) -> Schedule {
    let mut pool: Vec<(StepInput, Vec<String>)> = region
        .factors()
        .iter()
        .enumerate()
        .map(|(position, factor)| (StepInput::Factor(position), factor.scope().to_vec()))
        .collect();

    let mut steps: Vec<Step> = Vec::with_capacity(order.order.len() + 1);

    for variable in &order.order {
        let mut inputs = Vec::new();
        let mut scope: BTreeSet<String> = BTreeSet::new();
        scope.insert(variable.clone());

        let mut remainder: Vec<(StepInput, Vec<String>)> = Vec::with_capacity(pool.len());
        for (input, factor_scope) in pool.into_iter() {
            if factor_scope.iter().any(|name| name == variable) {
                inputs.push(input);
                scope.extend(factor_scope.iter().cloned());
            } else {
                remainder.push((input, factor_scope));
            }
        }
        pool = remainder;

        let scope: Vec<String> = scope.into_iter().collect();
        let result_scope: Vec<String> = scope
            .iter()
            .filter(|name| *name != variable)
            .cloned()
            .collect();

        pool.push((StepInput::Message(steps.len()), result_scope.clone()));
        steps.push(Step {
            eliminated: vec![variable.clone()],
            inputs,
            scope,
            result_scope,
        });
    }

    let mut scope: BTreeSet<String> = region.free_variables().clone();
    let mut inputs = Vec::new();
    for (input, factor_scope) in pool {
        inputs.push(input);
        scope.extend(factor_scope);
    }
    let scope: Vec<String> = scope.into_iter().collect();
    steps.push(Step {
        eliminated: Vec::new(),
        inputs,
        scope: scope.clone(),
        result_scope: scope,
    });

    measure(region, steps)
}

/// The conservative plan: one pass over the full joint assignment space.
///
/// 43.36's fallback of last resort and 43.37's "exact direct materialisation". It is written to be
/// a fair baseline rather than a foil. In particular it *streams* the joint: the combined
/// assignment space is iterated, never allocated, so its memory is the answer table and its cost
/// is time alone. A version that materialised the joint would lose on memory too, and beating it
/// would prove nothing.
pub fn direct_schedule(region: &QueryRegion) -> Schedule {
    let scope: Vec<String> = region.variables().map(str::to_string).collect();
    let result_scope: Vec<String> = region.free_variables().iter().cloned().collect();
    let eliminated: Vec<String> = region.bound_variables().into_iter().collect();
    let inputs = (0..region.factors().len()).map(StepInput::Factor).collect();

    measure(
        region,
        vec![Step {
            eliminated,
            inputs,
            scope,
            result_scope,
        }],
    )
}
