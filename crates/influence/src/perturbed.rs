//! Rebuilding a region with one factor's potential replaced.
//!
//! [`bioprism_backends::QueryRegion`] is immutable by construction — its fields are private and it
//! is validated once at build time so the executor's inner loop can index tables without guards.
//! That is the right design and it means a perturbation has to go back through the builder, which
//! is what this module does. Every component is read back through the region's public accessors,
//! so a region that round-trips through here with the identity perturbation differs from the one
//! it came from in exactly one assumption line. The test suite asserts that, because a rebuild
//! that silently dropped a cardinality source or a free variable would make every exact bound a
//! comparison between two different questions.
//!
//! The rebuilt region carries an extra assumption line naming the perturbation. A region that has
//! been altered and does not say so is the failure this workspace exists to prevent.

use crate::error::InfluenceError;
use bioprism_backends::{CardinalitySource, QueryRegion, RegionFactor};

/// Returns the region with `factor_id`'s table replaced, leaving everything else identical.
///
/// Fails when the factor is absent, when it carries no table to replace, or when the replacement
/// has the wrong number of entries — all three are caller bugs rather than analysis outcomes.
pub fn with_replaced_table(
    region: &QueryRegion,
    factor_id: &str,
    table: Vec<f64>,
    note: &str,
) -> Result<QueryRegion, InfluenceError> {
    let target = region
        .factors()
        .iter()
        .find(|factor| factor.id() == factor_id)
        .ok_or_else(|| InfluenceError::UnknownFactor {
            region: region.label().to_string(),
            factor: factor_id.to_string(),
        })?;
    if target.table().is_none() {
        return Err(InfluenceError::UntabledFactor {
            factor: factor_id.to_string(),
        });
    }

    let mut builder = QueryRegion::builder(region.label()).semiring(region.semiring());
    for (name, cardinality) in region.cardinality() {
        builder = match region.cardinality_source(name) {
            Some(CardinalitySource::Assumed) => builder.assumed_variable(name, *cardinality),
            _ => builder.observed_variable(name, *cardinality),
        };
    }
    for factor in region.factors() {
        let replacement = if factor.id() == factor_id {
            RegionFactor::with_table(factor.id(), factor.scope().to_vec(), table.clone())
        } else {
            match factor.table() {
                Some(existing) => {
                    RegionFactor::with_table(factor.id(), factor.scope().to_vec(), existing.to_vec())
                }
                None => RegionFactor::structural(factor.id(), factor.scope().to_vec()),
            }
        };
        builder = builder.factor(replacement);
    }
    for name in region.free_variables() {
        builder = builder.free(name);
    }
    for assumption in region.assumptions() {
        builder = builder.assumption(assumption);
    }
    builder = builder.assumption(note).provenance(region.provenance());

    builder
        .build()
        .map_err(|source| InfluenceError::PerturbedRegionRejected {
            message: source.to_string(),
        })
}

/// The region with `factor_id` removed in the sense of [`crate::Perturbation::Removal`].
pub fn with_factor_removed(
    region: &QueryRegion,
    factor_id: &str,
) -> Result<QueryRegion, InfluenceError> {
    let target = region
        .factors()
        .iter()
        .find(|factor| factor.id() == factor_id)
        .ok_or_else(|| InfluenceError::UnknownFactor {
            region: region.label().to_string(),
            factor: factor_id.to_string(),
        })?;
    let entries = target
        .table()
        .ok_or_else(|| InfluenceError::UntabledFactor {
            factor: factor_id.to_string(),
        })?
        .len();
    with_replaced_table(
        region,
        factor_id,
        vec![1.0; entries],
        &format!("factor {factor_id:?} replaced by the all-ones potential to measure its influence"),
    )
}

/// The subset of a factor's table each entry is compared against when checking admissibility.
pub(crate) fn table_of<'a>(
    region: &'a QueryRegion,
    factor_id: &str,
) -> Result<&'a [f64], InfluenceError> {
    let target = region
        .factors()
        .iter()
        .find(|factor| factor.id() == factor_id)
        .ok_or_else(|| InfluenceError::UnknownFactor {
            region: region.label().to_string(),
            factor: factor_id.to_string(),
        })?;
    target.table().ok_or_else(|| InfluenceError::UntabledFactor {
        factor: factor_id.to_string(),
    })
}
