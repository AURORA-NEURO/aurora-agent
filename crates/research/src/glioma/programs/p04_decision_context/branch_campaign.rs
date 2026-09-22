//! Bounded execution and failover for scenario-aware glioma decision branches.
//!
//! `branch_planner` produces alternatives; this module makes those alternatives executable.  It
//! runs one dependency-closed branch at a time through an institution-owned executor, scores the
//! returned local evidence against the branch forecast, and moves to the next Pareto alternative
//! when the observed result drifts or an action fails.  Negative and contradictory outcomes are
//! retained as scientific results, never converted into a silent success or erased retry.

use super::branch_planner::{DecisionBranchPlan, DecisionBranchPortfolio};
use super::campaign::{DecisionContextCampaignExecutionFailure, DecisionContextCampaignExecutor};
use super::context_compiler::{DecisionAction, DecisionContext};
use crate::glioma::evidence::{EvidenceRecord, EvidenceState};
use crate::glioma::programs::p02_evidence_knowledge::TypedKnowledge;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P04-F27";
pub const OUTPUT_SCHEMA: &str = "GliomaDecisionBranchCampaign1@1";
pub const MAX_BRANCHES: u16 = 32;
pub const MAX_RETRIES: u8 = 8;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionBranchCampaignRequest {
    pub objective: String,
    pub knowledge: TypedKnowledge,
    pub context: DecisionContext,
    pub branch_plan: DecisionBranchPlan,
    pub completed_action_order: Vec<String>,
    pub budget_units: u64,
    pub max_branches: u16,
    pub max_retries: u8,
    pub stop_on_completed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BranchExecutionDisposition {
    Completed,
    Drifted,
    Failed,
    BudgetBlocked,
    Held,
    Abstained,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionBranchExecution {
    pub branch_id: String,
    pub planned_order: Vec<String>,
    pub completed_order: Vec<String>,
    pub failed_order: Vec<String>,
    pub accepted_evidence_order: Vec<String>,
    pub planned_robustness_milli: i64,
    pub expected_value_milli: i64,
    pub worst_case_value_milli: i64,
    pub observed_value_milli: i64,
    pub drift_milli: i64,
    pub cost_units: u64,
    pub retry_count: u32,
    pub disposition: BranchExecutionDisposition,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecisionBranchCampaignStopReason {
    Completed,
    AllBranchesAttempted,
    BudgetExhausted,
    NoAdmissibleBranch,
    ExecutorFailed,
    InvalidEvidence,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecisionBranchCampaignDisposition {
    Completed,
    Partial,
    Failed,
    BudgetBlocked,
    Held,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionBranchCampaign {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub branch_order: Vec<String>,
    pub attempted_order: Vec<String>,
    pub completed_branch_id: Option<String>,
    pub next_branch_id: Option<String>,
    pub executions: Vec<DecisionBranchExecution>,
    pub records: Vec<EvidenceRecord>,
    pub completed_action_order: Vec<String>,
    pub failed_action_order: Vec<String>,
    pub accepted_evidence_order: Vec<String>,
    pub fallback_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub retry_count: u32,
    pub budget_spent_units: u64,
    pub remaining_budget_units: u64,
    pub simulation_only: bool,
    pub disposition: DecisionBranchCampaignDisposition,
    pub stop_reason: DecisionBranchCampaignStopReason,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum DecisionBranchCampaignError {
    #[error("decision-branch campaign request is invalid: {0}")]
    InvalidRequest(String),
    #[error("decision-branch campaign planning is invalid: {0}")]
    InvalidPlan(String),
    #[error("decision-branch campaign execution failed: {0}")]
    Execution(String),
    #[error("decision-branch campaign evidence is invalid: {0}")]
    InvalidEvidence(String),
    #[error("decision-branch campaign output is invalid: {0}")]
    InvalidOutput(String),
    #[error("decision-branch campaign digest failed: {0}")]
    Digest(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(output: &DecisionBranchCampaign) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "branch_order": output.branch_order,
        "attempted_order": output.attempted_order,
        "completed_branch_id": output.completed_branch_id,
        "next_branch_id": output.next_branch_id,
        "executions": output.executions,
        "records": output.records,
        "completed_action_order": output.completed_action_order,
        "failed_action_order": output.failed_action_order,
        "accepted_evidence_order": output.accepted_evidence_order,
        "fallback_order": output.fallback_order,
        "omission_order": output.omission_order,
        "negative_evidence_order": output.negative_evidence_order,
        "uncertainty_order": output.uncertainty_order,
        "retry_count": output.retry_count,
        "budget_spent_units": output.budget_spent_units,
        "remaining_budget_units": output.remaining_budget_units,
        "simulation_only": output.simulation_only,
        "disposition": output.disposition,
        "stop_reason": output.stop_reason,
    })
}

impl DecisionBranchCampaign {
    pub fn validate(&self) -> Result<(), DecisionBranchCampaignError> {
        let execution_ids = self
            .executions
            .iter()
            .map(|execution| execution.branch_id.clone())
            .collect::<Vec<_>>();
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || !canonical(&self.branch_order)
            || !canonical(&self.attempted_order)
            || !canonical(&self.completed_action_order)
            || !canonical(&self.failed_action_order)
            || !canonical(&self.accepted_evidence_order)
            || !canonical(&self.fallback_order)
            || !canonical(&self.omission_order)
            || !canonical(&self.negative_evidence_order)
            || !canonical(&self.uncertainty_order)
            || !canonical(&execution_ids)
            || self.executions.iter().any(|execution| {
                execution.branch_id.trim().is_empty()
                    || (execution.planned_order.is_empty()
                        && execution.disposition != BranchExecutionDisposition::Abstained)
                    || !canonical(&execution.planned_order)
                    || !canonical(&execution.completed_order)
                    || !canonical(&execution.failed_order)
                    || !canonical(&execution.accepted_evidence_order)
                    || (execution.cost_units == 0
                        && execution.disposition != BranchExecutionDisposition::Abstained
                        && execution.completed_order != execution.planned_order)
            })
            || self
                .executions
                .iter()
                .any(|execution| execution_ids.binary_search(&execution.branch_id).is_err())
        {
            return Err(DecisionBranchCampaignError::InvalidOutput(
                "identity, canonical partitions, branch ordering, or execution invariants are invalid".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| DecisionBranchCampaignError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(DecisionBranchCampaignError::InvalidOutput(
                "branch campaign digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

fn validate_request(
    request: &DecisionBranchCampaignRequest,
) -> Result<(), DecisionBranchCampaignError> {
    if request.objective.trim().is_empty()
        || request.budget_units == 0
        || request.max_branches == 0
        || request.max_branches > MAX_BRANCHES
        || request.max_retries > MAX_RETRIES
        || !canonical(&request.completed_action_order)
        || request
            .completed_action_order
            .iter()
            .any(|action| action.trim().is_empty())
    {
        return Err(DecisionBranchCampaignError::InvalidRequest(
            "objective, positive budget, bounded branch/retry limits, and canonical completed actions are required".into(),
        ));
    }
    request
        .knowledge
        .validate()
        .map_err(|error| DecisionBranchCampaignError::InvalidRequest(error.to_string()))?;
    request
        .context
        .validate()
        .map_err(|error| DecisionBranchCampaignError::InvalidRequest(error.to_string()))?;
    request
        .branch_plan
        .validate()
        .map_err(|error| DecisionBranchCampaignError::InvalidPlan(error.to_string()))?;
    if request.objective.trim() != request.knowledge.objective.trim()
        || request.objective.trim() != request.context.objective.trim()
        || request.objective.trim() != request.branch_plan.objective.trim()
        || request.branch_plan.context_digest != request.context.digest
    {
        return Err(DecisionBranchCampaignError::InvalidRequest(
            "objective and branch context digest must match knowledge, context, and plan".into(),
        ));
    }
    Ok(())
}

fn state_score(state: EvidenceState) -> i64 {
    match state {
        EvidenceState::Supported => 1_000,
        EvidenceState::Negative => 100,
        EvidenceState::Contradicted => -1_000,
        EvidenceState::Unknown => -400,
        EvidenceState::Stale => -500,
        EvidenceState::Unmeasured => -300,
    }
}

fn observed_value(records: &[EvidenceRecord]) -> i64 {
    let mut weighted = 0_i128;
    let mut weight = 0_i128;
    for record in records {
        let quality = i128::from(record.quality_milli)
            + i128::from(record.relevance_milli)
            + i128::from(record.reproducibility_milli);
        let quality = quality.max(1);
        weighted += i128::from(state_score(record.state)) * quality;
        weight += quality;
    }
    if weight == 0 {
        0
    } else {
        // Branch forecasts include a roughly 1,000-unit action-value baseline.  Add the same
        // baseline to the evidence state score so supported, negative, contradictory, and
        // unresolved observations are comparable to forecast values without pretending that a
        // result is more precise than its state.
        1_000 + (weighted / weight) as i64
    }
}

fn validate_returned_records(
    records: &[EvidenceRecord],
    action: &DecisionAction,
) -> Result<(), DecisionBranchCampaignError> {
    if records.is_empty() {
        return Err(DecisionBranchCampaignError::InvalidEvidence(format!(
            "action {} returned no evidence records",
            action.action_id
        )));
    }
    let mut ids = BTreeSet::new();
    for record in records {
        if record.evidence_id.trim().is_empty()
            || !ids.insert(record.evidence_id.clone())
            || record.claim.trim().is_empty()
            || record.scope.trim().is_empty()
            || record.source_artifact.contains_human_data
            || record.source_artifact.contains_direct_identifiers
            || !record.source_artifact.local_only
            || record.relevance_milli > 1_000
            || record.quality_milli > 1_000
            || record.reproducibility_milli > 1_000
        {
            return Err(DecisionBranchCampaignError::InvalidEvidence(format!(
                "action {} returned non-local, protected, duplicate, empty, or out-of-range evidence",
                action.action_id
            )));
        }
    }
    Ok(())
}

fn branch_order(plan: &DecisionBranchPlan) -> Vec<String> {
    let mut order = Vec::new();
    if let Some(selected) = &plan.selected_branch_id {
        order.push(selected.clone());
    }
    order.extend(plan.frontier_order.iter().cloned());
    order.extend(plan.branch_order.iter().cloned());
    order.sort_by(|left, right| {
        let left_rank = order_rank(left, plan);
        let right_rank = order_rank(right, plan);
        left_rank.cmp(&right_rank).then_with(|| left.cmp(right))
    });
    order.dedup();
    order
}

fn order_rank(branch_id: &str, plan: &DecisionBranchPlan) -> (u8, usize) {
    if plan.selected_branch_id.as_deref() == Some(branch_id) {
        return (0, 0);
    }
    if let Some(index) = plan.frontier_order.iter().position(|id| id == branch_id) {
        return (1, index);
    }
    (
        2,
        plan.branch_order
            .iter()
            .position(|id| id == branch_id)
            .unwrap_or(usize::MAX),
    )
}

fn portfolio<'a>(
    plan: &'a DecisionBranchPlan,
    branch_id: &str,
) -> Option<&'a DecisionBranchPortfolio> {
    plan.portfolios
        .iter()
        .find(|item| item.branch_id == branch_id)
}

/// Execute the best robust branch and fail over to the next frontier branch when observed local
/// evidence is materially worse than the forecast.  The executor remains institution-owned;
/// this controller performs no network, instrument, or protected-data effect itself.
pub fn execute_glioma_decision_branch_campaign<E: DecisionContextCampaignExecutor>(
    request: &DecisionBranchCampaignRequest,
    executor: &mut E,
) -> Result<DecisionBranchCampaign, DecisionBranchCampaignError> {
    validate_request(request)?;
    let context_actions = request
        .context
        .actions
        .iter()
        .map(|action| (action.action_id.clone(), action))
        .collect::<BTreeMap<_, _>>();
    let completed_initial = request
        .completed_action_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    if completed_initial
        .iter()
        .any(|action| !context_actions.contains_key(action))
    {
        return Err(DecisionBranchCampaignError::InvalidRequest(
            "completed action list contains an action outside the decision context".into(),
        ));
    }
    let order = branch_order(&request.branch_plan);
    let mut remaining = request.budget_units;
    let mut records_by_id = BTreeMap::<String, EvidenceRecord>::new();
    let mut completed_actions = completed_initial;
    let mut failed_actions = BTreeSet::new();
    let mut accepted_evidence = BTreeSet::new();
    let mut attempts = BTreeSet::new();
    let mut executions = Vec::new();
    let mut fallback = Vec::new();
    let mut omissions = request
        .branch_plan
        .omitted_action_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut negative = request
        .branch_plan
        .negative_evidence_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut uncertainty = request
        .branch_plan
        .uncertainty_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut retry_count = 0_u32;
    let mut budget_spent = 0_u64;
    let mut completed_branch_id = None;
    let mut stop_reason = DecisionBranchCampaignStopReason::AllBranchesAttempted;

    for (branch_index, branch_id) in order
        .iter()
        .take(usize::from(request.max_branches))
        .enumerate()
    {
        let portfolio = portfolio(&request.branch_plan, branch_id).ok_or_else(|| {
            DecisionBranchCampaignError::InvalidPlan(format!(
                "branch {branch_id} is absent from plan"
            ))
        })?;
        attempts.insert(branch_id.clone());
        if portfolio.selected_order.is_empty() {
            omissions.insert(format!("{branch_id}:empty-branch"));
            executions.push(DecisionBranchExecution {
                branch_id: branch_id.clone(),
                planned_order: Vec::new(),
                completed_order: Vec::new(),
                failed_order: Vec::new(),
                accepted_evidence_order: Vec::new(),
                planned_robustness_milli: portfolio.robustness_milli,
                expected_value_milli: portfolio.expected_value_milli,
                worst_case_value_milli: portfolio.worst_case_value_milli,
                observed_value_milli: 0,
                drift_milli: 0,
                cost_units: 0,
                retry_count: 0,
                disposition: BranchExecutionDisposition::Abstained,
            });
            continue;
        }
        let mut branch_completed = Vec::new();
        let mut branch_failed = Vec::new();
        let mut branch_evidence = Vec::new();
        let mut branch_records = Vec::new();
        let mut branch_cost = 0_u64;
        let mut branch_retries = 0_u32;
        let mut disposition = BranchExecutionDisposition::Completed;
        for action_id in &portfolio.selected_order {
            let action = context_actions.get(action_id).ok_or_else(|| {
                DecisionBranchCampaignError::InvalidPlan(format!(
                    "branch {branch_id} references action {action_id} outside context"
                ))
            })?;
            if completed_actions.contains(action_id) {
                branch_completed.push(action_id.clone());
                continue;
            }
            let cost = u64::from(action.candidate.cost_units);
            if cost == 0 || cost > remaining {
                disposition = BranchExecutionDisposition::BudgetBlocked;
                omissions.insert(format!("{branch_id}:{action_id}:budget"));
                break;
            }
            branch_cost = branch_cost.saturating_add(cost);
            budget_spent = budget_spent.saturating_add(cost);
            remaining = remaining.saturating_sub(cost);
            let mut returned = None;
            for attempt in 1..=request.max_retries.saturating_add(1) {
                match executor.execute_action(action, &request.context, &request.knowledge, attempt)
                {
                    Ok(evidence) => {
                        validate_returned_records(&evidence, action)?;
                        returned = Some(evidence);
                        break;
                    }
                    Err(DecisionContextCampaignExecutionFailure { reason, retryable }) => {
                        if reason.trim().is_empty() {
                            return Err(DecisionBranchCampaignError::Execution(
                                "executor returned an empty failure reason".into(),
                            ));
                        }
                        if retryable && attempt <= request.max_retries {
                            retry_count = retry_count.saturating_add(1);
                            branch_retries = branch_retries.saturating_add(1);
                            continue;
                        }
                        branch_failed.push(action_id.clone());
                        failed_actions.insert(action_id.clone());
                        disposition = BranchExecutionDisposition::Failed;
                        omissions.insert(format!("{branch_id}:{action_id}:executor-failed"));
                        break;
                    }
                }
            }
            if let Some(evidence) = returned {
                for record in evidence {
                    accepted_evidence.insert(record.evidence_id.clone());
                    branch_evidence.push(record.evidence_id.clone());
                    if matches!(
                        record.state,
                        EvidenceState::Negative | EvidenceState::Contradicted
                    ) {
                        negative.insert(record.evidence_id.clone());
                    }
                    if matches!(
                        record.state,
                        EvidenceState::Unknown | EvidenceState::Stale | EvidenceState::Unmeasured
                    ) {
                        uncertainty.insert(record.evidence_id.clone());
                    }
                    records_by_id.insert(record.evidence_id.clone(), record.clone());
                    branch_records.push(record);
                }
                branch_completed.push(action_id.clone());
                completed_actions.insert(action_id.clone());
            } else {
                break;
            }
            if remaining == 0 && branch_completed.len() < portfolio.selected_order.len() {
                disposition = BranchExecutionDisposition::BudgetBlocked;
                break;
            }
        }
        branch_completed.sort();
        branch_failed.sort();
        branch_evidence.sort();
        let observed = observed_value(&branch_records);
        let drift = observed.saturating_sub(portfolio.expected_value_milli);
        if disposition == BranchExecutionDisposition::Completed
            && branch_completed.len() == portfolio.selected_order.len()
            && observed < portfolio.worst_case_value_milli
        {
            disposition = BranchExecutionDisposition::Drifted;
            fallback.extend(order.iter().skip(branch_index + 1).cloned());
        }
        if disposition == BranchExecutionDisposition::Failed {
            fallback.extend(order.iter().skip(branch_index + 1).cloned());
        }
        executions.push(DecisionBranchExecution {
            branch_id: branch_id.clone(),
            planned_order: portfolio.selected_order.clone(),
            completed_order: branch_completed,
            failed_order: branch_failed,
            accepted_evidence_order: branch_evidence,
            planned_robustness_milli: portfolio.robustness_milli,
            expected_value_milli: portfolio.expected_value_milli,
            worst_case_value_milli: portfolio.worst_case_value_milli,
            observed_value_milli: observed,
            drift_milli: drift,
            cost_units: branch_cost,
            retry_count: branch_retries,
            disposition,
        });
        if disposition == BranchExecutionDisposition::Completed {
            completed_branch_id = Some(branch_id.clone());
            stop_reason = DecisionBranchCampaignStopReason::Completed;
            if request.stop_on_completed {
                break;
            }
        }
        if disposition == BranchExecutionDisposition::Failed {
            stop_reason = DecisionBranchCampaignStopReason::ExecutorFailed;
        }
        if remaining == 0 {
            stop_reason = DecisionBranchCampaignStopReason::BudgetExhausted;
            break;
        }
        if branch_index + 1 == usize::from(request.max_branches) {
            stop_reason = DecisionBranchCampaignStopReason::AllBranchesAttempted;
        }
    }
    if executions.is_empty() {
        stop_reason = DecisionBranchCampaignStopReason::NoAdmissibleBranch;
    }
    let next_branch_id = order
        .iter()
        .find(|branch| !attempts.contains(*branch))
        .cloned();
    let disposition = if completed_branch_id.is_some() {
        DecisionBranchCampaignDisposition::Completed
    } else if stop_reason == DecisionBranchCampaignStopReason::BudgetExhausted {
        DecisionBranchCampaignDisposition::BudgetBlocked
    } else if stop_reason == DecisionBranchCampaignStopReason::ExecutorFailed {
        DecisionBranchCampaignDisposition::Failed
    } else if executions.iter().any(|execution| {
        matches!(
            execution.disposition,
            BranchExecutionDisposition::Drifted | BranchExecutionDisposition::Held
        )
    }) {
        DecisionBranchCampaignDisposition::Partial
    } else {
        DecisionBranchCampaignDisposition::Unresolved
    };
    executions.sort_by(|left, right| left.branch_id.cmp(&right.branch_id));
    fallback.sort();
    fallback.dedup();
    let mut output = DecisionBranchCampaign {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        branch_order: request.branch_plan.branch_order.clone(),
        attempted_order: attempts.into_iter().collect(),
        completed_branch_id,
        next_branch_id,
        executions,
        records: records_by_id.into_values().collect(),
        completed_action_order: completed_actions.into_iter().collect(),
        failed_action_order: failed_actions.into_iter().collect(),
        accepted_evidence_order: accepted_evidence.into_iter().collect(),
        fallback_order: fallback,
        omission_order: omissions.into_iter().collect(),
        negative_evidence_order: negative.into_iter().collect(),
        uncertainty_order: uncertainty.into_iter().collect(),
        retry_count,
        budget_spent_units: budget_spent,
        remaining_budget_units: remaining,
        simulation_only: true,
        disposition,
        stop_reason,
        digest: ContentHash::of_bytes(b"unsealed-glioma-decision-branch-campaign"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| DecisionBranchCampaignError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::evidence::{EvidenceSourceKind, EvidenceState};
    use crate::glioma::programs::p02_evidence_knowledge::{
        compile_typed_knowledge, KnowledgeRequest,
    };
    use crate::glioma::programs::p04_decision_context::{
        compile_decision_context, plan_glioma_decision_branches, DecisionBranchPlannerRequest,
        DecisionContextRequest, DecisionScenario, DecisionScenarioOutcome,
        DryRunDecisionContextCampaignExecutor,
    };
    use crate::glioma_engine::{
        GliomaModality, GliomaModelSystem, GliomaSelectionWeights, LocalArtifactRef,
    };
    use bioprism_ids::ContentHash;
    use std::collections::BTreeSet;

    fn hash(label: &str) -> ContentHash {
        ContentHash::of_value(&serde_json::json!({"label": label})).unwrap()
    }

    fn inputs() -> (TypedKnowledge, DecisionContext, DecisionBranchPlan) {
        let record = EvidenceRecord {
            evidence_id: "branch-campaign-e1".into(),
            source_artifact: LocalArtifactRef {
                artifact_id: "branch-campaign-a1".into(),
                content_hash: hash("branch-campaign-a1"),
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
        let knowledge = compile_typed_knowledge(
            &KnowledgeRequest {
                objective: "rank invasion mechanisms".into(),
                required_modalities: BTreeSet::from([GliomaModality::Genomics]),
                required_model_systems: BTreeSet::from([GliomaModelSystem::Organoid]),
                min_support_milli: 700,
                min_sources_per_claim: 1,
                max_claims: 8,
            },
            &[record],
        )
        .unwrap();
        let context = compile_decision_context(
            &DecisionContextRequest {
                objective: "rank invasion mechanisms".into(),
                max_actions: 8,
                default_cost_units: 1,
            },
            &knowledge,
        )
        .unwrap();
        let candidate = context.actions[0].candidate.clone();
        let plan = plan_glioma_decision_branches(
            &DecisionBranchPlannerRequest {
                objective: "rank invasion mechanisms".into(),
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
            },
            &context,
        )
        .unwrap();
        (knowledge, context, plan)
    }

    #[test]
    fn campaign_executes_selected_branch_and_replays_stably() {
        let (knowledge, context, branch_plan) = inputs();
        let request = DecisionBranchCampaignRequest {
            objective: "rank invasion mechanisms".into(),
            knowledge,
            context,
            branch_plan,
            completed_action_order: Vec::new(),
            budget_units: 2,
            max_branches: 2,
            max_retries: 1,
            stop_on_completed: true,
        };
        let mut first_executor = DryRunDecisionContextCampaignExecutor;
        let first = execute_glioma_decision_branch_campaign(&request, &mut first_executor).unwrap();
        let mut second_executor = DryRunDecisionContextCampaignExecutor;
        let second =
            execute_glioma_decision_branch_campaign(&request, &mut second_executor).unwrap();
        assert_eq!(first, second);
        assert_eq!(
            first.disposition,
            DecisionBranchCampaignDisposition::Completed
        );
        assert!(first.completed_branch_id.is_some());
        assert_eq!(first.records.len(), 1);
        first.validate().unwrap();
    }
}
