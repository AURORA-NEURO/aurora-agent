//! The traversal policy (41.07), and the completeness question underneath it.
//!
//! 41.07's objects are query, seed nodes, edge filters, depth, budget, protected closure and an
//! omission record; its invariants are that "no traversal crosses denied policy labels", "cycles
//! are bounded", and "omitted required nodes invalidate the bundle".
//!
//! # Why the walk is undirected
//!
//! Reachability here ignores edge direction. That looks sloppy and is the opposite: this walk
//! exists to answer one question — *is there any relation at all between the task and this
//! module?* — and the answer is only worth something if it is the most generous notion of
//! relation available. A node that is unreachable from the seeds even when every edge is walked
//! both ways, at unbounded depth, over every edge type, is a node with no path to the task. That
//! is the only condition under which this crate is willing to write [`InfluenceClass::Zero`]
//! against an omission. A directed walk would report far more nodes as unreachable, and every one
//! of those extra reports would be an unearned zero-influence claim.
//!
//! Directed reasoning does exist in this crate; it lives where direction means something. The
//! mandatory closure in [`crate::bundle`] follows [`CompanionRequirement`] in the direction the
//! edge type specifies, and [`crate::impact`] walks against the edges.
//!
//! # Completeness is a property of the walk, not of the graph
//!
//! [`Traversal::completeness`] returns [`Completeness::Exhaustive`] only when nothing narrowed
//! the walk: no depth cap was hit, no policy label blocked a node, no edge type present in the
//! graph was filtered out, and no edge pointed at a module the registry does not hold. Any one of
//! those and the result is [`Completeness::Partial`] with the reasons attached — and every
//! unreached node downgrades from "provably cannot matter" to "not examined". This is the whole
//! honesty mechanism of the crate in one function: the *walk* has to earn the zero, the graph
//! cannot grant it.
//!
//! [`InfluenceClass::Zero`]: bioprism_section::InfluenceClass::Zero
//! [`CompanionRequirement`]: crate::vocabulary::CompanionRequirement

use crate::registry::{DocGraph, ModuleId};
use crate::vocabulary::{DocEdge, DocEdgeType};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

/// Which edges a walk follows, how far, and what it refuses to cross.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TraversalPolicy {
    pub follow: BTreeSet<DocEdgeType>,
    /// `None` is unbounded. Cycles are still bounded, by the visited set — 41.07's "cycles are
    /// bounded" is satisfied structurally rather than by a depth cap, so an unbounded walk still
    /// terminates.
    pub max_depth: Option<u32>,
    /// A node carrying any of these labels is not entered and not expanded through.
    pub denied_labels: BTreeSet<String>,
}

impl TraversalPolicy {
    /// Every edge type, no depth cap, nothing denied. The only policy under which an unreached
    /// node can be called zero-influence.
    pub fn exhaustive() -> Self {
        TraversalPolicy {
            follow: DocEdgeType::ALL.into_iter().collect(),
            max_depth: None,
            denied_labels: BTreeSet::new(),
        }
    }

    /// Follows only edges that create a companion obligation, plus symmetric ones. The default
    /// for bundle expansion: it reaches everything the bundle is *obliged* to contain without
    /// dragging in the corpus's citation web.
    pub fn normative() -> Self {
        TraversalPolicy {
            follow: DocEdgeType::ALL
                .into_iter()
                .filter(|kind| kind.followed_by_default())
                .collect(),
            max_depth: None,
            denied_labels: BTreeSet::new(),
        }
    }

    pub fn with_max_depth(mut self, depth: u32) -> Self {
        self.max_depth = Some(depth);
        self
    }

    pub fn denying(mut self, label: impl Into<String>) -> Self {
        self.denied_labels.insert(label.into());
        self
    }

    pub fn following(mut self, kind: DocEdgeType) -> Self {
        self.follow.insert(kind);
        self
    }

    pub fn not_following(mut self, kind: DocEdgeType) -> Self {
        self.follow.remove(&kind);
        self
    }

    pub fn follows(&self, kind: DocEdgeType) -> bool {
        self.follow.contains(&kind)
    }
}

/// A reason a walk did not see the whole graph.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "reason")]
pub enum PartialReason {
    /// The depth cap stopped expansion at these nodes; anything past them was never looked at.
    DepthLimited { at: Vec<ModuleId>, depth: u32 },
    /// These edge types exist in the graph and were not followed.
    EdgeTypesFiltered { kinds: Vec<DocEdgeType> },
    /// 41.07's denied labels. The nodes behind them were not examined.
    PolicyDenied { modules: Vec<ModuleId> },
    /// An edge named a module the registry does not hold, so the walk cannot know what lay beyond.
    DanglingEdge { from: ModuleId, to: ModuleId },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "completeness")]
pub enum Completeness {
    /// Nothing narrowed the walk. Unreached nodes have no path to the seeds.
    Exhaustive,
    Partial { reasons: Vec<PartialReason> },
}

impl Completeness {
    /// Whether an unreached node may be classified zero-influence on the strength of this walk.
    pub fn licenses_zero_influence(&self) -> bool {
        matches!(self, Completeness::Exhaustive)
    }
}

/// The result of one walk.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Traversal {
    pub seeds: Vec<ModuleId>,
    /// Reached module → depth at which it was first reached.
    pub reached: BTreeMap<ModuleId, u32>,
    pub blocked_by_policy: BTreeMap<ModuleId, String>,
    /// Reached, but its own edges were never expanded because the depth cap was hit here.
    pub frontier: BTreeSet<ModuleId>,
    pub dangling: Vec<DocEdge>,
    /// Edge types present in the graph that the policy did not follow. Types absent from the
    /// graph are not listed: not following an edge type nobody used narrows nothing.
    pub filtered_types: BTreeSet<DocEdgeType>,
}

impl Traversal {

    pub fn was_reached(&self, id: &ModuleId) -> bool {
        self.reached.contains_key(id)
    }

    pub fn completeness(&self) -> Completeness {
        let mut reasons = Vec::new();
        if !self.frontier.is_empty() {
            let depth = self
                .frontier
                .iter()
                .filter_map(|id| self.reached.get(id).copied())
                .max()
                .unwrap_or(0);
            reasons.push(PartialReason::DepthLimited {
                at: self.frontier.iter().cloned().collect(),
                depth,
            });
        }
        if !self.filtered_types.is_empty() {
            reasons.push(PartialReason::EdgeTypesFiltered {
                kinds: self.filtered_types.iter().copied().collect(),
            });
        }
        if !self.blocked_by_policy.is_empty() {
            reasons.push(PartialReason::PolicyDenied {
                modules: self.blocked_by_policy.keys().cloned().collect(),
            });
        }
        for edge in &self.dangling {
            reasons.push(PartialReason::DanglingEdge {
                from: edge.from.clone(),
                to: edge.to.clone(),
            });
        }
        if reasons.is_empty() {
            Completeness::Exhaustive
        } else {
            Completeness::Partial { reasons }
        }
    }
}

/// Walk the graph from `seeds` under `policy`.
///
/// Breadth-first with a sorted frontier, so the depth recorded for each node is the shortest one
/// and the output does not depend on insertion order. Seeds that are not in the registry are
/// skipped and recorded as dangling, which is what makes a route naming a deleted module void the
/// zero-influence licence rather than silently produce a smaller answer.
pub fn traverse(graph: &DocGraph, seeds: &[ModuleId], policy: &TraversalPolicy) -> Traversal {
    let mut reached: BTreeMap<ModuleId, u32> = BTreeMap::new();
    let mut blocked: BTreeMap<ModuleId, String> = BTreeMap::new();
    let mut frontier: BTreeSet<ModuleId> = BTreeSet::new();
    let mut dangling: Vec<DocEdge> = Vec::new();
    let mut queue: VecDeque<(ModuleId, u32)> = VecDeque::new();

    for seed in seeds {
        match graph.node(seed) {
            None => dangling.push(DocEdge::new(
                seed.clone(),
                seed.clone(),
                DocEdgeType::References,
            )),
            Some(node) => {
                if let Some(label) = denied_label(&node.policy_labels, policy) {
                    blocked.insert(seed.clone(), label);
                } else if !reached.contains_key(seed) {
                    reached.insert(seed.clone(), 0);
                    queue.push_back((seed.clone(), 0));
                }
            }
        }
    }

    while let Some((current, depth)) = queue.pop_front() {
        if policy.max_depth.is_some_and(|max| depth >= max) {
            frontier.insert(current);
            continue;
        }
        let mut neighbors: Vec<(ModuleId, &DocEdge)> = Vec::new();
        for edge in graph.edges() {
            if !policy.follows(edge.kind) {
                continue;
            }
            let other = if edge.from == current {
                &edge.to
            } else if edge.to == current {
                &edge.from
            } else {
                continue;
            };
            neighbors.push((other.clone(), edge));
        }
        neighbors.sort_by(|a, b| a.0.cmp(&b.0));
        for (neighbor, edge) in neighbors {
            if reached.contains_key(&neighbor) || blocked.contains_key(&neighbor) {
                continue;
            }
            match graph.node(&neighbor) {
                None => {
                    if !dangling.contains(edge) {
                        dangling.push(edge.clone());
                    }
                }
                Some(node) => match denied_label(&node.policy_labels, policy) {
                    Some(label) => {
                        blocked.insert(neighbor, label);
                    }
                    None => {
                        reached.insert(neighbor.clone(), depth + 1);
                        queue.push_back((neighbor, depth + 1));
                    }
                },
            }
        }
    }

    let present_types: BTreeSet<DocEdgeType> = graph.edges().iter().map(|edge| edge.kind).collect();
    let filtered_types = present_types
        .into_iter()
        .filter(|kind| !policy.follows(*kind))
        .collect();

    Traversal {
        seeds: seeds.to_vec(),
        reached,
        blocked_by_policy: blocked,
        frontier,
        dangling,
        filtered_types,
    }
}

fn denied_label(labels: &[String], policy: &TraversalPolicy) -> Option<String> {
    labels
        .iter()
        .find(|label| policy.denied_labels.contains(*label))
        .cloned()
}
