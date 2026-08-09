//! A cache where a hit is provably the same computation, and a miss says why.
//!
//! Blueprint 12.10 ("Cache and Prefix Reuse") states the requirement as *key completeness*:
//! "include all semantically relevant inputs: artifact digests, schemas, code, configuration,
//! model resolution, prompts, tools, permissions, seed, time/network fixture, platform,
//! evaluator, and policy". 12.08 adds that derived systems must never become canonical truth.
//! Together those say a cache is allowed to be fast but is not allowed to be a second, quieter
//! source of answers.
//!
//! `bioprism-scale`'s `ComputationKey` took the first step for its replay
//! cache: four private components, no constructor from a digest, and a lookup that compares
//! component by component *after* the digest matches. This module keeps that and pushes on it in
//! three places where a reproducibility platform still has room to lie.
//!
//! # 1. The component set is declared, not hardcoded
//!
//! A fixed four-field key is a bet that four things determine every result. [`KeySchema`] instead
//! names the components a particular computation depends on, and the schema's own digest is
//! folded into every key digest. The consequence is worth stating: **changing the schema cannot
//! produce a collision, only a miss.** Old entries live at addresses the new schema can never
//! compute, so there is no path where a key that gained a component silently matches an entry
//! written before that component existed. A cache that added `policy_version` to its key last
//! week does not have to be flushed; it simply stops hitting.
//!
//! # 2. The entry records which build produced it
//!
//! The key says *what* was computed. [`CodeIdentity`] says *which build did the computing*. Those
//! are different claims, and collapsing them costs one of two things: fold the build into the key
//! and every rebuild empties the cache; leave it out and a value computed by a since-fixed build
//! is served forever. So the schema declares a [`ReuseRule`]. [`ReuseRule::SameBuildOnly`] is the
//! default and refuses cross-build reuse; [`ReuseRule::AcrossBuilds`] permits it and is a
//! statement the caller is making — *this computation is reproducible across builds* — recorded
//! in the [`HitProof`] so a downstream reader can see the claim that was relied on.
//!
//! # 3. A miss carries its reason
//!
//! [`Lookup::Miss`] names why: no entry, a foreign schema, a cross-build refusal, or — the one
//! that matters — [`MissReason::UnprovenAfterPartialInvalidation`]. When
//! [`crate::invalidation::InvalidationPlan`] reports itself partial, [`Cache::apply`] marks every
//! entry in the unknown region unprovable, and those entries stop hitting until something proves
//! them again. This is the failure mode the module exists to make unreachable: *a cache that
//! quietly serves a stale entry after an incomplete invalidation.* Here an incomplete
//! invalidation costs hit rate, not correctness.
//!
//! `Err` is reserved for the alarming case. [`CacheError::KeyCollision`] means two different
//! computations reduced to one address, and it is never served and never downgraded to a miss.
//!
//! # Deliberately not implemented
//!
//! In-memory, single-process, no eviction (12.10's "cost-aware and pin-aware eviction" needs a
//! rebuild-cost model this crate has no source for), no tenancy or namespace isolation (12.10's
//! privacy section), no prefix or suffix reuse, no poison canaries, no sampled recomputation. The
//! values are `serde_json::Value` held in a map; nothing is persisted. [`Cache::restore`] is the
//! seam a durable index would attach to, and it is deliberately the one path that can surface a
//! [`CacheError::KeyCollision`] in ordinary operation.

use crate::epoch::Epoch;
use crate::error::CacheError;
use crate::invalidation::{DependencyDeclaration, InvalidationPlan};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

fn well_formed(field: &'static str, value: &str) -> Result<String, CacheError> {
    if value.trim().is_empty() || value.chars().any(char::is_control) {
        return Err(CacheError::MalformedField {
            field,
            value: value.to_string(),
        });
    }
    Ok(value.to_string())
}

/// The identity of the build that computed a value.
///
/// A digest of the compiled artifact, a commit plus toolchain, a container image — this crate
/// does not care which, only that it is a stable string the caller can produce again.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct CodeIdentity(String);

impl CodeIdentity {
    pub fn parse(value: impl Into<String>) -> Result<Self, CacheError> {
        well_formed("code identity", &value.into()).map(CodeIdentity)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for CodeIdentity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<CodeIdentity> for String {
    fn from(value: CodeIdentity) -> Self {
        value.0
    }
}

impl TryFrom<String> for CodeIdentity {
    type Error = CacheError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        CodeIdentity::parse(value)
    }
}

/// Whether a value may be reused across builds.
///
/// Not a performance knob. It is the caller's claim about determinism, and the wrong setting
/// fails in opposite directions: `SameBuildOnly` on a genuinely reproducible computation wastes
/// work every release, while `AcrossBuilds` on a computation that is *not* build-stable serves
/// last release's answer indefinitely. The conservative one is the default because the wasteful
/// failure is visible in a cost report and the other is visible in nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ReuseRule {
    /// A value may only be served to the build that produced it.
    #[default]
    SameBuildOnly,
    /// The caller declares this computation reproducible across builds; the producing build is
    /// still recorded and still reported in the hit proof.
    AcrossBuilds,
}

impl ReuseRule {
    pub fn name(self) -> &'static str {
        match self {
            ReuseRule::SameBuildOnly => "same-build-only",
            ReuseRule::AcrossBuilds => "across-builds",
        }
    }
}

/// The declared component set for a family of computations.
///
/// The schema is the place where "we thought about what determines this result" is written down,
/// and its digest travels in every key built from it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeySchema {
    name: String,
    components: BTreeSet<String>,
    reuse: ReuseRule,
}

impl KeySchema {
    /// Declares a schema. Refuses an empty component set: a schema with no components maps every
    /// computation in its family to one address, which is a cache that returns the first answer
    /// it ever saw.
    pub fn declare(
        name: impl Into<String>,
        components: impl IntoIterator<Item = impl Into<String>>,
        reuse: ReuseRule,
    ) -> Result<Self, CacheError> {
        let name = well_formed("schema name", &name.into())?;
        let mut declared = BTreeSet::new();
        for component in components {
            declared.insert(well_formed("component name", &component.into())?);
        }
        if declared.is_empty() {
            return Err(CacheError::SchemaWithoutComponents { schema: name });
        }
        Ok(KeySchema {
            name,
            components: declared,
            reuse,
        })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn components(&self) -> &BTreeSet<String> {
        &self.components
    }

    pub fn reuse(&self) -> ReuseRule {
        self.reuse
    }

    /// The schema's address. Folded into every key digest, which is what makes a schema change a
    /// miss rather than a collision.
    pub fn digest(&self) -> String {
        let body = serde_json::json!({
            "name": self.name,
            "components": self.components.iter().collect::<Vec<_>>(),
            "reuse": self.reuse.name(),
        });
        ContentHash::of_value(&body)
            .expect("schema body is strings, which always canonicalize")
            .as_str()
            .to_string()
    }
}

/// The wire shape of a key, used only so deserialization cannot bypass validation.
#[derive(Debug, Clone, Deserialize)]
pub struct ComputationKeyParts {
    pub schema_name: String,
    pub schema_digest: String,
    pub components: BTreeMap<String, String>,
}

/// A complete semantic key: which schema, and a value for every component it declares.
///
/// Fields are private, there is no constructor from a digest, and the `serde` path runs the same
/// validation as [`ComputationKey::build`]. A caller therefore cannot present a key it did not
/// assemble from parts, and every candidate the cache finds still has its components available
/// for comparison.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "ComputationKeyParts")]
pub struct ComputationKey {
    schema_name: String,
    schema_digest: String,
    components: BTreeMap<String, String>,
}

impl ComputationKey {
    /// Builds a key against a schema. Every declared component must be present and non-empty,
    /// and no undeclared component may be supplied.
    pub fn build(
        schema: &KeySchema,
        components: impl IntoIterator<Item = (impl Into<String>, impl Into<String>)>,
    ) -> Result<Self, CacheError> {
        let mut supplied: BTreeMap<String, String> = BTreeMap::new();
        for (name, value) in components {
            let name = name.into();
            let value = value.into();
            if !schema.components.contains(&name) {
                return Err(CacheError::UndeclaredComponent {
                    schema: schema.name.clone(),
                    component: name,
                });
            }
            if value.is_empty() {
                return Err(CacheError::EmptyComponent {
                    schema: schema.name.clone(),
                    component: name,
                });
            }
            supplied.insert(name, value);
        }
        for declared in &schema.components {
            if !supplied.contains_key(declared) {
                return Err(CacheError::IncompleteKey {
                    schema: schema.name.clone(),
                    component: declared.clone(),
                });
            }
        }
        Ok(ComputationKey {
            schema_name: schema.name.clone(),
            schema_digest: schema.digest(),
            components: supplied,
        })
    }

    pub fn schema_name(&self) -> &str {
        &self.schema_name
    }

    pub fn schema_digest(&self) -> &str {
        &self.schema_digest
    }

    pub fn components(&self) -> impl Iterator<Item = (&str, &str)> {
        self.components
            .iter()
            .map(|(name, value)| (name.as_str(), value.as_str()))
    }

    /// The lookup address. Never sufficient on its own — see [`Cache::lookup`].
    pub fn digest(&self) -> String {
        let body = serde_json::json!({
            "schema_digest": self.schema_digest,
            "schema_name": self.schema_name,
            "components": self.components,
        });
        ContentHash::of_value(&body)
            .expect("key body is strings, which always canonicalize")
            .as_str()
            .to_string()
    }

    /// Re-checks a key against the schema the cache actually holds.
    ///
    /// Run on every insert and lookup. A key that arrived over the wire carries a schema digest
    /// but not the schema, so this is where a deserialized key that names the right schema and
    /// the wrong components is caught.
    pub fn validate_against(&self, schema: &KeySchema) -> Result<(), CacheError> {
        if self.schema_digest != schema.digest() {
            return Err(CacheError::ForeignSchema {
                expected: schema.name.clone(),
                presented: self.schema_name.clone(),
            });
        }
        for name in self.components.keys() {
            if !schema.components.contains(name) {
                return Err(CacheError::UndeclaredComponent {
                    schema: schema.name.clone(),
                    component: name.clone(),
                });
            }
        }
        for declared in &schema.components {
            match self.components.get(declared) {
                None => {
                    return Err(CacheError::IncompleteKey {
                        schema: schema.name.clone(),
                        component: declared.clone(),
                    })
                }
                Some(value) if value.is_empty() => {
                    return Err(CacheError::EmptyComponent {
                        schema: schema.name.clone(),
                        component: declared.clone(),
                    })
                }
                Some(_) => {}
            }
        }
        Ok(())
    }
}

impl TryFrom<ComputationKeyParts> for ComputationKey {
    type Error = CacheError;

    fn try_from(parts: ComputationKeyParts) -> Result<Self, Self::Error> {
        let schema_name = well_formed("schema name", &parts.schema_name)?;
        let schema_digest = well_formed("schema digest", &parts.schema_digest)?;
        if parts.components.is_empty() {
            return Err(CacheError::SchemaWithoutComponents {
                schema: schema_name,
            });
        }
        for (name, value) in &parts.components {
            well_formed("component name", name)?;
            if value.is_empty() {
                return Err(CacheError::EmptyComponent {
                    schema: schema_name,
                    component: name.clone(),
                });
            }
        }
        Ok(ComputationKey {
            schema_name,
            schema_digest,
            components: parts.components,
        })
    }
}

/// Whether an entry may still be served.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EntryStatus {
    /// Nothing has happened that could have invalidated this entry without proving it.
    Proven,
    /// An invalidation reported itself partial and this entry fell in the unknown region.
    ///
    /// The entry is not deleted, because it may well still be correct and deleting it would
    /// destroy evidence about what the cache held. It simply stops being servable.
    Unproven {
        since: Epoch,
        /// The change whose consequences could not be fully traced.
        cause: String,
    },
}

/// A stored value with everything needed to prove a later hit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CacheEntry {
    pub key: ComputationKey,
    pub value: Value,
    pub produced_by: CodeIdentity,
    pub written_at: Epoch,
    pub dependencies: DependencyDeclaration,
    pub status: EntryStatus,
}

/// Why a hit was served: every component that matched, and the build provenance relied on.
///
/// The proof is the point. A caller that logs this can later answer "why did we not recompute
/// that" without trusting the cache's own account of itself.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HitProof {
    pub digest: String,
    pub schema_digest: String,
    pub matched: Vec<(String, String)>,
    pub produced_by: CodeIdentity,
    pub requested_by: CodeIdentity,
    pub reuse: ReuseRule,
    pub written_at: Epoch,
}

impl HitProof {
    /// Whether the value was computed by a build other than the one asking for it.
    pub fn is_cross_build(&self) -> bool {
        self.produced_by != self.requested_by
    }
}

/// A served value and its proof.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CacheHit {
    pub value: Value,
    pub proof: HitProof,
}

/// Why a lookup did not produce a value.
///
/// A bare `None` would make every one of these look alike in a metric, and the difference
/// between "we have never computed this" and "we refuse to serve what we have" is the difference
/// between a cold cache and a broken one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MissReason {
    /// Nothing is stored at this address.
    NoEntry,
    /// An entry exists at this address but was written under a different key schema. Only
    /// reachable through [`Cache::restore`], because a live schema digest is folded into the
    /// address.
    SchemaChanged {
        stored_schema_digest: String,
        requested_schema_digest: String,
    },
    /// The value was produced by another build and the schema forbids cross-build reuse.
    CrossBuild {
        produced_by: CodeIdentity,
        requested_by: CodeIdentity,
    },
    /// An invalidation could not prove this entry unaffected, so it is no longer servable.
    UnprovenAfterPartialInvalidation { since: Epoch, cause: String },
}

impl MissReason {
    pub fn name(&self) -> &'static str {
        match self {
            MissReason::NoEntry => "no-entry",
            MissReason::SchemaChanged { .. } => "schema-changed",
            MissReason::CrossBuild { .. } => "cross-build",
            MissReason::UnprovenAfterPartialInvalidation { .. } => "unproven",
        }
    }
}

/// The result of a lookup that did not fail.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Lookup {
    Hit(CacheHit),
    Miss(MissReason),
}

impl Lookup {
    pub fn hit(&self) -> Option<&CacheHit> {
        match self {
            Lookup::Hit(hit) => Some(hit),
            Lookup::Miss(_) => None,
        }
    }

    pub fn miss_reason(&self) -> Option<&MissReason> {
        match self {
            Lookup::Hit(_) => None,
            Lookup::Miss(reason) => Some(reason),
        }
    }

    pub fn is_hit(&self) -> bool {
        matches!(self, Lookup::Hit(_))
    }
}

/// What applying an invalidation plan did.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApplyReport {
    /// Entries proved invalid and removed.
    pub removed: BTreeSet<String>,
    /// Entries that could not be proved either way and are no longer servable.
    pub marked_unproven: BTreeSet<String>,
    /// Entries the plan proved unaffected, left untouched and still servable.
    pub left_proven: BTreeSet<String>,
    /// False when the plan reported itself partial. A caller writing an audit line should say
    /// so; the cache's own behaviour has already accounted for it.
    pub invalidation_was_complete: bool,
}

/// A cache that refuses to serve an entry it cannot prove identical, and refuses to serve one it
/// cannot prove current.
#[derive(Debug)]
pub struct Cache {
    schema: KeySchema,
    entries: BTreeMap<String, CacheEntry>,
    hits: u64,
    misses: BTreeMap<&'static str, u64>,
}

impl Cache {
    pub fn new(schema: KeySchema) -> Self {
        Cache {
            schema,
            entries: BTreeMap::new(),
            hits: 0,
            misses: BTreeMap::new(),
        }
    }

    /// Rebuilds a cache from a persisted index, keeping each persisted address exactly as written.
    ///
    /// Recomputing the address on restore is the tempting shortcut and it is precisely wrong: it
    /// would relocate every entry to wherever *this* build's digest function puts it, papering
    /// over a changed canonicalization, a changed schema or a corrupted file. Keeping the written
    /// address means the component-by-component check in [`Cache::lookup`] meets the
    /// disagreement head-on and reports it as [`CacheError::KeyCollision`]. This is the reachable
    /// path to that variant; a genuine SHA-256 collision is the other.
    pub fn restore(
        schema: KeySchema,
        entries: impl IntoIterator<Item = (String, CacheEntry)>,
    ) -> Self {
        let mut cache = Cache::new(schema);
        for (digest, entry) in entries {
            cache.entries.insert(digest, entry);
        }
        cache
    }

    pub fn schema(&self) -> &KeySchema {
        &self.schema
    }

    /// Stores a value, recording the build that produced it and what it depends on.
    pub fn insert(
        &mut self,
        key: ComputationKey,
        value: Value,
        produced_by: CodeIdentity,
        written_at: Epoch,
        dependencies: DependencyDeclaration,
    ) -> Result<String, CacheError> {
        key.validate_against(&self.schema)?;
        let digest = key.digest();
        self.entries.insert(
            digest.clone(),
            CacheEntry {
                key,
                value,
                produced_by,
                written_at,
                dependencies,
                status: EntryStatus::Proven,
            },
        );
        Ok(digest)
    }

    /// Looks up `key` on behalf of build `requested_by`.
    ///
    /// The order of checks is the contract:
    ///
    /// 1. the key is re-validated against this cache's schema;
    /// 2. a candidate is located by digest, or the lookup misses;
    /// 3. the candidate's schema digest must match, or the lookup misses;
    /// 4. **every component is compared**, and a difference is [`CacheError::KeyCollision`] naming
    ///    the component — never a miss, because a miss would let the caller recompute and
    ///    overwrite the evidence that two computations share an address;
    /// 5. an unproven entry misses;
    /// 6. a cross-build entry misses unless the schema permits reuse;
    /// 7. otherwise a hit, with a proof enumerating what matched.
    pub fn lookup(
        &mut self,
        key: &ComputationKey,
        requested_by: &CodeIdentity,
    ) -> Result<Lookup, CacheError> {
        key.validate_against(&self.schema)?;
        let digest = key.digest();

        let Some(entry) = self.entries.get(&digest) else {
            return Ok(self.record_miss(MissReason::NoEntry));
        };

        if entry.key.schema_digest != key.schema_digest {
            let reason = MissReason::SchemaChanged {
                stored_schema_digest: entry.key.schema_digest.clone(),
                requested_schema_digest: key.schema_digest.clone(),
            };
            return Ok(self.record_miss(reason));
        }

        for (name, presented) in key.components() {
            let stored = entry.key.components.get(name).map(String::as_str);
            if stored != Some(presented) {
                return Err(CacheError::KeyCollision {
                    digest,
                    component: name.to_string(),
                    stored: stored.unwrap_or_default().to_string(),
                    presented: presented.to_string(),
                });
            }
        }
        for (name, stored) in &entry.key.components {
            if !key.components.contains_key(name) {
                return Err(CacheError::KeyCollision {
                    digest,
                    component: name.clone(),
                    stored: stored.clone(),
                    presented: String::new(),
                });
            }
        }

        if let EntryStatus::Unproven { since, cause } = &entry.status {
            let reason = MissReason::UnprovenAfterPartialInvalidation {
                since: *since,
                cause: cause.clone(),
            };
            return Ok(self.record_miss(reason));
        }

        if &entry.produced_by != requested_by && self.schema.reuse == ReuseRule::SameBuildOnly {
            let reason = MissReason::CrossBuild {
                produced_by: entry.produced_by.clone(),
                requested_by: requested_by.clone(),
            };
            return Ok(self.record_miss(reason));
        }

        let proof = HitProof {
            digest,
            schema_digest: entry.key.schema_digest.clone(),
            matched: key
                .components()
                .map(|(name, value)| (name.to_string(), value.to_string()))
                .collect(),
            produced_by: entry.produced_by.clone(),
            requested_by: requested_by.clone(),
            reuse: self.schema.reuse,
            written_at: entry.written_at,
        };
        let value = entry.value.clone();
        self.hits += 1;
        Ok(Lookup::Hit(CacheHit { value, proof }))
    }

    fn record_miss(&mut self, reason: MissReason) -> Lookup {
        *self.misses.entry(reason.name()).or_insert(0) += 1;
        Lookup::Miss(reason)
    }

    /// The `(digest, declaration)` pairs an invalidation is computed over.
    pub fn declarations(&self) -> Vec<(String, &DependencyDeclaration)> {
        self.entries
            .iter()
            .map(|(digest, entry)| (digest.clone(), &entry.dependencies))
            .collect()
    }

    /// Acts on an invalidation plan.
    ///
    /// Removes what the plan proved invalid. Where the plan reported itself
    /// [`crate::invalidation::Completeness::Partial`], marks every entry in the unknown region
    /// [`EntryStatus::Unproven`], which stops it being served. This is the whole reason
    /// completeness is reported rather than assumed: an incomplete invalidation has a defined
    /// consequence, and it is a loss of hit rate rather than a loss of correctness.
    ///
    /// Refused if the cache's population changed since the plan was computed, because a plan
    /// applied to entries it never examined would leave the new ones servable and unexamined —
    /// which is exactly the silent staleness the module exists to prevent.
    pub fn apply(
        &mut self,
        plan: &InvalidationPlan,
        at: Epoch,
    ) -> Result<ApplyReport, crate::error::InvalidationError> {
        if plan.population != self.entries.len() {
            return Err(crate::error::InvalidationError::PopulationChanged {
                planned: plan.population,
                actual: self.entries.len(),
            });
        }

        let mut report = ApplyReport {
            invalidation_was_complete: plan.is_complete(),
            ..ApplyReport::default()
        };

        for digest in &plan.invalid_entries {
            if self.entries.remove(digest).is_some() {
                report.removed.insert(digest.clone());
            }
        }

        let cause = format!("invalidation of {} was partial", plan.changed);
        for digest in plan.unproven_entries() {
            if let Some(entry) = self.entries.get_mut(&digest) {
                entry.status = EntryStatus::Unproven {
                    since: at,
                    cause: cause.clone(),
                };
                report.marked_unproven.insert(digest);
            }
        }

        for digest in &plan.proved_unaffected {
            if self.entries.contains_key(digest) {
                report.left_proven.insert(digest.clone());
            }
        }

        Ok(report)
    }

    /// Restores an entry to servable status after something proved it current again.
    ///
    /// Takes the build that re-established the claim, so an audit trail can say who vouched.
    /// There is deliberately no bulk "clear all unproven": an operator who wants the hit rate
    /// back must name what they are re-asserting.
    pub fn reprove(&mut self, digest: &str, by: &CodeIdentity) -> Option<&CodeIdentity> {
        let entry = self.entries.get_mut(digest)?;
        entry.status = EntryStatus::Proven;
        entry.produced_by = by.clone();
        Some(&entry.produced_by)
    }

    pub fn get(&self, digest: &str) -> Option<&CacheEntry> {
        self.entries.get(digest)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Entries no longer servable because an invalidation could not prove them current.
    pub fn unproven(&self) -> BTreeSet<String> {
        self.entries
            .iter()
            .filter(|(_, entry)| matches!(entry.status, EntryStatus::Unproven { .. }))
            .map(|(digest, _)| digest.clone())
            .collect()
    }

    pub fn hits(&self) -> u64 {
        self.hits
    }

    /// Misses by reason. 12.10 asks for "savings and incorrect hits" as metrics; a breakdown by
    /// reason is the honest version, since a cache full of unproven entries and a cold cache
    /// report the same hit rate.
    pub fn misses_by_reason(&self) -> &BTreeMap<&'static str, u64> {
        &self.misses
    }

    pub fn hit_rate(&self) -> f64 {
        let misses: u64 = self.misses.values().sum();
        let total = self.hits + misses;
        if total == 0 {
            0.0
        } else {
            self.hits as f64 / total as f64
        }
    }
}
