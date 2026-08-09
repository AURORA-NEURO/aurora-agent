//! Backup and restore that never claims integrity it did not check.
//!
//! Blueprint 12.18 lists what to protect, in priority order: "signing keys and revocation state;
//! registry manifests; result bundles; event metadata; private pack metadata; unique artifacts;
//! rebuildable indexes and caches". 12.19 gives the restore sequence — "restore trust roots and
//! metadata first, verify object digest closure, then rebuild indexes and resume workers" — and
//! one sentence that is the actual contract: **"do not claim result integrity until closure
//! checks pass."**
//!
//! # Why a verdict rather than a boolean
//!
//! A restore that finished is not a restore that worked. The three things that can be true after
//! one are different, and a boolean merges the last two:
//!
//! - every object came back and every manifest reference resolves — [`RestoreVerdict::Verified`];
//! - objects came back but some manifest names a child that did not — [`RestoreVerdict::ClosureBroken`],
//!   with the missing children listed per manifest;
//! - objects came back and were *not checkable*, because the backup carried a digest without the
//!   bytes to recompute it — [`RestoreVerdict::Unverified`].
//!
//! The third is the one worth having. A backup that stored digests for artifacts held elsewhere
//! restores instantly and proves nothing, and reporting it as success is how an organisation
//! discovers during an incident that its disaster recovery was a manifest of promises. The same
//! refusal `bioprism-lens` makes about unchecked evidence, applied to a restore.
//!
//! # Order
//!
//! [`BackupClass::restore_order`] encodes 12.19's sequence, and [`BackupSet::restore`] refuses a
//! request that runs classes out of order. Trust roots before metadata before artifacts before
//! projections is not a preference: restoring artifacts before the catalog that authorizes them
//! means the authorization step has nothing to check against.
//!
//! # Deliberately not implemented
//!
//! No bytes are copied anywhere — a "backup" here is a set of `(name, digest)` pairs with a flag
//! saying whether the content was captured, and verification compares recorded digests rather
//! than rehashing content this crate does not hold. No encryption, no key custody, no
//! cryptographic erasure, no replication, no regions, no RPO or RTO measurement, no scheduling.
//! 12.19's portability export and its "failure communication" section are absent entirely.

use crate::error::BackupError;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// What a backed-up thing is, in 12.18's priority order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum BackupClass {
    /// Signing keys and revocation state. Nothing else can be trusted until these are back.
    TrustRoots,
    /// The relational catalog, registry manifests and audit log.
    Catalog,
    /// The append-only event log.
    EventLog,
    /// Unique artifacts and result bundles: the irreproducible evidence.
    Artifacts,
    /// Search, graph and analytical projections. Rebuildable from the classes above.
    Projections,
}

impl BackupClass {
    pub const ALL: [BackupClass; 5] = [
        BackupClass::TrustRoots,
        BackupClass::Catalog,
        BackupClass::EventLog,
        BackupClass::Artifacts,
        BackupClass::Projections,
    ];

    pub fn name(self) -> &'static str {
        match self {
            BackupClass::TrustRoots => "trust-roots",
            BackupClass::Catalog => "catalog",
            BackupClass::EventLog => "event-log",
            BackupClass::Artifacts => "artifacts",
            BackupClass::Projections => "projections",
        }
    }

    /// Position in 12.19's restore sequence. Lower restores first.
    pub fn restore_order(self) -> u8 {
        match self {
            BackupClass::TrustRoots => 0,
            BackupClass::Catalog => 1,
            BackupClass::EventLog => 2,
            BackupClass::Artifacts => 3,
            BackupClass::Projections => 4,
        }
    }

    /// Whether losing this class is recoverable by rebuilding rather than by restoring.
    ///
    /// A missing projection is a rebuild task; a missing artifact is an incident. A restore that
    /// treated them alike would either panic over an index or shrug at lost evidence.
    pub fn is_rebuildable(self) -> bool {
        matches!(self, BackupClass::Projections)
    }
}

impl fmt::Display for BackupClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// One backed-up item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackedUpItem {
    pub name: String,
    pub digest: String,
    /// False when the backup recorded the digest but not the content — a pointer to something
    /// held elsewhere. Such an item restores without being checkable, and that is what
    /// [`RestoreVerdict::Unverified`] reports.
    pub content_captured: bool,
}

/// A backup: items by class, plus the reference closure the restore must satisfy.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackupSet {
    items: BTreeMap<BackupClass, BTreeMap<String, BackedUpItem>>,
    /// Manifest name to the item names it requires. 12.05's "bundle manifest lists every required
    /// child digest and role"; roles are not modelled.
    closure: BTreeMap<String, BTreeSet<String>>,
}

impl BackupSet {
    pub fn new() -> Self {
        BackupSet::default()
    }

    pub fn with_item(
        mut self,
        class: BackupClass,
        name: impl Into<String>,
        digest: impl Into<String>,
        content_captured: bool,
    ) -> Result<Self, BackupError> {
        let name = name.into();
        let digest = digest.into();
        if name.trim().is_empty() {
            return Err(BackupError::MalformedField {
                field: "item name",
                value: name,
            });
        }
        if digest.trim().is_empty() {
            return Err(BackupError::MalformedField {
                field: "item digest",
                value: digest,
            });
        }
        self.items.entry(class).or_default().insert(
            name.clone(),
            BackedUpItem {
                name,
                digest,
                content_captured,
            },
        );
        Ok(self)
    }

    /// Declares that `manifest` requires these children to be present after a restore.
    pub fn requiring(
        mut self,
        manifest: impl Into<String>,
        children: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.closure.insert(
            manifest.into(),
            children.into_iter().map(Into::into).collect(),
        );
        self
    }

    pub fn items(&self, class: BackupClass) -> BTreeMap<&str, &BackedUpItem> {
        self.items
            .get(&class)
            .map(|items| {
                items
                    .iter()
                    .map(|(name, item)| (name.as_str(), item))
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn len(&self) -> usize {
        self.items.values().map(BTreeMap::len).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Restores the named classes, in the order given.
    ///
    /// The order is checked, not sorted: silently reordering the caller's request would hide a
    /// runbook that has the sequence wrong, and the runbook is what gets followed at three in the
    /// morning.
    pub fn restore(&self, order: &[BackupClass]) -> Result<RestoreReport, BackupError> {
        for window in order.windows(2) {
            if window[0].restore_order() > window[1].restore_order() {
                return Err(BackupError::OutOfOrder {
                    earlier: window[1].name(),
                    later: window[0].name(),
                });
            }
        }

        let mut restored: BTreeMap<BackupClass, usize> = BTreeMap::new();
        let mut present: BTreeSet<String> = BTreeSet::new();
        let mut unverifiable: BTreeSet<String> = BTreeSet::new();

        for class in order {
            let items = self.items.get(class).cloned().unwrap_or_default();
            restored.insert(*class, items.len());
            for (name, item) in items {
                present.insert(name.clone());
                if !item.content_captured {
                    unverifiable.insert(name);
                }
            }
        }

        let mut missing_children: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for (manifest, children) in &self.closure {
            let missing: BTreeSet<String> = children
                .iter()
                .filter(|child| !present.contains(*child))
                .cloned()
                .collect();
            if !missing.is_empty() {
                missing_children.insert(manifest.clone(), missing);
            }
        }

        let classes_not_restored: BTreeSet<BackupClass> = BackupClass::ALL
            .into_iter()
            .filter(|class| !order.contains(class))
            .collect();
        let rebuild_required: BTreeSet<BackupClass> = classes_not_restored
            .iter()
            .copied()
            .filter(|class| class.is_rebuildable())
            .collect();
        let evidence_not_restored: BTreeSet<BackupClass> = classes_not_restored
            .into_iter()
            .filter(|class| !class.is_rebuildable())
            .collect();

        let verdict = if !missing_children.is_empty() {
            RestoreVerdict::ClosureBroken {
                manifests: missing_children.keys().cloned().collect(),
            }
        } else if !unverifiable.is_empty() {
            RestoreVerdict::Unverified {
                items: unverifiable.clone(),
            }
        } else if !evidence_not_restored.is_empty() {
            RestoreVerdict::Unverified {
                items: evidence_not_restored
                    .iter()
                    .map(|class| format!("class:{class}"))
                    .collect(),
            }
        } else {
            RestoreVerdict::Verified {
                objects: present.len(),
            }
        };

        Ok(RestoreReport {
            order: order.to_vec(),
            restored,
            missing_children,
            unverifiable,
            rebuild_required,
            evidence_not_restored,
            verdict,
        })
    }
}

/// Whether a restore may be described as intact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RestoreVerdict {
    /// Every reference resolved and every restored item was checkable.
    Verified { objects: usize },
    /// A manifest names a child that is not present. This is the failure 12.19 forbids claiming
    /// integrity through.
    ClosureBroken { manifests: BTreeSet<String> },
    /// Nothing is known to be broken, and something could not be checked. Not a success.
    Unverified { items: BTreeSet<String> },
}

impl RestoreVerdict {
    /// True only for [`RestoreVerdict::Verified`].
    pub fn is_verified(&self) -> bool {
        matches!(self, RestoreVerdict::Verified { .. })
    }

    pub fn name(&self) -> &'static str {
        match self {
            RestoreVerdict::Verified { .. } => "verified",
            RestoreVerdict::ClosureBroken { .. } => "closure-broken",
            RestoreVerdict::Unverified { .. } => "unverified",
        }
    }
}

/// What a restore did and what it could not vouch for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RestoreReport {
    pub order: Vec<BackupClass>,
    pub restored: BTreeMap<BackupClass, usize>,
    /// Manifest to the children that did not come back.
    pub missing_children: BTreeMap<String, BTreeSet<String>>,
    /// Items whose content was not captured, so their digests could not be checked.
    pub unverifiable: BTreeSet<String>,
    /// Classes not restored that can be rebuilt from those that were. A task, not an incident.
    pub rebuild_required: BTreeSet<BackupClass>,
    /// Classes not restored that cannot be rebuilt. An incident.
    pub evidence_not_restored: BTreeSet<BackupClass>,
    pub verdict: RestoreVerdict,
}

impl RestoreReport {
    pub fn total_restored(&self) -> usize {
        self.restored.values().sum()
    }
}
