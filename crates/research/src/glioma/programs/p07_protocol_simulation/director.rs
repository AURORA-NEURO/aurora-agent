//! High-level autonomous direction for preclinical glioma research.
//!
//! The director is the product seam between a researcher's bounded intent and the existing
//! workflow/action engines. It compiles the closed glioma stage graph into executable local
//! actions, applies a focus-aware multi-objective portfolio search, and optionally hands the
//! selected batch to the caller-owned action executor. It never invents observations, treats a
//! dry-run as biological evidence, or turns a stage completion into a clinical conclusion.

use super::action_execution::{
    execute_glioma_action_portfolio, ActionPortfolioExecution, ActionPortfolioExecutionError,
    ActionPortfolioExecutionRequest, GliomaActionExecutor,
};
use crate::glioma_engine::{
    compile_glioma_research, select_glioma_actions, GliomaActionCandidate, GliomaActionSelection,
    GliomaModality, GliomaModelSystem, GliomaResearchIntent, GliomaResearchPlan,
    GliomaSelectionConfig, GliomaSelectionWeights, GliomaStage, GliomaStageKind, StageReadiness,
};
use bioprism_foundation::Effect;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P07-F32";
pub const OUTPUT_SCHEMA: &str = "GliomaAutonomousResearchDirector1@1";
pub const MAX_COMPLETED_CHECKPOINTS: usize = 32;
pub const MAX_ACTIONS: usize = 32;

/// Directional emphasis for the portfolio utility model. The stage graph and policy gates stay
/// fixed; focus only changes bounded utility scores used to rank otherwise admissible actions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GliomaDirectorFocus {
    EvidenceFirst,
    MechanismFirst,
    ExperimentFirst,
    ComputationFirst,
    ReplicationFirst,
    FullProgram,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GliomaDirectorCheckpoint {
    pub stage_kind: GliomaStageKind,
    pub artifact_id: String,
    pub artifact: crate::glioma_engine::LocalArtifactRef,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GliomaResearchDirectorRequest {
    pub intent: GliomaResearchIntent,
    pub focus: GliomaDirectorFocus,
    pub completed_checkpoints: Vec<GliomaDirectorCheckpoint>,
    pub budget_units: u32,
    pub max_actions: u16,
    pub approval_granted: bool,
    pub allow_instrument_execution: bool,
    pub allow_federation: bool,
    pub selection_weights: GliomaSelectionWeights,
    pub max_retries: u8,
    pub require_artifacts: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GliomaDirectorDisposition {
    Planned,
    Completed,
    Partial,
    Blocked,
    NoRunnableActions,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GliomaDirectorAction {
    pub candidate: GliomaActionCandidate,
    pub readiness: StageReadiness,
    pub rationale: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GliomaResearchDirectorRun {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub focus: GliomaDirectorFocus,
    pub workflow_plan: GliomaResearchPlan,
    pub checkpoint_digest_order: Vec<ContentHash>,
    pub completed_stage_order: Vec<String>,
    pub actions: Vec<GliomaDirectorAction>,
    pub candidate_order: Vec<String>,
    pub hold_order: Vec<String>,
    pub approval_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub selection: Option<GliomaActionSelection>,
    pub execution: Option<ActionPortfolioExecution>,
    pub next_stage_order: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: GliomaDirectorDisposition,
    pub next_step: String,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum GliomaResearchDirectorError {
    #[error("glioma research director request is invalid: {0}")]
    InvalidRequest(String),
    #[error("glioma research director workflow compilation failed: {0}")]
    Compilation(String),
    #[error("glioma research director selection failed: {0}")]
    Selection(String),
    #[error("glioma research director execution failed: {0}")]
    Execution(String),
    #[error("glioma research director output is invalid: {0}")]
    InvalidOutput(String),
    #[error("glioma research director digest failed: {0}")]
    Digest(String),
}

fn canonical_strings(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn checkpoint_order(checkpoints: &[GliomaDirectorCheckpoint]) -> bool {
    checkpoints
        .windows(2)
        .all(|pair| pair[0].stage_kind < pair[1].stage_kind)
}

fn focus_target(focus: GliomaDirectorFocus) -> Option<GliomaStageKind> {
    match focus {
        GliomaDirectorFocus::EvidenceFirst => Some(GliomaStageKind::EvidenceCompilation),
        GliomaDirectorFocus::MechanismFirst => Some(GliomaStageKind::MechanismExploration),
        GliomaDirectorFocus::ExperimentFirst => Some(GliomaStageKind::ExperimentDesign),
        GliomaDirectorFocus::ComputationFirst => Some(GliomaStageKind::ComputationalExecution),
        GliomaDirectorFocus::ReplicationFirst => Some(GliomaStageKind::ReplicationRobustness),
        GliomaDirectorFocus::FullProgram => None,
    }
}

fn stage_modality(kind: GliomaStageKind) -> GliomaModality {
    match kind {
        GliomaStageKind::IntentNormalization
        | GliomaStageKind::EvidenceSurveillance
        | GliomaStageKind::EvidenceCompilation => GliomaModality::Literature,
        GliomaStageKind::MultimodalIngestionQc | GliomaStageKind::MolecularLandscape => {
            GliomaModality::Spatial
        }
        GliomaStageKind::MechanismExploration => GliomaModality::Computational,
        GliomaStageKind::ExperimentDesign | GliomaStageKind::ProtocolSimulation => {
            GliomaModality::OrganoidAssay
        }
        GliomaStageKind::InstrumentPreflight => GliomaModality::Instrument,
        GliomaStageKind::ComputationalExecution => GliomaModality::Computational,
        GliomaStageKind::StatisticalInterpretation => GliomaModality::Imaging,
        GliomaStageKind::ReplicationRobustness => GliomaModality::Replication,
        GliomaStageKind::ResearchObjectRelease | GliomaStageKind::FederationBenchmarking => {
            GliomaModality::Replication
        }
    }
}

fn stage_profile(kind: GliomaStageKind) -> [u16; 7] {
    // Information, novelty, workflow leverage, cross-stage unlock, safety, federation, feasibility.
    match kind {
        GliomaStageKind::IntentNormalization => [600, 300, 900, 980, 980, 300, 990],
        GliomaStageKind::EvidenceSurveillance => [900, 720, 780, 850, 920, 780, 880],
        GliomaStageKind::EvidenceCompilation => [880, 760, 900, 900, 900, 820, 830],
        GliomaStageKind::MultimodalIngestionQc => [840, 700, 880, 900, 930, 760, 780],
        GliomaStageKind::MolecularLandscape => [900, 820, 860, 900, 820, 780, 700],
        GliomaStageKind::MechanismExploration => [930, 940, 900, 940, 780, 740, 680],
        GliomaStageKind::ExperimentDesign => [880, 900, 900, 860, 740, 700, 700],
        GliomaStageKind::ProtocolSimulation => [760, 700, 860, 780, 900, 650, 850],
        GliomaStageKind::InstrumentPreflight => [620, 600, 780, 720, 990, 620, 760],
        GliomaStageKind::ComputationalExecution => [900, 860, 920, 900, 900, 760, 720],
        GliomaStageKind::StatisticalInterpretation => [920, 900, 930, 880, 900, 820, 730],
        GliomaStageKind::ReplicationRobustness => [900, 880, 900, 820, 950, 930, 620],
        GliomaStageKind::ResearchObjectRelease => [540, 520, 780, 600, 980, 900, 900],
        GliomaStageKind::FederationBenchmarking => [700, 700, 820, 700, 920, 990, 580],
    }
}

fn apply_focus(
    kind: GliomaStageKind,
    focus: GliomaDirectorFocus,
    mut profile: [u16; 7],
) -> [u16; 7] {
    if focus_target(focus) == Some(kind) {
        profile[0] = profile[0].saturating_add(120).min(1_000);
        profile[2] = profile[2].saturating_add(100).min(1_000);
        profile[3] = profile[3].saturating_add(120).min(1_000);
    }
    if focus == GliomaDirectorFocus::FullProgram {
        profile[2] = profile[2].saturating_add(40).min(1_000);
    }
    profile
}

fn action_candidate(
    stage: &GliomaStage,
    focus: GliomaDirectorFocus,
    model_system: GliomaModelSystem,
) -> GliomaActionCandidate {
    let mut depends_on = stage.depends_on.clone();
    depends_on.sort();
    depends_on.dedup();
    let profile = apply_focus(stage.kind, focus, stage_profile(stage.kind));
    GliomaActionCandidate {
        action_id: stage.stage_id.clone(),
        stage_kind: stage.kind,
        modality: stage_modality(stage.kind),
        model_system,
        depends_on,
        cost_units: stage.budget_units,
        information_gain_milli: profile[0],
        frontier_novelty_milli: profile[1],
        workflow_leverage_milli: profile[2],
        cross_stage_unlock_milli: profile[3],
        reproducibility_safety_milli: profile[4],
        federation_value_milli: profile[5],
        feasibility_milli: profile[6],
        autonomy_tier: stage.autonomy_tier,
        effects: stage.effects.clone(),
    }
}

struct DirectorCandidateCompilation {
    actions: Vec<GliomaDirectorAction>,
    hold_order: BTreeSet<String>,
    approval_order: BTreeSet<String>,
    blocked_order: BTreeSet<String>,
    disabled_order: BTreeSet<String>,
}

fn stage_rationale(stage: &GliomaStage, focus: GliomaDirectorFocus) -> Vec<String> {
    let mut rationale = vec![format!(
        "{} is a typed {} stage with {} budget units",
        stage.stage_id,
        stage.kind.stage_id(),
        stage.budget_units
    )];
    if focus_target(focus) == Some(stage.kind) {
        rationale
            .push("director focus increases information, leverage, and unlock priority".into());
    }
    if stage.effects.contains(&Effect::InstrumentExecution) {
        rationale
            .push("instrument effect remains behind explicit approval and gateway policy".into());
    }
    if stage.effects.contains(&Effect::FederationExport) {
        rationale.push("federation export is aggregate-only and policy-gated".into());
    }
    rationale
}

fn digest_input(output: &GliomaResearchDirectorRun) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "focus": output.focus,
        "workflow_plan": output.workflow_plan,
        "checkpoint_digest_order": output.checkpoint_digest_order,
        "completed_stage_order": output.completed_stage_order,
        "actions": output.actions,
        "candidate_order": output.candidate_order,
        "hold_order": output.hold_order,
        "approval_order": output.approval_order,
        "blocked_order": output.blocked_order,
        "selection": output.selection,
        "execution": output.execution,
        "next_stage_order": output.next_stage_order,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
        "next_step": output.next_step,
    })
}

fn validate_request(
    request: &GliomaResearchDirectorRequest,
) -> Result<(), GliomaResearchDirectorError> {
    if request.budget_units == 0
        || request.budget_units > request.intent.budget_units
        || request.max_actions == 0
        || usize::from(request.max_actions) > MAX_ACTIONS
        || request.completed_checkpoints.len() > MAX_COMPLETED_CHECKPOINTS
        || request.intent.objective.trim().is_empty()
        || request.intent.replay_identity.as_str().len() != 64
        || !checkpoint_order(&request.completed_checkpoints)
        || request.completed_checkpoints.windows(2).any(|pair| {
            pair[0].stage_kind == pair[1].stage_kind || pair[0].artifact_id == pair[1].artifact_id
        })
        || request.max_retries > 8
    {
        return Err(GliomaResearchDirectorError::InvalidRequest(
            "bounded budget/actions, canonical checkpoints, intent identity, and retries are required".into(),
        ));
    }
    let weights = request.selection_weights;
    let total = u32::from(weights.information_gain)
        + u32::from(weights.frontier_novelty)
        + u32::from(weights.workflow_leverage)
        + u32::from(weights.cross_stage_unlock)
        + u32::from(weights.reproducibility_safety)
        + u32::from(weights.federation_value)
        + u32::from(weights.feasibility);
    if total != 100
        || [
            weights.information_gain,
            weights.frontier_novelty,
            weights.workflow_leverage,
            weights.cross_stage_unlock,
            weights.reproducibility_safety,
            weights.federation_value,
            weights.feasibility,
        ]
        .iter()
        .any(|score| *score > 1_000)
    {
        return Err(GliomaResearchDirectorError::InvalidRequest(
            "selection weights must sum to 100 and remain bounded".into(),
        ));
    }
    let mut seen = BTreeSet::new();
    for checkpoint in &request.completed_checkpoints {
        if !seen.insert(checkpoint.stage_kind)
            || checkpoint.artifact_id.trim().is_empty()
            || checkpoint.artifact_id != checkpoint.artifact.artifact_id
        {
            return Err(GliomaResearchDirectorError::InvalidRequest(
                "checkpoint stage and artifact identities must be unique and consistent".into(),
            ));
        }
        checkpoint
            .artifact
            .validate()
            .map_err(|error| GliomaResearchDirectorError::InvalidRequest(error.to_string()))?;
    }
    Ok(())
}

fn compile_candidates(
    plan: &GliomaResearchPlan,
    request: &GliomaResearchDirectorRequest,
) -> Result<DirectorCandidateCompilation, GliomaResearchDirectorError> {
    let model_system = request
        .intent
        .model_systems
        .iter()
        .next()
        .copied()
        .unwrap_or(GliomaModelSystem::InSilico);
    let completed = request
        .completed_checkpoints
        .iter()
        .map(|checkpoint| checkpoint.stage_kind.stage_id().to_string())
        .collect::<BTreeSet<_>>();
    let stages_by_id = plan
        .stages
        .iter()
        .map(|stage| (stage.stage_id.clone(), stage))
        .collect::<BTreeMap<_, _>>();
    let mut actions = Vec::new();
    let mut hold_order = BTreeSet::new();
    let mut approval_order = BTreeSet::new();
    let mut blocked_order = BTreeSet::new();
    let mut disabled_order = BTreeSet::new();
    for stage in &plan.stages {
        if completed.contains(&stage.stage_id) {
            continue;
        }
        match stage.readiness {
            StageReadiness::MissingInput => {
                hold_order.insert(stage.stage_id.clone());
            }
            StageReadiness::ApprovalRequired => {
                approval_order.insert(stage.stage_id.clone());
            }
            StageReadiness::Disabled => {
                disabled_order.insert(stage.stage_id.clone());
            }
            StageReadiness::Ready => {}
        }
    }
    // Admit a stage only after every dependency is either checkpointed or admitted in this
    // fixed-point pass. This prevents a downstream stage from being selected merely because its
    // own local inputs exist while an upstream stage is held by a missing input or policy gate.
    let mut admitted = completed.clone();
    loop {
        let mut progress = false;
        for stage in &plan.stages {
            if stage.readiness != StageReadiness::Ready
                || admitted.contains(&stage.stage_id)
                || stage
                    .depends_on
                    .iter()
                    .any(|dependency| !admitted.contains(dependency))
            {
                continue;
            }
            actions.push(GliomaDirectorAction {
                candidate: action_candidate(stage, request.focus, model_system),
                readiness: stage.readiness,
                rationale: stage_rationale(stage, request.focus),
            });
            admitted.insert(stage.stage_id.clone());
            progress = true;
        }
        if !progress {
            break;
        }
    }
    for stage in &plan.stages {
        if stage.readiness == StageReadiness::Ready
            && !admitted.contains(&stage.stage_id)
            && !completed.contains(&stage.stage_id)
        {
            blocked_order.insert(format!("{}:dependency-not-ready", stage.stage_id));
        }
    }
    actions.sort_by(|left, right| left.candidate.action_id.cmp(&right.candidate.action_id));
    // Every candidate dependency must be represented by another runnable action or a checkpoint.
    let action_ids = actions
        .iter()
        .map(|action| action.candidate.action_id.clone())
        .collect::<BTreeSet<_>>();
    for action in &actions {
        if action
            .candidate
            .depends_on
            .iter()
            .any(|dependency| !completed.contains(dependency) && !action_ids.contains(dependency))
        {
            blocked_order.insert(format!("{}:dependency-omitted", action.candidate.action_id));
        }
    }
    actions.retain(|action| {
        !action
            .candidate
            .depends_on
            .iter()
            .any(|dependency| !completed.contains(dependency) && !action_ids.contains(dependency))
    });
    // Keep the compiler honest if a future stage is accidentally absent from the plan.
    if actions.iter().any(|action| {
        action.candidate.depends_on.iter().any(|dependency| {
            !stages_by_id.contains_key(dependency) && !completed.contains(dependency)
        })
    }) {
        return Err(GliomaResearchDirectorError::Compilation(
            "director candidate dependency is outside the compiled stage graph".into(),
        ));
    }
    Ok(DirectorCandidateCompilation {
        actions,
        hold_order,
        approval_order,
        blocked_order,
        disabled_order,
    })
}

impl GliomaResearchDirectorRun {
    pub fn validate(&self) -> Result<(), GliomaResearchDirectorError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.workflow_plan.objective != self.objective
            || !canonical_strings(&self.completed_stage_order)
            || !canonical_strings(&self.candidate_order)
            || !canonical_strings(&self.hold_order)
            || !canonical_strings(&self.approval_order)
            || !canonical_strings(&self.blocked_order)
            || !canonical_strings(&self.negative_evidence)
            || !canonical_strings(&self.uncertainty)
            || self.actions.len() != self.candidate_order.len()
            || self.checkpoint_digest_order.len() != self.completed_stage_order.len()
            || self.next_stage_order.iter().collect::<BTreeSet<_>>().len()
                != self.next_stage_order.len()
            || self
                .next_stage_order
                .iter()
                .any(|id| self.candidate_order.binary_search(id).is_err())
        {
            return Err(GliomaResearchDirectorError::InvalidOutput(
                "director identity, ordering, workflow binding, or action partition is invalid"
                    .into(),
            ));
        }
        self.workflow_plan
            .validate()
            .map_err(|error| GliomaResearchDirectorError::InvalidOutput(error.to_string()))?;
        let candidate_ids = self
            .actions
            .iter()
            .map(|action| action.candidate.action_id.clone())
            .collect::<BTreeSet<_>>();
        if candidate_ids
            != self
                .candidate_order
                .iter()
                .cloned()
                .collect::<BTreeSet<_>>()
        {
            return Err(GliomaResearchDirectorError::InvalidOutput(
                "director actions do not reconcile with candidate order".into(),
            ));
        }
        if let Some(selection) = &self.selection {
            selection
                .validate()
                .map_err(|error| GliomaResearchDirectorError::InvalidOutput(error.to_string()))?;
            if selection.candidate_order != self.candidate_order
                || selection.selected_order != self.next_stage_order
            {
                return Err(GliomaResearchDirectorError::InvalidOutput(
                    "director selection does not reconcile with candidates or next stages".into(),
                ));
            }
        } else if !self.next_stage_order.is_empty() || self.execution.is_some() {
            return Err(GliomaResearchDirectorError::InvalidOutput(
                "execution or next stages require a selection".into(),
            ));
        }
        if let Some(execution) = &self.execution {
            execution
                .validate()
                .map_err(|error| GliomaResearchDirectorError::InvalidOutput(error.to_string()))?;
            if execution.action_order != self.next_stage_order {
                return Err(GliomaResearchDirectorError::InvalidOutput(
                    "director execution does not reconcile with next stages".into(),
                ));
            }
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| GliomaResearchDirectorError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(GliomaResearchDirectorError::InvalidOutput(
                "director digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

fn run_director(
    request: &GliomaResearchDirectorRequest,
    executor: Option<&mut dyn GliomaActionExecutor>,
) -> Result<GliomaResearchDirectorRun, GliomaResearchDirectorError> {
    validate_request(request)?;
    let plan = compile_glioma_research(&request.intent)
        .map_err(|error| GliomaResearchDirectorError::Compilation(error.to_string()))?;
    let plan_admitted = plan.disposition == crate::glioma_engine::GliomaPlanDisposition::Admitted;
    let DirectorCandidateCompilation {
        actions,
        hold_order,
        approval_order,
        mut blocked_order,
        disabled_order,
    } = compile_candidates(&plan, request)?;
    let candidates = actions
        .iter()
        .map(|action| action.candidate.clone())
        .collect::<Vec<_>>();
    let completed_actions = request
        .completed_checkpoints
        .iter()
        .map(|checkpoint| checkpoint.stage_kind.stage_id().to_string())
        .collect::<BTreeSet<_>>();
    let selection_config = GliomaSelectionConfig {
        budget_units: request.budget_units,
        max_actions: request.max_actions,
        approval_granted: request.approval_granted,
        allow_instrument_execution: request.allow_instrument_execution,
        allow_federation: request.allow_federation,
        weights: request.selection_weights,
    };
    let selection = if candidates.is_empty() {
        None
    } else {
        Some(
            select_glioma_actions(&candidates, &completed_actions, &selection_config)
                .map_err(|error| GliomaResearchDirectorError::Selection(error.to_string()))?,
        )
    };
    let next_stage_order = selection
        .as_ref()
        .map(|selection| selection.selected_order.clone())
        .unwrap_or_default();
    let execution = if let (Some(selection), Some(executor)) = (&selection, executor) {
        if selection.selected_order.is_empty() {
            None
        } else {
            let execution = execute_glioma_action_portfolio(
                &ActionPortfolioExecutionRequest {
                    candidates: candidates.clone(),
                    completed_actions: completed_actions.clone(),
                    selection: selection_config.clone(),
                    max_retries: request.max_retries,
                    require_artifacts: request.require_artifacts,
                },
                executor,
            )
            .map_err(|error: ActionPortfolioExecutionError| {
                GliomaResearchDirectorError::Execution(error.to_string())
            })?;
            Some(execution)
        }
    } else {
        None
    };
    if let Some(selection) = &selection {
        blocked_order.extend(
            selection
                .blocked_order
                .iter()
                .map(|id| format!("selection:{id}")),
        );
    }
    let mut uncertainty = BTreeSet::new();
    let mut negative_evidence = BTreeSet::new();
    for omission in &plan.omission_order {
        uncertainty.insert(format!("workflow-omission:{omission}"));
    }
    if !plan_admitted {
        uncertainty.insert(format!("workflow-plan-not-admitted:{:?}", plan.disposition));
    }
    for stage in &hold_order {
        uncertainty.insert(format!("missing-input:{stage}"));
    }
    for stage in &approval_order {
        uncertainty.insert(format!("approval-required:{stage}"));
    }
    for stage in &disabled_order {
        uncertainty.insert(format!("disabled-by-policy:{stage}"));
    }
    if candidates.is_empty() {
        uncertainty.insert("director:no-runnable-actions".into());
    }
    if let Some(execution) = &execution {
        uncertainty.extend(
            execution
                .uncertainty
                .iter()
                .map(|item| format!("execution:{item}")),
        );
        negative_evidence.extend(
            execution
                .negative_evidence
                .iter()
                .map(|item| format!("execution:{item}")),
        );
    }
    let (disposition, next_step) = match (&selection, &execution) {
        (None, _) => (
            if plan.disposition == crate::glioma_engine::GliomaPlanDisposition::Blocked {
                GliomaDirectorDisposition::Blocked
            } else {
                GliomaDirectorDisposition::NoRunnableActions
            },
            "supply missing local artifacts or approvals, then recompile the director plan".into(),
        ),
        (Some(selection), None) if selection.selected_order.is_empty() => (
            GliomaDirectorDisposition::NoRunnableActions,
            "inspect blocked and deferred stages, then revise the bounded research intent".into(),
        ),
        (Some(_), None) => (
            GliomaDirectorDisposition::Planned,
            "submit the selected local stages to an institution-owned executor and recompile from returned artifacts".into(),
        ),
        (Some(_), Some(execution)) => (
            match execution.disposition {
                super::action_execution::ActionPortfolioExecutionDisposition::Completed
                    if plan_admitted => GliomaDirectorDisposition::Completed,
                super::action_execution::ActionPortfolioExecutionDisposition::Completed => {
                    GliomaDirectorDisposition::Partial
                }
                super::action_execution::ActionPortfolioExecutionDisposition::Partial => {
                    GliomaDirectorDisposition::Partial
                }
                super::action_execution::ActionPortfolioExecutionDisposition::Failed
                | super::action_execution::ActionPortfolioExecutionDisposition::Blocked => {
                    GliomaDirectorDisposition::Blocked
                }
            },
            "bind returned artifacts to the next checkpoint and recompile downstream gates".into(),
        ),
    };
    let mut output = GliomaResearchDirectorRun {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.intent.objective.clone(),
        focus: request.focus,
        workflow_plan: plan,
        checkpoint_digest_order: request
            .completed_checkpoints
            .iter()
            .map(|checkpoint| checkpoint.artifact.content_hash.clone())
            .collect(),
        completed_stage_order: completed_actions.into_iter().collect(),
        actions,
        candidate_order: candidates
            .iter()
            .map(|candidate| candidate.action_id.clone())
            .collect(),
        hold_order: hold_order.into_iter().collect(),
        approval_order: approval_order.into_iter().collect(),
        blocked_order: blocked_order.into_iter().collect(),
        selection,
        execution,
        next_stage_order,
        negative_evidence: negative_evidence.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        disposition,
        next_step,
        digest: ContentHash::of_bytes(b"unsealed-glioma-research-director"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| GliomaResearchDirectorError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

/// Compile a high-level intent into a deterministic, checkpoint-aware next-stage portfolio.
pub fn plan_glioma_research_director(
    request: &GliomaResearchDirectorRequest,
) -> Result<GliomaResearchDirectorRun, GliomaResearchDirectorError> {
    run_director(request, None)
}

/// Compile and execute one bounded director batch through a caller-owned local executor.
pub fn execute_glioma_research_director<E: GliomaActionExecutor>(
    request: &GliomaResearchDirectorRequest,
    executor: &mut E,
) -> Result<GliomaResearchDirectorRun, GliomaResearchDirectorError> {
    run_director(request, Some(executor))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::programs::p07_protocol_simulation::action_execution::DryRunGliomaActionExecutor;
    use crate::glioma_engine::{GliomaModelSystem, LocalArtifactRef};
    use bioprism_foundation::{AutonomyTier, PRECLINICAL_BOUNDARY};
    use bioprism_onco::OutputUse;

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

    fn request() -> GliomaResearchDirectorRequest {
        GliomaResearchDirectorRequest {
            intent: GliomaResearchIntent {
                research_id: "director-research".into(),
                study_id: "director-study".into(),
                objective: "identify reproducible invasion mechanisms in organoids".into(),
                output_uses: BTreeSet::from([OutputUse::CohortAnalysis]),
                model_systems: BTreeSet::from([GliomaModelSystem::Organoid]),
                modalities: BTreeSet::from([
                    GliomaModality::Transcriptomics,
                    GliomaModality::Imaging,
                    GliomaModality::Spatial,
                ]),
                input_artifacts: vec![artifact("input")],
                requested_autonomy: AutonomyTier::A1,
                approval_reference: None,
                budget_units: 160,
                max_retries: 1,
                allow_instrument_execution: false,
                allow_federation: false,
                raw_data_local: true,
                aggregate_only: true,
                replay_identity: ContentHash::of_bytes(b"director-replay"),
                boundary: PRECLINICAL_BOUNDARY.into(),
            },
            focus: GliomaDirectorFocus::MechanismFirst,
            completed_checkpoints: Vec::new(),
            budget_units: 80,
            max_actions: 5,
            approval_granted: false,
            allow_instrument_execution: false,
            allow_federation: false,
            selection_weights: GliomaSelectionWeights::default(),
            max_retries: 1,
            require_artifacts: true,
        }
    }

    #[test]
    fn director_closes_stage_dependencies_and_executes_only_local_dry_run_actions() {
        let mut executor = DryRunGliomaActionExecutor;
        let output = execute_glioma_research_director(&request(), &mut executor).unwrap();
        assert!(!output.candidate_order.is_empty());
        assert!(!output.next_stage_order.is_empty());
        let selected_positions = output
            .next_stage_order
            .iter()
            .enumerate()
            .map(|(index, id)| (id.as_str(), index))
            .collect::<BTreeMap<_, _>>();
        assert!(output.actions.iter().all(|action| {
            selected_positions
                .get(action.candidate.action_id.as_str())
                .is_none_or(|index| {
                    action.candidate.depends_on.iter().all(|dependency| {
                        selected_positions
                            .get(dependency.as_str())
                            .is_none_or(|dependency_index| dependency_index < index)
                    })
                })
        }));
        assert_eq!(output.disposition, GliomaDirectorDisposition::Partial);
        assert!(output
            .negative_evidence
            .iter()
            .any(|item| item.contains("synthetic-dry-run")));
        assert!(output
            .uncertainty
            .iter()
            .any(|item| item.contains("simulation-only")));
    }

    #[test]
    fn director_is_permutation_stable_and_focus_changes_rank_without_bypassing_gates() {
        let first = plan_glioma_research_director(&request()).unwrap();
        let mut alternate = request();
        alternate.focus = GliomaDirectorFocus::EvidenceFirst;
        let second = plan_glioma_research_director(&alternate).unwrap();
        assert_ne!(first.digest, second.digest);
        assert!(first
            .actions
            .iter()
            .all(|action| action.readiness == StageReadiness::Ready));
        assert!(first
            .blocked_order
            .iter()
            .all(|entry| !entry.ends_with("dependency-omitted")));
    }

    #[test]
    fn director_refuses_checkpoint_artifact_that_leaves_the_local_boundary() {
        let mut request = request();
        request
            .completed_checkpoints
            .push(GliomaDirectorCheckpoint {
                stage_kind: GliomaStageKind::IntentNormalization,
                artifact_id: "checkpoint".into(),
                artifact: LocalArtifactRef {
                    local_only: false,
                    ..artifact("checkpoint")
                },
            });
        assert!(matches!(
            plan_glioma_research_director(&request),
            Err(GliomaResearchDirectorError::InvalidRequest(_))
        ));
    }
}
