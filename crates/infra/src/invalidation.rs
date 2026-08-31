//! Invalidation that reports its own completeness.
//!
//! Blueprint 12.08 disposes of invalidation in one sentence: "content-addressed immutability
//! avoids most invalidation. Mutable price, health, and advisory data use explicit
//! version/expiry." That is true and it is not enough. *Most* is doing the work, and the
//! residue — the entries whose inputs are mutable, or whose dependency set nobody wrote down —
//! is exactly where a cache serves a stale answer to a reproducibility platform.
//!
//! # The rule
//!
//! Given a change to a resource, an invalidation must answer *which entries are now invalid*.
//! It can only answer that from declared dependencies. Where the declaration is missing, the
//! honest answer is not "no entries" — it is "these entries, plus an unknown region I cannot
//! see into". So [`InvalidationPlan::completeness`] is either [`Completeness::Complete`] or
//! [`Completeness::Partial`] carrying an [`UnknownRegion`] that names, concretely:
//!
//! - the **opaque resources** reached, which might depend on the change without saying so;
//! - the **entries with no declared dependencies**, which might depend on anything;
//! - the **entries that depend on an opaque resource**, which inherit its uncertainty.
//!
//! `bioprism-lens` and `bioprism-section` refuse to collapse "checked and clean" into "not
//! checked". This is the same refusal applied to a cache: an invalidation that could not be
//! proved total is never reported as total, and [`crate::cache::Cache::apply`] responds to a
//! partial plan by marking the unknown region unprovable rather than leaving it servable.
//!
//! # Deliberately not implemented
//!
//! No incremental maintenance, no change-data-capture, no subscription to an upstream. A plan is
//! computed from scratch over the whole declared graph each time; the graphs this crate is sized
//! for are configuration-scale, not corpus-scale, and `bioprism-graph` is where a real traversal
//! engine belongs. There is also no notion of a *partial* change to a resource: a resource
//! either changed or it did not.

use crate::error::InvalidationError;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt;

/// A thing a cached computation can depend on: a dataset release, a schema, a policy, a table.
///
/// Deliberately a string newtype rather than a typed union. The set of things a computation can
/// depend on is open — 12.10 lists "artifact digests, schemas, code, configuration, model
/// resolution, prompts, tools, permissions, seed, time/network fixture, platform, evaluator, and
/// policy" and does not claim the list is closed — and a closed enum here would force callers to
/// smuggle unmodelled dependencies through a `Other(String)` variant, which is the same string
/// with extra ceremony.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ResourceId(String);

impl ResourceId {
    pub fn parse(value: impl Into<String>) -> Result<Self, InvalidationError> {
        let value = value.into();
        if value.trim().is_empty() || value.chars().any(char::is_control) {
            return Err(InvalidationError::MalformedResource(value));
        }
        Ok(ResourceId(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ResourceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<ResourceId> for String {
    fn from(value: ResourceId) -> Self {
        value.0
    }
}

impl TryFrom<String> for ResourceId {
    type Error = InvalidationError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        ResourceId::parse(value)
    }
}

/// What a cache entry says about what it depends on.
///
/// The two variants are not a boolean with a payload. `Undeclared` is a positive statement that
/// nobody wrote down the dependencies — it is the state a legacy entry, an imported entry or a
/// hastily-added call site is in — and it is what makes an invalidation partial. An
/// implementation that defaulted it to an empty `Declared` set would turn "unknown" into
/// "depends on nothing", which is the strongest possible claim, asserted from ignorance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DependencyDeclaration {
    /// The complete set of resources this entry's value depends on.
    Declared(BTreeSet<ResourceId>),
    /// Nobody stated the dependencies. Any change may invalidate this entry.
    Undeclared,
}

impl DependencyDeclaration {
    /// A declaration over the given resources. An empty set is a real claim — "this value
    /// depends on nothing mutable" — and is kept distinct from [`DependencyDeclaration::Undeclared`].
    pub fn on(resources: impl IntoIterator<Item = ResourceId>) -> Self {
        DependencyDeclaration::Declared(resources.into_iter().collect())
    }

    pub fn is_declared(&self) -> bool {
        matches!(self, DependencyDeclaration::Declared(_))
    }

    pub fn resources(&self) -> Option<&BTreeSet<ResourceId>> {
        match self {
            DependencyDeclaration::Declared(set) => Some(set),
            DependencyDeclaration::Undeclared => None,
        }
    }
}

/// Declared dependencies between resources, plus the resources known to be undeclared.
///
/// Edges point from a resource to the resources it depends on. Invalidation walks them in
/// reverse: a change to `X` affects everything that reaches `X`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DependencyGraph {
    depends_on: BTreeMap<ResourceId, BTreeSet<ResourceId>>,
    opaque: BTreeSet<ResourceId>,
}

impl DependencyGraph {
    pub fn new() -> Self {
        DependencyGraph::default()
    }

    /// States that `resource` depends on exactly `dependencies` and on nothing else.
    pub fn declare(
        &mut self,
        resource: ResourceId,
        dependencies: impl IntoIterator<Item = ResourceId>,
    ) -> Result<(), InvalidationError> {
        if self.opaque.contains(&resource) {
            return Err(InvalidationError::ContradictoryDeclaration(
                resource.as_str().to_string(),
            ));
        }
        self.depends_on
            .entry(resource)
            .or_default()
            .extend(dependencies);
        Ok(())
    }

    /// States that `resource` exists but its dependencies are unknown.
    ///
    /// This is the honest way to record an upstream nobody has mapped — a vendor feed, a manual
    /// export, a table maintained by another team. Every invalidation that could reach it is
    /// reported as partial, which is the correct cost of not having written the edges down.
    pub fn declare_opaque(&mut self, resource: ResourceId) -> Result<(), InvalidationError> {
        if self.depends_on.contains_key(&resource) {
            return Err(InvalidationError::ContradictoryDeclaration(
                resource.as_str().to_string(),
            ));
        }
        self.opaque.insert(resource);
        Ok(())
    }

    pub fn opaque_resources(&self) -> &BTreeSet<ResourceId> {
        &self.opaque
    }

    /// Resources with a declared dependency set, plus every resource named as a dependency.
    pub fn known_resources(&self) -> BTreeSet<ResourceId> {
        let mut known: BTreeSet<ResourceId> = self.depends_on.keys().cloned().collect();
        for dependencies in self.depends_on.values() {
            known.extend(dependencies.iter().cloned());
        }
        known.extend(self.opaque.iter().cloned());
        known
    }

    fn dependents(&self) -> BTreeMap<ResourceId, BTreeSet<ResourceId>> {
        let mut reverse: BTreeMap<ResourceId, BTreeSet<ResourceId>> = BTreeMap::new();
        for (resource, dependencies) in &self.depends_on {
            for dependency in dependencies {
                reverse
                    .entry(dependency.clone())
                    .or_default()
                    .insert(resource.clone());
            }
        }
        reverse
    }

    /// Everything that transitively depends on `changed`, including `changed` itself.
    ///
    /// Breadth-first with a visited set, so a declared cycle terminates instead of hanging. A
    /// cycle is a modelling error rather than a fault here — see [`DependencyGraph::find_cycle`]
    /// for a caller that wants to know — but it must not be able to wedge an invalidation.
    pub fn affected_by(&self, changed: &ResourceId) -> BTreeSet<ResourceId> {
        let reverse = self.dependents();
        let mut affected = BTreeSet::new();
        let mut queue = VecDeque::new();
        queue.push_back(changed.clone());
        while let Some(current) = queue.pop_front() {
            if !affected.insert(current.clone()) {
                continue;
            }
            if let Some(dependents) = reverse.get(&current) {
                for dependent in dependents {
                    queue.push_back(dependent.clone());
                }
            }
        }
        affected
    }

    /// Walks *downward* from a set of resources and reports every resource reached whose own
    /// dependencies are not declared.
    ///
    /// This is what makes opacity transitive, and it is the subtle half of the completeness
    /// question. An entry that depends only on well-declared resources is still unprovable if
    /// one of *those* resources depends on something opaque: the opaque node might depend on the
    /// change, so the entry might too. A one-hop check would call that entry proved-unaffected
    /// and it would be wrong.
    ///
    /// Returns the explicitly-opaque resources reached and, separately, the resources reached
    /// that the graph has never heard of at all. The second set is the likelier failure in
    /// practice: someone declares a dependency on a resource nobody added to the graph, and a
    /// naive implementation reads the empty edge set as "depends on nothing".
    pub fn opacity_reached_from(
        &self,
        from: &BTreeSet<ResourceId>,
    ) -> (BTreeSet<ResourceId>, BTreeSet<ResourceId>) {
        let mut opaque = BTreeSet::new();
        let mut unknown = BTreeSet::new();
        let mut seen: BTreeSet<ResourceId> = BTreeSet::new();
        let mut queue: VecDeque<ResourceId> = from.iter().cloned().collect();
        while let Some(current) = queue.pop_front() {
            if !seen.insert(current.clone()) {
                continue;
            }
            if self.opaque.contains(&current) {
                opaque.insert(current);
                continue;
            }
            match self.depends_on.get(&current) {
                Some(dependencies) => queue.extend(dependencies.iter().cloned()),
                None => {
                    unknown.insert(current);
                }
            }
        }
        (opaque, unknown)
    }

    /// Reports one declared dependency cycle, if any exists.
    ///
    /// Offered because a cycle in a *declared* dependency graph means two resources each claim
    /// to be downstream of the other, and no rebuild order satisfies both. Invalidation survives
    /// it; a rebuild scheduler would not.
    pub fn find_cycle(&self) -> Option<Vec<ResourceId>> {
        let mut settled: BTreeSet<ResourceId> = BTreeSet::new();
        for start in self.depends_on.keys() {
            let mut path: Vec<ResourceId> = Vec::new();
            let mut on_path: BTreeSet<ResourceId> = BTreeSet::new();
            if let Some(cycle) = self.walk(start, &mut path, &mut on_path, &mut settled) {
                return Some(cycle);
            }
        }
        None
    }

    fn walk(
        &self,
        node: &ResourceId,
        path: &mut Vec<ResourceId>,
        on_path: &mut BTreeSet<ResourceId>,
        settled: &mut BTreeSet<ResourceId>,
    ) -> Option<Vec<ResourceId>> {
        if on_path.contains(node) {
            let start = path.iter().position(|seen| seen == node).unwrap_or(0);
            let mut cycle = path[start..].to_vec();
            cycle.push(node.clone());
            return Some(cycle);
        }
        if settled.contains(node) {
            return None;
        }
        on_path.insert(node.clone());
        path.push(node.clone());
        if let Some(dependencies) = self.depends_on.get(node) {
            for dependency in dependencies {
                if let Some(cycle) = self.walk(dependency, path, on_path, settled) {
                    return Some(cycle);
                }
            }
        }
        path.pop();
        on_path.remove(node);
        settled.insert(node.clone());
        None
    }
}

/// The region an invalidation could not see into.
///
/// Every field is a set of concrete names, not a count and not a flag. An operator holding this
/// can go and declare the missing edges; an operator holding `partial: true` can only guess.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnknownRegion {
    /// Resources an entry reaches whose own dependencies were explicitly declared unknown.
    pub opaque_resources: BTreeSet<ResourceId>,
    /// Resources an entry reaches that the graph has never heard of. Distinct from opaque
    /// because the remedy differs: an opaque resource was mapped and found unmappable, an
    /// unknown one was simply never added.
    pub unknown_resources: BTreeSet<ResourceId>,
    /// Entries that never declared what they depend on.
    pub entries_without_declared_dependencies: BTreeSet<String>,
    /// Entries whose declared dependencies reach an opaque or unknown resource.
    pub entries_depending_on_opaque_resources: BTreeSet<String>,
}

impl UnknownRegion {
    pub fn is_empty(&self) -> bool {
        self.opaque_resources.is_empty()
            && self.unknown_resources.is_empty()
            && self.entries_without_declared_dependencies.is_empty()
            && self.entries_depending_on_opaque_resources.is_empty()
    }

    /// Every entry in the region, by digest.
    pub fn entries(&self) -> BTreeSet<String> {
        let mut all = self.entries_without_declared_dependencies.clone();
        all.extend(self.entries_depending_on_opaque_resources.iter().cloned());
        all
    }
}

/// Whether the invalidation is total or merely as much as could be proved.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Completeness {
    /// Every entry that could be affected was identified. The graph was fully declared over the
    /// reachable region and every entry declared its dependencies.
    Complete,
    /// Some entries could neither be proved affected nor proved unaffected.
    Partial(UnknownRegion),
}

impl Completeness {
    pub fn is_complete(&self) -> bool {
        matches!(self, Completeness::Complete)
    }

    pub fn unknown_region(&self) -> Option<&UnknownRegion> {
        match self {
            Completeness::Complete => None,
            Completeness::Partial(region) => Some(region),
        }
    }
}

/// What a change implies for a set of cache entries.
///
/// Produced by [`InvalidationPlan::compute`] and consumed by [`crate::cache::Cache::apply`].
/// Computing it is pure: nothing is dropped until a caller applies the plan, which mirrors the
/// dry-run default 12.22 requires of garbage collection and lets an operator read the
/// completeness verdict before acting on it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InvalidationPlan {
    pub changed: ResourceId,
    /// `changed` plus everything that transitively depends on it through declared edges.
    pub affected_resources: BTreeSet<ResourceId>,
    /// Entries proved invalid: they declared a dependency on an affected resource.
    pub invalid_entries: BTreeSet<String>,
    /// Entries proved unaffected: they declared dependencies and none of them are affected, and
    /// none of them are opaque.
    pub proved_unaffected: BTreeSet<String>,
    pub completeness: Completeness,
    /// The number of entries the plan was computed over, so applying it to a different
    /// population is refused rather than silently partial.
    pub population: usize,
}

impl InvalidationPlan {
    /// Computes the consequences of `changed` for `entries`.
    ///
    /// `entries` is `(digest, declaration)` pairs, which is what a cache can hand over without
    /// exposing its values.
    pub fn compute<'a>(
        graph: &DependencyGraph,
        changed: ResourceId,
        entries: impl IntoIterator<Item = (String, &'a DependencyDeclaration)>,
    ) -> Self {
        let affected = graph.affected_by(&changed);

        let mut invalid = BTreeSet::new();
        let mut unaffected = BTreeSet::new();
        let mut region = UnknownRegion::default();
        let mut population = 0usize;

        for (digest, declaration) in entries {
            population += 1;
            match declaration {
                DependencyDeclaration::Undeclared => {
                    region
                        .entries_without_declared_dependencies
                        .insert(digest.clone());
                }
                DependencyDeclaration::Declared(resources) => {
                    if resources.iter().any(|resource| affected.contains(resource)) {
                        invalid.insert(digest.clone());
                        continue;
                    }
                    let (opaque, unknown) = graph.opacity_reached_from(resources);
                    if opaque.is_empty() && unknown.is_empty() {
                        unaffected.insert(digest.clone());
                    } else {
                        region.opaque_resources.extend(opaque);
                        region.unknown_resources.extend(unknown);
                        region
                            .entries_depending_on_opaque_resources
                            .insert(digest.clone());
                    }
                }
            }
        }

        let completeness = if region.is_empty() {
            Completeness::Complete
        } else {
            Completeness::Partial(region)
        };

        InvalidationPlan {
            changed,
            affected_resources: affected,
            invalid_entries: invalid,
            proved_unaffected: unaffected,
            completeness,
            population,
        }
    }

    pub fn is_complete(&self) -> bool {
        self.completeness.is_complete()
    }

    /// Entries this plan cannot vouch for either way.
    pub fn unproven_entries(&self) -> BTreeSet<String> {
        self.completeness
            .unknown_region()
            .map(UnknownRegion::entries)
            .unwrap_or_default()
    }
}
