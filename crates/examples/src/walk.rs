//! The neighbourhood-walk probe.
//!
//! Blueprint 43.38 requires matched, equal-engineering comparison and 43.41 requires that "if
//! graph baselines remain compact under equal optimization, report that result". A slice that
//! claims a structural advantage for compilation has to run the competitor, on the same world,
//! with the same query, at every depth — not at the one depth that flatters the compiler.
//!
//! This is a small re-implementation of the undirected incidence walk rather than a call into
//! `bioprism-baseline`. That is deliberate: a reference example must not depend on the comparison
//! harness it is supposed to make checkable, or the two drift together and neither catches the
//! other. The projection is the standard factor-graph view — factors and variables are nodes, a
//! factor is adjacent to every variable it consumes or produces, and a fact is adjacent to the
//! variable it provides — which is exactly what a competent graph-retrieval implementation would
//! traverse. Nothing here is weakened to make the walk lose.
//!
//! What the probe does *not* implement is a vector or embedding retriever: no embedding model is
//! available offline, so the lexical and dense baselines of 43.41's comparison table are outside
//! this crate. That gap is reported, not silently skipped.

use crate::report::{DepthObservation, GraphWalkObservation};
use bioprism_fiber::{oracle, Query};
use bioprism_section::OracleVerdict;
use bioprism_world::{World, WorldSource};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum Node {
    Variable(String),
    Factor(String),
    Fact(String),
}

type Incidence = BTreeMap<Node, BTreeSet<Node>>;

fn incidence(world: &World) -> Incidence {
    let mut adjacency: Incidence = BTreeMap::new();
    let mut link = |a: Node, b: Node| {
        adjacency.entry(a.clone()).or_default().insert(b.clone());
        adjacency.entry(b).or_default().insert(a);
    };

    for factor in &world.factors {
        let node = Node::Factor(factor.id.as_str().to_string());
        for variable in factor.inputs.iter().chain(factor.outputs.iter()) {
            link(node.clone(), Node::Variable(variable.as_str().to_string()));
        }
    }
    for fact in &world.facts {
        link(
            Node::Fact(fact.id.as_str().to_string()),
            Node::Variable(fact.provides.as_str().to_string()),
        );
    }
    adjacency
}

fn reachable_facts(adjacency: &Incidence, query: &Query, depth: usize) -> BTreeSet<String> {
    let mut seen: BTreeSet<Node> = query
        .targets
        .iter()
        .map(|target| Node::Variable(target.as_str().to_string()))
        .collect();
    let mut frontier: VecDeque<(Node, usize)> = seen.iter().cloned().map(|n| (n, 0)).collect();

    while let Some((node, at)) = frontier.pop_front() {
        if at >= depth {
            continue;
        }
        if let Some(neighbours) = adjacency.get(&node) {
            for neighbour in neighbours {
                if seen.insert(neighbour.clone()) {
                    frontier.push_back((neighbour.clone(), at + 1));
                }
            }
        }
    }

    seen.into_iter()
        .filter_map(|node| match node {
            Node::Fact(id) => Some(id),
            _ => None,
        })
        .collect()
}

/// The exact fact ids a depth-limited walk reaches.
///
/// Exposed separately from [`sweep`] because "the walk selects eleven facts" and "the walk selects
/// *the same* eleven facts" are different claims, and only the second one settles whether the
/// compiler bought anything on a given world.
pub fn facts_at(world: &World, query: &Query, depth: usize) -> BTreeSet<String> {
    reachable_facts(&incidence(world), query, depth)
}

/// Runs the deterministic oracle over exactly the facts a strategy selected.
///
/// This is what makes "sound" mean something. A strategy is not judged on how many facts it kept
/// but on whether the same oracle, given only its selection, reaches the same verdict with the
/// same witnesses as it does on the whole world.
fn verdict_over(world: &World, selection: &BTreeSet<String>) -> Option<OracleVerdict> {
    oracle::evaluate_facts(
        world
            .facts
            .iter()
            .filter(|fact| selection.contains(fact.id.as_str())),
    )
    .ok()
}

/// Sweeps a depth-limited incidence walk from depth 1 to `max_depth`.
///
/// A depth is *usable* when it is sound, retains the whole protected closure, and selects fewer
/// than half the world's facts. All three conditions are required by the architecture, and each
/// one alone is satisfiable by a strategy nobody would ship: full context is sound and closed but
/// not compact, an empty selection is compact but neither, and a lexical retriever can be sound
/// and compact while quietly dropping protected evidence.
pub fn sweep(world: &World, query: &Query, max_depth: usize) -> GraphWalkObservation {
    let adjacency = incidence(world);
    let all_facts: BTreeSet<String> = world
        .facts
        .iter()
        .map(|fact| fact.id.as_str().to_string())
        .collect();
    let reference = verdict_over(world, &all_facts);
    let protected = world.fact_ids_with_any_tag(&query.protected_tags);
    let compact_below = world.facts.len() / 2;

    let mut depths = Vec::new();
    let mut usable_depths = Vec::new();

    for depth in 1..=max_depth {
        let selection = reachable_facts(&adjacency, query, depth);
        let verdict_preserving = match (&reference, verdict_over(world, &selection)) {
            (Some(expected), Some(actual)) => &actual == expected,
            _ => false,
        };
        let protected_recall = if protected.is_empty() {
            1.0
        } else {
            selection.intersection(&protected).count() as f64 / protected.len() as f64
        };
        let usable =
            verdict_preserving && protected_recall == 1.0 && selection.len() < compact_below;
        if usable {
            usable_depths.push(depth);
        }
        depths.push(DepthObservation {
            depth,
            facts_selected: selection.len(),
            verdict_preserving,
            protected_recall,
            usable,
        });
    }

    GraphWalkObservation {
        max_depth,
        depths,
        usable_depths,
    }
}
