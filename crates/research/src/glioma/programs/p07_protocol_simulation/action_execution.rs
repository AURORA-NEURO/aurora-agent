//! Execution of a beam-selected preclinical glioma action portfolio.
//!
//! The action selector can now hand a dependency-safe portfolio to a local worker, simulator,
//! analysis service, or instrument gateway.  This module owns the orchestration semantics:
//! bounded retries, dependency closure, typed local artifacts, negative/partial result handling,
//! and fail-closed stopping.  The executor trait is the only effectful seam; the research crate
//! never opens a device connection, moves raw data, or makes a clinical decision.

use crate::glioma_engine::{
    select_glioma_actions, GliomaActionCandidate, GliomaActionSelection, GliomaSelectionConfig,
    LocalArtifactRef,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P07-F19";
pub const OUTPUT_SCHEMA: &str = "GliomaActionPortfolioExecution1@1";
pub const MAX_RETRIES: u8 = 8;
pub const MAX_ACTIONS: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActionPortfolioExecutionRequest {
    pub candidates: Vec<GliomaActionCandidate>,
    pub completed_actions: BTreeSet<String>,
    pub selection: GliomaSelectionConfig,
    pub max_retries: u8,
    pub require_artifacts: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionExecutionDisposition {
    Completed,
    Negative,
    Partial,
    Failed,
    Skipped,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActionExecutionResult {
    pub action_id: String,
    pub disposition: ActionExecutionDisposition,
    pub attempt_count: u8,
    pub artifact: Option<LocalArtifactRef>,
    pub note: String,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionExecutionFailure {
    pub reason: String,
    pub retryable: bool,
}

/// Institution-local workers implement this seam.  Implementations may invoke a deterministic
/// analysis, queue a simulator, or call an already-authorized gateway; they must return a typed
/// local result and never smuggle an untyped effect through the portfolio controller.
pub trait GliomaActionExecutor {
    fn execute_action(
        &mut self,
        candidate: &GliomaActionCandidate,
        attempt: u8,
    ) -> Result<ActionExecutionResult, ActionExecutionFailure>;
}

/// A deterministic executor for demos and integration tests.  It creates only synthetic local
/// artifacts and is explicitly not biological evidence or instrument execution.
#[derive(Debug, Default)]
pub struct DryRunGliomaActionExecutor;

impl GliomaActionExecutor for DryRunGliomaActionExecutor {
    fn execute_action(
        &mut self,
        candidate: &GliomaActionCandidate,
        attempt: u8,
    ) -> Result<ActionExecutionResult, ActionExecutionFailure> {
        let content_hash = ContentHash::of_value(&serde_json::json!({
            "action_id": candidate.action_id,
            "stage_kind": candidate.stage_kind,
            "modality": candidate.modality,
            "model_system": candidate.model_system,
            "attempt": attempt,
            "simulation_only": true,
        }))
        .map_err(|error| ActionExecutionFailure {
            reason: format!("dry-run artifact digest failed: {error}"),
            retryable: false,
        })?;
        Ok(ActionExecutionResult {
            action_id: candidate.action_id.clone(),
            disposition: ActionExecutionDisposition::Completed,
            attempt_count: attempt,
            artifact: Some(LocalArtifactRef {
                artifact_id: format!("dry-run-action:{}", candidate.action_id),
                content_hash,
                content_type: "application/vnd.aurora.glioma.action+json".into(),
                local_only: true,
                contains_human_data: false,
                contains_direct_identifiers: false,
            }),
            note: "dry-run action completed; no biological or instrument effect occurred".into(),
            uncertainty: vec!["simulation-only-result".into()],
            negative_evidence: vec!["synthetic-dry-run-not-biological-evidence".into()],
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionPortfolioExecutionDisposition {
    Completed,
    Partial,
    Failed,
    Blocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionPortfolioStopReason {
    Completed,
    SelectionBlocked,
    ExecutorFailed,
    DependencyBlocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActionPortfolioExecution {
    pub feature_id: String,
    pub output_schema: String,
    pub selection: GliomaActionSelection,
    pub action_order: Vec<String>,
    pub results: Vec<ActionExecutionResult>,
    pub completed_order: Vec<String>,
    pub negative_order: Vec<String>,
    pub partial_order: Vec<String>,
    pub failed_order: Vec<String>,
    pub skipped_order: Vec<String>,
    pub deferred_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub retry_count: u32,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub disposition: ActionPortfolioExecutionDisposition,
    pub stop_reason: ActionPortfolioStopReason,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ActionPortfolioExecutionError {
    #[error("action portfolio execution request is invalid: {0}")]
    InvalidRequest(String),
    #[error("action portfolio execution output is invalid: {0}")]
    InvalidOutput(String),
    #[error("action portfolio execution digest failed: {0}")]
    Digest(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(output: &ActionPortfolioExecution) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "selection": output.selection,
        "action_order": output.action_order,
        "results": output.results,
        "completed_order": output.completed_order,
        "negative_order": output.negative_order,
        "partial_order": output.partial_order,
        "failed_order": output.failed_order,
        "skipped_order": output.skipped_order,
        "deferred_order": output.deferred_order,
        "blocked_order": output.blocked_order,
        "retry_count": output.retry_count,
        "uncertainty": output.uncertainty,
        "negative_evidence": output.negative_evidence,
        "disposition": output.disposition,
        "stop_reason": output.stop_reason,
    })
}

impl ActionPortfolioExecution {
    pub fn validate(&self) -> Result<(), ActionPortfolioExecutionError> {
        self.selection
            .validate()
            .map_err(|error| ActionPortfolioExecutionError::InvalidOutput(error.to_string()))?;
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.action_order != self.selection.selected_order
            || self.action_order.len() != self.results.len()
            || self.action_order.len() > MAX_ACTIONS
            || self.action_order.windows(2).any(|pair| pair[0] == pair[1])
            || !canonical(&self.completed_order)
            || !canonical(&self.negative_order)
            || !canonical(&self.partial_order)
            || !canonical(&self.failed_order)
            || !canonical(&self.skipped_order)
            || !canonical(&self.deferred_order)
            || !canonical(&self.blocked_order)
            || !canonical(&self.uncertainty)
            || !canonical(&self.negative_evidence)
            || self.results.iter().any(|result| {
                result.action_id.trim().is_empty()
                    || result.note.trim().is_empty()
                    || result.uncertainty.iter().any(|item| item.trim().is_empty())
                    || result
                        .negative_evidence
                        .iter()
                        .any(|item| item.trim().is_empty())
                    || match result.disposition {
                        ActionExecutionDisposition::Skipped => result.attempt_count != 0,
                        _ => result.attempt_count == 0,
                    }
            })
            || self.results.iter().any(|result| {
                result
                    .artifact
                    .as_ref()
                    .is_some_and(|artifact| artifact.validate().is_err())
            })
        {
            return Err(ActionPortfolioExecutionError::InvalidOutput(
                "identity, selection, action result, ordering, artifact, or limitation invariants are invalid".into(),
            ));
        }
        let action_ids = self.action_order.iter().cloned().collect::<BTreeSet<_>>();
        let result_ids = self
            .results
            .iter()
            .map(|result| result.action_id.clone())
            .collect::<BTreeSet<_>>();
        if action_ids != result_ids {
            return Err(ActionPortfolioExecutionError::InvalidOutput(
                "action order and result identities do not reconcile".into(),
            ));
        }
        let status_order = |disposition: ActionExecutionDisposition| {
            self.results
                .iter()
                .filter(|result| result.disposition == disposition)
                .map(|result| result.action_id.clone())
                .collect::<BTreeSet<_>>()
        };
        for (label, order, disposition) in [
            (
                "completed",
                &self.completed_order,
                ActionExecutionDisposition::Completed,
            ),
            (
                "negative",
                &self.negative_order,
                ActionExecutionDisposition::Negative,
            ),
            (
                "partial",
                &self.partial_order,
                ActionExecutionDisposition::Partial,
            ),
            (
                "failed",
                &self.failed_order,
                ActionExecutionDisposition::Failed,
            ),
            (
                "skipped",
                &self.skipped_order,
                ActionExecutionDisposition::Skipped,
            ),
        ] {
            if order.iter().cloned().collect::<BTreeSet<_>>() != status_order(disposition) {
                return Err(ActionPortfolioExecutionError::InvalidOutput(format!(
                    "{label} action partition does not reconcile"
                )));
            }
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| ActionPortfolioExecutionError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(ActionPortfolioExecutionError::InvalidOutput(
                "action portfolio execution digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

fn validate_request(
    request: &ActionPortfolioExecutionRequest,
) -> Result<(), ActionPortfolioExecutionError> {
    if request.candidates.is_empty()
        || request.candidates.len() > MAX_ACTIONS
        || request.max_retries > MAX_RETRIES
        || request
            .completed_actions
            .iter()
            .any(|action_id| action_id.trim().is_empty())
    {
        return Err(ActionPortfolioExecutionError::InvalidRequest(
            "non-empty bounded candidates, bounded retries, and non-empty completed action ids are required".into(),
        ));
    }
    Ok(())
}

fn provider_result(
    candidate: &GliomaActionCandidate,
    mut result: ActionExecutionResult,
    attempt: u8,
    require_artifacts: bool,
) -> Result<ActionExecutionResult, ActionPortfolioExecutionError> {
    if result.action_id != candidate.action_id
        || result.note.trim().is_empty()
        || matches!(
            result.disposition,
            ActionExecutionDisposition::Failed | ActionExecutionDisposition::Skipped
        )
        || result.uncertainty.iter().any(|item| item.trim().is_empty())
        || result
            .negative_evidence
            .iter()
            .any(|item| item.trim().is_empty())
    {
        return Err(ActionPortfolioExecutionError::InvalidOutput(format!(
            "executor returned an invalid result for action {}",
            candidate.action_id
        )));
    }
    if require_artifacts && result.artifact.is_none() {
        return Err(ActionPortfolioExecutionError::InvalidOutput(format!(
            "executor returned no required local artifact for action {}",
            candidate.action_id
        )));
    }
    if let Some(artifact) = &result.artifact {
        artifact
            .validate()
            .map_err(|error| ActionPortfolioExecutionError::InvalidOutput(error.to_string()))?;
    }
    result.attempt_count = attempt;
    Ok(result)
}

/// Execute the selected local action portfolio in dependency order.
pub fn execute_glioma_action_portfolio<E: GliomaActionExecutor>(
    request: &ActionPortfolioExecutionRequest,
    executor: &mut E,
) -> Result<ActionPortfolioExecution, ActionPortfolioExecutionError> {
    validate_request(request)?;
    let selection = select_glioma_actions(
        &request.candidates,
        &request.completed_actions,
        &request.selection,
    )
    .map_err(|error| ActionPortfolioExecutionError::InvalidRequest(error.to_string()))?;
    let candidate_map = request
        .candidates
        .iter()
        .map(|candidate| (candidate.action_id.clone(), candidate))
        .collect::<BTreeMap<_, _>>();
    let mut results = Vec::new();
    let mut statuses = request
        .completed_actions
        .iter()
        .map(|action_id| (action_id.clone(), ActionExecutionDisposition::Completed))
        .collect::<BTreeMap<_, _>>();
    let mut retry_count = 0_u32;
    let mut halted = false;
    let mut stop_reason = if selection.selected_order.is_empty() {
        ActionPortfolioStopReason::SelectionBlocked
    } else {
        ActionPortfolioStopReason::Completed
    };
    for action_id in &selection.selected_order {
        let candidate = candidate_map
            .get(action_id)
            .expect("selection was produced from validated candidates");
        if halted {
            let result = ActionExecutionResult {
                action_id: action_id.clone(),
                disposition: ActionExecutionDisposition::Skipped,
                attempt_count: 0,
                artifact: None,
                note: "skipped after an earlier action failed or returned a partial result".into(),
                uncertainty: vec!["dependency-chain-halted".into()],
                negative_evidence: Vec::new(),
            };
            statuses.insert(action_id.clone(), result.disposition);
            results.push(result);
            continue;
        }
        if candidate.depends_on.iter().any(|dependency| {
            !matches!(
                statuses.get(dependency),
                Some(ActionExecutionDisposition::Completed | ActionExecutionDisposition::Negative)
            )
        }) {
            let result = ActionExecutionResult {
                action_id: action_id.clone(),
                disposition: ActionExecutionDisposition::Skipped,
                attempt_count: 0,
                artifact: None,
                note: "skipped because a dependency did not complete or produce a negative result"
                    .into(),
                uncertainty: vec!["dependency-not-qualified".into()],
                negative_evidence: Vec::new(),
            };
            statuses.insert(action_id.clone(), result.disposition);
            results.push(result);
            halted = true;
            stop_reason = ActionPortfolioStopReason::DependencyBlocked;
            continue;
        }
        let mut accepted = None;
        for attempt in 1..=request.max_retries.saturating_add(1) {
            match executor.execute_action(candidate, attempt) {
                Ok(result) => {
                    let result =
                        provider_result(candidate, result, attempt, request.require_artifacts)?;
                    let disposition = result.disposition;
                    if disposition == ActionExecutionDisposition::Partial {
                        halted = true;
                        stop_reason = ActionPortfolioStopReason::DependencyBlocked;
                    }
                    accepted = Some(result);
                    break;
                }
                Err(failure) => {
                    if failure.reason.trim().is_empty() {
                        return Err(ActionPortfolioExecutionError::InvalidOutput(format!(
                            "executor returned an empty failure reason for action {}",
                            candidate.action_id
                        )));
                    }
                    if failure.retryable && attempt <= request.max_retries {
                        retry_count = retry_count.saturating_add(1);
                        continue;
                    }
                    accepted = Some(ActionExecutionResult {
                        action_id: action_id.clone(),
                        disposition: ActionExecutionDisposition::Failed,
                        attempt_count: attempt,
                        artifact: None,
                        note: failure.reason,
                        uncertainty: vec!["executor-failure".into()],
                        negative_evidence: Vec::new(),
                    });
                    halted = true;
                    stop_reason = ActionPortfolioStopReason::ExecutorFailed;
                    break;
                }
            }
        }
        let result = accepted.ok_or_else(|| {
            ActionPortfolioExecutionError::InvalidOutput(format!(
                "executor produced no result for action {}",
                candidate.action_id
            ))
        })?;
        statuses.insert(action_id.clone(), result.disposition);
        results.push(result);
    }

    let mut completed_order = Vec::new();
    let mut negative_order = Vec::new();
    let mut partial_order = Vec::new();
    let mut failed_order = Vec::new();
    let mut skipped_order = Vec::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative_evidence = BTreeSet::new();
    for result in &results {
        match result.disposition {
            ActionExecutionDisposition::Completed => completed_order.push(result.action_id.clone()),
            ActionExecutionDisposition::Negative => negative_order.push(result.action_id.clone()),
            ActionExecutionDisposition::Partial => {
                partial_order.push(result.action_id.clone());
                uncertainty.insert(format!("{}:partial-result", result.action_id));
            }
            ActionExecutionDisposition::Failed => {
                failed_order.push(result.action_id.clone());
                uncertainty.insert(format!("{}:executor-failure", result.action_id));
            }
            ActionExecutionDisposition::Skipped => {
                skipped_order.push(result.action_id.clone());
                uncertainty.insert(format!("{}:skipped", result.action_id));
            }
        }
        uncertainty.extend(result.uncertainty.iter().cloned());
        negative_evidence.extend(result.negative_evidence.iter().cloned());
        if result.disposition == ActionExecutionDisposition::Negative {
            negative_evidence.insert(format!("{}:{}", result.action_id, result.note));
        }
    }
    // Status partitions are canonical sets, while `results` retains executable topological order.
    // Sorting the partitions prevents a high-value action chosen before a lexical prerequisite
    // from making an otherwise valid execution impossible to validate.
    completed_order.sort();
    negative_order.sort();
    partial_order.sort();
    failed_order.sort();
    skipped_order.sort();
    if !selection.deferred_order.is_empty() {
        uncertainty.insert("selection-deferred-actions-remain".into());
    }
    if !selection.blocked_order.is_empty() {
        uncertainty.insert("selection-blocked-actions-remain".into());
    }
    let disposition = if selection.selected_order.is_empty() {
        ActionPortfolioExecutionDisposition::Blocked
    } else if !failed_order.is_empty() {
        ActionPortfolioExecutionDisposition::Failed
    } else if !partial_order.is_empty() || !skipped_order.is_empty() {
        ActionPortfolioExecutionDisposition::Partial
    } else {
        ActionPortfolioExecutionDisposition::Completed
    };
    if disposition == ActionPortfolioExecutionDisposition::Completed {
        stop_reason = ActionPortfolioStopReason::Completed;
    }
    let mut output = ActionPortfolioExecution {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        action_order: selection.selected_order.clone(),
        results,
        completed_order,
        negative_order,
        partial_order,
        failed_order,
        skipped_order,
        deferred_order: selection.deferred_order.clone(),
        blocked_order: selection.blocked_order.clone(),
        retry_count,
        uncertainty: uncertainty.into_iter().collect(),
        negative_evidence: negative_evidence.into_iter().collect(),
        disposition,
        stop_reason,
        selection,
        digest: ContentHash::of_bytes(b"unsealed-glioma-action-portfolio-execution"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| ActionPortfolioExecutionError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma_engine::{GliomaModality, GliomaModelSystem, GliomaStageKind};
    use bioprism_foundation::{AutonomyTier, Effect};

    fn candidate(id: &str, cost: u32, information: u16) -> GliomaActionCandidate {
        GliomaActionCandidate {
            action_id: id.into(),
            stage_kind: GliomaStageKind::ExperimentDesign,
            modality: GliomaModality::Genomics,
            model_system: GliomaModelSystem::Organoid,
            depends_on: Vec::new(),
            cost_units: cost,
            information_gain_milli: information,
            frontier_novelty_milli: 0,
            workflow_leverage_milli: 0,
            cross_stage_unlock_milli: 0,
            reproducibility_safety_milli: 0,
            federation_value_milli: 0,
            feasibility_milli: 0,
            autonomy_tier: AutonomyTier::A1,
            effects: BTreeSet::from([
                Effect::ReadLocalData,
                Effect::ExecuteLocalComputation,
                Effect::WriteLocalArtifact,
            ]),
        }
    }

    fn request(candidates: Vec<GliomaActionCandidate>) -> ActionPortfolioExecutionRequest {
        ActionPortfolioExecutionRequest {
            candidates,
            completed_actions: BTreeSet::new(),
            selection: GliomaSelectionConfig {
                budget_units: 10,
                max_actions: 3,
                ..GliomaSelectionConfig::default()
            },
            max_retries: 1,
            require_artifacts: true,
        }
    }

    #[test]
    fn dry_run_executes_selected_dependency_safe_portfolio() {
        let first = candidate("a", 3, 800);
        let mut second = candidate("b", 3, 700);
        second.depends_on = vec![first.action_id.clone()];
        let mut executor = DryRunGliomaActionExecutor;
        let output =
            execute_glioma_action_portfolio(&request(vec![second, first]), &mut executor).unwrap();
        assert_eq!(
            output.disposition,
            ActionPortfolioExecutionDisposition::Completed
        );
        assert_eq!(output.action_order, vec!["a", "b"]);
        assert_eq!(output.completed_order, vec!["a", "b"]);
        output.validate().unwrap();
    }

    struct RetryOnce {
        attempts: u8,
    }

    impl GliomaActionExecutor for RetryOnce {
        fn execute_action(
            &mut self,
            candidate: &GliomaActionCandidate,
            attempt: u8,
        ) -> Result<ActionExecutionResult, ActionExecutionFailure> {
            self.attempts = self.attempts.saturating_add(1);
            if attempt == 1 {
                return Err(ActionExecutionFailure {
                    reason: "transient local worker lock".into(),
                    retryable: true,
                });
            }
            DryRunGliomaActionExecutor.execute_action(candidate, attempt)
        }
    }

    #[test]
    fn retryable_failure_is_retried_and_bound_to_execution() {
        let mut executor = RetryOnce { attempts: 0 };
        let output =
            execute_glioma_action_portfolio(&request(vec![candidate("a", 2, 800)]), &mut executor)
                .unwrap();
        assert_eq!(executor.attempts, 2);
        assert_eq!(output.retry_count, 1);
        assert_eq!(output.results[0].attempt_count, 2);
        output.validate().unwrap();
    }

    struct Refusing;

    impl GliomaActionExecutor for Refusing {
        fn execute_action(
            &mut self,
            candidate: &GliomaActionCandidate,
            _attempt: u8,
        ) -> Result<ActionExecutionResult, ActionExecutionFailure> {
            Err(ActionExecutionFailure {
                reason: format!("{} refused", candidate.action_id),
                retryable: false,
            })
        }
    }

    #[test]
    fn failed_prerequisite_skips_downstream_action() {
        let first = candidate("a", 3, 800);
        let mut second = candidate("b", 3, 700);
        second.depends_on = vec![first.action_id.clone()];
        let mut request = request(vec![first, second]);
        request.max_retries = 0;
        let mut executor = Refusing;
        let output = execute_glioma_action_portfolio(&request, &mut executor).unwrap();
        assert_eq!(
            output.disposition,
            ActionPortfolioExecutionDisposition::Failed
        );
        assert_eq!(output.failed_order, vec!["a"]);
        assert_eq!(output.skipped_order, vec!["b"]);
        assert_eq!(
            output.stop_reason,
            ActionPortfolioStopReason::ExecutorFailed
        );
        output.validate().unwrap();
    }
}
