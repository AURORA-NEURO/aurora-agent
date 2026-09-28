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
use crate::glioma::programs::p07_protocol_simulation::{
    GliomaAutonomousResearchEngineDisposition, GliomaAutonomousResearchEngineRun,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
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
    #[error("release operating-cycle request is invalid: {0}")]
    InvalidRequest(String),
    #[error("release operating-cycle replay failed: {0}")]
    Replay(#[from] ReplayCampaignError),
    #[error("release operating-cycle gate failed: {0}")]
    Gate(#[from] ReleaseGateError),
    #[error("release operating-cycle output is invalid: {0}")]
    InvalidOutput(String),
    #[error("release operating-cycle digest failed: {0}")]
    Digest(String),
}

fn validate_engine_binding(
    engine: &GliomaAutonomousResearchEngineRun,
    request: &GliomaReleaseOperatingCycleRequest,
) -> Result<(), GliomaReleaseOperatingCycleError> {
    engine
        .validate()
        .map_err(|error| GliomaReleaseOperatingCycleError::InvalidRequest(error.to_string()))?;
    if engine.disposition != GliomaAutonomousResearchEngineDisposition::Completed
        || !engine.pending_action_order.is_empty()
        || !engine.hold_order.is_empty()
        || !engine.approval_order.is_empty()
        || !engine.blocked_order.is_empty()
    {
        return Err(GliomaReleaseOperatingCycleError::InvalidRequest(
            "only a qualified engine run with no pending, held, approval, or blocked work can enter release".into(),
        ));
    }
    let Some(last_cycle) = engine.cycles.last() else {
        return Err(GliomaReleaseOperatingCycleError::InvalidRequest(
            "a qualified engine run must contain at least one execution cycle".into(),
        ));
    };
    let plan = &last_cycle.director.workflow_plan;
    let release = &request.replay.release;
    if release.research_id != plan.research_id
        || release.study_id != plan.study_id
        || release.objective != engine.objective
        || release.plan_digest != plan.plan_digest
        || release.execution_digest != engine.digest
        || release.replay_identity != plan.replay_identity
    {
        return Err(GliomaReleaseOperatingCycleError::InvalidRequest(
            "release identity, plan, execution digest, objective, or replay identity is not bound to the qualified engine run".into(),
        ));
    }
    let release_artifacts = release
        .artifacts
        .iter()
        .map(|artifact| (artifact.artifact_id.as_str(), artifact))
        .collect::<BTreeMap<_, _>>();
    if engine.completed_checkpoints.iter().any(|checkpoint| {
        release_artifacts
            .get(checkpoint.artifact_id.as_str())
            .is_none_or(|artifact| {
                artifact.content_hash != checkpoint.artifact.content_hash
                    || artifact.content_type != checkpoint.artifact.content_type
            })
    }) {
        return Err(GliomaReleaseOperatingCycleError::InvalidRequest(
            "every qualified engine checkpoint must be represented by the exact local release artifact".into(),
        ));
    }
    if engine
        .negative_evidence
        .iter()
        .any(|item| !release.negative_evidence.contains(item))
    {
        return Err(GliomaReleaseOperatingCycleError::InvalidRequest(
            "release metadata cannot omit negative evidence reported by the engine".into(),
        ));
    }
    Ok(())
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

/// Bind a qualified autonomous P07 engine run to the P11 replay and release gates.
///
/// This is the cross-program handoff used by a production research service: it refuses to turn
/// a budget-exhausted, partial, held, or stale engine run into a publishable object, and it binds
/// the final plan digest, engine execution digest, replay identity, exact checkpoint artifacts,
/// and negative evidence before P11 performs replay or release evaluation.
pub fn execute_glioma_engine_release_operating_cycle<E: ReplayCampaignExecutor>(
    engine: &GliomaAutonomousResearchEngineRun,
    request: &GliomaReleaseOperatingCycleRequest,
    executor: &mut E,
) -> Result<GliomaReleaseOperatingCycle, GliomaReleaseOperatingCycleError> {
    validate_engine_binding(engine, request)?;
    execute_glioma_release_operating_cycle(request, executor)
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
    use crate::glioma::programs::p07_protocol_simulation::{
        execute_glioma_autonomous_research_engine, DryRunGliomaActionExecutor,
        GliomaAdaptiveReplanningPolicy, GliomaAutonomousResearchEngineDisposition,
        GliomaAutonomousResearchEngineRequest, GliomaDirectorFocus,
    };
    use crate::glioma::programs::p11_research_object_release::replay::ReplayTask;
    use crate::glioma::release::ResearchObjectRequest;
    use crate::glioma_engine::{
        GliomaModality, GliomaModelSystem, GliomaResearchIntent, LocalArtifactRef,
    };
    use bioprism_foundation::{AutonomyTier, PRECLINICAL_BOUNDARY};
    use bioprism_onco::OutputUse;
    use std::collections::BTreeSet;

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

    fn engine_request() -> GliomaAutonomousResearchEngineRequest {
        let input_hash = ContentHash::of_bytes(b"engine-release-input");
        GliomaAutonomousResearchEngineRequest {
            mission_id: "release-handoff-mission".into(),
            intent: GliomaResearchIntent {
                research_id: "release-handoff-research".into(),
                study_id: "release-handoff-study".into(),
                objective: "reproduce a qualified preclinical glioma workflow".into(),
                output_uses: BTreeSet::from([OutputUse::CohortAnalysis]),
                model_systems: BTreeSet::from([
                    GliomaModelSystem::CellLine,
                    GliomaModelSystem::Organoid,
                ]),
                modalities: BTreeSet::from([
                    GliomaModality::Literature,
                    GliomaModality::Genomics,
                    GliomaModality::Transcriptomics,
                    GliomaModality::Imaging,
                    GliomaModality::Computational,
                    GliomaModality::Replication,
                ]),
                input_artifacts: vec![LocalArtifactRef {
                    artifact_id: "engine-input".into(),
                    content_hash: input_hash.clone(),
                    content_type: "application/vnd.aurora.local-study+json".into(),
                    local_only: true,
                    contains_human_data: false,
                    contains_direct_identifiers: false,
                }],
                requested_autonomy: AutonomyTier::A1,
                approval_reference: None,
                budget_units: 400,
                max_retries: 1,
                allow_instrument_execution: false,
                allow_federation: false,
                raw_data_local: true,
                aggregate_only: true,
                replay_identity: input_hash,
                boundary: PRECLINICAL_BOUNDARY.into(),
            },
            focus: GliomaDirectorFocus::FullProgram,
            completed_checkpoints: Vec::new(),
            budget_units: 400,
            max_actions: 4,
            max_cycles: 32,
            approval_granted: false,
            allow_instrument_execution: false,
            allow_federation: false,
            selection_weights: Default::default(),
            max_retries: 1,
            require_artifacts: true,
            outcome_summaries: BTreeMap::new(),
            adaptive_policy: GliomaAdaptiveReplanningPolicy::default(),
        }
    }

    fn cycle_bound_to_engine(
        engine: &crate::glioma::programs::p07_protocol_simulation::GliomaAutonomousResearchEngineRun,
    ) -> GliomaReleaseOperatingCycleRequest {
        let plan = &engine.cycles.last().unwrap().director.workflow_plan;
        let artifacts = engine
            .completed_checkpoints
            .iter()
            .map(|checkpoint| checkpoint.artifact.clone())
            .collect::<Vec<_>>();
        let tasks = artifacts
            .iter()
            .enumerate()
            .map(|(index, artifact)| ReplayTask {
                task_id: format!("engine-replay-{index:03}"),
                program_id: "p07-protocol-simulation".into(),
                artifact_id: artifact.artifact_id.clone(),
                expected_content_hash: artifact.content_hash.clone(),
                cost_units: 1,
                required: true,
                deterministic: true,
                depends_on: Vec::new(),
            })
            .collect::<Vec<_>>();
        let mut cycle = request();
        cycle.replay.release = ResearchObjectRequest {
            research_id: plan.research_id.clone(),
            study_id: plan.study_id.clone(),
            objective: engine.objective.clone(),
            plan_digest: plan.plan_digest.clone(),
            execution_digest: engine.digest.clone(),
            replay_identity: plan.replay_identity.clone(),
            program_order: vec!["p07-protocol-simulation".into()],
            artifacts,
            negative_evidence: engine.negative_evidence.clone(),
            limitations: vec!["synthetic replay fixture; not biological evidence".into()],
            raw_data_local: true,
            aggregate_only: true,
        };
        cycle.replay.tasks = tasks;
        cycle.replay.budget_units = cycle.replay.tasks.len() as u64 + 1;
        cycle.replay.max_rounds = 4;
        cycle
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

    #[test]
    fn qualified_engine_run_is_bound_to_exact_release_and_replay_inputs() {
        let mut engine_executor = DryRunGliomaActionExecutor;
        let engine =
            execute_glioma_autonomous_research_engine(&engine_request(), &mut engine_executor)
                .unwrap();
        assert_eq!(
            engine.disposition,
            GliomaAutonomousResearchEngineDisposition::Completed,
            "the fixture must exercise the qualified handoff path"
        );
        let cycle_request = cycle_bound_to_engine(&engine);
        let mut replay_executor = DryRunReplayCampaignExecutor;
        let cycle = execute_glioma_engine_release_operating_cycle(
            &engine,
            &cycle_request,
            &mut replay_executor,
        )
        .unwrap();
        assert_eq!(
            cycle.disposition,
            GliomaReleaseOperatingCycleDisposition::Publishable
        );
        cycle.validate().unwrap();
    }

    #[test]
    fn release_handoff_rejects_a_tampered_engine_digest_before_replay() {
        let mut engine_executor = DryRunGliomaActionExecutor;
        let engine =
            execute_glioma_autonomous_research_engine(&engine_request(), &mut engine_executor)
                .unwrap();
        let mut cycle_request = cycle_bound_to_engine(&engine);
        cycle_request.replay.release.execution_digest = hash("tampered-engine");
        let mut replay_executor = DryRunReplayCampaignExecutor;
        assert!(matches!(
            execute_glioma_engine_release_operating_cycle(
                &engine,
                &cycle_request,
                &mut replay_executor,
            ),
            Err(GliomaReleaseOperatingCycleError::InvalidRequest(_))
        ));
    }
}
