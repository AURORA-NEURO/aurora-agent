//! A world read lazily from an indexed store.
//!
//! Implements [`WorldSource`] with point lookups only. No method here reads the corpus: aggregate
//! answers come from the manifest, and record answers come from a binary search over an on-disk
//! index. Compiling against a `LazyWorld` therefore costs what the *compiled region* costs, which
//! is what blueprint 43.34 asks for and what the eager path could not deliver.
//!
//! Logical semantics are identical to the eager path by construction: the records returned are the
//! same documents, parsed by the same code. `tests/store_parity.rs` asserts that both produce the
//! same certificate bytes.

use crate::build::{StoreManifest, STORE_SCHEMA_VERSION};
use crate::error::StoreError;
use crate::sorted_index::SortedIndex;
use bioprism_ids::{ContentHash, WorldId};
use bioprism_world::{CausalEvent, Fact, Factor, WorldSource, WorldSourceError};
use serde_json::Value;
use std::collections::BTreeSet;
use std::path::Path;

pub struct LazyWorld {
    manifest: StoreManifest,
    facts: SortedIndex,
    variables: SortedIndex,
    factors: SortedIndex,
    producers: SortedIndex,
    tags: SortedIndex,
    shadowed: SortedIndex,
    events: Vec<CausalEvent>,
}

impl LazyWorld {
    pub fn open(directory: &Path) -> Result<Self, StoreError> {
        let manifest: StoreManifest =
            serde_json::from_str(&std::fs::read_to_string(directory.join("manifest.json"))?)?;
        if manifest.schema_version != STORE_SCHEMA_VERSION {
            return Err(StoreError::UnsupportedSchema {
                expected: STORE_SCHEMA_VERSION,
                actual: manifest.schema_version,
            });
        }
        WorldId::parse(manifest.world_id.clone()).map_err(|_| StoreError::MalformedWorld)?;
        ContentHash::parse(manifest.world_sha256.clone())
            .map_err(|_| StoreError::MalformedWorld)?;
        if manifest
            .tag_counts
            .values()
            .any(|count| *count == 0 || *count > manifest.total_facts)
        {
            return Err(StoreError::MalformedWorld);
        }

        let events = manifest
            .events
            .iter()
            .map(CausalEvent::from_json)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| StoreError::MalformedWorld)?;

        let facts = SortedIndex::open(directory, "facts")?;
        let variables = SortedIndex::open(directory, "variables")?;
        let factors = SortedIndex::open(directory, "factors")?;
        let producers = SortedIndex::open(directory, "producers")?;
        let tags = SortedIndex::open(directory, "tags")?;
        let shadowed = SortedIndex::open(directory, "shadowed")?;
        if facts.len() != manifest.total_facts || factors.len() != manifest.total_factors {
            return Err(StoreError::MalformedWorld);
        }

        Ok(LazyWorld {
            facts,
            variables,
            factors,
            producers,
            tags,
            shadowed,
            events,
            manifest,
        })
    }

    pub fn manifest(&self) -> &StoreManifest {
        &self.manifest
    }

    fn lookup_json(
        index: &SortedIndex,
        name: &str,
        key: &str,
    ) -> Result<Option<Value>, WorldSourceError> {
        let Some(text) = index.get(key).map_err(source_error)? else {
            return Ok(None);
        };
        serde_json::from_str(&text).map(Some).map_err(|error| {
            WorldSourceError::Corrupt(format!("{name} index value for {key:?}: {error}"))
        })
    }

    fn lookup_ids(
        index: &SortedIndex,
        name: &str,
        key: &str,
    ) -> Result<Vec<String>, WorldSourceError> {
        let Some(value) = Self::lookup_json(index, name, key)? else {
            return Ok(Vec::new());
        };
        let ids: Vec<String> = serde_json::from_value(value).map_err(|error| {
            WorldSourceError::Corrupt(format!("{name} index ids for {key:?}: {error}"))
        })?;
        Ok(ids)
    }
}

fn source_error(error: StoreError) -> WorldSourceError {
    match error {
        StoreError::Io(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => {
            WorldSourceError::Corrupt(error.to_string())
        }
        StoreError::Io(error) => WorldSourceError::Unavailable(error.to_string()),
        other => WorldSourceError::Corrupt(other.to_string()),
    }
}

impl WorldSource for LazyWorld {
    fn world_id(&self) -> &str {
        &self.manifest.world_id
    }

    fn world_digest(&self) -> ContentHash {
        ContentHash::parse(self.manifest.world_sha256.clone())
            .expect("manifest digest was written by the builder")
    }

    fn total_facts(&self) -> usize {
        self.manifest.total_facts
    }

    fn total_factors(&self) -> usize {
        self.manifest.total_factors
    }

    fn count_with_tag(&self, tag: &str) -> Result<usize, WorldSourceError> {
        let declared = self.manifest.tag_counts.get(tag).copied().unwrap_or(0);
        if declared > self.manifest.total_facts {
            return Err(WorldSourceError::Corrupt(format!(
                "manifest declares {declared} facts tagged {tag:?}, above the world total"
            )));
        }
        Ok(declared)
    }

    fn fact_ids_with_any_tag(
        &self,
        tags: &BTreeSet<String>,
    ) -> Result<BTreeSet<String>, WorldSourceError> {
        let mut fact_ids = BTreeSet::new();
        for tag in tags {
            let ids = Self::lookup_ids(&self.tags, "tags", tag)?;
            let unique: BTreeSet<&str> = ids.iter().map(String::as_str).collect();
            if unique.len() != ids.len() {
                return Err(WorldSourceError::Corrupt(format!(
                    "tags index repeats a fact id for {tag:?}"
                )));
            }
            let declared = self.manifest.tag_counts.get(tag).copied().unwrap_or(0);
            if declared != ids.len() {
                return Err(WorldSourceError::Corrupt(format!(
                    "manifest declares {declared} fact(s) tagged {tag:?}, but the index returns {}",
                    ids.len()
                )));
            }
            for id in ids {
                let fact = self.fact(&id)?.ok_or_else(|| {
                    WorldSourceError::Corrupt(format!(
                        "tags index names missing fact `{id}` for {tag:?}"
                    ))
                })?;
                if !fact.has_tag(tag) {
                    return Err(WorldSourceError::Corrupt(format!(
                        "tags index names fact `{id}` without tag {tag:?}"
                    )));
                }
                fact_ids.insert(id);
            }
        }
        Ok(fact_ids)
    }

    fn fact(&self, id: &str) -> Result<Option<Fact>, WorldSourceError> {
        let Some(raw) = Self::lookup_json(&self.facts, "facts", id)? else {
            return Ok(None);
        };
        let fact = Fact::from_json(&raw)
            .map_err(|error| WorldSourceError::Corrupt(format!("fact record {id:?}: {error}")))?;
        if fact.id.as_str() != id {
            return Err(WorldSourceError::Corrupt(format!(
                "facts index key {id:?} points to record `{}`",
                fact.id.as_str()
            )));
        }
        Ok(Some(fact))
    }

    fn fact_providing(&self, variable: &str) -> Result<Option<Fact>, WorldSourceError> {
        let Some(id) = self.variables.get(variable).map_err(source_error)? else {
            return Ok(None);
        };
        let fact = self.fact(&id)?.ok_or_else(|| {
            WorldSourceError::Corrupt(format!(
                "variables index names missing fact `{id}` for {variable:?}"
            ))
        })?;
        if fact.provides.as_str() != variable {
            return Err(WorldSourceError::Corrupt(format!(
                "variables index maps {variable:?} to fact `{id}` providing {:?}",
                fact.provides.as_str()
            )));
        }
        Ok(Some(fact))
    }

    fn shadowed_provider_ids(&self, variable: &str) -> Result<Vec<String>, WorldSourceError> {
        let ids = Self::lookup_ids(&self.shadowed, "shadowed", variable)?;
        let mut unique = BTreeSet::new();
        for id in &ids {
            if !unique.insert(id.as_str()) {
                return Err(WorldSourceError::Corrupt(format!(
                    "shadowed index repeats fact id `{id}` for {variable:?}"
                )));
            }
            let fact = self.fact(id)?.ok_or_else(|| {
                WorldSourceError::Corrupt(format!(
                    "shadowed index names missing fact `{id}` for {variable:?}"
                ))
            })?;
            if fact.provides.as_str() != variable {
                return Err(WorldSourceError::Corrupt(format!(
                    "shadowed index names fact `{id}` providing {:?} for {variable:?}",
                    fact.provides.as_str()
                )));
            }
        }
        Ok(ids)
    }

    fn factor(&self, id: &str) -> Result<Option<Factor>, WorldSourceError> {
        let Some(raw) = Self::lookup_json(&self.factors, "factors", id)? else {
            return Ok(None);
        };
        let factor = Factor::from_json(&raw)
            .map_err(|error| WorldSourceError::Corrupt(format!("factor record {id:?}: {error}")))?;
        if factor.id.as_str() != id {
            return Err(WorldSourceError::Corrupt(format!(
                "factors index key {id:?} points to record `{}`",
                factor.id.as_str()
            )));
        }
        Ok(Some(factor))
    }

    fn producer_ids(&self, variable: &str) -> Result<Vec<String>, WorldSourceError> {
        let ids = Self::lookup_ids(&self.producers, "producers", variable)?;
        for id in &ids {
            let factor = self.factor(id)?.ok_or_else(|| {
                WorldSourceError::Corrupt(format!(
                    "producers index names missing factor `{id}` for {variable:?}"
                ))
            })?;
            if !factor
                .outputs
                .iter()
                .any(|output| output.as_str() == variable)
            {
                return Err(WorldSourceError::Corrupt(format!(
                    "producers index names factor `{id}` without output {variable:?}"
                )));
            }
        }
        Ok(ids)
    }

    fn events(&self) -> Vec<CausalEvent> {
        self.events.clone()
    }
}
