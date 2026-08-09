//! The world and query half of a slice.
//!
//! Blueprint 38.01 requires a reference world to be reproducible from a deterministic generator
//! and its manifests, not shipped as an opaque blob. A [`SliceWorld`] is therefore a *spec plus a
//! query overlay*, both serialisable: the world is whatever `bioprism-worldgen` produces for that
//! spec, and the query is whatever `bioprism-worldgen` produces for it with the overlay's fields
//! substituted.
//!
//! The overlay exists because the generator emits one canonical audit query — every protected tag,
//! a 64-fact budget, a decision time that sits exactly on the training event. Several of the
//! properties in [`crate::Property`] are only visible when one of those is moved: a budget below
//! the protected closure, a decision time before the training release, a target that reaches
//! almost nothing. Moving a query field is not a different world, so it belongs here rather than
//! in a second generator.

use crate::error::ExampleError;
use bioprism_fiber::Query;
use bioprism_world::World;
use bioprism_worldgen::{generate, WorldSpec};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// Fields substituted into the generated `fiber-query/0.1` document.
///
/// Every field is optional and `None` means "keep what the generator emitted". The overlay never
/// adds a field the schema does not define, so an overlaid query is still a v0.1 query and is
/// parsed by the same [`Query::from_json`] a caller would use.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueryOverlay {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub query_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub targets: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub protected_tags: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decision_time: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_facts: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub goal: Option<String>,
}

impl QueryOverlay {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn query_id(mut self, value: impl Into<String>) -> Self {
        self.query_id = Some(value.into());
        self
    }

    pub fn targets<I, S>(mut self, values: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.targets = Some(values.into_iter().map(Into::into).collect());
        self
    }

    pub fn protected_tags<I, S>(mut self, values: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.protected_tags = Some(values.into_iter().map(Into::into).collect());
        self
    }

    pub fn decision_time(mut self, value: impl Into<String>) -> Self {
        self.decision_time = Some(value.into());
        self
    }

    pub fn max_facts(mut self, value: usize) -> Self {
        self.max_facts = Some(value);
        self
    }

    pub fn goal(mut self, value: impl Into<String>) -> Self {
        self.goal = Some(value.into());
        self
    }

    fn apply(&self, document: &mut Value) {
        let Some(map) = document.as_object_mut() else {
            return;
        };
        if let Some(value) = &self.query_id {
            map.insert("query_id".into(), json!(value));
        }
        if let Some(value) = &self.targets {
            map.insert("targets".into(), json!(value));
        }
        if let Some(value) = &self.protected_tags {
            map.insert("protected_tags".into(), json!(value));
        }
        if let Some(value) = &self.decision_time {
            map.insert("decision_time".into(), json!(value));
        }
        if let Some(value) = &self.goal {
            map.insert("goal".into(), json!(value));
        }
        if let Some(value) = self.max_facts {
            if let Some(budgets) = map.get_mut("budgets").and_then(Value::as_object_mut) {
                budgets.insert("max_facts".into(), json!(value));
            }
        }
    }
}

/// A generated world paired with the query asked of it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SliceWorld {
    pub spec: WorldSpec,
    #[serde(default)]
    pub query: QueryOverlay,
}

impl SliceWorld {
    pub fn new(spec: WorldSpec) -> Self {
        SliceWorld {
            spec,
            query: QueryOverlay::default(),
        }
    }

    pub fn with_query(mut self, overlay: QueryOverlay) -> Self {
        self.query = overlay;
        self
    }

    /// The two wire documents, before parsing.
    ///
    /// Exposed because 38.01's acceptance list starts with "a clean machine loads the world": a
    /// reader must be able to write these out and hand them to any conforming implementation,
    /// not only to the one in this workspace.
    pub fn documents(&self) -> (Value, Value) {
        let generated = generate(&self.spec);
        let mut query = generated.query;
        self.query.apply(&mut query);
        (generated.world, query)
    }

    /// Parses both documents.
    ///
    /// A failure here is [`ExampleError`], not a slice failure: an example whose own world does
    /// not load has demonstrated nothing about the platform.
    pub fn build(&self, slice_id: &str) -> Result<(World, Query), ExampleError> {
        let (world_doc, query_doc) = self.documents();
        let world = World::from_json(world_doc).map_err(|source| ExampleError::World {
            slice: slice_id.to_string(),
            source,
        })?;
        let query = Query::from_json(query_doc).map_err(|source| ExampleError::Query {
            slice: slice_id.to_string(),
            source,
        })?;
        Ok((world, query))
    }
}
