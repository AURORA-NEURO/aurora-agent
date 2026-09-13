//! Guarded execution of the adaptive interpretation frontier.
//!
//! P10's adaptive frontier already turns cross-family interpretation debt into typed next
//! actions. This feature closes the product loop: it compiles that frontier, applies an explicit
//! unresolved-synthesis dispatch policy, and executes the selected local actions through the
//! existing dependency-safe action executor. The executor never turns a dry-run artifact into
//! evidence, never bypasses approval or effect gates, and keeps negative, partial, failed, and
//! blocked outcomes available for the next synthesis round.

use super::adaptive_frontier::{
    plan_glioma_adaptive_research_frontier, AdaptiveFrontierDisposition, AdaptiveFrontierError,
    AdaptiveFrontierRequest, AdaptiveResearchFrontier,
};
use crate::glioma::programs::p07_protocol_simulation::{
    execute_glioma_action_portfolio, ActionExecutionDisposition, ActionPortfolioExecution,
    ActionPortfolioExecutionError, ActionPortfolioExecutionRequest, DryRunGliomaActionExecutor,
    GliomaActionExecutor,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P10-F25";
pub const OUTPUT_SCHEMA: &str = "GliomaAdaptiveFrontierExecution1@1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdaptiveFrontierExecutionRequest {
    pub frontier: AdaptiveFrontierRequest,
    pub max_retries: u8,
    pub require_artifacts: bool,
    /// Unresolved synthesis normally holds dispatch. Setting this true still leaves all
    /// candidate approval/effect gates active, but permits local evidence-gap or stress actions
    /// to run so that the next synthesis round can retire the declared debt.
    pub allow_unresolved_dispatch: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdaptiveFrontierExecutionDisposition {
    Executed,
    Negative,
    Partial,
    Failed,
    Held,
    Blocked,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdaptiveFrontierExecution {
    pub feature_id: String,
    pub output_schema: String,
    pub hypothesis: String,
    pub model_system: crate::glioma_engine::GliomaModelSystem,
    pub frontier: AdaptiveResearchFrontier,
    pub dispatched_order: Vec<String>,
    pub execution: Option<ActionPortfolioExecution>,
    pub completed_order: Vec<String>,
    pub negative_order: Vec<String>,
    pub partial_order: Vec<String>,
    pub failed_order: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub disposition: AdaptiveFrontierExecutionDisposition,
    pub next_operator_action: String,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AdaptiveFrontierExecutionError {
    #[error("adaptive frontier execution planning failed: {0}")]
    Frontier(#[from] AdaptiveFrontierError),
    #[error("adaptive frontier execution failed: {0}")]
    Execution(#[from] ActionPortfolioExecutionError),
    #[error("adaptive frontier execution output is invalid: {0}")]
    InvalidOutput(String),
    #[error("adaptive frontier execution digest failed: {0}")]
    Digest(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(output: &AdaptiveFrontierExecution) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "hypothesis": output.hypothesis,
        "model_system": output.model_system,
        "frontier": output.frontier,
        "dispatched_order": output.dispatched_order,
        "execution": output.execution,
        "completed_order": output.completed_order,
        "negative_order": output.negative_order,
        "partial_order": output.partial_order,
        "failed_order": output.failed_order,
        "uncertainty": output.uncertainty,
        "negative_evidence": output.negative_evidence,
        "disposition": output.disposition,
        "next_operator_action": output.next_operator_action,
    })
}

fn disposition(execution: &ActionPortfolioExecution) -> AdaptiveFrontierExecutionDisposition {
    if !execution.negative_order.is_empty() {
        AdaptiveFrontierExecutionDisposition::Negative
    } else {
        match execution.disposition {
            crate::glioma::programs::p07_protocol_simulation::ActionPortfolioExecutionDisposition::Completed => {
                AdaptiveFrontierExecutionDisposition::Executed
            }
            crate::glioma::programs::p07_protocol_simulation::ActionPortfolioExecutionDisposition::Partial => {
                AdaptiveFrontierExecutionDisposition::Partial
            }
            crate::glioma::programs::p07_protocol_simulation::ActionPortfolioExecutionDisposition::Failed => {
                AdaptiveFrontierExecutionDisposition::Failed
            }
            crate::glioma::programs::p07_protocol_simulation::ActionPortfolioExecutionDisposition::Blocked => {
                AdaptiveFrontierExecutionDisposition::Blocked
            }
        }
    }
}

impl AdaptiveFrontierExecution {
    pub fn validate(&self) -> Result<(), AdaptiveFrontierExecutionError> {
        self.frontier
            .validate()
            .map_err(|error| AdaptiveFrontierExecutionError::InvalidOutput(error.to_string()))?;
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.hypothesis.trim().is_empty()
            || self.hypothesis != self.frontier.hypothesis
            || self.model_system != self.frontier.model_system
            || self
                .dispatched_order
                .windows(2)
                .any(|pair| pair[0] == pair[1])
            || !canonical(&self.completed_order)
            || !canonical(&self.negative_order)
            || !canonical(&self.partial_order)
            || !canonical(&self.failed_order)
            || !canonical(&self.uncertainty)
            || !canonical(&self.negative_evidence)
            || self.next_operator_action.trim().is_empty()
        {
            return Err(AdaptiveFrontierExecutionError::InvalidOutput(
                "identity, frontier binding, ordering, or operator-action invariants failed".into(),
            ));
        }
        if let Some(execution) = &self.execution {
            execution.validate().map_err(|error| {
                AdaptiveFrontierExecutionError::InvalidOutput(error.to_string())
            })?;
            if execution.action_order != self.dispatched_order {
                return Err(AdaptiveFrontierExecutionError::InvalidOutput(
                    "execution order does not match the dispatched frontier order".into(),
                ));
            }
            let expected = |kind: ActionExecutionDisposition| {
                let mut ids = execution
                    .results
                    .iter()
                    .filter(|result| result.disposition == kind)
                    .map(|result| result.action_id.clone())
                    .collect::<Vec<_>>();
                ids.sort();
                ids
            };
            if self.completed_order != expected(ActionExecutionDisposition::Completed)
                || self.negative_order != expected(ActionExecutionDisposition::Negative)
                || self.partial_order != expected(ActionExecutionDisposition::Partial)
                || self.failed_order != expected(ActionExecutionDisposition::Failed)
            {
                return Err(AdaptiveFrontierExecutionError::InvalidOutput(
                    "execution status partitions do not reconcile".into(),
                ));
            }
        } else if !matches!(
            self.disposition,
            AdaptiveFrontierExecutionDisposition::Held
                | AdaptiveFrontierExecutionDisposition::Blocked
                | AdaptiveFrontierExecutionDisposition::Unresolved
        ) {
            return Err(AdaptiveFrontierExecutionError::InvalidOutput(
                "an executed disposition requires an action execution".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| AdaptiveFrontierExecutionError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(AdaptiveFrontierExecutionError::InvalidOutput(
                "adaptive frontier execution digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

/// Compile and execute the selected P10 adaptive frontier through a caller-owned action worker.
pub fn execute_glioma_adaptive_frontier<E: GliomaActionExecutor + ?Sized>(
    request: &AdaptiveFrontierExecutionRequest,
    executor: &mut E,
) -> Result<AdaptiveFrontierExecution, AdaptiveFrontierExecutionError> {
    let frontier = plan_glioma_adaptive_research_frontier(&request.frontier)?;
    let mut uncertainty = frontier.uncertainty.clone();
    let mut negative_evidence = frontier.negative_evidence.clone();
    let selected_order = frontier.next_action_order.clone();
    let blocked_by_hold = matches!(frontier.disposition, AdaptiveFrontierDisposition::Hold)
        && !request.allow_unresolved_dispatch;
    let (execution, disposition, next_operator_action) = if selected_order.is_empty() {
        uncertainty.push("adaptive-frontier-selected-no-runnable-actions".into());
        (
            None,
            AdaptiveFrontierExecutionDisposition::Blocked,
            "obtain the missing approval, local artifact, or budget before dispatching the adaptive frontier".into(),
        )
    } else if blocked_by_hold {
        uncertainty.push("adaptive-frontier-unresolved-synthesis-hold".into());
        (
            None,
            AdaptiveFrontierExecutionDisposition::Held,
            "resolve the declared interpretation hold or explicitly permit bounded local dispatch before execution".into(),
        )
    } else {
        let action_request = ActionPortfolioExecutionRequest {
            candidates: frontier
                .candidates
                .iter()
                .map(|candidate| candidate.action.clone())
                .collect(),
            completed_actions: request.frontier.completed_actions.clone(),
            selection: crate::glioma_engine::GliomaSelectionConfig {
                budget_units: request.frontier.budget_units,
                max_actions: request.frontier.max_actions,
                approval_granted: request.frontier.approval_granted,
                allow_instrument_execution: request.frontier.allow_instrument_execution,
                allow_federation: request.frontier.allow_federation,
                weights: request.frontier.selection_weights,
            },
            max_retries: request.max_retries,
            require_artifacts: request.require_artifacts,
        };
        let execution = execute_glioma_action_portfolio(&action_request, executor)?;
        if execution.selection != frontier.selection {
            return Err(AdaptiveFrontierExecutionError::InvalidOutput(
                "action executor changed the planned adaptive selection".into(),
            ));
        }
        negative_evidence.extend(execution.negative_evidence.iter().cloned());
        uncertainty.extend(execution.uncertainty.iter().cloned());
        let disposition = disposition(&execution);
        let next = match disposition {
            AdaptiveFrontierExecutionDisposition::Executed
            | AdaptiveFrontierExecutionDisposition::Negative => {
                format!("resynthesize the returned adaptive artifacts before selecting another frontier: {}", selected_order.join(", "))
            }
            AdaptiveFrontierExecutionDisposition::Partial => {
                "repair or complete the partial frontier results before interpretation promotion".into()
            }
            AdaptiveFrontierExecutionDisposition::Failed => {
                "inspect the typed executor failure and retry only through an approved recovery path".into()
            }
            AdaptiveFrontierExecutionDisposition::Blocked => {
                "resolve dependency, approval, budget, or effect gates before dispatch".into()
            }
            AdaptiveFrontierExecutionDisposition::Held
            | AdaptiveFrontierExecutionDisposition::Unresolved => unreachable!(),
        };
        (Some(execution), disposition, next)
    };
    uncertainty.sort();
    uncertainty.dedup();
    negative_evidence.sort();
    negative_evidence.dedup();
    let (completed_order, negative_order, partial_order, failed_order) = execution
        .as_ref()
        .map(|execution| {
            (
                execution.completed_order.clone(),
                execution.negative_order.clone(),
                execution.partial_order.clone(),
                execution.failed_order.clone(),
            )
        })
        .unwrap_or_default();
    let mut output = AdaptiveFrontierExecution {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        hypothesis: frontier.hypothesis.clone(),
        model_system: frontier.model_system,
        frontier,
        dispatched_order: selected_order,
        execution,
        completed_order,
        negative_order,
        partial_order,
        failed_order,
        uncertainty,
        negative_evidence,
        disposition,
        next_operator_action,
        digest: ContentHash::of_bytes(b"unsealed-glioma-adaptive-frontier-execution"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| AdaptiveFrontierExecutionError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

pub fn dry_run_glioma_adaptive_frontier_executor() -> DryRunGliomaActionExecutor {
    DryRunGliomaActionExecutor
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::programs::p10_interpretation_replication::{
        synthesize_glioma_interpretation, InterpretationEvidence, InterpretationEvidenceDirection,
        InterpretationEvidenceFamily, InterpretationSynthesisDisposition,
        InterpretationSynthesisRequest,
    };
    use crate::glioma_engine::{GliomaModelSystem, GliomaSelectionWeights, LocalArtifactRef};
    use std::collections::BTreeSet;

    fn request(
        disposition: InterpretationSynthesisDisposition,
    ) -> AdaptiveFrontierExecutionRequest {
        let hash = ContentHash::of_bytes(b"adaptive-frontier-execution-test");
        let mut evidence = Vec::new();
        for (id, family) in [
            ("causal", InterpretationEvidenceFamily::CausalContrast),
            ("replication", InterpretationEvidenceFamily::Replication),
            ("sensitivity", InterpretationEvidenceFamily::Sensitivity),
        ] {
            evidence.push(InterpretationEvidence {
                evidence_id: id.into(),
                family,
                independent_group: id.into(),
                model_system: GliomaModelSystem::Organoid,
                direction: if disposition == InterpretationSynthesisDisposition::Negative {
                    InterpretationEvidenceDirection::Null
                } else {
                    InterpretationEvidenceDirection::Positive
                },
                effect_milli: if disposition == InterpretationSynthesisDisposition::Negative {
                    20
                } else {
                    400
                },
                uncertainty_milli: 40,
                quality_milli: 900,
                sample_count: 8,
                artifact: LocalArtifactRef {
                    artifact_id: id.into(),
                    content_hash: hash.clone(),
                    content_type: "application/json".into(),
                    local_only: true,
                    contains_human_data: false,
                    contains_direct_identifiers: false,
                },
                negative_evidence: Vec::new(),
            });
        }
        let synthesis = synthesize_glioma_interpretation(&InterpretationSynthesisRequest {
            objective: "adaptive frontier execution".into(),
            hypothesis: "glioma invasion depends on a preclinical mechanism".into(),
            model_system: GliomaModelSystem::Organoid,
            min_evidence: 2,
            min_independent_groups: 2,
            min_families: if disposition == InterpretationSynthesisDisposition::Unresolved {
                4
            } else {
                2
            },
            min_quality_milli: 700,
            effect_threshold_milli: 100,
            max_disagreement_milli: 700,
            max_leave_one_out_shift_milli: 700,
            require_replication_family: disposition
                != InterpretationSynthesisDisposition::Unresolved,
            replay_identity: hash,
            evidence,
        })
        .unwrap();
        AdaptiveFrontierExecutionRequest {
            frontier: AdaptiveFrontierRequest {
                synthesis,
                completed_actions: BTreeSet::new(),
                budget_units: 80,
                max_actions: 3,
                approval_granted: true,
                allow_instrument_execution: false,
                allow_federation: false,
                selection_weights: GliomaSelectionWeights::default(),
            },
            max_retries: 1,
            require_artifacts: true,
            allow_unresolved_dispatch: false,
        }
    }

    #[test]
    fn qualified_frontier_executes_and_preserves_selection_binding() {
        let mut executor = dry_run_glioma_adaptive_frontier_executor();
        let output = execute_glioma_adaptive_frontier(
            &request(InterpretationSynthesisDisposition::Qualified),
            &mut executor,
        )
        .unwrap();
        assert_eq!(
            output.disposition,
            AdaptiveFrontierExecutionDisposition::Executed
        );
        assert!(output.execution.is_some());
        assert_eq!(output.dispatched_order, output.frontier.next_action_order);
        assert!(output
            .negative_evidence
            .iter()
            .any(|item| item.contains("synthetic-dry-run")));
    }

    #[test]
    fn unresolved_frontier_is_held_until_dispatch_is_explicit() {
        let mut executor = dry_run_glioma_adaptive_frontier_executor();
        let output = execute_glioma_adaptive_frontier(
            &request(InterpretationSynthesisDisposition::Unresolved),
            &mut executor,
        )
        .unwrap();
        assert_eq!(
            output.disposition,
            AdaptiveFrontierExecutionDisposition::Held
        );
        assert!(output.execution.is_none());
        assert!(output
            .uncertainty
            .iter()
            .any(|item| item.contains("unresolved-synthesis-hold")));
    }

    #[test]
    fn repeated_execution_replays_identically() {
        let request = request(InterpretationSynthesisDisposition::Qualified);
        let mut first_executor = dry_run_glioma_adaptive_frontier_executor();
        let mut second_executor = dry_run_glioma_adaptive_frontier_executor();
        let first = execute_glioma_adaptive_frontier(&request, &mut first_executor).unwrap();
        let second = execute_glioma_adaptive_frontier(&request, &mut second_executor).unwrap();
        assert_eq!(first, second);
    }
}
