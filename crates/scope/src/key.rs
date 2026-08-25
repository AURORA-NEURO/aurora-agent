//! Scope keys and the refinement order.
//!
//! A `ScopeKey` is a conjunction of per-dimension constraints. Binding *more* dimensions, or
//! binding one *more tightly*, makes a scope narrower. Refinement is therefore a partial
//! order, not a total one: two scopes that constrain disjoint dimensions are incomparable,
//! and the compiler must not pretend otherwise.

use crate::class::{DimensionRegistry, ScopeClass};
use crate::error::ScopeError;
use crate::time::{Interval, Timestamp};
use bioprism_ids::to_canonical_string;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ScopeValue {
    Exact(String),
    OneOf(BTreeSet<String>),
    Window(Interval),
}

impl ScopeValue {
    /// True when `self` is at least as narrow as `coarser`.
    pub fn refines(&self, coarser: &ScopeValue) -> bool {
        use ScopeValue::*;
        match (self, coarser) {
            (Exact(a), Exact(b)) => a == b,
            (Exact(a), OneOf(set)) => set.contains(a),
            (OneOf(a), OneOf(b)) => a.is_subset(b),
            (OneOf(a), Exact(b)) => a.len() == 1 && a.contains(b),
            (Window(a), Window(b)) => a.refines(b),
            _ => false,
        }
    }

    pub fn describe(&self) -> String {
        match self {
            ScopeValue::Exact(v) => v.clone(),
            ScopeValue::OneOf(set) => {
                let items: Vec<&str> = set.iter().map(String::as_str).collect();
                format!("one_of[{}]", items.join(","))
            }
            ScopeValue::Window(interval) => format!(
                "[{},{})",
                interval.start.map(|t| t.to_rfc3339()).unwrap_or_else(|| "-inf".into()),
                interval.end.map(|t| t.to_rfc3339()).unwrap_or_else(|| "+inf".into()),
            ),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ScopeKey {
    bindings: BTreeMap<String, ScopeValue>,
}

impl ScopeKey {
    pub const fn new() -> Self {
        ScopeKey {
            bindings: BTreeMap::new(),
        }
    }

    pub fn bind(mut self, dimension: impl Into<String>, value: ScopeValue) -> Self {
        self.bindings.insert(dimension.into(), value);
        self
    }

    pub fn exact(self, dimension: impl Into<String>, value: impl Into<String>) -> Self {
        self.bind(dimension, ScopeValue::Exact(value.into()))
    }

    pub fn get(&self, dimension: &str) -> Option<&ScopeValue> {
        self.bindings.get(dimension)
    }

    pub fn dimensions(&self) -> impl Iterator<Item = &str> {
        self.bindings.keys().map(String::as_str)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &ScopeValue)> {
        self.bindings.iter().map(|(k, v)| (k.as_str(), v))
    }

    pub fn len(&self) -> usize {
        self.bindings.len()
    }

    pub fn is_empty(&self) -> bool {
        self.bindings.is_empty()
    }

    /// Parses the `scope` object carried by facts and factors in a FIBER world.
    pub fn from_json(value: &Value) -> Result<Self, ScopeError> {
        let object = value
            .as_object()
            .ok_or_else(|| ScopeError::NotAnObject(value_kind(value).to_string()))?;
        let mut bindings = BTreeMap::new();
        for (dimension, raw) in object {
            bindings.insert(dimension.clone(), parse_value(dimension, raw)?);
        }
        Ok(ScopeKey { bindings })
    }

    /// True when `self` is at least as narrow as `coarser`.
    pub fn refines(&self, coarser: &ScopeKey) -> bool {
        coarser.bindings.iter().all(|(dimension, coarse_value)| {
            self.bindings
                .get(dimension)
                .is_some_and(|fine| fine.refines(coarse_value))
        })
    }

    pub fn classes(&self, registry: &DimensionRegistry) -> BTreeSet<ScopeClass> {
        self.bindings
            .keys()
            .map(|name| registry.classify(name))
            .collect()
    }

    pub fn unclassified_dimensions(&self, registry: &DimensionRegistry) -> Vec<String> {
        registry.unclassified(self.bindings.keys().map(String::as_str))
    }
}

fn parse_value(dimension: &str, raw: &Value) -> Result<ScopeValue, ScopeError> {
    match raw {
        Value::String(text) => Ok(ScopeValue::Exact(text.clone())),
        Value::Bool(_) | Value::Number(_) => {
            let canonical = to_canonical_string(raw).map_err(|_| ScopeError::UntypableValue {
                dimension: dimension.to_string(),
                value: value_kind(raw).to_string(),
            })?;
            Ok(ScopeValue::Exact(canonical))
        }
        Value::Array(items) => {
            let mut set = BTreeSet::new();
            for item in items {
                match item {
                    Value::String(text) => {
                        set.insert(text.clone());
                    }
                    other => {
                        return Err(ScopeError::UntypableValue {
                            dimension: dimension.to_string(),
                            value: value_kind(other).to_string(),
                        })
                    }
                }
            }
            Ok(ScopeValue::OneOf(set))
        }
        Value::Object(fields) if fields.contains_key("start") || fields.contains_key("end") => {
            let start = parse_endpoint(fields.get("start"))?;
            let end = parse_endpoint(fields.get("end"))?;
            Ok(ScopeValue::Window(Interval { start, end }))
        }
        other => Err(ScopeError::UntypableValue {
            dimension: dimension.to_string(),
            value: value_kind(other).to_string(),
        }),
    }
}

fn parse_endpoint(raw: Option<&Value>) -> Result<Option<Timestamp>, ScopeError> {
    match raw {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(text)) => Ok(Some(Timestamp::parse(text)?)),
        Some(other) => Err(ScopeError::UntypableValue {
            dimension: "interval endpoint".to_string(),
            value: value_kind(other).to_string(),
        }),
    }
}

fn value_kind(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "bool",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}
