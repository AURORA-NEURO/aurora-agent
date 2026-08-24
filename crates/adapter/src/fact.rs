//! Building `fiber-world/0.1` fact documents.
//!
//! An adapter's output is not an arbitrary graph fragment: it is a set of local sections in
//! the sense of blueprint 43.04, each valid inside one [`ScopeKey`]. This module is the single
//! place that knows the wire shape, so that a new adapter cannot invent a slightly different
//! one and have it accepted by a lenient reader downstream.
//!
//! The qualifier fields — unit, coordinate frame, ontology version — are carried *inside* the
//! value rather than in tags. Tags are a filtering channel that the compiler uses for
//! protected-closure computation (43.13); putting a unit there would make `mm` a thing a query
//! can select on, which it is not. A qualified value is a compound value, and modelling it as
//! one keeps the magnitude and the unit inseparable in every downstream copy.

use crate::error::AdapterError;
use crate::location::SourceLocation;
use bioprism_ids::{FactId, VariableName};
use bioprism_scope::ScopeKey;
use serde_json::{Map, Value};
use std::collections::BTreeSet;

/// Qualifiers that travel with a value. Each is `None` only when the adapter has positively
/// established that the dimension does not apply, never as a default for "did not look".
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ValueQualifiers {
    /// Unit of measurement, carried verbatim from the mapping policy.
    pub unit: Option<String>,
    /// Coordinate frame: a reference genome build, an imaging frame, a slide space.
    pub frame: Option<String>,
    /// Controlled vocabulary this value's term belongs to.
    pub ontology: Option<String>,
    /// Version of that vocabulary. 28.00 forbids mapping terms "without recording mapping
    /// version", so an ontology without a version is reported as an unmapped term rather than
    /// accepted as a mapping.
    pub ontology_version: Option<String>,
}

impl ValueQualifiers {
    pub fn is_empty(&self) -> bool {
        self.unit.is_none()
            && self.frame.is_none()
            && self.ontology.is_none()
            && self.ontology_version.is_none()
    }

    /// Wraps `value`, leaving it bare when there is nothing to qualify.
    ///
    /// The bare form matters: wrapping every scalar in `{"value": …}` would make the world's
    /// facts uniformly shaped but would also make "this number has no declared unit" and "this
    /// number is dimensionless" look identical, which is the conflation this crate exists to
    /// prevent everywhere else.
    pub fn apply(&self, value: Value) -> Value {
        if self.is_empty() {
            return value;
        }
        let mut map = Map::new();
        map.insert("value".to_string(), value);
        if let Some(unit) = &self.unit {
            map.insert("unit".to_string(), Value::String(unit.clone()));
        }
        if let Some(frame) = &self.frame {
            map.insert("frame".to_string(), Value::String(frame.clone()));
        }
        if let Some(ontology) = &self.ontology {
            map.insert("ontology".to_string(), Value::String(ontology.clone()));
        }
        if let Some(version) = &self.ontology_version {
            map.insert(
                "ontology_version".to_string(),
                Value::String(version.clone()),
            );
        }
        Value::Object(map)
    }
}

/// A fact under construction, before validation.
///
/// Drafts exist so that identifier and scope failures are reported against the source location
/// that produced them. A `serde_json::Value` assembled inline would fail validation later with
/// no idea which row it came from.
#[derive(Debug, Clone)]
pub struct FactDraft {
    id: String,
    provides: String,
    value: Value,
    scope: ScopeKey,
    tags: BTreeSet<String>,
    provenance: Vec<String>,
    origin: SourceLocation,
}

impl FactDraft {
    pub fn new(
        id: impl Into<String>,
        provides: impl Into<String>,
        value: Value,
        scope: ScopeKey,
        origin: SourceLocation,
    ) -> Self {
        FactDraft {
            id: id.into(),
            provides: provides.into(),
            value,
            scope,
            tags: BTreeSet::new(),
            provenance: Vec::new(),
            origin,
        }
    }

    pub fn tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.insert(tag.into());
        self
    }

    pub fn tags<I, S>(mut self, tags: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.tags.extend(tags.into_iter().map(Into::into));
        self
    }

    pub fn provenance(mut self, entry: impl Into<String>) -> Self {
        self.provenance.push(entry.into());
        self
    }

    pub fn provenances<I, S>(mut self, entries: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.provenance.extend(entries.into_iter().map(Into::into));
        self
    }

    pub fn origin(&self) -> &SourceLocation {
        &self.origin
    }

    /// Validates the identifiers and emits the wire document.
    ///
    /// Scope is serialized through [`ScopeKey`]'s own representation rather than rebuilt here,
    /// so that a scope this crate emits and a scope `bioprism-world` parses are the same
    /// object by construction rather than by agreement between two hand-written mappings.
    pub fn build(self) -> Result<Value, AdapterError> {
        let id = FactId::parse(self.id)
            .map_err(|error| AdapterError::identifier(self.origin.clone(), error))?;
        let provides = VariableName::parse(self.provides)
            .map_err(|error| AdapterError::identifier(self.origin.clone(), error))?;
        let scope = serde_json::to_value(&self.scope).map_err(|error| {
            AdapterError::malformed_fact(self.origin.clone(), error.to_string())
        })?;

        let mut map = Map::new();
        map.insert("id".to_string(), Value::String(id.to_string()));
        map.insert("provides".to_string(), Value::String(provides.to_string()));
        map.insert("value".to_string(), self.value);
        map.insert("scope".to_string(), scope);
        map.insert(
            "tags".to_string(),
            Value::Array(self.tags.into_iter().map(Value::String).collect()),
        );
        map.insert(
            "provenance".to_string(),
            Value::Array(self.provenance.into_iter().map(Value::String).collect()),
        );
        Ok(Value::Object(map))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bioprism_world::Fact;

    fn draft() -> FactDraft {
        FactDraft::new(
            "fact.demo.r1.age",
            "age",
            Value::from(41),
            ScopeKey::new().exact("cohort", "RG-DEMO-001"),
            SourceLocation::cell("demo", 1, "age"),
        )
    }

    #[test]
    fn a_built_draft_parses_as_a_fiber_world_fact() {
        let document = draft()
            .tag("clinical")
            .provenance("demo#record=1")
            .build()
            .unwrap();
        let fact = Fact::from_json(&document).unwrap();
        assert_eq!(fact.id.as_str(), "fact.demo.r1.age");
        assert_eq!(fact.provides.as_str(), "age");
        assert!(fact.has_tag("clinical"));
    }

    #[test]
    fn an_empty_identifier_fails_against_the_location_that_produced_it() {
        let bad = FactDraft::new(
            "",
            "age",
            Value::Null,
            ScopeKey::new(),
            SourceLocation::cell("demo", 3, "age"),
        );
        match bad.build().unwrap_err() {
            AdapterError::Identifier { location, .. } => assert_eq!(location.record, Some(3)),
            other => panic!("expected an identifier error, got {other}"),
        }
    }

    #[test]
    fn an_unqualified_value_is_not_wrapped_in_an_envelope() {
        let bare = ValueQualifiers::default().apply(Value::from(2.5));
        assert_eq!(bare, Value::from(2.5));
    }

    #[test]
    fn a_unit_is_carried_inside_the_value_not_as_a_tag() {
        let qualified = ValueQualifiers {
            unit: Some("mm".into()),
            ..Default::default()
        }
        .apply(Value::from(2.5));
        assert_eq!(qualified["unit"], Value::from("mm"));
        assert_eq!(qualified["value"], Value::from(2.5));
    }
}
