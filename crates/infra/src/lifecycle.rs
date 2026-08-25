//! Storage layout and data lifecycle: blueprint 40.06, with 12.22's retention graph.
//!
//! 40.06 is one of the build-ready contracts and it is unusually specific for section 12. It
//! gives a directory tree, and then four sentences that are the whole contract:
//!
//! > Objects are immutable. Mutable workspaces are copy-on-write and become artifacts only after
//! > hashing. Garbage collection follows signed manifests, active leases, retention policies,
//! > legal holds, and pinned references. Controlled objects include classification and residency
//! > metadata; deletion creates a tombstone and audit event rather than silently rewriting result
//! > history.
//!
//! The last clause is the one this module is built around, and the two before it are what makes
//! it hard.
//!
//! # Deletion leaves a tombstone
//!
//! A published result names the objects it was computed from. If deleting an object removes it
//! from the record, the result silently becomes unverifiable while continuing to look verified.
//! So [`Lifecycle::delete`] never removes the object's identity: it replaces the record with a
//! [`Tombstone`] carrying the digest, the classification, the residency, the basis and the epoch,
//! and every later [`Lifecycle::resolve`] of that object returns
//! [`LifecycleError::Tombstoned`] — an answer, not a `NotFound`. A manifest that referenced the
//! object still references it. History is intact; the bytes are gone.
//!
//! # Storage cannot reclaim ahead of the ledger
//!
//! `bioprism-ledger` already owns retention: its [`RetentionWindow`] states, per axis, the point
//! from which as-of queries are exact. Rather than invent a second retention model, this module
//! *asks* that one, and the coupling runs one way:
//!
//! - [`DeletionBasis::Reclaim`] — deleting to recover space — is refused for any object created
//!   at or after `answerable_from_record`. The ledger still promises to answer about that period,
//!   and an answer naming an object whose bytes are gone is a promise broken quietly.
//! - A ledger that has **never compacted** has an unrestricted window, so nothing is behind a
//!   boundary and *no* object may be reclaimed. That reads as severe and it is correct: an
//!   append-only log that has not compacted still claims to answer everything.
//!   [`LifecycleError::RetentionWindowUnrestricted`] is a separate variant so the remedy —
//!   compact the ledger first — is visible in the error rather than inferred.
//! - [`DeletionBasis::Lawful`] — 12.22's privacy deletion — is permitted anywhere, because it is
//!   compelled. It leaves the same tombstone, which is exactly 12.22's "minimal non-identifying
//!   tombstone and impact notice", and it is the reason `bioprism-ledger` has a `Redaction` type
//!   that keeps the entry digest while destroying the payload.
//!
//! # Deliberately not implemented
//!
//! No filesystem. [`LocalLayout`] computes 40.06's paths as strings and creates nothing; there is
//! no `config.toml`, no SQLite, no DuckDB, no object bytes anywhere in this crate. No leases —
//! 40.06 lists `workspaces/<lease-id>/` and 12.17 owns lease lifecycle. No signatures, so
//! "follows *signed* manifests" is followed as "follows manifests". No grace period or quarantine
//! staging: deletion happens or it does not, the same simplification `bioprism-ledger` makes and
//! for the same reason — a grace period needs a clock. Classification and residency are recorded
//! and required, never enforced; nothing here encrypts, and a tombstone destroys a record rather
//! than a key.

use crate::epoch::Epoch;
use crate::error::LifecycleError;
use bioprism_ids::ContentHash;
use bioprism_ledger::{RecordTime, RetentionWindow};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt;

fn well_formed(field: &'static str, value: &str) -> Result<String, LifecycleError> {
    if value.trim().is_empty() || value.chars().any(char::is_control) {
        return Err(LifecycleError::MalformedField {
            field,
            value: value.to_string(),
        });
    }
    Ok(value.to_string())
}

/// The name of a managed object.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ObjectId(String);

impl ObjectId {
    pub fn parse(value: impl Into<String>) -> Result<Self, LifecycleError> {
        well_formed("object id", &value.into()).map(ObjectId)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ObjectId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<ObjectId> for String {
    fn from(value: ObjectId) -> Self {
        value.0
    }
}

impl TryFrom<String> for ObjectId {
    type Error = LifecycleError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        ObjectId::parse(value)
    }
}

/// How sensitive an object is. 40.06: "controlled objects include classification metadata".
///
/// Required on every object rather than optional, because an unclassified object is one nobody
/// can decide the residency of, and the default a system picks for it is always the permissive
/// one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Classification {
    Public,
    Internal,
    Controlled,
}

impl Classification {
    pub fn name(self) -> &'static str {
        match self {
            Classification::Public => "public",
            Classification::Internal => "internal",
            Classification::Controlled => "controlled",
        }
    }

    /// Whether the object may be deduplicated across tenants. 12.01: "public artifacts may be
    /// globally deduplicated by digest while authorization remains explicit."
    pub fn permits_global_deduplication(self) -> bool {
        matches!(self, Classification::Public)
    }
}

/// Where an object is allowed to live. 40.06: "residency metadata".
///
/// A string rather than an enum of jurisdictions: the set is open, changes by contract, and a
/// closed enum would force an `Other(String)` escape hatch within a year.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Residency(String);

impl Residency {
    pub fn parse(value: impl Into<String>) -> Result<Self, LifecycleError> {
        well_formed("residency", &value.into()).map(Residency)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Residency {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<Residency> for String {
    fn from(value: Residency) -> Self {
        value.0
    }
}

impl TryFrom<String> for Residency {
    type Error = LifecycleError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Residency::parse(value)
    }
}

/// What losing an area costs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Durability {
    /// Losing it destroys evidence. Must be backed up.
    Canonical,
    /// Reconstructible from canonical data. 12.08: "losing them is inconvenient, not evidentiary
    /// corruption."
    Rebuildable,
    /// Valid only for the life of a process.
    Ephemeral,
}

/// The 40.06 local layout, as a closed set of areas.
///
/// Typed rather than string paths so that the durability classification below is attached to the
/// area itself. A backup that copies everything under `.bioprism/` is wasteful; one that copies
/// the wrong subset is a data loss discovered during a restore.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum StorageArea {
    Config,
    Metadata,
    Analytics,
    Events,
    Objects,
    Runs,
    Workspaces,
    Indexes,
    Locks,
}

impl StorageArea {
    pub const ALL: [StorageArea; 9] = [
        StorageArea::Config,
        StorageArea::Metadata,
        StorageArea::Analytics,
        StorageArea::Events,
        StorageArea::Objects,
        StorageArea::Runs,
        StorageArea::Workspaces,
        StorageArea::Indexes,
        StorageArea::Locks,
    ];

    /// The path relative to the root, exactly as 40.06 draws it.
    pub fn relative_path(self) -> &'static str {
        match self {
            StorageArea::Config => "config.toml",
            StorageArea::Metadata => "metadata.sqlite",
            StorageArea::Analytics => "analytics.duckdb",
            StorageArea::Events => "events",
            StorageArea::Objects => "objects",
            StorageArea::Runs => "runs",
            StorageArea::Workspaces => "workspaces",
            StorageArea::Indexes => "indexes",
            StorageArea::Locks => "locks",
        }
    }

    /// What a restore must include, and what it may rebuild.
    ///
    /// `Workspaces` is ephemeral because 40.06 says mutable workspaces "become artifacts only
    /// after hashing" — an unhashed workspace is by definition not yet evidence. `Analytics` is
    /// rebuildable because the result lake is derived from the event log and the catalog (12.06).
    pub fn durability(self) -> Durability {
        match self {
            StorageArea::Config
            | StorageArea::Metadata
            | StorageArea::Events
            | StorageArea::Objects
            | StorageArea::Runs => Durability::Canonical,
            StorageArea::Analytics | StorageArea::Indexes => Durability::Rebuildable,
            StorageArea::Workspaces | StorageArea::Locks => Durability::Ephemeral,
        }
    }
}

/// Path arithmetic for the 40.06 tree. Creates nothing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalLayout {
    root: String,
}

impl LocalLayout {
    pub fn new(root: impl Into<String>) -> Result<Self, LifecycleError> {
        well_formed("layout root", &root.into()).map(|root| LocalLayout { root })
    }

    pub fn root(&self) -> &str {
        &self.root
    }

    pub fn area(&self, area: StorageArea) -> String {
        format!("{}/{}", self.root, area.relative_path())
    }

    /// The path for a content-addressed object.
    ///
    /// 40.06 draws `objects/sha256/ab/cd...` and does not say how deep the fan-out goes. Two
    /// levels of two hex characters is this crate's choice — 65,536 leaf directories, which keeps
    /// a directory listing tractable at the million-object scale `bioprism-scale` targets — and it
    /// is recorded here as a decision rather than presented as specification.
    pub fn object_path(&self, digest: &ContentHash) -> String {
        let text = digest.as_str();
        let (first, rest) = text.split_at(2);
        let (second, tail) = rest.split_at(2);
        format!(
            "{}/{}/sha256/{first}/{second}/{tail}",
            self.root,
            StorageArea::Objects.relative_path()
        )
    }

    /// Areas a restore must carry, in the sense of [`Durability::Canonical`].
    pub fn canonical_areas() -> Vec<StorageArea> {
        StorageArea::ALL
            .into_iter()
            .filter(|area| area.durability() == Durability::Canonical)
            .collect()
    }
}

/// Why an object is being deleted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeletionBasis {
    /// Recovering space. Subject to the ledger's retained window.
    Reclaim,
    /// Compelled deletion under 12.22's privacy rules. Permitted regardless of the window,
    /// because the obligation overrides the promise; the tombstone is what remains.
    Lawful,
}

impl DeletionBasis {
    pub fn name(self) -> &'static str {
        match self {
            DeletionBasis::Reclaim => "reclaim",
            DeletionBasis::Lawful => "lawful",
        }
    }
}

/// A managed object's record. Never its bytes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObjectRecord {
    pub id: ObjectId,
    pub digest: ContentHash,
    pub bytes: u64,
    pub classification: Classification,
    pub residency: Residency,
    /// When the system learned of this object, on the ledger's record axis. This is the field the
    /// retained-window check reads, which is why it is a [`RecordTime`] and not an [`Epoch`].
    pub created: RecordTime,
    /// Pinned by a release, result, review, incident or hold. Never swept.
    pub pinned: bool,
}

/// What remains after a deletion.
///
/// Carries the digest but not the content, which is what makes it "minimal non-identifying" in
/// 12.22's sense: it proves which object was deleted without revealing what the object said.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tombstone {
    pub id: ObjectId,
    pub digest: ContentHash,
    pub bytes_reclaimed: u64,
    pub classification: Classification,
    pub residency: Residency,
    pub basis: DeletionBasis,
    pub reason: String,
    pub at: Epoch,
    /// Manifests that still name this object. Non-empty is normal and is the point: those
    /// manifests were not rewritten.
    pub still_referenced_by: BTreeSet<String>,
}

/// What to sweep, and whether to actually do it.
///
/// `dry_run` defaults to true, matching 12.22's "dry run is default" and `bioprism-ledger`'s
/// compaction policy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GcPolicy {
    pub dry_run: bool,
    /// Manifests treated as roots for this collection. Anything reachable from one survives.
    pub roots: BTreeSet<String>,
}

impl GcPolicy {
    pub fn from_roots(roots: impl IntoIterator<Item = impl Into<String>>) -> Self {
        GcPolicy {
            dry_run: true,
            roots: roots.into_iter().map(Into::into).collect(),
        }
    }

    /// Opts out of the dry run. Named for what it does rather than taking a bare boolean.
    pub fn applying(mut self) -> Self {
        self.dry_run = false;
        self
    }
}

/// Why each survivor survived, and what was swept.
///
/// Shaped after `bioprism-ledger`'s `CompactionReport` on purpose: the same operator reads both,
/// and 12.22 requires garbage collection to "verify no broken closures" and "publish audit"
/// rather than simply report a count.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GcReport {
    pub dry_run: bool,
    pub examined: usize,
    /// Named directly by a root manifest.
    pub retained_by_root: BTreeSet<ObjectId>,
    /// Reachable from a root through the manifest closure.
    pub retained_by_closure: BTreeSet<ObjectId>,
    /// Held by an explicit pin: release, result, review, incident, legal hold.
    pub retained_by_pin: BTreeSet<ObjectId>,
    /// Held because the ledger still promises to answer about the period they were created in.
    pub retained_by_retention_window: BTreeSet<ObjectId>,
    /// Unreachable and outside the window. Swept unless this was a dry run.
    pub swept: BTreeSet<ObjectId>,
    pub bytes_swept: u64,
    /// Manifests naming an object this collection does not hold. A non-empty set means the
    /// reference graph was already broken before collection ran, and the sweep is reported
    /// alongside it rather than being credited with a clean closure.
    pub dangling_references: BTreeMap<String, BTreeSet<String>>,
}

impl GcReport {
    pub fn retained(&self) -> usize {
        self.examined - self.swept.len()
    }

    /// True when every manifest reference resolved to a held or tombstoned object.
    pub fn closure_intact(&self) -> bool {
        self.dangling_references.is_empty()
    }
}

/// Object records, manifests, tombstones, and the ledger's retention window.
#[derive(Debug)]
pub struct Lifecycle {
    window: RetentionWindow,
    objects: BTreeMap<ObjectId, ObjectRecord>,
    tombstones: BTreeMap<ObjectId, Tombstone>,
    /// Manifest name to the objects and child manifests it names.
    manifests: BTreeMap<String, BTreeSet<String>>,
}

impl Lifecycle {
    /// Adopts a retention window from `bioprism-ledger`. There is no constructor that invents
    /// one, because a second retention model is exactly what this crate must not add.
    pub fn under(window: RetentionWindow) -> Self {
        Lifecycle {
            window,
            objects: BTreeMap::new(),
            tombstones: BTreeMap::new(),
            manifests: BTreeMap::new(),
        }
    }

    pub fn window(&self) -> &RetentionWindow {
        &self.window
    }

    /// Tightens the window after the ledger compacts further, using the ledger's own rule so the
    /// two can never disagree about which direction is safe.
    pub fn adopt_window(&mut self, window: RetentionWindow) {
        self.window = self.window.tighten(window);
    }

    pub fn register(&mut self, record: ObjectRecord) -> ObjectId {
        let id = record.id.clone();
        self.objects.insert(id.clone(), record);
        id
    }

    /// Declares that `manifest` names these objects and child manifests.
    pub fn register_manifest(
        &mut self,
        manifest: impl Into<String>,
        members: impl IntoIterator<Item = impl Into<String>>,
    ) -> Result<(), LifecycleError> {
        let manifest = well_formed("manifest name", &manifest.into())?;
        let members: BTreeSet<String> = members.into_iter().map(Into::into).collect();
        self.manifests.insert(manifest, members);
        Ok(())
    }

    /// Resolves an object.
    ///
    /// A deleted object answers [`LifecycleError::Tombstoned`], never "not found". The difference
    /// is the whole of 40.06's deletion clause: a result that names this object learns that it
    /// was deliberately destroyed and when, rather than that the storage layer has lost track.
    pub fn resolve(&self, id: &ObjectId) -> Result<&ObjectRecord, LifecycleError> {
        if let Some(tombstone) = self.tombstones.get(id) {
            return Err(LifecycleError::Tombstoned {
                object: id.as_str().to_string(),
                at: tombstone.at,
                reason: tombstone.reason.clone(),
            });
        }
        self.objects
            .get(id)
            .ok_or_else(|| LifecycleError::UnknownObject(id.as_str().to_string()))
    }

    pub fn tombstone(&self, id: &ObjectId) -> Option<&Tombstone> {
        self.tombstones.get(id)
    }

    pub fn tombstones(&self) -> &BTreeMap<ObjectId, Tombstone> {
        &self.tombstones
    }

    pub fn len(&self) -> usize {
        self.objects.len()
    }

    pub fn is_empty(&self) -> bool {
        self.objects.is_empty()
    }

    /// Whether reclamation of this object would strand a ledger answer.
    ///
    /// Separated from [`Lifecycle::delete`] so a capacity planner can ask "what could I reclaim"
    /// without attempting anything.
    pub fn admits_reclamation(&self, id: &ObjectId) -> Result<(), LifecycleError> {
        let record = self.resolve(id)?;
        match self.window.answerable_from_record {
            None => Err(LifecycleError::RetentionWindowUnrestricted {
                object: id.as_str().to_string(),
            }),
            Some(from) if record.created >= from => Err(LifecycleError::WithinRetainedWindow {
                object: id.as_str().to_string(),
                created: record.created.to_string(),
                retained_from: from.to_string(),
            }),
            Some(_) => Ok(()),
        }
    }

    /// Deletes an object, leaving a tombstone.
    ///
    /// [`DeletionBasis::Reclaim`] additionally requires the object to be outside the ledger's
    /// retained window and unreferenced by any manifest. [`DeletionBasis::Lawful`] requires
    /// neither, and its tombstone records the manifests that still name the object, so the
    /// impact of the compelled deletion is stated rather than discovered later.
    pub fn delete(
        &mut self,
        id: &ObjectId,
        basis: DeletionBasis,
        reason: impl Into<String>,
        at: Epoch,
    ) -> Result<Tombstone, LifecycleError> {
        let reason = well_formed("deletion reason", &reason.into())?;
        let referenced = self.referencing_manifests(id);

        if basis == DeletionBasis::Reclaim {
            self.admits_reclamation(id)?;
            if let Some(manifest) = referenced.iter().next() {
                return Err(LifecycleError::StillReferenced {
                    object: id.as_str().to_string(),
                    by: manifest.clone(),
                });
            }
        }

        let record = self.resolve(id)?.clone();
        let tombstone = Tombstone {
            id: record.id.clone(),
            digest: record.digest.clone(),
            bytes_reclaimed: record.bytes,
            classification: record.classification,
            residency: record.residency.clone(),
            basis,
            reason,
            at,
            still_referenced_by: referenced,
        };
        self.objects.remove(id);
        self.tombstones.insert(id.clone(), tombstone.clone());
        Ok(tombstone)
    }

    fn referencing_manifests(&self, id: &ObjectId) -> BTreeSet<String> {
        self.manifests
            .iter()
            .filter(|(_, members)| members.contains(id.as_str()))
            .map(|(name, _)| name.clone())
            .collect()
    }

    /// Everything reachable from the root manifests, following child manifests transitively.
    fn reachable(&self, roots: &BTreeSet<String>) -> (BTreeSet<String>, BTreeSet<String>) {
        let mut direct: BTreeSet<String> = BTreeSet::new();
        let mut transitive: BTreeSet<String> = BTreeSet::new();
        let mut seen_manifests: BTreeSet<String> = BTreeSet::new();
        let mut queue: VecDeque<(String, bool)> =
            roots.iter().map(|root| (root.clone(), true)).collect();

        while let Some((manifest, is_root)) = queue.pop_front() {
            if !seen_manifests.insert(manifest.clone()) {
                continue;
            }
            let Some(members) = self.manifests.get(&manifest) else {
                continue;
            };
            for member in members {
                if self.manifests.contains_key(member) {
                    queue.push_back((member.clone(), false));
                } else if is_root {
                    direct.insert(member.clone());
                } else {
                    transitive.insert(member.clone());
                }
            }
        }
        transitive.retain(|member| !direct.contains(member));
        (direct, transitive)
    }

    /// Reachability collection: mark from roots, then sweep.
    ///
    /// 12.22's sequence is "snapshot metadata → mark reachable → quarantine candidates → grace
    /// period → delete blocks → verify no broken closures → publish audit". Quarantine and grace
    /// need a clock and are absent; the rest is here, and the closure verification is reported as
    /// [`GcReport::dangling_references`] rather than asserted, so a caller checks the claim
    /// instead of believing this documentation.
    ///
    /// Sweeping does **not** produce tombstones. A tombstone is the record of a deliberate
    /// deletion of a known object; sweeping removes objects nothing refers to, and minting a
    /// tombstone for each would fill the audit trail with notices about things no result ever
    /// named. An object that a manifest references is never unreachable, so the two paths do not
    /// overlap.
    pub fn garbage_collect(&mut self, policy: &GcPolicy) -> GcReport {
        let (direct, transitive) = self.reachable(&policy.roots);

        let mut report = GcReport {
            dry_run: policy.dry_run,
            examined: self.objects.len(),
            ..GcReport::default()
        };

        for (name, members) in &self.manifests {
            let missing: BTreeSet<String> = members
                .iter()
                .filter(|member| {
                    if self.manifests.contains_key(*member) {
                        return false;
                    }
                    match ObjectId::parse((*member).clone()) {
                        Ok(id) => {
                            !self.objects.contains_key(&id) && !self.tombstones.contains_key(&id)
                        }
                        Err(_) => true,
                    }
                })
                .cloned()
                .collect();
            if !missing.is_empty() {
                report.dangling_references.insert(name.clone(), missing);
            }
        }

        let mut sweep: Vec<ObjectId> = Vec::new();
        for (id, record) in &self.objects {
            let name = id.as_str().to_string();
            if direct.contains(&name) {
                report.retained_by_root.insert(id.clone());
            } else if transitive.contains(&name) {
                report.retained_by_closure.insert(id.clone());
            } else if record.pinned {
                report.retained_by_pin.insert(id.clone());
            } else if self.admits_reclamation(id).is_err() {
                report.retained_by_retention_window.insert(id.clone());
            } else {
                report.swept.insert(id.clone());
                report.bytes_swept += record.bytes;
                sweep.push(id.clone());
            }
        }

        if !policy.dry_run {
            for id in sweep {
                self.objects.remove(&id);
            }
        }

        report
    }
}
