//! Discovery: finding a pack, and being told why it was offered.
//!
//! Blueprint 10.10 (Catalog, Search and Recommendation) and 10.11 (Search, Discovery and
//! Recommendation) are near-duplicates of each other. Between them they require faceted search,
//! that recommendation "expose why an item was recommended", that ranking "avoid popularity-only
//! ranking", and that every recommendation state "matched features, excluded alternatives, trust
//! filters". They also ask for semantic embeddings, capability-tree navigation, structural
//! fingerprints and failure-signature similarity, none of which is implementable over a catalog
//! and all of which needs the pack documents `bioprism-registry` holds.
//!
//! What is implementable here is the part with a checkable contract, and it turns out to be the
//! part the blueprint is most insistent about.
//!
//! # Every result states its reason, and a result with no reason is not returned
//!
//! [`Match::why`] is never empty. That is enforced structurally: a release enters the results only
//! by satisfying facets, and the satisfied facets are what the reason is made of. An empty
//! [`Query`] is [`SearchError::NoFacets`] rather than "everything", because a query with nothing
//! to match on would produce results with nothing to explain — and a list of packs offered for no
//! stated reason is a ranking dressed as a search.
//!
//! # There is no popularity signal, because there is nothing to measure it with
//!
//! 10.11 permits popularity to break ties. This crate has no download counts, no telemetry and no
//! usage history — deliberately, per 10.02's local-first contract — so a popularity field would be
//! a number nobody could check. Ranking uses only what is in the catalog: how many facets matched,
//! then the answering registry's tier, then name, then version. Ties break on name, which is
//! arbitrary and *known* to be arbitrary, rather than on a proxy for merit that is not one.
//!
//! # A search result is an answer, so it carries provenance
//!
//! Same rule as [`mod@crate::resolve`]: a [`Match`] says which registry offered it, whether that
//! registry was authoritative for the namespace, and how current its copy is. A pack discovered
//! through a stale mirror is discoverable and is not thereby current.
//!
//! # Not implemented
//!
//! No embeddings, no semantic similarity, no graph traversal over lineage, no capability tree, no
//! structural fingerprints, no private-search sketches (10.11's whole "Privacy" section). Facets
//! are exact and textual matching is substring, which is honest about being lexical rather than
//! pretending to be semantic.

use crate::catalog::Catalog;
use crate::lifecycle::Intent;
use crate::mirror::{Freshness, FreshnessPolicy, MirrorError};
use crate::name::{Namespace, PackName};
use crate::registry::{AuthorityError, Federation, NameAuthority};
use bioprism_registry::TrustTier;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

/// One thing a caller is looking for.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "facet", content = "value", rename_all = "snake_case")]
pub enum Facet {
    /// Packs in one namespace.
    InNamespace(Namespace),
    /// An exact keyword from the release's declared keyword set.
    Keyword(String),
    /// A substring of the pack's local name or summary. Lexical, and named so.
    Term(String),
    /// A floor on the answering registry's tier. See [`crate::catalog`] on whose opinion that is.
    TierAtLeast(TrustTier),
    /// Packs that declare a dependency on a given name.
    DependsOn(PackName),
    /// Exclude anything the lifecycle would refuse to a new dependent: yanked versions, withdrawn
    /// versions, sunset and removed pack lines.
    UsableByANewDependent,
}

impl fmt::Display for Facet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Facet::InNamespace(namespace) => write!(f, "namespace {namespace}"),
            Facet::Keyword(keyword) => write!(f, "keyword {keyword:?}"),
            Facet::Term(term) => write!(f, "term {term:?}"),
            Facet::TierAtLeast(tier) => write!(f, "tier at least {}", tier.as_str()),
            Facet::DependsOn(name) => write!(f, "depends on {name}"),
            Facet::UsableByANewDependent => f.write_str("usable by a new dependent"),
        }
    }
}

/// What a caller asked for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Query {
    pub facets: Vec<Facet>,
    /// Applied to mirrors exactly as in resolution: a catalog whose currency the caller will not
    /// accept contributes nothing rather than contributing silently.
    pub freshness: FreshnessPolicy,
    pub limit: Option<usize>,
}

impl Query {
    pub fn new(facets: Vec<Facet>) -> Self {
        Query {
            facets,
            freshness: FreshnessPolicy::default(),
            limit: None,
        }
    }

    pub fn under(mut self, freshness: FreshnessPolicy) -> Self {
        self.freshness = freshness;
        self
    }

    pub fn limited_to(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }

    fn validate(&self) -> Result<(), SearchError> {
        if self.facets.is_empty() {
            return Err(SearchError::NoFacets);
        }
        if self.facets.iter().any(|facet| {
            matches!(
                facet,
                Facet::Keyword(keyword) | Facet::Term(keyword) if keyword.trim().is_empty()
            )
        }) {
            return Err(SearchError::InvalidQuery {
                detail: "keyword and term facets must contain a non-whitespace value".into(),
            });
        }
        Ok(())
    }
}

/// Why one release was offered.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "why", rename_all = "snake_case")]
pub enum Why {
    NamespaceMatched {
        namespace: String,
    },
    KeywordMatched {
        keyword: String,
    },
    TermInName {
        term: String,
    },
    TermInSummary {
        term: String,
    },
    /// The tier is attributed, because a tier in a catalog is a registry's opinion and not a
    /// property of the artifact that this crate verified.
    TierMet {
        required: TrustTier,
        observed: TrustTier,
        according_to: String,
    },
    DependencyMatched {
        on: String,
    },
    UsableByANewDependent,
}

impl fmt::Display for Why {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Why::NamespaceMatched { namespace } => write!(f, "is in namespace {namespace}"),
            Why::KeywordMatched { keyword } => write!(f, "declares keyword {keyword:?}"),
            Why::TermInName { term } => write!(f, "name contains {term:?}"),
            Why::TermInSummary { term } => write!(f, "summary contains {term:?}"),
            Why::TierMet {
                required,
                observed,
                according_to,
            } => write!(
                f,
                "{according_to} assesses it {}, at or above {}",
                observed.as_str(),
                required.as_str()
            ),
            Why::DependencyMatched { on } => write!(f, "depends on {on}"),
            Why::UsableByANewDependent => f.write_str("is usable by a new dependent"),
        }
    }
}

/// One offered release, with its reason and its provenance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Match {
    pub name: PackName,
    pub version: crate::name::Version,
    pub digest: String,
    pub summary: String,
    pub tier: TrustTier,
    pub authority: NameAuthority,
    pub freshness: Freshness,
    /// Never empty. See the module docs.
    pub why: Vec<Why>,
}

impl Match {
    pub fn is_authoritative(&self) -> bool {
        self.authority.is_authoritative()
    }
}

/// A release that matched something and was dropped, with the facet that dropped it.
///
/// 10.11 requires a recommendation to state its "excluded alternatives". This is that list, kept
/// to near misses — anything that satisfied at least one facet — because everything else in the
/// catalog was never an alternative.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Excluded {
    pub name: PackName,
    pub version: crate::name::Version,
    pub failed: String,
}

/// What a search found, what it nearly found, and whether it stopped early.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Results {
    pub matches: Vec<Match>,
    pub excluded: Vec<Excluded>,
    /// True when [`Query::limit`] dropped matches. Reported rather than left implicit, so a caller
    /// cannot mistake a truncated list for an exhaustive one.
    pub truncated: bool,
}

impl Results {
    pub fn is_empty(&self) -> bool {
        self.matches.is_empty()
    }

    pub fn len(&self) -> usize {
        self.matches.len()
    }
}

/// Searches every catalog the caller supplied, preferring the authority for each name.
pub fn search(
    federation: &Federation,
    catalogs: &[Catalog],
    query: &Query,
) -> Result<Results, SearchError> {
    query.validate()?;

    let mut best: BTreeMap<(PackName, crate::name::Version), Match> = BTreeMap::new();
    let mut excluded = Vec::new();

    for catalog in catalogs {
        catalog
            .validate()
            .map_err(|error| SearchError::CatalogInvalid {
                registry: catalog.id().clone(),
                detail: error.to_string(),
            })?;
        let freshness = catalog.sync().freshness(None);
        if query.freshness.check(&freshness).is_err() {
            continue;
        }
        for release in catalog.releases() {
            let authority = federation
                .standing_for(catalog.id(), &release.name)
                .map_err(SearchError::Authority)?;
            let mut why = Vec::new();
            let mut failed = None;
            for facet in &query.facets {
                match reason(catalog, release, facet) {
                    Some(found) => why.push(found),
                    None => {
                        failed = Some(facet.to_string());
                        break;
                    }
                }
            }
            match failed {
                Some(failed) if !why.is_empty() => excluded.push(Excluded {
                    name: release.name.clone(),
                    version: release.version,
                    failed,
                }),
                Some(_) => {}
                None => {
                    let key = (release.name.clone(), release.version);
                    let candidate = Match {
                        name: release.name.clone(),
                        version: release.version,
                        digest: release.digest.clone(),
                        summary: release.summary.clone(),
                        tier: release.tier,
                        authority,
                        freshness: freshness.clone(),
                        why,
                    };
                    if let Some(held) = best.get(&key) {
                        if held.digest != candidate.digest {
                            let (mirror, origin, mirror_digest, origin_digest) =
                                if held.is_authoritative() && !candidate.is_authoritative() {
                                    (
                                        candidate.authority.answered_by().clone(),
                                        held.authority.authority().clone(),
                                        candidate.digest.clone(),
                                        held.digest.clone(),
                                    )
                                } else if candidate.is_authoritative() && !held.is_authoritative() {
                                    (
                                        held.authority.answered_by().clone(),
                                        candidate.authority.authority().clone(),
                                        held.digest.clone(),
                                        candidate.digest.clone(),
                                    )
                                } else {
                                    (
                                        candidate.authority.answered_by().clone(),
                                        held.authority.authority().clone(),
                                        candidate.digest.clone(),
                                        held.digest.clone(),
                                    )
                                };
                            return Err(SearchError::Mirror(MirrorError::Divergent {
                                subject: format!("{}@{}", release.name, release.version),
                                mirror,
                                origin,
                                mirror_digest,
                                origin_digest,
                            }));
                        }
                    }
                    let replace = best.get(&key).is_none_or(|held| {
                        !held.is_authoritative() && candidate.is_authoritative()
                    });
                    if replace {
                        best.insert(key, candidate);
                    }
                }
            }
        }
    }

    let mut matches: Vec<Match> = best.into_values().collect();
    matches.sort_by(|left, right| {
        right
            .why
            .len()
            .cmp(&left.why.len())
            .then(right.tier.cmp(&left.tier))
            .then(left.name.cmp(&right.name))
            .then(right.version.cmp(&left.version))
    });
    excluded.sort();
    excluded.dedup();

    let truncated = query.limit.is_some_and(|limit| matches.len() > limit);
    if let Some(limit) = query.limit {
        matches.truncate(limit);
    }

    Ok(Results {
        matches,
        excluded,
        truncated,
    })
}

fn reason(catalog: &Catalog, release: &crate::catalog::PackRelease, facet: &Facet) -> Option<Why> {
    match facet {
        Facet::InNamespace(namespace) => {
            (release.name.namespace() == namespace).then(|| Why::NamespaceMatched {
                namespace: namespace.to_string(),
            })
        }
        Facet::Keyword(keyword) => {
            release
                .keywords
                .contains(keyword)
                .then(|| Why::KeywordMatched {
                    keyword: keyword.clone(),
                })
        }
        Facet::Term(term) => {
            if release.name.local().contains(term.as_str()) {
                Some(Why::TermInName { term: term.clone() })
            } else if release.summary.contains(term.as_str()) {
                Some(Why::TermInSummary { term: term.clone() })
            } else {
                None
            }
        }
        Facet::TierAtLeast(required) => (release.tier >= *required).then(|| Why::TierMet {
            required: *required,
            observed: release.tier,
            according_to: catalog.id().to_string(),
        }),
        Facet::DependsOn(on) => release
            .dependencies
            .iter()
            .any(|dependency| &dependency.name == on)
            .then(|| Why::DependencyMatched { on: on.to_string() }),
        Facet::UsableByANewDependent => catalog
            .lifecycle()
            .admits(&release.name, release.version, Intent::NewDependent)
            .is_ok()
            .then_some(Why::UsableByANewDependent),
    }
}

/// Why a search did not happen.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SearchError {
    #[error("a query with no facets has nothing to explain about its results")]
    NoFacets,

    #[error("invalid search query: {detail}")]
    InvalidQuery { detail: String },

    #[error("{registry} contains an invalid catalog: {detail}")]
    CatalogInvalid {
        registry: crate::registry::RegistryId,
        detail: String,
    },

    #[error(transparent)]
    Authority(#[from] AuthorityError),

    #[error(transparent)]
    Mirror(#[from] MirrorError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::PackRelease;
    use crate::mirror::{Replication, StalenessBound};
    use crate::name::{Version, VersionReq};
    use crate::registry::{Authority, RegistryId};
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

    fn v(major: u64, minor: u64, patch: u64) -> Version {
        Version::new(major, minor, patch)
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

    fn stocked() -> Catalog {
        let mut catalog = Catalog::origin(
            Authority::new(id("origin"))
                .owning(ns("bioprism"))
                .expect("owns"),
        );
        catalog
            .record(
                PackRelease::new(name("bioprism/onco-tp53"), v(1, 0, 0), "sha256:tp53")
                    .described("TP53 decision cells over the onco world")
                    .keyworded(["onco", "tp53"])
                    .at_tier(TrustTier::Reviewed),
            )
            .expect("records");
        catalog
            .record(
                PackRelease::new(name("bioprism/onco-kras"), v(2, 0, 0), "sha256:kras")
                    .described("KRAS decision cells")
                    .keyworded(["onco", "kras"])
                    .at_tier(TrustTier::Exploratory)
                    .depending_on(
                        name("bioprism/onco-tp53"),
                        VersionReq::Compatible(v(1, 0, 0)),
                    ),
            )
            .expect("records");
        catalog
    }

    fn offline(facets: Vec<Facet>) -> Query {
        Query::new(facets).under(FreshnessPolicy::OFFLINE)
    }

    #[test]
    fn every_result_states_at_least_one_reason_it_was_offered() {
        let results = search(
            &federation(),
            &[stocked()],
            &offline(vec![Facet::Keyword("onco".to_string())]),
        )
        .expect("searches");
        assert_eq!(results.len(), 2);
        assert!(results.matches.iter().all(|found| !found.why.is_empty()));
    }

    #[test]
    fn a_query_with_no_facets_is_refused_rather_than_returning_the_whole_catalog() {
        let error = search(&federation(), &[stocked()], &offline(vec![]))
            .expect_err("everything is not a query");
        assert!(matches!(error, SearchError::NoFacets));
    }

    #[test]
    fn an_empty_text_facet_is_refused_rather_than_matching_every_release() {
        let error = search(
            &federation(),
            &[stocked()],
            &offline(vec![Facet::Keyword("   ".into())]),
        )
        .expect_err("an empty facet has no search meaning");
        assert!(matches!(error, SearchError::InvalidQuery { .. }));
    }

    #[test]
    fn a_tier_in_a_result_is_attributed_to_the_registry_that_assessed_it() {
        let results = search(
            &federation(),
            &[stocked()],
            &offline(vec![Facet::TierAtLeast(TrustTier::Reviewed)]),
        )
        .expect("searches");
        assert_eq!(results.len(), 1);
        assert!(matches!(
            results.matches[0].why.as_slice(),
            [Why::TierMet { according_to, .. }] if according_to == "origin"
        ));
    }

    #[test]
    fn a_near_miss_is_reported_as_an_excluded_alternative_with_the_facet_that_dropped_it() {
        let results = search(
            &federation(),
            &[stocked()],
            &offline(vec![
                Facet::Keyword("onco".to_string()),
                Facet::TierAtLeast(TrustTier::Reviewed),
            ]),
        )
        .expect("searches");
        assert_eq!(results.len(), 1);
        assert_eq!(results.excluded.len(), 1);
        assert_eq!(results.excluded[0].name, name("bioprism/onco-kras"));
        assert!(results.excluded[0].failed.contains("reviewed"));
    }

    #[test]
    fn a_release_matching_nothing_at_all_is_not_an_excluded_alternative() {
        let results = search(
            &federation(),
            &[stocked()],
            &offline(vec![Facet::Keyword("kras".to_string())]),
        )
        .expect("searches");
        assert_eq!(results.len(), 1);
        assert!(results.excluded.is_empty());
    }

    #[test]
    fn results_are_ranked_by_facets_matched_then_tier_and_never_by_popularity() {
        let results = search(
            &federation(),
            &[stocked()],
            &offline(vec![Facet::InNamespace(ns("bioprism"))]),
        )
        .expect("searches");
        assert_eq!(results.matches[0].tier, TrustTier::Reviewed);
        assert_eq!(results.matches[0].name, name("bioprism/onco-tp53"));
    }

    #[test]
    fn a_search_result_says_which_registry_offered_it_and_whether_it_was_authoritative() {
        let mut copy = Catalog::mirror(
            Authority::new(id("site-mirror"))
                .carrying(ns("bioprism"), id("origin"))
                .expect("carries"),
            Replication::mirror(id("origin"), Epoch(4), StalenessBound::epochs(2)),
        );
        for release in stocked().releases() {
            copy.record(release.clone()).expect("records");
        }
        let results = search(
            &federation(),
            &[copy],
            &offline(vec![Facet::Keyword("onco".to_string())]),
        )
        .expect("searches");
        assert!(results
            .matches
            .iter()
            .all(|found| !found.is_authoritative()));
        assert!(results
            .matches
            .iter()
            .all(|found| found.freshness.is_undetermined()));
    }

    #[test]
    fn the_same_release_offered_by_an_origin_and_a_mirror_is_attributed_to_the_origin() {
        let mut copy = Catalog::mirror(
            Authority::new(id("site-mirror"))
                .carrying(ns("bioprism"), id("origin"))
                .expect("carries"),
            Replication::mirror(id("origin"), Epoch(4), StalenessBound::epochs(2)),
        );
        for release in stocked().releases() {
            copy.record(release.clone()).expect("records");
        }
        let results = search(
            &federation(),
            &[copy, stocked()],
            &offline(vec![Facet::Keyword("onco".to_string())]),
        )
        .expect("searches");
        assert_eq!(results.len(), 2);
        assert!(results.matches.iter().all(Match::is_authoritative));
    }

    #[test]
    fn a_search_refuses_a_divergent_binding_instead_of_hiding_it_behind_origin_preference() {
        let mut copy = Catalog::mirror(
            Authority::new(id("site-mirror"))
                .carrying(ns("bioprism"), id("origin"))
                .expect("carries"),
            Replication::mirror(id("origin"), Epoch(4), StalenessBound::epochs(2)),
        );
        copy.record(PackRelease::new(
            name("bioprism/onco-tp53"),
            v(1, 0, 0),
            "sha256:tampered",
        ))
        .expect("records");
        let error = search(
            &federation(),
            &[stocked(), copy],
            &offline(vec![Facet::InNamespace(ns("bioprism"))]),
        )
        .expect_err("a name/version cannot legitimately bind to two digests");
        assert!(matches!(
            error,
            SearchError::Mirror(MirrorError::Divergent { .. })
        ));
    }

    #[test]
    fn a_catalog_whose_currency_the_caller_will_not_accept_contributes_nothing() {
        let mut copy = Catalog::mirror(
            Authority::new(id("site-mirror"))
                .carrying(ns("bioprism"), id("origin"))
                .expect("carries"),
            Replication::mirror(id("origin"), Epoch(4), StalenessBound::epochs(2)),
        );
        for release in stocked().releases() {
            copy.record(release.clone()).expect("records");
        }
        let strict = Query::new(vec![Facet::Keyword("onco".to_string())]);
        let results = search(&federation(), &[copy], &strict).expect("searches");
        assert!(results.is_empty());
    }

    #[test]
    fn a_yanked_version_is_still_discoverable_and_is_excluded_when_usability_is_asked_for() {
        let mut catalog = stocked();
        catalog
            .lifecycle_mut()
            .yank(
                &name("bioprism/onco-tp53"),
                v(1, 0, 0),
                "leaked label",
                Epoch(3),
            )
            .expect("yanks");

        let discoverable = search(
            &federation(),
            &[catalog.clone()],
            &offline(vec![Facet::Keyword("tp53".to_string())]),
        )
        .expect("searches");
        assert_eq!(discoverable.len(), 1);

        let usable = search(
            &federation(),
            &[catalog],
            &offline(vec![
                Facet::Keyword("tp53".to_string()),
                Facet::UsableByANewDependent,
            ]),
        )
        .expect("searches");
        assert!(usable.is_empty());
        assert_eq!(usable.excluded.len(), 1);
    }

    #[test]
    fn a_dependency_facet_finds_dependents_and_not_the_dependency() {
        let results = search(
            &federation(),
            &[stocked()],
            &offline(vec![Facet::DependsOn(name("bioprism/onco-tp53"))]),
        )
        .expect("searches");
        assert_eq!(results.len(), 1);
        assert_eq!(results.matches[0].name, name("bioprism/onco-kras"));
    }

    #[test]
    fn a_limit_that_drops_matches_says_it_dropped_them() {
        let unlimited = search(
            &federation(),
            &[stocked()],
            &offline(vec![Facet::Keyword("onco".to_string())]),
        )
        .expect("searches");
        assert!(!unlimited.truncated);

        let capped = search(
            &federation(),
            &[stocked()],
            &offline(vec![Facet::Keyword("onco".to_string())]).limited_to(1),
        )
        .expect("searches");
        assert_eq!(capped.len(), 1);
        assert!(capped.truncated);
    }

    #[test]
    fn term_matching_is_lexical_and_says_which_field_it_hit() {
        let results = search(
            &federation(),
            &[stocked()],
            &offline(vec![Facet::Term("decision cells".to_string())]),
        )
        .expect("searches");
        assert_eq!(results.len(), 2);
        assert!(results
            .matches
            .iter()
            .all(|found| matches!(found.why.as_slice(), [Why::TermInSummary { .. }])));
    }

    #[test]
    fn a_query_survives_a_json_round_trip() {
        let query = offline(vec![
            Facet::InNamespace(ns("bioprism")),
            Facet::Keyword("onco".to_string()),
            Facet::TierAtLeast(TrustTier::Reviewed),
            Facet::DependsOn(name("bioprism/onco-tp53")),
            Facet::Term("decision".to_string()),
            Facet::UsableByANewDependent,
        ])
        .limited_to(3);
        let text = serde_json::to_string(&query).expect("serialises");
        let back: Query = serde_json::from_str(&text).expect("deserialises");
        assert_eq!(back, query);
    }

    #[test]
    fn search_is_deterministic_across_repeated_runs() {
        let query = offline(vec![Facet::InNamespace(ns("bioprism"))]);
        let first = search(&federation(), &[stocked()], &query).expect("searches");
        let second = search(&federation(), &[stocked()], &query).expect("searches");
        assert_eq!(
            serde_json::to_string(&first).expect("serialises"),
            serde_json::to_string(&second).expect("serialises")
        );
    }
}
