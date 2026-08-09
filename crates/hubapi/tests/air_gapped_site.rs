//! One story, told through the public API: an air-gapped site running off a mirror.
//!
//! The unit tests check each invariant where it lives. This checks that they compose — that a
//! deployment which never reaches the origin gets the same *answers* and a visibly different
//! *record* of where they came from, and that nothing it can do offline promotes a mirror into an
//! authority or a peer's opinion into its own.

use bioprism_hub::Epoch;
use bioprism_hubapi::{
    adopt, resolve, resolve_dependencies, resolve_in, search, AdoptionPolicy, Authority, Basis,
    Catalog, DependencyError, Facet, Federation, FederationError, FreshnessPolicy, Intent,
    MirrorError, Namespace, Note, PackName, PackRelease, Query, RegistryId, Replication, Request,
    ResolveError, StalenessBound, Subject, TrustStanding, Version, VersionReq,
};
use bioprism_registry::{TierAssessment, TrustTier};

const ORIGIN: &str = "bioatlas-hub";
const SITE: &str = "site-mirror";
const OTHER: &str = "partner-hub";

fn ns() -> Namespace {
    Namespace::parse("bioprism").expect("parses")
}

fn id(text: &str) -> RegistryId {
    RegistryId::parse(text).expect("parses")
}

fn name(local: &str) -> PackName {
    PackName::parse(format!("bioprism/{local}")).expect("parses")
}

fn v(major: u64, minor: u64, patch: u64) -> Version {
    Version::new(major, minor, patch)
}

fn origin_authority() -> Authority {
    Authority::new(id(ORIGIN)).owning(ns()).expect("owns")
}

fn site_authority() -> Authority {
    Authority::new(id(SITE))
        .carrying(ns(), id(ORIGIN))
        .expect("carries")
}

fn federation() -> Federation {
    let mut federation = Federation::new();
    federation.admit(origin_authority()).expect("admitted");
    federation.admit(site_authority()).expect("admitted");
    federation
        .admit(
            Authority::new(id(OTHER))
                .owning(Namespace::parse("partner").expect("parses"))
                .expect("owns"),
        )
        .expect("admitted");
    federation
}

/// `onco-suite 1.0.0` needs `onco-core ^1.2`; `onco-core` exists at 1.2.0, 1.5.0 and 2.0.0.
fn published() -> Vec<PackRelease> {
    let mut releases = Vec::new();
    for version in [v(1, 2, 0), v(1, 5, 0), v(2, 0, 0)] {
        releases.push(
            PackRelease::new(name("onco-core"), version, format!("sha256:core-{version}"))
                .described("core decision cells for the onco world")
                .keyworded(["onco", "core"])
                .at_tier(TrustTier::Reviewed),
        );
    }
    releases.push(
        PackRelease::new(name("onco-suite"), v(1, 0, 0), "sha256:suite-1.0.0")
            .described("the onco evaluation suite")
            .keyworded(["onco", "suite"])
            .at_tier(TrustTier::GeneratedVerified)
            .depending_on(name("onco-core"), VersionReq::Compatible(v(1, 2, 0))),
    );
    releases
}

fn hub() -> Catalog {
    let mut catalog = Catalog::origin(origin_authority());
    for release in published() {
        catalog.record(release).expect("records");
    }
    catalog
}

/// The bundle the site was handed: everything the hub held as of epoch 40, promising to be no more
/// than eight epochs behind.
fn site() -> Catalog {
    let mut catalog = Catalog::mirror(
        site_authority(),
        Replication::mirror(id(ORIGIN), Epoch(40), StalenessBound::epochs(8)),
    );
    for release in published() {
        catalog.record(release).expect("records");
    }
    catalog
}

fn suite_request() -> Request {
    Request::new(name("onco-suite"), VersionReq::Any).under(FreshnessPolicy::OFFLINE)
}

#[test]
fn the_site_and_the_hub_reach_the_same_result_by_different_routes() {
    let from_hub = resolve(&hub(), &suite_request()).expect("resolves");
    let from_site = resolve(&site(), &suite_request()).expect("resolves");

    assert!(from_hub.agrees_with(&from_site));
    assert_eq!(from_hub.subject(), from_site.subject());
    assert_ne!(from_hub.provenance(), from_site.provenance());

    assert_eq!(from_hub.answered_by(), &id(ORIGIN));
    assert!(from_hub.is_authoritative());
    assert_eq!(from_site.answered_by(), &id(SITE));
    assert!(!from_site.is_authoritative());
    assert_eq!(from_site.provenance().authority.authority(), &id(ORIGIN));
}

#[test]
fn the_sites_answer_never_claims_to_be_current() {
    let offline = resolve(&site(), &suite_request()).expect("resolves");
    assert!(!offline.provenance().freshness.is_from_authority());
    assert!(offline.provenance().freshness.is_undetermined());

    let judged = resolve(&site(), &suite_request().as_of(Epoch(60))).expect("resolves");
    assert!(!judged.provenance().freshness.is_within_declared_bound());
    assert_eq!(judged.provenance().freshness.lag(), Some(20));
    assert!(judged.agrees_with(&offline));
}

#[test]
fn a_site_that_has_not_opted_into_offline_operation_is_refused_rather_than_served_stale() {
    let strict = Request::new(name("onco-suite"), VersionReq::Any);
    let error = resolve(&site(), &strict).expect_err("the default policy will not guess");
    assert!(matches!(
        error,
        ResolveError::Mirror(MirrorError::CurrencyUndetermined { .. })
    ));
}

#[test]
fn an_offline_closure_pins_everything_and_says_it_rests_on_a_mirror() {
    let lock = resolve_dependencies(&federation(), &[site()], &suite_request()).expect("resolves");

    assert_eq!(lock.len(), 2);
    assert_eq!(lock.version_of(&name("onco-suite")), Some(v(1, 0, 0)));
    assert_eq!(lock.version_of(&name("onco-core")), Some(v(1, 5, 0)));
    assert!(!lock.is_fully_authoritative());
    assert_eq!(
        lock.answering_registries().into_iter().collect::<Vec<_>>(),
        [SITE]
    );

    let online = resolve_dependencies(&federation(), &[hub()], &suite_request()).expect("resolves");
    assert!(online.is_fully_authoritative());
    for (key, locked) in online.entries() {
        let offline = lock
            .get(&PackName::parse(key.clone()).expect("parses"))
            .expect("pinned");
        assert_eq!(offline.digest(), locked.digest());
        assert_eq!(offline.version(), locked.version());
    }
}

#[test]
fn a_yank_that_arrives_with_the_next_bundle_moves_new_work_and_leaves_old_work_alone() {
    let mut refreshed = site();
    refreshed
        .lifecycle_mut()
        .yank(
            &name("onco-core"),
            v(1, 5, 0),
            "instance 41 leaked a holdout label",
            Epoch(48),
        )
        .expect("yanks");

    let fresh_work = resolve_dependencies(&federation(), &[refreshed.clone()], &suite_request())
        .expect("resolves");
    assert_eq!(fresh_work.version_of(&name("onco-core")), Some(v(1, 2, 0)));
    assert!(fresh_work.remarked().is_empty());

    let existing_work = resolve_dependencies(
        &federation(),
        &[refreshed],
        &suite_request().honouring_an_existing_pin(),
    )
    .expect("a yank does not rewrite a build that was already correct");
    assert_eq!(
        existing_work.version_of(&name("onco-core")),
        Some(v(1, 5, 0))
    );
    assert!(matches!(
        existing_work
            .get(&name("onco-core"))
            .expect("pinned")
            .notes(),
        [Note::YankedButPinned { .. }]
    ));
}

#[test]
fn a_withdrawal_stops_the_air_gapped_build_instead_of_sliding_it_sideways() {
    let mut refreshed = site();
    for version in [v(1, 2, 0), v(1, 5, 0)] {
        refreshed
            .lifecycle_mut()
            .withdraw(
                &name("onco-core"),
                version,
                "the archive shipped a live credential",
                "BIOPRISM-2026-04",
                Epoch(48),
            )
            .expect("withdraws");
    }
    let error = resolve_dependencies(
        &federation(),
        &[refreshed],
        &suite_request().honouring_an_existing_pin(),
    )
    .expect_err("a withdrawal is meant to break the build");
    assert!(matches!(error, DependencyError::Unresolvable { .. }));
}

#[test]
fn a_conflict_introduced_upstream_names_the_two_requirements_and_not_a_guess() {
    let mut catalog = site();
    catalog
        .record(
            PackRelease::new(name("onco-probe"), v(1, 0, 0), "sha256:probe-1.0.0")
                .depending_on(name("onco-core"), VersionReq::Compatible(v(2, 0, 0))),
        )
        .expect("records");
    catalog
        .record(
            PackRelease::new(name("onco-bundle"), v(1, 0, 0), "sha256:bundle-1.0.0")
                .depending_on(name("onco-suite"), VersionReq::Any)
                .depending_on(name("onco-probe"), VersionReq::Any),
        )
        .expect("records");

    let request =
        Request::new(name("onco-bundle"), VersionReq::Any).under(FreshnessPolicy::OFFLINE);
    let error = resolve_dependencies(&federation(), &[catalog], &request)
        .expect_err("core cannot be both ^1.2 and ^2.0");
    let DependencyError::Collision(collision) = error else {
        panic!("expected a named pair");
    };
    assert_eq!(collision.on, name("onco-core"));
    let sources = [
        collision.left.source.to_string(),
        collision.right.source.to_string(),
    ];
    assert!(sources.contains(&"bioprism/onco-suite@1.0.0".to_string()));
    assert!(sources.contains(&"bioprism/onco-probe@1.0.0".to_string()));
}

#[test]
fn discovery_offline_says_which_registry_offered_each_result_and_why() {
    let query = Query::new(vec![
        Facet::Keyword("onco".to_string()),
        Facet::TierAtLeast(TrustTier::Reviewed),
        Facet::UsableByANewDependent,
    ])
    .under(FreshnessPolicy::OFFLINE);

    let results = search(&federation(), &[site()], &query).expect("searches");
    assert_eq!(results.len(), 3);
    assert!(results.matches.iter().all(|found| found.why.len() == 3));
    assert!(results
        .matches
        .iter()
        .all(|found| !found.is_authoritative()));
    assert!(results
        .matches
        .iter()
        .all(|found| found.freshness.is_undetermined()));
    assert_eq!(results.excluded.len(), 1);
    assert_eq!(results.excluded[0].name, name("onco-suite"));
}

#[test]
fn the_hub_and_the_site_cannot_disagree_about_a_digest_without_it_being_reported() {
    let mut tampered = Catalog::mirror(
        site_authority(),
        Replication::mirror(id(ORIGIN), Epoch(40), StalenessBound::epochs(8)),
    );
    tampered
        .record(PackRelease::new(
            name("onco-suite"),
            v(1, 0, 0),
            "sha256:not-the-suite",
        ))
        .expect("records");

    let error = resolve_in(&federation(), &[hub(), tampered], &suite_request())
        .expect_err("an immutable binding leaves no room for a legitimate disagreement");
    assert!(matches!(
        error,
        ResolveError::Mirror(MirrorError::Divergent { .. })
    ));
}

#[test]
fn the_hubs_review_does_not_become_the_sites_review_by_being_shipped_with_the_bundle() {
    let subject = Subject::new(name("onco-core"), v(1, 5, 0), "sha256:core-1.5.0");
    let at_the_hub = TrustStanding::earned(
        id(ORIGIN),
        subject.clone(),
        &TierAssessment {
            earned: TrustTier::Reviewed,
            rungs: Vec::new(),
        },
    );
    assert!(at_the_hub.holds_in(&id(ORIGIN)));
    assert!(!at_the_hub.holds_in(&id(SITE)));

    let shipped = at_the_hub.attest();
    assert!(matches!(
        adopt(&id(SITE), &shipped, &AdoptionPolicy::new()),
        Err(FederationError::PeerNotRecognised { .. })
    ));

    let policy = AdoptionPolicy::new()
        .recognising(id(ORIGIN), TrustTier::GeneratedVerified)
        .expect("a ceiling below gold is grantable");
    let adoption = adopt(&id(SITE), &shipped, &policy).expect("the site decided in advance");
    assert_eq!(adoption.standing.tier(), TrustTier::GeneratedVerified);
    assert_eq!(adoption.capped_from, Some(TrustTier::Reviewed));
    assert_eq!(
        adoption.standing.basis(),
        &Basis::Adopted { from: id(ORIGIN) }
    );

    let relayed = adoption.standing.attest();
    let onward = AdoptionPolicy::new()
        .recognising(id(SITE), TrustTier::GeneratedVerified)
        .expect("grantable");
    assert!(matches!(
        adopt(&id(OTHER), &relayed, &onward),
        Err(FederationError::TrustWouldTransit { .. })
    ));
}

#[test]
fn the_site_cannot_answer_for_a_namespace_the_hub_never_delegated() {
    let request = Request::new(
        PackName::parse("partner/private-pack").expect("parses"),
        VersionReq::Any,
    )
    .under(FreshnessPolicy::OFFLINE);
    let error = resolve(&site(), &request).expect_err("the site carries bioprism and nothing else");
    assert!(matches!(error, ResolveError::Authority(_)));

    assert!(federation().audit().is_empty());
}

#[test]
fn a_pin_honoured_offline_is_the_same_pin_the_hub_would_have_given() {
    let pinned = Request::new(name("onco-core"), VersionReq::Exact(v(1, 5, 0)))
        .honouring_an_existing_pin()
        .under(FreshnessPolicy::OFFLINE);
    let from_site = resolve(&site(), &pinned).expect("resolves");
    let from_hub = resolve(&hub(), &pinned).expect("resolves");
    assert!(from_site.agrees_with(&from_hub));
    assert_eq!(from_site.digest(), "sha256:core-1.5.0");

    let admission = site()
        .lifecycle()
        .admits(&name("onco-core"), v(1, 5, 0), Intent::ExistingDependent)
        .expect("nothing is wrong with it");
    assert!(admission.is_unremarkable());
}
