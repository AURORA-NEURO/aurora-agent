//! Lookup indices over a world.
//!
//! Indices store positions rather than references so the index can be held alongside the world
//! without borrow entanglement. Where the CPython reference builds a dict comprehension that
//! silently lets a later duplicate win, this reproduces that behaviour exactly and records the
//! shadowed entries so [`crate::validate`] can report them.

use crate::fact::Fact;
use crate::factor::Factor;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Default)]
pub struct WorldIndex {
    pub fact_by_id: BTreeMap<String, usize>,
    pub factor_by_id: BTreeMap<String, usize>,
    pub fact_by_variable: BTreeMap<String, usize>,
    pub producers_by_variable: BTreeMap<String, Vec<usize>>,
    /// Positions of the facts carrying each tag, in document order.
    ///
    /// The protected closure and the omission counts are the only two questions the compiler asks
    /// about tags, and both were answered by scanning the whole corpus — a dozen full passes per
    /// compile, on a world whose compiled region is a dozen facts. `WorldIndex` is already built
    /// once per world and already walks every fact, so this costs one pass and removes the rest.
    /// A `BTreeMap` rather than a hash map because `WorldIndex` derives `Debug` and this workspace
    /// treats a nondeterministic rendering of an index as a defect.
    pub facts_by_tag: BTreeMap<String, Vec<usize>>,
    pub shadowed_variables: Vec<String>,
    /// Positions of the facts that provide a variable and *lost* to a later one, in document order.
    ///
    /// [`Self::fact_by_variable`] keeps only the winner, so before this existed a shadowed fact was
    /// unreachable through any index and the compiler could not tell it apart from a fact nothing
    /// in the world depends on. The two are opposite claims: a shadowed fact provides a variable
    /// the slice may well need, so it has a backward dependency path and its omission is a
    /// tiebreak rather than a proof. `shadowed_variables` records *that* shadowing happened, for
    /// [`crate::validate`]; this records *which facts* it happened to, which is what a per-fact
    /// influence classification needs.
    pub shadowed_by_variable: BTreeMap<String, Vec<usize>>,
}

impl WorldIndex {
    pub fn build(facts: &[Fact], factors: &[Factor]) -> Self {
        let mut index = WorldIndex::default();

        for (position, fact) in facts.iter().enumerate() {
            index.fact_by_id.insert(fact.id.as_str().to_string(), position);
            if let Some(displaced) = index
                .fact_by_variable
                .insert(fact.provides.as_str().to_string(), position)
            {
                index.shadowed_variables.push(fact.provides.as_str().to_string());
                index
                    .shadowed_by_variable
                    .entry(fact.provides.as_str().to_string())
                    .or_default()
                    .push(displaced);
            }
            for tag in &fact.tags {
                index.facts_by_tag.entry(tag.clone()).or_default().push(position);
            }
        }

        for (position, factor) in factors.iter().enumerate() {
            index
                .factor_by_id
                .insert(factor.id.as_str().to_string(), position);
            for output in &factor.outputs {
                index
                    .producers_by_variable
                    .entry(output.as_str().to_string())
                    .or_default()
                    .push(position);
            }
        }

        index
    }

    pub fn fact_position(&self, id: &str) -> Option<usize> {
        self.fact_by_id.get(id).copied()
    }

    pub fn factor_position(&self, id: &str) -> Option<usize> {
        self.factor_by_id.get(id).copied()
    }

    pub fn fact_position_for_variable(&self, variable: &str) -> Option<usize> {
        self.fact_by_variable.get(variable).copied()
    }

    pub fn producers(&self, variable: &str) -> &[usize] {
        self.producers_by_variable
            .get(variable)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    /// Positions of the facts providing `variable` that a later fact shadowed, in document order.
    pub fn shadowed_providers(&self, variable: &str) -> &[usize] {
        self.shadowed_by_variable
            .get(variable)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    /// Positions of the facts carrying `tag`, in document order.
    pub fn tagged(&self, tag: &str) -> &[usize] {
        self.facts_by_tag
            .get(tag)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }
}
