//! Local evidence sections.
//!
//! Blueprint 43.04: evidence is stored as *local sections indexed by validity scope*. A fact is
//! not a globally true statement; it is a value that holds inside one scope. `provides` names
//! the variable the section supplies to the factor calculus.

use crate::error::WorldError;
use crate::json::{object, required, required_str, string_list};
use bioprism_ids::{FactId, VariableName};
use bioprism_scope::ScopeKey;
use serde_json::Value;
use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq)]
pub struct Fact {
    pub id: FactId,
    pub provides: VariableName,
    pub value: Value,
    pub scope: ScopeKey,
    pub tags: BTreeSet<String>,
    pub provenance: Vec<String>,
    raw: Value,
}

impl Fact {
    /// The original document for this fact.
    ///
    /// Decision Sections echo evidence back to the model, and certificates hash what was
    /// delivered. Rebuilding that payload from the typed form would drop any field this version
    /// does not model, changing the hash and breaking replay against another implementation.
    pub fn raw(&self) -> &Value {
        &self.raw
    }

    pub fn from_json(value: &Value) -> Result<Self, WorldError> {
        let map = object(value, "fact")?;
        let raw_id = required_str(map, "id", "fact")?;
        let subject = format!("fact {raw_id}");

        let id = FactId::parse(raw_id.clone())
            .map_err(|e| WorldError::Identifier { subject: subject.clone(), message: e.to_string() })?;
        let provides = VariableName::parse(required_str(map, "provides", &subject)?)
            .map_err(|e| WorldError::Identifier { subject: subject.clone(), message: e.to_string() })?;
        let scope = ScopeKey::from_json(required(map, "scope", &subject)?)
            .map_err(|source| WorldError::Scope { subject: subject.clone(), source })?;

        Ok(Fact {
            id,
            provides,
            value: required(map, "value", &subject)?.clone(),
            scope,
            tags: string_list(map, "tags", &subject)?.into_iter().collect(),
            provenance: string_list(map, "provenance", &subject)?,
            raw: value.clone(),
        })
    }

    pub fn has_any_tag(&self, tags: &BTreeSet<String>) -> bool {
        !self.tags.is_disjoint(tags)
    }

    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.contains(tag)
    }
}
