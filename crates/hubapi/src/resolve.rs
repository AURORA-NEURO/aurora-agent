//! Name resolution: an answer, plus who answered and on what standing.
//!
//! Blueprint 10.04 says "aliases resolve to exact manifests" and stops there. 10.02's local-first
//! contract and 10.18's federation both assume that sentence works across several registries
//! without saying what the answer is supposed to carry when it does. This module decides that.
//!
//! # The shape of the answer
//!
//! A [`Resolution`] has two halves that are kept apart on purpose:
//!
//! - [`Resolution::subject`] — the name, the version and the digest. This is the **result**, and
//!   two resolutions of the same question against the origin and against a mirror must produce
//!   equal subjects. [`Resolution::agrees_with`] compares exactly this.
//! - [`Resolution::provenance`] — which registry answered, whether it was the authority for that
//!   namespace, how current its copy is, what the consumer's freshness policy was, and every
//!   lifecycle note attached. This is what makes the two resolutions **distinguishable**.
//!
//! A consumer can therefore always ask the two questions that matter offline — *who answered* and
//! *were they entitled to* — and the type gives no way to obtain a subject without also having the
//! provenance that came with it.
//!
//! # A carrier is consulted only after the authority
//!
//! [`resolve_in`] asks the owning registry first and falls through to carriers. Not because a
//! mirror is untrustworthy, but because when both can answer, the answer that needs no caveat is
//! the better one to have recorded. When several catalogs hold the same `name@version`, their
//! digests are compared before an answer is returned: a disagreement there is never staleness (a
//! binding is immutable, see [`crate::catalog`]) and is reported as
//! [`crate::mirror::MirrorError::Divergent`].
//!
//! # Three ways to have no answer, and they are not the same
//!
//! [`ResolveError`] separates *this registry does not hold that name*, *it holds the name but no
//! version in your range*, and *versions in your range exist but the lifecycle excludes all of
//! them*. A resolver that collapsed those into one "not found" would leave the caller unable to
//! tell a typo from a yank from a constraint they wrote too narrowly.
//!
//! # Not implemented
//!
//! No fetching, no caching, no retries, no fallback ordering across network endpoints. Nothing in
//! this module reaches anything. A catalog is a value the caller already has, and "offline" is not
//! a mode it enters — it is the only mode there is.

use crate::catalog::Catalog;
use crate::lifecycle::{Intent, LifecycleError, Note};
use crate::mirror::{Freshness, FreshnessPolicy, MirrorError};
use crate::name::{PackName, Version, VersionReq};
use crate::registry::{AuthorityError, Federation, NameAuthority, RegistryId};
use bioprism_hub::Epoch;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Everything a caller has to say before a name can be resolved.
///
/// Bundled into one value rather than passed as five arguments because every field is a decision
/// the caller is making, and a caller that has not thought about [`Request::intent`] or
/// [`Request::freshness`] should have to notice that it has not.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Request {
    pub name: PackName,
    pub req: VersionReq,
    /// Whether the caller is choosing for the first time or honouring an existing commitment. See
    /// [`crate::lifecycle`] for why the same catalog answers these differently.
    pub intent: Intent,
    pub freshness: FreshnessPolicy,
    /// The reference epoch a mirror's staleness is judged against. `None` is the ordinary
    /// air-gapped case and produces [`Freshness::Undetermined`] rather than an optimistic default.
    pub as_of: Option<Epoch>,
}

impl Request {
    /// A first-time selection under the strict freshness default.
    pub fn new(name: PackName, req: VersionReq) -> Self {
        Request {
            name,
            req,
            intent: Intent::NewDependent,
            freshness: FreshnessPolicy::default(),
            as_of: None,
        }
    }

    pub fn honouring_an_existing_pin(mut self) -> Self {
        self.intent = Intent::ExistingDependent;
        self
    }

    pub fn under(mut self, freshness: FreshnessPolicy) -> Self {
        self.freshness = freshness;
        self
    }

    pub fn as_of(mut self, epoch: Epoch) -> Self {
        self.as_of = Some(epoch);
        self
    }
}

/// The result of a resolution: the part that must not depend on who answered.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Resolved {
    pub name: PackName,
    pub version: Version,
    pub digest: String,
}

impl fmt::Display for Resolved {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}@{} ({})", self.name, self.version, self.digest)
    }
}

/// The part of a resolution that does depend on who answered.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Provenance {
    pub authority: NameAuthority,
    pub freshness: Freshness,
    /// The policy the answer was accepted under. Recorded rather than merely applied, so that an
    /// air-gapped deployment's decision to proceed on unverifiable currency is visible in the
    /// artifact instead of being invisible in a config file.
    pub accepted_under: FreshnessPolicy,
    /// Everything the lifecycle had to say: yanks honoured, deprecations in force.
    pub notes: Vec<Note>,
}

/// A name, resolved.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Resolution {
    subject: Resolved,
    provenance: Provenance,
}

impl Resolution {
    pub fn subject(&self) -> &Resolved {
        &self.subject
    }

    pub fn provenance(&self) -> &Provenance {
        &self.provenance
    }

    pub fn version(&self) -> Version {
        self.subject.version
    }

    pub fn digest(&self) -> &str {
        &self.subject.digest
    }

    /// Which registry produced this answer.
    pub fn answered_by(&self) -> &RegistryId {
        self.provenance.authority.answered_by()
    }

    /// Whether the registry that produced it was the one entitled to decide the name.
    pub fn is_authoritative(&self) -> bool {
        self.provenance.authority.is_authoritative()
    }

    /// Whether the two resolutions reached the same result, ignoring who said so.
    ///
    /// This is the mirror contract in one method: it must return true for the same question asked
    /// of the origin and of a mirror, while the provenances differ.
    pub fn agrees_with(&self, other: &Resolution) -> bool {
        self.subject == other.subject
    }
}

impl fmt::Display for Resolution {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} from {} [{}]",
            self.subject, self.provenance.authority, self.provenance.freshness
        )
    }
}

/// Resolves a name against one catalog.
pub fn resolve(catalog: &Catalog, request: &Request) -> Result<Resolution, ResolveError> {
    let authority = catalog.standing_for(&request.name)?;
    let freshness = catalog.sync().freshness(request.as_of);
    request.freshness.check(&freshness)?;

    if !catalog.holds(&request.name) {
        return Err(ResolveError::NameNotHeld {
            registry: catalog.id().clone(),
            name: request.name.to_string(),
        });
    }

    let available = catalog.versions_of(&request.name);
    let in_range: Vec<Version> = available
        .iter()
        .copied()
        .filter(|version| request.req.matches(version))
        .collect();
    if in_range.is_empty() {
        return Err(ResolveError::NoVersionInRange {
            registry: catalog.id().clone(),
            name: request.name.to_string(),
            req: request.req.to_string(),
            held: available.iter().map(Version::to_string).collect(),
        });
    }

    let mut refusals = Vec::new();
    for version in in_range.iter().rev() {
        match catalog
            .lifecycle()
            .admits(&request.name, *version, request.intent)
        {
            Ok(admission) => {
                let release = catalog
                    .release(&request.name, version)
                    .expect("a version reported by the catalog is held by it");
                return Ok(Resolution {
                    subject: Resolved {
                        name: request.name.clone(),
                        version: *version,
                        digest: release.digest.clone(),
                    },
                    provenance: Provenance {
                        authority,
                        freshness,
                        accepted_under: request.freshness,
                        notes: admission.notes,
                    },
                });
            }
            Err(error) => refusals.push(error),
        }
    }

    Err(ResolveError::EveryCandidateExcluded {
        registry: catalog.id().clone(),
        name: request.name.to_string(),
        req: request.req.to_string(),
        refusals,
    })
}

/// Resolves a name across a federation, preferring the registry that owns the namespace.
///
/// The catalogs are checked against the federation, not taken at their word: a catalog holding a
/// name it has no standing for is [`ResolveError::Authority`], and two catalogs binding the same
/// version to different digests is [`ResolveError::Mirror`] carrying
/// [`MirrorError::Divergent`].
pub fn resolve_in(
    federation: &Federation,
    catalogs: &[Catalog],
    request: &Request,
) -> Result<Resolution, ResolveError> {
    let mut standing = Vec::new();
    for catalog in catalogs {
        match federation.standing_for(catalog.id(), &request.name) {
            Ok(found) => standing.push((catalog, found)),
            Err(error) => {
                if catalog.holds(&request.name) {
                    return Err(ResolveError::Authority(error));
                }
            }
        }
    }

    let ordered: Vec<&Catalog> = standing
        .iter()
        .filter(|(_, standing)| standing.is_authoritative())
        .map(|(catalog, _)| *catalog)
        .chain(
            standing
                .iter()
                .filter(|(_, standing)| !standing.is_authoritative())
                .map(|(catalog, _)| *catalog),
        )
        .collect();

    if ordered.is_empty() {
        return Err(ResolveError::NoRegistryWithStanding {
            name: request.name.to_string(),
        });
    }

    let mut failures = Vec::new();
    for catalog in &ordered {
        match resolve(catalog, request) {
            Ok(resolution) => {
                cross_check(&ordered, &resolution)?;
                return Ok(resolution);
            }
            Err(error) => failures.push(error),
        }
    }

    Err(failures
        .into_iter()
        .next()
        .expect("a non-empty catalog list yields at least one failure"))
}

/// Confirms that every catalog holding the resolved version binds it to the same digest.
fn cross_check(catalogs: &[&Catalog], resolution: &Resolution) -> Result<(), ResolveError> {
    let subject = &resolution.subject;
    for catalog in catalogs {
        let Some(release) = catalog.release(&subject.name, &subject.version) else {
            continue;
        };
        if release.digest != subject.digest {
            return Err(ResolveError::Mirror(MirrorError::Divergent {
                subject: format!("{}@{}", subject.name, subject.version),
                mirror: catalog.id().clone(),
                origin: resolution.answered_by().clone(),
                mirror_digest: release.digest.clone(),
                origin_digest: subject.digest.clone(),
            }));
        }
    }
    Ok(())
}

/// Why a name did not resolve.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ResolveError {
    #[error(transparent)]
    Authority(#[from] AuthorityError),

    #[error(transparent)]
    Mirror(#[from] MirrorError),

    #[error("no registry in this federation has standing for {name}")]
    NoRegistryWithStanding { name: String },

    #[error("{registry} has standing for {name} but holds no version of it")]
    NameNotHeld { registry: RegistryId, name: String },

    #[error("{registry} holds {name} at {} version(s), none matching {req}: {}", held.len(), held.join(", "))]
    NoVersionInRange {
        registry: RegistryId,
        name: String,
        req: String,
        held: Vec<String>,
    },

    #[error("{registry} holds {} version(s) of {name} matching {req}, and the lifecycle excludes every one", refusals.len())]
    EveryCandidateExcluded {
        registry: RegistryId,
        name: String,
        req: String,
        refusals: Vec<LifecycleError>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::PackRelease;
    use crate::mirror::{Replication, StalenessBound};
    use crate::name::Namespace;
    use crate::registry::Authority;

    fn ns(text: &str) -> Namespace {
        Namespace::parse(text).expect("parses")
    }

    fn id(text: &str) -> RegistryId {
        RegistryId::parse(text).expect("parses")
    }

    fn name(text: &str) -> PackName {
        PackName::parse(text).expect("parses")
    }

    fn v(major: u64, minor: u64, patch: u64) -> Version {
        Version::new(major, minor, patch)
    }

    fn stocked(mut catalog: Catalog) -> Catalog {
        for version in [v(1, 0, 0), v(1, 2, 0), v(2, 0, 0)] {
            catalog
                .record(PackRelease::new(
                    name("bioprism/onco-tp53"),
                    version,
                    format!("sha256:{version}"),
                ))
                .expect("records");
        }
        catalog
    }

    fn origin() -> Catalog {
        stocked(Catalog::origin(
            Authority::new(id("origin"))
                .owning(ns("bioprism"))
                .expect("owns"),
        ))
    }

    fn mirror_at(synced_at: u64, bound: u64) -> Catalog {
        stocked(Catalog::mirror(
            Authority::new(id("site-mirror"))
                .carrying(ns("bioprism"), id("origin"))
                .expect("carries"),
            Replication::mirror(
                id("origin"),
                Epoch(synced_at),
                StalenessBound::epochs(bound),
            ),
        ))
    }

    fn federation() -> Federation {
        let mut federation = Federation::new();
        federation
            .admit(
                Authority::new(id("origin"))
                    .owning(ns("bioprism"))
                    .expect("owns"),
            )
            .expect("admitted");
        federation
            .admit(
                Authority::new(id("site-mirror"))
                    .carrying(ns("bioprism"), id("origin"))
                    .expect("carries"),
            )
            .expect("admitted");
        federation
    }

    fn any() -> Request {
        Request::new(name("bioprism/onco-tp53"), VersionReq::Any).under(FreshnessPolicy::OFFLINE)
    }

    #[test]
    fn a_resolution_names_the_registry_that_answered_and_whether_it_was_authoritative() {
        let direct = resolve(&origin(), &any()).expect("resolves");
        assert_eq!(direct.answered_by(), &id("origin"));
        assert!(direct.is_authoritative());

        let copied = resolve(&mirror_at(4, 8), &any()).expect("resolves");
        assert_eq!(copied.answered_by(), &id("site-mirror"));
        assert!(!copied.is_authoritative());
        assert_eq!(copied.provenance().authority.authority(), &id("origin"));
    }

    #[test]
    fn a_mirror_answer_and_an_origin_answer_agree_in_result_and_differ_in_provenance() {
        let direct = resolve(&origin(), &any()).expect("resolves");
        let copied = resolve(&mirror_at(4, 8), &any()).expect("resolves");
        assert!(direct.agrees_with(&copied));
        assert_eq!(direct.subject(), copied.subject());
        assert_ne!(direct.provenance(), copied.provenance());
    }

    #[test]
    fn resolution_picks_the_highest_version_in_range_rather_than_the_newest_recorded() {
        let request = Request::new(
            name("bioprism/onco-tp53"),
            VersionReq::Compatible(v(1, 0, 0)),
        )
        .under(FreshnessPolicy::OFFLINE);
        let resolution = resolve(&origin(), &request).expect("resolves");
        assert_eq!(resolution.version(), v(1, 2, 0));
    }

    #[test]
    fn a_stale_mirror_resolution_carries_a_different_freshness_than_a_fresh_one() {
        let fresh = resolve(&mirror_at(9, 4), &any().as_of(Epoch(9))).expect("resolves");
        let stale = resolve(&mirror_at(1, 4), &any().as_of(Epoch(9))).expect("resolves");
        assert!(fresh.agrees_with(&stale));
        assert_ne!(fresh.provenance().freshness, stale.provenance().freshness);
        assert!(!stale.provenance().freshness.is_within_declared_bound());
    }

    #[test]
    fn the_strict_default_refuses_a_mirror_whose_currency_cannot_be_established() {
        let strict = Request::new(name("bioprism/onco-tp53"), VersionReq::Any);
        let error = resolve(&mirror_at(4, 8), &strict)
            .expect_err("no reference epoch means no claim about currency");
        assert!(matches!(
            error,
            ResolveError::Mirror(MirrorError::CurrencyUndetermined { .. })
        ));
    }

    #[test]
    fn the_offline_policy_travels_with_the_answer_it_permitted() {
        let resolution = resolve(&mirror_at(4, 8), &any()).expect("resolves");
        assert!(resolution.provenance().accepted_under.accept_undetermined);
        assert!(resolution.provenance().freshness.is_undetermined());
    }

    #[test]
    fn a_name_the_registry_has_no_standing_for_is_refused_before_anything_is_looked_up() {
        let request =
            Request::new(name("elsewhere/other"), VersionReq::Any).under(FreshnessPolicy::OFFLINE);
        let error = resolve(&origin(), &request).expect_err("standing is checked first");
        assert!(matches!(
            error,
            ResolveError::Authority(AuthorityError::OutsideAuthority { .. })
        ));
    }

    #[test]
    fn holding_no_version_and_holding_none_in_range_are_different_failures() {
        let empty = Catalog::origin(
            Authority::new(id("origin"))
                .owning(ns("bioprism"))
                .expect("owns"),
        );
        assert!(matches!(
            resolve(&empty, &any()),
            Err(ResolveError::NameNotHeld { .. })
        ));

        let narrow = Request::new(
            name("bioprism/onco-tp53"),
            VersionReq::Compatible(v(9, 0, 0)),
        )
        .under(FreshnessPolicy::OFFLINE);
        assert!(matches!(
            resolve(&origin(), &narrow),
            Err(ResolveError::NoVersionInRange { ref held, .. }) if held.len() == 3
        ));
    }

    #[test]
    fn a_range_whose_every_candidate_is_yanked_reports_the_yanks_and_not_an_empty_range() {
        let mut catalog = origin();
        catalog
            .lifecycle_mut()
            .yank(
                &name("bioprism/onco-tp53"),
                v(1, 2, 0),
                "leaked label",
                Epoch(3),
            )
            .expect("yanks");
        catalog
            .lifecycle_mut()
            .yank(
                &name("bioprism/onco-tp53"),
                v(1, 0, 0),
                "leaked label",
                Epoch(3),
            )
            .expect("yanks");

        let request = Request::new(
            name("bioprism/onco-tp53"),
            VersionReq::Compatible(v(1, 0, 0)),
        )
        .under(FreshnessPolicy::OFFLINE);
        let error = resolve(&catalog, &request).expect_err("nothing left to choose");
        assert!(matches!(
            error,
            ResolveError::EveryCandidateExcluded { ref refusals, .. } if refusals.len() == 2
        ));
    }

    #[test]
    fn a_yank_moves_the_selection_down_and_the_answer_says_nothing_about_the_yanked_version() {
        let mut catalog = origin();
        catalog
            .lifecycle_mut()
            .yank(
                &name("bioprism/onco-tp53"),
                v(1, 2, 0),
                "leaked label",
                Epoch(3),
            )
            .expect("yanks");
        let request = Request::new(
            name("bioprism/onco-tp53"),
            VersionReq::Compatible(v(1, 0, 0)),
        )
        .under(FreshnessPolicy::OFFLINE);
        let resolution = resolve(&catalog, &request).expect("falls back");
        assert_eq!(resolution.version(), v(1, 0, 0));
        assert!(resolution.provenance().notes.is_empty());
    }

    #[test]
    fn an_existing_dependent_still_resolves_a_yanked_pin_and_the_note_says_so() {
        let mut catalog = origin();
        catalog
            .lifecycle_mut()
            .yank(
                &name("bioprism/onco-tp53"),
                v(1, 2, 0),
                "leaked label",
                Epoch(3),
            )
            .expect("yanks");
        let request = Request::new(name("bioprism/onco-tp53"), VersionReq::Exact(v(1, 2, 0)))
            .honouring_an_existing_pin()
            .under(FreshnessPolicy::OFFLINE);
        let resolution = resolve(&catalog, &request).expect("a pin survives a yank");
        assert_eq!(resolution.version(), v(1, 2, 0));
        assert!(matches!(
            resolution.provenance().notes.as_slice(),
            [Note::YankedButPinned { .. }]
        ));
    }

    #[test]
    fn the_federation_asks_the_authority_before_it_asks_a_carrier() {
        let answer =
            resolve_in(&federation(), &[mirror_at(4, 8), origin()], &any()).expect("resolves");
        assert!(answer.is_authoritative());
        assert_eq!(answer.answered_by(), &id("origin"));
    }

    #[test]
    fn with_only_a_carrier_present_the_federation_still_answers_and_says_it_was_a_carrier() {
        let answer = resolve_in(&federation(), &[mirror_at(4, 8)], &any()).expect("resolves");
        assert!(!answer.is_authoritative());
        assert_eq!(answer.provenance().authority.authority(), &id("origin"));
    }

    #[test]
    fn a_mirror_binding_a_version_to_a_different_digest_is_divergence_and_not_staleness() {
        let mut liar = Catalog::mirror(
            Authority::new(id("site-mirror"))
                .carrying(ns("bioprism"), id("origin"))
                .expect("carries"),
            Replication::mirror(id("origin"), Epoch(4), StalenessBound::epochs(8)),
        );
        liar.record(PackRelease::new(
            name("bioprism/onco-tp53"),
            v(2, 0, 0),
            "sha256:tampered",
        ))
        .expect("records");

        let error = resolve_in(&federation(), &[origin(), liar], &any())
            .expect_err("an immutable binding makes disagreement impossible to excuse");
        assert!(matches!(
            error,
            ResolveError::Mirror(MirrorError::Divergent { .. })
        ));
    }

    #[test]
    fn a_federation_with_nobody_holding_standing_says_so_rather_than_reporting_a_missing_name() {
        let federation = federation();
        let request =
            Request::new(name("elsewhere/other"), VersionReq::Any).under(FreshnessPolicy::OFFLINE);
        assert!(matches!(
            resolve_in(&federation, &[origin()], &request),
            Err(ResolveError::NoRegistryWithStanding { .. })
        ));
    }

    #[test]
    fn a_resolution_survives_a_json_round_trip_with_both_halves_intact() {
        let resolution = resolve(&mirror_at(1, 4), &any().as_of(Epoch(9))).expect("resolves");
        let text = serde_json::to_string(&resolution).expect("serialises");
        let back: Resolution = serde_json::from_str(&text).expect("deserialises");
        assert_eq!(back, resolution);
        assert!(!back.is_authoritative());
    }
}
