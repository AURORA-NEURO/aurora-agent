//! The dense combine-and-aggregate kernel.
//!
//! One primitive implements every physical step in this crate: iterate an assignment space,
//! multiply the operands that project onto it, and aggregate into a result indexed by the
//! surviving variables. Variable elimination calls it once per bound variable; direct
//! materialisation calls it once for all of them. Sharing the kernel is what makes the comparison
//! in [`crate::phase`] a claim about *schedules* rather than about implementation quality, which is
//! the equal-engineering requirement of 43.38.
//!
//! Tables are row-major over an ordered scope with the last variable varying fastest. Operand and
//! result offsets are maintained incrementally as the odometer advances, so a cell costs one
//! multiply per operand and one aggregate regardless of arity. The combined space is never
//! allocated — only the result is — which is what keeps direct materialisation honest about
//! memory.

use crate::semiring::Semiring;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DenseFactor {
    pub scope: Vec<String>,
    pub values: Vec<f64>,
}

impl DenseFactor {
    pub fn entries(&self) -> usize {
        self.values.len()
    }
}

/// A step whose tables do not fit in addressable memory.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Oversized {
    pub scope: Vec<String>,
    pub entries: f64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct OpCount {
    pub multiply: u64,
    pub aggregate: u64,
}

impl OpCount {
    pub fn accumulate(&mut self, other: OpCount) {
        self.multiply = self.multiply.saturating_add(other.multiply);
        self.aggregate = self.aggregate.saturating_add(other.aggregate);
    }
}

/// Strides for a row-major layout over `scope`, last variable fastest.
fn strides(scope: &[String], cardinality: &BTreeMap<String, usize>) -> Vec<usize> {
    let mut result = vec![1usize; scope.len()];
    for position in (0..scope.len().saturating_sub(1)).rev() {
        let next = cardinality
            .get(&scope[position + 1])
            .copied()
            .expect("region validation guarantees every scope variable has a cardinality");
        result[position] = result[position + 1].saturating_mul(next);
    }
    result
}

/// Offset contributed to `target`'s index by each position of `combined`.
fn contributions(
    combined: &[String],
    target: &[String],
    cardinality: &BTreeMap<String, usize>,
) -> Vec<usize> {
    let target_strides = strides(target, cardinality);
    combined
        .iter()
        .map(|name| {
            target
                .iter()
                .position(|other| other == name)
                .map(|position| target_strides[position])
                .unwrap_or(0)
        })
        .collect()
}

fn checked_extent(
    scope: &[String],
    cardinality: &BTreeMap<String, usize>,
) -> Result<usize, Oversized> {
    let mut extent = 1usize;
    for name in scope {
        let card = cardinality
            .get(name)
            .copied()
            .expect("region validation guarantees every scope variable has a cardinality");
        extent = extent.checked_mul(card).ok_or_else(|| Oversized {
            scope: scope.to_vec(),
            entries: scope
                .iter()
                .map(|name| cardinality[name] as f64)
                .product::<f64>(),
        })?;
    }
    Ok(extent)
}

/// Runs one combine-and-aggregate step.
///
/// `combined` is the assignment space to iterate and `result` the scope that survives it; the
/// variables in `combined` but not `result` are the ones aggregated away. When the two are equal
/// the step is a pure product and performs no aggregation.
pub(crate) fn combine(
    inputs: &[&DenseFactor],
    combined: &[String],
    result: &[String],
    cardinality: &BTreeMap<String, usize>,
    semiring: Semiring,
) -> Result<(DenseFactor, OpCount), Oversized> {
    let cells = checked_extent(combined, cardinality)?;
    let entries = checked_extent(result, cardinality)?;

    let card: Vec<usize> = combined.iter().map(|name| cardinality[name]).collect();
    let input_contributions: Vec<Vec<usize>> = inputs
        .iter()
        .map(|factor| contributions(combined, &factor.scope, cardinality))
        .collect();
    let result_contributions = contributions(combined, result, cardinality);

    let aggregating = result.len() != combined.len();
    let mut values = vec![semiring.zero(); entries];
    let mut assignment = vec![0usize; combined.len()];
    let mut offsets = vec![0usize; inputs.len()];
    let mut result_offset = 0usize;

    loop {
        let mut product = semiring.one();
        for (position, factor) in inputs.iter().enumerate() {
            product = semiring.mul(product, factor.values[offsets[position]]);
        }
        if aggregating {
            values[result_offset] = semiring.add(values[result_offset], product);
        } else {
            values[result_offset] = product;
        }

        let mut position = combined.len();
        let mut carried = true;
        while position > 0 {
            position -= 1;
            if assignment[position] + 1 < card[position] {
                assignment[position] += 1;
                for (index, contribution) in input_contributions.iter().enumerate() {
                    offsets[index] += contribution[position];
                }
                result_offset += result_contributions[position];
                carried = false;
                break;
            }
            let unwind = assignment[position];
            for (index, contribution) in input_contributions.iter().enumerate() {
                offsets[index] -= unwind * contribution[position];
            }
            result_offset -= unwind * result_contributions[position];
            assignment[position] = 0;
        }
        if carried {
            break;
        }
    }

    let cells = cells as u64;
    let ops = OpCount {
        multiply: cells.saturating_mul(inputs.len() as u64),
        aggregate: if aggregating { cells } else { 0 },
    };

    Ok((
        DenseFactor {
            scope: result.to_vec(),
            values,
        },
        ops,
    ))
}
