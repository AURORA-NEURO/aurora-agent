//! Dependency resolution that reports what it could not do.
//!
//! Blueprint 10.04 asks for "dependencies are recursively pinned" and 10.08 adds "the resolver
//! checks conflicts". Neither says what the resolver *says* when there is a conflict, which is the
//! only part a person ever reads.
//!
//! # It always names two requirements
//!
//! When a constraint set on one name cannot be satisfied, this module reports
//! [`Collision`]: the name, and the two requirements that cannot both hold, each with the pack
//! that asked for it. Not a list of everything considered, not "no solution found" — a pair, which
//! is the smallest object that can be argued with.
//!
//! That is possible because [`crate::name::VersionReq`] denotes a contiguous interval and never a
//! union. In a totally ordered set the intersection of intervals `[lᵢ, hᵢ)` is
//! `[max lᵢ, min hⱼ)`, which is empty exactly when some `lᵢ ≥ hⱼ` — that is, exactly when some
//! *pair* is already empty. So a two-requirement witness is guaranteed to exist whenever the set
//! is unsatisfiable, and finding it is a scan rather than a search. Admit disjunctive requirements
//! and the guarantee dies with them, along with any hope of a comprehensible error.
//!
//! # No backtracking, and therefore no silent choice
//!
//! The resolver runs a fixpoint: given the versions currently selected, collect every requirement
//! their manifests impose, intersect per name, select the highest admissible version in each
//! interval, and repeat until the selection stops changing. Selecting is `max`, which is a
//! function of the constraint set, not a preference among alternatives. There is no point at which
//! it tries a version, finds it unworkable and quietly tries another — so there is no state in
//! which it knows something the caller does not.
//!
//! If the selection does not settle, that is [`DependencyError::DidNotStabilise`] naming the
//! oscillating packs, rather than whatever the last round happened to hold.
//!
//! # Three failures that are not the same failure
//!
//! - [`DependencyError::Collision`] — the requirements contradict each other. Nothing anybody
//!   publishes will fix it; one of the two requirements has to change.
//! - [`DependencyError::NoVersionSatisfies`] — the requirements are consistent and no published
//!   version lands in the interval they leave. Publishing one would fix it.
//! - [`DependencyError::Unresolvable`] — the versions exist and something else refused: a yank, a
//!   sunset pack line, a mirror too stale for the caller's policy.
//!
//! # Not implemented
//!
//! No feature flags, no optional dependencies, no platform selection, no dev-dependencies, no
//! workspace inheritance. 10.04 mentions none of them. No lockfile format either: a [`Lock`] is a
//! value with serde on it, and what a deployment writes to disk is its own business.

use crate::catalog::Catalog;
use crate::lifecycle::Note;
use crate::name::{Bounds, PackName, Version, VersionReq};
use crate::registry::Federation;
use crate::resolve::{resolve_in, Request, Resolution, ResolveError};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// How many fixpoint rounds are attempted before the resolver gives up and says so.
///
/// Each round can only learn about dependencies one level deeper, so a graph of depth *d* needs at
/// least *d* rounds. The bound is generous rather than tuned; exceeding it is reported, never
/// papered over with whatever the last round held.
const MAX_ROUNDS: usize = 64;

/// Who asked for a requirement.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(tag = "source", rename_all = "snake_case")]
pub enum Source {
    /// The caller's own request.
    Root,
    /// Another pack's manifest.
    Pack { name: PackName, version: Version },
}

impl fmt::Display for Source {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Source::Root => f.write_str("the root request"),
            Source::Pack { name, version } => write!(f, "{name}@{version}"),
        }
    }
}

/// One constraint on one name, and the pack that imposed it.
///
/// The source is not decoration. A collision report that named two version ranges without naming
/// who wanted them would tell a reader what is wrong and not where to go.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Requirement {
    pub on: PackName,
    pub req: VersionReq,
    pub source: Source,
}

impl fmt::Display for Requirement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} requires {} {}", self.source, self.on, self.req)
    }
}

/// Two requirements on one name that cannot both hold.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Collision {
    pub on: PackName,
    pub left: Requirement,
    pub right: Requirement,
}

impl fmt::Display for Collision {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} collides with {}", self.left, self.right)
    }
}

/// One pinned pack, with everything that asked for it and where the answer came from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Locked {
    pub resolution: Resolution,
    pub required_by: Vec<Requirement>,
}

impl Locked {
    pub fn version(&self) -> Version {
        self.resolution.version()
    }

    pub fn digest(&self) -> &str {
        self.resolution.digest()
    }

    pub fn notes(&self) -> &[Note] {
        &self.resolution.provenance().notes
    }
}

/// A resolved dependency closure.
///
/// Ordered by name, so two runs over the same inputs produce byte-identical serialisations. Every
/// entry keeps its [`Resolution`], which means the lock records not just what was chosen but which
/// registry said so and how current that registry's copy was — the same provenance a single
/// resolution carries, preserved through the closure rather than flattened out of it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Lock {
    entries: BTreeMap<String, Locked>,
}

impl Lock {
    pub fn get(&self, name: &PackName) -> Option<&Locked> {
        self.entries.get(&name.to_string())
    }

    pub fn version_of(&self, name: &PackName) -> Option<Version> {
        self.get(name).map(Locked::version)
    }

    pub fn entries(&self) -> impl Iterator<Item = (&String, &Locked)> {
        self.entries.iter()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// True when every entry was answered by the registry that owns its namespace.
    ///
    /// A lock that is not fully authoritative is not thereby wrong. It is a lock whose currency
    /// rests on mirrors, and that is a fact about the lock a reader should be able to obtain in
    /// one call rather than by inspecting every entry.
    pub fn is_fully_authoritative(&self) -> bool {
        self.entries
            .values()
            .all(|locked| locked.resolution.is_authoritative())
    }

    /// Every registry that contributed an answer.
    pub fn answering_registries(&self) -> BTreeSet<String> {
        self.entries
            .values()
            .map(|locked| locked.resolution.answered_by().to_string())
            .collect()
    }

    /// Every entry carrying a lifecycle note — yanked pins, deprecated lines.
    pub fn remarked(&self) -> Vec<&Locked> {
        self.entries
            .values()
            .filter(|locked| !locked.notes().is_empty())
            .collect()
    }
}

/// Resolves a root request and everything it transitively depends on.
///
/// The root request's [`Request::intent`], [`Request::freshness`] and [`Request::as_of`] apply to
/// every pack in the closure. That is deliberate: a caller honouring a lockfile is honouring it
/// for the whole graph, and a freshness policy that applied only to the root would be a policy
/// with a hole in it exactly where the transitive dependencies are.
pub fn resolve_dependencies(
    federation: &Federation,
    catalogs: &[Catalog],
    root: &Request,
) -> Result<Lock, DependencyError> {
    let mut selection: BTreeMap<PackName, Version> = BTreeMap::new();
    let mut seen: Vec<BTreeMap<PackName, Version>> = Vec::new();

    for _ in 0..MAX_ROUNDS {
        let requirements = collect(catalogs, root, &selection);
        let mut next: BTreeMap<PackName, Version> = BTreeMap::new();

        for (name, imposed) in &requirements {
            let bounds = intersect(imposed);
            if bounds.is_empty() {
                return Err(DependencyError::Collision(Box::new(
                    witness(name, imposed).expect(
                        "an empty intersection of intervals in a total order has a pairwise witness",
                    ),
                )));
            }
            let version = select(federation, catalogs, root, name, imposed, &bounds)?;
            next.insert(name.clone(), version);
        }

        if next == selection {
            return build(federation, catalogs, root, &requirements, &selection);
        }
        if seen.contains(&next) {
            return Err(DependencyError::DidNotStabilise {
                rounds: seen.len(),
                packs: oscillating(&seen, &next),
            });
        }
        seen.push(selection);
        selection = next;
    }

    Err(DependencyError::DidNotStabilise {
        rounds: MAX_ROUNDS,
        packs: selection.keys().map(PackName::to_string).collect(),
    })
}

/// Walks the graph from the root, expanding a pack's own requirements only once a version has been
/// selected for it. That is why the fixpoint needs a round per level of depth.
fn collect(
    catalogs: &[Catalog],
    root: &Request,
    selection: &BTreeMap<PackName, Version>,
) -> BTreeMap<PackName, Vec<Requirement>> {
    let mut requirements: BTreeMap<PackName, Vec<Requirement>> = BTreeMap::new();
    let mut queue = vec![Requirement {
        on: root.name.clone(),
        req: root.req,
        source: Source::Root,
    }];
    let mut expanded: BTreeSet<(PackName, Version)> = BTreeSet::new();

    while let Some(requirement) = queue.pop() {
        let name = requirement.on.clone();
        requirements
            .entry(name.clone())
            .or_default()
            .push(requirement);

        let Some(version) = selection.get(&name).copied() else {
            continue;
        };
        if !expanded.insert((name.clone(), version)) {
            continue;
        }
        for catalog in catalogs {
            let Some(release) = catalog.release(&name, &version) else {
                continue;
            };
            for dependency in &release.dependencies {
                queue.push(Requirement {
                    on: dependency.name.clone(),
                    req: dependency.req,
                    source: Source::Pack {
                        name: name.clone(),
                        version,
                    },
                });
            }
            break;
        }
    }

    for imposed in requirements.values_mut() {
        imposed.sort();
        imposed.dedup();
    }
    requirements
}

fn intersect(imposed: &[Requirement]) -> Bounds {
    imposed.iter().fold(Bounds::UNBOUNDED, |acc, requirement| {
        acc.intersect(&requirement.req.bounds())
    })
}

/// Finds the pair that proves the set unsatisfiable. See the module docs for why one exists.
fn witness(name: &PackName, imposed: &[Requirement]) -> Option<Collision> {
    for (index, left) in imposed.iter().enumerate() {
        for right in &imposed[index + 1..] {
            if left.req.bounds().intersect(&right.req.bounds()).is_empty() {
                return Some(Collision {
                    on: name.clone(),
                    left: left.clone(),
                    right: right.clone(),
                });
            }
        }
    }
    None
}

/// The highest published version inside the interval that the registries will actually hand over.
fn select(
    federation: &Federation,
    catalogs: &[Catalog],
    root: &Request,
    name: &PackName,
    imposed: &[Requirement],
    bounds: &Bounds,
) -> Result<Version, DependencyError> {
    let request = narrowed(root, name, bounds, imposed);
    match resolve_in(federation, catalogs, &request) {
        Ok(resolution) => Ok(resolution.version()),
        Err(ResolveError::NoVersionInRange { held, .. }) => {
            Err(DependencyError::NoVersionSatisfies {
                on: name.to_string(),
                bounds: bounds.to_string(),
                required_by: imposed.to_vec(),
                held,
            })
        }
        Err(error) => Err(DependencyError::Unresolvable {
            on: name.to_string(),
            required_by: imposed.to_vec(),
            cause: Box::new(error),
        }),
    }
}

/// Turns an interval back into a request. `None` bounds become [`VersionReq::Any`] and a bounded
/// interval becomes a [`VersionReq::Range`], so the resolver asks a registry exactly the question
/// the constraint set left open — never a widened one.
fn narrowed(root: &Request, name: &PackName, bounds: &Bounds, imposed: &[Requirement]) -> Request {
    let req = match (bounds.low, bounds.high) {
        (None, None) => VersionReq::Any,
        (Some(low), None) => VersionReq::AtLeast(low),
        (None, Some(high)) => VersionReq::Range {
            low: Version::ZERO,
            high,
        },
        (Some(low), Some(high)) => VersionReq::Range { low, high },
    };
    let exact = imposed
        .iter()
        .find_map(|requirement| match requirement.req {
            VersionReq::Exact(version) => Some(version),
            _ => None,
        });
    Request {
        name: name.clone(),
        req: exact.map(VersionReq::Exact).unwrap_or(req),
        intent: root.intent,
        freshness: root.freshness,
        as_of: root.as_of,
    }
}

fn build(
    federation: &Federation,
    catalogs: &[Catalog],
    root: &Request,
    requirements: &BTreeMap<PackName, Vec<Requirement>>,
    selection: &BTreeMap<PackName, Version>,
) -> Result<Lock, DependencyError> {
    let mut entries = BTreeMap::new();
    for (name, imposed) in requirements {
        let version = selection
            .get(name)
            .copied()
            .expect("the fixpoint selected a version for every required name");
        let request = Request {
            name: name.clone(),
            req: VersionReq::Exact(version),
            intent: root.intent,
            freshness: root.freshness,
            as_of: root.as_of,
        };
        let resolution = resolve_in(federation, catalogs, &request).map_err(|error| {
            DependencyError::Unresolvable {
                on: name.to_string(),
                required_by: imposed.to_vec(),
                cause: Box::new(error),
            }
        })?;
        entries.insert(
            name.to_string(),
            Locked {
                resolution,
                required_by: imposed.clone(),
            },
        );
    }
    Ok(Lock { entries })
}

fn oscillating(
    seen: &[BTreeMap<PackName, Version>],
    repeat: &BTreeMap<PackName, Version>,
) -> Vec<String> {
    let mut unstable: BTreeSet<String> = BTreeSet::new();
    for earlier in seen {
        for (name, version) in repeat {
            if earlier.get(name) != Some(version) {
                unstable.insert(name.to_string());
            }
        }
    }
    unstable.into_iter().collect()
}

/// Why a dependency closure could not be produced.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DependencyError {
    #[error("{} has no satisfiable version: {}", .0.on, .0)]
    Collision(Box<Collision>),

    #[error("{on} is required at {bounds}, and no published version is in that range (held: {})", held.join(", "))]
    NoVersionSatisfies {
        on: String,
        bounds: String,
        required_by: Vec<Requirement>,
        held: Vec<String>,
    },

    #[error("{on} could not be resolved: {cause}")]
    Unresolvable {
        on: String,
        required_by: Vec<Requirement>,
        cause: Box<ResolveError>,
    },

    #[error("the selection did not settle after {rounds} round(s); unstable: {}", packs.join(", "))]
    DidNotStabilise { rounds: usize, packs: Vec<String> },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::PackRelease;
    use crate::mirror::{FreshnessPolicy, Replication, StalenessBound};
    use crate::name::Namespace;
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

    fn empty_origin() -> Catalog {
        Catalog::origin(
            Authority::new(id("origin"))
                .owning(ns("bioprism"))
                .expect("owns"),
        )
    }

    fn release(local: &str, version: Version) -> PackRelease {
        PackRelease::new(
            name(&format!("bioprism/{local}")),
            version,
            format!("sha256:{local}-{version}"),
        )
    }

    /// `app` depends on `core ^1.0`; `core` has 1.0.0, 1.4.0 and 2.0.0; `probe` needs `core ^2.0`.
    fn graph() -> Catalog {
        let mut catalog = empty_origin();
        for version in [v(1, 0, 0), v(1, 4, 0), v(2, 0, 0)] {
            catalog.record(release("core", version)).expect("records");
        }
        catalog
            .record(
                release("app", v(1, 0, 0))
                    .depending_on(name("bioprism/core"), VersionReq::Compatible(v(1, 0, 0))),
            )
            .expect("records");
        catalog
            .record(
                release("probe", v(1, 0, 0))
                    .depending_on(name("bioprism/core"), VersionReq::Compatible(v(2, 0, 0))),
            )
            .expect("records");
        catalog
            .record(
                release("suite", v(1, 0, 0))
                    .depending_on(name("bioprism/app"), VersionReq::Any)
                    .depending_on(name("bioprism/probe"), VersionReq::Any),
            )
            .expect("records");
        catalog
    }

    fn request(local: &str) -> Request {
        Request::new(name(&format!("bioprism/{local}")), VersionReq::Any)
            .under(FreshnessPolicy::OFFLINE)
    }

    #[test]
    fn a_closure_pins_every_transitive_dependency() {
        let lock =
            resolve_dependencies(&federation(), &[graph()], &request("app")).expect("resolves");
        assert_eq!(lock.len(), 2);
        assert_eq!(lock.version_of(&name("bioprism/app")), Some(v(1, 0, 0)));
        assert_eq!(lock.version_of(&name("bioprism/core")), Some(v(1, 4, 0)));
    }

    #[test]
    fn an_unsatisfiable_constraint_set_names_the_two_requirements_that_collide() {
        let error = resolve_dependencies(&federation(), &[graph()], &request("suite"))
            .expect_err("core cannot be both ^1 and ^2");
        let DependencyError::Collision(collision) = error else {
            panic!("expected a collision, got {error}");
        };
        assert_eq!(collision.on, name("bioprism/core"));
        let sources = [
            collision.left.source.to_string(),
            collision.right.source.to_string(),
        ];
        assert!(sources.contains(&"bioprism/app@1.0.0".to_string()));
        assert!(sources.contains(&"bioprism/probe@1.0.0".to_string()));
        assert_ne!(collision.left.req, collision.right.req);
    }

    #[test]
    fn a_collision_is_reported_rather_than_a_version_being_quietly_chosen() {
        let error = resolve_dependencies(&federation(), &[graph()], &request("suite"))
            .expect_err("no silent pick");
        assert!(matches!(error, DependencyError::Collision(_)));
    }

    #[test]
    fn contradictory_requirements_and_an_unpublished_range_are_different_errors() {
        let mut catalog = empty_origin();
        catalog
            .record(release("core", v(1, 0, 0)))
            .expect("records");
        catalog
            .record(
                release("app", v(1, 0, 0))
                    .depending_on(name("bioprism/core"), VersionReq::Compatible(v(3, 0, 0))),
            )
            .expect("records");

        let error = resolve_dependencies(&federation(), &[catalog], &request("app"))
            .expect_err("nobody published core 3");
        assert!(matches!(
            error,
            DependencyError::NoVersionSatisfies { ref held, .. } if held == &["1.0.0"]
        ));
    }

    #[test]
    fn the_intersection_of_consistent_requirements_narrows_the_selection() {
        let mut catalog = empty_origin();
        for version in [v(1, 0, 0), v(1, 4, 0), v(1, 9, 0)] {
            catalog.record(release("core", version)).expect("records");
        }
        catalog
            .record(
                release("app", v(1, 0, 0))
                    .depending_on(name("bioprism/core"), VersionReq::Compatible(v(1, 0, 0))),
            )
            .expect("records");
        catalog
            .record(
                release("probe", v(1, 0, 0))
                    .depending_on(name("bioprism/core"), VersionReq::Approximately(v(1, 4, 0))),
            )
            .expect("records");
        catalog
            .record(
                release("suite", v(1, 0, 0))
                    .depending_on(name("bioprism/app"), VersionReq::Any)
                    .depending_on(name("bioprism/probe"), VersionReq::Any),
            )
            .expect("records");

        let lock = resolve_dependencies(&federation(), &[catalog], &request("suite"))
            .expect("^1.0 and ~1.4 overlap at 1.4.x");
        assert_eq!(lock.version_of(&name("bioprism/core")), Some(v(1, 4, 0)));
    }

    #[test]
    fn a_lock_records_which_registry_answered_for_every_entry() {
        let lock =
            resolve_dependencies(&federation(), &[graph()], &request("app")).expect("resolves");
        assert!(lock.is_fully_authoritative());
        assert_eq!(
            lock.answering_registries().into_iter().collect::<Vec<_>>(),
            ["origin"]
        );
    }

    #[test]
    fn a_closure_answered_by_a_mirror_is_not_reported_as_authoritative() {
        let mut copy = Catalog::mirror(
            Authority::new(id("site-mirror"))
                .carrying(ns("bioprism"), id("origin"))
                .expect("carries"),
            Replication::mirror(id("origin"), Epoch(4), StalenessBound::epochs(2)),
        );
        for release in graph().releases() {
            copy.record(release.clone()).expect("records");
        }
        let lock = resolve_dependencies(&federation(), &[copy], &request("app"))
            .expect("resolves offline");
        assert_eq!(lock.len(), 2);
        assert!(!lock.is_fully_authoritative());
        assert_eq!(
            lock.answering_registries().into_iter().collect::<Vec<_>>(),
            ["site-mirror"]
        );
    }

    #[test]
    fn a_yanked_transitive_dependency_is_stepped_over_for_a_new_dependent() {
        let mut catalog = graph();
        catalog
            .lifecycle_mut()
            .yank(&name("bioprism/core"), v(1, 4, 0), "leaked label", Epoch(2))
            .expect("yanks");
        let lock = resolve_dependencies(&federation(), &[catalog], &request("app"))
            .expect("falls back to 1.0.0");
        assert_eq!(lock.version_of(&name("bioprism/core")), Some(v(1, 0, 0)));
        assert!(lock.remarked().is_empty());
    }

    #[test]
    fn a_yanked_transitive_dependency_is_kept_for_an_existing_dependent_and_flagged() {
        let mut catalog = graph();
        catalog
            .lifecycle_mut()
            .yank(&name("bioprism/core"), v(1, 4, 0), "leaked label", Epoch(2))
            .expect("yanks");
        let lock = resolve_dependencies(
            &federation(),
            &[catalog],
            &request("app").honouring_an_existing_pin(),
        )
        .expect("a yank does not break an existing closure");
        assert_eq!(lock.version_of(&name("bioprism/core")), Some(v(1, 4, 0)));
        assert_eq!(lock.remarked().len(), 1);
    }

    #[test]
    fn a_withdrawn_dependency_stops_the_closure_instead_of_sliding_to_a_neighbour() {
        let mut catalog = graph();
        for version in [v(1, 0, 0), v(1, 4, 0)] {
            catalog
                .lifecycle_mut()
                .withdraw(
                    &name("bioprism/core"),
                    version,
                    "the archive shipped a live credential",
                    "BIOPRISM-2026-04",
                    Epoch(2),
                )
                .expect("withdraws");
        }
        let error = resolve_dependencies(&federation(), &[catalog], &request("app"))
            .expect_err("a withdrawal is not a preference");
        assert!(matches!(error, DependencyError::Unresolvable { .. }));
    }

    #[test]
    fn an_exact_pin_survives_the_intersection_that_would_otherwise_widen_it() {
        let mut catalog = empty_origin();
        for version in [v(1, 0, 0), v(1, 4, 0)] {
            catalog.record(release("core", version)).expect("records");
        }
        catalog
            .record(
                release("app", v(1, 0, 0))
                    .depending_on(name("bioprism/core"), VersionReq::Exact(v(1, 0, 0))),
            )
            .expect("records");
        let lock =
            resolve_dependencies(&federation(), &[catalog], &request("app")).expect("resolves");
        assert_eq!(lock.version_of(&name("bioprism/core")), Some(v(1, 0, 0)));
    }

    #[test]
    fn a_dependency_cycle_resolves_rather_than_looping_forever() {
        let mut catalog = empty_origin();
        catalog
            .record(
                release("left", v(1, 0, 0))
                    .depending_on(name("bioprism/right"), VersionReq::Compatible(v(1, 0, 0))),
            )
            .expect("records");
        catalog
            .record(
                release("right", v(1, 0, 0))
                    .depending_on(name("bioprism/left"), VersionReq::Compatible(v(1, 0, 0))),
            )
            .expect("records");
        let lock = resolve_dependencies(&federation(), &[catalog], &request("left"))
            .expect("a cycle among consistent constraints has a fixpoint");
        assert_eq!(lock.len(), 2);
    }

    #[test]
    fn every_requirement_that_led_to_a_pin_is_recorded_against_it() {
        let lock =
            resolve_dependencies(&federation(), &[graph()], &request("app")).expect("resolves");
        let core = lock.get(&name("bioprism/core")).expect("pinned");
        assert_eq!(core.required_by.len(), 1);
        assert_eq!(
            core.required_by[0].source,
            Source::Pack {
                name: name("bioprism/app"),
                version: v(1, 0, 0),
            }
        );
        let app = lock.get(&name("bioprism/app")).expect("pinned");
        assert_eq!(app.required_by[0].source, Source::Root);
    }

    #[test]
    fn a_lock_survives_a_json_round_trip_with_its_provenance_intact() {
        let lock =
            resolve_dependencies(&federation(), &[graph()], &request("app")).expect("resolves");
        let text = serde_json::to_string(&lock).expect("serialises");
        let back: Lock = serde_json::from_str(&text).expect("deserialises");
        assert_eq!(back, lock);
        assert!(back.is_fully_authoritative());
    }

    #[test]
    fn resolution_is_deterministic_across_repeated_runs() {
        let first =
            resolve_dependencies(&federation(), &[graph()], &request("app")).expect("resolves");
        let second =
            resolve_dependencies(&federation(), &[graph()], &request("app")).expect("resolves");
        assert_eq!(
            serde_json::to_string(&first).expect("serialises"),
            serde_json::to_string(&second).expect("serialises")
        );
    }

    #[test]
    fn a_collision_witness_exists_for_every_unsatisfiable_interval_set() {
        let on = name("bioprism/core");
        let imposed = vec![
            Requirement {
                on: on.clone(),
                req: VersionReq::AtLeast(v(2, 0, 0)),
                source: Source::Root,
            },
            Requirement {
                on: on.clone(),
                req: VersionReq::Compatible(v(1, 0, 0)),
                source: Source::Root,
            },
            Requirement {
                on: on.clone(),
                req: VersionReq::Any,
                source: Source::Root,
            },
        ];
        assert!(intersect(&imposed).is_empty());
        let found = witness(&on, &imposed).expect("a pairwise witness always exists");
        assert!(found
            .left
            .req
            .bounds()
            .intersect(&found.right.req.bounds())
            .is_empty());
    }
}
