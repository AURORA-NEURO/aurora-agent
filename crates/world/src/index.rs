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
    pub shadowed_variables: Vec<String>,
}

impl WorldIndex {
    pub fn build(facts: &[Fact], factors: &[Factor]) -> Self {
        let mut index = WorldIndex::default();

        for (position, fact) in facts.iter().enumerate() {
            index.fact_by_id.insert(fact.id.as_str().to_string(), position);
            if index
                .fact_by_variable
                .insert(fact.provides.as_str().to_string(), position)
                .is_some()
            {
                index.shadowed_variables.push(fact.provides.as_str().to_string());
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
}
