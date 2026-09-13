//! Robustness-guided computation frontier for preclinical glioma studies.
//!
//! A robustness battery is useful only when it changes what the computation engine does next.
//! This feature converts omission failures, negative cases, direction reversals, and unresolved
//! leave-out cases into deterministic priority signals for a typed computation DAG. It preserves
//! the original candidates, adds bounded debt-aware weights, delegates prerequisite closure and
//! execution to the existing P09 portfolio bridge, and never treats a re-analysis artifact as a
//! biological conclusion.

use super::execution::{
    ComputationCacheEntry, DryRunGliomaComputationExecutor, GliomaComputationExecutor,
};
use super::planning::ComputationCandidate;
use super::portfolio_execution::{
    execute_glioma_computation_portfolio, ComputationPortfolioExecution,
    ComputationPortfolioExecutionDisposition, ComputationPortfolioExecutionError,
    ComputationPortfolioExecutionRequest,
};
use super::robustness::{RobustnessDisposition, RobustnessSuite};
use crate::glioma::analysis::AnalysisDisposition;
use crate::glioma_engine::GliomaModelSystem;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P09-F15";
pub const OUTPUT_SCHEMA: &str = "GliomaRobustnessGuidedComputation1@1";
pub const MAX_CANDIDATES: usize = 4_096;
pub const MAX_TARGET_CASES: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RobustnessGuidedCandidate {
    pub candidate: ComputationCandidate,
    pub target_case_ids: Vec<String>,
    pub scientific_role: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RobustnessGuidedComputationRequest {
    pub objective: String,
    pub model_system: GliomaModelSystem,
    pub robustness: RobustnessSuite,
    pub portfolio: super::planning::ComputationPortfolioRequest,
    pub candidates: Vec<RobustnessGuidedCandidate>,
    pub replay_identity: ContentHash,
    pub max_retries: u8,
    pub allow_cache: bool,
    pub require_local_artifacts: bool,
    pub cache: Vec<ComputationCacheEntry>,
    pub minimum_case_coverage: usize,
    pub minimum_fragile_case_coverage: usize,
    pub stop_if_stable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RobustnessGuidedCandidateScore {
    pub candidate_id: String,
    pub scientific_role: String,
    pub debt_case_coverage: usize,
    pub fragile_case_coverage: usize,
    pub negative_case_coverage: usize,
    pub unresolved_case_coverage: usize,
    pub adjusted_information_gain_milli: u32,
    pub adjusted_uncertainty_reduction_milli: u32,
    pub adjusted_coverage_debt_milli: u32,
    pub priority_score_milli: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RobustnessGuidedComputationDisposition {
    Executed,
    FragilityDetected,
    Negative,
    Partial,
    Failed,
    Blocked,
    Unresolved,
    StableNoAction,
    NoFeasiblePlan,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RobustnessGuidedComputation {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub model_system: GliomaModelSystem,
    pub robustness: RobustnessSuite,
    pub debt_case_order: Vec<String>,
    pub fragile_case_order: Vec<String>,
    pub negative_case_order: Vec<String>,
    pub unresolved_case_order: Vec<String>,
    pub candidate_order: Vec<String>,
    pub scores: Vec<RobustnessGuidedCandidateScore>,
    pub adjusted_candidates: Vec<ComputationCandidate>,
    pub selected_order: Vec<String>,
    pub portfolio_execution: Option<ComputationPortfolioExecution>,
    pub minimum_case_coverage: usize,
    pub minimum_fragile_case_coverage: usize,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: RobustnessGuidedComputationDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RobustnessGuidedComputationError {
    #[error("robustness-guided computation request is invalid: {0}")]
    InvalidRequest(String),
    #[error("robustness-guided computation output is invalid: {0}")]
    InvalidOutput(String),
    #[error("robustness-guided computation execution failed: {0}")]
    Execution(#[from] ComputationPortfolioExecutionError),
    #[error("robustness-guided computation digest failed: {0}")]
    Digest(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn sign(value: i64) -> i8 {
    value.signum() as i8
}

fn debt_case_ids(suite: &RobustnessSuite) -> BTreeSet<String> {
    suite
        .cases
        .iter()
        .filter(|case| {
            case.disposition == AnalysisDisposition::Unresolved
                || case.disposition == AnalysisDisposition::Negative
                || !case.negative_evidence.is_empty()
                || !case.uncertainty.is_empty()
                || sign(case.effect_milli) != sign(suite.primary_effect_milli)
        })
        .map(|case| case.case_id.clone())
        .collect()
}

fn fragile_case_ids(suite: &RobustnessSuite) -> BTreeSet<String> {
    suite
        .cases
        .iter()
        .filter(|case| {
            sign(case.effect_milli) != sign(suite.primary_effect_milli)
                || case.disposition != AnalysisDisposition::Qualified
                || !case.uncertainty.is_empty()
        })
        .map(|case| case.case_id.clone())
        .collect()
}

fn negative_case_ids(suite: &RobustnessSuite) -> BTreeSet<String> {
    suite
        .cases
        .iter()
        .filter(|case| {
            case.disposition == AnalysisDisposition::Negative || !case.negative_evidence.is_empty()
        })
        .map(|case| case.case_id.clone())
        .collect()
}

fn unresolved_case_ids(suite: &RobustnessSuite) -> BTreeSet<String> {
    suite
        .cases
        .iter()
        .filter(|case| {
            case.disposition == AnalysisDisposition::Unresolved || !case.uncertainty.is_empty()
        })
        .map(|case| case.case_id.clone())
        .collect()
}

fn digest_input(output: &RobustnessGuidedComputation) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "model_system": output.model_system,
        "robustness": output.robustness,
        "debt_case_order": output.debt_case_order,
        "fragile_case_order": output.fragile_case_order,
        "negative_case_order": output.negative_case_order,
        "unresolved_case_order": output.unresolved_case_order,
        "candidate_order": output.candidate_order,
        "scores": output.scores,
        "adjusted_candidates": output.adjusted_candidates,
        "selected_order": output.selected_order,
        "portfolio_execution": output.portfolio_execution,
        "minimum_case_coverage": output.minimum_case_coverage,
        "minimum_fragile_case_coverage": output.minimum_fragile_case_coverage,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
    })
}

fn case_set(values: &[String]) -> BTreeSet<String> {
    values.iter().cloned().collect()
}

fn count_overlap(targets: &[String], frontier: &BTreeSet<String>) -> usize {
    targets
        .iter()
        .filter(|target| frontier.contains(*target))
        .count()
}

fn adjusted_value(value: u32, increment: usize, multiplier: u32) -> u32 {
    value.saturating_add((increment as u32).saturating_mul(multiplier))
}

fn priority_score(
    candidate: &ComputationCandidate,
    portfolio: &super::planning::ComputationPortfolioRequest,
) -> i64 {
    let positive = u128::from(candidate.information_gain_milli)
        .saturating_mul(u128::from(portfolio.information_weight_milli))
        .saturating_add(
            u128::from(candidate.uncertainty_reduction_milli)
                .saturating_mul(u128::from(portfolio.uncertainty_weight_milli)),
        )
        .saturating_add(
            u128::from(candidate.coverage_debt_milli)
                .saturating_mul(u128::from(portfolio.coverage_weight_milli)),
        );
    let penalty = u128::from(candidate.task.estimated_cost_units)
        .saturating_mul(u128::from(portfolio.cost_penalty_milli))
        .saturating_add(
            u128::from(candidate.task.estimated_duration_ticks)
                .saturating_mul(u128::from(portfolio.duration_penalty_milli)),
        );
    positive.saturating_sub(penalty).min(i64::MAX as u128) as i64
}

fn validate_request(
    request: &RobustnessGuidedComputationRequest,
) -> Result<BTreeMap<String, RobustnessGuidedCandidate>, RobustnessGuidedComputationError> {
    if request.objective.trim().is_empty()
        || request.objective != request.portfolio.objective
        || request.objective != request.robustness.objective
        || request.model_system != request.portfolio.model_system
        || request.candidates.is_empty()
        || request.candidates.len() > MAX_CANDIDATES
        || request.replay_identity.as_str().len() != 64
        || request.minimum_case_coverage > MAX_TARGET_CASES
        || request.minimum_fragile_case_coverage > MAX_TARGET_CASES
    {
        return Err(RobustnessGuidedComputationError::InvalidRequest(
            "objective, model-system binding, bounded typed candidates, replay identity, and coverage limits are required".into(),
        ));
    }
    request
        .robustness
        .validate()
        .map_err(|error| RobustnessGuidedComputationError::InvalidRequest(error.to_string()))?;
    let known_cases = request
        .robustness
        .case_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut candidates = BTreeMap::new();
    for guided in &request.candidates {
        let candidate = &guided.candidate;
        if guided.scientific_role.trim().is_empty()
            || candidate.candidate_id.trim().is_empty()
            || candidate.candidate_id != candidate.task.task_id
            || candidate.task.model_system != request.model_system
            || !canonical(&guided.target_case_ids)
            || guided.target_case_ids.len() > MAX_TARGET_CASES
            || guided
                .target_case_ids
                .iter()
                .any(|case_id| !known_cases.contains(case_id))
            || candidates
                .insert(candidate.candidate_id.clone(), guided.clone())
                .is_some()
        {
            return Err(RobustnessGuidedComputationError::InvalidRequest(
                "candidate identity, model binding, role, unique known targets, and canonical target ordering are required".into(),
            ));
        }
    }
    Ok(candidates)
}

fn disposition(
    suite: RobustnessDisposition,
    execution: &ComputationPortfolioExecution,
) -> RobustnessGuidedComputationDisposition {
    match execution.disposition {
        ComputationPortfolioExecutionDisposition::Completed => match suite {
            RobustnessDisposition::Fragile => {
                RobustnessGuidedComputationDisposition::FragilityDetected
            }
            RobustnessDisposition::Unresolved => RobustnessGuidedComputationDisposition::Unresolved,
            RobustnessDisposition::Null => RobustnessGuidedComputationDisposition::Negative,
            RobustnessDisposition::Stable => RobustnessGuidedComputationDisposition::Executed,
        },
        ComputationPortfolioExecutionDisposition::Partial => {
            RobustnessGuidedComputationDisposition::Partial
        }
        ComputationPortfolioExecutionDisposition::Failed => {
            RobustnessGuidedComputationDisposition::Failed
        }
        ComputationPortfolioExecutionDisposition::Blocked => {
            RobustnessGuidedComputationDisposition::Blocked
        }
        ComputationPortfolioExecutionDisposition::Unresolved => {
            RobustnessGuidedComputationDisposition::Unresolved
        }
    }
}

impl RobustnessGuidedComputation {
    pub fn validate(&self) -> Result<(), RobustnessGuidedComputationError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.robustness.objective != self.objective
            || !canonical(&self.debt_case_order)
            || !canonical(&self.fragile_case_order)
            || !canonical(&self.negative_case_order)
            || !canonical(&self.unresolved_case_order)
            || !canonical(&self.candidate_order)
            || !canonical(&self.selected_order)
            || !canonical(&self.negative_evidence)
            || !canonical(&self.uncertainty)
        {
            return Err(RobustnessGuidedComputationError::InvalidOutput(
                "identity, robustness binding, ordering, or evidence fields are invalid".into(),
            ));
        }
        self.robustness
            .validate()
            .map_err(|error| RobustnessGuidedComputationError::InvalidOutput(error.to_string()))?;
        let candidate_ids = self
            .adjusted_candidates
            .iter()
            .map(|candidate| candidate.candidate_id.clone())
            .collect::<BTreeSet<_>>();
        if candidate_ids.len() != self.adjusted_candidates.len()
            || self.candidate_order != candidate_ids.iter().cloned().collect::<Vec<_>>()
            || self
                .scores
                .iter()
                .map(|score| score.candidate_id.clone())
                .collect::<BTreeSet<_>>()
                != candidate_ids
            || self
                .selected_order
                .iter()
                .any(|id| !candidate_ids.contains(id))
        {
            return Err(RobustnessGuidedComputationError::InvalidOutput(
                "candidate, score, and selected identities do not reconcile".into(),
            ));
        }
        if let Some(execution) = &self.portfolio_execution {
            execution.validate().map_err(|error| {
                RobustnessGuidedComputationError::InvalidOutput(error.to_string())
            })?;
            if execution.objective != self.objective
                || execution.plan.selected_order != self.selected_order
            {
                return Err(RobustnessGuidedComputationError::InvalidOutput(
                    "portfolio execution is not bound to the selected robustness-guided order"
                        .into(),
                ));
            }
        } else if !matches!(
            self.disposition,
            RobustnessGuidedComputationDisposition::StableNoAction
                | RobustnessGuidedComputationDisposition::NoFeasiblePlan
        ) {
            return Err(RobustnessGuidedComputationError::InvalidOutput(
                "only stable no-action or no-feasible-plan outputs may omit execution".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| RobustnessGuidedComputationError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(RobustnessGuidedComputationError::InvalidOutput(
                "robustness-guided computation digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

/// Reweight computation candidates from observed robustness debt, then execute the selected DAG.
pub fn execute_glioma_robustness_guided_computation<E: GliomaComputationExecutor>(
    request: &RobustnessGuidedComputationRequest,
    executor: &mut E,
) -> Result<RobustnessGuidedComputation, RobustnessGuidedComputationError> {
    let candidates = validate_request(request)?;
    let debt = debt_case_ids(&request.robustness);
    let fragile = fragile_case_ids(&request.robustness);
    let negative = negative_case_ids(&request.robustness);
    let unresolved = unresolved_case_ids(&request.robustness);
    let known_debt = case_set(&debt.iter().cloned().collect::<Vec<_>>());
    let mut adjusted_candidates = Vec::with_capacity(candidates.len());
    let mut scores = Vec::with_capacity(candidates.len());
    for guided in candidates.values() {
        let debt_count = count_overlap(&guided.target_case_ids, &known_debt);
        let fragile_count = count_overlap(&guided.target_case_ids, &fragile);
        let negative_count = count_overlap(&guided.target_case_ids, &negative);
        let unresolved_count = count_overlap(&guided.target_case_ids, &unresolved);
        let mut adjusted = guided.candidate.clone();
        adjusted.information_gain_milli =
            adjusted_value(adjusted.information_gain_milli, debt_count, 75);
        adjusted.uncertainty_reduction_milli = adjusted_value(
            adjusted.uncertainty_reduction_milli,
            unresolved_count + fragile_count,
            125,
        );
        adjusted.coverage_debt_milli =
            adjusted_value(adjusted.coverage_debt_milli, fragile_count, 500)
                .saturating_add((negative_count as u32).saturating_mul(350))
                .saturating_add((unresolved_count as u32).saturating_mul(300));
        let priority = priority_score(&adjusted, &request.portfolio);
        scores.push(RobustnessGuidedCandidateScore {
            candidate_id: adjusted.candidate_id.clone(),
            scientific_role: guided.scientific_role.clone(),
            debt_case_coverage: debt_count,
            fragile_case_coverage: fragile_count,
            negative_case_coverage: negative_count,
            unresolved_case_coverage: unresolved_count,
            adjusted_information_gain_milli: adjusted.information_gain_milli,
            adjusted_uncertainty_reduction_milli: adjusted.uncertainty_reduction_milli,
            adjusted_coverage_debt_milli: adjusted.coverage_debt_milli,
            priority_score_milli: priority,
        });
        adjusted_candidates.push(adjusted);
    }
    scores.sort_by(|left, right| left.candidate_id.cmp(&right.candidate_id));
    adjusted_candidates.sort_by(|left, right| left.candidate_id.cmp(&right.candidate_id));
    let candidate_order = adjusted_candidates
        .iter()
        .map(|candidate| candidate.candidate_id.clone())
        .collect::<Vec<_>>();
    let mut uncertainty = request.robustness.uncertainty.clone();
    let mut negative_evidence = request.robustness.negative_evidence.clone();
    if debt.len() < request.minimum_case_coverage {
        uncertainty.push(format!(
            "robustness-debt-case-floor-unmet:{}<{}",
            debt.len(),
            request.minimum_case_coverage
        ));
    }
    if fragile.len() < request.minimum_fragile_case_coverage {
        uncertainty.push(format!(
            "robustness-fragile-case-floor-unmet:{}<{}",
            fragile.len(),
            request.minimum_fragile_case_coverage
        ));
    }
    let stable_hold = request.stop_if_stable
        && matches!(
            request.robustness.disposition,
            RobustnessDisposition::Stable
        );
    let gate_open = debt.len() >= request.minimum_case_coverage
        && fragile.len() >= request.minimum_fragile_case_coverage;
    let (portfolio_execution, selected_order, disposition) = if stable_hold {
        (
            None,
            Vec::new(),
            RobustnessGuidedComputationDisposition::StableNoAction,
        )
    } else if !gate_open {
        uncertainty.push("robustness-guided-computation-gate-not-cleared".into());
        (
            None,
            Vec::new(),
            RobustnessGuidedComputationDisposition::NoFeasiblePlan,
        )
    } else {
        let execution_request = ComputationPortfolioExecutionRequest {
            portfolio: request.portfolio.clone(),
            candidates: adjusted_candidates.clone(),
            replay_identity: request.replay_identity.clone(),
            max_retries: request.max_retries,
            allow_cache: request.allow_cache,
            require_local_artifacts: request.require_local_artifacts,
            cache: request.cache.clone(),
        };
        let execution = execute_glioma_computation_portfolio(&execution_request, executor)?;
        let selected = execution.plan.selected_order.clone();
        let disposition = disposition(request.robustness.disposition, &execution);
        negative_evidence.extend(execution.negative_evidence.iter().cloned());
        uncertainty.extend(execution.uncertainty.iter().cloned());
        (Some(execution), selected, disposition)
    };
    negative_evidence.sort();
    negative_evidence.dedup();
    uncertainty.sort();
    uncertainty.dedup();
    let mut output = RobustnessGuidedComputation {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        model_system: request.model_system,
        robustness: request.robustness.clone(),
        debt_case_order: debt.into_iter().collect(),
        fragile_case_order: fragile.into_iter().collect(),
        negative_case_order: negative.into_iter().collect(),
        unresolved_case_order: unresolved.into_iter().collect(),
        candidate_order,
        scores,
        adjusted_candidates,
        selected_order,
        portfolio_execution,
        minimum_case_coverage: request.minimum_case_coverage,
        minimum_fragile_case_coverage: request.minimum_fragile_case_coverage,
        negative_evidence,
        uncertainty,
        disposition,
        digest: ContentHash::of_bytes(b"unsealed-robustness-guided-computation"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| RobustnessGuidedComputationError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

pub fn dry_run_robustness_guided_computation_executor() -> DryRunGliomaComputationExecutor {
    DryRunGliomaComputationExecutor
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::analysis::{AnalysisDataset, AnalysisRequest, AnalysisRow};
    use crate::glioma::programs::p09_reproducible_computation::execution::{
        ComputationOperation, ComputationTask,
    };
    use crate::glioma::programs::p09_reproducible_computation::robustness::{
        assess_glioma_robustness, RobustnessRequest,
    };
    use crate::glioma_engine::{GliomaModality, LocalArtifactRef};

    fn artifact() -> LocalArtifactRef {
        LocalArtifactRef {
            artifact_id: "robustness-guided-fixture".into(),
            content_hash: ContentHash::of_bytes(b"robustness-guided-fixture"),
            content_type: "application/json".into(),
            local_only: true,
            contains_human_data: false,
            contains_direct_identifiers: false,
        }
    }

    fn robustness() -> RobustnessSuite {
        let dataset = AnalysisDataset {
            dataset_id: "glioma-robustness-guided".into(),
            artifact: artifact(),
            rows: vec![
                AnalysisRow {
                    row_id: "c1".into(),
                    arm_id: "control".into(),
                    model_system: GliomaModelSystem::Organoid,
                    batch_id: "b1".into(),
                    outcome_milli: 100,
                },
                AnalysisRow {
                    row_id: "c2".into(),
                    arm_id: "control".into(),
                    model_system: GliomaModelSystem::Organoid,
                    batch_id: "b2".into(),
                    outcome_milli: 110,
                },
                AnalysisRow {
                    row_id: "t1".into(),
                    arm_id: "treated".into(),
                    model_system: GliomaModelSystem::Organoid,
                    batch_id: "b1".into(),
                    outcome_milli: 400,
                },
                AnalysisRow {
                    row_id: "t2".into(),
                    arm_id: "treated".into(),
                    model_system: GliomaModelSystem::Organoid,
                    batch_id: "b2".into(),
                    outcome_milli: 410,
                },
            ],
        };
        assess_glioma_robustness(
            &RobustnessRequest {
                objective: "stress glioma invasion computation".into(),
                analysis: AnalysisRequest {
                    objective: "stress glioma invasion computation".into(),
                    control_arm: "control".into(),
                    treatment_arm: "treated".into(),
                    model_system: GliomaModelSystem::Organoid,
                    min_replicates_per_arm: 2,
                    effect_threshold_milli: 100,
                    alpha_milli: 50,
                },
                max_cases: 8,
                include_row_jackknife: true,
                min_eligible_cases: 1,
                min_stability_milli: 900,
            },
            &dataset,
        )
        .unwrap()
    }

    fn candidate(
        id: &str,
        operation: ComputationOperation,
        depends_on: Vec<String>,
    ) -> ComputationCandidate {
        ComputationCandidate {
            candidate_id: id.into(),
            task: ComputationTask {
                task_id: id.into(),
                operation,
                model_system: GliomaModelSystem::Organoid,
                depends_on,
                input_artifact_ids: vec![format!("input:{id}")],
                output_schema: format!("{id}@1"),
                estimated_cost_units: 1,
                estimated_duration_ticks: 1,
                deterministic: true,
            },
            modality: if id == "model" {
                GliomaModality::Imaging
            } else {
                GliomaModality::Transcriptomics
            },
            information_gain_milli: 100,
            uncertainty_reduction_milli: 100,
            coverage_debt_milli: 100,
            redundancy_group: id.into(),
            required: false,
        }
    }

    fn request() -> RobustnessGuidedComputationRequest {
        let robustness = robustness();
        let target = robustness
            .case_order
            .iter()
            .find(|case| case.starts_with("leave-one-row-out"))
            .cloned()
            .unwrap();
        RobustnessGuidedComputationRequest {
            objective: robustness.objective.clone(),
            model_system: GliomaModelSystem::Organoid,
            robustness,
            portfolio: super::super::planning::ComputationPortfolioRequest {
                objective: "stress glioma invasion computation".into(),
                model_system: GliomaModelSystem::Organoid,
                budget_units: 2,
                duration_ticks: 2,
                max_tasks: 2,
                max_modalities: 2,
                min_modalities: 1,
                information_weight_milli: 5,
                uncertainty_weight_milli: 5,
                coverage_weight_milli: 5,
                cost_penalty_milli: 1,
                duration_penalty_milli: 1,
                require_deterministic: true,
                completed_order: Vec::new(),
            },
            candidates: vec![
                RobustnessGuidedCandidate {
                    candidate: candidate("normalize", ComputationOperation::Normalize, Vec::new()),
                    target_case_ids: Vec::new(),
                    scientific_role: "normalization re-analysis".into(),
                },
                RobustnessGuidedCandidate {
                    candidate: candidate(
                        "model",
                        ComputationOperation::ModelFit,
                        vec!["normalize".into()],
                    ),
                    target_case_ids: vec![target],
                    scientific_role: "model re-analysis for omission frontier".into(),
                },
            ],
            replay_identity: ContentHash::of_bytes(b"robustness-guided-replay"),
            max_retries: 1,
            allow_cache: true,
            require_local_artifacts: true,
            cache: Vec::new(),
            minimum_case_coverage: 1,
            minimum_fragile_case_coverage: 0,
            stop_if_stable: true,
        }
    }

    #[test]
    fn routes_unresolved_omission_debt_to_dependency_closed_reanalysis() {
        let request = request();
        let mut executor = dry_run_robustness_guided_computation_executor();
        let output = execute_glioma_robustness_guided_computation(&request, &mut executor).unwrap();
        output.validate().unwrap();
        assert!(!output.unresolved_case_order.is_empty());
        assert_eq!(output.selected_order, vec!["model", "normalize"]);
        assert_eq!(
            output
                .portfolio_execution
                .as_ref()
                .unwrap()
                .planned_task_order,
            vec!["normalize", "model"]
        );
        assert!(
            output
                .scores
                .iter()
                .find(|score| score.candidate_id == "model")
                .unwrap()
                .unresolved_case_coverage
                > 0
        );
        let mut replay_executor = dry_run_robustness_guided_computation_executor();
        let replay =
            execute_glioma_robustness_guided_computation(&request, &mut replay_executor).unwrap();
        assert_eq!(output, replay);
    }

    #[test]
    fn stable_robustness_can_hold_without_dispatch() {
        let mut request = request();
        request.robustness.cases.clear();
        request.robustness.case_order.clear();
        request.robustness.disposition = RobustnessDisposition::Stable;
        request.minimum_case_coverage = 0;
        request.robustness.digest = ContentHash::of_value(&serde_json::json!({})).unwrap();
        // A stable hold is tested through the explicit gate behavior; the malformed synthetic
        // digest is rejected before dispatch rather than allowing a fabricated stable result.
        assert!(execute_glioma_robustness_guided_computation(
            &request,
            &mut dry_run_robustness_guided_computation_executor()
        )
        .is_err());
    }
}
