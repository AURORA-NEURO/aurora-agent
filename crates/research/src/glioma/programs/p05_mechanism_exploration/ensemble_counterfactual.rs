//! Model-averaged mechanism counterfactuals for preclinical glioma research.
//!
//! A single mechanistic graph can make a perturbation look decisive merely because its edge
//! weights are optimistic. This feature runs the same signed intervention against a bounded
//! ensemble of independently declared graphs, averages effects by explicit model priors, and
//! exposes the disagreement envelope. It is an assay-prioritisation product, not a causal or
//! clinical claim: model agreement is a reproducibility gate, not proof that an intervention will
//! work in a biological system.

use super::counterfactual::{
    simulate_glioma_counterfactual, CounterfactualDisposition, CounterfactualIntervention,
    CounterfactualRequest, MechanismCounterfactual,
};
use super::graph_propagation::{MechanismGraphEdge, MechanismGraphNode};
use crate::glioma_engine::GliomaModelSystem;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P05-F19";
pub const OUTPUT_SCHEMA: &str = "GliomaMechanismEnsembleCounterfactual1@1";
pub const MAX_MODELS: usize = 64;
pub const MAX_PRIOR_MILLI: u16 = 1_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CounterfactualEnsembleRequest {
    pub objective: String,
    pub model_system: GliomaModelSystem,
    pub max_iterations: u16,
    pub convergence_tolerance_milli: u16,
    pub damping_milli: u16,
    pub min_edge_confidence_milli: u16,
    pub min_effect_milli: u64,
    pub min_model_agreement_milli: u16,
    pub top_k: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CounterfactualModel {
    pub model_id: String,
    pub prior_milli: u16,
    pub nodes: Vec<MechanismGraphNode>,
    pub edges: Vec<MechanismGraphEdge>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnsembleModelResult {
    pub model_id: String,
    pub prior_milli: u16,
    pub simulation: MechanismCounterfactual,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EnsembleDirection {
    Increases,
    Decreases,
    NoMaterialChange,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnsembleTargetSummary {
    pub node_id: String,
    pub weighted_effect_milli: i64,
    pub min_effect_milli: i64,
    pub max_effect_milli: i64,
    pub absolute_change_milli: u64,
    pub agreement_milli: u16,
    pub model_count: u16,
    pub model_disagreement_milli: u64,
    pub direction: EnsembleDirection,
    pub uncertainty: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EnsembleDisposition {
    Qualified,
    Partial,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismCounterfactualEnsemble {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub model_system: GliomaModelSystem,
    pub model_order: Vec<String>,
    pub node_order: Vec<String>,
    pub intervention_order: Vec<String>,
    pub models: Vec<EnsembleModelResult>,
    pub target_order: Vec<String>,
    pub targets: Vec<EnsembleTargetSummary>,
    pub total_prior_milli: u64,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub disposition: EnsembleDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum EnsembleCounterfactualError {
    #[error("counterfactual ensemble request is invalid: {0}")]
    InvalidRequest(String),
    #[error("counterfactual ensemble model is invalid: {0}")]
    InvalidModel(String),
    #[error("counterfactual ensemble output is invalid: {0}")]
    InvalidOutput(String),
    #[error("counterfactual ensemble digest failed: {0}")]
    Digest(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(output: &MechanismCounterfactualEnsemble) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "model_system": output.model_system,
        "model_order": output.model_order,
        "node_order": output.node_order,
        "intervention_order": output.intervention_order,
        "models": output.models,
        "target_order": output.target_order,
        "targets": output.targets,
        "total_prior_milli": output.total_prior_milli,
        "uncertainty": output.uncertainty,
        "negative_evidence": output.negative_evidence,
        "disposition": output.disposition,
    })
}

impl MechanismCounterfactualEnsemble {
    pub fn validate(&self) -> Result<(), EnsembleCounterfactualError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.model_order.is_empty()
            || self.node_order.is_empty()
            || self.intervention_order.is_empty()
            || !canonical(&self.model_order)
            || !canonical(&self.node_order)
            || !canonical(&self.intervention_order)
            || !canonical(&self.target_order)
            || !canonical(&self.uncertainty)
            || !canonical(&self.negative_evidence)
            || self.models.len() != self.model_order.len()
            || self.targets.len() != self.node_order.len()
            || self.total_prior_milli == 0
            || self.models.iter().any(|model| {
                model.model_id.trim().is_empty()
                    || model.prior_milli == 0
                    || model.prior_milli > MAX_PRIOR_MILLI
                    || model.simulation.model_system != self.model_system
                    || model.simulation.intervention_order != self.intervention_order
                    || model.simulation.node_order != self.node_order
                    || model.simulation.validate().is_err()
            })
            || self
                .targets
                .windows(2)
                .any(|pair| pair[0].node_id >= pair[1].node_id)
            || self.targets.iter().any(|target| {
                target.node_id.trim().is_empty()
                    || target.agreement_milli > 1_000
                    || target.model_count != self.models.len() as u16
                    || target.min_effect_milli > target.max_effect_milli
                    || target.absolute_change_milli != target.weighted_effect_milli.unsigned_abs()
                    || !canonical(&target.uncertainty)
            })
            || self.uncertainty.iter().any(|item| item.trim().is_empty())
            || self
                .negative_evidence
                .iter()
                .any(|item| item.trim().is_empty())
        {
            return Err(EnsembleCounterfactualError::InvalidOutput(
                "identity, model/node ordering, simulation contracts, bounds, or summaries are invalid".into(),
            ));
        }
        let model_ids = self
            .models
            .iter()
            .map(|model| model.model_id.clone())
            .collect::<BTreeSet<_>>();
        if model_ids != self.model_order.iter().cloned().collect::<BTreeSet<_>>()
            || self
                .models
                .iter()
                .map(|model| model.prior_milli as u64)
                .sum::<u64>()
                != self.total_prior_milli
        {
            return Err(EnsembleCounterfactualError::InvalidOutput(
                "model identities or prior mass do not reconcile".into(),
            ));
        }
        let expected_nodes = self.node_order.iter().cloned().collect::<BTreeSet<_>>();
        let target_nodes = self
            .targets
            .iter()
            .map(|target| target.node_id.clone())
            .collect::<BTreeSet<_>>();
        if expected_nodes != target_nodes {
            return Err(EnsembleCounterfactualError::InvalidOutput(
                "target summaries do not cover the model node set".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| EnsembleCounterfactualError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(EnsembleCounterfactualError::InvalidOutput(
                "ensemble digest is not bound to the simulations".into(),
            ));
        }
        Ok(())
    }
}

fn direction(effect_milli: i64, min_effect_milli: u64) -> EnsembleDirection {
    if effect_milli.unsigned_abs() < min_effect_milli {
        EnsembleDirection::NoMaterialChange
    } else if effect_milli > 0 {
        EnsembleDirection::Increases
    } else if effect_milli < 0 {
        EnsembleDirection::Decreases
    } else {
        EnsembleDirection::NoMaterialChange
    }
}

fn sign(direction: EnsembleDirection) -> i8 {
    match direction {
        EnsembleDirection::Increases => 1,
        EnsembleDirection::Decreases => -1,
        EnsembleDirection::NoMaterialChange => 0,
        EnsembleDirection::Unresolved => 0,
    }
}

fn validate_request(
    request: &CounterfactualEnsembleRequest,
    models: &[CounterfactualModel],
    interventions: &[CounterfactualIntervention],
) -> Result<(), EnsembleCounterfactualError> {
    if request.objective.trim().is_empty()
        || request.max_iterations == 0
        || request.convergence_tolerance_milli > 1_000
        || request.damping_milli > 1_000
        || request.min_edge_confidence_milli > 1_000
        || request.min_model_agreement_milli > 1_000
        || request.top_k == 0
        || request.top_k > super::counterfactual::MAX_NODES
        || models.is_empty()
        || models.len() > MAX_MODELS
        || interventions.is_empty()
    {
        return Err(EnsembleCounterfactualError::InvalidRequest(
            "objective, bounded model ensemble, convergence, agreement, top-k, and intervention parameters are required".into(),
        ));
    }
    let mut model_ids = BTreeSet::new();
    let mut expected_nodes: Option<BTreeSet<String>> = None;
    for model in models {
        if model.model_id.trim().is_empty()
            || !model_ids.insert(model.model_id.clone())
            || model.prior_milli == 0
            || model.prior_milli > MAX_PRIOR_MILLI
            || model.nodes.is_empty()
        {
            return Err(EnsembleCounterfactualError::InvalidModel(
                "model identity, positive prior, and non-empty node graph are required".into(),
            ));
        }
        let nodes = model
            .nodes
            .iter()
            .map(|node| node.node_id.clone())
            .collect::<BTreeSet<_>>();
        if nodes.len() != model.nodes.len() {
            return Err(EnsembleCounterfactualError::InvalidModel(
                "model node identifiers must be unique".into(),
            ));
        }
        if let Some(expected) = &expected_nodes {
            if expected != &nodes {
                return Err(EnsembleCounterfactualError::InvalidModel(
                    "all models must declare the same node state space".into(),
                ));
            }
        } else {
            expected_nodes = Some(nodes);
        }
    }
    Ok(())
}

/// Run one signed intervention set across multiple mechanistic models and expose robust targets.
pub fn simulate_glioma_counterfactual_ensemble(
    request: &CounterfactualEnsembleRequest,
    models: &[CounterfactualModel],
    interventions: &[CounterfactualIntervention],
) -> Result<MechanismCounterfactualEnsemble, EnsembleCounterfactualError> {
    validate_request(request, models, interventions)?;
    let mut ordered_models = models.to_vec();
    ordered_models.sort_by(|left, right| left.model_id.cmp(&right.model_id));
    let model_order = ordered_models
        .iter()
        .map(|model| model.model_id.clone())
        .collect::<Vec<_>>();
    let total_prior_milli = ordered_models
        .iter()
        .map(|model| u64::from(model.prior_milli))
        .sum::<u64>();
    let mut simulations = Vec::with_capacity(ordered_models.len());
    for model in &ordered_models {
        let simulation_request = CounterfactualRequest {
            objective: request.objective.clone(),
            model_system: request.model_system,
            max_iterations: request.max_iterations,
            convergence_tolerance_milli: request.convergence_tolerance_milli,
            damping_milli: request.damping_milli,
            min_edge_confidence_milli: request.min_edge_confidence_milli,
            min_effect_milli: request.min_effect_milli,
            top_k: request.top_k,
        };
        let simulation = simulate_glioma_counterfactual(
            &simulation_request,
            &model.nodes,
            &model.edges,
            interventions,
        )
        .map_err(|error| EnsembleCounterfactualError::InvalidModel(error.to_string()))?;
        simulations.push(EnsembleModelResult {
            model_id: model.model_id.clone(),
            prior_milli: model.prior_milli,
            simulation,
        });
    }
    let node_order = simulations[0].simulation.node_order.clone();
    let intervention_order = simulations[0].simulation.intervention_order.clone();
    let mut targets = Vec::with_capacity(node_order.len());
    let mut ensemble_uncertainty = BTreeSet::new();
    let mut negative_evidence = BTreeSet::new();
    for model in &simulations {
        for item in &model.simulation.uncertainty {
            ensemble_uncertainty.insert(format!("{}:{item}", model.model_id));
        }
        for item in &model.simulation.negative_evidence {
            negative_evidence.insert(format!("{}:{item}", model.model_id));
        }
    }
    for node_id in &node_order {
        let effects = simulations
            .iter()
            .map(|model| {
                model
                    .simulation
                    .contrasts
                    .iter()
                    .find(|contrast| &contrast.node_id == node_id)
                    .map(|contrast| (model.prior_milli, contrast.effect_milli, contrast.direction))
                    .ok_or_else(|| {
                        EnsembleCounterfactualError::InvalidModel(format!(
                            "model {} omitted node {}",
                            model.model_id, node_id
                        ))
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let weighted_numerator = effects.iter().fold(0_i128, |sum, (prior, effect, _)| {
            sum.saturating_add(i128::from(*effect) * i128::from(*prior))
        });
        let weighted_effect_milli = (weighted_numerator / i128::from(total_prior_milli)) as i64;
        let min_effect_milli = effects
            .iter()
            .map(|(_, effect, _)| *effect)
            .min()
            .unwrap_or(0);
        let max_effect_milli = effects
            .iter()
            .map(|(_, effect, _)| *effect)
            .max()
            .unwrap_or(0);
        let model_direction = direction(weighted_effect_milli, request.min_effect_milli);
        let weighted_sign = sign(model_direction);
        let agreeing_prior = effects
            .iter()
            .map(|(prior, effect, _)| {
                if sign(direction(*effect, request.min_effect_milli)) == weighted_sign {
                    u64::from(*prior)
                } else {
                    0
                }
            })
            .sum::<u64>();
        let agreement_milli = agreeing_prior
            .saturating_mul(1_000)
            .checked_div(total_prior_milli)
            .unwrap_or(0)
            .min(1_000) as u16;
        let mut uncertainty = BTreeSet::new();
        if agreement_milli < request.min_model_agreement_milli {
            uncertainty.insert("model-direction-disagreement-below-floor".into());
            ensemble_uncertainty.insert(format!("target:{node_id}:model-direction-disagreement"));
        }
        if simulations
            .iter()
            .any(|model| model.simulation.disposition != CounterfactualDisposition::Qualified)
        {
            uncertainty.insert("one-or-more-models-not-qualified".into());
        }
        let model_disagreement_milli = max_effect_milli
            .saturating_sub(min_effect_milli)
            .unsigned_abs();
        let direction = if !uncertainty.is_empty() {
            EnsembleDirection::Unresolved
        } else {
            model_direction
        };
        targets.push(EnsembleTargetSummary {
            node_id: node_id.clone(),
            weighted_effect_milli,
            min_effect_milli,
            max_effect_milli,
            absolute_change_milli: weighted_effect_milli.unsigned_abs(),
            agreement_milli,
            model_count: simulations.len() as u16,
            model_disagreement_milli,
            direction,
            uncertainty: uncertainty.into_iter().collect(),
        });
    }
    let mut ranked = targets.clone();
    ranked.sort_by(|left, right| {
        right
            .absolute_change_milli
            .cmp(&left.absolute_change_milli)
            .then_with(|| right.agreement_milli.cmp(&left.agreement_milli))
            .then_with(|| left.node_id.cmp(&right.node_id))
    });
    let target_order = ranked
        .iter()
        .take(request.top_k.min(ranked.len()))
        .map(|target| target.node_id.clone())
        .collect::<Vec<_>>();
    targets.sort_by(|left, right| left.node_id.cmp(&right.node_id));
    let all_models_qualified = simulations
        .iter()
        .all(|model| model.simulation.disposition == CounterfactualDisposition::Qualified);
    let all_targets_agree = targets.iter().all(|target| {
        target.agreement_milli >= request.min_model_agreement_milli
            && target.direction != EnsembleDirection::Unresolved
    });
    let disposition = if all_models_qualified && all_targets_agree {
        EnsembleDisposition::Qualified
    } else if !all_models_qualified
        && simulations
            .iter()
            .all(|model| model.simulation.disposition == CounterfactualDisposition::Unresolved)
    {
        EnsembleDisposition::Unresolved
    } else {
        EnsembleDisposition::Partial
    };
    if !all_targets_agree {
        ensemble_uncertainty.insert("ensemble-target-agreement-gate-not-met".into());
    }
    let mut output = MechanismCounterfactualEnsemble {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        model_system: request.model_system,
        model_order,
        node_order,
        intervention_order,
        models: simulations,
        target_order,
        targets,
        total_prior_milli,
        uncertainty: ensemble_uncertainty.into_iter().collect(),
        negative_evidence: negative_evidence.into_iter().collect(),
        disposition,
        digest: ContentHash::of_bytes(b"unsealed-glioma-counterfactual-ensemble"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| EnsembleCounterfactualError::Digest(error.to_string()))?;
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

    fn model(id: &str, prior_milli: u16, support: u16) -> CounterfactualModel {
        CounterfactualModel {
            model_id: id.into(),
            prior_milli,
            nodes: vec![node("egfr", support), node("invasion", 0)],
            edges: vec![MechanismGraphEdge {
                edge_id: format!("{id}-edge"),
                source_node_id: "egfr".into(),
                target_node_id: "invasion".into(),
                relation: MechanismGraphRelation::Activates,
                confidence_milli: 900,
                evidence_order: vec![format!("evidence-{id}")],
            }],
        }
    }

    fn request() -> CounterfactualEnsembleRequest {
        CounterfactualEnsembleRequest {
            objective: "rank robust EGFR inhibition targets".into(),
            model_system: GliomaModelSystem::Organoid,
            max_iterations: 100,
            convergence_tolerance_milli: 1,
            damping_milli: 600,
            min_edge_confidence_milli: 500,
            min_effect_milli: 10,
            min_model_agreement_milli: 750,
            top_k: 2,
        }
    }

    fn intervention() -> CounterfactualIntervention {
        CounterfactualIntervention {
            intervention_id: "inhibit-egfr".into(),
            node_id: "egfr".into(),
            delta_milli: -600,
            rationale: "test whether EGFR support propagates to invasion".into(),
            evidence_order: vec!["paper-egfr".into()],
        }
    }

    #[test]
    fn model_averaging_exposes_agreement_and_replays() {
        let first = simulate_glioma_counterfactual_ensemble(
            &request(),
            &[model("model-a", 600, 800), model("model-b", 400, 700)],
            &[intervention()],
        )
        .unwrap();
        let second = simulate_glioma_counterfactual_ensemble(
            &request(),
            &[model("model-b", 400, 700), model("model-a", 600, 800)],
            &[intervention()],
        )
        .unwrap();
        assert_eq!(first, second);
        assert_eq!(first.disposition, EnsembleDisposition::Qualified);
        let invasion = first
            .targets
            .iter()
            .find(|target| target.node_id == "invasion")
            .unwrap();
        assert_eq!(invasion.direction, EnsembleDirection::Decreases);
        assert_eq!(invasion.agreement_milli, 1_000);
        first.validate().unwrap();
    }

    #[test]
    fn opposing_models_are_partial_not_a_consensus_claim() {
        let mut opposing = model("model-b", 400, 700);
        opposing.edges[0].relation = MechanismGraphRelation::Inhibits;
        let output = simulate_glioma_counterfactual_ensemble(
            &request(),
            &[model("model-a", 600, 800), opposing],
            &[intervention()],
        )
        .unwrap();
        assert_eq!(output.disposition, EnsembleDisposition::Partial);
        assert!(output
            .uncertainty
            .iter()
            .any(|item| item.contains("agreement")));
        assert!(output
            .targets
            .iter()
            .any(|target| target.direction == EnsembleDirection::Unresolved));
    }
}
