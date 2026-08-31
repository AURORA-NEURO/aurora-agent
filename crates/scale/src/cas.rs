//! Content-addressed objects, delta snapshots, and a replay cache that proves its hits.
//!
//! Blueprint 35.12: "make millions of forkable states feasible through immutable objects,
//! copy-on-write deltas, and safe cache keys."
//!
//! ## A cache hit must be provably the same computation
//!
//! This is the part of 35.12 worth building carefully. 35.10's scale constraint says expensive
//! preprocessing is "cached only under complete semantic keys", and the failure mode it guards
//! against is subtle: a cache keyed on the *prompt* returns a stale answer when the oracle changed;
//! a cache keyed on a *digest* returns a wrong answer when two different computations were reduced
//! to the same string before hashing.
//!
//! So [`ComputationKey`] has four private components — inputs, code, environment, oracle — and:
//!
//! - it cannot be constructed with any component empty ([`CacheError::IncompleteKey`]);
//! - there is no `from_digest`, so no caller can present a key they did not build from parts;
//! - [`ReplayCache::lookup`] locates a candidate by digest and then compares **component by
//!   component**, returning [`CacheError::KeyCollision`] naming the differing component rather than
//!   serving the entry;
//! - a hit carries a [`HitProof`] enumerating the four components that matched.
//!
//! A hit is therefore evidence, not a coincidence of hashing. That is the difference between "the
//! same key" and "the same computation", and it is the only reason a replay cache is safe to put in
//! front of an oracle.
//!
//! ## Snapshots
//!
//! [`Snapshot`] is copy-on-write: a base plus added and removed entries. [`ObjectStore::materialize`]
//! replays the chain from its root, with cycle detection, because a delta chain built by a
//! distributed generator is exactly the structure that acquires a loop and then hangs.
//!
//! ## Not implemented
//!
//! In-memory. No garbage collection, no chunk-level deduplication within an object, no environment
//! layers, no WorldTape event ledger — all four are 35.12 components and all four need a durable
//! backend this crate does not have. [`put_world_release`] is the one bridge to real storage: it
//! builds a `bioprism-store` indexed store on disk and content-addresses its manifest, so a world
//! release is identified by the same digest the compile path already trusts.

use crate::error::CacheError;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// An immutable byte object, addressed by the hash of its content.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Object {
    pub address: String,
    pub bytes: Vec<u8>,
}

/// Copy-on-write difference from a base snapshot.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Delta {
    /// Logical name → object address.
    pub added: BTreeMap<String, String>,
    /// Names removed relative to the base.
    pub removed: BTreeSet<String>,
}

impl Delta {
    pub fn new() -> Self {
        Delta::default()
    }

    pub fn add(mut self, name: impl Into<String>, address: impl Into<String>) -> Self {
        self.added.insert(name.into(), address.into());
        self
    }

    pub fn remove(mut self, name: impl Into<String>) -> Self {
        self.removed.insert(name.into());
        self
    }

    pub fn entry_count(&self) -> usize {
        self.added.len() + self.removed.len()
    }
}

/// A named state, stored as a delta against an optional base.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Snapshot {
    pub id: String,
    pub base: Option<String>,
    pub delta: Delta,
}

/// In-memory content-addressed store with delta snapshots.
#[derive(Debug, Default)]
pub struct ObjectStore {
    objects: BTreeMap<String, Vec<u8>>,
    snapshots: BTreeMap<String, Snapshot>,
}

impl ObjectStore {
    pub fn new() -> Self {
        ObjectStore::default()
    }

    /// Stores bytes and returns their address. Storing identical bytes twice is a no-op, which is
    /// where the deduplication comes from.
    pub fn put(&mut self, bytes: impl Into<Vec<u8>>) -> String {
        let bytes = bytes.into();
        let address = ContentHash::of_bytes(&bytes).as_str().to_string();
        self.objects.entry(address.clone()).or_insert(bytes);
        address
    }

    pub fn get(&self, address: &str) -> Result<&[u8], CacheError> {
        self.objects
            .get(address)
            .map(Vec::as_slice)
            .ok_or_else(|| CacheError::MissingObject(address.to_string()))
    }

    pub fn object_count(&self) -> usize {
        self.objects.len()
    }

    /// Rehashes every object and reports the first that does not match its address.
    ///
    /// 35.12 lists corruption detection as an operational metric; content addressing makes it a
    /// loop rather than a checksum sidecar.
    pub fn verify(&self) -> Result<(), CacheError> {
        for (address, bytes) in &self.objects {
            let actual = ContentHash::of_bytes(bytes).as_str().to_string();
            if &actual != address {
                return Err(CacheError::CorruptObject {
                    id: address.clone(),
                    actual,
                });
            }
        }
        Ok(())
    }

    pub fn snapshot(
        &mut self,
        id: impl Into<String>,
        base: Option<String>,
        delta: Delta,
    ) -> Result<&Snapshot, CacheError> {
        let id = id.into();
        if let Some(base) = &base {
            if !self.snapshots.contains_key(base) {
                return Err(CacheError::MissingSnapshot(base.clone()));
            }
        }
        self.snapshots.insert(
            id.clone(),
            Snapshot {
                id: id.clone(),
                base,
                delta,
            },
        );
        self.snapshots
            .get(&id)
            .ok_or(CacheError::MissingSnapshot(id))
    }

    /// Replays a snapshot's chain from the root, producing the full name → address map.
    pub fn materialize(&self, id: &str) -> Result<BTreeMap<String, String>, CacheError> {
        let mut chain = Vec::new();
        let mut seen: BTreeSet<String> = BTreeSet::new();
        let mut cursor = Some(id.to_string());
        while let Some(current) = cursor {
            if !seen.insert(current.clone()) {
                return Err(CacheError::SnapshotCycle(current));
            }
            let snapshot = self
                .snapshots
                .get(&current)
                .ok_or_else(|| CacheError::MissingSnapshot(current.clone()))?;
            cursor = snapshot.base.clone();
            chain.push(snapshot);
        }

        let mut state: BTreeMap<String, String> = BTreeMap::new();
        for snapshot in chain.into_iter().rev() {
            for name in &snapshot.delta.removed {
                state.remove(name);
            }
            for (name, address) in &snapshot.delta.added {
                state.insert(name.clone(), address.clone());
            }
        }
        Ok(state)
    }

    /// Entries physically stored along the chain, against entries a full snapshot would carry.
    ///
    /// The ratio a caller reports as the delta saving. Counts entries rather than bytes because
    /// unchanged entries share an object address and so cost nothing in the object store either.
    pub fn delta_saving(&self, id: &str) -> Result<(usize, usize), CacheError> {
        let materialized = self.materialize(id)?.len();
        let mut stored = 0usize;
        let mut cursor = Some(id.to_string());
        let mut seen: BTreeSet<String> = BTreeSet::new();
        while let Some(current) = cursor {
            if !seen.insert(current.clone()) {
                return Err(CacheError::SnapshotCycle(current));
            }
            let snapshot = self
                .snapshots
                .get(&current)
                .ok_or_else(|| CacheError::MissingSnapshot(current.clone()))?;
            stored += snapshot.delta.entry_count();
            cursor = snapshot.base.clone();
        }
        Ok((stored, materialized))
    }
}

/// Parts of a [`ComputationKey`], used only so deserialization cannot bypass validation.
#[derive(Debug, Clone, Deserialize)]
pub struct ComputationKeyParts {
    pub inputs: String,
    pub code: String,
    pub environment: String,
    pub oracle: String,
}

/// A complete semantic cache key: the four things that determine a computation's result.
///
/// Fields are private and there is no constructor from a digest, so a key always has all four
/// components available for the component-by-component comparison [`ReplayCache::lookup`] performs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "ComputationKeyParts")]
pub struct ComputationKey {
    inputs: String,
    code: String,
    environment: String,
    oracle: String,
}

impl ComputationKey {
    /// Every component is required. An empty one is [`CacheError::IncompleteKey`], because a key
    /// that omits the environment is the key that serves last month's answer.
    pub fn new(
        inputs: impl Into<String>,
        code: impl Into<String>,
        environment: impl Into<String>,
        oracle: impl Into<String>,
    ) -> Result<Self, CacheError> {
        let key = ComputationKey {
            inputs: inputs.into(),
            code: code.into(),
            environment: environment.into(),
            oracle: oracle.into(),
        };
        for (name, value) in key.components() {
            if value.is_empty() {
                return Err(CacheError::IncompleteKey(name));
            }
        }
        Ok(key)
    }

    pub fn components(&self) -> [(&'static str, &str); 4] {
        [
            ("inputs", self.inputs.as_str()),
            ("code", self.code.as_str()),
            ("environment", self.environment.as_str()),
            ("oracle", self.oracle.as_str()),
        ]
    }

    /// The lookup address. Never sufficient on its own — see [`ReplayCache::lookup`].
    pub fn digest(&self) -> String {
        let document = serde_json::json!({
            "inputs": self.inputs,
            "code": self.code,
            "environment": self.environment,
            "oracle": self.oracle,
        });
        ContentHash::of_value(&document)
            .map(|hash| hash.as_str().to_string())
            .unwrap_or_default()
    }
}

impl TryFrom<ComputationKeyParts> for ComputationKey {
    type Error = CacheError;

    fn try_from(parts: ComputationKeyParts) -> Result<Self, Self::Error> {
        ComputationKey::new(parts.inputs, parts.code, parts.environment, parts.oracle)
    }
}

/// Why a hit was served: the components that matched, by name and value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HitProof {
    pub digest: String,
    pub matched: Vec<(String, String)>,
}

/// A served entry with its proof.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CacheHit {
    pub value: Value,
    pub proof: HitProof,
}

/// A replay cache that refuses to serve an entry it cannot prove identical.
#[derive(Debug, Default)]
pub struct ReplayCache {
    entries: BTreeMap<String, (ComputationKey, Value)>,
    hits: usize,
    misses: usize,
}

impl ReplayCache {
    pub fn new() -> Self {
        ReplayCache::default()
    }

    pub fn insert(&mut self, key: ComputationKey, value: Value) {
        self.entries.insert(key.digest(), (key, value));
    }

    /// Rebuilds a cache from a persisted index, keeping the persisted digest as written.
    ///
    /// A persisted index is not necessarily consistent with this build's digest function — a
    /// changed canonicalization, a changed key schema, a corrupted file. Recomputing the digest on
    /// restore would paper over exactly that, so the persisted address is kept and the
    /// component-by-component check in [`ReplayCache::lookup`] catches the disagreement as a
    /// [`CacheError::KeyCollision`]. This is the reachable path to that branch; a genuine SHA-256
    /// collision is the other one.
    pub fn restore(entries: impl IntoIterator<Item = (String, ComputationKey, Value)>) -> Self {
        let mut cache = ReplayCache::new();
        for (digest, key, value) in entries {
            cache.entries.insert(digest, (key, value));
        }
        cache
    }

    /// Looks up `key`, comparing every component of any candidate before serving it.
    ///
    /// `Ok(None)` is a miss. `Err(KeyCollision)` means two different computations reduced to one
    /// digest — a bug or an attack, and either way not something to serve silently.
    pub fn lookup(&mut self, key: &ComputationKey) -> Result<Option<CacheHit>, CacheError> {
        let digest = key.digest();
        let Some((stored, value)) = self.entries.get(&digest) else {
            self.misses += 1;
            return Ok(None);
        };
        for ((name, stored_value), (_, presented)) in
            stored.components().into_iter().zip(key.components())
        {
            if stored_value != presented {
                return Err(CacheError::KeyCollision {
                    key: digest,
                    component: name,
                    stored: stored_value.to_string(),
                    presented: presented.to_string(),
                });
            }
        }
        self.hits += 1;
        Ok(Some(CacheHit {
            value: value.clone(),
            proof: HitProof {
                digest,
                matched: key
                    .components()
                    .into_iter()
                    .map(|(name, value)| (name.to_string(), value.to_string()))
                    .collect(),
            },
        }))
    }

    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            self.hits as f64 / total as f64
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// A world release identified by the manifest of its indexed store.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorldRelease {
    /// Address of the manifest bytes in the object store.
    pub manifest_address: String,
    /// The digest `bioprism-store` computed over the world itself.
    pub world_sha256: String,
    pub total_facts: usize,
    pub total_factors: usize,
}

/// Builds a `bioprism-store` indexed store for `world` under `directory` and content-addresses its
/// manifest.
///
/// The bridge from this crate's in-memory accounting to the storage layer that actually made the
/// compile path output-sensitive. The manifest, not the world document, is what gets addressed:
/// it already carries the world digest and the aggregates, so a release is identified by the same
/// bytes the compiler trusts.
pub fn put_world_release(
    store: &mut ObjectStore,
    world: &Value,
    directory: &Path,
) -> Result<WorldRelease, CacheError> {
    let manifest = bioprism_store::build(world, directory)
        .map_err(|error| CacheError::Store(error.to_string()))?;
    let bytes =
        serde_json::to_vec(&manifest).map_err(|error| CacheError::Store(error.to_string()))?;
    Ok(WorldRelease {
        manifest_address: store.put(bytes),
        world_sha256: manifest.world_sha256,
        total_facts: manifest.total_facts,
        total_factors: manifest.total_factors,
    })
}
