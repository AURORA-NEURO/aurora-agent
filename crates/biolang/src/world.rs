//! BioWorld IR — blueprint 25.01.
//!
//! The top-level package: what state a world starts in, what assets it contains, what may be done
//! in it, what is hidden, what it costs, who evaluates it, and under what licence.
//!
//! # The invariants that are actually enforced
//!
//! - *Every asset has provenance and a resolvable locator.* [`Asset`] has no constructor that omits
//!   either, and [`BioWorld::validate`] refuses an empty one. "Resolvable" is checked as
//!   well-formedness only; there is no resolver here, for the same reason `bioprism-bioir` gives —
//!   there is no artifact-shape contract to resolve against.
//! - *Hidden labels are not reachable through participant tools.* A hidden item that also appears in
//!   the initial visible state, or that an action in the catalog produces, is refused. This is a
//!   reachability check over declarations, not an information-flow proof.
//! - *Counterfactual capability is explicitly graded.* [`CounterfactualGrade`] has no `Default`, so
//!   a world must state what kind of counterfactual it supports.
//!
//! # The invariant that is not
//!
//! *A world version is immutable after publication.* Immutability is a property of a registry, not
//! of a struct: nothing here can stop a caller mutating a `BioWorld` it owns. What this module
//! provides is the digest that makes a mutation *detectable* — [`crate::canonical::Canonical`] over
//! the whole world — and `bioprism-registry` is where publication lives. Saying the type enforces
//! immutability would be the kind of implied capability this workspace forbids.

use crate::error::WorldError;
use crate::ids::{ActionId, AssetId, StateId};
use bioprism_ids::{ContentHash, WorldId};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fmt;

/// A `major.minor.patch` version, parsed rather than trusted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SemanticVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl SemanticVersion {
    pub fn new(major: u32, minor: u32, patch: u32) -> Self {
        SemanticVersion {
            major,
            minor,
            patch,
        }
    }

    pub fn parse(value: &str) -> Result<Self, WorldError> {
        let malformed = || WorldError::MalformedVersion {
            value: value.to_string(),
        };
        let parts: Vec<&str> = value.split('.').collect();
        let [major, minor, patch] = parts.as_slice() else {
            return Err(malformed());
        };
        Ok(SemanticVersion::new(
            major.parse().map_err(|_| malformed())?,
            minor.parse().map_err(|_| malformed())?,
            patch.parse().map_err(|_| malformed())?,
        ))
    }
}

impl fmt::Display for SemanticVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// What kind of world this is. 25.01 requires "world class and intended use".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorldClass {
    /// Replay of something that happened.
    Observed,
    /// Generated, with a known generator.
    Synthetic,
    /// Outcomes not yet known to anyone.
    Prospective,
    /// Observed history with synthetic continuation.
    Hybrid,
}

/// How far counterfactual reasoning is supported. 25.01: "Counterfactual capability is explicitly
/// graded."
///
/// No `Default`. A world that does not say is a world whose counterfactual claims cannot be
/// interpreted, and the missing grade is what would be filled in optimistically.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CounterfactualGrade {
    /// Only the factual trajectory exists.
    None,
    /// Alternative actions may be taken, but their outcomes are not modelled.
    BranchOnly,
    /// Alternative outcomes come from a declared model.
    Modelled,
    /// Alternative outcomes were observed, e.g. a matched arm.
    Observed,
}

/// Where an asset came from and where it can be found.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Asset {
    pub asset_id: AssetId,
    /// The content hash the world is addressed by.
    pub digest: ContentHash,
    /// A locator string. Checked for non-emptiness only; nothing here resolves it.
    pub locator: String,
    /// Who produced it and how, in prose.
    pub provenance: String,
    /// Access labels a reader must hold. Empty means no label is required, which is a claim.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub required_labels: BTreeSet<String>,
}

impl Asset {
    pub fn new(
        asset_id: AssetId,
        digest: ContentHash,
        locator: impl Into<String>,
        provenance: impl Into<String>,
    ) -> Self {
        Asset {
            asset_id,
            digest,
            locator: locator.into(),
            provenance: provenance.into(),
            required_labels: BTreeSet::new(),
        }
    }

    pub fn requiring(mut self, label: impl Into<String>) -> Self {
        self.required_labels.insert(label.into());
        self
    }
}

/// What a participant can see at the start.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VisibleState {
    pub state_id: StateId,
    /// The asset ids reachable from it.
    pub exposed: BTreeSet<AssetId>,
}

/// Something the world holds back, and until when.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HiddenItem {
    pub asset_id: AssetId,
    /// Why it is hidden: a held-out label, a prospective outcome, a controlled record.
    pub reason: String,
}

/// An entry in the action catalog, as the world sees it.
///
/// The full action definition lives in [`crate::intervention`]; a world carries the id, the assets
/// the action can produce, and nothing else, so that the catalog can be checked for reachability of
/// prohibited items without loading every definition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CatalogEntry {
    pub action_id: ActionId,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub produces: BTreeSet<AssetId>,
}

/// Licence and access policy. 25.01 requires both.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LicensePolicy {
    pub license: String,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub access_labels: BTreeSet<String>,
    /// True when the world may be redistributed with its assets embedded.
    ///
    /// 25.01's security note is explicit that "controlled data may be referenced without being
    /// embedded in a public package", so this is a declaration a packager reads, not a permission
    /// this crate grants.
    pub embeddable: bool,
}

/// The world manifest.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BioWorld {
    pub world_id: WorldId,
    pub version: SemanticVersion,
    pub class: WorldClass,
    pub intended_use: String,
    /// Ontology and reference releases this world is coded in, as `name -> release`.
    pub standards: std::collections::BTreeMap<String, String>,
    pub assets: Vec<Asset>,
    pub visible: VisibleState,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hidden: Vec<HiddenItem>,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub prohibited: BTreeSet<AssetId>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub actions: Vec<CatalogEntry>,
    /// Resource kinds the world meters. Amounts belong to a state, not to the world.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub metered_resources: BTreeSet<String>,
    /// The oracle mesh, by identifier. Definitions live in [`crate::oracle`].
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub oracle_mesh: BTreeSet<String>,
    pub license: LicensePolicy,
    pub counterfactual: CounterfactualGrade,
}

impl BioWorld {
    /// Every invariant 25.01 states that a manifest can be checked against on its own.
    pub fn validate(&self) -> Result<(), WorldError> {
        for asset in &self.assets {
            if asset.locator.trim().is_empty() {
                return Err(WorldError::AssetUnderclared {
                    asset: asset.asset_id.to_string(),
                    missing: "locator".to_string(),
                });
            }
            if asset.provenance.trim().is_empty() {
                return Err(WorldError::AssetUnderclared {
                    asset: asset.asset_id.to_string(),
                    missing: "provenance".to_string(),
                });
            }
        }

        for item in &self.hidden {
            if self.visible.exposed.contains(&item.asset_id) {
                return Err(WorldError::HiddenItemVisible {
                    item: item.asset_id.to_string(),
                });
            }
        }

        for entry in &self.actions {
            for produced in &entry.produces {
                if self.prohibited.contains(produced) {
                    return Err(WorldError::ProhibitedItemReachable {
                        action: entry.action_id.to_string(),
                        item: produced.to_string(),
                    });
                }
                if self.hidden.iter().any(|item| &item.asset_id == produced) {
                    return Err(WorldError::HiddenItemVisible {
                        item: produced.to_string(),
                    });
                }
            }
        }

        Ok(())
    }

    /// Checks that every oracle the world cites is in its declared mesh.
    pub fn validate_oracles<'a, I>(&self, cited: I) -> Result<(), WorldError>
    where
        I: IntoIterator<Item = &'a str>,
    {
        for oracle in cited {
            if !self.oracle_mesh.contains(oracle) {
                return Err(WorldError::OracleNotInMesh {
                    oracle: oracle.to_string(),
                });
            }
        }
        Ok(())
    }

    pub fn asset(&self, id: &AssetId) -> Option<&Asset> {
        self.assets.iter().find(|asset| &asset.asset_id == id)
    }
}
