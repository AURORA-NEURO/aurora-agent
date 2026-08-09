//! Emitting `fiber-world/0.1` documents.
//!
//! Blueprint 43.02 fixes the world as `F = ⟨B, p:E→B, Φ, K, P, L⟩`; the wire schema materialises
//! three of those six, and [`bioprism_world::World`] is the acceptance check. This module is the
//! narrow writer that sits in front of it: it assembles the three arrays in a fixed key order and
//! then hands the document to `World::from_json`, so a world that this crate ships is a world the
//! reference runtime accepts, by construction rather than by assertion.
//!
//! # Why a writer and not a second generator
//!
//! `bioprism-worldgen` already generates the split-integrity family, and §38 does not ask for a
//! rival generator — it asks for *worlds*. The knobs `worldgen` exposes (attachment, relay depth,
//! tag camouflage) are reused here unchanged, imported from `bioprism_worldgen::spec`, as is its
//! `SplitMix64`. What `worldgen` cannot express is which variables an event manages, which are
//! protected, and where the decision cut falls relative to the releases — the three things §38.08
//! turns on. Those are enumerated in [`crate::knobs`] rather than silently reimplemented.
//!
//! # Not implemented
//!
//! No scope algebra, no abstract-domain registry, no cover: `fiber-world/0.1` carries none of
//! them, and inventing wire fields here would produce documents no other crate in the workspace
//! could read.

use crate::error::BioWorldError;
use bioprism_ids::ContentHash;
use bioprism_world::World;
use serde_json::{json, Map, Value};

/// The wire schema every world in this crate emits.
pub const WORLD_SCHEMA_VERSION: &str = "fiber-world/0.1";

/// Accumulates the three arrays of a `fiber-world/0.1` document.
///
/// Insertion order is the emitted order (`serde_json` is built with `preserve_order` in this
/// workspace), which is what lets a fixture on disk be compared to a freshly built world byte for
/// byte.
#[derive(Debug, Clone)]
pub struct WorldBuilder {
    world_id: String,
    description: String,
    cohort: String,
    facts: Vec<Value>,
    factors: Vec<Value>,
    events: Vec<Value>,
}

impl WorldBuilder {
    pub fn new(
        world_id: impl Into<String>,
        description: impl Into<String>,
        cohort: impl Into<String>,
    ) -> Self {
        WorldBuilder {
            world_id: world_id.into(),
            description: description.into(),
            cohort: cohort.into(),
            facts: Vec::new(),
            factors: Vec::new(),
            events: Vec::new(),
        }
    }

    /// Adds a local section (43.04). `tags` is the protection and camouflage vocabulary; the
    /// closure matches whole tags, so a tag that merely *tokenises* into the protected vocabulary
    /// is correctly not protected.
    pub fn fact(
        &mut self,
        id: &str,
        provides: &str,
        value: Value,
        tags: &[&str],
        provenance: &[&str],
    ) -> &mut Self {
        self.facts.push(json!({
            "id": id,
            "provides": provides,
            "value": value,
            "scope": { "cohort": self.cohort },
            "tags": tags,
            "provenance": provenance,
        }));
        self
    }

    /// Adds a typed factor (43.07). `kind` is free text on the wire; this crate uses it to mark
    /// the roles its structural characterisation reads back — `hypothesis_support_rule`,
    /// `mutual_exclusion_rule`, `relay_rule` — so those roles are declared in the document rather
    /// than inferred from names.
    pub fn factor(
        &mut self,
        id: &str,
        inputs: &[&str],
        outputs: &[&str],
        kind: &str,
        tags: &[&str],
        cost: f64,
    ) -> &mut Self {
        self.factors.push(json!({
            "id": id,
            "inputs": inputs,
            "outputs": outputs,
            "kind": kind,
            "scope": { "cohort": self.cohort },
            "tags": tags,
            "cost": cost,
        }));
        self
    }

    /// Adds a causal event (43.09).
    ///
    /// `event_time` and `availability_time` are separate arguments and neither defaults to the
    /// other, because collapsing them is the temporal-leakage bug the whole section exists to
    /// catch: a result describing a February specimen but released in April is not readable by a
    /// March decision.
    pub fn event(
        &mut self,
        id: &str,
        event_time: &str,
        availability_time: &str,
        causal_parents: &[&str],
        produces: &[&str],
    ) -> &mut Self {
        self.events.push(json!({
            "id": id,
            "event_time": event_time,
            "availability_time": availability_time,
            "causal_parents": causal_parents,
            "produces": produces,
        }));
        self
    }

    pub fn fact_count(&self) -> usize {
        self.facts.len()
    }

    pub fn factor_count(&self) -> usize {
        self.factors.len()
    }

    pub fn event_count(&self) -> usize {
        self.events.len()
    }

    /// The assembled document, without acceptance checking.
    pub fn document(&self) -> Value {
        json!({
            "schema_version": WORLD_SCHEMA_VERSION,
            "world_id": self.world_id,
            "description": self.description,
            "facts": self.facts,
            "factors": self.factors,
            "events": self.events,
        })
    }

    /// The document, checked against the reference runtime's acceptance rules.
    pub fn build(&self) -> Result<BioWorld, BioWorldError> {
        BioWorld::from_document(self.document())
    }
}

/// A world this crate ships, holding both the document and its parsed form.
///
/// The raw document is kept because content addressing is taken over the *original* bytes;
/// re-serialising the typed form would drop any field this version does not model and change the
/// digest, which is exactly the cross-implementation replay hazard 43.26 warns about.
#[derive(Debug, Clone)]
pub struct BioWorld {
    document: Value,
    world: World,
}

impl BioWorld {
    pub fn from_document(document: Value) -> Result<Self, BioWorldError> {
        let world_id = document
            .get("world_id")
            .and_then(Value::as_str)
            .unwrap_or("<unnamed>")
            .to_string();
        let world =
            World::from_json(document.clone()).map_err(|source| BioWorldError::WorldRejected {
                world_id,
                message: source.to_string(),
            })?;
        Ok(BioWorld { document, world })
    }

    /// Parses a document from JSON text, as a consumer reading a shipped fixture would.
    pub fn from_json_str(text: &str) -> Result<Self, BioWorldError> {
        let document: Value =
            serde_json::from_str(text).map_err(|source| BioWorldError::WorldRejected {
                world_id: "<unparsed>".into(),
                message: source.to_string(),
            })?;
        BioWorld::from_document(document)
    }

    pub fn id(&self) -> &str {
        self.world.world_id.as_str()
    }

    pub fn document(&self) -> &Value {
        &self.document
    }

    pub fn world(&self) -> &World {
        &self.world
    }

    /// Pretty-printed JSON, the form the fixtures on disk carry.
    pub fn to_pretty_json(&self) -> String {
        serde_json::to_string_pretty(&self.document).expect("world document is finite JSON")
    }

    pub fn digest(&self) -> Result<String, BioWorldError> {
        ContentHash::of_value(&self.document)
            .map(|hash| hash.as_str().to_string())
            .map_err(|source| BioWorldError::Digest {
                subject: format!("world {}", self.id()),
                message: source.to_string(),
            })
    }
}

/// A per-subject value map, in subject order.
///
/// Facts in these worlds are cohort-scoped sections whose value is one entry per subject, matching
/// the shape `worldgen` uses; keeping the shape identical means a consumer written against the
/// generated family reads these worlds without a special case.
pub fn per_subject(subjects: &[String], mut value_for: impl FnMut(usize, &str) -> Value) -> Value {
    let map: Map<String, Value> = subjects
        .iter()
        .enumerate()
        .map(|(index, subject)| (subject.clone(), value_for(index, subject)))
        .collect();
    Value::Object(map)
}

/// Stable subject identifiers, `S001`-style, matching the generated family.
pub fn subject_ids(count: usize) -> Vec<String> {
    (1..=count).map(|n| format!("S{n:03}")).collect()
}
