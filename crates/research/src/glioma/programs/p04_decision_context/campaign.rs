//! Bounded autonomous question-to-action campaigns for preclinical glioma research.
//!
//! P04 turns a typed knowledge graph into a decision context, selects a dependency-safe local
//! action portfolio, dispatches it through an institution-owned executor, and recompiles the
//! context from returned evidence. This is the closed-loop product surface between evidence and
//! research execution: plans are never treated as observations, negative/contradictory evidence
//! remains visible, and every round has a deterministic replay boundary.

use super::action_bridge::{
    plan_decision_actions, DecisionActionPlan, DecisionActionPlanDisposition,
    DecisionActionPlanRequest,
};
use super::context_compiler::{
    compile_decision_context, DecisionAction, DecisionActionKind, DecisionContext,
    DecisionContextDisposition, DecisionContextRequest,
};
use crate::glioma::evidence::{EvidenceRecord, EvidenceSourceKind, EvidenceState};
use crate::glioma::programs::p02_evidence_knowledge::{
    compile_typed_knowledge, KnowledgeDisposition, KnowledgeRequest, TypedKnowledge,
};
use crate::glioma_engine::LocalArtifactRef;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P04-F20";
pub const OUTPUT_SCHEMA: &str = "GliomaDecisionContextCampaign1@1";
pub const MAX_ROUNDS: u16 = 24;
pub const MAX_RETRIES: u8 = 6;
pub const MAX_RECORDS: usize = 100_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionContextCampaignRequest {
    pub knowledge: KnowledgeRequest,
    pub context: DecisionContextRequest,
    pub action_plan: DecisionActionPlanRequest,
    pub records: Vec<EvidenceRecord>,
    pub budget_units: u64,
    pub max_rounds: u16,
    pub max_retries: u8,
    pub stop_on_qualified: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecisionContextCampaignExecutionFailure {
    pub reason: String,
    pub retryable: bool,
}

/// Institution-local assay, replication, computation, and literature adapters implement this
/// seam. The controller supplies a typed action and receives only validated local evidence rows.
pub trait DecisionContextCampaignExecutor {
    fn execute_action(
        &mut self,
        action: &DecisionAction,
        context: &DecisionContext,
        knowledge: &TypedKnowledge,
        attempt: u8,
    ) -> Result<Vec<EvidenceRecord>, DecisionContextCampaignExecutionFailure>;
}

/// Deterministic MCP sandbox adapter. It emits metadata-only synthetic evidence and preserves the
/// action's scientific posture (contradiction, negative result, uncertainty, or support).
#[derive(Debug, Default)]
pub struct DryRunDecisionContextCampaignExecutor;

impl DecisionContextCampaignExecutor for DryRunDecisionContextCampaignExecutor {
    fn execute_action(
        &mut self,
        action: &DecisionAction,
        context: &DecisionContext,
        knowledge: &TypedKnowledge,
        attempt: u8,
    ) -> Result<Vec<EvidenceRecord>, DecisionContextCampaignExecutionFailure> {
        let claim = knowledge
            .claims
            .iter()
            .find(|claim| claim.claim_id == action.claim_id)
            .ok_or_else(|| DecisionContextCampaignExecutionFailure {
                reason: format!("claim {} is absent from typed knowledge", action.claim_id),
                retryable: false,
            })?;
        if context
            .actions
            .iter()
            .all(|candidate| candidate.action_id != action.action_id)
        {
            return Err(DecisionContextCampaignExecutionFailure {
                reason: format!(
                    "action {} is absent from decision context",
                    action.action_id
                ),
                retryable: false,
            });
        }
        let (source_kind, state) = match action.kind {
            DecisionActionKind::CloseCoverage => {
                (EvidenceSourceKind::Dataset, EvidenceState::Supported)
            }
            DecisionActionKind::ResolveContradiction => {
                (EvidenceSourceKind::Replication, EvidenceState::Contradicted)
            }
            DecisionActionKind::FalsifyNegative => {
                (EvidenceSourceKind::Assay, EvidenceState::Negative)
            }
            DecisionActionKind::ResolveEvidence => {
                (EvidenceSourceKind::Literature, EvidenceState::Unknown)
            }
            DecisionActionKind::ValidateMechanism => {
                (EvidenceSourceKind::Computation, EvidenceState::Supported)
            }
        };
        let content_hash = ContentHash::of_value(&serde_json::json!({
            "action_id": action.action_id,
            "claim_id": action.claim_id,
            "attempt": attempt,
            "simulation_only": true,
        }))
        .map_err(|error| DecisionContextCampaignExecutionFailure {
            reason: format!("dry-run decision digest failed: {error}"),
            retryable: false,
        })?;
        Ok(vec![EvidenceRecord {
            evidence_id: format!("dry-run-decision:{}", action.action_id),
            source_artifact: LocalArtifactRef {
                artifact_id: format!("dry-run-decision:{}", action.action_id),
                content_hash,
                content_type: "application/vnd.aurora.glioma.decision-context+json".into(),
                local_only: true,
                contains_human_data: false,
                contains_direct_identifiers: false,
            },
            source_kind,
            claim: claim.statement.clone(),
            scope: claim.scope.clone(),
            modality: action.target_modality,
            model_system: Some(action.target_model_system),
            state,
            relevance_milli: 800,
            quality_milli: 800,
            reproducibility_milli: 800,
            release_epoch: 1,
        }])
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionContextCampaignRound {
    pub round: u16,
    pub action_order: Vec<String>,
    pub selected_order: Vec<String>,
    pub deferred_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub completed_order: Vec<String>,
    pub failed_order: Vec<String>,
    pub accepted_evidence_order: Vec<String>,
    pub knowledge: TypedKnowledge,
    pub context: DecisionContext,
    pub action_plan: DecisionActionPlan,
    pub cost_units: u64,
    pub budget_before_units: u64,
    pub budget_after_units: u64,
    pub retry_count: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecisionContextCampaignDisposition {
    Qualified,
    Partial,
    BudgetBlocked,
    Failed,
    NoActions,
    Unresolved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecisionContextCampaignStopReason {
    Qualified,
    BudgetExhausted,
    NoActions,
    MaxRounds,
    ExecutorFailed,
    NoProgress,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionContextCampaign {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub rounds: Vec<DecisionContextCampaignRound>,
    pub records: Vec<EvidenceRecord>,
    pub completed_order: Vec<String>,
    pub failed_order: Vec<String>,
    pub accepted_evidence_order: Vec<String>,
    pub retry_count: u32,
    pub budget_spent_units: u64,
    pub remaining_budget_units: u64,
    pub final_knowledge: TypedKnowledge,
    pub final_context: DecisionContext,
    pub final_action_plan: DecisionActionPlan,
    pub omission_order: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub simulation_only: bool,
    pub disposition: DecisionContextCampaignDisposition,
    pub stop_reason: DecisionContextCampaignStopReason,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum DecisionContextCampaignError {
    #[error("decision-context campaign request is invalid: {0}")]
    InvalidRequest(String),
    #[error("decision-context campaign planning failed: {0}")]
    Planning(String),
    #[error("decision-context campaign execution failed: {0}")]
    Execution(String),
    #[error("decision-context campaign output is invalid: {0}")]
    InvalidOutput(String),
    #[error("decision-context campaign digest failed: {0}")]
    Digest(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(campaign: &DecisionContextCampaign) -> serde_json::Value {
    serde_json::json!({
        "feature_id": campaign.feature_id,
        "output_schema": campaign.output_schema,
        "objective": campaign.objective,
        "rounds": campaign.rounds,
        "records": campaign.records,
        "completed_order": campaign.completed_order,
        "failed_order": campaign.failed_order,
        "accepted_evidence_order": campaign.accepted_evidence_order,
        "retry_count": campaign.retry_count,
        "budget_spent_units": campaign.budget_spent_units,
        "remaining_budget_units": campaign.remaining_budget_units,
        "final_knowledge": campaign.final_knowledge,
        "final_context": campaign.final_context,
        "final_action_plan": campaign.final_action_plan,
        "omission_order": campaign.omission_order,
        "negative_evidence": campaign.negative_evidence,
        "uncertainty": campaign.uncertainty,
        "simulation_only": campaign.simulation_only,
        "disposition": campaign.disposition,
        "stop_reason": campaign.stop_reason,
    })
}

fn validate_records(records: &[EvidenceRecord]) -> Result<(), DecisionContextCampaignError> {
    if records.len() > MAX_RECORDS {
        return Err(DecisionContextCampaignError::InvalidRequest(
            "evidence-record bound exceeded".into(),
        ));
    }
    let mut ids = BTreeSet::new();
    for record in records {
        record
            .source_artifact
            .validate()
            .map_err(|error| DecisionContextCampaignError::InvalidRequest(error.to_string()))?;
        if record.evidence_id.trim().is_empty()
            || record.claim.trim().is_empty()
            || record.scope.trim().is_empty()
            || record.relevance_milli > 1_000
            || record.quality_milli > 1_000
            || record.reproducibility_milli > 1_000
            || !ids.insert(record.evidence_id.clone())
        {
            return Err(DecisionContextCampaignError::InvalidRequest(
                "evidence identity, claim, scope, scores, artifact, or uniqueness is invalid"
                    .into(),
            ));
        }
    }
    Ok(())
}

fn validate_request(
    request: &DecisionContextCampaignRequest,
) -> Result<(), DecisionContextCampaignError> {
    if request.budget_units == 0
        || request.max_rounds == 0
        || request.max_rounds > MAX_ROUNDS
        || request.max_retries > MAX_RETRIES
        || request.action_plan.selection.budget_units == 0
        || request.action_plan.selection.max_actions == 0
    {
        return Err(DecisionContextCampaignError::InvalidRequest(
            "positive budget, bounded rounds/retries, and a runnable selection policy are required"
                .into(),
        ));
    }
    if request.knowledge.objective.trim() != request.context.objective.trim()
        || request.context.objective.trim() != request.action_plan.objective.trim()
    {
        return Err(DecisionContextCampaignError::InvalidRequest(
            "knowledge, decision-context, and action-plan objectives must match".into(),
        ));
    }
    validate_records(&request.records)?;
    let knowledge = compile_typed_knowledge(&request.knowledge, &request.records)
        .map_err(|error| DecisionContextCampaignError::InvalidRequest(error.to_string()))?;
    let context = compile_decision_context(&request.context, &knowledge)
        .map_err(|error| DecisionContextCampaignError::InvalidRequest(error.to_string()))?;
    let mut plan_request = request.action_plan.clone();
    plan_request.selection.budget_units = plan_request
        .selection
        .budget_units
        .min(request.budget_units.min(u64::from(u32::MAX)) as u32);
    plan_decision_actions(&plan_request, &context)
        .map_err(|error| DecisionContextCampaignError::InvalidRequest(error.to_string()))?;
    Ok(())
}

fn replace_records(records: &mut Vec<EvidenceRecord>, replacements: Vec<EvidenceRecord>) {
    for replacement in replacements {
        if let Some(existing) = records
            .iter_mut()
            .find(|record| record.evidence_id == replacement.evidence_id)
        {
            *existing = replacement;
        } else {
            records.push(replacement);
        }
    }
    records.sort_by(|left, right| left.evidence_id.cmp(&right.evidence_id));
}

fn validate_returned_records(
    records: &[EvidenceRecord],
    action: &DecisionAction,
    knowledge: &TypedKnowledge,
) -> Result<(), DecisionContextCampaignError> {
    if records.is_empty() {
        return Err(DecisionContextCampaignError::Execution(format!(
            "executor returned no evidence for action {}",
            action.action_id
        )));
    }
    let claim = knowledge
        .claims
        .iter()
        .find(|claim| claim.claim_id == action.claim_id)
        .ok_or_else(|| {
            DecisionContextCampaignError::Execution(format!(
                "action {} references an absent claim {}",
                action.action_id, action.claim_id
            ))
        })?;
    validate_records(records).map_err(|error| {
        DecisionContextCampaignError::Execution(format!(
            "executor returned invalid evidence for {}: {error}",
            action.action_id
        ))
    })?;
    if records.iter().any(|record| {
        record.claim.trim() != claim.statement.trim()
            || record.scope.trim() != claim.scope.trim()
            || record.modality != action.target_modality
            || record.model_system != Some(action.target_model_system)
    }) {
        return Err(DecisionContextCampaignError::Execution(format!(
            "executor returned evidence outside action {} target contract",
            action.action_id
        )));
    }
    Ok(())
}

fn bounded_plan_request(
    request: &DecisionContextCampaignRequest,
    completed: &BTreeSet<String>,
    _remaining: u64,
) -> DecisionActionPlanRequest {
    let selection = request.action_plan.selection.clone();
    // Keep the selector's scientific portfolio policy intact even when the campaign budget is
    // exhausted. The controller compares the selected portfolio cost with the remaining campaign
    // budget and emits an explicit BudgetExhausted stop instead of turning it into NoActions.
    DecisionActionPlanRequest {
        objective: request.action_plan.objective.clone(),
        completed_action_order: completed.iter().cloned().collect(),
        selection,
    }
}

fn selected_cost(context: &DecisionContext, selected: &[String]) -> u64 {
    selected
        .iter()
        .filter_map(|id| {
            context
                .actions
                .iter()
                .find(|action| &action.action_id == id)
                .map(|action| u64::from(action.candidate.cost_units))
        })
        .sum()
}

impl DecisionContextCampaign {
    pub fn validate(&self) -> Result<(), DecisionContextCampaignError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.rounds.len() > MAX_ROUNDS as usize
            || !canonical(&self.completed_order)
            || !canonical(&self.failed_order)
            || !canonical(&self.accepted_evidence_order)
            || self
                .completed_order
                .iter()
                .any(|id| self.failed_order.binary_search(id).is_ok())
        {
            return Err(DecisionContextCampaignError::InvalidOutput(
                "identity, canonical partitions, or campaign fields are invalid".into(),
            ));
        }
        validate_records(&self.records)
            .map_err(|error| DecisionContextCampaignError::InvalidOutput(error.to_string()))?;
        self.final_knowledge
            .validate()
            .map_err(|error| DecisionContextCampaignError::InvalidOutput(error.to_string()))?;
        self.final_context
            .validate()
            .map_err(|error| DecisionContextCampaignError::InvalidOutput(error.to_string()))?;
        self.final_action_plan
            .validate()
            .map_err(|error| DecisionContextCampaignError::InvalidOutput(error.to_string()))?;
        if self.final_knowledge.objective != self.objective
            || self.final_context.objective != self.objective
            || self.final_action_plan.objective != self.objective
            || self.final_action_plan.context_digest != self.final_context.digest
        {
            return Err(DecisionContextCampaignError::InvalidOutput(
                "final knowledge, context, and action plan do not reconcile".into(),
            ));
        }
        let mut round_ids = BTreeSet::new();
        let mut spent = 0_u64;
        let mut retries = 0_u32;
        for round in &self.rounds {
            if round.round == 0
                || !round_ids.insert(round.round)
                || !canonical(&round.action_order)
                || !canonical(&round.selected_order)
                || !canonical(&round.deferred_order)
                || !canonical(&round.blocked_order)
                || !canonical(&round.completed_order)
                || !canonical(&round.failed_order)
                || !canonical(&round.accepted_evidence_order)
                || round.budget_after_units > round.budget_before_units
                || round.cost_units != round.budget_before_units - round.budget_after_units
                || round.action_order != round.action_plan.action_order
                || round.selected_order != round.action_plan.selected_order
                || round.deferred_order != round.action_plan.deferred_order
                || round.blocked_order != round.action_plan.blocked_order
            {
                return Err(DecisionContextCampaignError::InvalidOutput(
                    "round ordering, budget, or nested action-plan invariants are invalid".into(),
                ));
            }
            round
                .knowledge
                .validate()
                .map_err(|error| DecisionContextCampaignError::InvalidOutput(error.to_string()))?;
            round
                .context
                .validate()
                .map_err(|error| DecisionContextCampaignError::InvalidOutput(error.to_string()))?;
            round
                .action_plan
                .validate()
                .map_err(|error| DecisionContextCampaignError::InvalidOutput(error.to_string()))?;
            if round.action_plan.context_digest != round.context.digest
                || round.action_plan.objective != round.context.objective
            {
                return Err(DecisionContextCampaignError::InvalidOutput(
                    "round action plan is not bound to its context".into(),
                ));
            }
            spent = spent.saturating_add(round.cost_units);
            retries = retries.saturating_add(round.retry_count);
        }
        if spent != self.budget_spent_units || retries != self.retry_count {
            return Err(DecisionContextCampaignError::InvalidOutput(
                "budget or retry count does not reconcile with rounds".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| DecisionContextCampaignError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(DecisionContextCampaignError::InvalidOutput(
                "campaign digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

/// Execute bounded question-to-action rounds, recompiling P04 context after every accepted local
/// evidence response and never promoting an unresolved claim merely because an action was planned.
pub fn execute_glioma_decision_context_campaign<E: DecisionContextCampaignExecutor>(
    request: &DecisionContextCampaignRequest,
    executor: &mut E,
) -> Result<DecisionContextCampaign, DecisionContextCampaignError> {
    validate_request(request)?;
    let mut records = request.records.clone();
    let mut rounds = Vec::new();
    let mut completed = request
        .action_plan
        .completed_action_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut failed = BTreeSet::new();
    let mut accepted_evidence = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut budget_spent = 0_u64;
    let mut retry_count = 0_u32;
    let mut stop_reason = DecisionContextCampaignStopReason::MaxRounds;

    for round_number in 1..=request.max_rounds {
        let knowledge = compile_typed_knowledge(&request.knowledge, &records)
            .map_err(|error| DecisionContextCampaignError::Planning(error.to_string()))?;
        let context = compile_decision_context(&request.context, &knowledge)
            .map_err(|error| DecisionContextCampaignError::Planning(error.to_string()))?;
        let remaining = request.budget_units.saturating_sub(budget_spent);
        let plan_request = bounded_plan_request(request, &completed, remaining);
        let action_plan = plan_decision_actions(&plan_request, &context)
            .map_err(|error| DecisionContextCampaignError::Planning(error.to_string()))?;
        omissions.extend(context.omission_order.iter().cloned());
        omissions.extend(action_plan.omission_order.iter().cloned());
        negative.extend(knowledge.negative_evidence_order.iter().cloned());
        uncertainty.extend(knowledge.uncertainty_order.iter().cloned());
        uncertainty.extend(context.uncertainty_order.iter().cloned());

        if request.stop_on_qualified
            && action_plan.disposition == DecisionActionPlanDisposition::Qualified
            && action_plan.selected_order.is_empty()
        {
            stop_reason = DecisionContextCampaignStopReason::Qualified;
            break;
        }
        if remaining == 0 {
            stop_reason = DecisionContextCampaignStopReason::BudgetExhausted;
            break;
        }
        if action_plan.selected_order.is_empty() {
            stop_reason = DecisionContextCampaignStopReason::NoActions;
            break;
        }
        let planned_cost = selected_cost(&context, &action_plan.selected_order);
        if planned_cost == 0 || planned_cost > remaining {
            stop_reason = DecisionContextCampaignStopReason::BudgetExhausted;
            break;
        }

        let before_budget = remaining;
        let mut completed_round = Vec::new();
        let mut failed_round = Vec::new();
        let mut accepted_round = Vec::new();
        let mut retry_round = 0_u32;
        let mut progress = false;
        for action_id in &action_plan.selected_order {
            let action = context
                .actions
                .iter()
                .find(|action| &action.action_id == action_id)
                .ok_or_else(|| {
                    DecisionContextCampaignError::Planning(format!(
                        "selected action {action_id} is absent from context"
                    ))
                })?;
            let mut accepted = None;
            for attempt in 1..=request.max_retries.saturating_add(1) {
                match executor.execute_action(action, &context, &knowledge, attempt) {
                    Ok(returned) => {
                        validate_returned_records(&returned, action, &knowledge)?;
                        accepted = Some(returned);
                        break;
                    }
                    Err(error) => {
                        if error.reason.trim().is_empty() {
                            return Err(DecisionContextCampaignError::Execution(
                                "executor returned an empty failure reason".into(),
                            ));
                        }
                        if error.retryable && attempt <= request.max_retries {
                            retry_count = retry_count.saturating_add(1);
                            retry_round = retry_round.saturating_add(1);
                            continue;
                        }
                        failed_round.push(action.action_id.clone());
                        failed.insert(action.action_id.clone());
                        break;
                    }
                }
            }
            if let Some(returned) = accepted {
                completed_round.push(action.action_id.clone());
                completed.insert(action.action_id.clone());
                for record in &returned {
                    accepted_round.push(record.evidence_id.clone());
                    accepted_evidence.insert(record.evidence_id.clone());
                }
                replace_records(&mut records, returned);
                progress = true;
            } else {
                break;
            }
        }
        completed_round.sort();
        failed_round.sort();
        accepted_round.sort();
        budget_spent = budget_spent.saturating_add(planned_cost);
        let after_budget = request.budget_units.saturating_sub(budget_spent);
        let updated_knowledge = compile_typed_knowledge(&request.knowledge, &records)
            .map_err(|error| DecisionContextCampaignError::Planning(error.to_string()))?;
        let updated_context = compile_decision_context(&request.context, &updated_knowledge)
            .map_err(|error| DecisionContextCampaignError::Planning(error.to_string()))?;
        omissions.extend(updated_context.omission_order.iter().cloned());
        negative.extend(updated_knowledge.negative_evidence_order.iter().cloned());
        uncertainty.extend(updated_knowledge.uncertainty_order.iter().cloned());
        uncertainty.extend(updated_context.uncertainty_order.iter().cloned());
        rounds.push(DecisionContextCampaignRound {
            round: round_number,
            action_order: action_plan.action_order.clone(),
            selected_order: action_plan.selected_order.clone(),
            deferred_order: action_plan.deferred_order.clone(),
            blocked_order: action_plan.blocked_order.clone(),
            completed_order: completed_round,
            failed_order: failed_round.clone(),
            accepted_evidence_order: accepted_round,
            // These fields are the immutable planning input bound to `action_plan`. The
            // recompilation after the round is represented by the next round and by the final
            // knowledge/context fields, so a replay can distinguish planned state from returned
            // evidence without attaching the wrong digest.
            knowledge: knowledge.clone(),
            context: context.clone(),
            action_plan,
            cost_units: planned_cost,
            budget_before_units: before_budget,
            budget_after_units: after_budget,
            retry_count: retry_round,
        });
        if !failed_round.is_empty() {
            stop_reason = DecisionContextCampaignStopReason::ExecutorFailed;
            break;
        }
        if !progress {
            stop_reason = DecisionContextCampaignStopReason::NoProgress;
            break;
        }
        if after_budget == 0 {
            stop_reason = DecisionContextCampaignStopReason::BudgetExhausted;
            break;
        }
    }

    let final_knowledge = compile_typed_knowledge(&request.knowledge, &records)
        .map_err(|error| DecisionContextCampaignError::Planning(error.to_string()))?;
    let final_context = compile_decision_context(&request.context, &final_knowledge)
        .map_err(|error| DecisionContextCampaignError::Planning(error.to_string()))?;
    let final_plan_request = bounded_plan_request(
        request,
        &completed,
        request.budget_units.saturating_sub(budget_spent),
    );
    let final_action_plan = plan_decision_actions(&final_plan_request, &final_context)
        .map_err(|error| DecisionContextCampaignError::Planning(error.to_string()))?;
    omissions.extend(final_context.omission_order.iter().cloned());
    omissions.extend(final_action_plan.omission_order.iter().cloned());
    negative.extend(final_knowledge.negative_evidence_order.iter().cloned());
    uncertainty.extend(final_knowledge.uncertainty_order.iter().cloned());
    uncertainty.extend(final_context.uncertainty_order.iter().cloned());
    if request.stop_on_qualified
        && final_knowledge.disposition == KnowledgeDisposition::Qualified
        && final_context.disposition == DecisionContextDisposition::Qualified
        && final_context.deferred_action_order.is_empty()
        && final_action_plan.selected_order.is_empty()
        && final_action_plan.deferred_order.is_empty()
    {
        stop_reason = DecisionContextCampaignStopReason::Qualified;
    }
    let disposition = match stop_reason {
        DecisionContextCampaignStopReason::Qualified => {
            DecisionContextCampaignDisposition::Qualified
        }
        DecisionContextCampaignStopReason::BudgetExhausted => {
            DecisionContextCampaignDisposition::BudgetBlocked
        }
        DecisionContextCampaignStopReason::ExecutorFailed => {
            DecisionContextCampaignDisposition::Failed
        }
        DecisionContextCampaignStopReason::NoActions => {
            DecisionContextCampaignDisposition::NoActions
        }
        _ if final_knowledge.disposition == KnowledgeDisposition::Unresolved
            || final_context.disposition == DecisionContextDisposition::Unresolved =>
        {
            DecisionContextCampaignDisposition::Unresolved
        }
        _ => DecisionContextCampaignDisposition::Partial,
    };
    let mut output = DecisionContextCampaign {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.knowledge.objective.clone(),
        rounds,
        records,
        completed_order: completed.into_iter().collect(),
        failed_order: failed.into_iter().collect(),
        accepted_evidence_order: accepted_evidence.into_iter().collect(),
        retry_count,
        budget_spent_units: budget_spent,
        remaining_budget_units: request.budget_units.saturating_sub(budget_spent),
        final_knowledge,
        final_context,
        final_action_plan,
        omission_order: omissions.into_iter().collect(),
        negative_evidence: negative.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        simulation_only: true,
        disposition,
        stop_reason,
        digest: ContentHash::of_bytes(b"unsealed-glioma-decision-context-campaign"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| DecisionContextCampaignError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::evidence::EvidenceSourceKind;
    use crate::glioma_engine::{GliomaModality, GliomaModelSystem};

    fn evidence(id: &str, state: EvidenceState) -> EvidenceRecord {
        EvidenceRecord {
            evidence_id: id.into(),
            source_artifact: LocalArtifactRef {
                artifact_id: format!("artifact-{id}"),
                content_hash: ContentHash::of_bytes(id.as_bytes()),
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
            state,
            relevance_milli: 900,
            quality_milli: 900,
            reproducibility_milli: 900,
            release_epoch: 1,
        }
    }

    fn request() -> DecisionContextCampaignRequest {
        DecisionContextCampaignRequest {
            knowledge: KnowledgeRequest {
                objective: "rank glioma invasion actions".into(),
                required_modalities: BTreeSet::from([GliomaModality::Genomics]),
                required_model_systems: BTreeSet::from([GliomaModelSystem::Organoid]),
                min_support_milli: 700,
                min_sources_per_claim: 1,
                max_claims: 8,
            },
            context: DecisionContextRequest {
                objective: "rank glioma invasion actions".into(),
                max_actions: 8,
                default_cost_units: 2,
            },
            action_plan: DecisionActionPlanRequest {
                objective: "rank glioma invasion actions".into(),
                completed_action_order: Vec::new(),
                selection: crate::glioma_engine::GliomaSelectionConfig {
                    budget_units: 2,
                    max_actions: 1,
                    approval_granted: true,
                    allow_instrument_execution: false,
                    allow_federation: false,
                    weights: Default::default(),
                },
            },
            records: vec![evidence("seed", EvidenceState::Supported)],
            budget_units: 2,
            max_rounds: 3,
            max_retries: 1,
            stop_on_qualified: true,
        }
    }

    #[test]
    fn campaign_dispatches_context_action_and_replays_deterministically() {
        let request = request();
        let mut first_executor = DryRunDecisionContextCampaignExecutor;
        let mut second_executor = DryRunDecisionContextCampaignExecutor;
        let first =
            execute_glioma_decision_context_campaign(&request, &mut first_executor).unwrap();
        let second =
            execute_glioma_decision_context_campaign(&request, &mut second_executor).unwrap();
        assert_eq!(first, second);
        assert_eq!(
            first.disposition,
            DecisionContextCampaignDisposition::Qualified
        );
        assert_eq!(
            first.final_action_plan.disposition,
            DecisionActionPlanDisposition::NoRunnableActions
        );
        assert!(!first.completed_order.is_empty());
        first.validate().unwrap();
    }

    #[test]
    fn negative_and_contradictory_evidence_remain_visible() {
        let mut request = request();
        request.records = vec![evidence("negative", EvidenceState::Negative)];
        request.stop_on_qualified = false;
        let mut executor = DryRunDecisionContextCampaignExecutor;
        let output = execute_glioma_decision_context_campaign(&request, &mut executor).unwrap();
        assert!(!output.negative_evidence.is_empty());
        assert_ne!(
            output.disposition,
            DecisionContextCampaignDisposition::Qualified
        );
        output.validate().unwrap();
    }

    #[test]
    fn budget_exhaustion_is_explicit() {
        let mut request = request();
        request.budget_units = 1;
        let mut executor = DryRunDecisionContextCampaignExecutor;
        let output = execute_glioma_decision_context_campaign(&request, &mut executor).unwrap();
        assert_eq!(
            output.stop_reason,
            DecisionContextCampaignStopReason::BudgetExhausted
        );
        assert_eq!(
            output.disposition,
            DecisionContextCampaignDisposition::BudgetBlocked
        );
        output.validate().unwrap();
    }

    #[test]
    fn resumed_campaign_does_not_repeat_completed_context_action() {
        let request = request();
        let mut first_executor = DryRunDecisionContextCampaignExecutor;
        let first =
            execute_glioma_decision_context_campaign(&request, &mut first_executor).unwrap();
        let mut resumed = request;
        resumed.action_plan.completed_action_order = first.completed_order.clone();
        let mut second_executor = DryRunDecisionContextCampaignExecutor;
        let second =
            execute_glioma_decision_context_campaign(&resumed, &mut second_executor).unwrap();
        assert!(second.rounds.is_empty());
        assert_eq!(second.completed_order, first.completed_order);
        assert_eq!(
            second.disposition,
            DecisionContextCampaignDisposition::Qualified
        );
        second.validate().unwrap();
    }
}
