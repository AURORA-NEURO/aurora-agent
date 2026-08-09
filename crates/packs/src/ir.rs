//! Benchmark Pack IR — what a pack *is* (blueprint 03.06).
//!
//! A pack is the unit PRISM ships, cites, and eventually retires: a manifest that says what is
//! being measured and under what license, plus content that says which parent environments the
//! instances descend from, which decision families were mined, which oracles can decide them, and
//! how many of the advertised instances actually exist.
//!
//! Two things this module deliberately does not do.
//!
//! It does not generate instances. Parents are referenced by [`bioprism_ids::WorldId`] and
//! generated instances by seed range; materializing either is `worldgen` and `mutation` work. The
//! IR carries the *claim* about instances so the claim can be checked against what the generator
//! can produce, which is 03.06's rule that "published counts must be materializable and
//! validated, not merely theoretical".
//!
//! It does not assign trust tiers. 03.06 lists a quality tier in the manifest, but tiering and
//! release gating are the registry's contract; a pack that tiered itself would be marking its own
//! homework. What is here is the evidence a tiering decision would consume.
//!
//! The content hash is taken over the canonical encoding from `bioprism-ids`, so two documents
//! that differ only in key order or whitespace address the same pack, and any change to a
//! parent, a family, an oracle or a count changes the address.

use crate::error::PackError;
use crate::taxonomy::{CapabilityFamily, Domain, OracleTier, PackAxis};
use bioprism_ids::{ContentHash, WorldId};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt;

/// A pack identifier, of the form `namespace.name`.
///
/// Constrained rather than free-form because these strings appear in dependency edges and in
/// published results, where a pack that can be spelled two ways is a pack whose results cannot be
/// joined.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct PackId(String);

impl PackId {
    pub fn parse(value: impl Into<String>) -> Result<Self, PackError> {
        let value = value.into();
        let starts_with_letter = value.chars().next().is_some_and(|c| c.is_ascii_lowercase());
        let charset_ok = value
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '.' || c == '-');
        let has_namespace = value.contains('.');
        let no_edge_dots =
            !value.starts_with('.') && !value.ends_with('.') && !value.contains("..");
        if starts_with_letter && charset_ok && has_namespace && no_edge_dots {
            Ok(PackId(value))
        } else {
            Err(PackError::MalformedPackId(value))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for PackId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<PackId> for String {
    fn from(value: PackId) -> Self {
        value.0
    }
}

impl TryFrom<String> for PackId {
    type Error = PackError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        PackId::parse(value)
    }
}

/// Pack version. Distinct from schema version: the pack may change without the IR changing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct PackVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl PackVersion {
    pub fn new(major: u32, minor: u32, patch: u32) -> Self {
        PackVersion {
            major,
            minor,
            patch,
        }
    }
}

impl fmt::Display for PackVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// The inclusive range of IR schema versions a runtime must support to execute this pack.
///
/// 03.06 requires the range because "changes to schemas and scoring semantics are versioned and
/// cannot retroactively rewrite already published results" — a runtime outside the range must
/// refuse rather than guess.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SchemaRange {
    pub min_inclusive: u32,
    pub max_inclusive: u32,
}

impl SchemaRange {
    pub fn new(min_inclusive: u32, max_inclusive: u32) -> Self {
        SchemaRange {
            min_inclusive,
            max_inclusive,
        }
    }

    pub fn accepts(&self, schema: u32) -> bool {
        schema >= self.min_inclusive && schema <= self.max_inclusive
    }

    pub fn is_empty(&self) -> bool {
        self.min_inclusive > self.max_inclusive
    }
}

/// A dependency on another pack.
///
/// The digest is optional in the wire format only so that an unpinned dependency can be *reported*
/// rather than silently deserialized into something plausible; [`PackManifest::validate`] rejects
/// it. 03.06: "Pack dependencies are pinned by digest."
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PackDependency {
    pub id: PackId,
    pub digest: Option<ContentHash>,
}

impl PackDependency {
    pub fn pinned(id: PackId, digest: ContentHash) -> Self {
        PackDependency {
            id,
            digest: Some(digest),
        }
    }
}

/// Trusted metadata. Parsed before any content, per 03.06's trust boundary.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PackManifest {
    pub id: PackId,
    pub version: PackVersion,
    pub schema_range: SchemaRange,
    /// Human title, as it appears in the benchmark card.
    pub title: String,
    /// One sentence naming what the pack claims to measure. Not a description of its tasks —
    /// the construct, which is what a score would be interpreted as evidence about.
    pub measures: String,
    /// The blueprint module this pack realizes, e.g. `15.05` or `29.12`.
    pub blueprint_module: String,
    pub axis: PackAxis,
    pub capabilities: Vec<CapabilityFamily>,
    pub domains: Vec<Domain>,
    pub owners: Vec<String>,
    pub license: String,
    pub dependencies: Vec<PackDependency>,
}

impl PackManifest {
    /// Checks the conditions under which a score from this pack could not be interpreted at all.
    ///
    /// Deliberately narrow: this is not pack eligibility (15.00 requires calibration, exploit
    /// review and a maintainer plan on top), and it is not a trust tier. It rejects only the
    /// manifests that are self-defeating — no capability claim, no owner, an empty schema range,
    /// a dependency that would be resolved by name.
    pub fn validate(&self) -> Result<(), PackError> {
        if self.capabilities.is_empty() {
            return Err(PackError::NoCapabilityClaim(self.id.to_string()));
        }
        if self.schema_range.is_empty() {
            return Err(PackError::EmptySchemaRange {
                pack: self.id.to_string(),
                min: self.schema_range.min_inclusive,
                max: self.schema_range.max_inclusive,
            });
        }
        for dependency in &self.dependencies {
            if dependency.digest.is_none() {
                return Err(PackError::UnpinnedDependency {
                    pack: self.id.to_string(),
                    dependency: dependency.id.to_string(),
                });
            }
        }
        Ok(())
    }
}

/// A parent environment and how many decision parents were mined from it.
///
/// 15.00's invariant list requires a benchmark family, a parent task, a generated instance, an
/// execution trial and a scored result never to be conflated. Keeping the parent count and the
/// instance count in different types is the cheapest way to stop the two being added together.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParentEnvironment {
    pub world: WorldId,
    /// Decision cells mined from executions of this world.
    pub decision_parents: u64,
}

/// A half-open seed range for a deterministic generator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SeedRange {
    pub start: u64,
    pub end_exclusive: u64,
}

impl SeedRange {
    pub fn new(start: u64, end_exclusive: u64) -> Self {
        SeedRange {
            start,
            end_exclusive,
        }
    }

    /// How many instances this range could materialize. Zero if inverted.
    pub fn capacity(&self) -> u64 {
        self.end_exclusive.saturating_sub(self.start)
    }
}

/// Where the pack's instances come from, and how many of them exist as opposed to being implied.
///
/// The `declared`/`validated` split is the whole point. 03.06 permits a pack to represent large
/// instance sets by a generator and a seed range, and immediately constrains it: counts must be
/// materializable and validated. A pack advertising a million descendants of which two thousand
/// have been checked is making two different claims, and they are stored in two different fields.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum InstanceSource {
    /// Hand-authored instances. Declared and validated counts coincide by construction.
    Authored { validated: u64 },
    /// A deterministic generator over a seed range.
    DeterministicGenerator {
        seeds: SeedRange,
        declared: u64,
        validated: u64,
    },
    /// Instances imported through an adapter from an external benchmark.
    AdapterImport {
        adapter: String,
        declared: u64,
        validated: u64,
    },
}

impl InstanceSource {
    pub fn declared(&self) -> u64 {
        match self {
            InstanceSource::Authored { validated } => *validated,
            InstanceSource::DeterministicGenerator { declared, .. } => *declared,
            InstanceSource::AdapterImport { declared, .. } => *declared,
        }
    }

    pub fn validated(&self) -> u64 {
        match self {
            InstanceSource::Authored { validated } => *validated,
            InstanceSource::DeterministicGenerator { validated, .. } => *validated,
            InstanceSource::AdapterImport { validated, .. } => *validated,
        }
    }

    fn validate(&self, pack: &PackId) -> Result<(), PackError> {
        if let InstanceSource::DeterministicGenerator {
            seeds, declared, ..
        } = self
        {
            if *declared > seeds.capacity() {
                return Err(PackError::CountsExceedGenerator {
                    pack: pack.to_string(),
                    declared: *declared,
                    available: seeds.capacity(),
                });
            }
        }
        if self.validated() > self.declared() {
            return Err(PackError::ValidatedExceedsDeclared {
                pack: pack.to_string(),
                declared: self.declared(),
                validated: self.validated(),
            });
        }
        Ok(())
    }
}

/// The denominators 15.00 requires every published count to carry.
///
/// Verbatim from the module's closing section: "Always publish: pack families, parent
/// environments, decision parents, validated generated instances, effective sample size, executed
/// trials, and independent reproductions. Never use 'millions of benchmarks' without these
/// denominators."
///
/// `effective_sample_size` is `Option` because it is not computed here. It is the count of
/// independent equivalence classes, which `bioprism_mutation::diversity` measures from the
/// lineage; a pack that has not had that measurement taken reports `None`, and
/// [`PublicCounts::headline`] says so rather than substituting the instance count.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PublicCounts {
    pub parent_environments: u64,
    pub decision_parents: u64,
    pub declared_instances: u64,
    pub validated_instances: u64,
    pub effective_sample_size: Option<u64>,
    pub executed_trials: u64,
    pub independent_reproductions: u64,
}

impl PublicCounts {
    /// The sentence a pack release note must lead with.
    ///
    /// It is a single format string with no branch that omits a denominator, which is the only
    /// reliable way to keep the instance count from travelling alone.
    pub fn headline(&self, pack: &PackId) -> String {
        let effective = match self.effective_sample_size {
            Some(n) => format!("{n} independent equivalence classes"),
            None => "effective sample size not measured".to_string(),
        };
        format!(
            "{pack}: {} validated instances ({} declared) from {} parent environment(s) and {} \
             decision parent(s); {effective}; {} executed trial(s); {} independent reproduction(s).",
            self.validated_instances,
            self.declared_instances,
            self.parent_environments,
            self.decision_parents,
            self.executed_trials,
            self.independent_reproductions,
        )
    }

    /// The fraction of advertised instances that have actually been validated.
    ///
    /// `None` when nothing is declared. This is a materialization ratio, not a quality measure:
    /// it says how much of the headline exists, not whether what exists is any good.
    pub fn materialized_fraction(&self) -> Option<f64> {
        if self.declared_instances == 0 {
            None
        } else {
            Some(self.validated_instances as f64 / self.declared_instances as f64)
        }
    }
}

/// Untrusted-by-default content: parents, families, mutation relations, oracles, counts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PackContent {
    pub parent_environments: Vec<ParentEnvironment>,
    /// Microbenchmark families, in the module's own words.
    pub decision_families: Vec<String>,
    /// Names of the validated mutation relations the pack ships.
    pub mutation_relations: Vec<String>,
    pub oracles: Vec<OracleTier>,
    pub instances: InstanceSource,
    pub executed_trials: u64,
    pub independent_reproductions: u64,
    pub effective_sample_size: Option<u64>,
}

impl PackContent {
    pub fn strongest_oracle(&self) -> Option<OracleTier> {
        self.oracles.iter().copied().max_by_key(|t| t.strength())
    }

    pub fn has_grounded_oracle(&self) -> bool {
        self.oracles.iter().any(|t| t.is_execution_grounded())
    }

    pub fn counts(&self) -> PublicCounts {
        PublicCounts {
            parent_environments: self.parent_environments.len() as u64,
            decision_parents: self
                .parent_environments
                .iter()
                .map(|p| p.decision_parents)
                .sum(),
            declared_instances: self.instances.declared(),
            validated_instances: self.instances.validated(),
            effective_sample_size: self.effective_sample_size,
            executed_trials: self.executed_trials,
            independent_reproductions: self.independent_reproductions,
        }
    }
}

/// A complete, content-addressed pack.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PackIr {
    pub manifest: PackManifest,
    pub content: PackContent,
}

impl PackIr {
    pub fn validate(&self) -> Result<(), PackError> {
        self.manifest.validate()?;
        if self.content.oracles.is_empty() {
            return Err(PackError::NoOracle(self.manifest.id.to_string()));
        }
        self.content.instances.validate(&self.manifest.id)?;
        Ok(())
    }

    /// The pack's address: sha256 over the canonical JSON encoding of the whole document.
    ///
    /// Health and calibration are deliberately outside this hash. A pack does not change when a
    /// new system is evaluated against it, and a saturation finding must be attributable to a
    /// specific revision — see [`crate::health::PackAssessment`], which carries this digest.
    pub fn digest(&self) -> Result<ContentHash, PackError> {
        Ok(ContentHash::of_value(&self.to_value()?)?)
    }

    pub fn to_value(&self) -> Result<Value, PackError> {
        Ok(serde_json::to_value(self)?)
    }

    pub fn parse(json: &str) -> Result<Self, PackError> {
        Ok(serde_json::from_str(json)?)
    }

    /// Reads the manifest without deserializing the content.
    ///
    /// 03.06's trust boundary: "Metadata is parsed before code." A pack whose generators,
    /// verifiers or content are malformed — or hostile — must still be inspectable for its
    /// license, owners, capability claim and dependencies, because those are what decide whether
    /// the content is ever touched.
    pub fn parse_manifest_first(json: &str) -> Result<PackManifest, PackError> {
        let document: Value = serde_json::from_str(json)?;
        let manifest = document.get("manifest").ok_or(PackError::MissingManifest)?;
        Ok(serde_json::from_value(manifest.clone())?)
    }
}
