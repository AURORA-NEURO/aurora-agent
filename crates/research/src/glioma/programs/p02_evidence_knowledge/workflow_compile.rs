//! Local workflow orchestration for federated-continual research actions.
//!
//! F12 ranks a bounded frontier; this feature turns its selected actions into a local execution
//! DAG with deterministic parallel waves, checkpoints, retries, compensations, and critical-path
//! estimates. It is intentionally planning-only: the local dispatcher owns effects, and physical
//! or approval-required actions cannot silently cross this boundary.

use super::continual_agent::{
    FederatedAgentDecision, FederatedAgentPlanItem, FederatedContinualAgentPlan,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P02-F13";
pub const OUTPUT_SCHEMA: &str = "GliomaLocalResearchWorkflow1@1";
pub const MAX_STEPS: usize = 4_096;
pub const MAX_WAVES: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalWorkflowRequest {
    pub objective: String,
    pub plan: FederatedContinualAgentPlan,
    pub max_steps: usize,
    pub max_waves: usize,
    pub retry_limit: u8,
    pub checkpoint_every: usize,
    pub allow_approval_required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalWorkflowStep {
    pub step_id: String,
    pub action_id: String,
    pub claim_key: String,
    pub dependency_order: Vec<String>,
    pub wave: usize,
    pub retry_limit: u8,
    pub checkpoint_after: bool,
    pub compensation_kind: String,
    pub expected_artifact_kind: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LocalWorkflowDisposition {
    Ready,
    ApprovalRequired,
    Partial,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalResearchWorkflow {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub source_plan_digest: ContentHash,
    pub step_order: Vec<String>,
    pub parallel_waves: Vec<Vec<String>>,
    pub checkpoint_order: Vec<String>,
    pub compensation_order: Vec<String>,
    pub steps: Vec<LocalWorkflowStep>,
    pub critical_path_steps: usize,
    pub total_cost_milli: u32,
    pub omitted_order: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: LocalWorkflowDisposition,
    pub next_step: String,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum LocalWorkflowError {
    #[error("local workflow request is invalid: {0}")]
    InvalidRequest(String),
    #[error("local workflow output is invalid: {0}")]
    InvalidOutput(String),
    #[error("local workflow digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(output: &LocalResearchWorkflow) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "source_plan_digest": output.source_plan_digest,
        "step_order": output.step_order,
        "parallel_waves": output.parallel_waves,
        "checkpoint_order": output.checkpoint_order,
        "compensation_order": output.compensation_order,
        "steps": output.steps,
        "critical_path_steps": output.critical_path_steps,
        "total_cost_milli": output.total_cost_milli,
        "omitted_order": output.omitted_order,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
        "next_step": output.next_step,
    })
}

impl LocalResearchWorkflow {
    pub fn validate(&self) -> Result<(), LocalWorkflowError> {
        let ids = self
            .steps
            .iter()
            .map(|step| step.action_id.clone())
            .collect::<Vec<_>>();
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || ids != self.step_order
            || !canonical(&self.step_order)
            || !canonical(&self.checkpoint_order)
            || !canonical(&self.compensation_order)
            || !canonical(&self.omitted_order)
            || !canonical(&self.uncertainty)
            || self.critical_path_steps > self.parallel_waves.len()
            || self.steps.iter().any(|step| {
                step.step_id.trim().is_empty()
                    || step.action_id.trim().is_empty()
                    || step.claim_key.trim().is_empty()
                    || !canonical(&step.dependency_order)
                    || step.compensation_kind.trim().is_empty()
                    || step.expected_artifact_kind.trim().is_empty()
            })
            || self.parallel_waves.iter().any(|wave| !canonical(wave))
        {
            return Err(LocalWorkflowError::InvalidOutput(
                "identity, ordering, wave, critical-path, or step fields are invalid".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| LocalWorkflowError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(LocalWorkflowError::InvalidOutput(
                "local workflow digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

fn compensation_kind(item: &FederatedAgentPlanItem) -> &'static str {
    match item.kind {
        super::continual_agent::FederatedAgentActionKind::AcquireCoverage => {
            "preserve_prior_knowledge"
        }
        super::continual_agent::FederatedAgentActionKind::ReplicateHighInfluence => {
            "retain_site_exclusion"
        }
        super::continual_agent::FederatedAgentActionKind::AdjudicateContradiction => {
            "restore_contested_state"
        }
        super::continual_agent::FederatedAgentActionKind::HarmonizeModality => {
            "restore_pre_harmonized_state"
        }
        super::continual_agent::FederatedAgentActionKind::ContinueMonitor => "close_monitor_window",
        super::continual_agent::FederatedAgentActionKind::QuarantineNegative => "retain_quarantine",
    }
}

fn artifact_kind(item: &FederatedAgentPlanItem) -> &'static str {
    match item.kind {
        super::continual_agent::FederatedAgentActionKind::AcquireCoverage => {
            "typed_evidence_artifact"
        }
        super::continual_agent::FederatedAgentActionKind::ReplicateHighInfluence => {
            "replication_artifact"
        }
        super::continual_agent::FederatedAgentActionKind::AdjudicateContradiction => {
            "adjudication_artifact"
        }
        super::continual_agent::FederatedAgentActionKind::HarmonizeModality => {
            "harmonized_knowledge_artifact"
        }
        super::continual_agent::FederatedAgentActionKind::ContinueMonitor => "monitor_snapshot",
        super::continual_agent::FederatedAgentActionKind::QuarantineNegative => {
            "negative_result_artifact"
        }
    }
}

pub fn compile_local_research_workflow(
    request: &LocalWorkflowRequest,
) -> Result<LocalResearchWorkflow, LocalWorkflowError> {
    if request.objective.trim().is_empty()
        || request.max_steps == 0
        || request.max_steps > MAX_STEPS
        || request.max_waves == 0
        || request.max_waves > MAX_WAVES
        || request.checkpoint_every == 0
    {
        return Err(LocalWorkflowError::InvalidRequest(
            "objective, step/wave capacity, and checkpoint interval are invalid".into(),
        ));
    }
    request
        .plan
        .validate()
        .map_err(|error| LocalWorkflowError::InvalidRequest(error.to_string()))?;
    if request.plan.objective != request.objective {
        return Err(LocalWorkflowError::InvalidRequest(
            "workflow objective must bind to the agent plan objective".into(),
        ));
    }
    let selected = request
        .plan
        .items
        .iter()
        .filter(|item| matches!(item.decision, FederatedAgentDecision::Selected))
        .map(|item| (item.action_id.clone(), item))
        .collect::<BTreeMap<_, _>>();
    let mut omitted_order = request.plan.omitted_order.clone();
    let mut uncertainty = request.plan.uncertainty.clone();
    if !request.allow_approval_required && !request.plan.approval_order.is_empty() {
        uncertainty.push("approval-required actions remain outside the local workflow".into());
    }
    let mut scheduled = BTreeSet::new();
    let mut steps = Vec::new();
    let mut waves = Vec::new();
    let mut pending = selected.keys().cloned().collect::<BTreeSet<_>>();
    while !pending.is_empty() {
        if waves.len() >= request.max_waves {
            omitted_order.extend(
                pending
                    .iter()
                    .map(|id| format!("{id}: wave capacity is exhausted")),
            );
            break;
        }
        let ready = pending
            .iter()
            .filter(|id| {
                selected[*id]
                    .depends_on
                    .iter()
                    .all(|dependency| scheduled.contains(dependency))
            })
            .cloned()
            .collect::<Vec<_>>();
        if ready.is_empty() {
            omitted_order.extend(
                pending
                    .iter()
                    .map(|id| format!("{id}: dependency cycle or missing selected prerequisite")),
            );
            break;
        }
        let mut wave = Vec::new();
        for action_id in ready {
            let item = selected[&action_id];
            let wave_index = waves.len();
            let checkpoint_after = (steps.len() + 1) % request.checkpoint_every == 0;
            steps.push(LocalWorkflowStep {
                step_id: format!("workflow-step:{action_id}"),
                action_id: action_id.clone(),
                claim_key: item.claim_key.clone(),
                dependency_order: item.depends_on.clone(),
                wave: wave_index,
                retry_limit: request.retry_limit,
                checkpoint_after,
                compensation_kind: compensation_kind(item).into(),
                expected_artifact_kind: artifact_kind(item).into(),
            });
            wave.push(action_id.clone());
            scheduled.insert(action_id.clone());
            pending.remove(&action_id);
        }
        wave.sort();
        waves.push(wave);
    }
    steps.sort_by(|left, right| left.action_id.cmp(&right.action_id));
    let mut checkpoint_order = steps
        .iter()
        .filter(|step| step.checkpoint_after)
        .map(|step| step.action_id.clone())
        .collect::<Vec<_>>();
    let compensation_order = steps
        .iter()
        .map(|step| step.action_id.clone())
        .collect::<Vec<_>>();
    checkpoint_order.sort();
    omitted_order.sort();
    omitted_order.dedup();
    uncertainty.sort();
    uncertainty.dedup();
    let mut total_cost_milli = 0_u32;
    for step in &steps {
        total_cost_milli = total_cost_milli.saturating_add(selected[&step.action_id].cost_milli);
    }
    let step_order = steps
        .iter()
        .map(|step| step.action_id.clone())
        .collect::<Vec<_>>();
    let approval_required =
        !request.plan.approval_order.is_empty() && !request.allow_approval_required;
    let disposition = if approval_required {
        LocalWorkflowDisposition::ApprovalRequired
    } else if steps.is_empty() || !pending.is_empty() {
        LocalWorkflowDisposition::Blocked
    } else if !omitted_order.is_empty() || !request.plan.deferred_order.is_empty() {
        LocalWorkflowDisposition::Partial
    } else {
        LocalWorkflowDisposition::Ready
    };
    let next_step = match disposition {
        LocalWorkflowDisposition::Ready => "hand parallel waves to the local action dispatcher with checkpoint and compensation hooks",
        LocalWorkflowDisposition::ApprovalRequired => "obtain approval before admitting physical or approval-required actions",
        LocalWorkflowDisposition::Partial => "resolve omitted dependencies or deferred actions before full workflow promotion",
        LocalWorkflowDisposition::Blocked => "recompile a dependency-complete selected frontier before execution",
    };
    let mut output = LocalResearchWorkflow {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        source_plan_digest: request.plan.digest.clone(),
        step_order,
        parallel_waves: waves,
        checkpoint_order,
        compensation_order,
        steps,
        critical_path_steps: scheduled
            .iter()
            .filter_map(|action_id| {
                request
                    .plan
                    .items
                    .iter()
                    .find(|item| &item.action_id == action_id)
            })
            .map(|_| 1_usize)
            .sum::<usize>()
            .min(MAX_WAVES),
        total_cost_milli,
        omitted_order,
        uncertainty,
        disposition,
        next_step: next_step.into(),
        digest: ContentHash::of_value(&serde_json::Value::Null)
            .map_err(|error| LocalWorkflowError::Digest(error.to_string()))?,
    };
    output.critical_path_steps = output
        .steps
        .iter()
        .map(|step| step.wave + 1)
        .max()
        .unwrap_or(0);
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| LocalWorkflowError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::programs::p02_evidence_knowledge::continual_agent::{
        FederatedAgentActionKind, FederatedAgentDecision,
    };

    fn item(
        action_id: &str,
        kind: FederatedAgentActionKind,
        dependencies: Vec<String>,
        decision: FederatedAgentDecision,
    ) -> FederatedAgentPlanItem {
        FederatedAgentPlanItem {
            action_id: action_id.into(),
            claim_key: "egfr-invasion".into(),
            kind,
            priority_score_milli: 900,
            cost_milli: 100,
            expected_information_milli: 800,
            decision,
            depends_on: dependencies,
            target_site_order: vec!["site-a".into()],
            rationale: "test plan".into(),
        }
    }

    fn plan(
        items: Vec<FederatedAgentPlanItem>,
        selected: Vec<String>,
        approvals: Vec<String>,
    ) -> FederatedContinualAgentPlan {
        let mut action_order = items
            .iter()
            .map(|item| item.action_id.clone())
            .collect::<Vec<_>>();
        action_order.sort();
        let mut output = FederatedContinualAgentPlan {
            feature_id: super::super::continual_agent::FEATURE_ID.into(),
            output_schema: super::super::continual_agent::OUTPUT_SCHEMA.into(),
            objective: "local workflow".into(),
            source_knowledge_digest: ContentHash::of_value(&serde_json::json!({"source": "test"}))
                .unwrap(),
            plan_order: action_order,
            selected_order: selected,
            approval_order: approvals,
            blocked_order: Vec::new(),
            deferred_order: Vec::new(),
            quarantined_order: Vec::new(),
            items,
            budget_milli: 200,
            spent_milli: 200,
            remaining_budget_milli: 0,
            omitted_order: Vec::new(),
            uncertainty: Vec::new(),
            disposition: super::super::continual_agent::FederatedAgentDisposition::Selected,
            next_step: "test".into(),
            digest: ContentHash::of_value(&serde_json::Value::Null).unwrap(),
        };
        output.digest = ContentHash::of_value(&serde_json::json!({
            "feature_id": output.feature_id,
            "output_schema": output.output_schema,
            "objective": output.objective,
            "source_knowledge_digest": output.source_knowledge_digest,
            "plan_order": output.plan_order,
            "selected_order": output.selected_order,
            "approval_order": output.approval_order,
            "blocked_order": output.blocked_order,
            "deferred_order": output.deferred_order,
            "quarantined_order": output.quarantined_order,
            "items": output.items,
            "budget_milli": output.budget_milli,
            "spent_milli": output.spent_milli,
            "remaining_budget_milli": output.remaining_budget_milli,
            "omitted_order": output.omitted_order,
            "uncertainty": output.uncertainty,
            "disposition": output.disposition,
            "next_step": output.next_step,
        }))
        .unwrap();
        output
    }

    #[test]
    fn compiles_dependency_waves_with_checkpoints_and_compensation() {
        let workflow = compile_local_research_workflow(&LocalWorkflowRequest {
            objective: "local workflow".into(),
            plan: plan(
                vec![
                    item(
                        "base",
                        FederatedAgentActionKind::AcquireCoverage,
                        Vec::new(),
                        FederatedAgentDecision::Selected,
                    ),
                    item(
                        "dependent",
                        FederatedAgentActionKind::ReplicateHighInfluence,
                        vec!["base".into()],
                        FederatedAgentDecision::Selected,
                    ),
                ],
                vec!["base".into(), "dependent".into()],
                Vec::new(),
            ),
            max_steps: 8,
            max_waves: 8,
            retry_limit: 2,
            checkpoint_every: 1,
            allow_approval_required: false,
        })
        .expect("workflow");
        assert_eq!(workflow.disposition, LocalWorkflowDisposition::Ready);
        assert_eq!(
            workflow.parallel_waves,
            vec![vec![String::from("base")], vec![String::from("dependent")]]
        );
        assert_eq!(workflow.critical_path_steps, 2);
        assert_eq!(workflow.checkpoint_order, vec!["base", "dependent"]);
        workflow.validate().expect("digest validates");
    }

    #[test]
    fn approval_required_plan_does_not_become_a_runnable_workflow() {
        let workflow = compile_local_research_workflow(&LocalWorkflowRequest {
            objective: "local workflow".into(),
            plan: plan(
                vec![item(
                    "physical",
                    FederatedAgentActionKind::AcquireCoverage,
                    Vec::new(),
                    FederatedAgentDecision::ApprovalRequired,
                )],
                Vec::new(),
                vec!["physical".into()],
            ),
            max_steps: 8,
            max_waves: 8,
            retry_limit: 1,
            checkpoint_every: 1,
            allow_approval_required: false,
        })
        .expect("workflow");
        assert_eq!(
            workflow.disposition,
            LocalWorkflowDisposition::ApprovalRequired
        );
        assert!(workflow.steps.is_empty());
    }
}
