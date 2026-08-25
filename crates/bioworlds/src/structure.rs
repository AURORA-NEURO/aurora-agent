//! Structural characterisation: the measurements that predict whether a world discriminates.
//!
//! Blueprint 43.39 asks for worlds that "vary mathematical structure independently"; `docs/FINDINGS.md`
//! records what happens when they do not. On the shipped reference world a 5-hop neighbourhood walk
//! and a BM25 retriever select the *identical eleven facts* FIBER selects, so that benchmark
//! measures nothing about the method. Two structural properties explain it: a separating depth
//! exists, and distractor tags name themselves as distractors.
//!
//! This module computes both, plus the protected/unprotected split, the temporal-withholding
//! split, and an elimination-width upper bound on treewidth. Every world this crate ships is
//! reported through it, including the one that comes out looking like the reference world.
//!
//! # What these numbers are, exactly
//!
//! * **Neighbourhood distance** is measured on the tripartite incidence graph — variables,
//!   factors and facts as nodes, `factor—variable` for each input and output and `fact—variable`
//!   for `provides` — starting from the target *variable*, counting one edge as one hop. That
//!   convention is not arbitrary: it reproduces the published depths in `docs/FINDINGS.md`
//!   exactly (4 hops selects nothing, 5 and 6 select the eleven decisive facts, 7 selects all
//!   761), and `tests/reference_world_calibration.rs` asserts it against the shipped fixture. It
//!   is nonetheless *this crate's* metric, not a claim about how `bioprism-baseline` tunes its
//!   walk.
//! * **Separating depth** is the smallest radius whose fact ball contains every decisive fact and
//!   no distractor. `None` means no radius does, which is the structural precondition for
//!   `structural_discrimination_no_usable_depth`. It is a statement about neighbourhood traversal
//!   only. It is *not* evidence about BM25, and it is *not* evidence that a compiled verdict
//!   differs — nothing in this crate can compile one.
//! * **Elimination width** is a greedy min-degree elimination ordering's induced width. It is an
//!   upper bound on treewidth, never treewidth, and the field is named for what it is.
//!
//! # Not implemented
//!
//! Exact treewidth; a lexical-retrieval simulation (camouflage is reported as a token-overlap
//! fraction, which is a proxy for a BM25 shortcut, not a run of one); and any verdict-level
//! comparison, which would need the compiler this crate deliberately does not depend on.

use crate::builder::BioWorld;
use crate::error::BioWorldError;
use crate::query::QueryShape;
use bioprism_world::World;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

/// A node of the incidence graph neighbourhood distance is measured on.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum Node {
    Variable(String),
    Factor(String),
    Fact(String),
}

/// Hop distances from the target over the tripartite incidence graph.
#[derive(Debug, Clone)]
pub struct Neighbourhood {
    distances: BTreeMap<Node, usize>,
}

impl Neighbourhood {
    /// BFS from the target variable. A target no factor produces still yields a graph — the
    /// caller finds out via [`Neighbourhood::fact_distance`] returning `None` everywhere.
    pub fn from_target(world: &World, target: &str) -> Self {
        let mut adjacency: BTreeMap<Node, BTreeSet<Node>> = BTreeMap::new();
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

        let start = Node::Variable(target.to_string());
        let mut distances = BTreeMap::new();
        let mut queue = VecDeque::new();
        distances.insert(start.clone(), 0usize);
        queue.push_back(start);

        while let Some(current) = queue.pop_front() {
            let depth = distances[&current];
            if let Some(neighbours) = adjacency.get(&current) {
                for neighbour in neighbours {
                    if !distances.contains_key(neighbour) {
                        distances.insert(neighbour.clone(), depth + 1);
                        queue.push_back(neighbour.clone());
                    }
                }
            }
        }

        Neighbourhood { distances }
    }

    pub fn fact_distance(&self, fact_id: &str) -> Option<usize> {
        self.distances
            .get(&Node::Fact(fact_id.to_string()))
            .copied()
    }

    /// Fact ids within `radius` hops, i.e. what a neighbourhood walk of that depth would admit.
    pub fn facts_within(&self, radius: usize) -> BTreeSet<String> {
        self.distances
            .iter()
            .filter_map(|(node, depth)| match node {
                Node::Fact(id) if *depth <= radius => Some(id.clone()),
                _ => None,
            })
            .collect()
    }

    pub fn max_distance(&self) -> usize {
        self.distances.values().copied().max().unwrap_or(0)
    }
}

/// The directed backward closure: what the target actually depends on.
///
/// Distinct from neighbourhood distance, and deliberately so. A neighbourhood walk is undirected
/// — that is why hub attachment matters at all — while dependency is directed. A variable can sit
/// two hops from the target and contribute nothing to it. `variables` here is the set the target
/// genuinely depends on, and it is the set the withholding claim is checked against.
#[derive(Debug, Clone)]
pub struct DependencyClosure {
    pub variables: BTreeSet<String>,
    pub factors: BTreeSet<String>,
}

impl DependencyClosure {
    pub fn of_target(world: &World, target: &str) -> Self {
        let mut variables = BTreeSet::new();
        let mut factors = BTreeSet::new();
        let mut frontier = vec![target.to_string()];
        variables.insert(target.to_string());

        while let Some(variable) = frontier.pop() {
            for factor in world.producers_of(&variable) {
                if !factors.insert(factor.id.as_str().to_string()) {
                    continue;
                }
                for input in &factor.inputs {
                    if variables.insert(input.as_str().to_string()) {
                        frontier.push(input.as_str().to_string());
                    }
                }
            }
        }

        DependencyClosure { variables, factors }
    }

    pub fn depends_on(&self, variable: &str) -> bool {
        self.variables.contains(variable)
    }
}

/// How a world splits under a query's protected vocabulary and temporal cut.
///
/// The pair of fields that matters for §38.08 is [`TemporalSplit::withheld_and_not_protected`]
/// against [`TemporalSplit::withheld_and_protected`]. In the generated family the first is empty
/// and the second is everything, which is precisely why an early cut there cannot withhold
/// evidence without also breaking the closure.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TemporalSplit {
    pub decision_time: String,
    pub event_managed: Vec<String>,
    pub event_managed_and_protected: Vec<String>,
    pub event_managed_and_not_protected: Vec<String>,
    /// Event-managed variables whose every governing event is released after the cut.
    pub withheld: Vec<String>,
    pub withheld_and_protected: Vec<String>,
    pub withheld_and_not_protected: Vec<String>,
    /// The subset of `withheld_and_not_protected` the target actually depends on. This is the set
    /// that makes temporal withholding a separate failure from a closure violation.
    pub withheld_not_protected_and_decisive: Vec<String>,
}

impl TemporalSplit {
    /// Whether every protected fact is still readable at the cut.
    ///
    /// A cut that withholds protected evidence conflates two failures; §38.08's firewall is only
    /// interesting when the closure survives it.
    pub fn protected_closure_survives_the_cut(&self) -> bool {
        self.withheld_and_protected.is_empty()
    }
}

/// Everything this crate can say about a world's structure without compiling anything.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StructuralProfile {
    pub world_id: String,
    pub target: String,
    pub facts: usize,
    pub factors: usize,
    pub events: usize,
    pub protected_facts: usize,
    pub unprotected_facts: usize,
    /// Facts whose provided variable is in the target's directed backward closure.
    pub decisive_facts: usize,
    pub decisive_variables: usize,
    /// Facts carrying the world's declared distractor tag.
    pub distractor_facts: usize,
    pub min_hops_to_a_decisive_fact: Option<usize>,
    pub max_hops_to_a_decisive_fact: Option<usize>,
    pub min_hops_to_a_distractor_fact: Option<usize>,
    /// Smallest radius admitting every decisive fact and no distractor; `None` if none does.
    pub separating_depth: Option<usize>,
    /// Greedy min-degree induced width over the whole world. An upper bound on treewidth.
    pub elimination_width: usize,
    /// The same bound restricted to the target's dependency closure.
    pub decisive_elimination_width: usize,
    pub max_factor_arity: usize,
    /// Fraction of distractor facts carrying a non-protected tag that shares a token with the
    /// protected vocabulary. A proxy for how far a lexical shortcut is denied, not a BM25 run.
    pub tag_camouflage_fraction: f64,
    pub temporal: TemporalSplit,
}

impl StructuralProfile {
    /// Whether a neighbourhood walk has a depth that is both sound (admits every decisive fact)
    /// and compact (admits no distractor).
    pub fn a_separating_depth_exists(&self) -> bool {
        self.separating_depth.is_some()
    }
}

/// Measures a world against a query shape.
///
/// `distractor_tag` is the tag whose facts the world declares irrelevant. It is a parameter rather
/// than a constant because a world is free to name its own; passing a tag no fact carries yields
/// `distractor_facts: 0` and a separating depth of the decisive radius, which is the honest answer
/// for a world with no distractors rather than an error.
pub fn profile(
    world: &BioWorld,
    query: &QueryShape,
    distractor_tag: &str,
) -> Result<StructuralProfile, BioWorldError> {
    let inner = world.world();
    let target = query
        .target()
        .ok_or_else(|| BioWorldError::UnknownTarget {
            slice: query.query_id.clone(),
            target: format!("{:?}", query.targets),
            world_id: world.id().to_string(),
        })?
        .to_string();

    if inner.producers_of(&target).next().is_none() {
        return Err(BioWorldError::UnknownTarget {
            slice: query.query_id.clone(),
            target: target.clone(),
            world_id: world.id().to_string(),
        });
    }

    let closure = DependencyClosure::of_target(inner, &target);
    let neighbourhood = Neighbourhood::from_target(inner, &target);

    let protected_facts: Vec<&str> = inner
        .facts
        .iter()
        .filter(|fact| fact.has_any_tag(&query.protected_tags))
        .map(|fact| fact.id.as_str())
        .collect();
    let protected_variables: BTreeSet<String> = inner
        .facts
        .iter()
        .filter(|fact| fact.has_any_tag(&query.protected_tags))
        .map(|fact| fact.provides.as_str().to_string())
        .collect();

    let decisive_fact_ids: BTreeSet<String> = inner
        .facts
        .iter()
        .filter(|fact| closure.depends_on(fact.provides.as_str()))
        .map(|fact| fact.id.as_str().to_string())
        .collect();
    let distractor_fact_ids: BTreeSet<String> = inner
        .facts
        .iter()
        .filter(|fact| fact.has_tag(distractor_tag))
        .map(|fact| fact.id.as_str().to_string())
        .collect();

    let hops = |ids: &BTreeSet<String>| -> (Option<usize>, Option<usize>) {
        let all: Vec<usize> = ids
            .iter()
            .filter_map(|id| neighbourhood.fact_distance(id))
            .collect();
        (all.iter().copied().min(), all.iter().copied().max())
    };
    let (min_decisive, max_decisive) = hops(&decisive_fact_ids);
    let (min_distractor, _) = hops(&distractor_fact_ids);

    let separating_depth = (0..=neighbourhood.max_distance()).find(|radius| {
        let ball = neighbourhood.facts_within(*radius);
        decisive_fact_ids.is_subset(&ball) && ball.is_disjoint(&distractor_fact_ids)
    });

    let protected_tokens = query.protected_tokens();
    let camouflaged = inner
        .facts
        .iter()
        .filter(|fact| distractor_fact_ids.contains(fact.id.as_str()))
        .filter(|fact| {
            fact.tags.iter().any(|tag| {
                !query.protects(tag) && tag.split('_').any(|token| protected_tokens.contains(token))
            })
        })
        .count();
    let tag_camouflage_fraction = if distractor_fact_ids.is_empty() {
        0.0
    } else {
        camouflaged as f64 / distractor_fact_ids.len() as f64
    };

    Ok(StructuralProfile {
        world_id: world.id().to_string(),
        facts: inner.facts.len(),
        factors: inner.factors.len(),
        events: inner.events.len(),
        protected_facts: protected_facts.len(),
        unprotected_facts: inner.facts.len() - protected_facts.len(),
        decisive_facts: decisive_fact_ids.len(),
        decisive_variables: closure.variables.len(),
        distractor_facts: distractor_fact_ids.len(),
        min_hops_to_a_decisive_fact: min_decisive,
        max_hops_to_a_decisive_fact: max_decisive,
        min_hops_to_a_distractor_fact: min_distractor,
        separating_depth,
        elimination_width: elimination_width(inner, None),
        decisive_elimination_width: elimination_width(inner, Some(&closure.variables)),
        max_factor_arity: inner.factors.iter().map(|f| f.arity()).max().unwrap_or(0),
        tag_camouflage_fraction,
        temporal: temporal_split(inner, query, &closure, &protected_variables)?,
        target,
    })
}

fn temporal_split(
    world: &World,
    query: &QueryShape,
    closure: &DependencyClosure,
    protected_variables: &BTreeSet<String>,
) -> Result<TemporalSplit, BioWorldError> {
    let cut = query.cut()?;

    let mut released: BTreeSet<String> = BTreeSet::new();
    let mut managed: BTreeSet<String> = BTreeSet::new();
    for event in &world.events {
        for variable in &event.produces {
            managed.insert(variable.as_str().to_string());
            if event.is_available_at(cut) {
                released.insert(variable.as_str().to_string());
            }
        }
    }

    let withheld: Vec<String> = managed.difference(&released).cloned().collect();
    let sorted = |set: Vec<String>| -> Vec<String> {
        let mut out = set;
        out.sort();
        out.dedup();
        out
    };

    Ok(TemporalSplit {
        decision_time: query.decision_time.clone(),
        event_managed: sorted(managed.iter().cloned().collect()),
        event_managed_and_protected: sorted(
            managed
                .iter()
                .filter(|v| protected_variables.contains(*v))
                .cloned()
                .collect(),
        ),
        event_managed_and_not_protected: sorted(
            managed
                .iter()
                .filter(|v| !protected_variables.contains(*v))
                .cloned()
                .collect(),
        ),
        withheld_and_protected: sorted(
            withheld
                .iter()
                .filter(|v| protected_variables.contains(*v))
                .cloned()
                .collect(),
        ),
        withheld_and_not_protected: sorted(
            withheld
                .iter()
                .filter(|v| !protected_variables.contains(*v))
                .cloned()
                .collect(),
        ),
        withheld_not_protected_and_decisive: sorted(
            withheld
                .iter()
                .filter(|v| !protected_variables.contains(*v) && closure.depends_on(v))
                .cloned()
                .collect(),
        ),
        withheld: sorted(withheld),
    })
}

/// Greedy min-degree induced width over the moral graph of the factor hypergraph.
///
/// Each factor contributes a clique over its inputs and outputs; vertices are then eliminated
/// lowest-degree first, ties broken by name so the result is deterministic. The maximum degree at
/// elimination is an upper bound on treewidth — a *bound*, which is why the field is not called
/// `treewidth`. Restricting to `only` measures the decisive core without the distractor fringe.
fn elimination_width(world: &World, only: Option<&BTreeSet<String>>) -> usize {
    let admits = |variable: &str| only.is_none_or(|set| set.contains(variable));

    let mut adjacency: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for factor in &world.factors {
        let clique: Vec<String> = factor
            .inputs
            .iter()
            .chain(factor.outputs.iter())
            .map(|v| v.as_str().to_string())
            .filter(|v| admits(v))
            .collect();
        for left in &clique {
            adjacency.entry(left.clone()).or_default();
            for right in &clique {
                if left != right {
                    adjacency
                        .entry(left.clone())
                        .or_default()
                        .insert(right.clone());
                }
            }
        }
    }

    let mut width = 0usize;
    while let Some(chosen) = adjacency
        .iter()
        .min_by_key(|(name, neighbours)| (neighbours.len(), (*name).clone()))
        .map(|(name, _)| name.clone())
    {
        let neighbours = adjacency.remove(&chosen).unwrap_or_default();
        width = width.max(neighbours.len());
        for left in &neighbours {
            if let Some(entry) = adjacency.get_mut(left) {
                entry.remove(&chosen);
                for right in &neighbours {
                    if left != right {
                        entry.insert(right.clone());
                    }
                }
            }
        }
    }
    width
}
