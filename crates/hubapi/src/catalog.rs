//! One registry's catalog: what it holds, on whose authority, and how current its copy is.
//!
//! Blueprint 10.03 (Registry Entity and Metadata Model) and 10.04 between them require a name to
//! resolve to an exact manifest, dependencies to be pinned by digest, and a version to move
//! channels without changing its digest. This module holds the table those requirements are stated
//! over.
//!
//! # A version binding is immutable, and that is what makes a mirror possible
//!
//! Once `name@version` is bound to a digest it never binds to another. `bioprism-registry` already
//! enforces the same rule inside a single index; the reason it is restated here is that mirroring
//! depends on it entirely. If a binding could change, two registries answering the same question
//! at different moments could disagree legitimately, and there would be no way to tell a stale
//! mirror from a lying one. Because the binding is immutable, a disagreement in digest is never
//! staleness — it is [`crate::mirror::MirrorError::Divergent`], and a copy that does that is not a
//! copy.
//!
//! Note what is *not* immutable: availability and deprecation. A yank changes what a catalog will
//! recommend without changing what it holds, which is exactly 10.04's "a version may move channels
//! without changing its digest". Those live in [`crate::lifecycle`] and are consulted at
//! resolution time rather than baked into the binding.
//!
//! # The tier is the answering registry's opinion
//!
//! [`PackRelease::tier`] is a [`bioprism_registry::TrustTier`], and it is *that registry's*
//! assessment. `bioprism-registry` computes a tier from a pack document; nothing in this crate
//! recomputes it, and copying it across a federation boundary is precisely the move
//! [`crate::federation`] refuses. A tier in a catalog says "this registry says so", never "this is
//! so".
//!
//! # Not implemented
//!
//! No storage, no index, no archive format, no SBOM. A [`Catalog`] is an in-memory value that
//! serialises to JSON; how a deployment persists, ships or serves one is 10.02's problem and not
//! this crate's. No content-addressed store either: a digest here is a string that came from
//! somewhere else, and nothing in this crate hashes an artifact.

use crate::lifecycle::PackLifecycle;
use crate::mirror::Replication;
use crate::name::{PackName, Version, VersionReq};
use crate::registry::{Authority, AuthorityError, NameAuthority, RegistryId};
use bioprism_registry::TrustTier;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// One pack depending on another.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Dependency {
    pub name: PackName,
    pub req: VersionReq,
}

impl Dependency {
    pub fn new(name: PackName, req: VersionReq) -> Self {
        Dependency { name, req }
    }
}

impl fmt::Display for Dependency {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.name, self.req)
    }
}

/// A published version of a pack, as one registry holds it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackRelease {
    pub name: PackName,
    pub version: Version,
    /// The artifact digest. Produced by `bioprism-registry`; treated here as an opaque identity
    /// that two registries must agree on or be in conflict about.
    pub digest: String,
    pub dependencies: Vec<Dependency>,
    /// One line, for a search result to show. 10.11 wants far more metadata than this; the rest of
    /// it is the pack document's, which this crate does not hold.
    pub summary: String,
    pub keywords: BTreeSet<String>,
    /// The answering registry's assessment. See the module docs.
    pub tier: TrustTier,
}

impl PackRelease {
    pub fn new(name: PackName, version: Version, digest: impl Into<String>) -> Self {
        PackRelease {
            name,
            version,
            digest: digest.into(),
            dependencies: Vec::new(),
            summary: String::new(),
            keywords: BTreeSet::new(),
            tier: TrustTier::Unranked,
        }
    }

    pub fn depending_on(mut self, name: PackName, req: VersionReq) -> Self {
        self.dependencies.push(Dependency::new(name, req));
        self
    }

    pub fn described(mut self, summary: impl Into<String>) -> Self {
        self.summary = summary.into();
        self
    }

    pub fn keyworded<I, S>(mut self, keywords: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.keywords
            .extend(keywords.into_iter().map(|keyword| keyword.into()));
        self
    }

    pub fn at_tier(mut self, tier: TrustTier) -> Self {
        self.tier = tier;
        self
    }
}

/// A registry's holdings, its standing, and the currency of its copy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Catalog {
    authority: Authority,
    sync: Replication,
    releases: BTreeMap<String, PackRelease>,
    lifecycle: PackLifecycle,
}

impl Catalog {
    /// A registry that holds its own publications.
    pub fn origin(authority: Authority) -> Self {
        Catalog {
            authority,
            sync: Replication::Origin,
            releases: BTreeMap::new(),
            lifecycle: PackLifecycle::new(),
        }
    }

    /// A registry that holds a copy of somebody else's, taken at a stated epoch under a stated
    /// bound. The bound is the mirror's own claim; see [`crate::mirror`].
    pub fn mirror(authority: Authority, sync: Replication) -> Self {
        Catalog {
            authority,
            sync,
            releases: BTreeMap::new(),
            lifecycle: PackLifecycle::new(),
        }
    }

    pub fn id(&self) -> &RegistryId {
        self.authority.registry()
    }

    pub fn authority(&self) -> &Authority {
        &self.authority
    }

    pub fn sync(&self) -> &Replication {
        &self.sync
    }

    pub fn lifecycle(&self) -> &PackLifecycle {
        &self.lifecycle
    }

    pub fn lifecycle_mut(&mut self) -> &mut PackLifecycle {
        &mut self.lifecycle
    }

    fn key(name: &PackName, version: &Version) -> String {
        format!("{name}@{version}")
    }

    /// Records a release, refusing anything the registry has no standing to hold and any attempt
    /// to rebind an existing version to different content.
    ///
    /// Re-recording the identical binding succeeds and changes nothing, because replication
    /// re-delivering a record it already delivered is normal and is not an error.
    pub fn record(&mut self, release: PackRelease) -> Result<(), CatalogError> {
        self.authority.standing_for(&release.name)?;
        if release.digest.trim().is_empty() {
            return Err(CatalogError::DigestMissing {
                subject: Catalog::key(&release.name, &release.version),
            });
        }
        if let Some(dependency) = release
            .dependencies
            .iter()
            .find(|dependency| dependency.name == release.name)
        {
            return Err(CatalogError::SelfDependency {
                subject: Catalog::key(&release.name, &release.version),
                req: dependency.req.to_string(),
            });
        }
        let key = Catalog::key(&release.name, &release.version);
        if let Some(existing) = self.releases.get(&key) {
            if existing.digest != release.digest {
                return Err(CatalogError::VersionAlreadyBound {
                    subject: key,
                    existing: existing.digest.clone(),
                    offered: release.digest,
                });
            }
            return Ok(());
        }
        self.releases.insert(key, release);
        Ok(())
    }

    pub fn release(&self, name: &PackName, version: &Version) -> Option<&PackRelease> {
        self.releases.get(&Catalog::key(name, version))
    }

    /// Every version of a name this registry holds, ascending.
    pub fn versions_of(&self, name: &PackName) -> Vec<Version> {
        let mut versions: Vec<Version> = self
            .releases
            .values()
            .filter(|release| &release.name == name)
            .map(|release| release.version)
            .collect();
        versions.sort();
        versions
    }

    pub fn releases(&self) -> impl Iterator<Item = &PackRelease> {
        self.releases.values()
    }

    pub fn len(&self) -> usize {
        self.releases.len()
    }

    pub fn is_empty(&self) -> bool {
        self.releases.is_empty()
    }

    pub fn holds(&self, name: &PackName) -> bool {
        self.releases.values().any(|release| &release.name == name)
    }

    /// This registry's standing for a name, from its own declaration alone. Use
    /// [`crate::registry::Federation::standing_for`] when the delegation itself needs checking.
    pub fn standing_for(&self, name: &PackName) -> Result<NameAuthority, AuthorityError> {
        self.authority.standing_for(name)
    }
}

/// Why a release was not recorded.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CatalogError {
    #[error(transparent)]
    Authority(#[from] AuthorityError),

    #[error("{subject} was offered with no digest; a name with no content behind it resolves to nothing")]
    DigestMissing { subject: String },

    #[error(
        "{subject} is already bound to {existing}; rebinding it to {offered} would make two \
         registries able to disagree about the same name legitimately"
    )]
    VersionAlreadyBound {
        subject: String,
        existing: String,
        offered: String,
    },

    #[error("{subject} declares a dependency on itself ({req})")]
    SelfDependency { subject: String, req: String },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mirror::StalenessBound;
    use crate::name::Namespace;
    use bioprism_hub::Epoch;

    fn ns(text: &str) -> Namespace {
        Namespace::parse(text).expect("parses")
    }

    fn id(text: &str) -> RegistryId {
        RegistryId::parse(text).expect("parses")
    }

    fn name(text: &str) -> PackName {
        PackName::parse(text).expect("parses")
    }

    fn origin_authority() -> Authority {
        Authority::new(id("origin"))
            .owning(ns("bioprism"))
            .expect("owns")
    }

    fn release(version: Version, digest: &str) -> PackRelease {
        PackRelease::new(name("bioprism/onco-tp53"), version, digest)
    }

    #[test]
    fn a_registry_cannot_record_a_release_in_a_namespace_it_has_no_standing_for() {
        let mut catalog = Catalog::origin(origin_authority());
        let error = catalog
            .record(PackRelease::new(
                name("elsewhere/other"),
                Version::new(1, 0, 0),
                "sha256:aa",
            ))
            .expect_err("holding is answering");
        assert!(matches!(error, CatalogError::Authority(_)));
    }

    #[test]
    fn a_version_binding_cannot_be_rewritten_to_different_content() {
        let mut catalog = Catalog::origin(origin_authority());
        catalog
            .record(release(Version::new(1, 0, 0), "sha256:aa"))
            .expect("records");
        let error = catalog
            .record(release(Version::new(1, 0, 0), "sha256:bb"))
            .expect_err("immutability is what makes a mirror checkable");
        assert!(matches!(error, CatalogError::VersionAlreadyBound { .. }));
        assert_eq!(
            catalog
                .release(&name("bioprism/onco-tp53"), &Version::new(1, 0, 0))
                .expect("still there")
                .digest,
            "sha256:aa"
        );
    }

    #[test]
    fn recording_the_identical_binding_twice_is_replication_and_not_an_error() {
        let mut catalog = Catalog::mirror(
            Authority::new(id("mirror"))
                .carrying(ns("bioprism"), id("origin"))
                .expect("carries"),
            Replication::mirror(id("origin"), Epoch(4), StalenessBound::epochs(2)),
        );
        catalog
            .record(release(Version::new(1, 0, 0), "sha256:aa"))
            .expect("records");
        catalog
            .record(release(Version::new(1, 0, 0), "sha256:aa"))
            .expect("a repeated delivery is normal");
        assert_eq!(catalog.len(), 1);
    }

    #[test]
    fn a_release_with_no_digest_is_refused() {
        let mut catalog = Catalog::origin(origin_authority());
        assert!(matches!(
            catalog.record(release(Version::new(1, 0, 0), "   ")),
            Err(CatalogError::DigestMissing { .. })
        ));
    }

    #[test]
    fn a_release_cannot_depend_on_itself() {
        let mut catalog = Catalog::origin(origin_authority());
        let error = catalog
            .record(release(Version::new(1, 0, 0), "sha256:aa").depending_on(
                name("bioprism/onco-tp53"),
                VersionReq::Compatible(Version::new(1, 0, 0)),
            ))
            .expect_err("a self-dependency has no fixpoint to resolve to");
        assert!(matches!(error, CatalogError::SelfDependency { .. }));
    }

    #[test]
    fn versions_come_back_in_order_regardless_of_the_order_they_were_recorded() {
        let mut catalog = Catalog::origin(origin_authority());
        for version in [
            Version::new(1, 10, 0),
            Version::new(1, 2, 0),
            Version::new(2, 0, 0),
        ] {
            catalog
                .record(release(version, &format!("sha256:{version}")))
                .expect("records");
        }
        assert_eq!(
            catalog
                .versions_of(&name("bioprism/onco-tp53"))
                .iter()
                .map(Version::to_string)
                .collect::<Vec<_>>(),
            ["1.2.0", "1.10.0", "2.0.0"]
        );
    }

    #[test]
    fn a_mirror_catalog_reports_a_sync_and_an_origin_catalog_does_not() {
        let origin = Catalog::origin(origin_authority());
        assert!(origin.sync().is_origin());

        let copy = Catalog::mirror(
            Authority::new(id("mirror"))
                .carrying(ns("bioprism"), id("origin"))
                .expect("carries"),
            Replication::mirror(id("origin"), Epoch(4), StalenessBound::epochs(2)),
        );
        assert!(!copy.sync().is_origin());
        assert!(copy.is_empty());
    }

    #[test]
    fn a_catalog_survives_a_json_round_trip() {
        let mut catalog = Catalog::origin(origin_authority());
        catalog
            .record(
                release(Version::new(1, 0, 0), "sha256:aa")
                    .described("TP53 decision cells")
                    .keyworded(["tp53", "onco"])
                    .at_tier(TrustTier::Reviewed),
            )
            .expect("records");
        let text = serde_json::to_string(&catalog).expect("serialises");
        let back: Catalog = serde_json::from_str(&text).expect("deserialises");
        assert_eq!(back, catalog);
    }
}
