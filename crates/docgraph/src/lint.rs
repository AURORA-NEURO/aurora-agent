//! Graph linting (41.11).
//!
//! 41.11 asks for validation of "IDs, paths, edges, route closure, hashes, front matter, links,
//! examples, and graph-to-file consistency", with invariants that a release has "no broken
//! links", "every route node exists", "every graph node resolves", and "orphan normative modules
//! are reviewed".
//!
//! # Findings are data, not errors
//!
//! [`lint`] never fails. It takes a corpus that may be broken in several ways at once and returns
//! all of them, because an author fixing a documentation graph wants the list, not the first
//! item. The consumer decides what is fatal: [`LintReport::has_errors`] separates
//! [`LintSeverity::Error`] from [`LintSeverity::Warning`], and only the bundle compiler turns a
//! defect into a hard failure — and only when the defect would reach a delivered context.
//!
//! # What each check is actually for
//!
//! Two checks carry the weight of the edge vocabulary and deserve stating plainly. A
//! [`LintFinding::UnresolvedContradiction`] is a `contradicts` edge with no `resolved_by`: the
//! corpus states two things that cannot both be true and names nothing that settles it, so any
//! bundle touching either side must carry both, and a reader has to adjudicate. A
//! [`LintFinding::CycleInAcyclicEdgeType`] is a cycle over an edge type whose meaning is an
//! ordering — 41.03 mandates this for `requires` and lists "graph contains a cycle interpreted as
//! build order" among its failure modes; the check generalises to every type where
//! [`DocEdgeType::acyclic`] holds, because the argument was never specific to `requires`.
//!
//! # Not implemented
//!
//! No filesystem verification. [`lint`] never checks that [`ModuleNode::path`] exists on disk or
//! that a hash matches the bytes currently at that path, because this module takes a
//! [`DocGraph`] and a graph does not know where it came from. That check belongs to whoever built
//! the graph; [`crate::scan`] does it at scan time, where the bytes are in hand.
//!
//! [`ModuleNode::path`]: crate::registry::ModuleNode::path

use crate::registry::{DocGraph, ModuleId};
use crate::route::{RouteDefect, TaskRoute};
use crate::tokens::{estimate_tokens, ProfileLevel};
use crate::vocabulary::DocEdgeType;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// 41.04 sizes a context card at 60–180 tokens. Over the ceiling and the card has stopped being
/// the cheap thing an agent reads before deciding.
pub const CARD_TOKEN_CEILING: u32 = 180;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LintSeverity {
    /// The corpus cannot be released in this state.
    Error,
    /// Reviewable. 41.11 asks that orphan normative modules be "reviewed", not rejected.
    Warning,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "finding")]
pub enum LintFinding {
    /// An edge names a module the registry does not hold. 41.11: "every graph node resolves".
    DanglingEdge {
        from: ModuleId,
        to: ModuleId,
        kind: DocEdgeType,
        missing: ModuleId,
    },
    /// An edge from a module to itself. Meaningless under every member of the vocabulary.
    SelfEdge { module: ModuleId, kind: DocEdgeType },
    /// A normative module with no incoming edges: a contract nobody builds against.
    OrphanModule { module: ModuleId },
    /// A cycle over an edge type whose meaning is an ordering.
    CycleInAcyclicEdgeType {
        kind: DocEdgeType,
        cycle: Vec<ModuleId>,
    },
    /// A `contradicts` edge with no module named as settling it.
    UnresolvedContradiction { a: ModuleId, b: ModuleId },
    /// A `contradicts` edge whose named resolver is not in the registry.
    ContradictionResolverMissing {
        a: ModuleId,
        b: ModuleId,
        resolver: ModuleId,
    },
    /// A live module still requires one that has been superseded.
    RequiresSupersededModule {
        requirer: ModuleId,
        superseded: ModuleId,
        successor: ModuleId,
    },
    /// A module marked [`NodeStatus::Withdrawn`](crate::registry::NodeStatus::Withdrawn) with no
    /// `supersedes` edge pointing at it. It can never be delivered, and never explains why.
    WithdrawnWithoutSuccessor { module: ModuleId },
    /// The route's own 41.05 invariants.
    RouteDefect {
        route: crate::route::RouteId,
        defect: RouteDefect,
    },
    /// The mandatory set of a route cannot fit its declared budget. 41.06: routes "remain valid
    /// under their declared budget or fail explicitly" — this is the explicit part, found before
    /// an agent asks for the bundle.
    RouteBudgetUnsatisfiable {
        route: crate::route::RouteId,
        mandatory_cost: u32,
        budget: u32,
    },
    /// A module with no H1. 41.01: "every node resolves to a file and H1".
    MissingTitle { module: ModuleId },
    /// A module with no path.
    MissingPath { module: ModuleId },
    /// Two modules claiming the same H1: a search for the title cannot resolve.
    DuplicateTitle { title: String, modules: Vec<ModuleId> },
    /// A module declaring a 39.05 protected class whose card does not mention it. 41.04:
    /// "mandatory invariants are not compressed away".
    CardOmitsProtectedInvariant { module: ModuleId },
    /// The card has grown past the 41.04 size band.
    CardExceedsBand {
        module: ModuleId,
        tokens: u32,
        ceiling: u32,
    },
    /// A module declaring profile L3 or L4 with no contract text loaded: the registry promises a
    /// depth the corpus cannot deliver.
    DeclaredProfileUnavailable {
        module: ModuleId,
        declared: ProfileLevel,
        available: ProfileLevel,
    },
    /// A module with no hash. Not an error — hand-declared nodes legitimately have none — but
    /// 41.02 wants hashes on anything read from bytes.
    UnhashedModule { module: ModuleId },
}

impl LintFinding {
    pub fn severity(&self) -> LintSeverity {
        match self {
            LintFinding::DanglingEdge { .. }
            | LintFinding::SelfEdge { .. }
            | LintFinding::CycleInAcyclicEdgeType { .. }
            | LintFinding::ContradictionResolverMissing { .. }
            | LintFinding::RequiresSupersededModule { .. }
            | LintFinding::WithdrawnWithoutSuccessor { .. }
            | LintFinding::RouteDefect { .. }
            | LintFinding::RouteBudgetUnsatisfiable { .. }
            | LintFinding::MissingTitle { .. }
            | LintFinding::MissingPath { .. }
            | LintFinding::CardOmitsProtectedInvariant { .. } => LintSeverity::Error,
            LintFinding::OrphanModule { .. }
            | LintFinding::UnresolvedContradiction { .. }
            | LintFinding::DuplicateTitle { .. }
            | LintFinding::CardExceedsBand { .. }
            | LintFinding::DeclaredProfileUnavailable { .. }
            | LintFinding::UnhashedModule { .. } => LintSeverity::Warning,
        }
    }

    /// Short stable code, for a report a human scans.
    pub fn code(&self) -> &'static str {
        match self {
            LintFinding::DanglingEdge { .. } => "dangling_edge",
            LintFinding::SelfEdge { .. } => "self_edge",
            LintFinding::OrphanModule { .. } => "orphan_module",
            LintFinding::CycleInAcyclicEdgeType { .. } => "cycle_in_acyclic_edge_type",
            LintFinding::UnresolvedContradiction { .. } => "unresolved_contradiction",
            LintFinding::ContradictionResolverMissing { .. } => "contradiction_resolver_missing",
            LintFinding::RequiresSupersededModule { .. } => "requires_superseded_module",
            LintFinding::WithdrawnWithoutSuccessor { .. } => "withdrawn_without_successor",
            LintFinding::RouteDefect { .. } => "route_defect",
            LintFinding::RouteBudgetUnsatisfiable { .. } => "route_budget_unsatisfiable",
            LintFinding::MissingTitle { .. } => "missing_title",
            LintFinding::MissingPath { .. } => "missing_path",
            LintFinding::DuplicateTitle { .. } => "duplicate_title",
            LintFinding::CardOmitsProtectedInvariant { .. } => "card_omits_protected_invariant",
            LintFinding::CardExceedsBand { .. } => "card_exceeds_band",
            LintFinding::DeclaredProfileUnavailable { .. } => "declared_profile_unavailable",
            LintFinding::UnhashedModule { .. } => "unhashed_module",
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LintReport {
    pub findings: Vec<LintFinding>,
}

impl LintReport {
    pub fn has_errors(&self) -> bool {
        self.findings
            .iter()
            .any(|finding| finding.severity() == LintSeverity::Error)
    }

    pub fn errors(&self) -> impl Iterator<Item = &LintFinding> {
        self.findings
            .iter()
            .filter(|finding| finding.severity() == LintSeverity::Error)
    }

    pub fn warnings(&self) -> impl Iterator<Item = &LintFinding> {
        self.findings
            .iter()
            .filter(|finding| finding.severity() == LintSeverity::Warning)
    }

    pub fn with_code<'a>(&'a self, code: &'a str) -> impl Iterator<Item = &'a LintFinding> + 'a {
        self.findings.iter().filter(move |f| f.code() == code)
    }

    /// Counts by code, in code order. The shape a report gets summarised in.
    pub fn counts(&self) -> BTreeMap<&'static str, usize> {
        let mut counts = BTreeMap::new();
        for finding in &self.findings {
            *counts.entry(finding.code()).or_insert(0) += 1;
        }
        counts
    }
}

/// Lint a graph and the routes over it.
///
/// Routes are a parameter rather than a field of [`DocGraph`] because a corpus can serve several
/// route sets — the repository's own routes, a downstream consumer's — and a graph that owned its
/// routes would make "lint this graph against those routes" unexpressible.
pub fn lint(graph: &DocGraph, routes: &[TaskRoute]) -> LintReport {
    let mut findings = Vec::new();

    for edge in graph.edges() {
        if edge.from == edge.to {
            findings.push(LintFinding::SelfEdge {
                module: edge.from.clone(),
                kind: edge.kind,
            });
        }
        for endpoint in [&edge.from, &edge.to] {
            if !graph.contains(endpoint) {
                findings.push(LintFinding::DanglingEdge {
                    from: edge.from.clone(),
                    to: edge.to.clone(),
                    kind: edge.kind,
                    missing: endpoint.clone(),
                });
            }
        }
        if edge.kind == DocEdgeType::Contradicts {
            match &edge.resolved_by {
                None => findings.push(LintFinding::UnresolvedContradiction {
                    a: edge.from.clone(),
                    b: edge.to.clone(),
                }),
                Some(resolver) if !graph.contains(resolver) => {
                    findings.push(LintFinding::ContradictionResolverMissing {
                        a: edge.from.clone(),
                        b: edge.to.clone(),
                        resolver: resolver.clone(),
                    })
                }
                Some(_) => {}
            }
        }
    }

    for edge in graph.edges_of(DocEdgeType::Requires) {
        if let Some(successor) = graph.successors_of(&edge.to).find(|id| *id != &edge.from) {
            findings.push(LintFinding::RequiresSupersededModule {
                requirer: edge.from.clone(),
                superseded: edge.to.clone(),
                successor: successor.clone(),
            });
        }
    }

    for kind in DocEdgeType::ALL.into_iter().filter(|kind| kind.acyclic()) {
        if let Some(cycle) = find_cycle(graph, kind) {
            findings.push(LintFinding::CycleInAcyclicEdgeType { kind, cycle });
        }
    }

    let mut titles: BTreeMap<&str, Vec<ModuleId>> = BTreeMap::new();
    for node in graph.nodes() {
        if node.title.trim().is_empty() {
            findings.push(LintFinding::MissingTitle {
                module: node.id.clone(),
            });
        } else {
            titles.entry(node.title.as_str()).or_default().push(node.id.clone());
        }
        if node.path.trim().is_empty() {
            findings.push(LintFinding::MissingPath {
                module: node.id.clone(),
            });
        }
        if graph.in_edges(&node.id).next().is_none() && node.status.orphan_is_a_defect() {
            findings.push(LintFinding::OrphanModule {
                module: node.id.clone(),
            });
        }
        if node.status == crate::registry::NodeStatus::Withdrawn
            && graph.successors_of(&node.id).next().is_none()
        {
            findings.push(LintFinding::WithdrawnWithoutSuccessor {
                module: node.id.clone(),
            });
        }
        if node.is_non_omittable() && node.card.protected_invariants.is_empty() {
            findings.push(LintFinding::CardOmitsProtectedInvariant {
                module: node.id.clone(),
            });
        }
        let card_tokens = estimate_tokens(&node.card.render(node));
        if card_tokens > CARD_TOKEN_CEILING {
            findings.push(LintFinding::CardExceedsBand {
                module: node.id.clone(),
                tokens: card_tokens,
                ceiling: CARD_TOKEN_CEILING,
            });
        }
        let available = node.best_available_level();
        if node.declared_profile > available {
            findings.push(LintFinding::DeclaredProfileUnavailable {
                module: node.id.clone(),
                declared: node.declared_profile,
                available,
            });
        }
        if node.hash.is_none() {
            findings.push(LintFinding::UnhashedModule {
                module: node.id.clone(),
            });
        }
    }
    for (title, modules) in titles {
        if modules.len() > 1 {
            findings.push(LintFinding::DuplicateTitle {
                title: title.to_string(),
                modules,
            });
        }
    }

    for route in routes {
        for defect in route.check(graph) {
            findings.push(LintFinding::RouteDefect {
                route: route.id.clone(),
                defect,
            });
        }
        if let Some(budget) = route.budget {
            let cost = declared_mandatory_cost(graph, route);
            if cost > budget {
                findings.push(LintFinding::RouteBudgetUnsatisfiable {
                    route: route.id.clone(),
                    mandatory_cost: cost,
                    budget,
                });
            }
        }
    }

    LintReport { findings }
}

/// Cost of the route's *declared* mandatory modules at contract level.
///
/// A lower bound on what [`crate::bundle::compile_bundle`] will compute, since it does not run
/// the companion closure or protected closure. Reported as a lint finding only when even this
/// floor exceeds the budget, so a finding here is never a false positive.
fn declared_mandatory_cost(graph: &DocGraph, route: &TaskRoute) -> u32 {
    let modules: BTreeSet<ModuleId> = route
        .must_read
        .iter()
        .chain(route.non_omittable.iter())
        .cloned()
        .collect();
    modules
        .iter()
        .filter_map(|module| graph.node(module))
        .map(|node| node.cost_at(node.best_available_level()).tokens)
        .fold(0u32, |total, cost| total.saturating_add(cost))
}

/// The lexicographically-first cycle over one edge type, or `None`.
///
/// Iterative depth-first search with an explicit stack: a documentation corpus is shallow but a
/// generated one need not be, and a recursive walk that blows the stack turns a lint finding into
/// a crash.
fn find_cycle(graph: &DocGraph, kind: DocEdgeType) -> Option<Vec<ModuleId>> {
    let mut adjacency: BTreeMap<&ModuleId, Vec<&ModuleId>> = BTreeMap::new();
    for edge in graph.edges_of(kind) {
        if edge.from == edge.to {
            continue;
        }
        adjacency.entry(&edge.from).or_default().push(&edge.to);
    }
    let mut finished: BTreeSet<&ModuleId> = BTreeSet::new();

    for start in adjacency.keys().copied() {
        if finished.contains(start) {
            continue;
        }
        let mut path: Vec<&ModuleId> = Vec::new();
        let mut on_path: BTreeSet<&ModuleId> = BTreeSet::new();
        let mut stack: Vec<(&ModuleId, usize)> = vec![(start, 0)];
        path.push(start);
        on_path.insert(start);

        while let Some((node, index)) = stack.pop() {
            let neighbors = adjacency.get(node).map(Vec::as_slice).unwrap_or(&[]);
            if index < neighbors.len() {
                stack.push((node, index + 1));
                let next = neighbors[index];
                if on_path.contains(next) {
                    let position = path.iter().position(|entry| *entry == next).unwrap_or(0);
                    let mut cycle: Vec<ModuleId> =
                        path[position..].iter().map(|id| (*id).clone()).collect();
                    cycle.push(next.clone());
                    return Some(cycle);
                }
                if !finished.contains(next) {
                    stack.push((next, 0));
                    path.push(next);
                    on_path.insert(next);
                }
            } else {
                finished.insert(node);
                on_path.remove(node);
                if path.last() == Some(&node) {
                    path.pop();
                }
            }
        }
    }
    None
}
