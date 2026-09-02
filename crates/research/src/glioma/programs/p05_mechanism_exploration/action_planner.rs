//! Compile mechanism-discrimination results into executable glioma research actions.
//!
//! Mechanism discrimination already estimates which measurements separate competing models.
//! This module turns that scientific result into the typed action vocabulary consumed by the
//! autonomous campaign controller.  It is intentionally a compiler, not a dispatcher: it adds
//! no biological observation, keeps uncertainty and negative evidence, and emits only local
//! computation/read/artifact effects at A1.  A host can hand the resulting candidates to
//! `execute_glioma_autonomous_campaign` and supply its own planner/worker when new evidence is
//! available.

use super::super::p07_protocol_simulation::autonomous_campaign::{
    GliomaActionPlanner, GliomaAutonomousPlannerContext, GliomaPlannerFailure,
};
use super::discrimination::{MechanismDiscrimination, MechanismInformationGain};
use crate::glioma_engine::{
    GliomaActionCandidate, GliomaModality, GliomaModelSystem, GliomaStageKind,
};
use bioprism_foundation::{AutonomyTier, Effect};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P05-F20";
pub const OUTPUT_SCHEMA: &str = "GliomaMechanismActionPlan1@1";
pub const MAX_ACTIONS: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismActionPlannerConfig {
    pub model_system: GliomaModelSystem,
    pub modality: GliomaModality,
    pub max_actions: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismActionPlan {
    pub feature_id: String,
    pub output_schema: String,
    pub source_discrimination_digest: ContentHash,
    pub model_system: GliomaModelSystem,
    pub modality: GliomaModality,
    pub action_order: Vec<String>,
    pub candidates: Vec<GliomaActionCandidate>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MechanismActionPlannerError {
    #[error("mechanism action planner request is invalid: {0}")]
    InvalidRequest(String),
    #[error("mechanism action plan is invalid: {0}")]
    InvalidOutput(String),
    #[error("mechanism action plan digest failed: {0}")]
    Digest(String),
}

fn bounded_score(value: u64) -> u16 {
    value.min(1_000_000).saturating_div(1_000) as u16
}

fn candidate_priority(action: &MechanismInformationGain) -> (u128, String) {
    let feasibility = u128::from(action.feasibility_milli.max(1));
    let uncertainty = u128::from(action.measurement_uncertainty_milli.max(1));
    let score = u128::from(action.adjusted_information_milli)
        .saturating_mul(feasibility)
        .saturating_mul(1_000_000)
        .saturating_div(u128::from(action.cost_units).saturating_mul(uncertainty));
    (score, action.action_id.clone())
}

fn to_candidate(
    action: &MechanismInformationGain,
    config: &MechanismActionPlannerConfig,
) -> GliomaActionCandidate {
    let uncertainty_penalty = action.measurement_uncertainty_milli.min(1_000) as u16;
    let mechanism_unlock = (action.mechanism_order.len() as u16)
        .saturating_mul(125)
        .min(1_000);
    GliomaActionCandidate {
        action_id: format!("mechanism-assay:{}", action.action_id),
        stage_kind: GliomaStageKind::ExperimentDesign,
        modality: config.modality,
        model_system: config.model_system,
        depends_on: Vec::new(),
        cost_units: action.cost_units,
        information_gain_milli: bounded_score(action.adjusted_information_milli),
        frontier_novelty_milli: bounded_score(action.expected_information_milli),
        workflow_leverage_milli: bounded_score(action.expected_information_milli),
        cross_stage_unlock_milli: mechanism_unlock,
        reproducibility_safety_milli: 1_000 - uncertainty_penalty,
        federation_value_milli: action.feasibility_milli,
        feasibility_milli: action.feasibility_milli,
        autonomy_tier: AutonomyTier::A1,
        effects: BTreeSet::from([
            Effect::ReadLocalData,
            Effect::ExecuteLocalComputation,
            Effect::WriteLocalArtifact,
        ]),
    }
}

fn digest_input(plan: &MechanismActionPlan) -> serde_json::Value {
    serde_json::json!({
        "feature_id": plan.feature_id,
        "output_schema": plan.output_schema,
        "source_discrimination_digest": plan.source_discrimination_digest,
        "model_system": plan.model_system,
        "modality": plan.modality,
        "action_order": plan.action_order,
        "candidates": plan.candidates,
        "uncertainty": plan.uncertainty,
        "negative_evidence": plan.negative_evidence,
    })
}

impl MechanismActionPlan {
    pub fn validate(&self) -> Result<(), MechanismActionPlannerError> {
        let ids = self
            .candidates
            .iter()
            .map(|candidate| candidate.action_id.clone())
            .collect::<Vec<_>>();
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.source_discrimination_digest.as_str().len() != 64
            || self.action_order != ids
            || ids.is_empty()
            || ids.iter().any(|id| id.trim().is_empty())
            || ids.iter().collect::<HashSet<_>>().len() != ids.len()
            || self.candidates.len() > MAX_ACTIONS
            || self.uncertainty.windows(2).any(|pair| pair[0] >= pair[1])
            || self
                .negative_evidence
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || self.candidates.iter().any(|candidate| {
                candidate.stage_kind != GliomaStageKind::ExperimentDesign
                    || candidate.autonomy_tier != AutonomyTier::A1
                    || candidate.effects
                        != BTreeSet::from([
                            Effect::ReadLocalData,
                            Effect::ExecuteLocalComputation,
                            Effect::WriteLocalArtifact,
                        ])
            })
        {
            return Err(MechanismActionPlannerError::InvalidOutput(
                "identity, candidate ordering, bounds, provenance, uncertainty, or local-effect invariants are invalid".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| MechanismActionPlannerError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(MechanismActionPlannerError::InvalidOutput(
                "mechanism action plan digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

/// Compile the highest-value discriminator assays into candidates for the autonomous engine.
pub fn compile_mechanism_action_plan(
    discrimination: &MechanismDiscrimination,
    config: &MechanismActionPlannerConfig,
) -> Result<MechanismActionPlan, MechanismActionPlannerError> {
    discrimination
        .validate()
        .map_err(|error| MechanismActionPlannerError::InvalidRequest(error.to_string()))?;
    if config.max_actions == 0 || config.max_actions > MAX_ACTIONS {
        return Err(MechanismActionPlannerError::InvalidRequest(
            "max_actions must be between 1 and 256".into(),
        ));
    }
    let mut actions = discrimination.actions.clone();
    actions.sort_by(|left, right| {
        let left_priority = candidate_priority(left);
        let right_priority = candidate_priority(right);
        right_priority
            .0
            .cmp(&left_priority.0)
            .then_with(|| left_priority.1.cmp(&right_priority.1))
    });
    actions.truncate(config.max_actions);
    if actions.is_empty() {
        return Err(MechanismActionPlannerError::InvalidRequest(
            "mechanism discrimination returned no executable action".into(),
        ));
    }
    let candidates = actions
        .iter()
        .map(|action| to_candidate(action, config))
        .collect::<Vec<_>>();
    let action_order = candidates
        .iter()
        .map(|candidate| candidate.action_id.clone())
        .collect::<Vec<_>>();
    let mut uncertainty = discrimination.uncertainty.clone();
    uncertainty.push("mechanism-action-prioritization-is-not-a-biological-result".into());
    uncertainty.sort();
    uncertainty.dedup();
    let mut negative_evidence = discrimination.negative_evidence.clone();
    negative_evidence.sort();
    negative_evidence.dedup();
    let mut plan = MechanismActionPlan {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        source_discrimination_digest: discrimination.digest.clone(),
        model_system: config.model_system,
        modality: config.modality,
        action_order,
        candidates,
        uncertainty,
        negative_evidence,
        digest: ContentHash::of_bytes(b"unsealed-mechanism-action-plan"),
    };
    plan.digest = ContentHash::of_value(&digest_input(&plan))
        .map_err(|error| MechanismActionPlannerError::Digest(error.to_string()))?;
    plan.validate()?;
    Ok(plan)
}

/// A deterministic planner adapter for the autonomous campaign controller.
#[derive(Debug, Clone)]
pub struct GliomaMechanismActionPlanner {
    candidates: Vec<GliomaActionCandidate>,
}

impl GliomaMechanismActionPlanner {
    pub fn from_plan(plan: &MechanismActionPlan) -> Result<Self, MechanismActionPlannerError> {
        plan.validate()?;
        Ok(Self {
            candidates: plan.candidates.clone(),
        })
    }
}

impl GliomaActionPlanner for GliomaMechanismActionPlanner {
    fn propose_actions(
        &mut self,
        context: &GliomaAutonomousPlannerContext,
    ) -> Result<Vec<GliomaActionCandidate>, GliomaPlannerFailure> {
        Ok(self
            .candidates
            .iter()
            .filter(|candidate| {
                !context.completed_actions.contains(&candidate.action_id)
                    && !context.terminal_actions.contains(&candidate.action_id)
            })
            .cloned()
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::programs::p05_mechanism_exploration::discrimination::{
        MechanismDiscrimination, MechanismDiscriminationDisposition, MechanismDiscriminationRanking,
    };

    fn discrimination() -> MechanismDiscrimination {
        let mut actions = vec![
            MechanismInformationGain {
                action_id: "low".into(),
                feature_id: "f-low".into(),
                mechanism_order: vec!["m1".into(), "m2".into()],
                expected_information_milli: 400_000,
                adjusted_information_milli: 200_000,
                measurement_uncertainty_milli: 100,
                feasibility_milli: 900,
                cost_units: 2,
            },
            MechanismInformationGain {
                action_id: "high".into(),
                feature_id: "f-high".into(),
                mechanism_order: vec!["m1".into(), "m2".into()],
                expected_information_milli: 900_000,
                adjusted_information_milli: 800_000,
                measurement_uncertainty_milli: 100,
                feasibility_milli: 900,
                cost_units: 2,
            },
        ];
        actions.sort_by(|left, right| {
            right
                .adjusted_information_milli
                .cmp(&left.adjusted_information_milli)
                .then_with(|| left.action_id.cmp(&right.action_id))
        });
        MechanismDiscrimination {
            feature_id: "GAF-GLIOMA-P05-F09".into(),
            output_schema: "GliomaMechanismDiscrimination1@1".into(),
            objective: "invasion".into(),
            model_system: GliomaModelSystem::Organoid,
            mechanism_order: vec!["m1".into(), "m2".into()],
            rankings: vec![
                MechanismDiscriminationRanking {
                    mechanism_id: "m1".into(),
                    matched_feature_order: vec!["f-high".into()],
                    missing_feature_order: Vec::new(),
                    residual_loss_milli: 100,
                    coverage_milli: 1_000,
                    fit_score_milli: 900_000,
                    posterior_milli: 500,
                },
                MechanismDiscriminationRanking {
                    mechanism_id: "m2".into(),
                    matched_feature_order: vec!["f-high".into()],
                    missing_feature_order: Vec::new(),
                    residual_loss_milli: 200,
                    coverage_milli: 1_000,
                    fit_score_milli: 800_000,
                    posterior_milli: 500,
                },
            ],
            action_order: vec!["high".into(), "low".into()],
            actions,
            selected_action_order: vec!["high".into(), "low".into()],
            unresolved_mechanism_order: Vec::new(),
            negative_evidence: Vec::new(),
            uncertainty: Vec::new(),
            disposition: MechanismDiscriminationDisposition::Qualified,
            digest: ContentHash::of_bytes(b"placeholder"),
        }
    }

    #[test]
    fn planner_prioritizes_information_per_cost_and_is_replay_stable() {
        let mut discrimination = discrimination();
        let digest_input = serde_json::json!({
            "feature_id": discrimination.feature_id,
            "output_schema": discrimination.output_schema,
            "objective": discrimination.objective,
            "model_system": discrimination.model_system,
            "mechanism_order": discrimination.mechanism_order,
            "rankings": discrimination.rankings,
            "action_order": discrimination.action_order,
            "actions": discrimination.actions,
            "selected_action_order": discrimination.selected_action_order,
            "unresolved_mechanism_order": discrimination.unresolved_mechanism_order,
            "negative_evidence": discrimination.negative_evidence,
            "uncertainty": discrimination.uncertainty,
            "disposition": discrimination.disposition,
        });
        discrimination.digest = ContentHash::of_value(&digest_input).unwrap();
        discrimination.validate().unwrap();
        let config = MechanismActionPlannerConfig {
            model_system: GliomaModelSystem::Organoid,
            modality: GliomaModality::Transcriptomics,
            max_actions: 2,
        };
        let first = compile_mechanism_action_plan(&discrimination, &config).unwrap();
        let second = compile_mechanism_action_plan(&discrimination, &config).unwrap();
        assert_eq!(first, second);
        assert_eq!(
            first.action_order,
            vec!["mechanism-assay:high", "mechanism-assay:low"]
        );
        first.validate().unwrap();
    }

    #[test]
    fn planner_does_not_reoffer_completed_or_terminal_actions() {
        let mut discrimination = discrimination();
        let digest_input = serde_json::json!({
            "feature_id": discrimination.feature_id,
            "output_schema": discrimination.output_schema,
            "objective": discrimination.objective,
            "model_system": discrimination.model_system,
            "mechanism_order": discrimination.mechanism_order,
            "rankings": discrimination.rankings,
            "action_order": discrimination.action_order,
            "actions": discrimination.actions,
            "selected_action_order": discrimination.selected_action_order,
            "unresolved_mechanism_order": discrimination.unresolved_mechanism_order,
            "negative_evidence": discrimination.negative_evidence,
            "uncertainty": discrimination.uncertainty,
            "disposition": discrimination.disposition,
        });
        discrimination.digest = ContentHash::of_value(&digest_input).unwrap();
        let plan = compile_mechanism_action_plan(
            &discrimination,
            &MechanismActionPlannerConfig {
                model_system: GliomaModelSystem::Organoid,
                modality: GliomaModality::Genomics,
                max_actions: 2,
            },
        )
        .unwrap();
        let mut planner = GliomaMechanismActionPlanner::from_plan(&plan).unwrap();
        let context = GliomaAutonomousPlannerContext {
            round: 2,
            completed_actions: BTreeSet::from(["mechanism-assay:high".into()]),
            terminal_actions: BTreeSet::from(["mechanism-assay:low".into()]),
            available_action_ids: plan.action_order,
            budget_remaining_units: 10,
            previous_results: Vec::new(),
        };
        assert!(planner.propose_actions(&context).unwrap().is_empty());
    }
}
