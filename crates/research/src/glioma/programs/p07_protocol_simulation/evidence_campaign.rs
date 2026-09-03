//! Execute the bounded work selected by the P01 evidence-priority queue.
//!
//! P01 answers which local evidence items deserve attention next.  This module closes the
//! operational loop: a site supplies typed action adapters for the selected queue items and a
//! caller-owned executor runs the dependency-safe subset.  Missing adapters, policy blocks,
//! partial effects, negative results, and executor failures remain explicit so a campaign cannot
//! silently turn an unexecutable queue into a success.

use super::action_execution::{
    execute_glioma_action_portfolio, ActionPortfolioExecution, ActionPortfolioExecutionDisposition,
    ActionPortfolioExecutionError, ActionPortfolioExecutionRequest, GliomaActionExecutor,
    MAX_ACTIONS, MAX_RETRIES,
};
use crate::glioma::programs::p01_evidence_surveillance::EvidencePriorityPlan;
use crate::glioma_engine::{GliomaActionCandidate, GliomaSelectionConfig};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P07-F22";
pub const OUTPUT_SCHEMA: &str = "GliomaEvidenceCampaignExecution1@1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GliomaEvidenceCampaignRequest {
    pub objective: String,
    pub priority: EvidencePriorityPlan,
    pub candidates: Vec<GliomaActionCandidate>,
    pub completed_action_order: Vec<String>,
    pub selection: GliomaSelectionConfig,
    pub max_retries: u8,
    pub require_artifacts: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GliomaEvidenceCampaignDisposition {
    Completed,
    Partial,
    Blocked,
    NoRunnableActions,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GliomaEvidenceCampaignExecution {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub priority_digest: ContentHash,
    pub candidate_order: Vec<String>,
    pub selected_priority_order: Vec<String>,
    pub unavailable_priority_order: Vec<String>,
    pub deferred_priority_order: Vec<String>,
    pub completed_priority_order: Vec<String>,
    pub execution: Option<ActionPortfolioExecution>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: GliomaEvidenceCampaignDisposition,
    pub next_step: String,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum GliomaEvidenceCampaignError {
    #[error("glioma evidence campaign request is invalid: {0}")]
    InvalidRequest(String),
    #[error("glioma evidence-priority plan is invalid: {0}")]
    InvalidPriority(String),
    #[error("glioma evidence campaign execution failed: {0}")]
    Execution(String),
    #[error("glioma evidence campaign output is invalid: {0}")]
    InvalidOutput(String),
    #[error("glioma evidence campaign digest failed: {0}")]
    Digest(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn merge_sorted(left: &[String], right: &[String]) -> Vec<String> {
    left.iter()
        .chain(right.iter())
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn digest_input(output: &GliomaEvidenceCampaignExecution) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "priority_digest": output.priority_digest,
        "candidate_order": output.candidate_order,
        "selected_priority_order": output.selected_priority_order,
        "unavailable_priority_order": output.unavailable_priority_order,
        "deferred_priority_order": output.deferred_priority_order,
        "completed_priority_order": output.completed_priority_order,
        "execution": output.execution,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
        "next_step": output.next_step,
    })
}

impl GliomaEvidenceCampaignExecution {
    pub fn validate(&self) -> Result<(), GliomaEvidenceCampaignError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.priority_digest.as_str().len() != 64
            || !canonical(&self.candidate_order)
            || !canonical(&self.selected_priority_order)
            || !canonical(&self.unavailable_priority_order)
            || !canonical(&self.deferred_priority_order)
            || !canonical(&self.completed_priority_order)
            || !canonical(&self.negative_evidence)
            || !canonical(&self.uncertainty)
            || self.next_step.trim().is_empty()
            || self
                .unavailable_priority_order
                .iter()
                .any(|id| self.selected_priority_order.binary_search(id).is_err())
            || self
                .completed_priority_order
                .iter()
                .any(|id| self.selected_priority_order.binary_search(id).is_err())
        {
            return Err(GliomaEvidenceCampaignError::InvalidOutput(
                "identity, canonical ordering, priority partition, or next-step contract is invalid"
                    .into(),
            ));
        }
        let candidate_ids = self
            .candidate_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let selected = self
            .selected_priority_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let unavailable = self
            .unavailable_priority_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let deferred = self
            .deferred_priority_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let completed = self
            .completed_priority_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        if candidate_ids.len() != self.candidate_order.len()
            || selected.len() != self.selected_priority_order.len()
            || unavailable.len() != self.unavailable_priority_order.len()
            || deferred.len() != self.deferred_priority_order.len()
            || completed.len() != self.completed_priority_order.len()
            || unavailable.intersection(&deferred).next().is_some()
            || unavailable.intersection(&completed).next().is_some()
            || deferred.intersection(&completed).next().is_some()
        {
            return Err(GliomaEvidenceCampaignError::InvalidOutput(
                "candidate or priority partitions do not reconcile".into(),
            ));
        }
        if let Some(execution) = &self.execution {
            execution
                .validate()
                .map_err(|error| GliomaEvidenceCampaignError::InvalidOutput(error.to_string()))?;
            if execution
                .selection
                .selected_order
                .iter()
                .any(|id| !candidate_ids.contains(id))
            {
                return Err(GliomaEvidenceCampaignError::InvalidOutput(
                    "execution selected an action outside the candidate order".into(),
                ));
            }
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| GliomaEvidenceCampaignError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(GliomaEvidenceCampaignError::InvalidOutput(
                "evidence campaign digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

fn validate_request(
    request: &GliomaEvidenceCampaignRequest,
) -> Result<(), GliomaEvidenceCampaignError> {
    if request.objective.trim().is_empty()
        || !canonical(&request.completed_action_order)
        || request
            .completed_action_order
            .iter()
            .any(|id| id.trim().is_empty())
        || request.candidates.len() > MAX_ACTIONS
        || request.max_retries > MAX_RETRIES
    {
        return Err(GliomaEvidenceCampaignError::InvalidRequest(
            "objective, canonical completed actions, bounded candidates, and bounded retries are required"
                .into(),
        ));
    }
    Ok(())
}

fn candidate_closure(
    candidates: &[GliomaActionCandidate],
    allowed: &BTreeSet<String>,
    completed: &BTreeSet<String>,
) -> BTreeSet<String> {
    let by_id = candidates
        .iter()
        .map(|candidate| (candidate.action_id.as_str(), candidate))
        .collect::<std::collections::BTreeMap<_, _>>();
    let mut closure = allowed.clone();
    loop {
        let before = closure.len();
        let dependencies = closure
            .iter()
            .filter_map(|id| by_id.get(id.as_str()))
            .flat_map(|candidate| candidate.depends_on.iter())
            .filter(|dependency| !completed.contains(*dependency))
            .filter(|dependency| by_id.contains_key(dependency.as_str()))
            .cloned()
            .collect::<Vec<_>>();
        closure.extend(dependencies);
        if closure.len() == before {
            break;
        }
    }
    closure
}

/// Execute the pending portion of a P01 evidence-priority plan through typed local adapters.
///
/// A priority action without a supplied candidate is returned as `unavailable`, not discarded.
/// Candidates are dependency-closed before they reach the common executor, so a provider cannot
/// accidentally run a downstream assay without its declared local prerequisite.
pub fn execute_glioma_evidence_campaign<E: GliomaActionExecutor>(
    request: &GliomaEvidenceCampaignRequest,
    executor: &mut E,
) -> Result<GliomaEvidenceCampaignExecution, GliomaEvidenceCampaignError> {
    validate_request(request)?;
    request
        .priority
        .validate()
        .map_err(|error| GliomaEvidenceCampaignError::InvalidPriority(error.to_string()))?;
    if request.objective.trim() != request.priority.objective.trim() {
        return Err(GliomaEvidenceCampaignError::InvalidRequest(
            "campaign objective must match the evidence-priority objective".into(),
        ));
    }
    let completed = request
        .completed_action_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    if completed
        .iter()
        .any(|id| request.priority.action_order.binary_search(id).is_err())
    {
        return Err(GliomaEvidenceCampaignError::InvalidRequest(
            "completed action is outside the evidence-priority plan".into(),
        ));
    }
    let candidate_ids = request
        .candidates
        .iter()
        .map(|candidate| candidate.action_id.clone())
        .collect::<BTreeSet<_>>();
    if candidate_ids.len() != request.candidates.len()
        || candidate_ids
            .iter()
            .any(|id| request.priority.action_order.binary_search(id).is_err())
    {
        return Err(GliomaEvidenceCampaignError::InvalidRequest(
            "candidate ids must be unique and belong to the evidence-priority plan".into(),
        ));
    }
    let selected_priority_order = request.priority.selected_order.clone();
    let pending_selected_order = selected_priority_order
        .iter()
        .filter(|id| !completed.contains(*id))
        .cloned()
        .collect::<Vec<_>>();
    let unavailable_priority_order = pending_selected_order
        .iter()
        .filter(|id| !candidate_ids.contains(*id))
        .cloned()
        .collect::<Vec<_>>();
    let selected_available = pending_selected_order
        .iter()
        .filter(|id| candidate_ids.contains(*id))
        .cloned()
        .collect::<BTreeSet<_>>();
    let allowed = candidate_closure(&request.candidates, &selected_available, &completed);
    let candidates = request
        .candidates
        .iter()
        .filter(|candidate| allowed.contains(&candidate.action_id))
        .cloned()
        .collect::<Vec<_>>();
    let candidate_order = candidates
        .iter()
        .map(|candidate| candidate.action_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let completed_priority_order = completed.iter().cloned().collect::<Vec<_>>();
    let execution = if candidates.is_empty() {
        None
    } else {
        Some(
            execute_glioma_action_portfolio(
                &ActionPortfolioExecutionRequest {
                    candidates,
                    completed_actions: completed.clone(),
                    selection: request.selection.clone(),
                    max_retries: request.max_retries,
                    require_artifacts: request.require_artifacts,
                },
                executor,
            )
            .map_err(|error: ActionPortfolioExecutionError| {
                GliomaEvidenceCampaignError::Execution(error.to_string())
            })?,
        )
    };
    let executed = execution
        .as_ref()
        .map(|run| run.action_order.iter().cloned().collect::<BTreeSet<_>>())
        .unwrap_or_default();
    let deferred_priority_order = request
        .priority
        .action_order
        .iter()
        .filter(|id| {
            !completed.contains(*id)
                && !unavailable_priority_order.iter().any(|item| item == *id)
                && !executed.contains(*id)
        })
        .cloned()
        .collect::<Vec<_>>();
    let execution_negative = execution
        .as_ref()
        .map(|run| run.negative_evidence.clone())
        .unwrap_or_default();
    let execution_uncertainty = execution
        .as_ref()
        .map(|run| run.uncertainty.clone())
        .unwrap_or_default();
    let disposition = match execution.as_ref().map(|run| run.disposition) {
        None if !unavailable_priority_order.is_empty() => {
            GliomaEvidenceCampaignDisposition::Blocked
        }
        None if pending_selected_order.is_empty() => {
            if request.priority.uncertainty_order.is_empty() {
                GliomaEvidenceCampaignDisposition::NoRunnableActions
            } else {
                GliomaEvidenceCampaignDisposition::Unresolved
            }
        }
        None => GliomaEvidenceCampaignDisposition::Blocked,
        Some(ActionPortfolioExecutionDisposition::Completed)
            if unavailable_priority_order.is_empty() && deferred_priority_order.is_empty() =>
        {
            GliomaEvidenceCampaignDisposition::Completed
        }
        Some(ActionPortfolioExecutionDisposition::Completed) => {
            GliomaEvidenceCampaignDisposition::Partial
        }
        Some(ActionPortfolioExecutionDisposition::Partial) => {
            GliomaEvidenceCampaignDisposition::Partial
        }
        Some(ActionPortfolioExecutionDisposition::Failed)
        | Some(ActionPortfolioExecutionDisposition::Blocked) => {
            GliomaEvidenceCampaignDisposition::Blocked
        }
    };
    let next_step = match disposition {
        GliomaEvidenceCampaignDisposition::Completed => {
            "rebuild the local evidence snapshot from returned artifacts before the next cycle"
        }
        GliomaEvidenceCampaignDisposition::Partial => {
            "retain returned artifacts and requeue deferred or unresolved evidence actions"
        }
        GliomaEvidenceCampaignDisposition::Blocked => {
            "install or authorize the missing adapter, inspect the executor stop reason, then resume"
        }
        GliomaEvidenceCampaignDisposition::NoRunnableActions => {
            "wait for a new evidence record or a newly admitted action adapter"
        }
        GliomaEvidenceCampaignDisposition::Unresolved => {
            "resolve the evidence uncertainty before admitting another autonomous action"
        }
    };
    let mut output = GliomaEvidenceCampaignExecution {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        priority_digest: request.priority.digest.clone(),
        candidate_order,
        selected_priority_order,
        unavailable_priority_order,
        deferred_priority_order,
        completed_priority_order,
        execution,
        negative_evidence: merge_sorted(
            &request.priority.negative_evidence_order,
            &execution_negative,
        ),
        uncertainty: merge_sorted(&request.priority.uncertainty_order, &execution_uncertainty),
        disposition,
        next_step: next_step.into(),
        digest: ContentHash::of_bytes(b"unsealed-glioma-evidence-campaign"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| GliomaEvidenceCampaignError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::evidence::EvidenceSourceKind;
    use crate::glioma::programs::p01_evidence_surveillance::{
        prioritize_glioma_evidence, EvidencePriorityRequest, EvidencePriorityWeights,
    };
    use crate::glioma_engine::{
        GliomaModality, GliomaModelSystem, GliomaStageKind, LocalArtifactRef,
    };
    use bioprism_foundation::{AutonomyTier, Effect};
    use bioprism_ids::ContentHash;

    fn artifact(id: &str) -> LocalArtifactRef {
        LocalArtifactRef {
            artifact_id: format!("artifact:{id}"),
            content_hash: ContentHash::of_bytes(id.as_bytes()),
            content_type: "application/vnd.aurora.glioma-evidence+json".into(),
            local_only: true,
            contains_human_data: false,
            contains_direct_identifiers: false,
        }
    }

    fn priority() -> EvidencePriorityPlan {
        prioritize_glioma_evidence(
            &EvidencePriorityRequest {
                objective: "refresh glioma evidence".into(),
                current_epoch: 10,
                recency_half_life_epochs: 4,
                required_modalities: BTreeSet::new(),
                required_model_systems: BTreeSet::new(),
                max_actions: 2,
                min_priority_milli: 0,
                weights: EvidencePriorityWeights::default(),
            },
            &[crate::glioma::evidence::EvidenceRecord {
                evidence_id: "evidence-1".into(),
                source_artifact: artifact("evidence-1"),
                source_kind: EvidenceSourceKind::Dataset,
                claim: "invasion claim".into(),
                scope: "preclinical glioma".into(),
                modality: GliomaModality::Genomics,
                model_system: Some(GliomaModelSystem::Organoid),
                state: crate::glioma::evidence::EvidenceState::Stale,
                relevance_milli: 900,
                quality_milli: 900,
                reproducibility_milli: 900,
                release_epoch: 1,
            }],
        )
        .unwrap()
    }

    fn candidate(action_id: &str, approval: bool) -> GliomaActionCandidate {
        GliomaActionCandidate {
            action_id: action_id.into(),
            stage_kind: GliomaStageKind::EvidenceSurveillance,
            modality: GliomaModality::Genomics,
            model_system: GliomaModelSystem::Organoid,
            depends_on: Vec::new(),
            cost_units: 1,
            information_gain_milli: 900,
            frontier_novelty_milli: 800,
            workflow_leverage_milli: 800,
            cross_stage_unlock_milli: 800,
            reproducibility_safety_milli: 900,
            federation_value_milli: 500,
            feasibility_milli: 900,
            autonomy_tier: if approval {
                AutonomyTier::A3
            } else {
                AutonomyTier::A1
            },
            effects: BTreeSet::from([
                Effect::ReadLocalData,
                Effect::ExecuteLocalComputation,
                Effect::WriteLocalArtifact,
            ]),
        }
    }

    fn request(
        priority: EvidencePriorityPlan,
        candidates: Vec<GliomaActionCandidate>,
    ) -> GliomaEvidenceCampaignRequest {
        GliomaEvidenceCampaignRequest {
            objective: priority.objective.clone(),
            priority,
            candidates,
            completed_action_order: Vec::new(),
            selection: GliomaSelectionConfig {
                budget_units: 2,
                max_actions: 1,
                approval_granted: true,
                allow_instrument_execution: false,
                allow_federation: false,
                weights: Default::default(),
            },
            max_retries: 1,
            require_artifacts: true,
        }
    }

    #[test]
    fn executes_priority_action_through_local_executor() {
        let priority = priority();
        let action_id = priority.selected_order[0].clone();
        let mut executor = super::super::action_execution::DryRunGliomaActionExecutor;
        let output = execute_glioma_evidence_campaign(
            &request(priority, vec![candidate(&action_id, false)]),
            &mut executor,
        )
        .unwrap();
        assert_eq!(
            output.disposition,
            GliomaEvidenceCampaignDisposition::Completed
        );
        assert_eq!(output.candidate_order, vec![action_id]);
        assert!(output.execution.is_some());
        output.validate().unwrap();
    }

    #[test]
    fn missing_adapter_is_explicitly_blocked() {
        let priority = priority();
        let mut executor = super::super::action_execution::DryRunGliomaActionExecutor;
        let output =
            execute_glioma_evidence_campaign(&request(priority, Vec::new()), &mut executor)
                .unwrap();
        assert_eq!(
            output.disposition,
            GliomaEvidenceCampaignDisposition::Blocked
        );
        assert_eq!(output.unavailable_priority_order.len(), 1);
        assert!(output.execution.is_none());
    }

    #[test]
    fn approval_gate_blocks_without_running_an_action() {
        let priority = priority();
        let action_id = priority.selected_order[0].clone();
        let mut request = request(priority, vec![candidate(&action_id, true)]);
        request.selection.approval_granted = false;
        let mut executor = super::super::action_execution::DryRunGliomaActionExecutor;
        let output = execute_glioma_evidence_campaign(&request, &mut executor).unwrap();
        assert_eq!(
            output.disposition,
            GliomaEvidenceCampaignDisposition::Blocked
        );
        assert!(output.execution.is_some());
        assert!(output
            .execution
            .as_ref()
            .unwrap()
            .selection
            .blocked_order
            .contains(&action_id));
    }

    #[test]
    fn completed_priority_action_is_not_replayed() {
        let priority = priority();
        let action_id = priority.selected_order[0].clone();
        let mut request = request(priority, Vec::new());
        request.completed_action_order = vec![action_id.clone()];
        let mut executor = super::super::action_execution::DryRunGliomaActionExecutor;
        let output = execute_glioma_evidence_campaign(&request, &mut executor).unwrap();
        assert_eq!(
            output.disposition,
            GliomaEvidenceCampaignDisposition::Unresolved
        );
        assert_eq!(output.selected_priority_order, vec![action_id.clone()]);
        assert_eq!(output.completed_priority_order, vec![action_id]);
        assert!(output.execution.is_none());
        output.validate().unwrap();
    }
}
