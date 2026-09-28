//! Backward dependency slicing.
//!
//! Blueprint 43.17: derive a query-specific logical program by slicing backwards from the
//! targets through typed factor dependencies. A variable enters the slice only if some factor
//! producing a needed variable consumes it.
//!
//! This is what lets a 761-fact world compile to a handful of facts: the 750 exploratory
//! summaries in the reference world are richly *connected* — a graph walk would drag them in —
//! but no factor path leads from them to the target, so no amount of adjacency makes them
//! relevant. That distinction is the whole argument for slicing over neighbourhood traversal.

use bioprism_world::WorldSource;
use std::collections::BTreeSet;

use crate::error::FiberError;
use bioprism_world::WorldSourceError;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Slice {
    /// Variables reachable backwards from the targets, including the targets themselves.
    pub needed_variables: BTreeSet<String>,
    /// Factors that produce some needed variable.
    pub selected_factors: BTreeSet<String>,
}

/// Slices backwards from `targets`.
///
/// Terminates on cyclic factor graphs: a factor is expanded at most once, and a variable is
/// pushed at most once.
/// Fallible because a storage failure cannot be treated as an empty dependency slice.
pub fn backward_slice<'a, S, I>(source: &S, targets: I) -> Result<Slice, FiberError>
where
    S: WorldSource + ?Sized,
    I: IntoIterator<Item = &'a str>,
{
    let mut result = Slice::default();
    let mut stack: Vec<String> = Vec::new();

    for target in targets {
        if result.needed_variables.insert(target.to_string()) {
            stack.push(target.to_string());
        }
    }

    while let Some(variable) = stack.pop() {
        for factor_id in source.producer_ids(&variable)? {
            if !result.selected_factors.insert(factor_id.clone()) {
                continue;
            }
            let factor = source.factor(&factor_id)?.ok_or_else(|| {
                FiberError::WorldSource(WorldSourceError::Corrupt(format!(
                    "producer index names missing factor `{factor_id}`"
                )))
            })?;
            for input in &factor.inputs {
                if result.needed_variables.insert(input.as_str().to_string()) {
                    stack.push(input.as_str().to_string());
                }
            }
        }
    }

    Ok(result)
}

/// The largest input arity among selected factors.
///
/// Reported in the certificate's structural block as a cheap proxy for compiled width (43.18).
/// It is *not* treewidth and must not be presented as such.
pub fn max_selected_arity<S: WorldSource + ?Sized>(
    source: &S,
    selected_factors: &BTreeSet<String>,
) -> Result<usize, FiberError> {
    let mut max_arity = 0;
    for id in selected_factors {
        let factor = source.factor(id)?.ok_or_else(|| {
            FiberError::WorldSource(WorldSourceError::Corrupt(format!(
                "selected factor `{id}` is missing from the source"
            )))
        })?;
        max_arity = max_arity.max(factor.arity());
    }
    Ok(max_arity)
}
