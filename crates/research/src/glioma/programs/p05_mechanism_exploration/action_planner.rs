//! Compile mechanism-discrimination results into beam-selected executable glioma research actions.
//!
//! Mechanism discrimination already estimates which measurements separate competing models.
//! This module turns that scientific result into the typed action vocabulary consumed by the
//! autonomous campaign controller.  It is intentionally a compiler, not a dispatcher: it adds
//! no biological observation, keeps uncertainty and negative evidence, and emits only local
//! computation/read/artifact effects at A1.  A host can hand the resulting candidates to
//! `execute_glioma_autonomous_campaign` and supply its own planner/worker when new evidence is
//! available. Assay batches use a bounded deterministic portfolio search so mechanism coverage
//! is optimized jointly with information-per-cost rather than by a myopic greedy pick.

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
pub const OUTPUT_SCHEMA: &str = "GliomaMechanismActionPlan1@3";
pub const MAX_ACTIONS: usize = 256;
const MECHANISM_NOVELTY_BONUS: u128 = 1_000_000_000_000;
const MECHANISM_ACTION_BEAM_WIDTH: usize = 128;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismActionPlannerConfig {
    pub model_system: GliomaModelSystem,
    pub modality: GliomaModality,
    pub max_actions: usize,
    /// Total assay cost the compiled portfolio may reserve. Selection is bounded by both this
    /// envelope and `max_actions`, so a single expensive discriminator cannot silently crowd out
    /// a complementary set that fits the declared research budget.
    pub budget_units: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismActionPlan {
    pub feature_id: String,
    pub output_schema: String,
    pub source_discrimination_digest: ContentHash,
    pub model_system: GliomaModelSystem,
    pub modality: GliomaModality,
    pub budget_units: u64,
    pub budget_spent_units: u64,
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct MechanismActionBatchState {
    selected: Vec<usize>,
    cost_units: u64,
}

fn mechanism_batch_score(
    state: &MechanismActionBatchState,
    actions: &[MechanismInformationGain],
) -> u128 {
    let mut score = 0_u128;
    let mut covered = BTreeSet::new();
    for index in &state.selected {
        let action = &actions[*index];
        // Normalize broad discriminator proposals by the number of mechanisms they cover. This
        // prevents one high-scoring, shared assay from crowding out two near-tied orthogonal
        // assays that provide independent mechanism evidence as a batch.
        let mechanism_count = u128::from(action.mechanism_order.len().max(1) as u32);
        score = score.saturating_add(candidate_priority(action).0 / mechanism_count);
        let novel_count = action
            .mechanism_order
            .iter()
            .filter(|mechanism| covered.insert((*mechanism).clone()))
            .count() as u128;
        score = score.saturating_add(novel_count.saturating_mul(MECHANISM_NOVELTY_BONUS));
    }
    score
}

/// Select discriminator assays with a bounded mechanism-coverage portfolio search. A
/// high-information assay remains valuable, while a near-tied set that probes mechanisms not yet
/// represented in the batch can displace a redundant pair. All comparisons and tie-breaks are
/// integer and replay-stable.
fn select_discriminator_actions(
    actions: &[MechanismInformationGain],
    max_actions: usize,
    budget_units: u64,
) -> Vec<MechanismInformationGain> {
    if max_actions == 0 || actions.is_empty() {
        return Vec::new();
    }
    let mut states = vec![MechanismActionBatchState {
        selected: Vec::new(),
        cost_units: 0,
    }];
    for index in 0..actions.len() {
        let mut next = states.clone();
        for state in &states {
            let action_cost = u64::from(actions[index].cost_units);
            if state.selected.len() < max_actions
                && state.cost_units.saturating_add(action_cost) <= budget_units
            {
                let mut selected = state.selected.clone();
                selected.push(index);
                next.push(MechanismActionBatchState {
                    selected,
                    cost_units: state.cost_units.saturating_add(action_cost),
                });
            }
        }
        next.sort_by(|left, right| {
            mechanism_batch_score(right, actions)
                .cmp(&mechanism_batch_score(left, actions))
                .then_with(|| left.selected.cmp(&right.selected))
        });
        next.dedup_by(|left, right| left.selected == right.selected);
        next.truncate(MECHANISM_ACTION_BEAM_WIDTH);
        states = next;
    }
    let chosen = states
        .into_iter()
        .max_by(|left, right| {
            mechanism_batch_score(left, actions)
                .cmp(&mechanism_batch_score(right, actions))
                .then_with(|| right.selected.cmp(&left.selected))
        })
        .expect("mechanism action beam always retains an empty state");
    chosen
        .selected
        .into_iter()
        .map(|index| actions[index].clone())
        .collect()
}

fn digest_input(plan: &MechanismActionPlan) -> serde_json::Value {
    serde_json::json!({
        "feature_id": plan.feature_id,
        "output_schema": plan.output_schema,
        "source_discrimination_digest": plan.source_discrimination_digest,
        "model_system": plan.model_system,
        "modality": plan.modality,
        "budget_units": plan.budget_units,
        "budget_spent_units": plan.budget_spent_units,
        "action_order": plan.action_order,
        "candidates": plan.candidates,
        "uncertainty": plan.uncertainty,
        "negative_evidence": plan.negative_evidence,
    })
}

impl MechanismActionPlan {
    pub fn validate(&self) -> Result<(), MechanismActionPlannerError> {
        let computed_budget_spent_units = self
            .candidates
            .iter()
            .map(|candidate| u64::from(candidate.cost_units))
            .sum::<u64>();
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
            || self.budget_units == 0
            || self.budget_spent_units != computed_budget_spent_units
            || self.budget_spent_units > self.budget_units
            || self
                .candidates
                .iter()
                .any(|candidate| candidate.cost_units == 0)
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
    if config.max_actions == 0 || config.max_actions > MAX_ACTIONS || config.budget_units == 0 {
        return Err(MechanismActionPlannerError::InvalidRequest(
            "max_actions must be between 1 and 256 and budget_units must be positive".into(),
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
    actions = select_discriminator_actions(&actions, config.max_actions, config.budget_units);
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
    let budget_spent_units = candidates
        .iter()
        .map(|candidate| u64::from(candidate.cost_units))
        .sum::<u64>();
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
        budget_units: config.budget_units,
        budget_spent_units,
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
            budget_units: 4,
        };
        let first = compile_mechanism_action_plan(&discrimination, &config).unwrap();
        let second = compile_mechanism_action_plan(&discrimination, &config).unwrap();
        assert_eq!(first, second);
        assert_eq!(
            first.action_order,
            vec!["mechanism-assay:high", "mechanism-assay:low"]
        );
        assert_eq!(first.budget_units, 4);
        assert_eq!(first.budget_spent_units, 4);
        first.validate().unwrap();
    }

    #[test]
    fn mechanism_batch_beam_prefers_complementary_mechanisms() {
        let actions = vec![
            MechanismInformationGain {
                action_id: "shared-high".into(),
                feature_id: "f-shared-high".into(),
                mechanism_order: vec!["m1".into(), "m2".into()],
                expected_information_milli: 1_000_000,
                adjusted_information_milli: 1_000_000,
                measurement_uncertainty_milli: 100,
                feasibility_milli: 900,
                cost_units: 1,
            },
            MechanismInformationGain {
                action_id: "orthogonal-a".into(),
                feature_id: "f-orthogonal-a".into(),
                mechanism_order: vec!["m3".into()],
                expected_information_milli: 900_000,
                adjusted_information_milli: 900_000,
                measurement_uncertainty_milli: 100,
                feasibility_milli: 900,
                cost_units: 1,
            },
            MechanismInformationGain {
                action_id: "orthogonal-b".into(),
                feature_id: "f-orthogonal-b".into(),
                mechanism_order: vec!["m4".into()],
                expected_information_milli: 900_000,
                adjusted_information_milli: 900_000,
                measurement_uncertainty_milli: 100,
                feasibility_milli: 900,
                cost_units: 1,
            },
        ];
        let selected = select_discriminator_actions(&actions, 2, 2)
            .into_iter()
            .map(|action| action.action_id)
            .collect::<BTreeSet<_>>();
        assert_eq!(
            selected,
            BTreeSet::from(["orthogonal-a".to_string(), "orthogonal-b".to_string(),])
        );
    }

    #[test]
    fn mechanism_batch_budget_rejects_expensive_shortcut_for_complementary_pair() {
        let actions = vec![
            MechanismInformationGain {
                action_id: "expensive-shortcut".into(),
                feature_id: "f-expensive-shortcut".into(),
                mechanism_order: vec!["m1".into(), "m2".into(), "m3".into()],
                expected_information_milli: 1_000_000,
                adjusted_information_milli: 1_000_000,
                measurement_uncertainty_milli: 100,
                feasibility_milli: 900,
                cost_units: 6,
            },
            MechanismInformationGain {
                action_id: "cheap-a".into(),
                feature_id: "f-cheap-a".into(),
                mechanism_order: vec!["m1".into(), "m2".into()],
                expected_information_milli: 850_000,
                adjusted_information_milli: 850_000,
                measurement_uncertainty_milli: 100,
                feasibility_milli: 900,
                cost_units: 3,
            },
            MechanismInformationGain {
                action_id: "cheap-b".into(),
                feature_id: "f-cheap-b".into(),
                mechanism_order: vec!["m3".into(), "m4".into()],
                expected_information_milli: 850_000,
                adjusted_information_milli: 850_000,
                measurement_uncertainty_milli: 100,
                feasibility_milli: 900,
                cost_units: 3,
            },
        ];
        let selected = select_discriminator_actions(&actions, 3, 6);
        let selected_ids = selected
            .iter()
            .map(|action| action.action_id.as_str())
            .collect::<BTreeSet<_>>();
        assert_eq!(selected_ids, BTreeSet::from(["cheap-a", "cheap-b"]));
        assert_eq!(
            selected
                .iter()
                .map(|action| u64::from(action.cost_units))
                .sum::<u64>(),
            6
        );
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
                budget_units: 4,
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

    #[test]
    fn planner_prefers_orthogonal_mechanisms_over_a_near_tied_duplicate() {
        let actions = vec![
            MechanismInformationGain {
                action_id: "primary".into(),
                feature_id: "f-primary".into(),
                mechanism_order: vec!["m1".into(), "m2".into()],
                expected_information_milli: 900_000,
                adjusted_information_milli: 800_000,
                measurement_uncertainty_milli: 100,
                feasibility_milli: 900,
                cost_units: 2,
            },
            MechanismInformationGain {
                action_id: "redundant".into(),
                feature_id: "f-redundant".into(),
                mechanism_order: vec!["m1".into(), "m2".into()],
                expected_information_milli: 880_000,
                adjusted_information_milli: 790_000,
                measurement_uncertainty_milli: 100,
                feasibility_milli: 900,
                cost_units: 2,
            },
            MechanismInformationGain {
                action_id: "orthogonal".into(),
                feature_id: "f-orthogonal".into(),
                mechanism_order: vec!["m3".into(), "m4".into()],
                expected_information_milli: 820_000,
                adjusted_information_milli: 700_000,
                measurement_uncertainty_milli: 100,
                feasibility_milli: 900,
                cost_units: 2,
            },
        ];
        let selected = select_discriminator_actions(&actions, 2, 4);
        let selected_ids = selected
            .iter()
            .map(|action| action.action_id.as_str())
            .collect::<BTreeSet<_>>();
        assert_eq!(selected_ids.len(), 2);
        assert!(selected_ids.contains("primary"));
        assert!(selected_ids.contains("orthogonal"));
        assert!(!selected_ids.contains("redundant"));
    }
}
