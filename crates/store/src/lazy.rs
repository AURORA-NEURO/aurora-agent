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
use bioprism_ids::ContentHash;
use bioprism_world::{CausalEvent, Fact, Factor, WorldSource};
use serde_json::Value;
use std::collections::BTreeSet;
use std::path::Path;

pub struct LazyWorld {
    manifest: StoreManifest,
    world_digest: ContentHash,
    facts: SortedIndex,
    variables: SortedIndex,
    factors: SortedIndex,
    producers: SortedIndex,
    tags: SortedIndex,
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
        let world_digest = ContentHash::parse(manifest.world_sha256.clone())
            .map_err(|_| StoreError::MalformedWorld)?;

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
        if facts.len() != manifest.total_facts {
            return Err(StoreError::CorruptIndex(format!(
                "facts index contains {} records but the manifest declares {}",
                facts.len(),
                manifest.total_facts
            )));
        }
        if factors.len() != manifest.total_factors {
            return Err(StoreError::CorruptIndex(format!(
                "factors index contains {} records but the manifest declares {}",
                factors.len(),
                manifest.total_factors
            )));
        }
        if variables.len() > facts.len() || tags.len() != manifest.tag_counts.len() {
            return Err(StoreError::CorruptIndex(
                "manifest and derived index cardinalities disagree".into(),
            ));
        }
        for (tag, expected_count) in &manifest.tag_counts {
            let Some(raw_ids) = tags.get(tag)? else {
                return Err(StoreError::CorruptIndex(format!(
                    "manifest declares tag {tag:?} but its index has no record"
                )));
            };
            let ids: Vec<String> = serde_json::from_str(&raw_ids).map_err(|_| {
                StoreError::CorruptIndex(format!("tag index record {tag:?} is not a string list"))
            })?;
            if ids.len() != *expected_count {
                return Err(StoreError::CorruptIndex(format!(
                    "tag {tag:?} has {} indexed members but the manifest declares {}",
                    ids.len(),
                    expected_count
                )));
            }
        }

        Ok(LazyWorld {
            facts,
            variables,
            factors,
            producers,
            tags,
            events,
            world_digest,
            manifest,
        })
    }

    pub fn manifest(&self) -> &StoreManifest {
        &self.manifest
    }

    fn lookup_json(index: &SortedIndex, key: &str) -> Option<Value> {
        index
            .get(key)
            .ok()
            .flatten()
            .and_then(|text| serde_json::from_str(&text).ok())
    }

    fn lookup_ids(index: &SortedIndex, key: &str) -> Vec<String> {
        Self::lookup_json(index, key)
            .and_then(|value| serde_json::from_value::<Vec<String>>(value).ok())
            .unwrap_or_default()
    }
}

impl WorldSource for LazyWorld {
    fn world_id(&self) -> &str {
        &self.manifest.world_id
    }

    fn world_digest(&self) -> ContentHash {
        self.world_digest.clone()
    }

    fn total_facts(&self) -> usize {
        self.manifest.total_facts
    }

    fn total_factors(&self) -> usize {
        self.manifest.total_factors
    }

    fn count_with_tag(&self, tag: &str) -> usize {
        self.manifest.tag_counts.get(tag).copied().unwrap_or(0)
    }

    fn fact_ids_with_any_tag(&self, tags: &BTreeSet<String>) -> BTreeSet<String> {
        tags.iter()
            .flat_map(|tag| Self::lookup_ids(&self.tags, tag))
            .collect()
    }

    fn fact(&self, id: &str) -> Option<Fact> {
        Self::lookup_json(&self.facts, id).and_then(|raw| Fact::from_json(&raw).ok())
    }

    fn fact_providing(&self, variable: &str) -> Option<Fact> {
        let id = self.variables.get(variable).ok().flatten()?;
        self.fact(&id)
    }

    fn factor(&self, id: &str) -> Option<Factor> {
        Self::lookup_json(&self.factors, id).and_then(|raw| Factor::from_json(&raw).ok())
    }

    fn producer_ids(&self, variable: &str) -> Vec<String> {
        Self::lookup_ids(&self.producers, variable)
    }

    fn events(&self) -> Vec<CausalEvent> {
        self.events.clone()
    }
}
