//! Generated items, their lineage, and the count you are not allowed to publish alone.
//!
//! Blueprint 35.09 and 35.10. A [`GeneratedItem`] is a benchmark instance reduced to the six fields
//! that scale accounting actually needs: who it came from, what transformation produced it, what it
//! tests, and what it says. Everything else about an instance lives in `bioprism-mutation` and
//! `bioprism-world`; carrying it here would make the accounting depend on the world schema.
//!
//! ## Lineage is a chain, not a pointer
//!
//! `derived_from` names the *immediate* predecessor, and a family is defined by the **root** of
//! that chain. This matters for 35.11: a mutation of a mutation of a parent world shares a parent
//! with its cousin, and the two can look nothing alike. Surface similarity would not catch it;
//! [`Corpus::root_of`] does, by walking to the root with cycle detection.
//!
//! ## Content digest
//!
//! [`content_digest`] hashes `facts`, `factors` and `events` and deliberately **not** `world_id` or
//! `description`. This is the same rule as `bioprism_mutation::lineage::content_digest`, restated
//! rather than imported because this crate does not depend on `bioprism-mutation`; the property it
//! protects is asserted directly in `tests/effective_size.rs`, so the two definitions cannot drift
//! silently. The rule exists because a generator that could defeat deduplication by renaming its
//! output could inflate the instance count without adding a single benchmark.

use crate::error::ScaleError;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

/// Digest of a world's semantic content, ignoring its label.
///
/// Restates `bioprism_mutation::lineage::content_digest`. Two worlds with identical facts, factors
/// and events are the same benchmark however they are named.
pub fn content_digest(world: &Value) -> Result<String, ScaleError> {
    let content = serde_json::json!({
        "facts": world.get("facts").cloned().unwrap_or(Value::Null),
        "factors": world.get("factors").cloned().unwrap_or(Value::Null),
        "events": world.get("events").cloned().unwrap_or(Value::Null),
    });
    ContentHash::of_value(&content)
        .map(|digest| digest.as_str().to_string())
        .map_err(|error| ScaleError::Canonical(error.to_string()))
}

/// One generated benchmark instance as the factory accounting sees it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GeneratedItem {
    pub id: String,
    /// Immediate predecessor. `None` means this item *is* a parent world.
    pub derived_from: Option<String>,
    /// Mutation family applied to produce this item. Parent worlds use `"parent"`.
    pub family: String,
    /// Oracle signature — what this instance actually tests, not how it is worded.
    pub signature: String,
    /// Digest of semantic content, excluding labels. See [`content_digest`].
    pub content_digest: String,
    /// The parent decision this instance exercises. 35.16 counts decisions, not worlds, as the
    /// unit that mutation families multiply against.
    pub decision: String,
}

impl GeneratedItem {
    /// A root parent world: no predecessor, family `"parent"`.
    pub fn parent(id: impl Into<String>, decision: impl Into<String>, digest: impl Into<String>) -> Self {
        let id = id.into();
        GeneratedItem {
            id,
            derived_from: None,
            family: "parent".into(),
            signature: "parent".into(),
            content_digest: digest.into(),
            decision: decision.into(),
        }
    }

    /// A descendant produced by applying `family` to `parent`.
    pub fn descendant(
        id: impl Into<String>,
        parent: impl Into<String>,
        family: impl Into<String>,
        signature: impl Into<String>,
        digest: impl Into<String>,
        decision: impl Into<String>,
    ) -> Self {
        GeneratedItem {
            id: id.into(),
            derived_from: Some(parent.into()),
            family: family.into(),
            signature: signature.into(),
            content_digest: digest.into(),
            decision: decision.into(),
        }
    }
}

/// The instance count, in the one form that cannot be published on its own.
///
/// `NominalCount` implements neither `Serialize` nor `Deserialize`, and it is the only thing
/// [`Corpus::nominal`] returns. The nominal number reaches a report exclusively through
/// [`crate::effective::EffectiveSize`], which carries the effective count and the inflation ratio
/// in the same object. That is the type-level form of "instance count is not benchmark count":
/// there is no `to_json` that emits a million and stops.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct NominalCount(usize);

impl NominalCount {
    pub fn get(self) -> usize {
        self.0
    }
}

/// An enumerable set of generated items with resolvable lineage.
#[derive(Debug, Clone, Default)]
pub struct Corpus {
    items: BTreeMap<String, GeneratedItem>,
}

impl Corpus {
    pub fn new() -> Self {
        Corpus::default()
    }

    /// Adds an item, refusing a duplicate id.
    pub fn insert(&mut self, item: GeneratedItem) -> Result<(), ScaleError> {
        if self.items.contains_key(&item.id) {
            return Err(ScaleError::DuplicateItem(item.id));
        }
        self.items.insert(item.id.clone(), item);
        Ok(())
    }

    pub fn get(&self, id: &str) -> Option<&GeneratedItem> {
        self.items.get(id)
    }

    /// Iterates in id order, so every downstream count is deterministic.
    pub fn iter(&self) -> impl Iterator<Item = &GeneratedItem> {
        self.items.values()
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// The instance count. See [`NominalCount`] for why this is not a `usize`.
    pub fn nominal(&self) -> NominalCount {
        NominalCount(self.items.len())
    }

    /// The root of `id`'s lineage chain — the parent world its family is named by.
    ///
    /// Walks predecessors rather than trusting a stored root field, because a stored root is a
    /// second source of truth that a generator can get wrong. Cycles and dangling parents are
    /// typed errors, not silent truncation.
    pub fn root_of(&self, id: &str) -> Result<&GeneratedItem, ScaleError> {
        let mut current = self
            .items
            .get(id)
            .ok_or_else(|| ScaleError::UnknownItem(id.to_string()))?;
        let mut seen: BTreeSet<&str> = BTreeSet::new();
        while let Some(parent) = current.derived_from.as_deref() {
            if !seen.insert(current.id.as_str()) {
                return Err(ScaleError::LineageCycle(current.id.clone()));
            }
            current = self.items.get(parent).ok_or_else(|| ScaleError::DanglingParent {
                item: current.id.clone(),
                parent: parent.to_string(),
            })?;
            if seen.contains(current.id.as_str()) {
                return Err(ScaleError::LineageCycle(current.id.clone()));
            }
        }
        Ok(current)
    }

    /// Every item grouped by the id of its root parent world. This is the family partition that
    /// 35.11's hidden-family split must never cut across.
    pub fn families(&self) -> Result<BTreeMap<String, Vec<String>>, ScaleError> {
        let mut families: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for item in self.items.values() {
            let root = self.root_of(&item.id)?.id.clone();
            families.entry(root).or_default().push(item.id.clone());
        }
        Ok(families)
    }

    /// The family `id` belongs to, named by its root parent world.
    pub fn family_of(&self, id: &str) -> Result<String, ScaleError> {
        Ok(self.root_of(id)?.id.clone())
    }
}

impl Extend<GeneratedItem> for Corpus {
    /// Convenience for building fixtures. Silently overwrites on a duplicate id, so prefer
    /// [`Corpus::insert`] anywhere a duplicate would be a bug worth hearing about.
    fn extend<T: IntoIterator<Item = GeneratedItem>>(&mut self, iter: T) {
        for item in iter {
            self.items.insert(item.id.clone(), item);
        }
    }
}
