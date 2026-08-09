//! Graph and hypergraph baselines.
//!
//! These are the substrates 43.01 argues against, implemented properly so the argument can be
//! tested rather than asserted. The projection is the undirected incidence graph: factors and
//! variables are nodes, and a factor is adjacent to every variable it consumes or produces. That
//! is the standard factor-graph view and it is deliberately *generous* — it is exactly what a
//! competent graph-retrieval implementation would traverse.
//!
//! The result on the reference world is that both the depth-limited and the unbounded walk reach
//! all 761 facts, because 750 distractor factors consume the same protected `cohort_id` hub.
//! Hub expansion is a property of the substrate, not a bug in the baseline.

use crate::strategy::{ContextStrategy, Selection};
use bioprism_fiber::Query;
use bioprism_world::World;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum Node {
    Variable(String),
    Factor(String),
    Fact(String),
}

fn incidence(world: &World) -> BTreeMap<Node, BTreeSet<Node>> {
    let mut adjacency: BTreeMap<Node, BTreeSet<Node>> = BTreeMap::new();
    let mut link = |a: Node, b: Node| {
        adjacency.entry(a.clone()).or_default().insert(b.clone());
        adjacency.entry(b).or_default().insert(a);
    };

    for factor in &world.factors {
        let factor_node = Node::Factor(factor.id.as_str().to_string());
        for variable in factor.inputs.iter().chain(factor.outputs.iter()) {
            link(
                factor_node.clone(),
                Node::Variable(variable.as_str().to_string()),
            );
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

fn reachable(world: &World, query: &Query, max_depth: Option<usize>) -> BTreeSet<String> {
    let adjacency = incidence(world);
    let mut seen: BTreeSet<Node> = query
        .targets
        .iter()
        .map(|target| Node::Variable(target.as_str().to_string()))
        .collect();
    let mut frontier: VecDeque<(Node, usize)> =
        seen.iter().cloned().map(|node| (node, 0)).collect();

    while let Some((node, depth)) = frontier.pop_front() {
        if max_depth.is_some_and(|limit| depth >= limit) {
            continue;
        }
        if let Some(neighbours) = adjacency.get(&node) {
            for neighbour in neighbours {
                if seen.insert(neighbour.clone()) {
                    frontier.push_back((neighbour.clone(), depth + 1));
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

/// Depth-limited neighbourhood traversal, the classic graph-RAG shape.
pub struct KHopIncidence {
    pub depth: usize,
}

impl ContextStrategy for KHopIncidence {
    fn name(&self) -> String {
        format!("graph-{}-hop", self.depth)
    }

    fn method(&self) -> String {
        format!(
            "breadth-first walk of the undirected factor/variable incidence graph from the query \
             targets, to depth {}",
            self.depth
        )
    }

    fn select(&self, world: &World, query: &Query) -> Selection {
        Selection::new(reachable(world, query, Some(self.depth)))
            .noting("undirected incidence projection; edges carry no direction, so hubs expand")
    }
}

/// The whole connected component: what a hypergraph-component retrieval returns.
pub struct ConnectedComponent;

impl ContextStrategy for ConnectedComponent {
    fn name(&self) -> String {
        "hypergraph-component".into()
    }

    fn method(&self) -> String {
        "unbounded breadth-first walk of the incidence graph from the query targets".into()
    }

    fn select(&self, world: &World, query: &Query) -> Selection {
        Selection::new(reachable(world, query, None))
            .noting("no depth limit; returns the entire connected component")
    }
}

/// Facts touching any factor that also touches a target variable.
///
/// A tighter, query-aware graph baseline: rather than walking outward it takes only the factors
/// incident to a target and the facts feeding them. This is the strongest graph competitor here.
pub struct QueryGraph;

impl ContextStrategy for QueryGraph {
    fn name(&self) -> String {
        "query-graph".into()
    }

    fn method(&self) -> String {
        "facts feeding any factor incident to a query target variable".into()
    }

    fn select(&self, world: &World, query: &Query) -> Selection {
        let targets: BTreeSet<&str> = query.targets.iter().map(|t| t.as_str()).collect();
        let mut variables: BTreeSet<&str> = targets.clone();

        for factor in &world.factors {
            let incident = factor
                .inputs
                .iter()
                .chain(factor.outputs.iter())
                .any(|v| targets.contains(v.as_str()));
            if incident {
                for variable in factor.inputs.iter().chain(factor.outputs.iter()) {
                    variables.insert(variable.as_str());
                }
            }
        }

        Selection::new(
            world
                .facts
                .iter()
                .filter(|fact| variables.contains(fact.provides.as_str()))
                .map(|fact| fact.id.as_str().to_string())
                .collect(),
        )
        .noting("one factor hop, undirected, restricted to factors touching a target")
    }
}
