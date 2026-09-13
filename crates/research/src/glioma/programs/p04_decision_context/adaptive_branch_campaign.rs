//! Adaptive scenario-branch campaign for preclinical glioma research.
//!
//! The robust branch planner can execute a branch and fail over, but a useful research engine
//! must also update its knowledge/context frontier from the evidence returned by that branch.
//! This controller repeats knowledge compilation, decision-context compilation, robust branch
//! planning, and branch execution under one hard budget. It never treats a scenario projection as
//! an observation: only validated local evidence returned by the caller-owned executor enters the
//! next round.

use super::branch_campaign::{
    execute_glioma_decision_branch_campaign, DecisionBranchCampaign,
    DecisionBranchCampaignDisposition, DecisionBranchCampaignError, DecisionBranchCampaignRequest,
};
use super::branch_planner::{
    plan_glioma_decision_branches, DecisionBranchPlan, DecisionBranchPlannerError,
    DecisionBranchPlannerRequest,
};
use super::campaign::DecisionContextCampaignExecutor;
use super::context_compiler::{
    compile_decision_context, DecisionContext, DecisionContextError, DecisionContextRequest,
};
use crate::glioma::evidence::EvidenceRecord;
use crate::glioma::programs::p02_evidence_knowledge::{
    compile_typed_knowledge, KnowledgeError, KnowledgeRequest, TypedKnowledge,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P04-F28";
pub const OUTPUT_SCHEMA: &str = "GliomaAdaptiveDecisionBranchCampaign1@1";
pub const MAX_ROUNDS: u16 = 32;
pub const MAX_RETRIES: u8 = 8;
pub const MAX_RECORDS: usize = 100_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdaptiveDecisionBranchCampaignRequest {
    pub knowledge: KnowledgeRequest,
    pub context: DecisionContextRequest,
    pub branches: DecisionBranchPlannerRequest,
    pub records: Vec<EvidenceRecord>,
    pub budget_units: u64,
    pub max_rounds: u16,
    pub max_retries: u8,
    pub stop_on_completed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdaptiveDecisionBranchCampaignRound {
    pub round: u16,
    pub knowledge: TypedKnowledge,
    pub context: DecisionContext,
    pub branch_plan: DecisionBranchPlan,
    pub campaign: DecisionBranchCampaign,
    pub records_added_order: Vec<String>,
    pub completed_added_order: Vec<String>,
    pub budget_before_units: u64,
    pub budget_after_units: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdaptiveDecisionBranchCampaignDisposition {
    Completed,
    Partial,
    BudgetBlocked,
    Failed,
    Unresolved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdaptiveDecisionBranchCampaignStopReason {
    Completed,
    BudgetExhausted,
    MaxRounds,
    ExecutorFailed,
    NoProgress,
    NoAdmissibleBranch,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdaptiveDecisionBranchCampaign {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub rounds: Vec<AdaptiveDecisionBranchCampaignRound>,
    pub records: Vec<EvidenceRecord>,
    pub completed_action_order: Vec<String>,
    pub final_knowledge: TypedKnowledge,
    pub final_context: DecisionContext,
    pub final_branch_plan: Option<DecisionBranchPlan>,
    pub simulation_only: bool,
    pub budget_spent_units: u64,
    pub remaining_budget_units: u64,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: AdaptiveDecisionBranchCampaignDisposition,
    pub stop_reason: AdaptiveDecisionBranchCampaignStopReason,
    pub next_operator_action: String,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AdaptiveDecisionBranchCampaignError {
    #[error("adaptive decision campaign request is invalid: {0}")]
    InvalidRequest(String),
    #[error("adaptive decision campaign knowledge compilation failed: {0}")]
    Knowledge(#[from] KnowledgeError),
    #[error("adaptive decision campaign context compilation failed: {0}")]
    Context(#[from] DecisionContextError),
    #[error("adaptive decision campaign branch planning failed: {0}")]
    BranchPlanning(#[from] DecisionBranchPlannerError),
    #[error("adaptive decision campaign branch execution failed: {0}")]
    BranchExecution(#[from] DecisionBranchCampaignError),
    #[error("adaptive decision campaign output is invalid: {0}")]
    InvalidOutput(String),
    #[error("adaptive decision campaign digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(output: &AdaptiveDecisionBranchCampaign) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "rounds": output.rounds,
        "records": output.records,
        "completed_action_order": output.completed_action_order,
        "final_knowledge": output.final_knowledge,
        "final_context": output.final_context,
        "final_branch_plan": output.final_branch_plan,
        "simulation_only": output.simulation_only,
        "budget_spent_units": output.budget_spent_units,
        "remaining_budget_units": output.remaining_budget_units,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
        "stop_reason": output.stop_reason,
        "next_operator_action": output.next_operator_action,
    })
}

fn validate_request(
    request: &AdaptiveDecisionBranchCampaignRequest,
) -> Result<(), AdaptiveDecisionBranchCampaignError> {
    if request.knowledge.objective.trim().is_empty()
        || request.context.objective != request.knowledge.objective
        || request.branches.objective != request.knowledge.objective
        || request.budget_units == 0
        || request.budget_units > u64::from(u32::MAX)
        || request.max_rounds == 0
        || request.max_rounds > MAX_ROUNDS
        || request.max_retries > MAX_RETRIES
        || request.records.len() > MAX_RECORDS
    {
        return Err(AdaptiveDecisionBranchCampaignError::InvalidRequest(
            "objective binding, positive u32-bounded budget, bounded rounds/retries, and record limits are required".into(),
        ));
    }
    Ok(())
}

fn disposition(
    campaign: Option<&DecisionBranchCampaign>,
    stop_reason: AdaptiveDecisionBranchCampaignStopReason,
) -> AdaptiveDecisionBranchCampaignDisposition {
    match campaign.map(|item| item.disposition) {
        Some(DecisionBranchCampaignDisposition::Completed) => {
            AdaptiveDecisionBranchCampaignDisposition::Completed
        }
        Some(DecisionBranchCampaignDisposition::BudgetBlocked) | None
            if matches!(
                stop_reason,
                AdaptiveDecisionBranchCampaignStopReason::BudgetExhausted
            ) =>
        {
            AdaptiveDecisionBranchCampaignDisposition::BudgetBlocked
        }
        Some(DecisionBranchCampaignDisposition::Failed) | None
            if matches!(
                stop_reason,
                AdaptiveDecisionBranchCampaignStopReason::ExecutorFailed
            ) =>
        {
            AdaptiveDecisionBranchCampaignDisposition::Failed
        }
        Some(DecisionBranchCampaignDisposition::Failed) => {
            AdaptiveDecisionBranchCampaignDisposition::Failed
        }
        Some(DecisionBranchCampaignDisposition::BudgetBlocked) => {
            AdaptiveDecisionBranchCampaignDisposition::BudgetBlocked
        }
        Some(DecisionBranchCampaignDisposition::Partial) => {
            AdaptiveDecisionBranchCampaignDisposition::Partial
        }
        Some(DecisionBranchCampaignDisposition::Held)
        | Some(DecisionBranchCampaignDisposition::Unresolved)
        | None => AdaptiveDecisionBranchCampaignDisposition::Unresolved,
    }
}

impl AdaptiveDecisionBranchCampaign {
    pub fn validate(&self) -> Result<(), AdaptiveDecisionBranchCampaignError> {
        let record_order = self
            .records
            .iter()
            .map(|record| record.evidence_id.clone())
            .collect::<Vec<_>>();
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || !canonical(&record_order)
            || !canonical(&self.completed_action_order)
            || !canonical(&self.negative_evidence)
            || !canonical(&self.uncertainty)
            || self.final_knowledge.objective != self.objective
            || self.final_context.objective != self.objective
            || self.next_operator_action.trim().is_empty()
        {
            return Err(AdaptiveDecisionBranchCampaignError::InvalidOutput(
                "identity, ordering, budget, objective, or operator-action invariant failed".into(),
            ));
        }
        self.final_knowledge.validate().map_err(|error| {
            AdaptiveDecisionBranchCampaignError::InvalidOutput(error.to_string())
        })?;
        self.final_context.validate().map_err(|error| {
            AdaptiveDecisionBranchCampaignError::InvalidOutput(error.to_string())
        })?;
        let mut seen_rounds = BTreeSet::new();
        let mut seen_records = BTreeSet::new();
        for round in &self.rounds {
            if round.round == 0
                || !seen_rounds.insert(round.round)
                || round.knowledge.objective != self.objective
                || round.context.objective != self.objective
                || round.branch_plan.objective != self.objective
                || round.campaign.objective != self.objective
                || !canonical(&round.records_added_order)
                || !canonical(&round.completed_added_order)
                || round.budget_after_units > round.budget_before_units
                || round
                    .budget_before_units
                    .saturating_sub(round.budget_after_units)
                    != round.campaign.budget_spent_units
            {
                return Err(AdaptiveDecisionBranchCampaignError::InvalidOutput(
                    "round identity, objective, ordering, branch binding, or budget invariant failed".into(),
                ));
            }
            for record_id in &round.records_added_order {
                if !seen_records.insert(record_id.clone()) {
                    return Err(AdaptiveDecisionBranchCampaignError::InvalidOutput(
                        "a record was added in more than one adaptive round".into(),
                    ));
                }
            }
            round.knowledge.validate().map_err(|error| {
                AdaptiveDecisionBranchCampaignError::InvalidOutput(error.to_string())
            })?;
            round.context.validate().map_err(|error| {
                AdaptiveDecisionBranchCampaignError::InvalidOutput(error.to_string())
            })?;
            round.branch_plan.validate().map_err(|error| {
                AdaptiveDecisionBranchCampaignError::InvalidOutput(error.to_string())
            })?;
            round.campaign.validate().map_err(|error| {
                AdaptiveDecisionBranchCampaignError::InvalidOutput(error.to_string())
            })?;
        }
        if let Some(plan) = &self.final_branch_plan {
            plan.validate().map_err(|error| {
                AdaptiveDecisionBranchCampaignError::InvalidOutput(error.to_string())
            })?;
            if plan.context_digest != self.final_context.digest {
                return Err(AdaptiveDecisionBranchCampaignError::InvalidOutput(
                    "final branch plan is not bound to final decision context".into(),
                ));
            }
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| AdaptiveDecisionBranchCampaignError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(AdaptiveDecisionBranchCampaignError::InvalidOutput(
                "adaptive decision campaign digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

/// Recompile knowledge/context and replan robust branches after each returned evidence batch.
pub fn execute_glioma_adaptive_decision_branch_campaign<E: DecisionContextCampaignExecutor>(
    request: &AdaptiveDecisionBranchCampaignRequest,
    executor: &mut E,
) -> Result<AdaptiveDecisionBranchCampaign, AdaptiveDecisionBranchCampaignError> {
    validate_request(request)?;
    let mut records = request.records.clone();
    records.sort_by(|left, right| left.evidence_id.cmp(&right.evidence_id));
    let mut record_ids = records
        .iter()
        .map(|record| record.evidence_id.clone())
        .collect::<BTreeSet<_>>();
    if record_ids.len() != records.len() {
        return Err(AdaptiveDecisionBranchCampaignError::InvalidRequest(
            "initial evidence ids must be unique".into(),
        ));
    }
    let mut completed = request.branches.completed_action_order.clone();
    completed.sort();
    let mut completed_set = completed.iter().cloned().collect::<BTreeSet<_>>();
    let mut remaining = request.budget_units;
    let mut spent = 0_u64;
    let mut rounds = Vec::new();
    let mut stop_reason = AdaptiveDecisionBranchCampaignStopReason::MaxRounds;
    let mut last_campaign = None;
    for round_number in 1..=request.max_rounds {
        if remaining == 0 {
            stop_reason = AdaptiveDecisionBranchCampaignStopReason::BudgetExhausted;
            break;
        }
        let knowledge = compile_typed_knowledge(&request.knowledge, &records)?;
        let context = compile_decision_context(&request.context, &knowledge)?;
        let mut branch_request = request.branches.clone();
        branch_request.completed_action_order = completed.to_vec();
        branch_request.budget_units = remaining.min(u32::MAX as u64) as u32;
        let branch_plan = plan_glioma_decision_branches(&branch_request, &context)?;
        let campaign_request = DecisionBranchCampaignRequest {
            objective: request.knowledge.objective.clone(),
            knowledge: knowledge.clone(),
            context: context.clone(),
            branch_plan: branch_plan.clone(),
            completed_action_order: completed.to_vec(),
            budget_units: remaining,
            max_branches: request.branches.max_branches,
            max_retries: request.max_retries,
            stop_on_completed: request.stop_on_completed,
        };
        let campaign = execute_glioma_decision_branch_campaign(&campaign_request, executor)?;
        let budget_before = remaining;
        let cost = campaign.budget_spent_units.min(remaining);
        remaining = remaining.saturating_sub(cost);
        spent = spent.saturating_add(cost);
        let mut records_added_order = Vec::new();
        for record in &campaign.records {
            if !record_ids.insert(record.evidence_id.clone()) {
                return Err(AdaptiveDecisionBranchCampaignError::InvalidRequest(
                    "executor returned a duplicate evidence id across adaptive rounds".into(),
                ));
            }
            records_added_order.push(record.evidence_id.clone());
            records.push(record.clone());
        }
        records.sort_by(|left, right| left.evidence_id.cmp(&right.evidence_id));
        records_added_order.sort();
        let mut completed_added_order = Vec::new();
        for action_id in &campaign.completed_action_order {
            if completed_set.insert(action_id.clone()) {
                completed_added_order.push(action_id.clone());
            }
        }
        completed.extend(completed_added_order.iter().cloned());
        completed.sort();
        let made_progress = !records_added_order.is_empty() || !completed_added_order.is_empty();
        rounds.push(AdaptiveDecisionBranchCampaignRound {
            round: round_number,
            knowledge,
            context,
            branch_plan,
            campaign: campaign.clone(),
            records_added_order,
            completed_added_order,
            budget_before_units: budget_before,
            budget_after_units: remaining,
        });
        last_campaign = Some(campaign.clone());
        if request.stop_on_completed
            && campaign.disposition == DecisionBranchCampaignDisposition::Completed
        {
            stop_reason = AdaptiveDecisionBranchCampaignStopReason::Completed;
            break;
        }
        if campaign.disposition == DecisionBranchCampaignDisposition::Failed {
            stop_reason = AdaptiveDecisionBranchCampaignStopReason::ExecutorFailed;
            break;
        }
        if !made_progress {
            stop_reason = AdaptiveDecisionBranchCampaignStopReason::NoProgress;
            break;
        }
        if remaining == 0 {
            stop_reason = AdaptiveDecisionBranchCampaignStopReason::BudgetExhausted;
            break;
        }
        if round_number == request.max_rounds {
            stop_reason = AdaptiveDecisionBranchCampaignStopReason::MaxRounds;
        }
    }
    let final_knowledge = compile_typed_knowledge(&request.knowledge, &records)?;
    let final_context = compile_decision_context(&request.context, &final_knowledge)?;
    let mut final_branch_request = request.branches.clone();
    final_branch_request.completed_action_order = completed.clone();
    final_branch_request.budget_units = remaining.min(u32::MAX as u64) as u32;
    let final_branch_plan = if remaining > 0 {
        Some(plan_glioma_decision_branches(
            &final_branch_request,
            &final_context,
        )?)
    } else {
        None
    };
    let disposition = disposition(last_campaign.as_ref(), stop_reason);
    let mut negative_evidence = final_knowledge.negative_evidence_order.to_vec();
    let mut uncertainty = final_knowledge.uncertainty_order.clone();
    if let Some(campaign) = &last_campaign {
        negative_evidence.extend(campaign.negative_evidence_order.iter().cloned());
        uncertainty.extend(campaign.uncertainty_order.iter().cloned());
    }
    negative_evidence.sort();
    negative_evidence.dedup();
    uncertainty.sort();
    uncertainty.dedup();
    let next_operator_action = match disposition {
        AdaptiveDecisionBranchCampaignDisposition::Completed => {
            "review the completed branch evidence and preserve the observed result before promoting any downstream mechanism claim".into()
        }
        AdaptiveDecisionBranchCampaignDisposition::Partial => {
            "recompile from the returned evidence and execute only the next dependency-closed branch frontier".into()
        }
        AdaptiveDecisionBranchCampaignDisposition::BudgetBlocked => {
            "increase or reallocate the bounded research budget before another branch is dispatched".into()
        }
        AdaptiveDecisionBranchCampaignDisposition::Failed => {
            "repair the local executor boundary, preserve the failed branch evidence, and retry only through an approved adapter".into()
        }
        AdaptiveDecisionBranchCampaignDisposition::Unresolved => {
            "resolve the remaining evidence gaps or branch omissions; scenario projections are not observations".into()
        }
    };
    let mut output = AdaptiveDecisionBranchCampaign {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.knowledge.objective.clone(),
        rounds,
        records,
        completed_action_order: completed,
        final_knowledge,
        final_context,
        final_branch_plan,
        simulation_only: true,
        budget_spent_units: spent,
        remaining_budget_units: remaining,
        negative_evidence,
        uncertainty,
        disposition,
        stop_reason,
        next_operator_action,
        digest: ContentHash::of_bytes(b"unsealed-glioma-adaptive-decision-branch-campaign"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| AdaptiveDecisionBranchCampaignError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

pub fn execute_glioma_adaptive_decision_branch_campaign_dry_run(
    request: &AdaptiveDecisionBranchCampaignRequest,
) -> Result<AdaptiveDecisionBranchCampaign, AdaptiveDecisionBranchCampaignError> {
    let mut executor = super::campaign::DryRunDecisionContextCampaignExecutor;
    execute_glioma_adaptive_decision_branch_campaign(request, &mut executor)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::evidence::{EvidenceRecord, EvidenceSourceKind, EvidenceState};
    use crate::glioma::programs::p04_decision_context::{
        compile_decision_context, DecisionScenario, DecisionScenarioOutcome,
        DryRunDecisionContextCampaignExecutor,
    };
    use crate::glioma_engine::{
        GliomaModality, GliomaModelSystem, GliomaSelectionWeights, LocalArtifactRef,
    };
    use std::collections::BTreeSet;

    fn request() -> AdaptiveDecisionBranchCampaignRequest {
        let objective = "rank invasion mechanisms".to_string();
        let record = EvidenceRecord {
            evidence_id: "adaptive-branch-e1".into(),
            source_artifact: LocalArtifactRef {
                artifact_id: "adaptive-branch-a1".into(),
                content_hash: ContentHash::of_bytes(b"adaptive-branch-a1"),
                content_type: "application/vnd.aurora.glioma-evidence+json".into(),
                local_only: true,
                contains_human_data: false,
                contains_direct_identifiers: false,
            },
            source_kind: EvidenceSourceKind::Dataset,
            claim: "EGFR signaling increases invasion".into(),
            scope: "preclinical glioma".into(),
            modality: GliomaModality::Genomics,
            model_system: Some(GliomaModelSystem::Organoid),
            state: EvidenceState::Supported,
            relevance_milli: 900,
            quality_milli: 900,
            reproducibility_milli: 900,
            release_epoch: 1,
        };
        let knowledge = KnowledgeRequest {
            objective: objective.clone(),
            required_modalities: BTreeSet::from([GliomaModality::Genomics]),
            required_model_systems: BTreeSet::from([GliomaModelSystem::Organoid]),
            min_support_milli: 700,
            min_sources_per_claim: 1,
            max_claims: 8,
        };
        let typed = compile_typed_knowledge(&knowledge, std::slice::from_ref(&record)).unwrap();
        let context_request = DecisionContextRequest {
            objective: objective.clone(),
            max_actions: 8,
            default_cost_units: 1,
        };
        let context = compile_decision_context(&context_request, &typed).unwrap();
        let candidate = context.actions[0].candidate.clone();
        let branches = DecisionBranchPlannerRequest {
            objective: objective.clone(),
            candidates: vec![candidate.clone()],
            completed_action_order: Vec::new(),
            scenarios: vec![
                DecisionScenario {
                    scenario_id: "high".into(),
                    probability_milli: 600,
                    outcomes: vec![DecisionScenarioOutcome {
                        action_id: candidate.action_id.clone(),
                        value_milli: 900,
                        uncertainty_milli: 100,
                        failure_probability_milli: 50,
                    }],
                },
                DecisionScenario {
                    scenario_id: "low".into(),
                    probability_milli: 400,
                    outcomes: vec![DecisionScenarioOutcome {
                        action_id: candidate.action_id,
                        value_milli: 700,
                        uncertainty_milli: 200,
                        failure_probability_milli: 100,
                    }],
                },
            ],
            budget_units: 2,
            max_actions_per_branch: 4,
            max_branches: 4,
            beam_width: 8,
            minimum_robustness_milli: 0,
            uncertainty_penalty_milli: 100,
            failure_penalty_milli: 100,
            selection_weights: GliomaSelectionWeights::default(),
        };
        AdaptiveDecisionBranchCampaignRequest {
            knowledge,
            context: context_request,
            branches,
            records: vec![record],
            budget_units: 2,
            max_rounds: 3,
            max_retries: 1,
            stop_on_completed: true,
        }
    }

    #[test]
    fn adaptive_campaign_recompiles_and_replays_stably() {
        let request = request();
        let first = execute_glioma_adaptive_decision_branch_campaign_dry_run(&request).unwrap();
        let second = execute_glioma_adaptive_decision_branch_campaign_dry_run(&request).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.rounds.len(), 1);
        assert!(!first.records.is_empty());
        assert!(!first.completed_action_order.is_empty());
        assert!(first.remaining_budget_units < request.budget_units);
        first.validate().unwrap();
    }

    #[test]
    fn adaptive_campaign_rejects_objective_binding_drift() {
        let mut request = request();
        request.context.objective = "different objective".into();
        let mut executor = DryRunDecisionContextCampaignExecutor;
        let error =
            execute_glioma_adaptive_decision_branch_campaign(&request, &mut executor).unwrap_err();
        assert!(matches!(
            error,
            AdaptiveDecisionBranchCampaignError::InvalidRequest(_)
        ));
    }
}
