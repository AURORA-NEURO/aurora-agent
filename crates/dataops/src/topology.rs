//! Storage architecture (12.02): which class of data lives in which store, and what that store
//! is allowed to promise.
//!
//! 12.02's detailed design is five headings — Metadata, Artifacts, Events, Analytics, Search —
//! each naming a technology for local use and a technology for team use. Read as a shopping list
//! it is not implementable here: this crate has no SQLite, no S3 and no DuckDB. Read as what it
//! actually is, it is an **assignment from data class to store together with a set of promises**,
//! and the promises are checkable. That is the half implemented below.
//!
//! # The three promises the section makes and never enforces
//!
//! - *"Preserve immutable evidence."* Artifacts and events are evidence. A topology that puts
//!   them in a store which permits rewriting has broken the responsibility while satisfying every
//!   other sentence in the module. [`TopologyDraft::check`] refuses it.
//! - *"Search: structured indexes plus optional full-text/vector representations **that can be
//!   rebuilt from canonical metadata and authorized content**."* Rebuildability is only true if
//!   the thing it rebuilds from is itself canonical and present. A topology whose search index
//!   rebuilds from a metadata store that is itself rebuildable has no bottom, and the section
//!   does not say so.
//! - *"Support local and team deployments."* Two deployments are only the same platform if the
//!   promises match even though the technologies differ. [`parity`] is that comparison; without
//!   it, "supports local and team" is satisfied by any two unrelated topologies.
//!
//! # Where the freshness question enters
//!
//! A value read out of the analytics tables and the same value read out of the catalog are the
//! same bytes and different facts. [`StorageTopology::attest`] stamps the first
//! [`Basis::Derived`] and the second [`Basis::FirstHand`], so the two cannot be compared equal,
//! logged interchangeably, or fed to a decision that needed the canonical one. The lag is
//! supplied by the caller, because this crate has no clock and no replication monitor.
//!
//! # Not implemented
//!
//! No storage engine of any kind: no SQLite, no PostgreSQL, no object store, no Parquet writer,
//! no filesystem access at all. Nothing here holds bytes; [`StorageTopology`] holds statements
//! *about* where bytes would go. Encryption and retention are named in 12.02's responsibilities
//! and are not modelled — `bioprism-infra` carries the retention boundary and this crate does not
//! duplicate it. The content-addressed store itself is `bioprism-store`'s.

use crate::basis::{Attested, Basis, Coverage};
use crate::error::{check_name, TopologyError};
use bioprism_infra::{Durability, Epoch};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// The five classes 12.02 names, and no others.
///
/// A closed enum rather than a string, so a topology cannot forget one: [`DataClass::ALL`] is
/// what [`TopologyDraft::check`] iterates, and adding a sixth class breaks every match in the
/// crate rather than silently going unassigned.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DataClass {
    /// Resources, manifests, experiments, trials, policies, identities, registry state.
    Metadata,
    /// Traces, snapshots, packs, logs, outputs, bundles.
    Artifact,
    /// Append-only event segments.
    Event,
    /// Result tables for analytical queries.
    Analytics,
    /// Structured indexes and optional full-text or vector representations.
    Search,
}

impl DataClass {
    pub const ALL: [DataClass; 5] = [
        DataClass::Metadata,
        DataClass::Artifact,
        DataClass::Event,
        DataClass::Analytics,
        DataClass::Search,
    ];

    pub fn name(self) -> &'static str {
        match self {
            DataClass::Metadata => "metadata",
            DataClass::Artifact => "artifact",
            DataClass::Event => "event",
            DataClass::Analytics => "analytics",
            DataClass::Search => "search",
        }
    }

    /// Whether losing a write to this class destroys evidence rather than costing a rebuild.
    ///
    /// 12.02's responsibility list says "preserve immutable evidence" without saying which of the
    /// five classes *is* evidence. Artifacts are the traces and outputs a result is reconstructed
    /// from and events are the record of what happened; both are named in the invariant that
    /// every reported result links to an immutable run. Analytics and search are explicitly
    /// derivable. Metadata is the ambiguous one and is treated as non-evidentiary here, because
    /// 12.02 puts it in a relational database whose rows are updated — the append-only part of
    /// it is the status history, which is 12.03's constraint and not this module's.
    pub fn holds_immutable_evidence(self) -> bool {
        matches!(self, DataClass::Artifact | DataClass::Event)
    }
}

impl fmt::Display for DataClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// What a store permits to happen to a record already in it.
///
/// Separate from [`Durability`], which says what happens if the store is lost. A rebuildable
/// store may still be append-only, and a canonical store may still be mutable; conflating the two
/// is how a "durable" catalog ends up silently rewriting a published row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mutability {
    /// Written once, never changed, never deleted while referenced.
    Immutable,
    /// New records only; existing records are never edited.
    AppendOnly,
    /// Records may be updated in place.
    Mutable,
}

impl Mutability {
    pub fn preserves_evidence(self) -> bool {
        matches!(self, Mutability::Immutable | Mutability::AppendOnly)
    }

    pub fn name(self) -> &'static str {
        match self {
            Mutability::Immutable => "immutable",
            Mutability::AppendOnly => "append-only",
            Mutability::Mutable => "mutable",
        }
    }
}

/// Which of 12.02's two deployment shapes a topology describes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Deployment {
    /// One developer machine, embedded backends.
    Local,
    /// A shared installation with server backends.
    Team,
}

impl Deployment {
    pub fn name(self) -> &'static str {
        match self {
            Deployment::Local => "local",
            Deployment::Team => "team",
        }
    }
}

/// The pair of promises a data class is entitled to, independent of the technology holding it.
///
/// This is what [`parity`] compares. Two deployments that name completely different products may
/// still be the same platform; two that agree on the product and disagree here are not.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Promises {
    pub durability: Durability,
    pub mutability: Mutability,
}

/// A named store, its technology, and what it promises.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoreProfile {
    name: String,
    technology: String,
    durability: Durability,
    mutability: Mutability,
    rebuilt_from: BTreeSet<DataClass>,
}

impl StoreProfile {
    /// A store that is the last copy of what it holds.
    pub fn canonical(
        name: impl Into<String>,
        technology: impl Into<String>,
        mutability: Mutability,
    ) -> Result<Self, TopologyError> {
        Self::build(name, technology, Durability::Canonical, mutability, [])
    }

    /// A store that can be reconstructed from the named classes.
    ///
    /// The source list is required and is checked against the rest of the topology, so
    /// "rebuildable" cannot be asserted without saying rebuildable *from what*. An empty list is
    /// rejected at [`TopologyDraft::check`] time rather than here, because whether it is
    /// satisfiable depends on the other stores.
    pub fn rebuildable(
        name: impl Into<String>,
        technology: impl Into<String>,
        mutability: Mutability,
        rebuilt_from: impl IntoIterator<Item = DataClass>,
    ) -> Result<Self, TopologyError> {
        Self::build(
            name,
            technology,
            Durability::Rebuildable,
            mutability,
            rebuilt_from,
        )
    }

    /// A store valid only for the life of a process.
    pub fn ephemeral(
        name: impl Into<String>,
        technology: impl Into<String>,
        rebuilt_from: impl IntoIterator<Item = DataClass>,
    ) -> Result<Self, TopologyError> {
        Self::build(
            name,
            technology,
            Durability::Ephemeral,
            Mutability::Mutable,
            rebuilt_from,
        )
    }

    fn build(
        name: impl Into<String>,
        technology: impl Into<String>,
        durability: Durability,
        mutability: Mutability,
        rebuilt_from: impl IntoIterator<Item = DataClass>,
    ) -> Result<Self, TopologyError> {
        let name = name.into();
        let technology = technology.into();
        if !check_name(&name) {
            return Err(TopologyError::MalformedField {
                field: "store name",
                value: name,
            });
        }
        if !check_name(&technology) {
            return Err(TopologyError::MalformedField {
                field: "technology",
                value: technology,
            });
        }
        Ok(StoreProfile {
            name,
            technology,
            durability,
            mutability,
            rebuilt_from: rebuilt_from.into_iter().collect(),
        })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn technology(&self) -> &str {
        &self.technology
    }

    pub fn durability(&self) -> Durability {
        self.durability
    }

    pub fn mutability(&self) -> Mutability {
        self.mutability
    }

    pub fn rebuilt_from(&self) -> &BTreeSet<DataClass> {
        &self.rebuilt_from
    }

    pub fn promises(&self) -> Promises {
        Promises {
            durability: self.durability,
            mutability: self.mutability,
        }
    }
}

/// A topology under construction. Cannot answer questions.
///
/// The split between this and [`StorageTopology`] is the enforcement mechanism: `attest` and
/// `promises` exist only on the checked type, so there is no path by which an unvalidated
/// assignment produces a basis. Constructing the draft is infallible in the interesting cases;
/// the refusals all live in [`TopologyDraft::check`], where the whole picture is visible.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TopologyDraft {
    stores: Vec<StoreProfile>,
    assignment: BTreeMap<DataClass, String>,
}

impl TopologyDraft {
    pub fn new() -> Self {
        TopologyDraft::default()
    }

    pub fn with_store(mut self, profile: StoreProfile) -> Self {
        self.stores.push(profile);
        self
    }

    pub fn assign(mut self, class: DataClass, store: impl Into<String>) -> Self {
        self.assignment.insert(class, store.into());
        self
    }

    /// Checks every promise 12.02 makes, and returns a topology that can be read from.
    ///
    /// Refusals are ordered so the most structural comes first: a duplicate store name makes
    /// every later check ambiguous, an unassigned class makes rebuild-source resolution
    /// impossible, and only then are the evidence and rebuild rules meaningful.
    pub fn check(self, deployment: Deployment) -> Result<StorageTopology, TopologyError> {
        let mut stores: BTreeMap<String, StoreProfile> = BTreeMap::new();
        for profile in self.stores {
            if stores.contains_key(profile.name()) {
                return Err(TopologyError::DuplicateStore {
                    name: profile.name().to_string(),
                });
            }
            stores.insert(profile.name().to_string(), profile);
        }

        for class in DataClass::ALL {
            let Some(store_name) = self.assignment.get(&class) else {
                return Err(TopologyError::ClassUnassigned { class: class.name() });
            };
            if !stores.contains_key(store_name) {
                return Err(TopologyError::UndeclaredStore {
                    class: class.name(),
                    store: store_name.clone(),
                });
            }
        }

        for class in DataClass::ALL {
            let store = &stores[&self.assignment[&class]];
            if class.holds_immutable_evidence() && !store.mutability().preserves_evidence() {
                return Err(TopologyError::EvidenceStoreIsMutable {
                    class: class.name(),
                    store: store.name().to_string(),
                });
            }
            if store.durability() == Durability::Canonical {
                continue;
            }
            if store.rebuilt_from().is_empty() {
                return Err(TopologyError::NoCanonicalHolder {
                    class: class.name(),
                    store: store.name().to_string(),
                });
            }
            if store.rebuilt_from().contains(&class) {
                return Err(TopologyError::RebuildCycle {
                    store: store.name().to_string(),
                });
            }
            for source in store.rebuilt_from() {
                let source_store = &stores[&self.assignment[source]];
                if source_store.durability() != Durability::Canonical {
                    return Err(TopologyError::RebuildSourceNotCanonical {
                        store: store.name().to_string(),
                        from: source.name(),
                    });
                }
            }
        }

        Ok(StorageTopology {
            deployment,
            stores,
            assignment: self.assignment,
        })
    }
}

/// A checked assignment of every data class to a store.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StorageTopology {
    deployment: Deployment,
    stores: BTreeMap<String, StoreProfile>,
    assignment: BTreeMap<DataClass, String>,
}

impl StorageTopology {
    pub fn deployment(&self) -> Deployment {
        self.deployment
    }

    pub fn store_for(&self, class: DataClass) -> &StoreProfile {
        &self.stores[&self.assignment[&class]]
    }

    pub fn promises(&self, class: DataClass) -> Promises {
        self.store_for(class).promises()
    }

    /// Every technology this topology names, sorted.
    ///
    /// Used by 12.13's fallback check, which has to answer "does the local path require Kafka"
    /// without knowing anything else about the deployment.
    pub fn technologies(&self) -> BTreeSet<&str> {
        self.stores
            .values()
            .map(|store| store.technology())
            .collect()
    }

    /// The basis a read from this class carries.
    ///
    /// `lag_epochs` is what the caller believes the derived store is behind by; it is not
    /// measured here and nothing in this crate can measure it. A canonical store ignores it,
    /// which is why the argument is accepted unconditionally rather than being an `Option` the
    /// caller could omit and thereby claim currency by silence.
    pub fn basis_for(&self, class: DataClass, at: Epoch, lag_epochs: u64) -> Basis {
        let store = self.store_for(class);
        match store.durability() {
            Durability::Canonical => Basis::FirstHand { observed_at: at },
            Durability::Rebuildable | Durability::Ephemeral => Basis::Derived {
                source: store.name().to_string(),
                lag_epochs,
            },
        }
    }

    /// Wraps a value read from `class` in the basis that read actually had.
    ///
    /// Coverage is the caller's, because the topology does not know how much of a query's
    /// population an answer covered — only 12.12's telemetry and 12.03's outbox do.
    pub fn attest<T>(
        &self,
        class: DataClass,
        value: T,
        at: Epoch,
        lag_epochs: u64,
        coverage: Coverage,
    ) -> Attested<T> {
        Attested::new(value, self.basis_for(class, at, lag_epochs), coverage)
    }
}

/// One class on which two deployments promise different things.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PromiseDifference {
    pub class: DataClass,
    pub left: Promises,
    pub right: Promises,
    pub left_technology: String,
    pub right_technology: String,
}

/// The result of comparing two deployments class by class.
///
/// `compared` is reported alongside the differences because an empty difference list over zero
/// comparisons and an empty difference list over five are the same value and not the same
/// evidence — the same reason `bioprism-infra`'s check outcome reports how many values it
/// examined.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParityReport {
    pub compared: usize,
    pub differences: Vec<PromiseDifference>,
}

impl ParityReport {
    pub fn holds(&self) -> bool {
        self.differences.is_empty() && self.compared == DataClass::ALL.len()
    }
}

/// Compares the promises of two deployments class by class.
///
/// Technology differences are recorded and never reported as a difference in themselves: 12.02
/// *requires* SQLite locally and PostgreSQL for teams, so a comparison that flagged that would
/// flag the specification.
pub fn parity(left: &StorageTopology, right: &StorageTopology) -> ParityReport {
    let mut differences = Vec::new();
    for class in DataClass::ALL {
        let (left_store, right_store) = (left.store_for(class), right.store_for(class));
        if left_store.promises() != right_store.promises() {
            differences.push(PromiseDifference {
                class,
                left: left_store.promises(),
                right: right_store.promises(),
                left_technology: left_store.technology().to_string(),
                right_technology: right_store.technology().to_string(),
            });
        }
    }
    ParityReport {
        compared: DataClass::ALL.len(),
        differences,
    }
}

/// The topology 12.02's detailed design describes for one machine.
///
/// Written out because the section states it as prose and prose cannot be tested. If a later
/// reading of 12.02 disagrees with this, the disagreement is visible here rather than spread
/// through a dozen call sites.
pub fn reference_local() -> Result<StorageTopology, TopologyError> {
    TopologyDraft::new()
        .with_store(StoreProfile::canonical(
            "catalog",
            "sqlite",
            Mutability::Mutable,
        )?)
        .with_store(StoreProfile::canonical(
            "cas",
            "filesystem-cas",
            Mutability::Immutable,
        )?)
        .with_store(StoreProfile::canonical(
            "eventlog",
            "append-only-segments",
            Mutability::AppendOnly,
        )?)
        .with_store(StoreProfile::rebuildable(
            "results",
            "parquet-duckdb",
            Mutability::Mutable,
            [DataClass::Event, DataClass::Metadata],
        )?)
        .with_store(StoreProfile::rebuildable(
            "index",
            "structured-index",
            Mutability::Mutable,
            [DataClass::Metadata, DataClass::Artifact],
        )?)
        .assign(DataClass::Metadata, "catalog")
        .assign(DataClass::Artifact, "cas")
        .assign(DataClass::Event, "eventlog")
        .assign(DataClass::Analytics, "results")
        .assign(DataClass::Search, "index")
        .check(Deployment::Local)
}

/// The same promises on team technologies.
pub fn reference_team() -> Result<StorageTopology, TopologyError> {
    TopologyDraft::new()
        .with_store(StoreProfile::canonical(
            "catalog",
            "postgresql",
            Mutability::Mutable,
        )?)
        .with_store(StoreProfile::canonical(
            "cas",
            "s3-compatible",
            Mutability::Immutable,
        )?)
        .with_store(StoreProfile::canonical(
            "eventlog",
            "object-store-segments",
            Mutability::AppendOnly,
        )?)
        .with_store(StoreProfile::rebuildable(
            "results",
            "analytical-engine",
            Mutability::Mutable,
            [DataClass::Event, DataClass::Metadata],
        )?)
        .with_store(StoreProfile::rebuildable(
            "index",
            "structured-index",
            Mutability::Mutable,
            [DataClass::Metadata, DataClass::Artifact],
        )?)
        .assign(DataClass::Metadata, "catalog")
        .assign(DataClass::Artifact, "cas")
        .assign(DataClass::Event, "eventlog")
        .assign(DataClass::Analytics, "results")
        .assign(DataClass::Search, "index")
        .check(Deployment::Team)
}
