//! Reproducible research-object release operating cycle for preclinical glioma work.
//!
//! This feature is the handoff between autonomous analysis and a governed research commons. It
//! builds the release manifest, runs a dependency-aware replay campaign, evaluates the accountable
//! release predicates, and returns the next operator action. It never signs, uploads, or moves raw
//! data; institution-owned executors provide the only path to real replay computation.

use super::release_gate::{
    evaluate_glioma_release_gate, ReleaseGateError, ReleaseGateEvaluation, ReleaseGateRequest,
    ReleaseGateStatus,
};
use super::replay::{
    execute_glioma_replay_campaign, DryRunReplayCampaignExecutor, ReplayCampaign,
    ReplayCampaignError, ReplayCampaignExecutor, ReplayCampaignRequest,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P11-F24";
pub const OUTPUT_SCHEMA: &str = "GliomaResearchObjectReleaseOperatingCycle1@1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseExecutionMode {
    LocalSimulation,
    GovernedLocal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GliomaReleaseOperatingCycleRequest {
    pub replay: ReplayCampaignRequest,
    pub gate: ReleaseGateRequest,
    pub execution_mode: ReleaseExecutionMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GliomaReleaseOperatingCycleDisposition {
    Publishable,
    Hold,
    Unresolved,
    Blocked,
    NonReproducible,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GliomaReleaseOperatingCycle {
    pub feature_id: String,
    pub output_schema: String,
    pub research_id: String,
    pub study_id: String,
    pub phase_order: Vec<String>,
    pub campaign: ReplayCampaign,
    pub evaluation: ReleaseGateEvaluation,
    pub simulation_only: bool,
    pub execution_mode: ReleaseExecutionMode,
    pub next_operator_action: String,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: GliomaReleaseOperatingCycleDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum GliomaReleaseOperatingCycleError {
    #[error("release operating-cycle replay failed: {0}")]
    Replay(#[from] ReplayCampaignError),
    #[error("release operating-cycle gate failed: {0}")]
    Gate(#[from] ReleaseGateError),
    #[error("release operating-cycle output is invalid: {0}")]
    InvalidOutput(String),
    #[error("release operating-cycle digest failed: {0}")]
    Digest(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(output: &GliomaReleaseOperatingCycle) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "research_id": output.research_id,
        "study_id": output.study_id,
        "phase_order": output.phase_order,
        "campaign": output.campaign,
        "evaluation": output.evaluation,
        "simulation_only": output.simulation_only,
        "execution_mode": output.execution_mode,
        "next_operator_action": output.next_operator_action,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
    })
}

fn disposition(
    campaign: &ReplayCampaign,
    evaluation: &ReleaseGateEvaluation,
) -> GliomaReleaseOperatingCycleDisposition {
    if campaign.disposition == super::replay::ReplayCampaignDisposition::NonReproducible {
        GliomaReleaseOperatingCycleDisposition::NonReproducible
    } else {
        match evaluation.status {
            ReleaseGateStatus::Publishable => GliomaReleaseOperatingCycleDisposition::Publishable,
            ReleaseGateStatus::Hold => GliomaReleaseOperatingCycleDisposition::Hold,
            ReleaseGateStatus::Unresolved => GliomaReleaseOperatingCycleDisposition::Unresolved,
            ReleaseGateStatus::Blocked => GliomaReleaseOperatingCycleDisposition::Blocked,
        }
    }
}

impl GliomaReleaseOperatingCycle {
    pub fn validate(&self) -> Result<(), GliomaReleaseOperatingCycleError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.research_id.trim().is_empty()
            || self.study_id.trim().is_empty()
            || self.phase_order
                != [
                    "manifest_replay".to_string(),
                    "release_gate".to_string(),
                    "operator_handoff".to_string(),
                ]
            || !canonical(&self.negative_evidence)
            || !canonical(&self.uncertainty)
            || self.next_operator_action.trim().is_empty()
            || self.campaign.research_id != self.research_id
            || self.campaign.manifest.study_id != self.study_id
            || self.evaluation.research_id != self.research_id
            || self.evaluation.study_id != self.study_id
            || self.simulation_only
                != matches!(self.execution_mode, ReleaseExecutionMode::LocalSimulation)
        {
            return Err(GliomaReleaseOperatingCycleError::InvalidOutput(
                "identity, phases, campaign/gate binding, evidence ordering, or execution mode is invalid".into(),
            ));
        }
        self.campaign
            .validate()
            .map_err(|error| GliomaReleaseOperatingCycleError::InvalidOutput(error.to_string()))?;
        self.evaluation
            .validate()
            .map_err(|error| GliomaReleaseOperatingCycleError::InvalidOutput(error.to_string()))?;
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| GliomaReleaseOperatingCycleError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(GliomaReleaseOperatingCycleError::InvalidOutput(
                "release operating-cycle digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

/// Run replay and accountable-release gating through institution-owned replay infrastructure.
pub fn execute_glioma_release_operating_cycle<E: ReplayCampaignExecutor>(
    request: &GliomaReleaseOperatingCycleRequest,
    executor: &mut E,
) -> Result<GliomaReleaseOperatingCycle, GliomaReleaseOperatingCycleError> {
    let campaign = execute_glioma_replay_campaign(&request.replay, executor)?;
    let evaluation = evaluate_glioma_release_gate(&request.gate, &campaign)?;
    let disposition = disposition(&campaign, &evaluation);
    let mut negative_evidence = campaign.negative_evidence.clone();
    negative_evidence.extend(
        evaluation
            .warning_order
            .iter()
            .filter(|item| item.contains("negative") || item.contains("mismatch"))
            .cloned(),
    );
    negative_evidence.sort();
    negative_evidence.dedup();
    let mut uncertainty = campaign.uncertainty.clone();
    uncertainty.extend(evaluation.warning_order.iter().cloned());
    uncertainty.extend(evaluation.blocking_order.iter().cloned());
    uncertainty.sort();
    uncertainty.dedup();
    let next_operator_action = match disposition {
        GliomaReleaseOperatingCycleDisposition::Publishable => {
            "obtain the accountable signature, then publish the immutable research object and preserve the replay bundle".into()
        }
        GliomaReleaseOperatingCycleDisposition::Hold => {
            "resolve release warnings and complete independent review before publication".into()
        }
        GliomaReleaseOperatingCycleDisposition::Unresolved => {
            "replay missing or unavailable tasks and keep the release candidate unpublished".into()
        }
        GliomaReleaseOperatingCycleDisposition::Blocked => {
            "resolve manifest, coverage, exact-hash, or approval blockers before any release action".into()
        }
        GliomaReleaseOperatingCycleDisposition::NonReproducible => {
            "investigate the replay divergence, preserve the negative result, and do not publish a reproducibility claim".into()
        }
    };
    let mut output = GliomaReleaseOperatingCycle {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        research_id: campaign.research_id.clone(),
        study_id: campaign.manifest.study_id.clone(),
        phase_order: vec![
            "manifest_replay".into(),
            "release_gate".into(),
            "operator_handoff".into(),
        ],
        campaign,
        evaluation,
        simulation_only: matches!(
            request.execution_mode,
            ReleaseExecutionMode::LocalSimulation
        ),
        execution_mode: request.execution_mode,
        next_operator_action,
        negative_evidence,
        uncertainty,
        disposition,
        digest: ContentHash::of_bytes(b"unsealed-glioma-release-operating-cycle"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| GliomaReleaseOperatingCycleError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

/// Execute a deterministic local replay suitable for planning and integration tests.
pub fn execute_glioma_release_operating_cycle_dry_run(
    request: &GliomaReleaseOperatingCycleRequest,
) -> Result<GliomaReleaseOperatingCycle, GliomaReleaseOperatingCycleError> {
    let mut executor = DryRunReplayCampaignExecutor;
    execute_glioma_release_operating_cycle(request, &mut executor)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::programs::p11_research_object_release::replay::ReplayTask;
    use crate::glioma::release::ResearchObjectRequest;
    use crate::glioma_engine::LocalArtifactRef;

    fn hash(label: &str) -> ContentHash {
        ContentHash::of_value(&serde_json::json!({"label": label})).unwrap()
    }

    fn request() -> GliomaReleaseOperatingCycleRequest {
        let artifact_hash = hash("artifact");
        let task_hash = hash("task");
        GliomaReleaseOperatingCycleRequest {
            replay: ReplayCampaignRequest {
                release: ResearchObjectRequest {
                    research_id: "operating-release-research".into(),
                    study_id: "operating-release-study".into(),
                    objective: "release a reproducible preclinical glioma computation".into(),
                    plan_digest: hash("plan"),
                    execution_digest: hash("execution"),
                    replay_identity: hash("replay"),
                    program_order: vec!["p09-computation".into()],
                    artifacts: vec![LocalArtifactRef {
                        artifact_id: "artifact-main".into(),
                        content_hash: artifact_hash,
                        content_type: "application/json".into(),
                        local_only: true,
                        contains_human_data: false,
                        contains_direct_identifiers: false,
                    }],
                    negative_evidence: vec!["null-results-remain-visible".into()],
                    limitations: vec!["synthetic-replay-fixture".into()],
                    raw_data_local: true,
                    aggregate_only: true,
                },
                tasks: vec![ReplayTask {
                    task_id: "replay-task".into(),
                    program_id: "p09-computation".into(),
                    artifact_id: "artifact-main".into(),
                    expected_content_hash: task_hash,
                    cost_units: 1,
                    required: true,
                    deterministic: true,
                    depends_on: Vec::new(),
                }],
                budget_units: 4,
                max_rounds: 2,
                max_retries: 1,
                min_coverage_milli: 1_000,
                require_exact_hash: true,
            },
            gate: ReleaseGateRequest {
                required_coverage_milli: 1_000,
                require_exact_hash: true,
                require_reproducible: true,
                require_accountable_review: true,
                min_independent_approvals: 1,
                max_uncertainty_items: 16,
                reviews: vec![super::super::release_gate::ReleaseReviewAttestation {
                    reviewer_id: "reviewer-1".into(),
                    role: "independent-researcher".into(),
                    decision: super::super::release_gate::ReleaseReviewDecision::Approve,
                    evidence_digest: hash("review"),
                    independent: true,
                }],
            },
            execution_mode: ReleaseExecutionMode::LocalSimulation,
        }
    }

    #[test]
    fn operating_cycle_replays_gates_and_recommends_release_handoff() {
        let first = execute_glioma_release_operating_cycle_dry_run(&request()).unwrap();
        let second = execute_glioma_release_operating_cycle_dry_run(&request()).unwrap();
        assert_eq!(first, second);
        assert_eq!(
            first.disposition,
            GliomaReleaseOperatingCycleDisposition::Publishable
        );
        assert!(first.next_operator_action.contains("accountable signature"));
        first.validate().unwrap();
    }
}
