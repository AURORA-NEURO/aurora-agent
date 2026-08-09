//! Typed factors.
//!
//! Blueprint 43.07: interactions are typed factors `φ_i : X_{S_i} → K_i`, which may be stored
//! extensionally, computed intensionally, or compiled from rules. A bipartite factor graph or
//! a hypergraph is one *materialised plan view* of this set, never the canonical form — so a
//! factor carries its signature and cost, not an incidence list.

use crate::error::WorldError;
use crate::json::{object, optional_f64, required_str, required_string_list, string_list};
use bioprism_ids::{FactorId, VariableName};
use bioprism_scope::ScopeKey;
use serde_json::Value;
use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq)]
pub struct Factor {
    pub id: FactorId,
    pub inputs: Vec<VariableName>,
    pub outputs: Vec<VariableName>,
    pub kind: String,
    pub scope: Option<ScopeKey>,
    pub tags: BTreeSet<String>,
    pub cost: Option<f64>,
    raw: Value,
}

impl Factor {
    /// The original document for this factor, echoed verbatim into Decision Sections.
    pub fn raw(&self) -> &Value {
        &self.raw
    }

    pub fn from_json(value: &Value) -> Result<Self, WorldError> {
        let map = object(value, "factor")?;
        let raw_id = required_str(map, "id", "factor")?;
        let subject = format!("factor {raw_id}");

        let id = FactorId::parse(raw_id.clone())
            .map_err(|e| WorldError::Identifier { subject: subject.clone(), message: e.to_string() })?;

        let parse_vars = |names: Vec<String>| -> Result<Vec<VariableName>, WorldError> {
            names
                .into_iter()
                .map(|name| {
                    VariableName::parse(name).map_err(|e| WorldError::Identifier {
                        subject: subject.clone(),
                        message: e.to_string(),
                    })
                })
                .collect()
        };

        let scope = match map.get("scope") {
            None | Some(Value::Null) => None,
            Some(raw) => Some(
                ScopeKey::from_json(raw)
                    .map_err(|source| WorldError::Scope { subject: subject.clone(), source })?,
            ),
        };

        Ok(Factor {
            id,
            inputs: parse_vars(required_string_list(map, "inputs", &subject)?)?,
            outputs: parse_vars(required_string_list(map, "outputs", &subject)?)?,
            kind: required_str(map, "kind", &subject)?,
            scope,
            tags: string_list(map, "tags", &subject)?.into_iter().collect(),
            cost: optional_f64(map, "cost", &subject)?,
            raw: value.clone(),
        })
    }

    /// Number of distinct input variables. Used as the compiled-arity statistic reported in the
    /// certificate's structural block (43.18); it is a proxy for width, not width itself.
    pub fn arity(&self) -> usize {
        self.inputs.len()
    }

    pub fn is_deterministic_rule(&self) -> bool {
        self.kind == "deterministic_rule"
    }
}
