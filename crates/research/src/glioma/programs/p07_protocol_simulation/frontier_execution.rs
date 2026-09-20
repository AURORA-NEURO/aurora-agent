//! Execute an admitted scientific-frontier batch for preclinical glioma research.
//!
//! `scientific_frontier` is the cross-program admission layer: typed knowledge and multimodal
//! readiness decide which candidates are scientifically runnable. This module makes that plan
//! executable while preserving the admission boundary. Held and blocked candidates are removed
//! before the existing action executor is invoked, and the returned execution selection must
//! reconcile with the immutable frontier plan. No instrument, federation, raw-data, or clinical
//! effect is added by this bridge.

use super::action_execution::{
    execute_glioma_action_portfolio, ActionPortfolioExecution, ActionPortfolioExecutionError,
    ActionPortfolioExecutionRequest, GliomaActionExecutor, MAX_RETRIES,
};
use super::scientific_frontier::ScientificFrontierPlan;
use crate::glioma_engine::{GliomaActionCandidate, GliomaSelectionConfig};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P07-F08";
pub const OUTPUT_SCHEMA: &str = "GliomaScientificFrontierExecution1@1";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScientificFrontierExecutionRequest {
    pub objective: String,
    pub plan_digest: ContentHash,
    pub candidates: Vec<GliomaActionCandidate>,
    pub completed_action_order: Vec<String>,
    pub selection: GliomaSelectionConfig,
    pub max_retries: u8,
    pub require_artifacts: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScientificFrontierExecutionDisposition {
    Completed,
    Partial,
    Blocked,
    NoRunnableActions,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScientificFrontierExecution {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub plan_digest: ContentHash,
    pub admitted_order: Vec<String>,
    pub held_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub selected_order: Vec<String>,
    pub executed_order: Vec<String>,
    pub execution: Option<ActionPortfolioExecution>,
    pub simulation_only: bool,
    pub disposition: ScientificFrontierExecutionDisposition,
    pub next_step: String,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ScientificFrontierExecutionError {
    #[error("scientific frontier execution request is invalid: {0}")]
    InvalidRequest(String),
    #[error("scientific frontier plan is invalid: {0}")]
    InvalidPlan(String),
    #[error("scientific frontier execution failed: {0}")]
    Execution(String),
    #[error("scientific frontier execution output is invalid: {0}")]
    InvalidOutput(String),
    #[error("scientific frontier execution digest failed: {0}")]
    Digest(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn unique(values: &[String]) -> bool {
    values.iter().collect::<BTreeSet<_>>().len() == values.len()
}

fn digest_input(output: &ScientificFrontierExecution) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "plan_digest": output.plan_digest,
        "admitted_order": output.admitted_order,
        "held_order": output.held_order,
        "blocked_order": output.blocked_order,
        "selected_order": output.selected_order,
        "executed_order": output.executed_order,
        "execution": output.execution,
        "simulation_only": output.simulation_only,
        "disposition": output.disposition,
        "next_step": output.next_step,
    })
}

impl ScientificFrontierExecution {
    pub fn validate(&self) -> Result<(), ScientificFrontierExecutionError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || !canonical(&self.admitted_order)
            || !canonical(&self.held_order)
            || !canonical(&self.blocked_order)
            || !unique(&self.selected_order)
            || !unique(&self.executed_order)
            || self
                .selected_order
                .iter()
                .any(|id| !self.admitted_order.iter().any(|admitted| admitted == id))
            || self
                .executed_order
                .iter()
                .any(|id| !self.selected_order.iter().any(|selected| selected == id))
            || self.next_step.trim().is_empty()
        {
            return Err(ScientificFrontierExecutionError::InvalidOutput(
                "identity, gate partitions, execution order, or next-step contract is invalid"
                    .into(),
            ));
        }
        if self
            .admitted_order
            .iter()
            .any(|id| self.held_order.binary_search(id).is_ok())
            || self
                .admitted_order
                .iter()
                .any(|id| self.blocked_order.binary_search(id).is_ok())
            || self
                .held_order
                .iter()
                .any(|id| self.blocked_order.binary_search(id).is_ok())
        {
            return Err(ScientificFrontierExecutionError::InvalidOutput(
                "frontier gate partitions overlap".into(),
            ));
        }
        if let Some(execution) = &self.execution {
            execution.validate().map_err(|error| {
                ScientificFrontierExecutionError::InvalidOutput(error.to_string())
            })?;
            let result_ids = execution
                .results
                .iter()
                .map(|result| result.action_id.clone())
                .collect::<BTreeSet<_>>();
            if execution.action_order != self.selected_order
                || result_ids != self.executed_order.iter().cloned().collect::<BTreeSet<_>>()
            {
                return Err(ScientificFrontierExecutionError::InvalidOutput(
                    "nested action execution does not reconcile with the frontier batch".into(),
                ));
            }
        } else if !self.selected_order.is_empty() || !self.executed_order.is_empty() {
            return Err(ScientificFrontierExecutionError::InvalidOutput(
                "selected frontier actions require an execution result".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| ScientificFrontierExecutionError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(ScientificFrontierExecutionError::InvalidOutput(
                "frontier execution digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

fn validate_request(
    request: &ScientificFrontierExecutionRequest,
) -> Result<(), ScientificFrontierExecutionError> {
    if request.objective.trim().is_empty()
        || request.candidates.is_empty()
        || !canonical(&request.completed_action_order)
        || request.max_retries > MAX_RETRIES
    {
        return Err(ScientificFrontierExecutionError::InvalidRequest(
            "objective, candidate pool, canonical completed actions, and bounded retries are required".into(),
        ));
    }
    let mut ids = BTreeSet::new();
    if request
        .candidates
        .iter()
        .any(|candidate| !ids.insert(candidate.action_id.clone()))
    {
        return Err(ScientificFrontierExecutionError::InvalidRequest(
            "candidate action identifiers must be unique".into(),
        ));
    }
    Ok(())
}

/// Execute only the scientifically admitted frontier candidates. The executor's selection is
/// checked against the immutable plan, preventing a caller from accidentally passing the raw
/// candidate pool and bypassing knowledge/readiness gates.
pub fn execute_glioma_scientific_frontier<E: GliomaActionExecutor>(
    request: &ScientificFrontierExecutionRequest,
    plan: &ScientificFrontierPlan,
    executor: &mut E,
) -> Result<ScientificFrontierExecution, ScientificFrontierExecutionError> {
    validate_request(request)?;
    plan.validate()
        .map_err(|error| ScientificFrontierExecutionError::InvalidPlan(error.to_string()))?;
    if request.objective != plan.objective || request.plan_digest != plan.digest {
        return Err(ScientificFrontierExecutionError::InvalidRequest(
            "objective and plan digest must match the immutable scientific frontier".into(),
        ));
    }
    let candidate_ids = request
        .candidates
        .iter()
        .map(|candidate| candidate.action_id.clone())
        .collect::<BTreeSet<_>>();
    if candidate_ids
        != plan
            .candidate_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>()
    {
        return Err(ScientificFrontierExecutionError::InvalidRequest(
            "candidate pool does not match the plan candidate order".into(),
        ));
    }
    let admitted = plan.admitted_order.iter().cloned().collect::<BTreeSet<_>>();
    let candidates = request
        .candidates
        .iter()
        .filter(|candidate| admitted.contains(&candidate.action_id))
        .cloned()
        .collect::<Vec<_>>();
    let mut selected_order = plan
        .selection
        .as_ref()
        .map(|selection| selection.selected_order.clone())
        .unwrap_or_default();
    selected_order.sort();
    selected_order.dedup();
    if selected_order != plan.next_action_order {
        return Err(ScientificFrontierExecutionError::InvalidPlan(
            "frontier next-action order does not reconcile with its selector".into(),
        ));
    }
    let mut execution = None;
    let mut executed_order = Vec::new();
    let (disposition, next_step, simulation_only) = if candidates.is_empty()
        || selected_order.is_empty()
    {
        (
            if plan.held_order.is_empty() && plan.blocked_order.is_empty() {
                ScientificFrontierExecutionDisposition::NoRunnableActions
            } else {
                ScientificFrontierExecutionDisposition::Blocked
            },
            "hold until the knowledge/readiness gates admit a runnable frontier batch".into(),
            true,
        )
    } else {
        let result = execute_glioma_action_portfolio(
            &ActionPortfolioExecutionRequest {
                candidates,
                completed_actions: request.completed_action_order.iter().cloned().collect(),
                selection: request.selection.clone(),
                max_retries: request.max_retries,
                require_artifacts: request.require_artifacts,
            },
            executor,
        )
        .map_err(|error: ActionPortfolioExecutionError| {
            ScientificFrontierExecutionError::Execution(error.to_string())
        })?;
        if result.selection.selected_order
            != plan
                .selection
                .as_ref()
                .map(|s| s.selected_order.clone())
                .unwrap_or_default()
            || result.action_order
                != plan
                    .selection
                    .as_ref()
                    .map(|s| s.selected_order.clone())
                    .unwrap_or_default()
        {
            return Err(ScientificFrontierExecutionError::Execution(
                "local executor reselected a batch different from the scientific frontier plan"
                    .into(),
            ));
        }
        executed_order = result
            .results
            .iter()
            .map(|result| result.action_id.clone())
            .collect();
        let simulation = result.results.iter().all(|result| {
            result
                .negative_evidence
                .iter()
                .any(|evidence| evidence == "synthetic-dry-run-not-biological-evidence")
        });
        let disposition = match result.disposition {
            super::action_execution::ActionPortfolioExecutionDisposition::Completed => {
                ScientificFrontierExecutionDisposition::Completed
            }
            super::action_execution::ActionPortfolioExecutionDisposition::Partial => {
                ScientificFrontierExecutionDisposition::Partial
            }
            super::action_execution::ActionPortfolioExecutionDisposition::Failed => {
                ScientificFrontierExecutionDisposition::Failed
            }
            super::action_execution::ActionPortfolioExecutionDisposition::Blocked => {
                ScientificFrontierExecutionDisposition::Blocked
            }
        };
        let next = if matches!(
            disposition,
            ScientificFrontierExecutionDisposition::Completed
        ) {
            "recompile knowledge and multimodal readiness from the returned local artifacts before selecting another frontier batch".into()
        } else {
            "inspect the typed execution stop and replan only from returned artifacts".into()
        };
        execution = Some(result);
        (disposition, next, simulation)
    };
    let mut output = ScientificFrontierExecution {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        plan_digest: request.plan_digest.clone(),
        admitted_order: plan.admitted_order.clone(),
        held_order: plan.held_order.clone(),
        blocked_order: plan.blocked_order.clone(),
        selected_order: plan
            .selection
            .as_ref()
            .map(|selection| selection.selected_order.clone())
            .unwrap_or_default(),
        executed_order,
        execution,
        simulation_only,
        disposition,
        next_step,
        digest: ContentHash::of_bytes(b"unsealed-scientific-frontier-execution"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| ScientificFrontierExecutionError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma_engine::{
        select_glioma_actions, GliomaModality, GliomaModelSystem, GliomaStageKind,
    };
    use bioprism_foundation::{AutonomyTier, Effect};
    use serde_json::json;

    fn candidate(id: &str) -> GliomaActionCandidate {
        GliomaActionCandidate {
            action_id: id.into(),
            stage_kind: GliomaStageKind::MechanismExploration,
            modality: GliomaModality::Genomics,
            model_system: GliomaModelSystem::Organoid,
            depends_on: Vec::new(),
            cost_units: 1,
            information_gain_milli: 900,
            frontier_novelty_milli: 800,
            workflow_leverage_milli: 700,
            cross_stage_unlock_milli: 600,
            reproducibility_safety_milli: 900,
            federation_value_milli: 100,
            feasibility_milli: 900,
            autonomy_tier: AutonomyTier::A1,
            effects: BTreeSet::from([
                Effect::ReadLocalData,
                Effect::ExecuteLocalComputation,
                Effect::WriteLocalArtifact,
            ]),
        }
    }

    fn plan() -> (ScientificFrontierPlan, ScientificFrontierExecutionRequest) {
        let action = candidate("frontier-action");
        let selection_config = GliomaSelectionConfig {
            budget_units: 4,
            max_actions: 1,
            ..GliomaSelectionConfig::default()
        };
        let selection = select_glioma_actions(
            std::slice::from_ref(&action),
            &BTreeSet::new(),
            &selection_config,
        )
        .unwrap();
        let mut plan = ScientificFrontierPlan {
            feature_id: super::super::scientific_frontier::FEATURE_ID.into(),
            output_schema: super::super::scientific_frontier::OUTPUT_SCHEMA.into(),
            objective: "run frontier action".into(),
            knowledge_digest: ContentHash::of_bytes(b"knowledge"),
            frontier_digest: ContentHash::of_bytes(b"frontier"),
            readiness_digest: ContentHash::of_bytes(b"readiness"),
            candidate_order: vec![action.action_id.clone()],
            admitted_order: vec![action.action_id.clone()],
            held_order: Vec::new(),
            blocked_order: Vec::new(),
            gates: vec![super::super::scientific_frontier::FrontierCandidateGate {
                action_id: action.action_id.clone(),
                stage_kind: action.stage_kind,
                status: super::super::scientific_frontier::FrontierCandidateStatus::Admitted,
                reason: "test admission".into(),
            }],
            next_action_order: selection.selected_order.clone(),
            negative_evidence: Vec::new(),
            uncertainty: Vec::new(),
            disposition: super::super::scientific_frontier::ScientificFrontierDisposition::Ready,
            selection: Some(selection),
            digest: ContentHash::of_bytes(b"unsealed"),
        };
        plan.digest = ContentHash::of_value(&json!({
            "feature_id": plan.feature_id,
            "output_schema": plan.output_schema,
            "objective": plan.objective,
            "knowledge_digest": plan.knowledge_digest,
            "frontier_digest": plan.frontier_digest,
            "readiness_digest": plan.readiness_digest,
            "candidate_order": plan.candidate_order,
            "admitted_order": plan.admitted_order,
            "held_order": plan.held_order,
            "blocked_order": plan.blocked_order,
            "gates": plan.gates,
            "selection": plan.selection,
            "next_action_order": plan.next_action_order,
            "negative_evidence": plan.negative_evidence,
            "uncertainty": plan.uncertainty,
            "disposition": plan.disposition,
        }))
        .unwrap();
        let request = ScientificFrontierExecutionRequest {
            objective: plan.objective.clone(),
            plan_digest: plan.digest.clone(),
            candidates: vec![action],
            completed_action_order: Vec::new(),
            selection: selection_config,
            max_retries: 1,
            require_artifacts: true,
        };
        (plan, request)
    }

    #[test]
    fn executes_only_the_admitted_frontier_batch() {
        let (plan, request) = plan();
        let mut executor = super::super::action_execution::DryRunGliomaActionExecutor;
        let output = execute_glioma_scientific_frontier(&request, &plan, &mut executor).unwrap();
        assert_eq!(output.selected_order, vec!["frontier-action"]);
        assert_eq!(output.executed_order, vec!["frontier-action"]);
        assert_eq!(
            output.disposition,
            ScientificFrontierExecutionDisposition::Completed
        );
        assert!(output.simulation_only);
        output.validate().unwrap();
    }

    #[test]
    fn candidate_pool_drift_is_rejected_before_executor_call() {
        let (plan, mut request) = plan();
        request.candidates[0].action_id = "tampered".into();
        let mut executor = super::super::action_execution::DryRunGliomaActionExecutor;
        assert!(matches!(
            execute_glioma_scientific_frontier(&request, &plan, &mut executor),
            Err(ScientificFrontierExecutionError::InvalidRequest(_))
        ));
    }
}
