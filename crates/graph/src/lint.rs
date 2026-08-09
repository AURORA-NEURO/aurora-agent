//! Graph linting.
//!
//! Blueprint 41.11 asks for validation of "IDs, paths, edges, route closure, hashes, front matter,
//! links, examples, and graph-to-file consistency". The half of that which applies to a *generated*
//! view is structural: an edge must not point at a node the view does not contain, a `requires`
//! chain must not close into a cycle (41.03's third invariant), and a non-normative edge must not
//! have slipped in.
//!
//! Linting is separate from projecting on purpose. A lint finding describes the compiled region,
//! not the projection — a `requires` cycle means the compiler emitted a circular dependency, and
//! refusing to render it would hide the defect from the person best placed to see it. So the
//! projection renders, and the lint reports.

use crate::graph::GraphBody;
use crate::vocabulary::EdgeType;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// A structural defect in a projected graph.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "lint", rename_all = "snake_case")]
pub enum GraphLint {
    /// An edge endpoint names a node the view does not contain, so a reader cannot follow it.
    DanglingEndpoint {
        from: String,
        edge: EdgeType,
        to: String,
        missing: String,
    },
    /// 41.03: "requires edges are acyclic at the release-contract layer".
    RequiresCycle { members: Vec<String> },
    /// A node no edge touches. Not always wrong — a delivered fact that no selected factor
    /// consumes is legitimate, and 43.01 forbids dropping it for that reason — but worth naming.
    IsolatedNode { id: String },
    /// An edge the vocabulary itself marks non-normative. This crate never emits one; the lint
    /// exists for views assembled elsewhere and handed here for checking.
    NonNormativeEdge {
        from: String,
        edge: EdgeType,
        to: String,
    },
}

/// Runs every structural check over a projected graph.
pub fn lint_graph(body: &GraphBody) -> Vec<GraphLint> {
    let ids: BTreeSet<&str> = body.nodes.iter().map(|node| node.id.as_str()).collect();
    let mut findings: Vec<GraphLint> = Vec::new();

    let mut touched: BTreeSet<&str> = BTreeSet::new();
    for edge in &body.edges {
        touched.insert(edge.from.as_str());
        touched.insert(edge.to.as_str());

        for endpoint in [&edge.from, &edge.to] {
            if !ids.contains(endpoint.as_str()) {
                findings.push(GraphLint::DanglingEndpoint {
                    from: edge.from.clone(),
                    edge: edge.edge,
                    to: edge.to.clone(),
                    missing: endpoint.clone(),
                });
            }
        }

        if !edge.edge.is_normative() {
            findings.push(GraphLint::NonNormativeEdge {
                from: edge.from.clone(),
                edge: edge.edge,
                to: edge.to.clone(),
            });
        }
    }

    for node in &body.nodes {
        if !touched.contains(node.id.as_str()) {
            findings.push(GraphLint::IsolatedNode {
                id: node.id.clone(),
            });
        }
    }

    findings.extend(requires_cycles(body).into_iter().map(|members| {
        GraphLint::RequiresCycle { members }
    }));

    findings
}

/// Cycles reachable through `requires` edges only.
///
/// Same fixpoint technique the timeline uses for causal ancestry: grow each node's reachable set
/// until it stops changing, then a node contained in its own set sits on a cycle. Cheaper to
/// justify than a Tarjan implementation, and the sets are small because a compiled region is small.
fn requires_cycles(body: &GraphBody) -> Vec<Vec<String>> {
    let mut reachable: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for edge in body.edges_of(EdgeType::Requires) {
        reachable
            .entry(edge.from.as_str())
            .or_default()
            .insert(edge.to.as_str());
    }

    let sources: Vec<&str> = reachable.keys().copied().collect();
    let mut changed = true;
    while changed {
        changed = false;
        for source in &sources {
            let current = reachable.get(source).cloned().unwrap_or_default();
            let mut grown = current.clone();
            for step in &current {
                if let Some(onward) = reachable.get(step) {
                    grown.extend(onward.iter().copied());
                }
            }
            if grown.len() != current.len() {
                reachable.insert(source, grown);
                changed = true;
            }
        }
    }

    let mut members: Vec<String> = reachable
        .iter()
        .filter(|(source, set)| set.contains(*source))
        .map(|(source, _)| (*source).to_string())
        .collect();
    members.sort();

    if members.is_empty() {
        Vec::new()
    } else {
        vec![members]
    }
}
