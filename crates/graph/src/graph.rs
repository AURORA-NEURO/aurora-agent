//! The graph projection.
//!
//! Blueprint 41.01 asks for "addressable modules and typed dependency edges rather than a linear
//! book", 41.03 fixes the edge vocabulary, and 42.01 requires that "every rendered node, edge,
//! group, score, and warning resolves to a stable graph object, a time cut, source provenance,
//! uncertainty, access policy, and a machine representation". 43.01 then demotes the result: this
//! is a *generated projection* of a compiled region, never storage and never execution semantics.
//!
//! Every edge is typed and directed. There is no untyped adjacency list and no "related" edge:
//! when the relation between two objects is not one the vocabulary names, this projection emits
//! nothing, because 43.01's failure-mode contract says to "expose alternatives instead of
//! inserting a guessed edge".
//!
//! The edge directions this projection emits, each chosen so the vocabulary's own gloss reads
//! true:
//!
//! | Edge | Emitted as | Gloss it satisfies |
//! |---|---|---|
//! | `part_of` | evidence → decision, factor → decision | source belongs to target section |
//! | `provides` | evidence → factor consuming its variable | source produces inputs used by target |
//! | `implements` | evidence → variable it supplies, factor → output variable | source realizes target |
//! | `requires` | factor → input variable | source depends on target contract |
//! | `governs` | decision → unresolved obligation | target constrains source |
//! | `evaluates` | oracle → decision | source tests target |
//! | `contains` | oracle → witness, decision → refinement option | source indexes target |
//! | `references` | evidence → provenance handle, obligation → withheld fact | source links to target |
//!
//! `supersedes` and `related` are never emitted; see [`crate::vocabulary`].
//!
//! No parameters. [`GraphProjection`] is a unit struct on purpose: there is no depth, radius or
//! node cap to set, because 43.01 defines completeness by query obligations rather than by
//! neighbourhood radius. A caller who wants a smaller view must compile a smaller region.

use crate::error::ProjectionError;
use crate::factors::{selected_factors, SelectedFactor};
use crate::fidelity::{DropReason, FidelityLedger};
use crate::identity::{
    decision_node_id, oracle_node_id, refinement_id, source_node_id, variable_node_id,
};
use crate::markers::{conflicts, obligations};
use crate::view::{ProjectedBody, Projection, ProjectionKind};
use crate::vocabulary::{EdgeType, NodeKind, NodeStatus};
use bioprism_section::DecisionSection;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

/// One node of a projected region.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GraphNode {
    pub id: String,
    pub kind: NodeKind,
    pub label: String,
    pub status: NodeStatus,
    /// The stable handle a reader follows back into the section, world or certificate. Synthetic
    /// nodes (the decision, variables, the oracle) carry the handle they were derived from.
    pub handle: String,
    /// Validity scope, for nodes that have one. 43.04 makes evidence true only inside a scope, so
    /// dropping it from a node would turn a local section into a global claim.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<Value>,
}

/// One typed, directed edge.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct GraphEdge {
    pub from: String,
    pub edge: EdgeType,
    pub to: String,
}

/// A factor whose joint semantics the pairwise encoding cannot state.
///
/// 43.01: "If a graph view hides multiway semantics, provide the factor or relation inspector
/// alongside it." The note names the inspector rather than merely warning.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultiwayNote {
    pub factor_id: String,
    pub arity: usize,
    pub variables: Vec<String>,
    pub inspector: ProjectionKind,
}

/// An edge type in the vocabulary that this view deliberately does not emit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NotEmitted {
    pub edge: EdgeType,
    pub reason: String,
}

/// The vocabulary entry shipped with the view so a renderer never infers direction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EdgeGloss {
    pub edge: EdgeType,
    pub gloss: String,
    pub normative: bool,
}

/// The rendered graph.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GraphBody {
    /// Id of the single [`NodeKind::Decision`] node: the compiled region itself.
    pub decision_node: String,
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
    /// Obligations, rendered as nodes and repeated here so a consumer never has to scan for them.
    pub obligation_nodes: Vec<String>,
    /// Oracle witnesses, likewise.
    pub conflict_nodes: Vec<String>,
    pub multiway_factors: Vec<MultiwayNote>,
    pub edge_vocabulary: Vec<EdgeGloss>,
    pub not_emitted: Vec<NotEmitted>,
}

impl GraphBody {
    pub fn node(&self, id: &str) -> Option<&GraphNode> {
        self.nodes.iter().find(|node| node.id == id)
    }

    pub fn nodes_of(&self, kind: NodeKind) -> impl Iterator<Item = &GraphNode> {
        self.nodes.iter().filter(move |node| node.kind == kind)
    }

    pub fn edges_of(&self, edge: EdgeType) -> impl Iterator<Item = &GraphEdge> {
        self.edges.iter().filter(move |candidate| candidate.edge == edge)
    }

    /// Outgoing edges of a node. Present for inspection, not as a traversal budget: nothing in
    /// this crate expands a neighbourhood to decide relevance.
    pub fn outgoing(&self, id: &str) -> impl Iterator<Item = &GraphEdge> {
        let owned = id.to_string();
        self.edges.iter().filter(move |edge| edge.from == owned)
    }
}

impl ProjectedBody for GraphBody {
    fn stable_handles(&self) -> BTreeSet<String> {
        self.nodes.iter().map(|node| node.handle.clone()).collect()
    }
}

/// Projects a compiled region as typed nodes and directed edges.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GraphProjection;

impl GraphProjection {
    pub const fn new() -> Self {
        GraphProjection
    }
}

impl Projection for GraphProjection {
    type Body = GraphBody;
    const KIND: ProjectionKind = ProjectionKind::Graph;

    fn render(
        &self,
        section: &DecisionSection,
        ledger: &mut FidelityLedger,
    ) -> Result<GraphBody, ProjectionError> {
        let factors = selected_factors(section)?;
        let mut builder = Builder::new(section);

        builder.add_decision(section);
        builder.add_evidence(section, &factors);
        builder.add_factors(&factors);
        builder.add_oracle(section);
        builder.add_obligations(section, ledger);
        builder.add_conflicts(section, ledger);
        builder.add_refinements(section);

        let multiway_factors: Vec<MultiwayNote> = factors
            .iter()
            .filter(|factor| factor.is_multiway())
            .map(|factor| MultiwayNote {
                factor_id: factor.id.clone(),
                arity: factor.arity(),
                variables: distinct_variables(factor),
                inspector: ProjectionKind::Hypergraph,
            })
            .collect();

        if !multiway_factors.is_empty() {
            ledger.drop_feature(
                "joint factor semantics",
                DropReason::FlattenedToBinaryEdges,
                multiway_factors.len(),
                multiway_factors
                    .iter()
                    .take(3)
                    .map(|note| note.factor_id.clone())
                    .collect(),
                "hypergraph projection",
            );
        }

        if !section.selected_evidence.is_empty() {
            ledger.drop_feature(
                "evidence values",
                DropReason::ValuesElided,
                section.selected_evidence.len(),
                section
                    .selected_evidence
                    .iter()
                    .take(3)
                    .map(|capsule| capsule.id.clone())
                    .collect(),
                "table projection or Decision Section layer L2",
            );
        }

        let body = builder.finish(section, multiway_factors);
        Ok(body)
    }
}

/// Accumulates nodes and edges while de-duplicating node ids.
struct Builder {
    nodes: BTreeMap<String, GraphNode>,
    order: Vec<String>,
    edges: BTreeSet<GraphEdge>,
    decision_node: String,
    obligation_nodes: Vec<String>,
    conflict_nodes: Vec<String>,
}

impl Builder {
    fn new(section: &DecisionSection) -> Self {
        Builder {
            nodes: BTreeMap::new(),
            order: Vec::new(),
            edges: BTreeSet::new(),
            decision_node: decision_node_id(&section.query_id),
            obligation_nodes: Vec::new(),
            conflict_nodes: Vec::new(),
        }
    }

    /// Inserts a node, keeping the first status unless the new one is stronger.
    ///
    /// A fact named by an obligation and also delivered must not be relabelled `withheld`; a
    /// delivered fact that an obligation later names must not lose the fact that it is contested.
    fn put(&mut self, node: GraphNode) {
        match self.nodes.get_mut(&node.id) {
            Some(existing) => {
                if existing.status == NodeStatus::Withheld && node.status != NodeStatus::Withheld {
                    *existing = node;
                }
            }
            None => {
                self.order.push(node.id.clone());
                self.nodes.insert(node.id.clone(), node);
            }
        }
    }

    fn link(&mut self, from: &str, edge: EdgeType, to: &str) {
        self.edges.insert(GraphEdge {
            from: from.to_string(),
            edge,
            to: to.to_string(),
        });
    }

    fn add_decision(&mut self, section: &DecisionSection) {
        let id = self.decision_node.clone();
        self.put(GraphNode {
            id,
            kind: NodeKind::Decision,
            label: section.goal.clone(),
            status: if section.requires_refinement() {
                NodeStatus::Blocked
            } else {
                NodeStatus::Delivered
            },
            handle: section.query_id.clone(),
            scope: None,
        });
    }

    fn add_evidence(&mut self, section: &DecisionSection, factors: &[SelectedFactor]) {
        let decision = self.decision_node.clone();
        for capsule in &section.selected_evidence {
            self.put(GraphNode {
                id: capsule.id.clone(),
                kind: NodeKind::Evidence,
                label: capsule.provides.clone(),
                status: NodeStatus::Delivered,
                handle: capsule.id.clone(),
                scope: Some(capsule.scope.clone()),
            });
            self.link(&capsule.id, EdgeType::PartOf, &decision);

            let variable = variable_node_id(&capsule.provides);
            self.put(variable_node(&capsule.provides));
            self.link(&capsule.id, EdgeType::Implements, &variable);

            for factor in factors {
                if factor.inputs.contains(&capsule.provides) {
                    self.link(&capsule.id, EdgeType::Provides, &factor.id);
                }
            }

            for handle in &capsule.provenance {
                let source = source_node_id(handle);
                self.put(GraphNode {
                    id: source.clone(),
                    kind: NodeKind::Source,
                    label: handle.clone(),
                    status: NodeStatus::Delivered,
                    handle: handle.clone(),
                    scope: None,
                });
                self.link(&capsule.id, EdgeType::References, &source);
            }
        }
    }

    fn add_factors(&mut self, factors: &[SelectedFactor]) {
        let decision = self.decision_node.clone();
        for factor in factors {
            self.put(GraphNode {
                id: factor.id.clone(),
                kind: NodeKind::Factor,
                label: factor.kind.clone(),
                status: NodeStatus::Delivered,
                handle: factor.id.clone(),
                scope: factor.scope.clone(),
            });
            self.link(&factor.id, EdgeType::PartOf, &decision);

            for input in &factor.inputs {
                let variable = variable_node_id(input);
                self.put(variable_node(input));
                self.link(&factor.id, EdgeType::Requires, &variable);
            }
            for output in &factor.outputs {
                let variable = variable_node_id(output);
                self.put(variable_node(output));
                self.link(&factor.id, EdgeType::Implements, &variable);
            }
        }
    }

    fn add_oracle(&mut self, section: &DecisionSection) {
        let decision = self.decision_node.clone();
        let oracle = oracle_node_id(&section.oracle.oracle_kind);
        self.put(GraphNode {
            id: oracle.clone(),
            kind: NodeKind::Oracle,
            label: section.oracle.status.as_str().to_string(),
            status: if section.oracle.witnesses.is_empty() {
                NodeStatus::Delivered
            } else {
                NodeStatus::Contradicted
            },
            handle: section.oracle.oracle_kind.clone(),
            scope: None,
        });
        self.link(&oracle, EdgeType::Evaluates, &decision);
    }

    fn add_obligations(&mut self, section: &DecisionSection, ledger: &mut FidelityLedger) {
        let decision = self.decision_node.clone();
        for marker in obligations(section) {
            self.put(GraphNode {
                id: marker.id.clone(),
                kind: NodeKind::Obligation,
                label: marker.kind.clone(),
                status: NodeStatus::Blocked,
                handle: marker.id.clone(),
                scope: None,
            });
            self.link(&decision, EdgeType::Governs, &marker.id);
            for handle in &marker.handles {
                self.put(withheld_node(handle));
                self.link(&marker.id, EdgeType::References, handle);
            }
            ledger.carry_obligation(marker.id.clone());
            self.obligation_nodes.push(marker.id);
        }
    }

    fn add_conflicts(&mut self, section: &DecisionSection, ledger: &mut FidelityLedger) {
        let oracle = oracle_node_id(&section.oracle.oracle_kind);
        for marker in conflicts(section) {
            self.put(GraphNode {
                id: marker.id.clone(),
                kind: NodeKind::Conflict,
                label: marker.witness_kind.clone(),
                status: NodeStatus::Contradicted,
                handle: marker.id.clone(),
                scope: None,
            });
            self.link(&oracle, EdgeType::Contains, &marker.id);
            ledger.carry_conflict(marker.id.clone());
            self.conflict_nodes.push(marker.id);
        }
    }

    fn add_refinements(&mut self, section: &DecisionSection) {
        let decision = self.decision_node.clone();
        for (index, option) in section.refinement_frontier.iter().enumerate() {
            let id = refinement_id(index, &option.action);
            self.put(GraphNode {
                id: id.clone(),
                kind: NodeKind::Refinement,
                label: option.action.clone(),
                status: NodeStatus::Delivered,
                handle: id.clone(),
                scope: None,
            });
            self.link(&decision, EdgeType::Contains, &id);
            for fact in &option.facts {
                self.put(withheld_node(fact));
                self.link(&id, EdgeType::References, fact);
            }
        }
    }

    fn finish(mut self, section: &DecisionSection, multiway: Vec<MultiwayNote>) -> GraphBody {
        let nodes: Vec<GraphNode> = self
            .order
            .iter()
            .filter_map(|id| self.nodes.remove(id))
            .collect();
        GraphBody {
            decision_node: decision_node_id(&section.query_id),
            nodes,
            edges: self.edges.into_iter().collect(),
            obligation_nodes: self.obligation_nodes,
            conflict_nodes: self.conflict_nodes,
            multiway_factors: multiway,
            edge_vocabulary: EdgeType::ALL
                .into_iter()
                .map(|edge| EdgeGloss {
                    edge,
                    gloss: edge.gloss().to_string(),
                    normative: edge.is_normative(),
                })
                .collect(),
            not_emitted: vec![
                NotEmitted {
                    edge: EdgeType::Supersedes,
                    reason: "a Decision Section carries no amendment relation; supersession lives \
                             in the world event structure (43.09) and would have to be guessed here"
                        .into(),
                },
                NotEmitted {
                    edge: EdgeType::Related,
                    reason: "non-normative adjacency is a guessed edge; 43.01 requires exposing \
                             alternatives instead of inserting one"
                        .into(),
                },
            ],
        }
    }
}

fn variable_node(name: &str) -> GraphNode {
    GraphNode {
        id: variable_node_id(name),
        kind: NodeKind::Variable,
        label: name.to_string(),
        status: NodeStatus::Delivered,
        handle: name.to_string(),
        scope: None,
    }
}

/// A node for something the region names but does not deliver.
fn withheld_node(handle: &str) -> GraphNode {
    GraphNode {
        id: handle.to_string(),
        kind: NodeKind::Evidence,
        label: "not delivered in this region".to_string(),
        status: NodeStatus::Withheld,
        handle: handle.to_string(),
        scope: None,
    }
}

fn distinct_variables(factor: &SelectedFactor) -> Vec<String> {
    let unique: BTreeSet<String> = factor
        .inputs
        .iter()
        .chain(factor.outputs.iter())
        .cloned()
        .collect();
    unique.into_iter().collect()
}
