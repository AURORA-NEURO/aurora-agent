//! Evidence-to-execution autopilot for local preclinical glioma research.
//!
//! This is the vertical glue between the P04 decision context and the P07 action executor.  It
//! compiles no new science and does not invent observations; it takes already typed evidence
//! gaps, chooses a dependency-safe batch, and runs that batch through the caller-owned executor.
//! A real institution can supply an assay gateway or analysis worker.  The MCP adapter supplies a
//! synthetic worker, so the same route is useful for workflow rehearsals without touching biology.

use super::action_execution::{
    execute_glioma_action_portfolio, ActionPortfolioExecution, ActionPortfolioExecutionDisposition,
    ActionPortfolioExecutionError, ActionPortfolioExecutionRequest, GliomaActionExecutor,
    MAX_RETRIES,
};
use crate::glioma::programs::p04_decision_context::{
    plan_decision_actions, DecisionActionPlan, DecisionActionPlanError, DecisionActionPlanRequest,
    DecisionContext,
};
use crate::glioma_engine::GliomaSelectionConfig;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P07-F21";
pub const OUTPUT_SCHEMA: &str = "GliomaResearchAutopilot1@1";

/// A bounded execution request. The context is immutable input from P04; changing policy or
/// completed ids produces a new plan digest and cannot mutate a prior run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GliomaResearchAutopilotRequest {
    pub objective: String,
    pub context: DecisionContext,
    pub completed_action_order: Vec<String>,
    pub selection: GliomaSelectionConfig,
    pub max_retries: u8,
    pub require_artifacts: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GliomaResearchAutopilotDisposition {
    Completed,
    Partial,
    NoRunnableActions,
    Failed,
}

/// One autonomous cycle, ready to be resumed by recompiling the context from returned artifacts.
/// The execution field is absent only when P04 found no runnable action; this makes a hold a
/// first-class product result rather than a fake successful run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GliomaResearchAutopilotRun {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub context_digest: ContentHash,
    pub action_plan: DecisionActionPlan,
    pub selected_order: Vec<String>,
    pub executed_order: Vec<String>,
    pub execution: Option<ActionPortfolioExecution>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: GliomaResearchAutopilotDisposition,
    pub next_step: String,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum GliomaResearchAutopilotError {
    #[error("glioma research autopilot request is invalid: {0}")]
    InvalidRequest(String),
    #[error("glioma research autopilot context is invalid: {0}")]
    InvalidContext(String),
    #[error("glioma research autopilot planning failed: {0}")]
    Planning(String),
    #[error("glioma research autopilot execution failed: {0}")]
    Execution(String),
    #[error("glioma research autopilot output is invalid: {0}")]
    InvalidOutput(String),
    #[error("glioma research autopilot digest failed: {0}")]
    Digest(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(output: &GliomaResearchAutopilotRun) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "context_digest": output.context_digest,
        "action_plan": output.action_plan,
        "selected_order": output.selected_order,
        "executed_order": output.executed_order,
        "execution": output.execution,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
        "next_step": output.next_step,
    })
}

impl GliomaResearchAutopilotRun {
    pub fn validate(&self) -> Result<(), GliomaResearchAutopilotError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.context_digest.as_str().len() != 64
            || !canonical(&self.selected_order)
            || !canonical(&self.executed_order)
            || !canonical(&self.negative_evidence)
            || !canonical(&self.uncertainty)
            || self.next_step.trim().is_empty()
            || self.action_plan.context_digest != self.context_digest
            || self.selected_order != self.action_plan.selected_order
            || self
                .executed_order
                .iter()
                .any(|id| self.selected_order.binary_search(id).is_err())
        {
            return Err(GliomaResearchAutopilotError::InvalidOutput(
                "identity, action ordering, nested plan, or next-step contract is invalid".into(),
            ));
        }
        if let Some(execution) = &self.execution {
            execution
                .validate()
                .map_err(|error| GliomaResearchAutopilotError::InvalidOutput(error.to_string()))?;
            if execution.action_order != self.selected_order
                || execution
                    .results
                    .iter()
                    .map(|result| result.action_id.clone())
                    .collect::<BTreeSet<_>>()
                    != self.executed_order.iter().cloned().collect::<BTreeSet<_>>()
            {
                return Err(GliomaResearchAutopilotError::InvalidOutput(
                    "nested execution does not reconcile with selected or executed actions".into(),
                ));
            }
        } else if !self.selected_order.is_empty() {
            return Err(GliomaResearchAutopilotError::InvalidOutput(
                "a selected action batch must have an execution result".into(),
            ));
        }
        self.action_plan
            .validate()
            .map_err(|error| GliomaResearchAutopilotError::InvalidOutput(error.to_string()))?;
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| GliomaResearchAutopilotError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(GliomaResearchAutopilotError::InvalidOutput(
                "autopilot digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

fn validate_request(
    request: &GliomaResearchAutopilotRequest,
) -> Result<(), GliomaResearchAutopilotError> {
    if request.objective.trim().is_empty()
        || !canonical(&request.completed_action_order)
        || request
            .completed_action_order
            .iter()
            .any(|id| id.trim().is_empty())
        || request.max_retries > MAX_RETRIES
    {
        return Err(GliomaResearchAutopilotError::InvalidRequest(
            "objective, canonical completed actions, and bounded retries are required".into(),
        ));
    }
    Ok(())
}

fn merge_sorted(left: &[String], right: &[String]) -> Vec<String> {
    left.iter()
        .chain(right.iter())
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

/// Compile, select, and execute one autonomous local research cycle.
pub fn execute_glioma_research_autopilot<E: GliomaActionExecutor>(
    request: &GliomaResearchAutopilotRequest,
    executor: &mut E,
) -> Result<GliomaResearchAutopilotRun, GliomaResearchAutopilotError> {
    validate_request(request)?;
    request
        .context
        .validate()
        .map_err(|error| GliomaResearchAutopilotError::InvalidContext(error.to_string()))?;
    if request.objective.trim() != request.context.objective.trim() {
        return Err(GliomaResearchAutopilotError::InvalidRequest(
            "autopilot objective must match the compiled decision context".into(),
        ));
    }
    let action_plan = plan_decision_actions(
        &DecisionActionPlanRequest {
            objective: request.objective.clone(),
            completed_action_order: request.completed_action_order.clone(),
            selection: request.selection.clone(),
        },
        &request.context,
    )
    .map_err(|error: DecisionActionPlanError| {
        GliomaResearchAutopilotError::Planning(error.to_string())
    })?;
    let (
        execution,
        executed_order,
        disposition,
        next_step,
        execution_negative,
        execution_uncertainty,
    ) = if action_plan.selected_order.is_empty() {
        (
            None,
            Vec::new(),
            GliomaResearchAutopilotDisposition::NoRunnableActions,
            action_plan.next_step.clone(),
            Vec::new(),
            Vec::new(),
        )
    } else {
        let candidates = request
            .context
            .actions
            .iter()
            .map(|action| action.candidate.clone())
            .collect::<Vec<_>>();
        let completed = request
            .completed_action_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let execution = execute_glioma_action_portfolio(
            &ActionPortfolioExecutionRequest {
                candidates,
                completed_actions: completed,
                selection: request.selection.clone(),
                max_retries: request.max_retries,
                require_artifacts: request.require_artifacts,
            },
            executor,
        )
        .map_err(|error: ActionPortfolioExecutionError| {
            GliomaResearchAutopilotError::Execution(error.to_string())
        })?;
        if execution.selection.selected_order != action_plan.selected_order {
            return Err(GliomaResearchAutopilotError::Execution(
                "action selector changed between planning and execution".into(),
            ));
        }
        let disposition = match execution.disposition {
                ActionPortfolioExecutionDisposition::Completed => {
                    if action_plan.disposition == crate::glioma::programs::p04_decision_context::DecisionActionPlanDisposition::Qualified {
                        GliomaResearchAutopilotDisposition::Completed
                    } else {
                        GliomaResearchAutopilotDisposition::Partial
                    }
                }
                ActionPortfolioExecutionDisposition::Partial => {
                    GliomaResearchAutopilotDisposition::Partial
                }
                ActionPortfolioExecutionDisposition::Failed
                | ActionPortfolioExecutionDisposition::Blocked => {
                    GliomaResearchAutopilotDisposition::Failed
                }
            };
        let next_step = match disposition {
            GliomaResearchAutopilotDisposition::Completed
            | GliomaResearchAutopilotDisposition::Partial => {
                "recompile the decision context from returned local artifacts before the next cycle"
            }
            GliomaResearchAutopilotDisposition::Failed => {
                "hold and inspect executor failures or partial artifacts before resuming"
            }
            GliomaResearchAutopilotDisposition::NoRunnableActions => {
                "hold until a runnable action is available"
            }
        };
        (
            Some(execution.clone()),
            execution
                .results
                .iter()
                .map(|result| result.action_id.clone())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect(),
            disposition,
            next_step.to_string(),
            execution.negative_evidence.clone(),
            execution.uncertainty.clone(),
        )
    };
    let mut output = GliomaResearchAutopilotRun {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        context_digest: request.context.digest.clone(),
        selected_order: action_plan.selected_order.clone(),
        action_plan,
        executed_order,
        execution,
        negative_evidence: merge_sorted(
            &request.context.negative_evidence_order,
            &execution_negative,
        ),
        uncertainty: merge_sorted(&request.context.uncertainty_order, &execution_uncertainty),
        disposition,
        next_step,
        digest: ContentHash::of_bytes(b"unsealed-glioma-research-autopilot"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| GliomaResearchAutopilotError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::programs::p04_decision_context::{
        DecisionAction, DecisionActionKind, DecisionContextDisposition,
    };
    use crate::glioma_engine::{
        GliomaActionCandidate, GliomaModality, GliomaModelSystem, GliomaStageKind,
    };
    use bioprism_foundation::{AutonomyTier, Effect};

    fn context() -> DecisionContext {
        let candidate = GliomaActionCandidate {
            action_id: "decision:invasion".into(),
            stage_kind: GliomaStageKind::MechanismExploration,
            modality: GliomaModality::Computational,
            model_system: GliomaModelSystem::InSilico,
            depends_on: Vec::new(),
            cost_units: 2,
            information_gain_milli: 900,
            frontier_novelty_milli: 700,
            workflow_leverage_milli: 900,
            cross_stage_unlock_milli: 900,
            reproducibility_safety_milli: 900,
            federation_value_milli: 500,
            feasibility_milli: 900,
            autonomy_tier: AutonomyTier::A1,
            effects: BTreeSet::from([
                Effect::ReadLocalData,
                Effect::ExecuteLocalComputation,
                Effect::WriteLocalArtifact,
            ]),
        };
        let action = DecisionAction {
            action_id: candidate.action_id.clone(),
            claim_id: "claim:invasion".into(),
            kind: DecisionActionKind::ValidateMechanism,
            rationale: "validate the supported claim".into(),
            target_modality: GliomaModality::Computational,
            target_model_system: GliomaModelSystem::InSilico,
            priority_milli: 900,
            candidate,
        };
        let mut context = DecisionContext {
            feature_id: "GAF-GLIOMA-P04-F01".into(),
            output_schema: "GliomaDecisionContext1@1".into(),
            objective: "prioritize invasion assays".into(),
            claim_order: vec!["claim:invasion".into()],
            actions: vec![action],
            action_order: vec!["decision:invasion".into()],
            deferred_action_order: Vec::new(),
            omission_order: Vec::new(),
            negative_evidence_order: Vec::new(),
            uncertainty_order: Vec::new(),
            disposition: DecisionContextDisposition::Qualified,
            digest: ContentHash::of_bytes(b"unsealed"),
        };
        context.digest = ContentHash::of_value(&serde_json::json!({
            "feature_id": context.feature_id,
            "output_schema": context.output_schema,
            "objective": context.objective,
            "claim_order": context.claim_order,
            "actions": context.actions,
            "action_order": context.action_order,
            "deferred_action_order": context.deferred_action_order,
            "omission_order": context.omission_order,
            "negative_evidence_order": context.negative_evidence_order,
            "uncertainty_order": context.uncertainty_order,
            "disposition": context.disposition,
        }))
        .unwrap();
        context.validate().unwrap();
        context
    }

    fn request() -> GliomaResearchAutopilotRequest {
        GliomaResearchAutopilotRequest {
            objective: "prioritize invasion assays".into(),
            context: context(),
            completed_action_order: Vec::new(),
            selection: GliomaSelectionConfig {
                budget_units: 4,
                max_actions: 1,
                approval_granted: false,
                allow_instrument_execution: false,
                allow_federation: false,
                weights: Default::default(),
            },
            max_retries: 1,
            require_artifacts: true,
        }
    }

    #[test]
    fn autopilot_executes_selected_context_action() {
        let mut executor = super::super::action_execution::DryRunGliomaActionExecutor;
        let output = execute_glioma_research_autopilot(&request(), &mut executor).unwrap();
        assert_eq!(output.selected_order, vec!["decision:invasion"]);
        assert_eq!(output.executed_order, vec!["decision:invasion"]);
        assert_eq!(
            output.disposition,
            GliomaResearchAutopilotDisposition::Completed
        );
        assert!(output.execution.is_some());
        output.validate().unwrap();
    }

    #[test]
    fn completed_action_is_held_without_execution() {
        let mut request = request();
        request.completed_action_order = vec!["decision:invasion".into()];
        let mut executor = super::super::action_execution::DryRunGliomaActionExecutor;
        let output = execute_glioma_research_autopilot(&request, &mut executor).unwrap();
        assert!(output.execution.is_none());
        assert_eq!(
            output.disposition,
            GliomaResearchAutopilotDisposition::NoRunnableActions
        );
    }

    #[test]
    fn tampered_context_can_never_enter_action_execution() {
        let mut request = request();
        request.context.uncertainty_order = vec!["human-data".into()];
        let mut executor = super::super::action_execution::DryRunGliomaActionExecutor;
        // The context digest no longer matches after mutation, so the run is rejected before the
        // executor seam is called.
        assert!(execute_glioma_research_autopilot(&request, &mut executor).is_err());
    }
}
