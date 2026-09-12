//! Signed mechanism-network propagation for preclinical glioma research.
//!
//! A multimodal or spatial result often identifies a connected set of candidate regulators rather
//! than one isolated claim. This feature provides a bounded, deterministic graph calculation for
//! prioritising that network: activating edges transmit support, inhibiting edges transmit a
//! sign-reversed score, and a damped fixed-point iteration prevents cycles from becoming
//! unbounded confidence. Low-confidence edges, disconnected nodes, non-convergence, and
//! contradiction are retained as first-class limitations. The result is a research priority
//! artifact, never a diagnosis, prognosis, or treatment recommendation.

use crate::glioma_engine::{GliomaModality, GliomaModelSystem};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P05-F10";
pub const OUTPUT_SCHEMA: &str = "GliomaMechanismGraphPropagation1@1";
pub const MAX_NODES: usize = 4_096;
pub const MAX_EDGES: usize = 32_768;
pub const MAX_ITERATIONS: u16 = 512;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismGraphRequest {
    pub objective: String,
    pub model_system: GliomaModelSystem,
    pub max_iterations: u16,
    pub convergence_tolerance_milli: u16,
    /// Weight retained from each node's direct evidence; the remainder is network propagation.
    pub damping_milli: u16,
    pub min_edge_confidence_milli: u16,
    pub top_k: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismGraphNode {
    pub node_id: String,
    pub label: String,
    pub modality: GliomaModality,
    pub prior_milli: i16,
    pub support_milli: u16,
    pub contradiction_milli: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MechanismGraphRelation {
    Activates,
    Inhibits,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismGraphEdge {
    pub edge_id: String,
    pub source_node_id: String,
    pub target_node_id: String,
    pub relation: MechanismGraphRelation,
    pub confidence_milli: u16,
    pub evidence_order: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismNodeScore {
    pub node_id: String,
    pub base_score_milli: i64,
    pub propagated_score_milli: i64,
    pub absolute_change_milli: u64,
    pub incoming_edge_count: u32,
    pub outgoing_edge_count: u32,
    pub direct_evidence_state: String,
    pub uncertainty: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MechanismGraphDisposition {
    Qualified,
    Partial,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismGraphPropagation {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub model_system: GliomaModelSystem,
    pub node_order: Vec<String>,
    pub edge_order: Vec<String>,
    pub active_edge_order: Vec<String>,
    pub excluded_edge_order: Vec<String>,
    pub ranking_order: Vec<String>,
    pub scores: Vec<MechanismNodeScore>,
    pub converged: bool,
    pub iterations_run: u16,
    pub max_delta_milli: u64,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: MechanismGraphDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MechanismGraphError {
    #[error("mechanism graph request is invalid: {0}")]
    InvalidRequest(String),
    #[error("mechanism graph input is invalid: {0}")]
    InvalidInput(String),
    #[error("mechanism graph output is invalid: {0}")]
    InvalidOutput(String),
    #[error("mechanism graph digest failed: {0}")]
    Digest(String),
}

fn digest_input(output: &MechanismGraphPropagation) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "model_system": output.model_system,
        "node_order": output.node_order,
        "edge_order": output.edge_order,
        "active_edge_order": output.active_edge_order,
        "excluded_edge_order": output.excluded_edge_order,
        "ranking_order": output.ranking_order,
        "scores": output.scores,
        "converged": output.converged,
        "iterations_run": output.iterations_run,
        "max_delta_milli": output.max_delta_milli,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
    })
}

fn clamp_score(value: i128) -> i64 {
    value.clamp(-1_000, 1_000) as i64
}

fn direct_score(node: &MechanismGraphNode) -> i64 {
    clamp_score(
        i128::from(node.prior_milli) + i128::from(node.support_milli)
            - i128::from(node.contradiction_milli),
    )
}

impl MechanismGraphPropagation {
    pub fn validate(&self) -> Result<(), MechanismGraphError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.node_order.windows(2).any(|pair| pair[0] >= pair[1])
            || self.edge_order.windows(2).any(|pair| pair[0] >= pair[1])
            || self
                .active_edge_order
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || self
                .excluded_edge_order
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || self
                .scores
                .windows(2)
                .any(|pair| pair[0].node_id >= pair[1].node_id)
            || self.ranking_order.windows(2).any(|pair| pair[0] == pair[1])
            || self.max_delta_milli > 2_000
            || self.iterations_run > MAX_ITERATIONS
            || self.scores.iter().any(|score| {
                score.base_score_milli.abs() > 1_000
                    || score.propagated_score_milli.abs() > 1_000
                    || score.absolute_change_milli > 2_000
                    || score.direct_evidence_state.trim().is_empty()
                    || score.uncertainty.windows(2).any(|pair| pair[0] >= pair[1])
            })
            || self
                .negative_evidence
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || self.uncertainty.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return Err(MechanismGraphError::InvalidOutput(
                "identity, ordering, score bounds, or convergence metadata is invalid".into(),
            ));
        }
        let node_ids = self.node_order.iter().cloned().collect::<BTreeSet<_>>();
        let score_ids = self
            .scores
            .iter()
            .map(|score| score.node_id.clone())
            .collect::<BTreeSet<_>>();
        let edge_ids = self.edge_order.iter().cloned().collect::<BTreeSet<_>>();
        let active_ids = self
            .active_edge_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let excluded_ids = self
            .excluded_edge_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let ranking_ids = self.ranking_order.iter().cloned().collect::<BTreeSet<_>>();
        if node_ids.is_empty()
            || score_ids != node_ids
            || self.scores.len() != node_ids.len()
            || self.ranking_order.is_empty()
            || self.ranking_order.len() > node_ids.len()
            || ranking_ids.len() != self.ranking_order.len()
            || !ranking_ids.is_subset(&node_ids)
            || active_ids.intersection(&excluded_ids).next().is_some()
            || active_ids
                .union(&excluded_ids)
                .cloned()
                .collect::<BTreeSet<_>>()
                != edge_ids
        {
            return Err(MechanismGraphError::InvalidOutput(
                "node/score/ranking or edge partitions do not reconcile".into(),
            ));
        }
        let score_map = self
            .scores
            .iter()
            .map(|score| (score.node_id.as_str(), score.propagated_score_milli))
            .collect::<BTreeMap<_, _>>();
        if self.ranking_order.windows(2).any(|pair| {
            score_map[pair[0].as_str()] < score_map[pair[1].as_str()]
                || (score_map[pair[0].as_str()] == score_map[pair[1].as_str()] && pair[0] > pair[1])
        }) {
            return Err(MechanismGraphError::InvalidOutput(
                "mechanism ranking is not score ordered".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| MechanismGraphError::Digest(error.to_string()))?;
        if self.digest != expected {
            return Err(MechanismGraphError::InvalidOutput(
                "mechanism graph digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

/// Propagate signed mechanistic support over a bounded evidence graph.
pub fn propagate_glioma_mechanism_graph(
    request: &MechanismGraphRequest,
    nodes: &[MechanismGraphNode],
    edges: &[MechanismGraphEdge],
) -> Result<MechanismGraphPropagation, MechanismGraphError> {
    if request.objective.trim().is_empty()
        || request.max_iterations == 0
        || request.max_iterations > MAX_ITERATIONS
        || request.convergence_tolerance_milli == 0
        || request.convergence_tolerance_milli > 2_000
        || request.damping_milli > 1_000
        || request.min_edge_confidence_milli > 1_000
        || request.top_k == 0
        || nodes.is_empty()
        || nodes.len() > MAX_NODES
        || edges.len() > MAX_EDGES
    {
        return Err(MechanismGraphError::InvalidRequest(
            "objective, iteration/tolerance/damping bounds, top-k, and bounded non-empty nodes are required".into(),
        ));
    }
    let mut node_ids = BTreeSet::new();
    for node in nodes {
        if node.node_id.trim().is_empty()
            || node.label.trim().is_empty()
            || node.prior_milli.abs() > 1_000
            || node.support_milli > 1_000
            || node.contradiction_milli > 1_000
            || !node_ids.insert(node.node_id.clone())
        {
            return Err(MechanismGraphError::InvalidInput(
                "node identity, label, score bounds, or uniqueness is invalid".into(),
            ));
        }
    }
    let mut edge_ids = BTreeSet::new();
    for edge in edges {
        if edge.edge_id.trim().is_empty()
            || edge.source_node_id == edge.target_node_id
            || !node_ids.contains(&edge.source_node_id)
            || !node_ids.contains(&edge.target_node_id)
            || edge.confidence_milli == 0
            || edge.confidence_milli > 1_000
            || !edge.edge_id.is_ascii()
            || !edge.evidence_order.windows(2).all(|pair| pair[0] < pair[1])
            || edge
                .evidence_order
                .iter()
                .any(|item| item.trim().is_empty())
            || !edge_ids.insert(edge.edge_id.clone())
        {
            return Err(MechanismGraphError::InvalidInput(
                "edge identity, endpoint, confidence, evidence ordering, or uniqueness is invalid"
                    .into(),
            ));
        }
    }
    let node_map = nodes
        .iter()
        .map(|node| (node.node_id.clone(), node))
        .collect::<BTreeMap<_, _>>();
    let mut active_edges = edges
        .iter()
        .filter(|edge| edge.confidence_milli >= request.min_edge_confidence_milli)
        .collect::<Vec<_>>();
    active_edges.sort_by(|left, right| left.edge_id.cmp(&right.edge_id));
    let mut excluded_edges = edges
        .iter()
        .filter(|edge| edge.confidence_milli < request.min_edge_confidence_milli)
        .map(|edge| edge.edge_id.clone())
        .collect::<Vec<_>>();
    excluded_edges.sort();
    let mut incoming = BTreeMap::<String, Vec<&MechanismGraphEdge>>::new();
    let mut incoming_count = BTreeMap::<String, u32>::new();
    let mut outgoing_count = BTreeMap::<String, u32>::new();
    for edge in &active_edges {
        incoming
            .entry(edge.target_node_id.clone())
            .or_default()
            .push(edge);
        *incoming_count
            .entry(edge.target_node_id.clone())
            .or_default() += 1;
        *outgoing_count
            .entry(edge.source_node_id.clone())
            .or_default() += 1;
    }
    for incoming_edges in incoming.values_mut() {
        incoming_edges.sort_by(|left, right| left.edge_id.cmp(&right.edge_id));
    }
    let node_order = node_ids.iter().cloned().collect::<Vec<_>>();
    let base_scores = node_order
        .iter()
        .map(|node_id| (node_id.clone(), direct_score(node_map[node_id])))
        .collect::<BTreeMap<_, _>>();
    let mut current = base_scores.clone();
    let mut iterations_run = 0_u16;
    let mut max_delta = u64::MAX;
    let mut converged = false;
    while iterations_run < request.max_iterations {
        let mut next = BTreeMap::new();
        max_delta = 0;
        for node_id in &node_order {
            let base = base_scores[node_id] as i128;
            let incoming_edges = incoming.get(node_id);
            let network = incoming_edges.into_iter().flatten().fold(
                (0_i128, 0_u128),
                |(numerator, denominator), edge| {
                    let sign = match edge.relation {
                        MechanismGraphRelation::Activates => 1_i128,
                        MechanismGraphRelation::Inhibits => -1_i128,
                    };
                    (
                        numerator
                            + sign
                                * i128::from(edge.confidence_milli)
                                * i128::from(current[&edge.source_node_id]),
                        denominator + u128::from(edge.confidence_milli),
                    )
                },
            );
            let network_score = if network.1 == 0 {
                base
            } else {
                network.0 / network.1 as i128
            };
            let blended = (i128::from(request.damping_milli) * base
                + i128::from(1_000 - request.damping_milli) * network_score)
                / 1_000;
            let score = clamp_score(blended);
            max_delta = max_delta.max(
                (i128::from(score) - i128::from(current[node_id]))
                    .unsigned_abs()
                    .min(u128::from(u64::MAX)) as u64,
            );
            next.insert(node_id.clone(), score);
        }
        current = next;
        iterations_run += 1;
        if max_delta <= u64::from(request.convergence_tolerance_milli) {
            converged = true;
            break;
        }
    }
    let mut ranking_order = node_order.clone();
    ranking_order.sort_by(|left, right| {
        current[right]
            .cmp(&current[left])
            .then_with(|| left.cmp(right))
    });
    ranking_order.truncate(request.top_k.min(ranking_order.len()));
    let mut uncertainty = BTreeSet::new();
    if !converged {
        uncertainty.insert("propagation-did-not-converge-within-bound".into());
    }
    for node_id in &node_order {
        if !incoming_count.contains_key(node_id) && !outgoing_count.contains_key(node_id) {
            uncertainty.insert(format!("disconnected-node:{node_id}"));
        }
    }
    for edge_id in &excluded_edges {
        uncertainty.insert(format!("low-confidence-edge-excluded:{edge_id}"));
    }
    let mut negative = BTreeSet::new();
    if active_edges.is_empty() {
        negative.insert("no-edge-meets-confidence-floor".into());
    }
    if nodes
        .iter()
        .all(|node| node.contradiction_milli > node.support_milli)
    {
        negative.insert("direct-contradiction-exceeds-direct-support-for-all-nodes".into());
    }
    let scores = node_order
        .iter()
        .map(|node_id| {
            let node = node_map[node_id];
            let base = base_scores[node_id];
            let propagated = current[node_id];
            let mut node_uncertainty = BTreeSet::new();
            if !incoming_count.contains_key(node_id) && !outgoing_count.contains_key(node_id) {
                node_uncertainty.insert("node-is-disconnected".into());
            }
            if node.contradiction_milli > node.support_milli {
                node_uncertainty.insert("direct-contradiction-exceeds-support".into());
            }
            MechanismNodeScore {
                node_id: node_id.clone(),
                base_score_milli: base,
                propagated_score_milli: propagated,
                absolute_change_milli: (i128::from(propagated) - i128::from(base)).unsigned_abs()
                    as u64,
                incoming_edge_count: incoming_count.get(node_id).copied().unwrap_or(0),
                outgoing_edge_count: outgoing_count.get(node_id).copied().unwrap_or(0),
                direct_evidence_state: if node.contradiction_milli > node.support_milli {
                    "contested".into()
                } else if node.support_milli > 0 {
                    "supported".into()
                } else {
                    "unresolved".into()
                },
                uncertainty: node_uncertainty.into_iter().collect(),
            }
        })
        .collect::<Vec<_>>();
    let disposition = if !converged || active_edges.is_empty() {
        MechanismGraphDisposition::Unresolved
    } else if !excluded_edges.is_empty()
        || scores.iter().any(|score| {
            score
                .uncertainty
                .iter()
                .any(|item| item == "node-is-disconnected")
        })
        || scores
            .iter()
            .any(|score| score.direct_evidence_state == "contested")
    {
        MechanismGraphDisposition::Partial
    } else {
        MechanismGraphDisposition::Qualified
    };
    let mut output = MechanismGraphPropagation {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        model_system: request.model_system,
        node_order,
        edge_order: edge_ids.into_iter().collect(),
        active_edge_order: active_edges
            .iter()
            .map(|edge| edge.edge_id.clone())
            .collect(),
        excluded_edge_order: excluded_edges,
        ranking_order,
        scores,
        converged,
        iterations_run,
        max_delta_milli: max_delta.min(2_000),
        negative_evidence: negative.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        disposition,
        digest: ContentHash::of_bytes(b"unsealed-glioma-mechanism-graph"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| MechanismGraphError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nodes() -> Vec<MechanismGraphNode> {
        vec![
            MechanismGraphNode {
                node_id: "egfr".into(),
                label: "EGFR activation".into(),
                modality: GliomaModality::Genomics,
                prior_milli: 100,
                support_milli: 800,
                contradiction_milli: 0,
            },
            MechanismGraphNode {
                node_id: "stat3".into(),
                label: "STAT3 state".into(),
                modality: GliomaModality::Transcriptomics,
                prior_milli: 0,
                support_milli: 400,
                contradiction_milli: 0,
            },
            MechanismGraphNode {
                node_id: "invasion".into(),
                label: "invasion phenotype".into(),
                modality: GliomaModality::FunctionalPerturbation,
                prior_milli: 0,
                support_milli: 0,
                contradiction_milli: 0,
            },
        ]
    }

    fn edges() -> Vec<MechanismGraphEdge> {
        vec![
            MechanismGraphEdge {
                edge_id: "e-egfr-stat3".into(),
                source_node_id: "egfr".into(),
                target_node_id: "stat3".into(),
                relation: MechanismGraphRelation::Activates,
                confidence_milli: 900,
                evidence_order: vec!["paper-1".into()],
            },
            MechanismGraphEdge {
                edge_id: "e-stat3-invasion".into(),
                source_node_id: "stat3".into(),
                target_node_id: "invasion".into(),
                relation: MechanismGraphRelation::Activates,
                confidence_milli: 900,
                evidence_order: vec!["paper-2".into()],
            },
        ]
    }

    fn request() -> MechanismGraphRequest {
        MechanismGraphRequest {
            objective: "rank invasion mechanism network".into(),
            model_system: GliomaModelSystem::Organoid,
            max_iterations: 100,
            convergence_tolerance_milli: 1,
            damping_milli: 600,
            min_edge_confidence_milli: 500,
            top_k: 3,
        }
    }

    #[test]
    fn activating_network_converges_and_ranks_nodes() {
        let output = propagate_glioma_mechanism_graph(&request(), &nodes(), &edges()).unwrap();
        assert!(output.converged);
        assert_eq!(output.disposition, MechanismGraphDisposition::Qualified);
        assert_eq!(output.ranking_order.len(), 3);
        assert!(output
            .scores
            .iter()
            .any(|score| { score.node_id == "invasion" && score.propagated_score_milli > 0 }));
        output.validate().unwrap();
    }

    #[test]
    fn inhibition_reverses_downstream_network_signal() {
        let mut inhibited = edges();
        inhibited[1].relation = MechanismGraphRelation::Inhibits;
        let output = propagate_glioma_mechanism_graph(&request(), &nodes(), &inhibited).unwrap();
        let invasion = output
            .scores
            .iter()
            .find(|score| score.node_id == "invasion")
            .unwrap();
        assert!(invasion.propagated_score_milli < 0);
    }

    #[test]
    fn low_confidence_and_disconnected_nodes_are_partial_or_unresolved() {
        let mut edges = edges();
        edges[0].confidence_milli = 100;
        let mut nodes = nodes();
        nodes.push(MechanismGraphNode {
            node_id: "unmeasured".into(),
            label: "unmeasured state".into(),
            modality: GliomaModality::Proteomics,
            prior_milli: 0,
            support_milli: 0,
            contradiction_milli: 0,
        });
        let output = propagate_glioma_mechanism_graph(&request(), &nodes, &edges).unwrap();
        assert_eq!(output.disposition, MechanismGraphDisposition::Partial);
        assert!(output
            .uncertainty
            .iter()
            .any(|item| item == "low-confidence-edge-excluded:e-egfr-stat3"));
        assert!(output
            .uncertainty
            .iter()
            .any(|item| item == "disconnected-node:unmeasured"));
    }

    #[test]
    fn replay_is_stable_under_input_permutation() {
        let first = propagate_glioma_mechanism_graph(&request(), &nodes(), &edges()).unwrap();
        let mut reversed_nodes = nodes();
        reversed_nodes.reverse();
        let mut reversed_edges = edges();
        reversed_edges.reverse();
        let second =
            propagate_glioma_mechanism_graph(&request(), &reversed_nodes, &reversed_edges).unwrap();
        assert_eq!(first, second);
    }
}
