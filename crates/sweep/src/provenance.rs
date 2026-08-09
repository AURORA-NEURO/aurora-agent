//! Aliases, revisions, lineage, tombstones and the three axes of a version.
//!
//! Implements blueprint 03.11 (Provenance, Identifiers and Versioning). Four of its five design
//! parts are predicates over artifacts and are here; the fifth (the identifier scheme itself) is
//! discharged by `bioprism-ids`, which already supplies opaque run and event ids and content
//! digests, so this module takes those as given and does not restate them.
//!
//! # Three things that are usually one thing
//!
//! 03.11's version policy says: "Semantic versioning applies to public package behavior; schema
//! compatibility is declared separately. Benchmark score comparability may require a new major
//! version even when file formats are compatible."
//!
//! Most registries carry one number and let it mean all three. Here [`VersionPolicy`] carries
//! [`SemverChange`], [`SchemaCompatibility`] and [`ScoreComparability`] as independent fields,
//! because the case the blueprint names — a scoring change that keeps every file readable — is
//! exactly the case a single number cannot express, and it is the one that silently invalidates a
//! leaderboard. [`results_comparable`] consults the third axis only.
//!
//! # Absent and withdrawn are different
//!
//! 03.11: "Illegal, withdrawn, or dangerous artifacts can be tombstoned in a registry while their
//! hashes remain referenced for audit. Clients receive policy metadata rather than silent
//! disappearance."
//!
//! [`ResolveOutcome`] therefore has three variants, not two: `Resolved`, `Tombstoned` carrying the
//! reason and note, and `Unknown`. A client that treats them alike will still behave correctly; a
//! client that cannot tell them apart cannot behave correctly at all, because "this was withdrawn
//! for a stated reason" and "we have never heard of this" call for opposite actions. Lineage
//! remains queryable for a tombstoned revision — that is what "hashes remain referenced for audit"
//! means, and [`Catalog::lineage_of`] is the test of it.
//!
//! # No clock
//!
//! 03.11's lineage list includes a time. This crate has no clock and does not invent one:
//! [`Lineage::recorded_at`] is a caller-supplied string, opaque here, never generated and never
//! compared. Freshness reasoning belongs to whatever recorded the time.
//!
//! # What is not implemented
//!
//! No signatures. 03.11 says an alias "resolves to a signed manifest"; the signature check is not
//! here and cannot be faked by a `bool`, so [`Catalog`] verifies digests and nothing else, exactly
//! as `bioprism-registry` does for the same reason. No transport, no mirrors, no federation —
//! `bioprism-hubapi` owns 10.04's resolution surface and this is the identifier semantics beneath
//! it.

use std::collections::BTreeMap;

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};

use crate::error::{require_nonempty, SweepError};

/// A human-readable, mutable pointer: namespace, name, version.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Alias {
    pub namespace: String,
    pub name: String,
    pub version: String,
}

impl Alias {
    pub fn new(
        namespace: impl Into<String>,
        name: impl Into<String>,
        version: impl Into<String>,
    ) -> Result<Self, SweepError> {
        let (namespace, name, version) = (namespace.into(), name.into(), version.into());
        require_nonempty(&namespace, "Alias", "namespace")?;
        require_nonempty(&name, "Alias", "name")?;
        require_nonempty(&version, "Alias", "version")?;
        Ok(Alias { namespace, name, version })
    }

    pub fn as_string(&self) -> String {
        format!("{}/{}@{}", self.namespace, self.name, self.version)
    }
}

/// An immutable artifact, named by its digest.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Revision(ContentHash);

impl Revision {
    pub fn new(digest: ContentHash) -> Self {
        Revision(digest)
    }

    pub fn digest(&self) -> &ContentHash {
        &self.0
    }
}

/// What an alias resolved to.
///
/// The digest field is not optional. 03.11: "Results record the digest, never only the alias." A
/// `Resolution` with no revision is unrepresentable, so a result that carries a `Resolution`
/// carries a digest whether or not anybody remembered to check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Resolution {
    alias: Option<Alias>,
    revision: Revision,
}

impl Resolution {
    /// A resolution reached through a human alias. Both halves are recorded.
    pub fn via_alias(alias: Alias, revision: Revision) -> Self {
        Resolution { alias: Some(alias), revision }
    }

    /// A resolution stated directly by digest, with no alias involved.
    pub fn direct(revision: Revision) -> Self {
        Resolution { alias: None, revision }
    }

    pub fn revision(&self) -> &Revision {
        &self.revision
    }

    pub fn alias(&self) -> Option<&Alias> {
        self.alias.as_ref()
    }
}

/// Why an artifact was tombstoned. 03.11's three reasons.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TombstoneReason {
    Illegal,
    Withdrawn,
    Dangerous,
}

/// A withdrawal that stays on the record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tombstone {
    pub revision: Revision,
    pub reason: TombstoneReason,
    /// The policy metadata a client receives instead of a silence.
    pub note: String,
}

impl Tombstone {
    pub fn new(
        revision: Revision,
        reason: TombstoneReason,
        note: impl Into<String>,
    ) -> Result<Self, SweepError> {
        let note = note.into();
        require_nonempty(&note, "Tombstone", "note")?;
        Ok(Tombstone { revision, reason, note })
    }
}

/// Whether the derivation of an artifact was checked.
///
/// 03.11 asks each derived artifact to list "validation evidence". An empty evidence string and a
/// missing evidence field are the same thing to a reader, so the absence is a variant.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "state")]
pub enum Validation {
    /// Nobody checked this derivation.
    Unvalidated,
    /// Checked, with the named evidence.
    Validated { evidence: String },
}

/// How a derived artifact came to be. 03.11's lineage field list, in its order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Lineage {
    pub inputs: Vec<Revision>,
    pub transformation: String,
    pub implementation_version: String,
    pub parameters: BTreeMap<String, String>,
    pub seed: Option<u64>,
    pub actor: String,
    /// Caller-supplied and opaque. This crate never generates or interprets it.
    pub recorded_at: String,
    pub validation: Validation,
}

impl Lineage {
    pub fn new(
        transformation: impl Into<String>,
        implementation_version: impl Into<String>,
        actor: impl Into<String>,
        recorded_at: impl Into<String>,
    ) -> Result<Self, SweepError> {
        let transformation = transformation.into();
        let implementation_version = implementation_version.into();
        let actor = actor.into();
        require_nonempty(&transformation, "Lineage", "transformation")?;
        require_nonempty(&implementation_version, "Lineage", "implementation_version")?;
        require_nonempty(&actor, "Lineage", "actor")?;
        Ok(Lineage {
            inputs: Vec::new(),
            transformation,
            implementation_version,
            parameters: BTreeMap::new(),
            seed: None,
            actor,
            recorded_at: recorded_at.into(),
            validation: Validation::Unvalidated,
        })
    }

    pub fn from_input(mut self, revision: Revision) -> Self {
        self.inputs.push(revision);
        self
    }

    pub fn with_parameter(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.parameters.insert(key.into(), value.into());
        self
    }

    pub fn with_seed(mut self, seed: u64) -> Self {
        self.seed = Some(seed);
        self
    }

    pub fn validated(mut self, evidence: impl Into<String>) -> Result<Self, SweepError> {
        let evidence = evidence.into();
        require_nonempty(&evidence, "Lineage::validated", "evidence")?;
        self.validation = Validation::Validated { evidence };
        Ok(self)
    }

    /// Whether this artifact can be reproduced from what the lineage records.
    ///
    /// False when the transformation used a seed that was not recorded — a `None` seed on a
    /// transformation the caller declared stochastic. The caller declares that, because this crate
    /// cannot tell a deterministic transformation from a stochastic one by looking at its name.
    pub fn reproducible(&self, stochastic: bool) -> bool {
        !self.inputs.is_empty() && (!stochastic || self.seed.is_some())
    }
}

/// How a package version changed. 03.11 applies semver to "public package behavior".
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SemverChange {
    Patch,
    Minor,
    Major,
}

/// Whether existing files still parse. Declared separately from semver.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SchemaCompatibility {
    Compatible,
    Incompatible,
}

/// Whether scores from before the change may be put next to scores from after it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "state")]
pub enum ScoreComparability {
    /// Scoring semantics unchanged.
    Comparable,
    /// Scoring semantics changed. The reason is required because a broken comparability with no
    /// stated cause cannot be reviewed, only obeyed.
    Broken { reason: String },
}

/// The three axes of a version change, kept apart.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VersionPolicy {
    pub semver: SemverChange,
    pub schema: SchemaCompatibility,
    pub comparability: ScoreComparability,
}

impl VersionPolicy {
    pub fn new(
        semver: SemverChange,
        schema: SchemaCompatibility,
        comparability: ScoreComparability,
    ) -> Self {
        VersionPolicy { semver, schema, comparability }
    }

    /// Whether the declared semver change is large enough for what actually changed.
    ///
    /// 03.11's worked case: a scoring change that keeps every file readable still requires a major
    /// version. A policy that declares `Minor` alongside broken comparability is refused, naming
    /// which axis forced the major.
    pub fn check(&self) -> Result<(), SweepError> {
        let forcing = match (&self.schema, &self.comparability) {
            (SchemaCompatibility::Incompatible, _) => Some("schema compatibility"),
            (_, ScoreComparability::Broken { .. }) => Some("score comparability"),
            _ => None,
        };
        match forcing {
            Some(axis) if self.semver != SemverChange::Major => {
                Err(SweepError::malformed(
                    "VersionPolicy",
                    format!("{axis} forces a major version, but the change declares {:?}", self.semver),
                ))
            }
            _ => Ok(()),
        }
    }
}

/// Whether two results may be placed side by side.
///
/// Consults comparability only. Two results under mutually incompatible schemas are still
/// comparable if the scoring semantics did not change — the files need not be readable by the same
/// parser for the numbers to mean the same thing.
pub fn results_comparable(left: &PublishedResult, right: &PublishedResult) -> bool {
    left.comparability_epoch == right.comparability_epoch
}

/// A result that has been published, and therefore frozen.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublishedResult {
    pub id: String,
    pub revision: Revision,
    pub schema_version: String,
    /// An opaque token that changes whenever scoring semantics change. Equal tokens mean
    /// comparable results; unequal tokens mean nothing about the direction of the difference.
    pub comparability_epoch: String,
}

/// A catalog of aliases, revisions, tombstones, lineage and published results.
///
/// In-process and serialisable. No network, no signatures, no storage — see the module docs.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Catalog {
    aliases: BTreeMap<String, Revision>,
    tombstones: BTreeMap<String, Tombstone>,
    lineage: BTreeMap<String, Lineage>,
    published: BTreeMap<String, PublishedResult>,
}

/// The three answers to a resolution request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "outcome")]
pub enum ResolveOutcome {
    Resolved { resolution: Resolution },
    /// Withdrawn, with the policy metadata a client needs in order to say why.
    Tombstoned { tombstone: Tombstone },
    /// No such alias. Distinct from `Tombstoned` in the only way that matters: it carries no reason
    /// because there is none to carry.
    Unknown,
}

impl Catalog {
    pub fn new() -> Self {
        Catalog::default()
    }

    pub fn publish_alias(&mut self, alias: &Alias, revision: Revision) {
        self.aliases.insert(alias.as_string(), revision);
    }

    pub fn record_lineage(&mut self, revision: &Revision, lineage: Lineage) {
        self.lineage.insert(revision.digest().as_str().to_string(), lineage);
    }

    pub fn tombstone(&mut self, tombstone: Tombstone) {
        self.tombstones
            .insert(tombstone.revision.digest().as_str().to_string(), tombstone);
    }

    /// Resolve an alias to a digest, or say precisely why not.
    pub fn resolve(&self, alias: &Alias) -> ResolveOutcome {
        match self.aliases.get(&alias.as_string()) {
            None => ResolveOutcome::Unknown,
            Some(revision) => {
                match self.tombstones.get(revision.digest().as_str()) {
                    Some(tombstone) => {
                        ResolveOutcome::Tombstoned { tombstone: tombstone.clone() }
                    }
                    None => ResolveOutcome::Resolved {
                        resolution: Resolution::via_alias(alias.clone(), revision.clone()),
                    },
                }
            }
        }
    }

    /// Lineage stays readable after a tombstone: "hashes remain referenced for audit".
    pub fn lineage_of(&self, revision: &Revision) -> Option<&Lineage> {
        self.lineage.get(revision.digest().as_str())
    }

    /// Freeze a result. The first publication of an id wins.
    pub fn publish_result(&mut self, result: PublishedResult) -> Result<(), SweepError> {
        if let Some(existing) = self.published.get(&result.id) {
            if existing != &result {
                return Err(SweepError::RetroactiveRewrite {
                    result: result.id.clone(),
                    published: existing.comparability_epoch.clone(),
                    proposed: result.comparability_epoch.clone(),
                });
            }
            return Ok(());
        }
        self.published.insert(result.id.clone(), result);
        Ok(())
    }

    pub fn published_result(&self, id: &str) -> Option<&PublishedResult> {
        self.published.get(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rev(seed: &str) -> Revision {
        Revision::new(ContentHash::of_bytes(seed.as_bytes()))
    }

    fn alias() -> Alias {
        Alias::new("aurora", "onco-core", "1.2.0").unwrap()
    }

    #[test]
    fn a_resolution_always_carries_a_digest_even_when_reached_by_alias() {
        let resolution = Resolution::via_alias(alias(), rev("r1"));
        assert_eq!(resolution.revision(), &rev("r1"));
        assert_eq!(resolution.alias().unwrap().name, "onco-core");
    }

    #[test]
    fn a_tombstoned_alias_resolves_to_policy_metadata_not_to_unknown() {
        let mut catalog = Catalog::new();
        catalog.publish_alias(&alias(), rev("r1"));
        catalog.tombstone(
            Tombstone::new(rev("r1"), TombstoneReason::Withdrawn, "superseded by 1.3.0").unwrap(),
        );
        match catalog.resolve(&alias()) {
            ResolveOutcome::Tombstoned { tombstone } => {
                assert_eq!(tombstone.reason, TombstoneReason::Withdrawn);
                assert_eq!(tombstone.note, "superseded by 1.3.0");
            }
            other => panic!("expected a tombstone, got {other:?}"),
        }
    }

    #[test]
    fn an_unknown_alias_is_not_a_tombstone() {
        let catalog = Catalog::new();
        assert_eq!(catalog.resolve(&alias()), ResolveOutcome::Unknown);
    }

    #[test]
    fn lineage_survives_a_tombstone_so_the_hash_stays_auditable() {
        let mut catalog = Catalog::new();
        catalog.publish_alias(&alias(), rev("r1"));
        catalog.record_lineage(
            &rev("r1"),
            Lineage::new("mutate", "0.1.0", "ci", "opaque").unwrap().from_input(rev("parent")),
        );
        catalog.tombstone(
            Tombstone::new(rev("r1"), TombstoneReason::Dangerous, "leaks holdout answers")
                .unwrap(),
        );
        assert!(catalog.lineage_of(&rev("r1")).is_some());
    }

    #[test]
    fn a_tombstone_requires_a_note_because_a_reason_code_alone_is_not_metadata() {
        assert!(Tombstone::new(rev("r"), TombstoneReason::Illegal, "  ").is_err());
    }

    #[test]
    fn broken_score_comparability_forces_a_major_even_when_the_schema_is_compatible() {
        let policy = VersionPolicy::new(
            SemverChange::Minor,
            SchemaCompatibility::Compatible,
            ScoreComparability::Broken { reason: "partial credit rule changed".into() },
        );
        let err = policy.check().unwrap_err();
        assert!(format!("{err}").contains("score comparability"));
        let fixed = VersionPolicy::new(
            SemverChange::Major,
            SchemaCompatibility::Compatible,
            ScoreComparability::Broken { reason: "partial credit rule changed".into() },
        );
        assert!(fixed.check().is_ok());
    }

    #[test]
    fn an_incompatible_schema_also_forces_a_major() {
        let policy = VersionPolicy::new(
            SemverChange::Patch,
            SchemaCompatibility::Incompatible,
            ScoreComparability::Comparable,
        );
        assert!(policy.check().is_err());
    }

    #[test]
    fn a_patch_that_changes_neither_axis_is_accepted() {
        let policy = VersionPolicy::new(
            SemverChange::Patch,
            SchemaCompatibility::Compatible,
            ScoreComparability::Comparable,
        );
        assert!(policy.check().is_ok());
    }

    #[test]
    fn results_from_different_comparability_epochs_are_not_comparable() {
        let a = PublishedResult {
            id: "res-1".into(),
            revision: rev("r1"),
            schema_version: "3".into(),
            comparability_epoch: "e1".into(),
        };
        let b = PublishedResult {
            id: "res-2".into(),
            revision: rev("r2"),
            schema_version: "3".into(),
            comparability_epoch: "e2".into(),
        };
        assert!(!results_comparable(&a, &b));
        let c = PublishedResult { id: "res-3".into(), ..b.clone() };
        assert!(results_comparable(&b, &c));
    }

    #[test]
    fn schema_incompatibility_alone_does_not_make_results_incomparable() {
        let a = PublishedResult {
            id: "res-1".into(),
            revision: rev("r1"),
            schema_version: "3".into(),
            comparability_epoch: "e1".into(),
        };
        let b = PublishedResult { id: "res-2".into(), schema_version: "4".into(), ..a.clone() };
        assert!(results_comparable(&a, &b));
    }

    #[test]
    fn a_published_result_cannot_be_re_published_under_a_new_epoch() {
        let mut catalog = Catalog::new();
        let result = PublishedResult {
            id: "res-1".into(),
            revision: rev("r1"),
            schema_version: "3".into(),
            comparability_epoch: "e1".into(),
        };
        catalog.publish_result(result.clone()).unwrap();
        catalog.publish_result(result.clone()).unwrap();
        let rewritten = PublishedResult { comparability_epoch: "e2".into(), ..result };
        assert!(matches!(
            catalog.publish_result(rewritten),
            Err(SweepError::RetroactiveRewrite { .. })
        ));
    }

    #[test]
    fn an_unvalidated_lineage_is_a_distinct_state_from_a_validated_one() {
        let l = Lineage::new("normalise", "0.2.0", "importer", "opaque").unwrap();
        assert_eq!(l.validation, Validation::Unvalidated);
        let v = l.validated("golden fixture 7 reproduced").unwrap();
        assert!(matches!(v.validation, Validation::Validated { .. }));
    }

    #[test]
    fn a_stochastic_transformation_without_a_recorded_seed_is_not_reproducible() {
        let l = Lineage::new("sample", "0.1.0", "ci", "opaque")
            .unwrap()
            .from_input(rev("parent"));
        assert!(l.reproducible(false));
        assert!(!l.reproducible(true));
        assert!(l.with_seed(7).reproducible(true));
    }

    #[test]
    fn a_lineage_with_no_inputs_is_not_reproducible_whatever_else_it_records() {
        let l = Lineage::new("conjure", "0.1.0", "ci", "opaque").unwrap().with_seed(1);
        assert!(!l.reproducible(false));
    }
}
