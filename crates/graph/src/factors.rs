//! Reading the factor documents a Decision Section carries.
//!
//! 43.25 echoes selected factors into the section as their *original documents*, so that the
//! delivered bytes hash exactly. That makes them untyped `Value`s at this boundary, and a
//! projection has to read a signature out of them before it can draw anything.
//!
//! The read is strict. A factor without an id, a kind or a signature is not projectable, and
//! [`ProjectionError::MalformedFactor`] says which index failed. Substituting a default would
//! invent structure that the compiled region does not contain — the same class of mistake as
//! inserting a guessed edge (43.01).

use crate::error::ProjectionError;
use bioprism_section::DecisionSection;
use serde_json::Value;

/// A factor's projectable signature, read from its echoed document.
#[derive(Debug, Clone, PartialEq)]
pub struct SelectedFactor {
    pub id: String,
    pub kind: String,
    pub inputs: Vec<String>,
    pub outputs: Vec<String>,
    pub scope: Option<Value>,
}

impl SelectedFactor {
    /// Distinct variables the factor touches. This is the arity a hyperedge would have.
    pub fn arity(&self) -> usize {
        let mut variables: Vec<&str> = self
            .inputs
            .iter()
            .chain(self.outputs.iter())
            .map(String::as_str)
            .collect();
        variables.sort_unstable();
        variables.dedup();
        variables.len()
    }

    /// Whether pairwise edges can express this factor without loss.
    ///
    /// A factor over two variables is a binary relation and survives the graph projection intact.
    /// Anything wider is a joint constraint that a set of pairwise edges cannot state, which is
    /// why 43.01 requires the factor inspector alongside the graph.
    pub fn is_multiway(&self) -> bool {
        self.arity() > 2
    }
}

/// Reads every selected factor, in section order.
pub fn selected_factors(section: &DecisionSection) -> Result<Vec<SelectedFactor>, ProjectionError> {
    section
        .selected_factors
        .iter()
        .enumerate()
        .map(|(index, document)| read_factor(index, document))
        .collect()
}

fn read_factor(index: usize, document: &Value) -> Result<SelectedFactor, ProjectionError> {
    let malformed = |detail: &str| ProjectionError::MalformedFactor {
        index,
        detail: detail.to_string(),
    };

    let map = document
        .as_object()
        .ok_or_else(|| malformed("factor document is not a JSON object"))?;

    let text = |field: &str| -> Result<String, ProjectionError> {
        map.get(field)
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| malformed(&format!("missing string field {field:?}")))
    };

    let names = |field: &str| -> Result<Vec<String>, ProjectionError> {
        let array = map
            .get(field)
            .and_then(Value::as_array)
            .ok_or_else(|| malformed(&format!("missing array field {field:?}")))?;
        array
            .iter()
            .map(|entry| {
                entry
                    .as_str()
                    .map(str::to_string)
                    .ok_or_else(|| malformed(&format!("{field} contains a non-string entry")))
            })
            .collect()
    };

    Ok(SelectedFactor {
        id: text("id")?,
        kind: text("kind")?,
        inputs: names("inputs")?,
        outputs: names("outputs")?,
        scope: match map.get("scope") {
            None | Some(Value::Null) => None,
            Some(scope) => Some(scope.clone()),
        },
    })
}
