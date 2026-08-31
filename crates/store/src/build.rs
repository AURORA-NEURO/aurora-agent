//! Building a store from a world document.
//!
//! This is the one place that reads the whole corpus, and it happens once per world release
//! rather than once per query. Everything the compiler later needs as an aggregate — total
//! counts, per-tag counts, the world digest — is computed here and recorded in the manifest, so
//! that no query has to recompute it by scanning.

use crate::error::StoreError;
use crate::sorted_index::SortedIndexWriter;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::Path;

/// `0.2` adds the `shadowed` index.
///
/// The version is checked at [`crate::LazyWorld::open`], so a `0.1` directory is refused rather
/// than opened with the new index missing. That refusal is the point: an absent `shadowed` index
/// is indistinguishable at read time from a world in which nothing is shadowed, and answering
/// "nothing is shadowed" from a file that was never written is exactly the silent-wrong-answer the
/// influence classification depends on not happening.
pub const STORE_SCHEMA_VERSION: &str = "bioprism-store/0.2";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreManifest {
    pub schema_version: String,
    pub world_id: String,
    pub world_sha256: String,
    pub total_facts: usize,
    pub total_factors: usize,
    /// Number of facts carrying each tag. Lets `count_with_tag` answer without touching the corpus.
    pub tag_counts: BTreeMap<String, usize>,
    /// The causal event structure, inlined because it is small and every compile needs all of it.
    pub events: Vec<Value>,
    pub description: Option<String>,
}

/// Writes an indexed store for `world` into `directory`.
pub fn build(world: &Value, directory: &Path) -> Result<StoreManifest, StoreError> {
    std::fs::create_dir_all(directory)?;

    let object = world.as_object().ok_or(StoreError::MalformedWorld)?;
    let facts = object
        .get("facts")
        .and_then(Value::as_array)
        .ok_or(StoreError::MalformedWorld)?;
    let factors = object
        .get("factors")
        .and_then(Value::as_array)
        .ok_or(StoreError::MalformedWorld)?;
    let events = object
        .get("events")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    let mut fact_records = SortedIndexWriter::new();
    let mut variable_records = SortedIndexWriter::new();
    let mut tag_members: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut tag_counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut providers_by_variable: BTreeMap<String, Vec<String>> = BTreeMap::new();

    for fact in facts {
        let id = fact
            .get("id")
            .and_then(Value::as_str)
            .ok_or(StoreError::MalformedWorld)?;
        let provides = fact
            .get("provides")
            .and_then(Value::as_str)
            .ok_or(StoreError::MalformedWorld)?;

        fact_records.insert(id, serde_json::to_string(fact)?);
        variable_records.insert(provides, id);
        providers_by_variable
            .entry(provides.to_string())
            .or_default()
            .push(id.to_string());

        if let Some(tags) = fact.get("tags").and_then(Value::as_array) {
            for tag in tags.iter().filter_map(Value::as_str) {
                *tag_counts.entry(tag.to_string()).or_default() += 1;
                tag_members
                    .entry(tag.to_string())
                    .or_default()
                    .push(id.to_string());
            }
        }
    }

    let mut factor_records = SortedIndexWriter::new();
    let mut producers: BTreeMap<String, Vec<String>> = BTreeMap::new();

    for factor in factors {
        let id = factor
            .get("id")
            .and_then(Value::as_str)
            .ok_or(StoreError::MalformedWorld)?;
        factor_records.insert(id, serde_json::to_string(factor)?);

        if let Some(outputs) = factor.get("outputs").and_then(Value::as_array) {
            for output in outputs.iter().filter_map(Value::as_str) {
                producers
                    .entry(output.to_string())
                    .or_default()
                    .push(id.to_string());
            }
        }
    }

    let mut producer_records = SortedIndexWriter::new();
    for (variable, ids) in producers {
        producer_records.insert(variable, serde_json::to_string(&ids)?);
    }

    let mut tag_records = SortedIndexWriter::new();
    for (tag, ids) in tag_members {
        tag_records.insert(tag, serde_json::to_string(&ids)?);
    }

    // The `variables` index keeps only the winner of a shadowing race, matching the reference
    // runtime's dict semantics. The losers are written here instead of being discarded, because a
    // compiler that cannot see them cannot tell a shadowed omission from an unreachable one.
    let mut shadowed_records = SortedIndexWriter::new();
    for (variable, ids) in &providers_by_variable {
        if ids.len() > 1 {
            let shadowed = &ids[..ids.len() - 1];
            shadowed_records.insert(variable, serde_json::to_string(shadowed)?);
        }
    }

    fact_records.finish(directory, "facts")?;
    variable_records.finish(directory, "variables")?;
    factor_records.finish(directory, "factors")?;
    producer_records.finish(directory, "producers")?;
    tag_records.finish(directory, "tags")?;
    shadowed_records.finish(directory, "shadowed")?;

    let manifest = StoreManifest {
        schema_version: STORE_SCHEMA_VERSION.to_string(),
        world_id: object
            .get("world_id")
            .and_then(Value::as_str)
            .ok_or(StoreError::MalformedWorld)?
            .to_string(),
        world_sha256: ContentHash::of_value(world)
            .map_err(|_| StoreError::MalformedWorld)?
            .as_str()
            .to_string(),
        total_facts: facts.len(),
        total_factors: factors.len(),
        tag_counts,
        events,
        description: object
            .get("description")
            .and_then(Value::as_str)
            .map(str::to_string),
    };

    std::fs::write(
        directory.join("manifest.json"),
        serde_json::to_string_pretty(&manifest)?,
    )?;

    Ok(manifest)
}
