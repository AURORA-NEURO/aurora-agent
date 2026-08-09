//! Walking a costed schedule.
//!
//! Shared by both physical backends. The executor's only freedom is the schedule it is handed, so
//! the predicted work in an [`crate::Estimate`] and the observed work in an
//! [`ExecutionReceipt`] describe the same traversal. 43.18 asks the optimizer to "monitor actual
//! intermediate sizes" and reoptimize when estimates fail; here a mismatch is not a stale statistic
//! but a defect, and the crate's tests assert equality rather than a tolerance.

use crate::backend::{ComputedRegion, ExecutionReceipt, IntermediateReceipt};
use crate::dense::{combine, DenseFactor, OpCount};
use crate::error::{backend_name, Declined};
use crate::estimate::Budget;
use crate::region::QueryRegion;
use crate::schedule::{Schedule, StepInput};
use bioprism_section::Backend;

pub(crate) fn run(
    region: &QueryRegion,
    schedule: &Schedule,
    backend: Backend,
    order: Vec<String>,
    induced_width: Option<usize>,
    budget: &Budget,
) -> Result<ComputedRegion, Declined> {
    let name = backend_name(backend);

    let mut originals: Vec<DenseFactor> = Vec::with_capacity(region.factors().len());
    for factor in region.factors() {
        let Some(table) = factor.table() else {
            return Err(Declined::MissingFactorTable {
                backend: name,
                factor: factor.id().to_string(),
            });
        };
        originals.push(DenseFactor {
            scope: factor.scope().to_vec(),
            values: table.to_vec(),
        });
    }

    let mut messages: Vec<DenseFactor> = Vec::with_capacity(schedule.steps.len());
    let mut ops = OpCount::default();
    let mut intermediates = Vec::with_capacity(schedule.steps.len());
    let mut peak_entries = 0u64;
    let mut total_entries = 0u64;

    for step in &schedule.steps {
        let inputs: Vec<&DenseFactor> = step
            .inputs
            .iter()
            .map(|input| match input {
                StepInput::Factor(position) => &originals[*position],
                StepInput::Message(position) => &messages[*position],
            })
            .collect();

        let (produced, step_ops) = combine(
            &inputs,
            &step.scope,
            &step.result_scope,
            region.cardinality(),
            region.semiring(),
        )
        .map_err(|oversized| Declined::PeakMemoryAboveBudget {
            backend: name,
            predicted_entries: oversized.entries,
            budget_entries: budget.max_peak_entries,
        })?;

        ops.accumulate(step_ops);
        let entries = produced.entries() as u64;
        peak_entries = peak_entries.max(entries);
        total_entries = total_entries.saturating_add(entries);
        intermediates.push(IntermediateReceipt {
            eliminated: step.eliminated.clone(),
            scope: produced.scope.clone(),
            entries,
        });
        messages.push(produced);
    }

    let answer = messages
        .pop()
        .expect("every schedule ends with a combine over the free scope");

    Ok(ComputedRegion::new(
        answer,
        ExecutionReceipt {
            backend,
            semiring: region.semiring(),
            order,
            induced_width,
            observed_multiply_ops: ops.multiply,
            observed_aggregate_ops: ops.aggregate,
            observed_peak_entries: peak_entries,
            observed_total_entries: total_entries,
            intermediates,
        },
    ))
}
