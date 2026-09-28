//! Local, single-study release workflow orchestration for preclinical glioma research objects.
//!
//! GAF-GLIOMA-P11-F13 composes multimodal packaging, dependency closure, replay, and release
//! gating into one fail-closed workflow. Replay runs only after package and dependency closure
//! succeed. A ready result means eligible for accountable signing review; this workflow never
//! signs, uploads, publishes, or moves raw study data.

use super::dependency_closure::{
    DependencyClosureDisposition, DependencyClosureError, DependencyClosurePlan,
    DependencyClosureRequest, MAX_DEPTH, analyze_glioma_research_object_dependency_closure,
};
use super::multimodal_bundle::MultimodalResearchObjectInput;
use super::multimodal_bundle::{
    MultimodalResearchObjectBundle, MultimodalResearchObjectError, MultimodalResearchObjectRequest,
    compile_glioma_multimodal_research_object,
};
use super::release_gate::{
    ReleaseGateError, ReleaseGateEvaluation, ReleaseGateRequest, ReleaseGateStatus,
    evaluate_glioma_release_gate,
};
use super::replay::{
    DryRunReplayCampaignExecutor, ReplayCampaign, ReplayCampaignDisposition, ReplayCampaignError,
    ReplayCampaignExecutor, ReplayCampaignRequest, ReplayTask, execute_glioma_replay_campaign,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P11-F13";
pub const OUTPUT_SCHEMA: &str = "GliomaResearchObjectLocalReleaseWorkflow1@1";
pub const MAX_REQUIRED_PROGRAMS: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GliomaLocalReleaseWorkflowRequest {
    pub package: MultimodalResearchObjectRequest,
    pub max_dependency_depth: usize,
    pub required_programs: BTreeSet<String>,
    pub require_program_coverage: bool,
    pub replay: ReplayCampaignRequest,
    pub gate: ReleaseGateRequest,
}

/// Preflighted local workflow phases, before a replay executor is called.
pub(super) struct PreparedLocalReleaseWorkflow {
    pub(super) bundle: MultimodalResearchObjectBundle,
    pub(super) dependency_closure: DependencyClosurePlan,
    pub(super) replay_request: ReplayCampaignRequest,
    pub(super) gate_request: ReleaseGateRequest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GliomaLocalReleaseWorkflowDisposition {
    ReadyForSigning,
    Hold,
    Unresolved,
    Blocked,
    NonReproducible,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GliomaLocalReleaseWorkflow {
    pub feature_id: String,
    pub output_schema: String,
    pub research_id: String,
    pub study_id: String,
    pub phase_order: Vec<String>,
    pub bundle: MultimodalResearchObjectBundle,
    pub dependency_closure: DependencyClosurePlan,
    pub campaign: Option<ReplayCampaign>,
    pub gate: Option<ReleaseGateEvaluation>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: GliomaLocalReleaseWorkflowDisposition,
    pub next_operator_action: String,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum GliomaLocalReleaseWorkflowError {
    #[error("local release workflow request is invalid: {0}")]
    InvalidRequest(String),
    #[error("local release workflow bundle failed: {0}")]
    Bundle(#[from] MultimodalResearchObjectError),
    #[error("local release workflow dependency closure failed: {0}")]
    Closure(#[from] DependencyClosureError),
    #[error("local release workflow replay failed: {0}")]
    Replay(#[from] ReplayCampaignError),
    #[error("local release workflow gate failed: {0}")]
    Gate(#[from] ReleaseGateError),
    #[error("local release workflow output is invalid: {0}")]
    InvalidOutput(String),
    #[error("local release workflow digest failed: {0}")]
    Digest(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(workflow: &GliomaLocalReleaseWorkflow) -> serde_json::Value {
    serde_json::json!({
        "feature_id": workflow.feature_id,
        "output_schema": workflow.output_schema,
        "research_id": workflow.research_id,
        "study_id": workflow.study_id,
        "phase_order": workflow.phase_order,
        "bundle": workflow.bundle,
        "dependency_closure": workflow.dependency_closure,
        "campaign": workflow.campaign,
        "gate": workflow.gate,
        "negative_evidence": workflow.negative_evidence,
        "uncertainty": workflow.uncertainty,
        "disposition": workflow.disposition,
        "next_operator_action": workflow.next_operator_action,
    })
}

fn disposition(
    closure: &DependencyClosurePlan,
    campaign: Option<&ReplayCampaign>,
    gate: Option<&ReleaseGateEvaluation>,
) -> GliomaLocalReleaseWorkflowDisposition {
    match closure.disposition {
        DependencyClosureDisposition::Blocked => GliomaLocalReleaseWorkflowDisposition::Blocked,
        DependencyClosureDisposition::Partial => GliomaLocalReleaseWorkflowDisposition::Hold,
        DependencyClosureDisposition::Closed => {
            if campaign
                .is_some_and(|item| item.disposition == ReplayCampaignDisposition::NonReproducible)
            {
                return GliomaLocalReleaseWorkflowDisposition::NonReproducible;
            }
            match gate.map(|item| item.status) {
                Some(ReleaseGateStatus::Publishable) => {
                    GliomaLocalReleaseWorkflowDisposition::ReadyForSigning
                }
                Some(ReleaseGateStatus::Hold) => GliomaLocalReleaseWorkflowDisposition::Hold,
                Some(ReleaseGateStatus::Unresolved) => {
                    GliomaLocalReleaseWorkflowDisposition::Unresolved
                }
                Some(ReleaseGateStatus::Blocked) => GliomaLocalReleaseWorkflowDisposition::Blocked,
                None => GliomaLocalReleaseWorkflowDisposition::Blocked,
            }
        }
    }
}

fn next_action(disposition: GliomaLocalReleaseWorkflowDisposition) -> &'static str {
    match disposition {
        GliomaLocalReleaseWorkflowDisposition::ReadyForSigning => {
            "obtain accountable signatures, then use the institution's publication workflow"
        }
        GliomaLocalReleaseWorkflowDisposition::Hold => {
            "resolve package omissions or release warnings and rerun the local workflow"
        }
        GliomaLocalReleaseWorkflowDisposition::Unresolved => {
            "resolve unavailable replay evidence and keep the object out of signing"
        }
        GliomaLocalReleaseWorkflowDisposition::Blocked => {
            "repair package or dependency blockers before replay or signing"
        }
        GliomaLocalReleaseWorkflowDisposition::NonReproducible => {
            "preserve the divergent replay evidence and do not make a reproducibility claim"
        }
    }
}

fn validate_request(
    request: &GliomaLocalReleaseWorkflowRequest,
) -> Result<(), GliomaLocalReleaseWorkflowError> {
    let package_release = &request.package.release;
    let replay_release = &request.replay.release;
    if package_release.research_id != replay_release.research_id
        || package_release.study_id != replay_release.study_id
        || package_release.objective != replay_release.objective
        || package_release.plan_digest != replay_release.plan_digest
        || package_release.execution_digest != replay_release.execution_digest
        || package_release.replay_identity != replay_release.replay_identity
        || package_release.program_order != replay_release.program_order
        || package_release.negative_evidence != replay_release.negative_evidence
        || package_release.raw_data_local != replay_release.raw_data_local
        || package_release.aggregate_only != replay_release.aggregate_only
        || request.max_dependency_depth == 0
        || request.max_dependency_depth > MAX_DEPTH
        || request.required_programs.len() > MAX_REQUIRED_PROGRAMS
        || request
            .required_programs
            .iter()
            .any(|program| program.trim().is_empty())
        || (request.require_program_coverage && request.required_programs.is_empty())
    {
        return Err(GliomaLocalReleaseWorkflowError::InvalidRequest(
            "package and replay identities must match, and dependency bounds/program coverage must be coherent".into(),
        ));
    }
    super::release_gate::validate_request(&request.gate)?;
    Ok(())
}

fn validate_task_package_binding(
    tasks: &[ReplayTask],
    inputs: &[MultimodalResearchObjectInput],
) -> Result<(), GliomaLocalReleaseWorkflowError> {
    let package = inputs
        .iter()
        .map(|input| {
            (
                input.artifact.artifact_id.as_str(),
                (&input.artifact.content_hash, input.source_program.as_str()),
            )
        })
        .collect::<std::collections::BTreeMap<_, _>>();
    if tasks.len() != package.len()
        || tasks.iter().any(|task| {
            package.get(task.artifact_id.as_str())
                != Some(&(&task.expected_content_hash, task.program_id.as_str()))
        })
    {
        return Err(GliomaLocalReleaseWorkflowError::InvalidRequest(
            "replay tasks must cover each packaged artifact exactly once with its declared hash and source program".into(),
        ));
    }
    Ok(())
}

impl GliomaLocalReleaseWorkflow {
    pub fn validate(&self) -> Result<(), GliomaLocalReleaseWorkflowError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.research_id.trim().is_empty()
            || self.study_id.trim().is_empty()
            || self.phase_order
                != [
                    "multimodal_package".to_string(),
                    "dependency_closure".to_string(),
                    "replay".to_string(),
                    "release_gate".to_string(),
                    "operator_handoff".to_string(),
                ]
            || !canonical(&self.negative_evidence)
            || !canonical(&self.uncertainty)
            || self.next_operator_action != next_action(self.disposition)
            || self.bundle.manifest.research_id != self.research_id
            || self.bundle.manifest.study_id != self.study_id
            || self.dependency_closure.bundle_digest != self.bundle.digest
        {
            return Err(GliomaLocalReleaseWorkflowError::InvalidOutput(
                "identity, stage ordering, evidence ordering, or package/closure binding is invalid".into(),
            ));
        }
        self.bundle
            .validate()
            .map_err(|error| GliomaLocalReleaseWorkflowError::InvalidOutput(error.to_string()))?;
        self.dependency_closure
            .validate()
            .map_err(|error| GliomaLocalReleaseWorkflowError::InvalidOutput(error.to_string()))?;
        match (&self.campaign, &self.gate) {
            (Some(campaign), Some(gate)) => {
                campaign.validate().map_err(|error| {
                    GliomaLocalReleaseWorkflowError::InvalidOutput(error.to_string())
                })?;
                gate.validate().map_err(|error| {
                    GliomaLocalReleaseWorkflowError::InvalidOutput(error.to_string())
                })?;
                if self.dependency_closure.disposition != DependencyClosureDisposition::Closed
                    || campaign.research_id != self.research_id
                    || campaign.manifest.study_id != self.study_id
                    || campaign.manifest.manifest_digest != self.bundle.manifest.manifest_digest
                    || gate.research_id != self.research_id
                    || gate.study_id != self.study_id
                    || gate.manifest_digest != campaign.manifest.manifest_digest
                    || gate.replay_digest != campaign.digest
                {
                    return Err(GliomaLocalReleaseWorkflowError::InvalidOutput(
                        "campaign and gate must bind to the closed package identity".into(),
                    ));
                }
            }
            (None, None)
                if self.dependency_closure.disposition != DependencyClosureDisposition::Closed => {}
            _ => {
                return Err(GliomaLocalReleaseWorkflowError::InvalidOutput(
                    "replay and release-gate reports must either both exist after closure or both be absent".into(),
                ));
            }
        }
        let expected_disposition = disposition(
            &self.dependency_closure,
            self.campaign.as_ref(),
            self.gate.as_ref(),
        );
        if self.disposition != expected_disposition {
            return Err(GliomaLocalReleaseWorkflowError::InvalidOutput(
                "workflow disposition does not reconcile with closure, replay, and gate results"
                    .into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| GliomaLocalReleaseWorkflowError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(GliomaLocalReleaseWorkflowError::InvalidOutput(
                "local release workflow digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

/// Validate and compile every side-effect-free phase before a local executor is used.
pub(super) fn prepare_local_release_workflow(
    request: &GliomaLocalReleaseWorkflowRequest,
) -> Result<PreparedLocalReleaseWorkflow, GliomaLocalReleaseWorkflowError> {
    validate_request(request)?;
    let bundle = compile_glioma_multimodal_research_object(&request.package)?;
    let mut replay_request = request.replay.clone();
    replay_request.release.artifacts = request
        .package
        .inputs
        .iter()
        .map(|input| input.artifact.clone())
        .collect();
    replay_request.release.limitations = bundle.manifest.limitations.clone();
    super::replay::validate_tasks(&replay_request)?;
    validate_task_package_binding(&replay_request.tasks, &request.package.inputs)?;
    let dependency_closure =
        analyze_glioma_research_object_dependency_closure(&DependencyClosureRequest {
            objective: request.package.release.objective.clone(),
            bundle: bundle.clone(),
            max_depth: request.max_dependency_depth,
            required_programs: request.required_programs.clone(),
            require_program_coverage: request.require_program_coverage,
        })?;
    Ok(PreparedLocalReleaseWorkflow {
        bundle,
        dependency_closure,
        replay_request,
        gate_request: request.gate.clone(),
    })
}

/// Execute a preflighted workflow using the caller-owned replay executor.
pub(super) fn execute_prepared_local_release_workflow<E: ReplayCampaignExecutor>(
    prepared: PreparedLocalReleaseWorkflow,
    executor: &mut E,
) -> Result<GliomaLocalReleaseWorkflow, GliomaLocalReleaseWorkflowError> {
    let PreparedLocalReleaseWorkflow {
        bundle,
        dependency_closure,
        replay_request,
        gate_request,
    } = prepared;
    let (campaign, gate) = if dependency_closure.disposition == DependencyClosureDisposition::Closed
    {
        let campaign = execute_glioma_replay_campaign(&replay_request, executor)?;
        let gate = evaluate_glioma_release_gate(&gate_request, &campaign)?;
        (Some(campaign), Some(gate))
    } else {
        (None, None)
    };

    let mut negative_evidence = bundle
        .negative_evidence
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    negative_evidence.extend(
        dependency_closure
            .missing_upstream_order
            .iter()
            .map(|id| format!("dependency-missing-upstream:{id}")),
    );
    negative_evidence.extend(
        dependency_closure
            .cycle_order
            .iter()
            .map(|id| format!("dependency-cycle:{id}")),
    );
    negative_evidence.extend(
        dependency_closure
            .orphan_order
            .iter()
            .map(|id| format!("dependency-orphan:{id}")),
    );
    negative_evidence.extend(
        dependency_closure
            .depth_exceeded_order
            .iter()
            .map(|id| format!("dependency-depth-exceeded:{id}")),
    );
    negative_evidence.extend(
        dependency_closure
            .uncovered_program_order
            .iter()
            .map(|program| format!("uncovered-program:{program}")),
    );
    if let Some(campaign) = &campaign {
        negative_evidence.extend(campaign.negative_evidence.iter().cloned());
    }

    let mut uncertainty = bundle
        .limitations
        .iter()
        .chain(bundle.omission_order.iter())
        .chain(bundle.blocked_order.iter())
        .chain(dependency_closure.limitations.iter())
        .cloned()
        .collect::<BTreeSet<_>>();
    if let Some(campaign) = &campaign {
        uncertainty.extend(campaign.uncertainty.iter().cloned());
    }
    if let Some(gate) = &gate {
        uncertainty.extend(gate.warning_order.iter().cloned());
        uncertainty.extend(gate.blocking_order.iter().cloned());
        uncertainty.extend(gate.remediation_order.iter().cloned());
    }

    let disposition = disposition(&dependency_closure, campaign.as_ref(), gate.as_ref());
    let mut workflow = GliomaLocalReleaseWorkflow {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        research_id: bundle.manifest.research_id.clone(),
        study_id: bundle.manifest.study_id.clone(),
        phase_order: vec![
            "multimodal_package".into(),
            "dependency_closure".into(),
            "replay".into(),
            "release_gate".into(),
            "operator_handoff".into(),
        ],
        bundle,
        dependency_closure,
        campaign,
        gate,
        negative_evidence: negative_evidence.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        disposition,
        next_operator_action: next_action(disposition).into(),
        digest: ContentHash::of_bytes(b"unsealed-glioma-local-release-workflow"),
    };
    workflow.digest = ContentHash::of_value(&digest_input(&workflow))
        .map_err(|error| GliomaLocalReleaseWorkflowError::Digest(error.to_string()))?;
    workflow.validate()?;
    Ok(workflow)
}

/// Execute bundle compilation, closure, replay, and release gating for one local study.
pub fn execute_glioma_local_release_workflow<E: ReplayCampaignExecutor>(
    request: &GliomaLocalReleaseWorkflowRequest,
    executor: &mut E,
) -> Result<GliomaLocalReleaseWorkflow, GliomaLocalReleaseWorkflowError> {
    execute_prepared_local_release_workflow(prepare_local_release_workflow(request)?, executor)
}

/// Run the same workflow with a deterministic local replay executor.
pub fn execute_glioma_local_release_workflow_dry_run(
    request: &GliomaLocalReleaseWorkflowRequest,
) -> Result<GliomaLocalReleaseWorkflow, GliomaLocalReleaseWorkflowError> {
    let mut executor = DryRunReplayCampaignExecutor;
    execute_glioma_local_release_workflow(request, &mut executor)
}
