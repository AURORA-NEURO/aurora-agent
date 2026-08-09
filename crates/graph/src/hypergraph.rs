//! The hypergraph projection: the factor and relation inspector.
//!
//! 43.01 names this view twice. Once as a projection — `H_q = V_hyper(D_q)`, generated, not
//! canonical — and once as a remedy: "if a graph view hides multiway semantics, provide the factor
//! or relation inspector alongside it". This is that inspector. Where the graph projection has to
//! split a five-way factor into ten pairwise edges and admit the loss, this view keeps the factor
//! whole as a single hyperedge over its variables.
//!
//! **Explicit incidence here is a rendering choice.** 43.07 stores a factor as a typed signature
//! `φ_i : X_{S_i} → K_i` with a cost, and 43.01 is unambiguous that hypergraph systems "make
//! incidence explicit, but they do not by themselves solve the central context-engineering
//! problem". The pin list below is materialised so that a renderer, an accessibility layer or a
//! debugger has something to walk. Reading it back as the storage model would invert the
//! blueprint's central decision, so [`HypergraphBody::rendering_note`] says so inside the payload
//! and travels with it wherever the view goes.
//!
//! What this view still cannot show: the factor's *semantics*. A hyperedge says which variables a
//! factor constrains jointly; it does not say what the constraint is. That is recorded as a
//! dropped feature pointing back at the section, not glossed over.

use crate::error::ProjectionError;
use crate::factors::selected_factors;
use crate::fidelity::{DropReason, FidelityLedger};
use crate::identity::decision_node_id;
use crate::markers::{conflicts, obligations, ConflictMarker, ObligationMarker};
use crate::view::{ProjectedBody, Projection, ProjectionKind};
use bioprism_section::DecisionSection;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

/// The note that travels inside every hypergraph view.
pub const RENDERING_NOTE: &str = "Incidence is materialised in this view because a hypergraph \
rendering requires it. The canonical substrate stores typed factor signatures and local evidence \
sections (43.07), not an incidence list; this view is a generated projection (43.01) and must not \
be read back as storage or as execution semantics.";

/// Which side of a factor's signature a variable sits on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PinRole {
    Input,
    Output,
    /// The variable appears on both sides of the signature.
    Both,
}

impl PinRole {
    pub fn as_str(self) -> &'static str {
        match self {
            PinRole::Input => "input",
            PinRole::Output => "output",
            PinRole::Both => "both",
        }
    }
}

/// One attachment of a hyperedge to a vertex.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pin {
    pub variable: String,
    pub role: PinRole,
}

/// A factor, kept whole.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Hyperedge {
    pub id: String,
    /// The factor's `kind` from its echoed document — `deterministic_rule` and so on.
    pub kind: String,
    pub pins: Vec<Pin>,
    /// Number of distinct variables. The width proxy 43.18 reports, not width itself.
    pub arity: usize,
    /// True when the graph projection cannot express this factor without loss.
    pub multiway: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<Value>,
}

/// A variable, with the evidence that supplies it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HyperVertex {
    pub variable: String,
    /// Ids of evidence capsules whose `provides` is this variable.
    pub supplied_by: Vec<String>,
    /// Ids of hyperedges incident on this variable.
    pub incident_edges: Vec<String>,
}

/// The rendered hypergraph.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HypergraphBody {
    pub decision_node: String,
    pub vertices: Vec<HyperVertex>,
    pub factor_edges: Vec<Hyperedge>,
    pub max_arity: usize,
    /// Unresolved obligations, carried whole. A factor inspector that showed only the factors
    /// would present a blocked region as a solved one.
    pub obligations: Vec<ObligationMarker>,
    pub conflicts: Vec<ConflictMarker>,
    pub rendering_note: String,
}

impl HypergraphBody {
    pub fn edge(&self, id: &str) -> Option<&Hyperedge> {
        self.factor_edges.iter().find(|edge| edge.id == id)
    }

    /// Hyperedges the graph projection would have had to flatten.
    pub fn multiway_edges(&self) -> impl Iterator<Item = &Hyperedge> {
        self.factor_edges.iter().filter(|edge| edge.multiway)
    }
}

impl ProjectedBody for HypergraphBody {
    fn stable_handles(&self) -> BTreeSet<String> {
        let mut handles: BTreeSet<String> = BTreeSet::new();
        handles.insert(self.decision_node.clone());
        for vertex in &self.vertices {
            handles.extend(vertex.supplied_by.iter().cloned());
        }
        for edge in &self.factor_edges {
            handles.insert(edge.id.clone());
        }
        for obligation in &self.obligations {
            handles.insert(obligation.id.clone());
            handles.extend(obligation.handles.iter().cloned());
        }
        for conflict in &self.conflicts {
            handles.insert(conflict.id.clone());
        }
        handles
    }
}

/// Projects a compiled region as high-arity factor incidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct HypergraphProjection;

impl HypergraphProjection {
    pub const fn new() -> Self {
        HypergraphProjection
    }
}

impl Projection for HypergraphProjection {
    type Body = HypergraphBody;
    const KIND: ProjectionKind = ProjectionKind::Hypergraph;

    fn render(
        &self,
        section: &DecisionSection,
        ledger: &mut FidelityLedger,
    ) -> Result<HypergraphBody, ProjectionError> {
        let factors = selected_factors(section)?;

        let mut suppliers: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for capsule in &section.selected_evidence {
            suppliers
                .entry(capsule.provides.clone())
                .or_default()
                .push(capsule.id.clone());
        }

        let mut incident: BTreeMap<String, Vec<String>> = BTreeMap::new();
        let mut factor_edges = Vec::with_capacity(factors.len());
        for factor in &factors {
            let inputs: BTreeSet<&str> = factor.inputs.iter().map(String::as_str).collect();
            let outputs: BTreeSet<&str> = factor.outputs.iter().map(String::as_str).collect();
            let variables: BTreeSet<&str> = inputs.union(&outputs).copied().collect();

            let pins: Vec<Pin> = variables
                .iter()
                .map(|variable| Pin {
                    variable: (*variable).to_string(),
                    role: match (inputs.contains(variable), outputs.contains(variable)) {
                        (true, true) => PinRole::Both,
                        (false, true) => PinRole::Output,
                        _ => PinRole::Input,
                    },
                })
                .collect();

            for variable in &variables {
                incident
                    .entry((*variable).to_string())
                    .or_default()
                    .push(factor.id.clone());
            }

            factor_edges.push(Hyperedge {
                id: factor.id.clone(),
                kind: factor.kind.clone(),
                arity: pins.len(),
                multiway: factor.is_multiway(),
                pins,
                scope: factor.scope.clone(),
            });
        }

        let mut variable_names: BTreeSet<String> = suppliers.keys().cloned().collect();
        variable_names.extend(incident.keys().cloned());

        let vertices: Vec<HyperVertex> = variable_names
            .into_iter()
            .map(|variable| HyperVertex {
                supplied_by: suppliers.get(&variable).cloned().unwrap_or_default(),
                incident_edges: incident.get(&variable).cloned().unwrap_or_default(),
                variable,
            })
            .collect();

        let obligation_markers = obligations(section);
        for marker in &obligation_markers {
            ledger.carry_obligation(marker.id.clone());
        }
        let conflict_markers = conflicts(section);
        for marker in &conflict_markers {
            ledger.carry_conflict(marker.id.clone());
        }

        if !factors.is_empty() {
            ledger.drop_feature(
                "factor semantics",
                DropReason::NotRepresentable,
                factors.len(),
                factors.iter().take(3).map(|f| f.id.clone()).collect(),
                "selected_factors in the Decision Section",
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

        Ok(HypergraphBody {
            decision_node: decision_node_id(&section.query_id),
            max_arity: factor_edges.iter().map(|edge| edge.arity).max().unwrap_or(0),
            vertices,
            factor_edges,
            obligations: obligation_markers,
            conflicts: conflict_markers,
            rendering_note: RENDERING_NOTE.to_string(),
        })
    }
}
