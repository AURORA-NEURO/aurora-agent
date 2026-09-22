//! Adaptive evolutionary clone campaign for preclinical glioma research.
//!
//! This controller closes the scientific loop between P05 clonal-evolution inference, P06
//! branch-covering perturbation design, P10 replicate adjudication, and P07 continuation
//! planning. It executes only through a caller-owned local executor, accumulates returned
//! replicate observations, and keeps supported, null, contradictory, negative, and unresolved
//! branches distinct. A dry-run worker is provided for replay and MCP integration; its synthetic
//! observations are never biological evidence.

use super::clone_continuation::{
    plan_glioma_clone_continuation, CloneContinuationCandidate, CloneContinuationError,
    CloneContinuationPlan, CloneContinuationRequest,
};
use crate::glioma::programs::p05_mechanism_exploration::{
    analyze_glioma_clonal_evolution, ClonalEvolutionError, ClonalEvolutionGraph,
    ClonalEvolutionRequest, CloneProfile,
};
use crate::glioma::programs::p06_experiment_design::{
    plan_glioma_clone_perturbation_panel, ClonePerturbationCandidate, ClonePerturbationPanel,
    ClonePerturbationPanelError, ClonePerturbationPanelRequest,
};
use crate::glioma::programs::p10_interpretation_replication::{
    analyze_glioma_clone_panel_outcomes, ClonePanelMeasurementState, ClonePanelObservation,
    ClonePanelOutcomeAnalysis, ClonePanelOutcomeDisposition, ClonePanelOutcomeError,
    ClonePanelOutcomeRequest,
};
use crate::glioma_engine::{GliomaModelSystem, LocalArtifactRef};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P07-F18";
pub const OUTPUT_SCHEMA: &str = "GliomaAdaptiveCloneCampaign1@1";
pub const MAX_ROUNDS: u16 = 32;
pub const MAX_RETRIES: u8 = 8;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AdaptiveCloneCampaignRequest {
    pub evolution: ClonalEvolutionRequest,
    pub profiles: Vec<CloneProfile>,
    pub panel: ClonePerturbationPanelRequest,
    pub perturbations: Vec<ClonePerturbationCandidate>,
    pub outcome: ClonePanelOutcomeRequest,
    pub continuation: CloneContinuationRequest,
    pub continuation_candidates: Vec<CloneContinuationCandidate>,
    pub max_rounds: u16,
    pub max_retries: u8,
    pub stop_on_qualified: bool,
    pub stop_on_negative: bool,
    pub require_artifacts: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdaptiveCloneExecutionFailure {
    pub reason: String,
    pub retryable: bool,
}

/// Institution-local assay, imaging, or analysis workers implement this seam. The campaign
/// supplies one validated panel and round identity; the worker owns all material and data effects.
pub trait AdaptiveCloneCampaignExecutor {
    fn execute_panel(
        &mut self,
        panel: &ClonePerturbationPanel,
        round: u16,
        attempt: u8,
    ) -> Result<Vec<ClonePanelObservation>, AdaptiveCloneExecutionFailure>;
}

/// Deterministic sandbox worker. It emits one positive synthetic observation for every selected
/// candidate/branch cell, which is enough to exercise the full controller without claiming biology.
#[derive(Debug, Default)]
pub struct DryRunAdaptiveCloneCampaignExecutor;

impl AdaptiveCloneCampaignExecutor for DryRunAdaptiveCloneCampaignExecutor {
    fn execute_panel(
        &mut self,
        panel: &ClonePerturbationPanel,
        round: u16,
        attempt: u8,
    ) -> Result<Vec<ClonePanelObservation>, AdaptiveCloneExecutionFailure> {
        let mut observations = Vec::new();
        for branch in &panel.branch_coverage {
            for candidate_id in &branch.selected_candidate_order {
                let observation_id = format!(
                    "dry-run-clone:{round}:{candidate_id}:{}:attempt-{attempt}",
                    branch.branch_id
                );
                let content_hash = ContentHash::of_value(&serde_json::json!({
                    "observation_id": observation_id,
                    "candidate_id": candidate_id,
                    "branch_id": branch.branch_id,
                    "round": round,
                    "attempt": attempt,
                    "simulation_only": true,
                }))
                .map_err(|error| AdaptiveCloneExecutionFailure {
                    reason: format!("dry-run clone observation digest failed: {error}"),
                    retryable: false,
                })?;
                observations.push(ClonePanelObservation {
                    observation_id,
                    study_id: panel.study_id.clone(),
                    model_system: panel.model_system,
                    candidate_id: candidate_id.clone(),
                    branch_id: branch.branch_id.clone(),
                    replicate_id: format!("round-{round}"),
                    state: ClonePanelMeasurementState::Measured,
                    effect_milli: 850,
                    uncertainty_milli: 100,
                    artifact: LocalArtifactRef {
                        artifact_id: format!(
                            "dry-run-clone:{}:{}:{}",
                            round, candidate_id, branch.branch_id
                        ),
                        content_hash,
                        content_type: "application/vnd.aurora.glioma.clone-observation+json".into(),
                        local_only: true,
                        contains_human_data: false,
                        contains_direct_identifiers: false,
                    },
                });
            }
        }
        Ok(observations)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdaptiveCloneCampaignRound {
    pub round: u16,
    pub panel_digest: ContentHash,
    pub observation_order: Vec<String>,
    pub failed_order: Vec<String>,
    pub retry_count: u32,
    pub outcome: ClonePanelOutcomeAnalysis,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdaptiveCloneCampaignDisposition {
    Qualified,
    Negative,
    Partial,
    Unresolved,
    BudgetBlocked,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdaptiveCloneCampaignStopReason {
    Qualified,
    Negative,
    MaxRounds,
    ExecutorFailed,
    BudgetExhausted,
    NoProgress,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdaptiveCloneCampaign {
    pub feature_id: String,
    pub output_schema: String,
    pub study_id: String,
    pub model_system: GliomaModelSystem,
    pub graph: ClonalEvolutionGraph,
    pub panel: ClonePerturbationPanel,
    pub rounds: Vec<AdaptiveCloneCampaignRound>,
    pub observations: Vec<ClonePanelObservation>,
    pub final_outcome: ClonePanelOutcomeAnalysis,
    pub continuation: Option<CloneContinuationPlan>,
    pub simulation_only: bool,
    pub retry_count: u32,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: AdaptiveCloneCampaignDisposition,
    pub stop_reason: AdaptiveCloneCampaignStopReason,
    pub next_operator_action: String,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AdaptiveCloneCampaignError {
    #[error("adaptive clone campaign request is invalid: {0}")]
    InvalidRequest(String),
    #[error("adaptive clone campaign evolution failed: {0}")]
    Evolution(#[from] ClonalEvolutionError),
    #[error("adaptive clone campaign panel planning failed: {0}")]
    Panel(#[from] ClonePerturbationPanelError),
    #[error("adaptive clone campaign outcome analysis failed: {0}")]
    Outcome(#[from] ClonePanelOutcomeError),
    #[error("adaptive clone campaign continuation planning failed: {0}")]
    Continuation(#[from] CloneContinuationError),
    #[error("adaptive clone campaign executor failed: {0}")]
    Execution(String),
    #[error("adaptive clone campaign output is invalid: {0}")]
    InvalidOutput(String),
    #[error("adaptive clone campaign digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(output: &AdaptiveCloneCampaign) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "study_id": output.study_id,
        "model_system": output.model_system,
        "graph": output.graph,
        "panel": output.panel,
        "rounds": output.rounds,
        "observations": output.observations,
        "final_outcome": output.final_outcome,
        "continuation": output.continuation,
        "simulation_only": output.simulation_only,
        "retry_count": output.retry_count,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
        "stop_reason": output.stop_reason,
        "next_operator_action": output.next_operator_action,
    })
}

fn validate_request(
    request: &AdaptiveCloneCampaignRequest,
) -> Result<(), AdaptiveCloneCampaignError> {
    if request.profiles.is_empty()
        || request.perturbations.is_empty()
        || request.max_rounds == 0
        || request.max_rounds > MAX_ROUNDS
        || request.max_retries > MAX_RETRIES
        || request.panel.study_id != request.evolution.study_id
        || request.outcome.study_id != request.evolution.study_id
        || request.continuation.study_id != request.evolution.study_id
        || request.panel.model_system != request.evolution.model_system
        || request.outcome.model_system != request.evolution.model_system
        || request.continuation.model_system != request.evolution.model_system
    {
        return Err(AdaptiveCloneCampaignError::InvalidRequest(
            "bounded rounds/retries, non-empty profiles and perturbations, and consistent study/model bindings are required".into(),
        ));
    }
    if request.continuation_candidates.is_empty() {
        return Err(AdaptiveCloneCampaignError::InvalidRequest(
            "at least one continuation candidate is required for unresolved branch routing".into(),
        ));
    }
    Ok(())
}

fn classify_disposition(
    outcome: &ClonePanelOutcomeAnalysis,
    stop_reason: AdaptiveCloneCampaignStopReason,
) -> AdaptiveCloneCampaignDisposition {
    match outcome.disposition {
        ClonePanelOutcomeDisposition::Qualified => AdaptiveCloneCampaignDisposition::Qualified,
        ClonePanelOutcomeDisposition::Negative => AdaptiveCloneCampaignDisposition::Negative,
        ClonePanelOutcomeDisposition::Partial => AdaptiveCloneCampaignDisposition::Partial,
        ClonePanelOutcomeDisposition::Unresolved => match stop_reason {
            AdaptiveCloneCampaignStopReason::BudgetExhausted => {
                AdaptiveCloneCampaignDisposition::BudgetBlocked
            }
            AdaptiveCloneCampaignStopReason::ExecutorFailed => {
                AdaptiveCloneCampaignDisposition::Failed
            }
            _ => AdaptiveCloneCampaignDisposition::Unresolved,
        },
    }
}

impl AdaptiveCloneCampaign {
    pub fn validate(&self) -> Result<(), AdaptiveCloneCampaignError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.study_id.trim().is_empty()
            || self.rounds.len() > MAX_ROUNDS as usize
            || !canonical(
                &self
                    .observations
                    .iter()
                    .map(|item| item.observation_id.clone())
                    .collect::<Vec<_>>(),
            )
            || !canonical(&self.negative_evidence)
            || !canonical(&self.uncertainty)
            || self.retry_count
                != self
                    .rounds
                    .iter()
                    .map(|round| round.retry_count)
                    .sum::<u32>()
            || self.graph.study_id != self.study_id
            || self.graph.model_system != self.model_system
            || self.panel.study_id != self.study_id
            || self.panel.model_system != self.model_system
            || self.final_outcome.study_id != self.study_id
            || self.final_outcome.model_system != self.model_system
            || self.next_operator_action.trim().is_empty()
        {
            return Err(AdaptiveCloneCampaignError::InvalidOutput(
                "identity, ordering, binding, retry, or operator-action invariant failed".into(),
            ));
        }
        self.graph
            .validate()
            .map_err(|error| AdaptiveCloneCampaignError::InvalidOutput(error.to_string()))?;
        self.panel
            .validate()
            .map_err(|error| AdaptiveCloneCampaignError::InvalidOutput(error.to_string()))?;
        self.final_outcome
            .validate()
            .map_err(|error| AdaptiveCloneCampaignError::InvalidOutput(error.to_string()))?;
        for round in &self.rounds {
            if round.round == 0
                || round.panel_digest != self.panel.digest
                || !canonical(&round.observation_order)
                || !canonical(&round.failed_order)
            {
                return Err(AdaptiveCloneCampaignError::InvalidOutput(
                    "round identity, panel binding, or observation ordering is invalid".into(),
                ));
            }
            round
                .outcome
                .validate()
                .map_err(|error| AdaptiveCloneCampaignError::InvalidOutput(error.to_string()))?;
        }
        if let Some(continuation) = &self.continuation {
            continuation
                .validate()
                .map_err(|error| AdaptiveCloneCampaignError::InvalidOutput(error.to_string()))?;
            if continuation.source_outcome_digest != self.final_outcome.digest {
                return Err(AdaptiveCloneCampaignError::InvalidOutput(
                    "continuation plan is not bound to the final outcome".into(),
                ));
            }
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| AdaptiveCloneCampaignError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(AdaptiveCloneCampaignError::InvalidOutput(
                "adaptive clone campaign digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

/// Execute the bounded evolution-to-perturbation-to-outcome-to-continuation loop.
pub fn execute_glioma_adaptive_clone_campaign<E: AdaptiveCloneCampaignExecutor>(
    request: &AdaptiveCloneCampaignRequest,
    executor: &mut E,
) -> Result<AdaptiveCloneCampaign, AdaptiveCloneCampaignError> {
    validate_request(request)?;
    let graph = analyze_glioma_clonal_evolution(&request.evolution, &request.profiles)?;
    let panel =
        plan_glioma_clone_perturbation_panel(&request.panel, &graph, &request.perturbations)?;
    let mut observations = Vec::new();
    let mut observation_ids = BTreeSet::new();
    let mut rounds = Vec::new();
    let mut total_retries = 0_u32;
    let mut stop_reason = AdaptiveCloneCampaignStopReason::MaxRounds;
    let mut final_outcome =
        analyze_glioma_clone_panel_outcomes(&request.outcome, &panel, &observations)?;
    for round in 1..=request.max_rounds {
        let mut batch = None;
        let mut failed_order = Vec::new();
        let mut retries = 0_u32;
        for attempt in 0..=request.max_retries {
            match executor.execute_panel(&panel, round, attempt) {
                Ok(value) => {
                    batch = Some(value);
                    break;
                }
                Err(error) => {
                    failed_order.push(format!("round-{round}:attempt-{attempt}:{}", error.reason));
                    if !error.retryable || attempt == request.max_retries {
                        break;
                    }
                    retries = retries.saturating_add(1);
                }
            }
        }
        total_retries = total_retries.saturating_add(retries);
        let Some(batch) = batch else {
            stop_reason = AdaptiveCloneCampaignStopReason::ExecutorFailed;
            break;
        };
        if batch.is_empty() {
            stop_reason = AdaptiveCloneCampaignStopReason::NoProgress;
            break;
        }
        for observation in &batch {
            if !observation_ids.insert(observation.observation_id.clone()) {
                return Err(AdaptiveCloneCampaignError::Execution(
                    "executor returned a duplicate observation identity".into(),
                ));
            }
        }
        observations.extend(batch);
        observations.sort_by(|left, right| left.observation_id.cmp(&right.observation_id));
        final_outcome =
            analyze_glioma_clone_panel_outcomes(&request.outcome, &panel, &observations)?;
        let observation_order = observations
            .iter()
            .filter(|observation| observation_ids.contains(&observation.observation_id))
            .map(|observation| observation.observation_id.clone())
            .collect::<Vec<_>>();
        rounds.push(AdaptiveCloneCampaignRound {
            round,
            panel_digest: panel.digest.clone(),
            observation_order,
            failed_order,
            retry_count: retries,
            outcome: final_outcome.clone(),
        });
        if request.stop_on_qualified
            && final_outcome.disposition == ClonePanelOutcomeDisposition::Qualified
        {
            stop_reason = AdaptiveCloneCampaignStopReason::Qualified;
            break;
        }
        if request.stop_on_negative
            && final_outcome.disposition == ClonePanelOutcomeDisposition::Negative
        {
            stop_reason = AdaptiveCloneCampaignStopReason::Negative;
            break;
        }
        if round == request.max_rounds {
            stop_reason = AdaptiveCloneCampaignStopReason::MaxRounds;
        }
    }
    let continuation = if final_outcome.next_action_order.is_empty() {
        None
    } else {
        Some(plan_glioma_clone_continuation(
            &request.continuation,
            &final_outcome,
            &request.continuation_candidates,
        )?)
    };
    let disposition = classify_disposition(&final_outcome, stop_reason);
    let mut negative_evidence = final_outcome.negative_evidence.clone();
    let mut uncertainty = final_outcome.uncertainty.clone();
    if let Some(plan) = &continuation {
        negative_evidence.extend(plan.negative_evidence.iter().cloned());
        uncertainty.extend(plan.uncertainty.iter().cloned());
    }
    negative_evidence.sort();
    negative_evidence.dedup();
    uncertainty.sort();
    uncertainty.dedup();
    let next_operator_action = match disposition {
        AdaptiveCloneCampaignDisposition::Qualified => {
            "preserve the supported branch effects, review controls and replicate provenance, then decide whether to release or extend the mechanism program".into()
        }
        AdaptiveCloneCampaignDisposition::Negative => {
            "retain the negative or null branch result and revise the mechanism model before another perturbation".into()
        }
        AdaptiveCloneCampaignDisposition::Partial
        | AdaptiveCloneCampaignDisposition::Unresolved => {
            "execute only the dependency-closed continuation actions for unresolved or contradictory branches; do not promote missing cells".into()
        }
        AdaptiveCloneCampaignDisposition::BudgetBlocked => {
            "increase or reallocate the bounded clone campaign budget before dispatching another panel round".into()
        }
        AdaptiveCloneCampaignDisposition::Failed => {
            "inspect the executor failure, preserve returned observations, and retry only after the local assay boundary is repaired".into()
        }
    };
    let mut output = AdaptiveCloneCampaign {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        study_id: request.evolution.study_id.clone(),
        model_system: request.evolution.model_system,
        graph,
        panel,
        rounds,
        observations,
        final_outcome,
        continuation,
        simulation_only: true,
        retry_count: total_retries,
        negative_evidence,
        uncertainty,
        disposition,
        stop_reason,
        next_operator_action,
        digest: ContentHash::of_bytes(b"unsealed-glioma-adaptive-clone-campaign"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| AdaptiveCloneCampaignError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

pub fn execute_glioma_adaptive_clone_campaign_dry_run(
    request: &AdaptiveCloneCampaignRequest,
) -> Result<AdaptiveCloneCampaign, AdaptiveCloneCampaignError> {
    let mut executor = DryRunAdaptiveCloneCampaignExecutor;
    execute_glioma_adaptive_clone_campaign(request, &mut executor)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::programs::p05_mechanism_exploration::{CloneMarker, CloneMarkerState};
    use crate::glioma_engine::GliomaModelSystem;

    fn artifact(id: &str) -> LocalArtifactRef {
        LocalArtifactRef {
            artifact_id: id.into(),
            content_hash: ContentHash::of_bytes(id.as_bytes()),
            content_type: "application/json".into(),
            local_only: true,
            contains_human_data: false,
            contains_direct_identifiers: false,
        }
    }

    fn request() -> AdaptiveCloneCampaignRequest {
        let profile = |id: &str, clone_id: &str, timepoint: u32, markers: &[&str]| CloneProfile {
            profile_id: id.into(),
            study_id: "adaptive-clone-study".into(),
            sample_lineage: "lineage-a".into(),
            clone_id: clone_id.into(),
            timepoint,
            model_system: GliomaModelSystem::Organoid,
            abundance_milli: if timepoint == 0 { 400 } else { 700 },
            artifact: artifact(id),
            markers: markers
                .iter()
                .map(|marker_id| CloneMarker {
                    marker_id: (*marker_id).into(),
                    state: CloneMarkerState::Present,
                    confidence_milli: 900,
                })
                .collect(),
        };
        let perturbation = |candidate_id: &str, marker: &str| ClonePerturbationCandidate {
            candidate_id: candidate_id.into(),
            kind: crate::glioma::programs::p06_experiment_design::ClonePerturbationKind::Inhibit,
            target_marker_order: vec![marker.into()],
            cost_milli: 5,
            expected_effect_milli: 900,
            purpose: format!("test {marker} branch dependency"),
            artifact: artifact(candidate_id),
        };
        AdaptiveCloneCampaignRequest {
            evolution: ClonalEvolutionRequest {
                study_id: "adaptive-clone-study".into(),
                model_system: GliomaModelSystem::Organoid,
                min_shared_markers: 1,
                min_parent_score_milli: 500,
                max_time_gap: 5,
                min_abundance_milli: 1,
                allow_parallel_branches: true,
                max_parent_candidates: 2,
            },
            profiles: vec![
                profile("root", "clone-a", 0, &["egfr", "tp53"]),
                profile("ec", "clone-b", 1, &["egfr", "tp53", "ecDNA"]),
                profile("pt", "clone-c", 1, &["egfr", "tp53", "pten"]),
            ],
            panel: ClonePerturbationPanelRequest {
                study_id: "adaptive-clone-study".into(),
                model_system: GliomaModelSystem::Organoid,
                budget_milli: 10,
                min_coverage_milli: 500,
                max_selected: 2,
                require_branch_coverage: true,
                allow_uncertain_targets: true,
            },
            perturbations: vec![
                perturbation("ec-panel", "ecDNA"),
                perturbation("pt-panel", "pten"),
            ],
            outcome: ClonePanelOutcomeRequest {
                study_id: "adaptive-clone-study".into(),
                model_system: GliomaModelSystem::Organoid,
                min_replicates: 1,
                effect_threshold_milli: 500,
                max_uncertainty_milli: 200,
                require_all_selected: true,
                require_all_branches: true,
            },
            continuation: CloneContinuationRequest {
                study_id: "adaptive-clone-study".into(),
                model_system: GliomaModelSystem::Organoid,
                budget_milli: 10,
                max_selected: 2,
                max_risk_tier: 2,
                require_approval: false,
                allow_instrument_actions: false,
                allow_federation: false,
            },
            continuation_candidates: vec![CloneContinuationCandidate {
                action_id: "measure-unresolved".into(),
                target_id: "measure:ec-panel@branch".into(),
                kind: crate::glioma::programs::p07_protocol_simulation::CloneContinuationActionKind::Measure,
                cost_milli: 2,
                information_gain_milli: 900,
                risk_tier: 1,
                dependencies: Vec::new(),
                requires_instrument: false,
                requires_federation: false,
                description: "measure an unresolved clone branch".into(),
                artifact: artifact("measure-unresolved"),
            }],
            max_rounds: 2,
            max_retries: 1,
            stop_on_qualified: true,
            stop_on_negative: true,
            require_artifacts: true,
        }
    }

    #[test]
    fn dry_run_closes_evolution_panel_outcome_and_stops_on_qualification() {
        let first = execute_glioma_adaptive_clone_campaign_dry_run(&request()).unwrap();
        let second = execute_glioma_adaptive_clone_campaign_dry_run(&request()).unwrap();
        assert_eq!(first, second);
        assert_eq!(
            first.disposition,
            AdaptiveCloneCampaignDisposition::Qualified
        );
        assert_eq!(
            first.stop_reason,
            AdaptiveCloneCampaignStopReason::Qualified
        );
        assert_eq!(first.rounds.len(), 1);
        assert!(!first.observations.is_empty());
        assert_eq!(
            first.final_outcome.disposition,
            ClonePanelOutcomeDisposition::Qualified
        );
        assert!(first.continuation.is_none());
    }

    #[test]
    fn inconsistent_binding_is_rejected_before_evolution() {
        let mut request = request();
        request.panel.study_id = "different-study".into();
        assert!(matches!(
            execute_glioma_adaptive_clone_campaign_dry_run(&request),
            Err(AdaptiveCloneCampaignError::InvalidRequest(_))
        ));
    }
}
