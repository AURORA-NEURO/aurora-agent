//! Counterfactual mechanism perturbation for preclinical glioma research.
//!
//! This feature simulates signed node interventions over a typed mechanism graph and compares
//! the resulting fixed point with an unperturbed baseline. It is useful for prioritising assays
//! that could distinguish invasion, proliferation, stemness, or microenvironment mechanisms, but
//! it is not a causal estimate: interventions are mathematical perturbations until an admitted
//! experiment supplies observations.

use super::graph_propagation::{MechanismGraphEdge, MechanismGraphNode, MechanismGraphRelation};
use crate::glioma_engine::GliomaModelSystem;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P05-F18";
pub const OUTPUT_SCHEMA: &str = "GliomaMechanismCounterfactual1@1";
pub const MAX_INTERVENTIONS: usize = 128;
pub const MAX_NODES: usize = 4_096;
pub const MAX_EDGES: usize = 32_768;
pub const MAX_ITERATIONS: u16 = 512;
pub const SCORE_BOUND: i64 = 1_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CounterfactualRequest {
    pub objective: String,
    pub model_system: GliomaModelSystem,
    pub max_iterations: u16,
    pub convergence_tolerance_milli: u16,
    pub damping_milli: u16,
    pub min_edge_confidence_milli: u16,
    pub min_effect_milli: u64,
    pub top_k: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CounterfactualIntervention {
    pub intervention_id: String,
    pub node_id: String,
    pub delta_milli: i16,
    pub rationale: String,
    pub evidence_order: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CounterfactualDirection {
    Increases,
    Decreases,
    NoMaterialChange,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CounterfactualContrast {
    pub node_id: String,
    pub baseline_milli: i64,
    pub intervention_milli: i64,
    pub effect_milli: i64,
    pub absolute_change_milli: u64,
    pub direction: CounterfactualDirection,
    pub uncertainty: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CounterfactualDisposition {
    Qualified,
    Partial,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismCounterfactual {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub model_system: GliomaModelSystem,
    pub node_order: Vec<String>,
    pub intervention_order: Vec<String>,
    pub active_edge_order: Vec<String>,
    pub excluded_edge_order: Vec<String>,
    pub target_order: Vec<String>,
    pub interventions: Vec<CounterfactualIntervention>,
    pub contrasts: Vec<CounterfactualContrast>,
    pub baseline_converged: bool,
    pub intervention_converged: bool,
    pub baseline_iterations: u16,
    pub intervention_iterations: u16,
    pub baseline_max_delta_milli: u64,
    pub intervention_max_delta_milli: u64,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub disposition: CounterfactualDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CounterfactualError {
    #[error("mechanism counterfactual request is invalid: {0}")]
    InvalidRequest(String),
    #[error("mechanism counterfactual input is invalid: {0}")]
    InvalidInput(String),
    #[error("mechanism counterfactual output is invalid: {0}")]
    InvalidOutput(String),
    #[error("mechanism counterfactual digest failed: {0}")]
    Digest(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(output: &MechanismCounterfactual) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "model_system": output.model_system,
        "node_order": output.node_order,
        "intervention_order": output.intervention_order,
        "active_edge_order": output.active_edge_order,
        "excluded_edge_order": output.excluded_edge_order,
        "target_order": output.target_order,
        "interventions": output.interventions,
        "contrasts": output.contrasts,
        "baseline_converged": output.baseline_converged,
        "intervention_converged": output.intervention_converged,
        "baseline_iterations": output.baseline_iterations,
        "intervention_iterations": output.intervention_iterations,
        "baseline_max_delta_milli": output.baseline_max_delta_milli,
        "intervention_max_delta_milli": output.intervention_max_delta_milli,
        "uncertainty": output.uncertainty,
        "negative_evidence": output.negative_evidence,
        "disposition": output.disposition,
    })
}

impl MechanismCounterfactual {
    pub fn validate(&self) -> Result<(), CounterfactualError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.node_order.is_empty()
            || self.intervention_order.is_empty()
            || self.interventions.len() != self.intervention_order.len()
            || self.contrasts.len() != self.node_order.len()
            || self.node_order.windows(2).any(|pair| pair[0] == pair[1])
            || !canonical(&self.intervention_order)
            || !canonical(&self.active_edge_order)
            || !canonical(&self.excluded_edge_order)
            || !canonical(&self.target_order)
            || !canonical(&self.uncertainty)
            || !canonical(&self.negative_evidence)
            || self.interventions.iter().any(|intervention| {
                intervention.intervention_id.trim().is_empty()
                    || intervention.node_id.trim().is_empty()
                    || intervention.delta_milli == 0
                    || intervention.rationale.trim().is_empty()
                    || !canonical(&intervention.evidence_order)
            })
            || self.contrasts.iter().any(|contrast| {
                contrast.node_id.trim().is_empty()
                    || !canonical(&contrast.uncertainty)
                    || contrast
                        .uncertainty
                        .iter()
                        .any(|item| item.trim().is_empty())
            })
            || self.uncertainty.iter().any(|item| item.trim().is_empty())
            || self
                .negative_evidence
                .iter()
                .any(|item| item.trim().is_empty())
        {
            return Err(CounterfactualError::InvalidOutput(
                "identity, interventions, contrasts, ordering, or limitations are invalid".into(),
            ));
        }
        let nodes = self.node_order.iter().cloned().collect::<BTreeSet<_>>();
        let contrast_nodes = self
            .contrasts
            .iter()
            .map(|contrast| contrast.node_id.clone())
            .collect::<BTreeSet<_>>();
        if nodes != contrast_nodes {
            return Err(CounterfactualError::InvalidOutput(
                "node and contrast identities do not reconcile".into(),
            ));
        }
        let intervention_ids = self
            .interventions
            .iter()
            .map(|intervention| intervention.intervention_id.clone())
            .collect::<BTreeSet<_>>();
        if intervention_ids != self.intervention_order.iter().cloned().collect() {
            return Err(CounterfactualError::InvalidOutput(
                "intervention order does not reconcile".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| CounterfactualError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(CounterfactualError::InvalidOutput(
                "counterfactual digest is not bound to the simulation".into(),
            ));
        }
        Ok(())
    }
}

fn score(node: &MechanismGraphNode) -> i64 {
    (i64::from(node.prior_milli) + i64::from(node.support_milli)
        - i64::from(node.contradiction_milli))
    .clamp(-SCORE_BOUND, SCORE_BOUND)
}

fn propagate(
    nodes: &[MechanismGraphNode],
    edges: &[MechanismGraphEdge],
    request: &CounterfactualRequest,
    intervention_delta: &BTreeMap<String, i64>,
) -> (BTreeMap<String, i64>, bool, u16, u64) {
    let base = nodes
        .iter()
        .map(|node| {
            let delta = intervention_delta.get(&node.node_id).copied().unwrap_or(0);
            (
                node.node_id.clone(),
                (score(node) + delta).clamp(-SCORE_BOUND, SCORE_BOUND),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut values = base.clone();
    let mut converged = false;
    let mut iterations = 0;
    let mut max_delta = 0_u64;
    for iteration in 1..=request.max_iterations {
        let mut incoming = BTreeMap::<String, i64>::new();
        for edge in edges {
            let source = values.get(&edge.source_node_id).copied().unwrap_or(0);
            let sign = match edge.relation {
                MechanismGraphRelation::Activates => 1_i64,
                MechanismGraphRelation::Inhibits => -1_i64,
            };
            let contribution =
                (i128::from(source) * i128::from(edge.confidence_milli) * i128::from(sign))
                    .checked_div(1_000)
                    .unwrap_or(0)
                    .clamp(i128::from(-SCORE_BOUND), i128::from(SCORE_BOUND))
                    as i64;
            let entry = incoming.entry(edge.target_node_id.clone()).or_default();
            *entry = entry
                .saturating_add(contribution)
                .clamp(-SCORE_BOUND, SCORE_BOUND);
        }
        let mut next = BTreeMap::new();
        let mut delta = 0_u64;
        for node in nodes {
            let base_score = base.get(&node.node_id).copied().unwrap_or(0);
            let network_score = incoming.get(&node.node_id).copied().unwrap_or(0);
            let value = (i128::from(base_score) * i128::from(request.damping_milli)
                + i128::from(network_score)
                    * i128::from(1_000_u16.saturating_sub(request.damping_milli)))
            .checked_div(1_000)
            .unwrap_or(0)
            .clamp(i128::from(-SCORE_BOUND), i128::from(SCORE_BOUND))
                as i64;
            let old = values.get(&node.node_id).copied().unwrap_or(0);
            delta = delta.max(old.saturating_sub(value).unsigned_abs());
            next.insert(node.node_id.clone(), value);
        }
        values = next;
        iterations = iteration;
        max_delta = delta;
        if delta <= u64::from(request.convergence_tolerance_milli) {
            converged = true;
            break;
        }
    }
    (values, converged, iterations, max_delta)
}

fn validate_request(
    request: &CounterfactualRequest,
    nodes: &[MechanismGraphNode],
    edges: &[MechanismGraphEdge],
    interventions: &[CounterfactualIntervention],
) -> Result<(), CounterfactualError> {
    if request.objective.trim().is_empty()
        || request.max_iterations == 0
        || request.max_iterations > MAX_ITERATIONS
        || request.convergence_tolerance_milli > 1_000
        || request.damping_milli > 1_000
        || request.min_edge_confidence_milli > 1_000
        || request.top_k == 0
        || request.top_k > MAX_NODES
        || nodes.is_empty()
        || nodes.len() > MAX_NODES
        || edges.len() > MAX_EDGES
        || interventions.is_empty()
        || interventions.len() > MAX_INTERVENTIONS
    {
        return Err(CounterfactualError::InvalidRequest(
            "objective, bounded graph, intervention set, convergence, confidence, and top-k parameters are required".into(),
        ));
    }
    let mut node_ids = BTreeSet::new();
    for node in nodes {
        if node.node_id.trim().is_empty()
            || !node_ids.insert(node.node_id.clone())
            || node.support_milli > 1_000
            || node.contradiction_milli > 1_000
        {
            return Err(CounterfactualError::InvalidInput(
                "node identity, uniqueness, or score bounds are invalid".into(),
            ));
        }
    }
    let mut edge_ids = BTreeSet::new();
    for edge in edges {
        if edge.edge_id.trim().is_empty()
            || !edge_ids.insert(edge.edge_id.clone())
            || !node_ids.contains(&edge.source_node_id)
            || !node_ids.contains(&edge.target_node_id)
            || edge.confidence_milli > 1_000
            || edge.evidence_order.is_empty()
            || !canonical(&edge.evidence_order)
        {
            return Err(CounterfactualError::InvalidInput(
                "edge identity, endpoint, confidence, or evidence ordering is invalid".into(),
            ));
        }
    }
    let mut intervention_ids = BTreeSet::new();
    for intervention in interventions {
        if intervention.intervention_id.trim().is_empty()
            || !intervention_ids.insert(intervention.intervention_id.clone())
            || !node_ids.contains(&intervention.node_id)
            || intervention.delta_milli == 0
            || u64::from(intervention.delta_milli.unsigned_abs()) > SCORE_BOUND as u64
            || intervention.rationale.trim().is_empty()
            || !canonical(&intervention.evidence_order)
        {
            return Err(CounterfactualError::InvalidInput(
                "intervention identity, node, delta, rationale, or evidence ordering is invalid"
                    .into(),
            ));
        }
    }
    Ok(())
}

/// Simulate signed mechanism interventions and return baseline-versus-intervention contrasts.
pub fn simulate_glioma_counterfactual(
    request: &CounterfactualRequest,
    nodes: &[MechanismGraphNode],
    edges: &[MechanismGraphEdge],
    interventions: &[CounterfactualIntervention],
) -> Result<MechanismCounterfactual, CounterfactualError> {
    validate_request(request, nodes, edges, interventions)?;
    let mut ordered_nodes = nodes.to_vec();
    ordered_nodes.sort_by(|left, right| left.node_id.cmp(&right.node_id));
    let node_order = ordered_nodes
        .iter()
        .map(|node| node.node_id.clone())
        .collect::<Vec<_>>();
    let mut ordered_edges = edges.to_vec();
    ordered_edges.sort_by(|left, right| left.edge_id.cmp(&right.edge_id));
    let active = ordered_edges
        .iter()
        .filter(|edge| edge.confidence_milli >= request.min_edge_confidence_milli)
        .cloned()
        .collect::<Vec<_>>();
    let active_edge_order = active
        .iter()
        .map(|edge| edge.edge_id.clone())
        .collect::<Vec<_>>();
    let excluded_edge_order = ordered_edges
        .iter()
        .filter(|edge| edge.confidence_milli < request.min_edge_confidence_milli)
        .map(|edge| edge.edge_id.clone())
        .collect::<Vec<_>>();
    let mut ordered_interventions = interventions.to_vec();
    ordered_interventions.sort_by(|left, right| left.intervention_id.cmp(&right.intervention_id));
    let intervention_order = ordered_interventions
        .iter()
        .map(|intervention| intervention.intervention_id.clone())
        .collect::<Vec<_>>();
    let intervention_delta = ordered_interventions.iter().fold(
        BTreeMap::<String, i64>::new(),
        |mut map, intervention| {
            let entry = map.entry(intervention.node_id.clone()).or_default();
            *entry = entry.saturating_add(i64::from(intervention.delta_milli));
            map
        },
    );
    let (baseline, baseline_converged, baseline_iterations, baseline_max_delta_milli) =
        propagate(&ordered_nodes, &active, request, &BTreeMap::new());
    let (
        intervention,
        intervention_converged,
        intervention_iterations,
        intervention_max_delta_milli,
    ) = propagate(&ordered_nodes, &active, request, &intervention_delta);
    let mut uncertainty = BTreeSet::new();
    let mut negative_evidence = BTreeSet::new();
    if !active_edge_order.is_empty() && !baseline_converged {
        uncertainty.insert("baseline-fixed-point-did-not-converge".into());
    }
    if !active_edge_order.is_empty() && !intervention_converged {
        uncertainty.insert("intervention-fixed-point-did-not-converge".into());
    }
    if active_edge_order.is_empty() {
        uncertainty.insert("no-edge-met-confidence-floor".into());
    }
    if !excluded_edge_order.is_empty() {
        negative_evidence.insert("low-confidence-edges-excluded".into());
    }
    let mut contrasts = ordered_nodes
        .iter()
        .map(|node| {
            let base = baseline.get(&node.node_id).copied().unwrap_or(0);
            let intervened = intervention.get(&node.node_id).copied().unwrap_or(0);
            let effect = intervened.saturating_sub(base);
            let mut local_uncertainty = BTreeSet::new();
            if !baseline_converged || !intervention_converged {
                local_uncertainty.insert("fixed-point-non-convergence".into());
            }
            if excluded_edge_order.iter().any(|edge_id| {
                ordered_edges
                    .iter()
                    .find(|edge| &edge.edge_id == edge_id)
                    .is_some_and(|edge| edge.target_node_id == node.node_id)
            }) {
                local_uncertainty.insert("target-has-excluded-incoming-edge".into());
            }
            let direction = if !local_uncertainty.is_empty() {
                CounterfactualDirection::Unresolved
            } else if effect.unsigned_abs() < request.min_effect_milli {
                CounterfactualDirection::NoMaterialChange
            } else if effect > 0 {
                CounterfactualDirection::Increases
            } else {
                CounterfactualDirection::Decreases
            };
            CounterfactualContrast {
                node_id: node.node_id.clone(),
                baseline_milli: base,
                intervention_milli: intervened,
                effect_milli: effect,
                absolute_change_milli: effect.unsigned_abs(),
                direction,
                uncertainty: local_uncertainty.into_iter().collect(),
            }
        })
        .collect::<Vec<_>>();
    contrasts.sort_by(|left, right| {
        right
            .absolute_change_milli
            .cmp(&left.absolute_change_milli)
            .then_with(|| left.node_id.cmp(&right.node_id))
    });
    let target_order = contrasts
        .iter()
        .take(request.top_k.min(contrasts.len()))
        .map(|contrast| contrast.node_id.clone())
        .collect::<Vec<_>>();
    contrasts.sort_by(|left, right| left.node_id.cmp(&right.node_id));
    let disposition = if !baseline_converged || !intervention_converged {
        CounterfactualDisposition::Unresolved
    } else if active_edge_order.is_empty() || !excluded_edge_order.is_empty() {
        CounterfactualDisposition::Partial
    } else {
        CounterfactualDisposition::Qualified
    };
    let mut output = MechanismCounterfactual {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        model_system: request.model_system,
        node_order,
        intervention_order,
        active_edge_order,
        excluded_edge_order,
        target_order,
        interventions: ordered_interventions,
        contrasts,
        baseline_converged,
        intervention_converged,
        baseline_iterations,
        intervention_iterations,
        baseline_max_delta_milli,
        intervention_max_delta_milli,
        uncertainty: uncertainty.into_iter().collect(),
        negative_evidence: negative_evidence.into_iter().collect(),
        disposition,
        digest: ContentHash::of_bytes(b"unsealed-glioma-counterfactual"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| CounterfactualError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::programs::p05_mechanism_exploration::graph_propagation::{
        MechanismGraphNode, MechanismGraphRelation,
    };
    use crate::glioma_engine::GliomaModality;

    fn node(id: &str, support: u16) -> MechanismGraphNode {
        MechanismGraphNode {
            node_id: id.into(),
            label: id.into(),
            modality: GliomaModality::Genomics,
            prior_milli: 0,
            support_milli: support,
            contradiction_milli: 0,
        }
    }

    fn edge(
        id: &str,
        source: &str,
        target: &str,
        relation: MechanismGraphRelation,
    ) -> MechanismGraphEdge {
        MechanismGraphEdge {
            edge_id: id.into(),
            source_node_id: source.into(),
            target_node_id: target.into(),
            relation,
            confidence_milli: 900,
            evidence_order: vec![format!("evidence-{id}")],
        }
    }

    fn request() -> CounterfactualRequest {
        CounterfactualRequest {
            objective: "simulate EGFR inhibition on invasion mechanism".into(),
            model_system: GliomaModelSystem::Organoid,
            max_iterations: 100,
            convergence_tolerance_milli: 1,
            damping_milli: 600,
            min_edge_confidence_milli: 500,
            min_effect_milli: 10,
            top_k: 3,
        }
    }

    fn intervention(delta_milli: i16) -> CounterfactualIntervention {
        CounterfactualIntervention {
            intervention_id: "inhibit-egfr".into(),
            node_id: "egfr".into(),
            delta_milli,
            rationale: "test whether EGFR support propagates to invasion".into(),
            evidence_order: vec!["paper-egfr".into()],
        }
    }

    #[test]
    fn inhibition_reduces_downstream_network_signal() {
        let nodes = vec![node("egfr", 800), node("invasion", 0)];
        let edges = vec![edge(
            "egfr-invasion",
            "egfr",
            "invasion",
            MechanismGraphRelation::Activates,
        )];
        let output =
            simulate_glioma_counterfactual(&request(), &nodes, &edges, &[intervention(-600)])
                .unwrap();
        let invasion = output
            .contrasts
            .iter()
            .find(|contrast| contrast.node_id == "invasion")
            .unwrap();
        assert!(invasion.effect_milli < 0);
        assert_eq!(output.disposition, CounterfactualDisposition::Qualified);
        output.validate().unwrap();
    }

    #[test]
    fn input_permutation_replays_identically() {
        let nodes = vec![node("egfr", 800), node("invasion", 0)];
        let edges = vec![edge(
            "egfr-invasion",
            "egfr",
            "invasion",
            MechanismGraphRelation::Activates,
        )];
        let first =
            simulate_glioma_counterfactual(&request(), &nodes, &edges, &[intervention(200)])
                .unwrap();
        let second = simulate_glioma_counterfactual(
            &request(),
            &nodes.into_iter().rev().collect::<Vec<_>>(),
            &edges.into_iter().rev().collect::<Vec<_>>(),
            &[intervention(200)],
        )
        .unwrap();
        assert_eq!(first, second);
    }

    #[test]
    fn low_iteration_budget_is_unresolved_not_a_false_claim() {
        let mut request = request();
        request.max_iterations = 1;
        let nodes = vec![node("egfr", 800), node("invasion", 0)];
        let edges = vec![edge(
            "egfr-invasion",
            "egfr",
            "invasion",
            MechanismGraphRelation::Activates,
        )];
        let output =
            simulate_glioma_counterfactual(&request, &nodes, &edges, &[intervention(-600)])
                .unwrap();
        assert_eq!(output.disposition, CounterfactualDisposition::Unresolved);
        assert!(output
            .uncertainty
            .iter()
            .any(|item| item.contains("did-not-converge")));
    }
}
