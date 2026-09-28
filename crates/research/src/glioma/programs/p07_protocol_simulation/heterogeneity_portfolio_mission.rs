//! P06-to-P07 mission compilation for autonomous glioma research.
//!
//! The experiment portfolio planner chooses biologically diverse candidate arms, while the P07
//! scheduler chooses executable actions under dependency, risk, authority, and budget limits.
//! This bridge makes that handoff a typed product operation: selected portfolio candidates must
//! have declared action bindings, dependencies must be closed, and an underpowered or blocked
//! portfolio cannot be promoted into an autonomous mission. The bridge only plans; it never
//! invokes an instrument, moves raw data, or makes a clinical decision.

use super::adaptive_scheduler::{
    plan_glioma_adaptive_workflow, GliomaAdaptiveWorkflowSchedulerError,
    GliomaAdaptiveWorkflowSchedulerPlan, GliomaAdaptiveWorkflowSchedulerRequest,
    SchedulerObservation,
};
use crate::glioma::programs::p06_experiment_design::{
    HeterogeneityAwareExperimentPortfolio, HeterogeneityPortfolioDisposition,
};
use crate::glioma_engine::{GliomaActionCandidate, GliomaSelectionWeights};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const COMPOSITION_ID: &str = "glioma-heterogeneity-portfolio-mission-bridge";
pub const OUTPUT_SCHEMA: &str = "GliomaHeterogeneityPortfolioMission1@1";
pub const MAX_BINDINGS: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HeterogeneityPortfolioActionBinding {
    pub portfolio_candidate_id: String,
    pub action: GliomaActionCandidate,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HeterogeneityPortfolioMissionRequest {
    pub mission_id: String,
    pub objective: String,
    pub portfolio: HeterogeneityAwareExperimentPortfolio,
    pub bindings: Vec<HeterogeneityPortfolioActionBinding>,
    pub completed_action_order: Vec<String>,
    pub observations: Vec<SchedulerObservation>,
    pub budget_units: u32,
    pub max_actions: u16,
    pub beam_width: u16,
    pub risk_budget_milli: u32,
    pub approval_granted: bool,
    pub allow_instrument_execution: bool,
    pub allow_federation: bool,
    pub selection_weights: GliomaSelectionWeights,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HeterogeneityPortfolioMissionDisposition {
    Ready,
    Partial,
    PortfolioHold,
    RouteHold,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HeterogeneityPortfolioMissionPlan {
    pub composition_id: String,
    pub output_schema: String,
    pub mission_id: String,
    pub objective: String,
    pub portfolio_digest: ContentHash,
    pub selected_candidate_order: Vec<String>,
    pub deferred_candidate_order: Vec<String>,
    pub selected_action_order: Vec<String>,
    pub deferred_action_order: Vec<String>,
    pub scheduler: Option<GliomaAdaptiveWorkflowSchedulerPlan>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: HeterogeneityPortfolioMissionDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum HeterogeneityPortfolioMissionError {
    #[error("heterogeneity portfolio mission request is invalid: {0}")]
    InvalidRequest(String),
    #[error("heterogeneity portfolio mission scheduler failed: {0}")]
    Scheduler(String),
    #[error("heterogeneity portfolio mission output is invalid: {0}")]
    InvalidOutput(String),
    #[error("heterogeneity portfolio mission digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn sorted_unique(values: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut values = values.into_iter().collect::<Vec<_>>();
    values.sort();
    values.dedup();
    values
}

fn digest_input(output: &HeterogeneityPortfolioMissionPlan) -> serde_json::Value {
    serde_json::json!({
        "composition_id": output.composition_id,
        "output_schema": output.output_schema,
        "mission_id": output.mission_id,
        "objective": output.objective,
        "portfolio_digest": output.portfolio_digest,
        "selected_candidate_order": output.selected_candidate_order,
        "deferred_candidate_order": output.deferred_candidate_order,
        "selected_action_order": output.selected_action_order,
        "deferred_action_order": output.deferred_action_order,
        "scheduler": output.scheduler,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
    })
}

fn validate_request(
    request: &HeterogeneityPortfolioMissionRequest,
) -> Result<(), HeterogeneityPortfolioMissionError> {
    if request.mission_id.trim().is_empty()
        || request.objective.trim().is_empty()
        || request.bindings.is_empty()
        || request.bindings.len() > MAX_BINDINGS
        || request.budget_units == 0
        || request.max_actions == 0
        || request.beam_width == 0
        || request.risk_budget_milli > 1_000_000
    {
        return Err(HeterogeneityPortfolioMissionError::InvalidRequest(
            "mission/objective, bindings, positive scheduler bounds, and bounded risk are required"
                .into(),
        ));
    }
    request
        .portfolio
        .validate()
        .map_err(|error| HeterogeneityPortfolioMissionError::InvalidRequest(error.to_string()))?;
    if !canonical(&request.completed_action_order)
        || request
            .completed_action_order
            .windows(2)
            .any(|pair| pair[0] == pair[1])
    {
        return Err(HeterogeneityPortfolioMissionError::InvalidRequest(
            "completed actions must be strictly canonical".into(),
        ));
    }
    let mut binding_ids = BTreeSet::new();
    let mut action_ids = BTreeSet::new();
    for binding in &request.bindings {
        if binding.portfolio_candidate_id.trim().is_empty()
            || !binding_ids.insert(binding.portfolio_candidate_id.clone())
            || !action_ids.insert(binding.action.action_id.clone())
        {
            return Err(HeterogeneityPortfolioMissionError::InvalidRequest(
                "portfolio candidate ids and bound action ids must be unique and non-empty".into(),
            ));
        }
    }
    Ok(())
}

impl HeterogeneityPortfolioMissionPlan {
    pub fn validate(&self) -> Result<(), HeterogeneityPortfolioMissionError> {
        if self.composition_id != COMPOSITION_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.mission_id.trim().is_empty()
            || self.objective.trim().is_empty()
            || !canonical(&self.selected_candidate_order)
            || !canonical(&self.deferred_candidate_order)
            || !canonical(&self.selected_action_order)
            || !canonical(&self.deferred_action_order)
            || self
                .selected_candidate_order
                .iter()
                .any(|id| self.deferred_candidate_order.binary_search(id).is_ok())
            || self
                .selected_action_order
                .iter()
                .any(|id| self.deferred_action_order.binary_search(id).is_ok())
            || !canonical(&self.negative_evidence)
            || !canonical(&self.uncertainty)
            || matches!(
                self.disposition,
                HeterogeneityPortfolioMissionDisposition::Ready
            ) && self.scheduler.is_none()
            || matches!(
                self.disposition,
                HeterogeneityPortfolioMissionDisposition::PortfolioHold
                    | HeterogeneityPortfolioMissionDisposition::RouteHold
                    | HeterogeneityPortfolioMissionDisposition::Blocked
            ) && self.scheduler.is_some()
        {
            return Err(HeterogeneityPortfolioMissionError::InvalidOutput(
                "identity, partition ordering, disposition, or scheduler binding is invalid".into(),
            ));
        }
        if let Some(scheduler) = &self.scheduler {
            scheduler.validate().map_err(|error| {
                HeterogeneityPortfolioMissionError::InvalidOutput(error.to_string())
            })?;
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| HeterogeneityPortfolioMissionError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(HeterogeneityPortfolioMissionError::InvalidOutput(
                "mission plan digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

/// Compile a selected heterogeneity portfolio into a dependency-safe P07 mission plan. This is
/// intentionally non-dispatching: a host may later submit the returned scheduler plan to its
/// evidence, authority, and instrument gates.
pub fn plan_glioma_heterogeneity_portfolio_mission(
    request: &HeterogeneityPortfolioMissionRequest,
) -> Result<HeterogeneityPortfolioMissionPlan, HeterogeneityPortfolioMissionError> {
    validate_request(request)?;
    let selected_candidates = request
        .portfolio
        .selected
        .iter()
        .map(|selection| selection.candidate_id.clone())
        .collect::<BTreeSet<_>>();
    let binding_map = request
        .bindings
        .iter()
        .map(|binding| (binding.portfolio_candidate_id.clone(), &binding.action))
        .collect::<BTreeMap<_, _>>();
    let action_to_candidate = binding_map
        .iter()
        .map(|(candidate_id, action)| (action.action_id.clone(), candidate_id.clone()))
        .collect::<BTreeMap<_, _>>();
    let completed = request
        .completed_action_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut missing_bindings = selected_candidates
        .difference(&binding_map.keys().cloned().collect())
        .cloned()
        .collect::<Vec<_>>();
    missing_bindings.sort();

    let mut uncertainty = Vec::new();
    let mut negative_evidence = request.portfolio.negative_evidence.clone();
    let mut deferred_candidates = request
        .portfolio
        .deferred
        .iter()
        .map(|item| item.candidate_id.clone())
        .collect::<Vec<_>>();
    deferred_candidates.extend(missing_bindings.iter().cloned());
    let mut action_candidates = Vec::new();
    let mut route_hold = false;
    for candidate_id in selected_candidates.iter() {
        let Some(action) = binding_map.get(candidate_id) else {
            continue;
        };
        let missing_dependencies = action
            .depends_on
            .iter()
            .filter(|dependency| {
                !completed.contains(*dependency)
                    && !request.bindings.iter().any(|binding| {
                        selected_candidates.contains(&binding.portfolio_candidate_id)
                            && binding.action.action_id == **dependency
                    })
            })
            .cloned()
            .collect::<Vec<_>>();
        if !missing_dependencies.is_empty() {
            route_hold = true;
            uncertainty.push(format!(
                "{} missing selected or completed dependencies: {}",
                candidate_id,
                missing_dependencies.join(", ")
            ));
            deferred_candidates.push(candidate_id.clone());
        } else {
            action_candidates.push((*action).clone());
        }
    }

    let portfolio_hold = matches!(
        request.portfolio.disposition,
        HeterogeneityPortfolioDisposition::Blocked
            | HeterogeneityPortfolioDisposition::Underpowered
    );
    if portfolio_hold {
        negative_evidence.push(format!(
            "portfolio-disposition:{:?}",
            request.portfolio.disposition
        ));
    }
    if !missing_bindings.is_empty() {
        uncertainty.push(format!(
            "selected portfolio candidates without action bindings: {}",
            missing_bindings.join(", ")
        ));
    }

    let mut scheduler = None;
    let mut selected_actions = Vec::new();
    let mut deferred_actions = Vec::new();
    let disposition = if portfolio_hold {
        HeterogeneityPortfolioMissionDisposition::PortfolioHold
    } else if route_hold || !missing_bindings.is_empty() {
        HeterogeneityPortfolioMissionDisposition::RouteHold
    } else if action_candidates.is_empty() {
        HeterogeneityPortfolioMissionDisposition::Blocked
    } else {
        let scheduler_request = GliomaAdaptiveWorkflowSchedulerRequest {
            mission_id: request.mission_id.clone(),
            objective: request.objective.clone(),
            candidates: action_candidates,
            completed_action_order: request.completed_action_order.clone(),
            observations: request.observations.clone(),
            budget_units: request.budget_units,
            max_actions: request.max_actions,
            beam_width: request.beam_width,
            risk_budget_milli: request.risk_budget_milli,
            approval_granted: request.approval_granted,
            allow_instrument_execution: request.allow_instrument_execution,
            allow_federation: request.allow_federation,
            selection_weights: request.selection_weights,
        };
        let plan = plan_glioma_adaptive_workflow(&scheduler_request).map_err(
            |error: GliomaAdaptiveWorkflowSchedulerError| {
                HeterogeneityPortfolioMissionError::Scheduler(error.to_string())
            },
        )?;
        selected_actions = plan.selected_order.clone();
        deferred_actions = plan
            .deferred_order
            .iter()
            .chain(plan.blocked_order.iter())
            .cloned()
            .collect();
        deferred_candidates.extend(
            deferred_actions
                .iter()
                .filter_map(|action_id| action_to_candidate.get(action_id).cloned()),
        );
        negative_evidence.extend(plan.negative_evidence.clone());
        uncertainty.extend(plan.uncertainty.clone());
        scheduler = Some(plan);
        if selected_actions.is_empty() {
            HeterogeneityPortfolioMissionDisposition::Blocked
        } else if !deferred_actions.is_empty() {
            HeterogeneityPortfolioMissionDisposition::Partial
        } else {
            HeterogeneityPortfolioMissionDisposition::Ready
        }
    };

    let mut selected_candidate_order = selected_candidates
        .iter()
        .filter(|id| !deferred_candidates.contains(id))
        .cloned()
        .collect::<Vec<_>>();
    selected_candidate_order.sort();
    deferred_candidates = sorted_unique(deferred_candidates);
    selected_candidate_order.retain(|id| !deferred_candidates.binary_search(id).is_ok());
    deferred_actions = sorted_unique(deferred_actions);
    let mut output = HeterogeneityPortfolioMissionPlan {
        composition_id: COMPOSITION_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        mission_id: request.mission_id.clone(),
        objective: request.objective.clone(),
        portfolio_digest: request.portfolio.digest.clone(),
        selected_candidate_order,
        deferred_candidate_order: deferred_candidates,
        selected_action_order: sorted_unique(selected_actions),
        deferred_action_order: deferred_actions,
        scheduler,
        negative_evidence: sorted_unique(negative_evidence),
        uncertainty: sorted_unique(uncertainty),
        disposition,
        digest: ContentHash::of_bytes(b"unsealed-glioma-heterogeneity-portfolio-mission"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| HeterogeneityPortfolioMissionError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::programs::p06_experiment_design::{
        plan_glioma_heterogeneity_aware_experiment_portfolio,
        HeterogeneityAwareExperimentPortfolioRequest, HeterogeneityExperimentCandidate,
        HeterogeneityExperimentStratum,
    };
    use crate::glioma_engine::{GliomaModality, GliomaModelSystem, GliomaStageKind};
    use bioprism_foundation::{AutonomyTier, Effect};

    fn portfolio() -> HeterogeneityAwareExperimentPortfolio {
        plan_glioma_heterogeneity_aware_experiment_portfolio(
            &HeterogeneityAwareExperimentPortfolioRequest {
                objective: "validate invasion mechanism across model systems".into(),
                budget_units: 30,
                replication_reserve_fraction_milli: 200,
                min_model_systems: 2,
                min_power_milli: 100,
                max_risk_milli: 700,
                max_selected_arms: 3,
                strata: vec![
                    HeterogeneityExperimentStratum {
                        stratum_id: "organoid".into(),
                        model_system: GliomaModelSystem::Organoid,
                        prior_milli: 500,
                        heterogeneity_milli: 200,
                        required: true,
                    },
                    HeterogeneityExperimentStratum {
                        stratum_id: "in_silico".into(),
                        model_system: GliomaModelSystem::InSilico,
                        prior_milli: 500,
                        heterogeneity_milli: 500,
                        required: true,
                    },
                ],
                candidates: vec![
                    HeterogeneityExperimentCandidate {
                        candidate_id: "organoid-invasion".into(),
                        arm_id: "invasion".into(),
                        stratum_id: "organoid".into(),
                        modality: GliomaModality::Imaging,
                        independence_group: "site-a".into(),
                        cost_units_per_replicate: 4,
                        max_replicates: 3,
                        expected_effect_milli: 800,
                        effect_uncertainty_milli: 100,
                        reproducibility_milli: 900,
                        risk_milli: 100,
                        available: true,
                    },
                    HeterogeneityExperimentCandidate {
                        candidate_id: "insilico-invasion".into(),
                        arm_id: "invasion".into(),
                        stratum_id: "in_silico".into(),
                        modality: GliomaModality::Computational,
                        independence_group: "compute-a".into(),
                        cost_units_per_replicate: 2,
                        max_replicates: 3,
                        expected_effect_milli: 650,
                        effect_uncertainty_milli: 120,
                        reproducibility_milli: 950,
                        risk_milli: 50,
                        available: true,
                    },
                ],
            },
        )
        .unwrap()
    }

    fn binding(candidate_id: &str, action_id: &str) -> HeterogeneityPortfolioActionBinding {
        HeterogeneityPortfolioActionBinding {
            portfolio_candidate_id: candidate_id.into(),
            action: GliomaActionCandidate {
                action_id: action_id.into(),
                stage_kind: GliomaStageKind::ComputationalExecution,
                modality: GliomaModality::Computational,
                model_system: GliomaModelSystem::InSilico,
                depends_on: Vec::new(),
                cost_units: 1,
                information_gain_milli: 800,
                frontier_novelty_milli: 700,
                workflow_leverage_milli: 700,
                cross_stage_unlock_milli: 500,
                reproducibility_safety_milli: 900,
                federation_value_milli: 300,
                feasibility_milli: 900,
                autonomy_tier: AutonomyTier::A1,
                effects: BTreeSet::from([Effect::ReadLocalData, Effect::ExecuteLocalComputation]),
            },
        }
    }

    fn request(
        bindings: Vec<HeterogeneityPortfolioActionBinding>,
    ) -> HeterogeneityPortfolioMissionRequest {
        HeterogeneityPortfolioMissionRequest {
            mission_id: "portfolio-mission".into(),
            objective: "compile a reproducible invasion workflow".into(),
            portfolio: portfolio(),
            bindings,
            completed_action_order: Vec::new(),
            observations: Vec::new(),
            budget_units: 4,
            max_actions: 4,
            beam_width: 32,
            risk_budget_milli: 10_000,
            approval_granted: false,
            allow_instrument_execution: false,
            allow_federation: false,
            selection_weights: GliomaSelectionWeights::default(),
        }
    }

    #[test]
    fn bridge_compiles_selected_portfolio_into_scheduler_plan() {
        let output = plan_glioma_heterogeneity_portfolio_mission(&request(vec![
            binding("organoid-invasion", "organoid-analysis"),
            binding("insilico-invasion", "insilico-analysis"),
        ]))
        .unwrap();
        assert!(matches!(
            output.disposition,
            HeterogeneityPortfolioMissionDisposition::Ready
                | HeterogeneityPortfolioMissionDisposition::Partial
        ));
        assert!(!output.selected_action_order.is_empty());
        output.validate().unwrap();
    }

    #[test]
    fn bridge_holds_when_a_selected_candidate_has_no_action_binding() {
        let output = plan_glioma_heterogeneity_portfolio_mission(&request(vec![binding(
            "organoid-invasion",
            "organoid-analysis",
        )]))
        .unwrap();
        assert_eq!(
            output.disposition,
            HeterogeneityPortfolioMissionDisposition::RouteHold
        );
        assert!(output
            .uncertainty
            .iter()
            .any(|item| item.contains("insilico-invasion")));
        output.validate().unwrap();
    }
}
