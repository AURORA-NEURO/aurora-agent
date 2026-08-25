//! The FIBER world.
//!
//! Blueprint 43.02 defines an executable world as a six-part structure
//! `F = ⟨B, p:E→B, Φ, K, P, L⟩`. The `fiber-world/0.1` wire schema materialises three of those
//! six directly — local sections (`facts`), the factor family (`factors`) and the causal event
//! structure (`events`) — while the scope base rides on each section and the algebra and
//! abstract-domain registries are not yet on the wire. That gap is recorded honestly in
//! [`crate::validate`] rather than papered over with defaults.

use crate::error::WorldError;
use crate::event::CausalEvent;
use crate::fact::Fact;
use crate::factor::Factor;
use crate::index::WorldIndex;
use crate::json::object;
use bioprism_ids::{ContentHash, WorldId};
use serde_json::Value;
use std::collections::BTreeSet;
use std::sync::OnceLock;

pub const WORLD_SCHEMA_VERSION: &str = "fiber-world/0.1";

#[derive(Debug, Clone)]
pub struct World {
    pub world_id: WorldId,
    pub description: Option<String>,
    pub facts: Vec<Fact>,
    pub factors: Vec<Factor>,
    pub events: Vec<CausalEvent>,
    index: WorldIndex,
    raw: Value,
    /// Cached [`World::content_hash`].
    ///
    /// [`crate::source::WorldSource::world_digest`] documents the digest as precomputed, because a
    /// backend that re-read its corpus to answer it would defeat the trait's whole purpose. The
    /// eager world was the one implementation that did not honour that: it re-canonicalised and
    /// re-hashed the entire document on every call, and a compile asks for the digest once, so a
    /// second compile against the same world paid for it again.
    digest: OnceLock<ContentHash>,
}

impl World {
    /// Parses a world and runs exactly the reference runtime's acceptance checks.
    ///
    /// The raw `Value` is retained because certificate hashes are taken over the *original*
    /// document. Re-serialising the typed form would drop unknown fields and change the hash,
    /// breaking cross-implementation replay even when every typed field round-trips.
    pub fn from_json(raw: Value) -> Result<Self, WorldError> {
        let map = object(&raw, "world").map_err(|_| WorldError::NotAnObject)?;

        let schema_version = map
            .get("schema_version")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if schema_version != WORLD_SCHEMA_VERSION {
            return Err(WorldError::UnsupportedSchema {
                expected: WORLD_SCHEMA_VERSION,
                actual: schema_version.to_string(),
            });
        }

        let world_id_text = map
            .get("world_id")
            .and_then(Value::as_str)
            .ok_or(WorldError::MissingField { field: "world_id", subject: "world".into() })?;
        let world_id = WorldId::parse(world_id_text).map_err(|e| WorldError::Identifier {
            subject: "world".into(),
            message: e.to_string(),
        })?;

        let facts = parse_seq(map.get("facts"), "facts", Fact::from_json)?;
        let factors = parse_seq(map.get("factors"), "factors", Factor::from_json)?;
        let events = parse_seq(map.get("events"), "events", CausalEvent::from_json)?;

        let world = World {
            world_id,
            description: map.get("description").and_then(Value::as_str).map(str::to_string),
            index: WorldIndex::build(&facts, &factors),
            facts,
            factors,
            events,
            raw,
            digest: OnceLock::new(),
        };
        world.validate_reference_compat()?;
        Ok(world)
    }

    /// The three checks the CPython `validate_world` performs, in its order.
    ///
    /// Deliberately no stricter: a world the reference accepts must load here, or the two
    /// implementations disagree about which worlds exist.
    pub fn validate_reference_compat(&self) -> Result<(), WorldError> {
        let mut seen_facts = BTreeSet::new();
        for fact in &self.facts {
            if !seen_facts.insert(fact.id.as_str()) {
                return Err(WorldError::DuplicateFactId(fact.id.as_str().to_string()));
            }
        }

        let mut seen_factors = BTreeSet::new();
        for factor in &self.factors {
            if !seen_factors.insert(factor.id.as_str()) {
                return Err(WorldError::DuplicateFactorId(factor.id.as_str().to_string()));
            }
        }

        let provided: BTreeSet<&str> = self.facts.iter().map(|f| f.provides.as_str()).collect();
        let produced: BTreeSet<&str> = self
            .factors
            .iter()
            .flat_map(|f| f.outputs.iter().map(|o| o.as_str()))
            .collect();

        for factor in &self.factors {
            let missing: Vec<String> = factor
                .inputs
                .iter()
                .filter(|input| {
                    !provided.contains(input.as_str()) && !produced.contains(input.as_str())
                })
                .map(|input| input.as_str().to_string())
                .collect();
            if !missing.is_empty() {
                return Err(WorldError::UnknownFactorInputs {
                    factor: factor.id.as_str().to_string(),
                    missing,
                });
            }
        }

        Ok(())
    }

    pub fn index(&self) -> &WorldIndex {
        &self.index
    }

    pub fn raw(&self) -> &Value {
        &self.raw
    }

    /// Hash of the canonical world document.
    ///
    /// Memoised, which is sound because the digest is a pure function of `raw` and `raw` is
    /// immutable after [`World::from_json`] — there is no accessor that hands out a mutable
    /// reference to it. The first caller pays; every later caller reads the same bytes.
    pub fn content_hash(&self) -> ContentHash {
        self.digest
            .get_or_init(|| {
                ContentHash::of_value(&self.raw).expect("world was parsed from finite JSON")
            })
            .clone()
    }

    pub fn fact(&self, id: &str) -> Option<&Fact> {
        self.index.fact_position(id).map(|p| &self.facts[p])
    }

    pub fn factor(&self, id: &str) -> Option<&Factor> {
        self.index.factor_position(id).map(|p| &self.factors[p])
    }

    pub fn fact_providing(&self, variable: &str) -> Option<&Fact> {
        self.index
            .fact_position_for_variable(variable)
            .map(|p| &self.facts[p])
    }

    pub fn producers_of(&self, variable: &str) -> impl Iterator<Item = &Factor> {
        self.index.producers(variable).iter().map(|p| &self.factors[*p])
    }

    /// Variables that some event governs. A variable in this set is readable only once its
    /// event has become available; a variable outside it is always readable.
    pub fn event_managed_variables(&self) -> BTreeSet<&str> {
        self.events
            .iter()
            .flat_map(|e| e.produces.iter().map(|v| v.as_str()))
            .collect()
    }
}

fn parse_seq<T>(
    value: Option<&Value>,
    field: &'static str,
    parse: impl Fn(&Value) -> Result<T, WorldError>,
) -> Result<Vec<T>, WorldError> {
    match value {
        None => Err(WorldError::MissingField { field, subject: "world".into() }),
        Some(Value::Array(items)) => items.iter().map(parse).collect(),
        Some(_) => Err(WorldError::WrongType {
            field,
            subject: "world".into(),
            expected: "array",
        }),
    }
}
